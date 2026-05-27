//! WASM bindings — exposes `re202-core` to JavaScript / TypeScript.
//!
//! Mirrors the typed-tsify approach from ml10x-access-rs. As types land in
//! `re202-core` with `#[cfg_attr(feature = "tsify", derive(Tsify))]`, they
//! show up in the generated `.d.ts` automatically.

use wasm_bindgen::prelude::*;

/// Encode a Frame to its on-wire byte representation.
///
/// Roundtrip-tested against the System / Input Source frame in `re202-core`.
#[wasm_bindgen(js_name = encodeFrame)]
pub fn encode_frame(frame: JsValue) -> Result<Vec<u8>, JsError> {
    let frame: re202_core::Frame = serde_wasm_bindgen::from_value(frame)
        .map_err(|e| JsError::new(&format!("decode Frame from JS: {e}")))?;
    Ok(frame.encode())
}

/// Decode a SysEx byte sequence into a `Frame`.
#[wasm_bindgen(js_name = decodeFrame)]
pub fn decode_frame(bytes: &[u8]) -> Result<JsValue, JsError> {
    let frame = re202_core::Frame::decode(bytes)
        .map_err(|e| JsError::new(&format!("decode Frame from bytes: {e}")))?;
    serde_wasm_bindgen::to_value(&frame)
        .map_err(|e| JsError::new(&format!("encode Frame to JS: {e}")))
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn checksum_round_trip() {
        let f = re202_core::Frame::data_set(0x10, [0x10, 0x00, 0x00, 0x00], vec![0x00]);
        let bytes = f.encode();
        let decoded = re202_core::Frame::decode(&bytes).unwrap();
        assert_eq!(f, decoded);
    }
}
