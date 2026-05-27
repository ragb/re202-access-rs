//! Round-trip tests against real RE-202 SysEx captures.
//!
//! Each fixture in `tests/fixtures/*.syx` is a verbatim byte capture from a
//! physical RE-202 (firmware version reporting `00 00 00 00`, device id 0x10).
//! These tests assert the codec decodes the bytes correctly AND re-encodes
//! them to the identical byte sequence.

use std::path::Path;

use re202_core::address::{
    AddressSpace, MemorySlot, EDIT_BUFFER_BASE, MEMORY_BLOCK_LEN, MEMORY_MANUAL_BASE,
};
use re202_core::{classify_inbound, Frame, InboundMessage, CMD_DT1};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

#[test]
fn system_input_source_guitar_round_trips() {
    let bytes = fixture("system_input-source_guitar.syx");
    let frame = Frame::decode(&bytes).expect("decode");
    assert_eq!(frame.device_id, 0x10);
    assert_eq!(frame.command, CMD_DT1);
    assert_eq!(frame.address, [0x10, 0x00, 0x00, 0x00]);
    assert_eq!(frame.data, vec![0x00]); // Guitar
    assert_eq!(frame.encode(), bytes);
}

#[test]
fn system_full_dump_decodes_to_18_bytes() {
    let bytes = fixture("system_full-dump_defaultish.syx");
    let frame = Frame::decode(&bytes).expect("decode");
    assert_eq!(frame.command, CMD_DT1);
    assert_eq!(frame.address, [0x10, 0x00, 0x00, 0x00]);
    assert_eq!(
        frame.data.len(),
        18,
        "System area is documented as 18 bytes (offsets 0x00..0x11)"
    );

    // Spot-check a few values against what we know about this capture.
    assert_eq!(frame.data[0x00], 0x00, "Input Source = Guitar");
    assert_eq!(frame.data[0x09], 0x01, "MIDI Rx Channel = 1");
    assert_eq!(
        frame.data[0x11], 0x01,
        "MIDI Thru = ON (explains echo behavior)"
    );

    assert_eq!(frame.encode(), bytes);
}

#[test]
fn memory_001_decodes_to_33_bytes() {
    let bytes = fixture("memory_001_dump.syx");
    let frame = Frame::decode(&bytes).expect("decode");
    assert_eq!(frame.command, CMD_DT1);
    assert_eq!(
        frame.address,
        MemorySlot::User(1).base_address(),
        "MEMORY 1 lives at 0x20 0x20 0x00 0x00"
    );
    assert_eq!(
        frame.data.len(),
        MEMORY_BLOCK_LEN,
        "per-memory block is 33 bytes (offsets 0x00..0x20)"
    );

    // Tap Time nibble packing: bytes at offsets 0x1C..0x1F = 0x00 0x01 0x0F 0x04
    // → nibble-MSB→LSB → 0x01F4 = 500 ms (120 BPM).
    let t = ((frame.data[0x1C] as u16) << 12)
        | ((frame.data[0x1D] as u16) << 8)
        | ((frame.data[0x1E] as u16) << 4)
        | (frame.data[0x1F] as u16);
    assert_eq!(t, 500, "Tap Time should unpack to 500 ms");

    assert_eq!(frame.encode(), bytes);
}

#[test]
fn memory_manual_decodes_to_33_bytes() {
    let bytes = fixture("memory_manual_dump.syx");
    let frame = Frame::decode(&bytes).expect("decode");
    assert_eq!(frame.address, MEMORY_MANUAL_BASE);
    assert_eq!(frame.data.len(), MEMORY_BLOCK_LEN);
    assert_eq!(frame.encode(), bytes);
}

#[test]
fn memory_127_decodes_to_33_bytes() {
    let bytes = fixture("memory_127_dump.syx");
    let frame = Frame::decode(&bytes).expect("decode");
    assert_eq!(frame.address, MemorySlot::User(127).base_address());
    assert_eq!(frame.address, [0x30, 0x00, 0x00, 0x00]);
    assert_eq!(frame.data.len(), MEMORY_BLOCK_LEN);
    assert_eq!(frame.encode(), bytes);
}

#[test]
fn memory_002_decodes_with_user_programmed_values() {
    let bytes = fixture("memory_002_dump.syx");
    let frame = Frame::decode(&bytes).expect("decode");
    assert_eq!(frame.address, MemorySlot::User(2).base_address());
    assert_eq!(frame.address, [0x20, 0x30, 0x00, 0x00]);
    assert_eq!(frame.data.len(), MEMORY_BLOCK_LEN);
    // Tape = AGED, Mode = 0, distinct from MEMORY 1 / MANUAL / 127.
    assert_eq!(frame.data[0x00], 0x01);
    assert_eq!(frame.data[0x01], 0x00);
    assert_eq!(frame.encode(), bytes);
}

/// The edit-buffer mirror at 20 00 00 00 was captured when MEMORY 2 was the
/// active slot; therefore the data is identical to MEMORY 2's payload.
#[test]
fn edit_buffer_mirrors_active_memory() {
    let edit_bytes = fixture("edit_buffer_mirrors_memory2.syx");
    let m2_bytes = fixture("memory_002_dump.syx");

    let edit = Frame::decode(&edit_bytes).expect("decode edit");
    let m2 = Frame::decode(&m2_bytes).expect("decode mem2");

    assert_eq!(edit.address, EDIT_BUFFER_BASE);
    assert_eq!(edit.data.len(), MEMORY_BLOCK_LEN);
    // The discriminator: the edit buffer's data == active memory's data.
    assert_eq!(edit.data, m2.data);
    assert_eq!(edit.encode(), edit_bytes);
}

#[test]
fn classify_inbound_recognises_each_fixture() {
    for name in [
        "system_input-source_guitar.syx",
        "system_full-dump_defaultish.syx",
    ] {
        assert!(matches!(
            classify_inbound(&fixture(name)),
            InboundMessage::SystemDataSet { .. }
        ));
    }
    for name in [
        "memory_001_dump.syx",
        "memory_002_dump.syx",
        "memory_127_dump.syx",
        "memory_manual_dump.syx",
        "edit_buffer_mirrors_memory2.syx",
    ] {
        let bytes = fixture(name);
        assert!(
            matches!(
                classify_inbound(&bytes),
                InboundMessage::MemoryDataSet { .. }
            ),
            "{name} should classify as MemoryDataSet",
        );
        let frame = Frame::decode(&bytes).unwrap();
        assert_eq!(AddressSpace::classify(frame.address), AddressSpace::Memory);
    }
}
