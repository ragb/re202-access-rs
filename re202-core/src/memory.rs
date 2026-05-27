//! Memory slot model — one preset's worth of parameters, 33 bytes.
//!
//! Layout from the official MIDI Implementation v1.00, device-verified
//! against captured fixtures (`memory_001`, `memory_002`, `memory_manual`,
//! `memory_127`, and the `edit_buffer_mirrors_memory2` snapshot).

use serde::{Deserialize, Serialize};

use crate::address::MEMORY_BLOCK_LEN;
use crate::codec::CodecError;
use crate::system::TimeMode;

/// Typed view of a 33-byte memory block.
///
/// Used for both stored slots (MEMORY MANUAL, MEMORY 1..=127) and the
/// edit-buffer mirror at `20 00 00 00`. The address is what differentiates;
/// the payload format is the same.
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub tape: Tape,
    pub mode: Mode,
    pub repeat_rate: RangedParam,
    pub intensity: RangedParam,
    pub echo_volume: RangedParam,
    pub bass: RangedParam,
    pub treble: RangedParam,
    pub reverb_volume: RangedParam,
    pub saturation: RangedParam,
    pub wow_flutter: RangedParam,
    pub reverb_sw: bool,
    pub tap_sw: bool,
    /// 0..=2000 ms. Wire format packs this as four 4-bit nibbles, MSB→LSB,
    /// at offsets 0x1C..0x1F. Effective ceiling depends on Time Mode
    /// (Normal=1000 ms, Long=2000 ms) — not enforced here.
    pub tap_time_ms: u16,
    pub time_mode: TimeMode,
}

/// A value-with-expression-pedal-range parameter (Repeat Rate, Intensity, etc.).
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangedParam {
    pub value: u8,
    pub min: u8,
    pub max: u8,
}

impl RangedParam {
    /// Build from three raw 7-bit bytes (each 0..=127).
    fn from_bytes(value: u8, min: u8, max: u8, field: &'static str) -> Result<Self, CodecError> {
        for (b, sub) in [(value, "value"), (min, "min"), (max, "max")] {
            if b > 127 {
                return Err(CodecError::OutOfRange {
                    field,
                    value: b as u16,
                    valid: "0..=127",
                });
            }
            let _ = sub;
        }
        Ok(Self { value, min, max })
    }

    fn to_bytes(self, field: &'static str) -> Result<[u8; 3], CodecError> {
        for b in [self.value, self.min, self.max] {
            if b > 127 {
                return Err(CodecError::OutOfRange {
                    field,
                    value: b as u16,
                    valid: "0..=127",
                });
            }
        }
        Ok([self.value, self.min, self.max])
    }
}

#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tape {
    New,
    Aged,
}

/// 12 head-combination modes. Numbered to match the device's UI (Mode 1..12).
/// Wire byte = `mode_number - 1` (0..=11).
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi, from_wasm_abi))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    M1,
    M2,
    M3,
    M4,
    M5,
    M6,
    M7,
    M8,
    M9,
    M10,
    M11,
    M12,
}

impl Tape {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::New),
            1 => Ok(Self::Aged),
            _ => Err(CodecError::InvalidValue {
                field: "tape",
                value: b,
                valid: "0=new, 1=aged",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::New => 0,
            Self::Aged => 1,
        }
    }
}

impl Mode {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        Ok(match b {
            0 => Self::M1,
            1 => Self::M2,
            2 => Self::M3,
            3 => Self::M4,
            4 => Self::M5,
            5 => Self::M6,
            6 => Self::M7,
            7 => Self::M8,
            8 => Self::M9,
            9 => Self::M10,
            10 => Self::M11,
            11 => Self::M12,
            _ => {
                return Err(CodecError::InvalidValue {
                    field: "mode",
                    value: b,
                    valid: "0..=11 (wire) = Mode 1..=12 (UI)",
                })
            }
        })
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::M1 => 0,
            Self::M2 => 1,
            Self::M3 => 2,
            Self::M4 => 3,
            Self::M5 => 4,
            Self::M6 => 5,
            Self::M7 => 6,
            Self::M8 => 7,
            Self::M9 => 8,
            Self::M10 => 9,
            Self::M11 => 10,
            Self::M12 => 11,
        }
    }
}

