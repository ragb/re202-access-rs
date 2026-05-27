//! End-to-end typed round-trip against captured fixtures.
//!
//! For each fixture: bytes → Frame::decode → typed (SystemArea | Memory)
//! → typed.to_bytes → Frame::encode → bytes. Compare to original.
//!
//! Also covers YAML round-trip: typed → YAML string → typed → bytes.

use std::path::Path;

use re202_core::address::EDIT_BUFFER_BASE;
use re202_core::{Frame, Memory, SystemArea, Tape};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn assert_system_full_round_trip(name: &str) {
    let bytes = fixture(name);
    let frame = Frame::decode(&bytes).expect("frame decode");
    let typed = SystemArea::from_bytes(&frame.data).expect("typed decode");

    let typed_bytes = typed.to_bytes().expect("typed encode");
    let mut rebuilt = frame.clone();
    rebuilt.data = typed_bytes.to_vec();
    assert_eq!(rebuilt.encode(), bytes, "{name}: byte-exact round-trip");

    // YAML round trip
    let yaml = serde_yaml::to_string(&typed).unwrap();
    let from_yaml: SystemArea = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(from_yaml, typed);
}

fn assert_memory_full_round_trip(name: &str) {
    let bytes = fixture(name);
    let frame = Frame::decode(&bytes).expect("frame decode");
    let typed = Memory::from_bytes(&frame.data).expect("typed decode");

    let typed_bytes = typed.to_bytes().expect("typed encode");
    let mut rebuilt = frame.clone();
    rebuilt.data = typed_bytes.to_vec();
    assert_eq!(rebuilt.encode(), bytes, "{name}: byte-exact round-trip");

    // YAML round trip
    let yaml = serde_yaml::to_string(&typed).unwrap();
    let from_yaml: Memory = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(from_yaml, typed);
}

#[test]
fn system_fixture_round_trips_through_typed_and_yaml() {
    assert_system_full_round_trip("system_full-dump_defaultish.syx");
}

#[test]
fn each_memory_fixture_round_trips_through_typed_and_yaml() {
    for name in [
        "memory_001_dump.syx",
        "memory_002_dump.syx",
        "memory_127_dump.syx",
        "memory_manual_dump.syx",
        "edit_buffer_mirrors_memory2.syx",
    ] {
        assert_memory_full_round_trip(name);
    }
}

/// Cross-fixture invariant: the edit-buffer's typed value equals the
/// active memory's typed value (since the edit buffer mirrors it).
#[test]
fn edit_buffer_typed_equals_memory_002() {
    let edit_bytes = fixture("edit_buffer_mirrors_memory2.syx");
    let m2_bytes = fixture("memory_002_dump.syx");

    let edit_frame = Frame::decode(&edit_bytes).unwrap();
    let m2_frame = Frame::decode(&m2_bytes).unwrap();

    assert_eq!(edit_frame.address, EDIT_BUFFER_BASE);

    let edit_typed = Memory::from_bytes(&edit_frame.data).unwrap();
    let m2_typed = Memory::from_bytes(&m2_frame.data).unwrap();

    assert_eq!(edit_typed, m2_typed);
    // And a sanity check on the actual content of MEMORY 2:
    assert_eq!(edit_typed.tape, Tape::Aged);
}
