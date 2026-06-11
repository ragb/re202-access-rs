//! [`Device`] implementation for the BOSS RE-202 — the adapter that plugs the
//! RE-202's typed Roland-SysEx codec into the generic CLI engine
//! (`midi-access-cli`) and editor tooling, using `serde_yaml::Value` as the
//! document lingua franca.
//!
//! ## Areas
//!
//! The engine addresses one editable document per *area* token. The RE-202 has a
//! global System block plus 128 interchangeable 33-byte memory blocks (MANUAL +
//! 127 user slots) and a live edit-buffer mirror, so the areas are:
//!
//! - `system` — the global System area (18 bytes, `10 00 00 00`).
//! - `memory` — MEMORY MANUAL, the live manual-mode patch (`20 10 00 00`).
//! - `memory-1` … `memory-127` — the 127 stored user slots.
//! - `edit` — the edit-buffer mirror of the currently-active memory (`20 00 00 00`).
//!
//! `memory`, `edit`, and every `memory-N` share the one `Memory` codec and the
//! one `re202-memory` JSON Schema; only their target address differs. This keeps
//! per-slot `dump`/`sync` available through the engine's fixed subcommand set
//! (areas are just tokens). Two pre-migration conveniences have **no** equivalent
//! in the generic engine and are dropped as device-specific gaps: `--all` (bulk
//! dump/sync of every slot in one command) and `select` (a Program-Change slot
//! switch). The wasm/editor layer keeps full per-slot addressing.
//!
//! ## Wire framing
//!
//! Roland framing (`F0 41 <dev> 00 00 00 00 18 12 …` data-set / `11` request) and
//! the Address+Data checksum live in [`crate::sysex`]; this impl only wraps it.
//! `request`/`encode` return the SysEx frame(s) for an operation; `decode` splits
//! a collected dump stream and reads the first matching data-set block.

use std::sync::OnceLock;

use serde_yaml::Value;

use midi_access_core::{Area, Catalogs, Device, DeviceError, Inbound, Params};

use crate::address::{
    AddressSpace, MemorySlot, EDIT_BUFFER_BASE, MEMORY_BLOCK_LEN, MEMORY_SLOT_MAX, SYSTEM_BASE,
};
use crate::sysex::{Frame, CMD_DT1};
use crate::system::SYSTEM_AREA_LEN;
use crate::{classify_inbound as core_classify_inbound, InboundMessage, Memory, SystemArea};

/// The BOSS RE-202 Space Echo.
pub struct Re202;

/// What a given area targets on the wire.
#[derive(Debug, Clone, Copy)]
enum Target {
    /// The 18-byte System area.
    System,
    /// A 33-byte memory block at the given base address.
    Memory([u8; 4]),
}

/// Build the area table once: `system`, `memory` (= MANUAL), `edit`, and one
/// entry per user slot `memory-1` … `memory-127`. The per-slot names are leaked
/// to `'static` (a single one-time allocation for a process-lifetime table).
fn build_areas() -> Vec<Area> {
    let mut v = vec![
        Area {
            name: "system",
            label: "System area",
            about: "Global settings (input, controls, MIDI, reverb type) — 18 bytes",
        },
        Area {
            name: "memory",
            label: "Memory block",
            about: "MEMORY MANUAL — the live manual-mode patch (33 bytes)",
        },
        Area {
            name: "edit",
            label: "Memory block",
            about: "Edit-buffer mirror of the currently-active memory (33 bytes)",
        },
    ];
    for n in 1..=MEMORY_SLOT_MAX {
        let name: &'static str = Box::leak(format!("memory-{n}").into_boxed_str());
        v.push(Area {
            name,
            label: "Memory block",
            about: "One stored user memory slot (33 bytes)",
        });
    }
    v
}

/// Resolve a (canonical) area name to its wire target. The engine resolves CLI
/// tokens to canonical names via [`Area::matches`] before calling the device, so
/// matching the canonical names here is sufficient.
fn target_for(area: &str) -> Option<Target> {
    match area {
        "system" => Some(Target::System),
        "memory" => Some(Target::Memory(MemorySlot::Manual.base_address())),
        "edit" => Some(Target::Memory(EDIT_BUFFER_BASE)),
        other => {
            let n: u8 = other.strip_prefix("memory-")?.parse().ok()?;
            match MemorySlot::from_index(n)? {
                slot @ MemorySlot::User(_) => Some(Target::Memory(slot.base_address())),
                MemorySlot::Manual => None, // `memory-0` is not a thing
            }
        }
    }
}