fn byte_to_bool(b: u8, field: &'static str) -> Result<bool, CodecError> {
    match b {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CodecError::InvalidValue {
            field,
            value: b,
            valid: "0 or 1",
        }),
    }
}

/// Unpack 4 nibble bytes into a single 0..=2000 value.
/// Each input byte uses only its low 4 bits (`0000 dddd`). MSB first.
pub(crate) fn unpack_tap_time(bytes: &[u8; 4]) -> Result<u16, CodecError> {
    for (i, &b) in bytes.iter().enumerate() {
        if b > 0x0F {
            return Err(CodecError::InvalidValue {
                field: "tap_time_nibble",
                value: b,
                valid: "0..=15 (low nibble only)",
            });
        }
        let _ = i;
    }
    let v = ((bytes[0] as u16) << 12)
        | ((bytes[1] as u16) << 8)
        | ((bytes[2] as u16) << 4)
        | (bytes[3] as u16);
    Ok(v)
}

/// Pack a 0..=2000 value into 4 nibble bytes (low 4 bits each), MSB first.
pub(crate) fn pack_tap_time(ms: u16) -> Result<[u8; 4], CodecError> {
    if ms > 2000 {
        return Err(CodecError::OutOfRange {
            field: "tap_time_ms",
            value: ms,
            valid: "0..=2000",
        });
    }
    Ok([
        ((ms >> 12) & 0x0F) as u8,
        ((ms >> 8) & 0x0F) as u8,
        ((ms >> 4) & 0x0F) as u8,
        (ms & 0x0F) as u8,
    ])
}

