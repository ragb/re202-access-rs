//! WASM bindings — exposes `re202-core` to JavaScript / TypeScript.
//!
//! Typed encode/decode functions accept and return the typed core models
//! (SystemArea / Memory), which render in the generated `.d.ts` thanks to
//! the `tsify` derives on those types.

use re202_core::address::{
    MemorySlot, EDIT_BUFFER_BASE, MEMORY_BLOCK_LEN, MEMORY_MANUAL_BASE, MEMORY_SLOT_MAX,
    SYSTEM_BASE,
};
use re202_core::system::SYSTEM_AREA_LEN;
use re202_core::{Frame, Memory, SystemArea};
use wasm_bindgen::prelude::*;

// === Frame (raw codec) ===

/// Encode a raw `Frame` (untyped data payload) to its on-wire bytes.
#[wasm_bindgen(js_name = encodeFrame)]
pub fn encode_frame(frame: JsValue) -> Result<Vec<u8>, JsError> {
    let frame: Frame = serde_wasm_bindgen::from_value(frame)
        .map_err(|e| JsError::new(&format!("decode Frame from JS: {e}")))?;
    Ok(frame.encode())
}

/// Decode a raw SysEx byte sequence into a `Frame`.
#[wasm_bindgen(js_name = decodeFrame)]
pub fn decode_frame(bytes: &[u8]) -> Result<JsValue, JsError> {
    let frame = Frame::decode(bytes).map_err(|e| JsError::new(&format!("decode Frame: {e}")))?;
    serde_wasm_bindgen::to_value(&frame).map_err(|e| JsError::new(&e.to_string()))
}

// === Typed System / Memory ===