/// Best-effort reverse map: which area an inbound dump's address belongs to
/// (informational, for [`Inbound::Dump`]).
fn area_for_address(address: [u8; 4]) -> Option<String> {
    match AddressSpace::classify(address) {
        AddressSpace::System => Some("system".to_string()),
        AddressSpace::Memory => {
            if address == EDIT_BUFFER_BASE {
                Some("edit".to_string())
            } else if address == MemorySlot::Manual.base_address() {
                Some("memory".to_string())
            } else {
                (1..=MEMORY_SLOT_MAX)
                    .find(|&n| MemorySlot::User(n).base_address() == address)
                    .map(|n| format!("memory-{n}"))
            }
        }
        AddressSpace::Unknown => None,
    }
}

/// Roland device id for a 0..=15 channel (`0x10 + ch`). The engine validates the
/// channel is in range before any device call; offline paths pass `ch = 0`.
fn roland_device_id(ch: u8) -> u8 {
    0x10u8.wrapping_add(ch)
}

/// Encode a byte count as Roland's 4-byte, 7-bit big-endian size field.
fn size_field(n: usize) -> [u8; 4] {
    let v = n as u32;
    [
        ((v >> 21) & 0x7F) as u8,
        ((v >> 14) & 0x7F) as u8,
        ((v >> 7) & 0x7F) as u8,
        (v & 0x7F) as u8,
    ]
}

/// Find the data of the first DT1 (data-set) frame in `dump` at `expected`.
///
/// Matching the exact address (rather than an address-*space*) mirrors the
/// pre-migration `MidiSession::request`, and sidesteps the fact that
/// [`AddressSpace::classify`] doesn't recognise the carried high bytes of user
/// slots 7..=127 (`0x21`..`0x2F`).
fn first_block_data(dump: &[u8], expected: [u8; 4]) -> Result<Vec<u8>, DeviceError> {
    midi_access_core::split_sysex(dump)
        .into_iter()
        .filter_map(|frame| Frame::decode(&frame).ok())
        .find(|f| f.command == CMD_DT1 && f.address == expected)
        .map(|f| f.data)
        .ok_or_else(|| DeviceError::Decode(format!("no data-set frame at {expected:02X?} in dump")))
}

fn dec(e: impl std::fmt::Display) -> DeviceError {
    DeviceError::Decode(e.to_string())
}
fn enc(e: impl std::fmt::Display) -> DeviceError {
    DeviceError::Encode(e.to_string())
}
fn unknown(area: &str) -> DeviceError {
    DeviceError::UnknownArea(area.to_string())
}

impl Device for Re202 {
    const NAME: &'static str = "re202";

