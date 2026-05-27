//! A 60-second tour of the `re202-core` codec API.
//!
//! Run with:
//!
//! ```text
//! cargo run --example library_tour -p re202-core
//! ```
//!
//! No MIDI required — this example uses captured `.syx` fixtures.

use std::path::Path;

use re202_core::address::{MemorySlot, EDIT_BUFFER_BASE, MEMORY_BLOCK_LEN, SYSTEM_BASE};
use re202_core::system::SYSTEM_AREA_LEN;
use re202_core::{classify_inbound, Frame, InboundMessage, Memory, SystemArea};

fn main() {
    println!("=== re202-core library tour ===\n");

    // 1) Construct an outgoing Data Request for the whole System area.
    let rq1 = Frame::data_request(
        0x10,                                      // device id
        SYSTEM_BASE,                               // 10 00 00 00
        [0x00, 0x00, 0x00, SYSTEM_AREA_LEN as u8], // size = 18 bytes
    );
    println!("1. RQ1 to System area:");
    println!(
        "   bytes ({} long): {}\n",
        rq1.encode().len(),
        hex(&rq1.encode())
    );

    // 2) Decode a captured DT1 reply (System dump fixture) into typed values.
    let bytes = fixture("re202-core/tests/fixtures/system_full-dump_defaultish.syx");
    let frame = Frame::decode(&bytes).expect("decode frame");
    let system = SystemArea::from_bytes(&frame.data).expect("decode SystemArea");
    println!("2. Decoded System area:");
    println!("     input_source      = {:?}", system.input_source);
    println!("     reverb_type       = {:?}", system.reverb_type);
    println!("     midi.rx_channel   = {:?}", system.midi.rx_channel);
    println!("     midi.thru         = {}", system.midi.thru);
    println!();

    // 3) Same thing for a Memory slot fixture — and observe Tap Time unpacks
    //    from its 4-nibble wire format to a u16 millisecond value.
    let mem_bytes = fixture("re202-core/tests/fixtures/memory_002_dump.syx");
    let mem_frame = Frame::decode(&mem_bytes).expect("decode memory frame");
    let memory = Memory::from_bytes(&mem_frame.data).expect("decode Memory");
    println!(
        "3. Decoded MEMORY 2 (at address {:02X?}):",
        mem_frame.address
    );
    println!("     tape              = {:?}", memory.tape);
    println!("     mode              = {:?}", memory.mode);
    println!("     mode active heads = {:?}", memory.mode.active_heads());
    println!("     repeat_rate.value = {}", memory.repeat_rate.value);
    println!("     tap_time_ms       = {} ms", memory.tap_time_ms);
    println!();

    // 4) classify_inbound() routes anything the device might send into a
    //    typed enum — useful for a real-time MIDI input handler.
    let edit_buffer_bytes = fixture("re202-core/tests/fixtures/edit_buffer_mirrors_memory2.syx");
    match classify_inbound(&edit_buffer_bytes) {
        InboundMessage::MemoryDataSet { address, data, .. } if address == EDIT_BUFFER_BASE => {
            println!("4. classify_inbound() recognized edit-buffer mirror:");
            println!("     address           = {:02X?}", address);
            println!(
                "     data length       = {} bytes (= MEMORY_BLOCK_LEN = {})",
                data.len(),
                MEMORY_BLOCK_LEN
            );
        }
        other => println!("4. classify_inbound returned {other:?}"),
    }
    println!();

    // 5) Edit the parsed Memory and re-encode to bytes ready to be sent back
    //    as a DT1 — for example, into the live edit buffer at 20 00 00 00 to
    //    audibly change the patch without committing it to a slot.
    let mut tweaked = memory.clone();
    tweaked.repeat_rate.value = 100;
    tweaked.tap_time_ms = 750;
    let new_payload = tweaked.to_bytes().expect("re-encode");
    let dt1 = Frame::data_set(0x10, EDIT_BUFFER_BASE, new_payload.to_vec());
    println!("5. DT1 to edit buffer with repeat_rate=100, tap_time=750ms:");
    println!(
        "   bytes ({} long): {}",
        dt1.encode().len(),
        hex(&dt1.encode())
    );
    println!();

    // 6) Address arithmetic: MemorySlot makes the per-slot carry math safe.
    println!("6. Memory-slot address arithmetic:");
    for slot in [
        MemorySlot::Manual,
        MemorySlot::User(1),
        MemorySlot::User(6),   // last slot before the 7-bit carry
        MemorySlot::User(7),   // (7+1)*0x10 = 0x80, wraps into high byte
        MemorySlot::User(127), // the high-end anchor
    ] {
        println!("     {:?}  ->  {:02X?}", slot, slot.base_address());
    }
}

fn fixture(rel: &str) -> Vec<u8> {
    // Look up paths relative to the workspace root so this works whether the
    // user runs from the workspace root or the crate dir.
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let path = workspace_root.join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