impl Memory {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != MEMORY_BLOCK_LEN {
            return Err(CodecError::WrongLength {
                expected: MEMORY_BLOCK_LEN,
                actual: bytes.len(),
            });
        }
        let tap = [bytes[0x1C], bytes[0x1D], bytes[0x1E], bytes[0x1F]];
        Ok(Self {
            tape: Tape::from_byte(bytes[0x00])?,
            mode: Mode::from_byte(bytes[0x01])?,
            repeat_rate: RangedParam::from_bytes(
                bytes[0x02],
                bytes[0x03],
                bytes[0x04],
                "repeat_rate",
            )?,
            intensity: RangedParam::from_bytes(bytes[0x05], bytes[0x06], bytes[0x07], "intensity")?,
            echo_volume: RangedParam::from_bytes(
                bytes[0x08],
                bytes[0x09],
                bytes[0x0A],
                "echo_volume",
            )?,
            bass: RangedParam::from_bytes(bytes[0x0B], bytes[0x0C], bytes[0x0D], "bass")?,
            treble: RangedParam::from_bytes(bytes[0x0E], bytes[0x0F], bytes[0x10], "treble")?,
            reverb_volume: RangedParam::from_bytes(
                bytes[0x11],
                bytes[0x12],
                bytes[0x13],
                "reverb_volume",
            )?,
            saturation: RangedParam::from_bytes(
                bytes[0x14],
                bytes[0x15],
                bytes[0x16],
                "saturation",
            )?,
            wow_flutter: RangedParam::from_bytes(
                bytes[0x17],
                bytes[0x18],
                bytes[0x19],
                "wow_flutter",
            )?,
            reverb_sw: byte_to_bool(bytes[0x1A], "reverb_sw")?,
            tap_sw: byte_to_bool(bytes[0x1B], "tap_sw")?,
            tap_time_ms: unpack_tap_time(&tap)?,
            time_mode: TimeMode::from_byte(bytes[0x20])?,
        })
    }

    pub fn to_bytes(&self) -> Result<[u8; MEMORY_BLOCK_LEN], CodecError> {
        let mut b = [0u8; MEMORY_BLOCK_LEN];
        b[0x00] = self.tape.to_byte();
        b[0x01] = self.mode.to_byte();
        b[0x02..0x05].copy_from_slice(&self.repeat_rate.to_bytes("repeat_rate")?);
        b[0x05..0x08].copy_from_slice(&self.intensity.to_bytes("intensity")?);
        b[0x08..0x0B].copy_from_slice(&self.echo_volume.to_bytes("echo_volume")?);
        b[0x0B..0x0E].copy_from_slice(&self.bass.to_bytes("bass")?);
        b[0x0E..0x11].copy_from_slice(&self.treble.to_bytes("treble")?);
        b[0x11..0x14].copy_from_slice(&self.reverb_volume.to_bytes("reverb_volume")?);
        b[0x14..0x17].copy_from_slice(&self.saturation.to_bytes("saturation")?);
        b[0x17..0x1A].copy_from_slice(&self.wow_flutter.to_bytes("wow_flutter")?);
        b[0x1A] = self.reverb_sw as u8;
        b[0x1B] = self.tap_sw as u8;
        b[0x1C..0x20].copy_from_slice(&pack_tap_time(self.tap_time_ms)?);
        b[0x20] = self.time_mode.to_byte();
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpack_pack_tap_time_round_trips() {
        for &ms in &[0u16, 1, 50, 500, 1000, 1500, 2000] {
            let packed = pack_tap_time(ms).unwrap();
            assert_eq!(unpack_tap_time(&packed).unwrap(), ms);
        }
    }

    #[test]
    fn pack_tap_time_rejects_over_2000() {
        let err = pack_tap_time(2001).unwrap_err();
        assert!(matches!(
            err,
            CodecError::OutOfRange {
                field: "tap_time_ms",
                ..
            }
        ));
    }

    #[test]
    fn unpack_tap_time_rejects_high_nibble() {
        let err = unpack_tap_time(&[0x10, 0, 0, 0]).unwrap_err();
        assert!(matches!(
            err,
            CodecError::InvalidValue {
                field: "tap_time_nibble",
                ..
            }
        ));
    }

    #[test]
    fn ranged_param_yaml_is_nested() {
        let p = RangedParam {
            value: 77,
            min: 0,
            max: 127,
        };
        let yaml = serde_yaml::to_string(&p).unwrap();
        assert!(yaml.contains("value: 77"));
        assert!(yaml.contains("min: 0"));
        assert!(yaml.contains("max: 127"));
    }

    /// Bytes from `memory_001_dump.syx` (Tap Time = 500 ms).
    const MEMORY_1_BYTES: [u8; MEMORY_BLOCK_LEN] = [
        0x00, 0x04, 0x4D, 0x4D, 0x4D, 0x36, 0x36, 0x36, 0x60, 0x3F, 0x7F, 0x3F, 0x3F, 0x3F, 0x3E,
        0x3E, 0x3E, 0x06, 0x06, 0x06, 0x00, 0x00, 0x00, 0x52, 0x52, 0x52, 0x01, 0x00, 0x00, 0x01,
        0x0F, 0x04, 0x00,
    ];

    #[test]
    fn decode_memory_1_then_encode_round_trips() {
        let m = Memory::from_bytes(&MEMORY_1_BYTES).unwrap();
        assert_eq!(m.tape, Tape::New);
        assert_eq!(m.mode, Mode::M5); // wire 0x04 → UI Mode 5
        assert_eq!(m.repeat_rate.value, 77);
        assert_eq!(m.tap_time_ms, 500);
        assert_eq!(m.time_mode, TimeMode::Normal);
        assert_eq!(m.to_bytes().unwrap(), MEMORY_1_BYTES);
    }

    #[test]
    fn memory_yaml_uses_named_enums_and_nested_ranges() {
        let m = Memory::from_bytes(&MEMORY_1_BYTES).unwrap();
        let yaml = serde_yaml::to_string(&m).unwrap();
        assert!(yaml.contains("tape: new"));
        assert!(yaml.contains("mode: m5"));
        assert!(yaml.contains("tap_time_ms: 500"));
        assert!(yaml.contains("time_mode: normal"));
        assert!(yaml.contains("repeat_rate:"));
        assert!(yaml.contains("  value: 77"));
    }

    #[test]
    fn memory_yaml_round_trips() {
        let m = Memory::from_bytes(&MEMORY_1_BYTES).unwrap();
        let yaml = serde_yaml::to_string(&m).unwrap();
        let back: Memory = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(m, back);
        assert_eq!(back.to_bytes().unwrap(), MEMORY_1_BYTES);
    }
}