    fn areas() -> &'static [Area] {
        static AREAS: OnceLock<Vec<Area>> = OnceLock::new();
        AREAS.get_or_init(build_areas).as_slice()
    }

    fn params() -> Params {
        crate::catalog::params()
    }

    fn catalogs() -> &'static dyn Catalogs {
        &crate::catalog::RE202_CATALOGS
    }

    fn defaults(_area: &str) -> Option<Value> {
        // The codec carries no factory-default document; nothing to emit.
        None
    }

    fn schema(area: &str) -> Option<String> {
        #[cfg(feature = "schema")]
        {
            use midi_access_core::schema::schema_json;
            match target_for(area)? {
                Target::System => Some(schema_json::<SystemArea>()),
                Target::Memory(_) => Some(schema_json::<Memory>()),
            }
        }
        #[cfg(not(feature = "schema"))]
        {
            let _ = area;
            None
        }
    }

    fn request(area: &str, ch: u8) -> Result<Vec<u8>, DeviceError> {
        let device_id = roland_device_id(ch);
        let (address, len) = match target_for(area).ok_or_else(|| unknown(area))? {
            Target::System => (SYSTEM_BASE, SYSTEM_AREA_LEN),
            Target::Memory(addr) => (addr, MEMORY_BLOCK_LEN),
        };
        Ok(Frame::data_request(device_id, address, size_field(len)).encode())
    }

    fn decode(area: &str, dump: &[u8]) -> Result<Value, DeviceError> {
        match target_for(area).ok_or_else(|| unknown(area))? {
            Target::System => {
                let data = first_block_data(dump, SYSTEM_BASE)?;
                let system = SystemArea::from_bytes(&data).map_err(dec)?;
                serde_yaml::to_value(system).map_err(dec)
            }
            Target::Memory(addr) => {
                let data = first_block_data(dump, addr)?;
                let memory = Memory::from_bytes(&data).map_err(dec)?;
                serde_yaml::to_value(memory).map_err(dec)
            }
        }
    }

    fn encode(area: &str, doc: &Value, ch: u8) -> Result<Vec<u8>, DeviceError> {
        let device_id = roland_device_id(ch);
        match target_for(area).ok_or_else(|| unknown(area))? {
            Target::System => {
                let system: SystemArea = serde_yaml::from_value(doc.clone()).map_err(enc)?;
                let bytes = system.to_bytes().map_err(enc)?;
                Ok(Frame::data_set(device_id, SYSTEM_BASE, bytes.to_vec()).encode())
            }
            Target::Memory(addr) => {
                let memory: Memory = serde_yaml::from_value(doc.clone()).map_err(enc)?;
                let bytes = memory.to_bytes().map_err(enc)?;
                Ok(Frame::data_set(device_id, addr, bytes.to_vec()).encode())
            }
        }
    }

    fn classify_inbound(bytes: &[u8]) -> Inbound {
        // The generic engine's `identity` command looks for `Inbound::Identity`;
        // the device's own `classify_inbound` treats a Universal Identity Reply as
        // unparseable SysEx (it isn't a Roland data frame), so detect it here.
        if let Some(model) = classify_identity(bytes) {
            return Inbound::Identity {
                bytes: bytes.to_vec(),
                model,
            };
        }
        match core_classify_inbound(bytes) {
            InboundMessage::SystemDataSet { address, data, .. }
            | InboundMessage::MemoryDataSet { address, data, .. }
            | InboundMessage::UnknownDataSet { address, data, .. } => Inbound::Dump {
                area: area_for_address(address),
                address: address.to_vec(),
                data,
            },
            InboundMessage::DataRequest { address, .. } => Inbound::Request {
                address: address.to_vec(),
            },
            InboundMessage::UnparseableSysEx { bytes, .. } => Inbound::Other(bytes),
            InboundMessage::NonSysEx(bytes) => Inbound::Other(bytes),
        }
    }

    fn accepts(area: &str, doc: &Value) -> bool {
        // Parse-level kind check (matches the pre-migration `show`/`lint`): a file
        // may deserialize into the typed model yet fail to byte-encode.
        match target_for(area) {
            Some(Target::System) => serde_yaml::from_value::<SystemArea>(doc.clone()).is_ok(),
            Some(Target::Memory(_)) => serde_yaml::from_value::<Memory>(doc.clone()).is_ok(),
            None => false,
        }
    }
}