/// Encode a `SystemArea` into its 18-byte representation.
#[wasm_bindgen(js_name = encodeSystem)]
pub fn encode_system(system: SystemArea) -> Result<Vec<u8>, JsError> {
    system
        .to_bytes()
        .map(|a| a.to_vec())
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Decode 18 bytes into a `SystemArea`.
#[wasm_bindgen(js_name = decodeSystem)]
pub fn decode_system(bytes: &[u8]) -> Result<SystemArea, JsError> {
    SystemArea::from_bytes(bytes).map_err(|e| JsError::new(&e.to_string()))
}

/// Encode a `Memory` into its 33-byte representation.
#[wasm_bindgen(js_name = encodeMemory)]
pub fn encode_memory(memory: Memory) -> Result<Vec<u8>, JsError> {
    memory
        .to_bytes()
        .map(|a| a.to_vec())
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Decode 33 bytes into a `Memory`.
#[wasm_bindgen(js_name = decodeMemory)]
pub fn decode_memory(bytes: &[u8]) -> Result<Memory, JsError> {
    Memory::from_bytes(bytes).map_err(|e| JsError::new(&e.to_string()))
}

// === Address helpers ===

/// 4-byte SysEx base address of the System area (`10 00 00 00`).
#[wasm_bindgen(js_name = systemBase)]
pub fn system_base() -> Vec<u8> {
    SYSTEM_BASE.to_vec()
}

/// 4-byte SysEx base address of the edit-buffer mirror (`20 00 00 00`).
#[wasm_bindgen(js_name = editBufferBase)]
pub fn edit_buffer_base() -> Vec<u8> {
    EDIT_BUFFER_BASE.to_vec()
}

/// 4-byte SysEx base address of MEMORY MANUAL (`20 10 00 00`).
#[wasm_bindgen(js_name = memoryManualBase)]
pub fn memory_manual_base() -> Vec<u8> {
    MEMORY_MANUAL_BASE.to_vec()
}

/// 4-byte SysEx base address of MEMORY `n` (1..=127). Throws on out-of-range.
#[wasm_bindgen(js_name = memorySlotBase)]
pub fn memory_slot_base(n: u8) -> Result<Vec<u8>, JsError> {
    if !(1..=MEMORY_SLOT_MAX).contains(&n) {
        return Err(JsError::new(&format!(
            "memory slot {n} out of range (valid: 1..=127)"
        )));
    }
    Ok(MemorySlot::User(n).base_address().to_vec())
}

/// Byte length of the System area block.
#[wasm_bindgen(js_name = systemAreaLen)]
pub fn system_area_len() -> usize {
    SYSTEM_AREA_LEN
}

/// Byte length of a Memory block.
#[wasm_bindgen(js_name = memoryBlockLen)]
pub fn memory_block_len() -> usize {
    MEMORY_BLOCK_LEN
}

#[cfg(test)]
mod tests {
    use re202_core::address::{MemorySlot, MEMORY_BLOCK_LEN};
    use re202_core::system::SYSTEM_AREA_LEN;
    use re202_core::{Frame, Memory, SystemArea};
    use wasm_bindgen_test::wasm_bindgen_test;

    // Payload bytes from the captured `system_full-dump_defaultish.syx`.
    const SYSTEM_PAYLOAD: [u8; SYSTEM_AREA_LEN] = [
        0x00, 0x01, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x04, 0x01, 0x11, 0x01, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x01,
    ];

    // Payload bytes from the captured `memory_001_dump.syx`.
    const MEMORY_1_PAYLOAD: [u8; MEMORY_BLOCK_LEN] = [
        0x00, 0x04, 0x4D, 0x4D, 0x4D, 0x36, 0x36, 0x36, 0x60, 0x3F, 0x7F, 0x3F, 0x3F, 0x3F, 0x3E,
        0x3E, 0x3E, 0x06, 0x06, 0x06, 0x00, 0x00, 0x00, 0x52, 0x52, 0x52, 0x01, 0x00, 0x00, 0x01,
        0x0F, 0x04, 0x00,
    ];

    #[wasm_bindgen_test]
    fn frame_codec_round_trip() {
        let f = Frame::data_set(0x10, [0x10, 0x00, 0x00, 0x00], vec![0x00]);
        let bytes = f.encode();
        let decoded = Frame::decode(&bytes).unwrap();
        assert_eq!(f, decoded);
    }

    #[wasm_bindgen_test]
    fn system_codec_round_trip() {
        let s = SystemArea::from_bytes(&SYSTEM_PAYLOAD).unwrap();
        assert_eq!(s.to_bytes().unwrap(), SYSTEM_PAYLOAD);
    }

    #[wasm_bindgen_test]
    fn memory_codec_round_trip_with_tap_time_unpacking() {
        let m = Memory::from_bytes(&MEMORY_1_PAYLOAD).unwrap();
        assert_eq!(m.tap_time_ms, 500);
        assert_eq!(m.to_bytes().unwrap(), MEMORY_1_PAYLOAD);
    }

    #[wasm_bindgen_test]
    fn address_helpers_match_spec() {
        assert_eq!(super::system_base(), vec![0x10, 0x00, 0x00, 0x00]);
        assert_eq!(super::edit_buffer_base(), vec![0x20, 0x00, 0x00, 0x00]);
        assert_eq!(super::memory_manual_base(), vec![0x20, 0x10, 0x00, 0x00]);
        assert_eq!(
            super::memory_slot_base(1).unwrap(),
            vec![0x20, 0x20, 0x00, 0x00]
        );
        assert_eq!(
            super::memory_slot_base(127).unwrap(),
            vec![0x30, 0x00, 0x00, 0x00]
        );
        // Carry boundary.
        assert_eq!(
            super::memory_slot_base(8).unwrap(),
            MemorySlot::User(8).base_address().to_vec()
        );
        assert!(super::memory_slot_base(0).is_err());
        assert!(super::memory_slot_base(128).is_err());
    }

    #[wasm_bindgen_test]
    fn block_lens_constants() {
        assert_eq!(super::system_area_len(), SYSTEM_AREA_LEN);
        assert_eq!(super::memory_block_len(), MEMORY_BLOCK_LEN);
    }
}
