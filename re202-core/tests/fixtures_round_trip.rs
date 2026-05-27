//! Round-trip tests against real RE-202 SysEx captures.
//!
//! Each fixture in `tests/fixtures/*.syx` is a verbatim byte capture from a
//! physical RE-202 (firmware version reporting `00 00 00 00`, device id 0x10).
//! These tests assert the codec decodes the bytes correctly AND re-encodes
//! them to the identical byte sequence.

use std::path::Path;

use re202_core::address::{AddressSpace, MemorySlot, MEMORY_BLOCK_LEN};
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
    assert_eq!(frame.data[0x11], 0x01, "MIDI Thru = ON (explains echo behavior)");

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
fn classify_inbound_recognises_each_fixture() {
    let sys = fixture("system_input-source_guitar.syx");
    let sys_full = fixture("system_full-dump_defaultish.syx");
    let mem = fixture("memory_001_dump.syx");

    assert!(matches!(
        classify_inbound(&sys),
        InboundMessage::SystemDataSet { .. }
    ));
    assert!(matches!(
        classify_inbound(&sys_full),
        InboundMessage::SystemDataSet { .. }
    ));
    assert!(matches!(
        classify_inbound(&mem),
        InboundMessage::MemoryDataSet { .. }
    ));

    // Sanity-check the address-space classifier against each fixture's address.
    let mem_frame = Frame::decode(&mem).unwrap();
    assert_eq!(AddressSpace::classify(mem_frame.address), AddressSpace::Memory);
}