/// Detect a Universal Identity Reply (`F0 7E <dev> 06 02 …`). Returns the matched
/// model (`Some("RE-202")` when the Roland family bytes match, else `None`)
/// wrapped in an outer `Some` to signal "this *is* an identity reply".
fn classify_identity(bytes: &[u8]) -> Option<Option<String>> {
    let is_identity_reply = bytes.len() >= 6
        && bytes[0] == 0xF0
        && bytes[1] == 0x7E
        && bytes[3] == 0x06
        && bytes[4] == 0x02;
    if !is_identity_reply {
        return None;
    }
    // Roland (0x41), family 0x0418 (low byte 0x18, high byte 0x04).
    let is_re202 = bytes.len() >= 8 && bytes[5] == 0x41 && bytes[6] == 0x18 && bytes[7] == 0x04;
    Some(is_re202.then(|| "RE-202".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_doc() -> Value {
        let bytes = [
            0x00, 0x01, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x04, 0x01, 0x11, 0x01, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ];
        serde_yaml::to_value(SystemArea::from_bytes(&bytes).unwrap()).unwrap()
    }

    fn memory_doc() -> Value {
        let bytes = [
            0x00, 0x04, 0x4D, 0x4D, 0x4D, 0x36, 0x36, 0x36, 0x60, 0x3F, 0x7F, 0x3F, 0x3F, 0x3F,
            0x3E, 0x3E, 0x3E, 0x06, 0x06, 0x06, 0x00, 0x00, 0x00, 0x52, 0x52, 0x52, 0x01, 0x00,
            0x00, 0x01, 0x0F, 0x04, 0x00,
        ];
        serde_yaml::to_value(Memory::from_bytes(&bytes).unwrap()).unwrap()
    }

    #[test]
    fn areas_cover_system_memory_edit_and_127_slots() {
        let areas = Re202::areas();
        // system + memory + edit + memory-1..=127
        assert_eq!(areas.len(), 3 + 127);
        assert!(areas.iter().any(|a| a.name == "system"));
        assert!(areas.iter().any(|a| a.name == "memory"));
        assert!(areas.iter().any(|a| a.name == "edit"));
        assert!(areas.iter().any(|a| a.name == "memory-1"));
        assert!(areas.iter().any(|a| a.name == "memory-127"));
        // Labels drive `show`/`lint` wording.
        assert_eq!(
            areas.iter().find(|a| a.name == "system").unwrap().label,
            "System area"
        );
        assert_eq!(
            areas.iter().find(|a| a.name == "memory").unwrap().label,
            "Memory block"
        );
    }

    #[test]
    fn target_mapping() {
        assert!(matches!(target_for("system"), Some(Target::System)));
        assert!(matches!(
            target_for("memory"),
            Some(Target::Memory(a)) if a == [0x20, 0x10, 0x00, 0x00]
        ));
        assert!(matches!(
            target_for("edit"),
            Some(Target::Memory(a)) if a == EDIT_BUFFER_BASE
        ));
        assert!(matches!(
            target_for("memory-1"),
            Some(Target::Memory(a)) if a == [0x20, 0x20, 0x00, 0x00]
        ));
        assert!(matches!(
            target_for("memory-127"),
            Some(Target::Memory(a)) if a == [0x30, 0x00, 0x00, 0x00]
        ));
        assert!(target_for("memory-0").is_none());
        assert!(target_for("memory-128").is_none());
        assert!(target_for("bogus").is_none());
    }

    #[test]
    fn system_round_trips_through_value() {
        let doc = system_doc();
        let bytes = Re202::encode("system", &doc, 0).unwrap();
        let back = Re202::decode("system", &bytes).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn memory_round_trips_through_value_for_every_target_kind() {
        let doc = memory_doc();
        for area in ["memory", "edit", "memory-42", "memory-127"] {
            let bytes = Re202::encode(area, &doc, 0).unwrap();
            let back = Re202::decode(area, &bytes).unwrap();
            assert_eq!(back, doc, "round trip for area {area}");
        }
    }

    #[test]
    fn request_uses_roland_framing_and_device_id() {
        let req = Re202::request("system", 0).unwrap();
        // F0 41 10 00 00 00 00 18 11 [10 00 00 00] [00 00 00 12] chk F7
        assert_eq!(req[0], 0xF0);
        assert_eq!(req[1], 0x41); // Roland
        assert_eq!(req[2], 0x10); // device id 0x10 + ch 0
        assert_eq!(req[8], crate::sysex::CMD_RQ1);
        assert_eq!(&req[9..13], &[0x10, 0x00, 0x00, 0x00]); // System base
        assert_eq!(&req[13..17], &[0x00, 0x00, 0x00, 0x12]); // size 18
                                                             // ch shifts the device id.
        let req5 = Re202::request("memory", 5).unwrap();
        assert_eq!(req5[2], 0x15);
        assert_eq!(&req5[9..13], &[0x20, 0x10, 0x00, 0x00]); // MANUAL base
    }

    #[test]
    fn accepts_distinguishes_system_and_memory() {
        assert!(Re202::accepts("system", &system_doc()));
        assert!(!Re202::accepts("system", &memory_doc()));
        assert!(Re202::accepts("memory", &memory_doc()));
        assert!(Re202::accepts("memory-9", &memory_doc()));
        assert!(!Re202::accepts("memory", &system_doc()));
        assert!(!Re202::accepts("bogus", &memory_doc()));
    }

    #[test]
    fn classify_inbound_recognizes_re202_identity() {
        let reply = [
            0xF0, 0x7E, 0x10, 0x06, 0x02, 0x41, 0x18, 0x04, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0xF7,
        ];
        match Re202::classify_inbound(&reply) {
            Inbound::Identity { model, .. } => assert_eq!(model.as_deref(), Some("RE-202")),
            other => panic!("expected Identity, got {other:?}"),
        }
        // A non-Roland identity reply is still an Identity, but unrecognized.
        let other_reply = [0xF0, 0x7E, 0x10, 0x06, 0x02, 0x42, 0x00, 0x00, 0xF7];
        match Re202::classify_inbound(&other_reply) {
            Inbound::Identity { model, .. } => assert_eq!(model, None),
            other => panic!("expected Identity, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_maps_data_sets_to_dumps() {
        // DT1 to System / Input Source.
        let bytes = [
            0xF0, 0x41, 0x10, 0x00, 0x00, 0x00, 0x00, 0x18, 0x12, 0x10, 0x00, 0x00, 0x00, 0x00,
            0x70, 0xF7,
        ];
        match Re202::classify_inbound(&bytes) {
            Inbound::Dump { area, .. } => assert_eq!(area.as_deref(), Some("system")),
            other => panic!("expected Dump, got {other:?}"),
        }
    }

    #[test]
    fn unknown_area_errors() {
        assert!(matches!(
            Re202::request("nope", 0),
            Err(DeviceError::UnknownArea(_))
        ));
        assert!(matches!(
            Re202::decode("nope", &[]),
            Err(DeviceError::UnknownArea(_))
        ));
    }
}
