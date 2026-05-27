//! System area (global settings) — `10 00 00 00`, 18 bytes.
//!
//! Layout from the official MIDI Implementation v1.00, all device-verified.
//! See `docs/sysex-notes.md` for source.

use serde::{Deserialize, Serialize};

use crate::codec::CodecError;

/// Length of the encoded System area in bytes.
pub const SYSTEM_AREA_LEN: usize = 18;

/// Typed view of the System area.
///
/// Fields are ordered the same way they appear in the byte layout. Grouped:
/// `direct.*` covers offsets 0x03..=0x04; `midi.*` covers offsets 0x09..=0x11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemArea {
    pub input_source: InputSource,
    pub ctl1_function: Ctl1Function,
    pub ctl2_function: Ctl2Function,
    pub direct: DirectSettings,
    pub carryover: bool,
    pub time_mode: TimeMode,
    pub reverb_type: ReverbType,
    /// 1..=4 — number of memory slots cycled by the MEMORY footswitch.
    pub memory_extent: u8,
    pub midi: MidiSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectSettings {
    pub on: bool,
    pub mode: DirectMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiSettings {
    pub rx_channel: MidiChannel,
    pub tx_channel: TxChannel,
    pub pc_in: bool,
    pub pc_out: bool,
    pub cc_in: bool,
    pub cc_out: bool,
    pub sync_source: SyncSource,
    pub realtime_source: RealtimeSource,
    pub thru: bool,
}

// === enums ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSource {
    Guitar,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ctl1Function {
    MemoryUp,
    MemoryDown,
    EffectOnOff,
    Tap,
    Warp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ctl2Function {
    MemoryUp,
    MemoryDown,
    EffectOnOff,
    Tap,
    Twist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectMode {
    Analog,
    Re201Simulate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeMode {
    Normal,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverbType {
    Spring,
    Hall,
    Plate,
    Room,
    Ambience,
}

/// MIDI Rx channel — `off` or `1..=16`.
///
/// YAML: `off` or an integer 1..=16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum MidiChannel {
    Channel(u8),
    Symbolic(MidiChannelSymbol),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiChannelSymbol {
    Off,
}

/// MIDI Tx channel — `off`, `1..=16`, or `rx` (follows RX channel).
///
/// YAML: `off`, `rx`, or an integer 1..=16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TxChannel {
    Channel(u8),
    Symbolic(TxChannelSymbol),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxChannelSymbol {
    Off,
    Rx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    Internal,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeSource {
    Internal,
    Midi,
}

// === byte codec ===

impl SystemArea {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != SYSTEM_AREA_LEN {
            return Err(CodecError::WrongLength {
                expected: SYSTEM_AREA_LEN,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            input_source: InputSource::from_byte(bytes[0x00])?,
            ctl1_function: Ctl1Function::from_byte(bytes[0x01])?,
            ctl2_function: Ctl2Function::from_byte(bytes[0x02])?,
            direct: DirectSettings {
                on: byte_to_bool(bytes[0x03], "direct.on")?,
                mode: DirectMode::from_byte(bytes[0x04])?,
            },
            carryover: byte_to_bool(bytes[0x05], "carryover")?,
            time_mode: TimeMode::from_byte(bytes[0x06])?,
            reverb_type: ReverbType::from_byte(bytes[0x07])?,
            memory_extent: validate_range(bytes[0x08], 1, 4, "memory_extent")?,
            midi: MidiSettings {
                rx_channel: MidiChannel::from_byte(bytes[0x09])?,
                tx_channel: TxChannel::from_byte(bytes[0x0A])?,
                pc_in: byte_to_bool(bytes[0x0B], "midi.pc_in")?,
                pc_out: byte_to_bool(bytes[0x0C], "midi.pc_out")?,
                cc_in: byte_to_bool(bytes[0x0D], "midi.cc_in")?,
                cc_out: byte_to_bool(bytes[0x0E], "midi.cc_out")?,
                sync_source: SyncSource::from_byte(bytes[0x0F])?,
                realtime_source: RealtimeSource::from_byte(bytes[0x10])?,
                thru: byte_to_bool(bytes[0x11], "midi.thru")?,
            },
        })
    }

    pub fn to_bytes(&self) -> Result<[u8; SYSTEM_AREA_LEN], CodecError> {
        if !(1..=4).contains(&self.memory_extent) {
            return Err(CodecError::OutOfRange {
                field: "memory_extent",
                value: self.memory_extent as u16,
                valid: "1..=4",
            });
        }
        let mut b = [0u8; SYSTEM_AREA_LEN];
        b[0x00] = self.input_source.to_byte();
        b[0x01] = self.ctl1_function.to_byte();
        b[0x02] = self.ctl2_function.to_byte();
        b[0x03] = self.direct.on as u8;
        b[0x04] = self.direct.mode.to_byte();
        b[0x05] = self.carryover as u8;
        b[0x06] = self.time_mode.to_byte();
        b[0x07] = self.reverb_type.to_byte();
        b[0x08] = self.memory_extent;
        b[0x09] = self.midi.rx_channel.to_byte();
        b[0x0A] = self.midi.tx_channel.to_byte();
        b[0x0B] = self.midi.pc_in as u8;
        b[0x0C] = self.midi.pc_out as u8;
        b[0x0D] = self.midi.cc_in as u8;
        b[0x0E] = self.midi.cc_out as u8;
        b[0x0F] = self.midi.sync_source.to_byte();
        b[0x10] = self.midi.realtime_source.to_byte();
        b[0x11] = self.midi.thru as u8;
        Ok(b)
    }
}

// === enum byte mappings ===

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

fn validate_range(value: u8, min: u8, max: u8, field: &'static str) -> Result<u8, CodecError> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(CodecError::OutOfRange {
            field,
            value: value as u16,
            valid: "see field doc",
        })
    }
}

impl InputSource {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Guitar),
            1 => Ok(Self::Line),
            _ => Err(CodecError::InvalidValue {
                field: "input_source",
                value: b,
                valid: "0=guitar, 1=line",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Guitar => 0,
            Self::Line => 1,
        }
    }
}

impl Ctl1Function {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::MemoryUp),
            1 => Ok(Self::MemoryDown),
            2 => Ok(Self::EffectOnOff),
            3 => Ok(Self::Tap),
            4 => Ok(Self::Warp),
            _ => Err(CodecError::InvalidValue {
                field: "ctl1_function",
                value: b,
                valid: "0..=4",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::MemoryUp => 0,
            Self::MemoryDown => 1,
            Self::EffectOnOff => 2,
            Self::Tap => 3,
            Self::Warp => 4,
        }
    }
}

impl Ctl2Function {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::MemoryUp),
            1 => Ok(Self::MemoryDown),
            2 => Ok(Self::EffectOnOff),
            3 => Ok(Self::Tap),
            4 => Ok(Self::Twist),
            _ => Err(CodecError::InvalidValue {
                field: "ctl2_function",
                value: b,
                valid: "0..=4",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::MemoryUp => 0,
            Self::MemoryDown => 1,
            Self::EffectOnOff => 2,
            Self::Tap => 3,
            Self::Twist => 4,
        }
    }
}

impl DirectMode {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Analog),
            1 => Ok(Self::Re201Simulate),
            _ => Err(CodecError::InvalidValue {
                field: "direct.mode",
                value: b,
                valid: "0=analog, 1=re201_simulate",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Analog => 0,
            Self::Re201Simulate => 1,
        }
    }
}

impl TimeMode {
    pub(crate) fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Long),
            _ => Err(CodecError::InvalidValue {
                field: "time_mode",
                value: b,
                valid: "0=normal, 1=long",
            }),
        }
    }
    pub(crate) fn to_byte(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Long => 1,
        }
    }
}

impl ReverbType {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Spring),
            1 => Ok(Self::Hall),
            2 => Ok(Self::Plate),
            3 => Ok(Self::Room),
            4 => Ok(Self::Ambience),
            _ => Err(CodecError::InvalidValue {
                field: "reverb_type",
                value: b,
                valid: "0..=4",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Spring => 0,
            Self::Hall => 1,
            Self::Plate => 2,
            Self::Room => 3,
            Self::Ambience => 4,
        }
    }
}

impl MidiChannel {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Symbolic(MidiChannelSymbol::Off)),
            1..=16 => Ok(Self::Channel(b)),
            _ => Err(CodecError::InvalidValue {
                field: "midi.rx_channel",
                value: b,
                valid: "0 (=off) or 1..=16",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Symbolic(MidiChannelSymbol::Off) => 0,
            Self::Channel(n) => n,
        }
    }
}

impl TxChannel {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Symbolic(TxChannelSymbol::Off)),
            1..=16 => Ok(Self::Channel(b)),
            17 => Ok(Self::Symbolic(TxChannelSymbol::Rx)),
            _ => Err(CodecError::InvalidValue {
                field: "midi.tx_channel",
                value: b,
                valid: "0 (=off), 1..=16, or 17 (=rx)",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Symbolic(TxChannelSymbol::Off) => 0,
            Self::Channel(n) => n,
            Self::Symbolic(TxChannelSymbol::Rx) => 17,
        }
    }
}

impl SyncSource {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Internal),
            1 => Ok(Self::Auto),
            _ => Err(CodecError::InvalidValue {
                field: "midi.sync_source",
                value: b,
                valid: "0=internal, 1=auto",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Internal => 0,
            Self::Auto => 1,
        }
    }
}

impl RealtimeSource {
    fn from_byte(b: u8) -> Result<Self, CodecError> {
        match b {
            0 => Ok(Self::Internal),
            1 => Ok(Self::Midi),
            _ => Err(CodecError::InvalidValue {
                field: "midi.realtime_source",
                value: b,
                valid: "0=internal, 1=midi",
            }),
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Self::Internal => 0,
            Self::Midi => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes from the captured `system_full-dump_defaultish.syx` fixture.
    const FIXTURE_BYTES: [u8; SYSTEM_AREA_LEN] = [
        0x00, 0x01, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x04, 0x01, 0x11, 0x01, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x01,
    ];

    fn fixture_value() -> SystemArea {
        SystemArea {
            input_source: InputSource::Guitar,
            ctl1_function: Ctl1Function::MemoryDown,
            ctl2_function: Ctl2Function::MemoryDown,
            direct: DirectSettings {
                on: true,
                mode: DirectMode::Analog,
            },
            carryover: true,
            time_mode: TimeMode::Normal,
            reverb_type: ReverbType::Spring,
            memory_extent: 4,
            midi: MidiSettings {
                rx_channel: MidiChannel::Channel(1),
                tx_channel: TxChannel::Symbolic(TxChannelSymbol::Rx),
                pc_in: true,
                pc_out: true,
                cc_in: false,
                cc_out: false,
                sync_source: SyncSource::Internal,
                realtime_source: RealtimeSource::Internal,
                thru: true,
            },
        }
    }

    #[test]
    fn decode_then_encode_round_trips() {
        let decoded = SystemArea::from_bytes(&FIXTURE_BYTES).unwrap();
        assert_eq!(decoded, fixture_value());
        let bytes = decoded.to_bytes().unwrap();
        assert_eq!(bytes, FIXTURE_BYTES);
    }

    #[test]
    fn rejects_wrong_length() {
        let err = SystemArea::from_bytes(&[0u8; 10]).unwrap_err();
        assert_eq!(
            err,
            CodecError::WrongLength {
                expected: 18,
                actual: 10
            }
        );
    }

    #[test]
    fn rejects_invalid_enum_byte() {
        let mut bytes = FIXTURE_BYTES;
        bytes[0x00] = 0x05; // not a valid InputSource
        let err = SystemArea::from_bytes(&bytes).unwrap_err();
        assert!(matches!(
            err,
            CodecError::InvalidValue {
                field: "input_source",
                ..
            }
        ));
    }

    #[test]
    fn rejects_out_of_range_memory_extent_on_encode() {
        let mut v = fixture_value();
        v.memory_extent = 5;
        let err = v.to_bytes().unwrap_err();
        assert!(matches!(
            err,
            CodecError::OutOfRange {
                field: "memory_extent",
                ..
            }
        ));
    }

    #[test]
    fn yaml_round_trip() {
        let v = fixture_value();
        let yaml = serde_yaml::to_string(&v).unwrap();
        let back: SystemArea = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn yaml_renders_named_enums() {
        let yaml = serde_yaml::to_string(&fixture_value()).unwrap();
        assert!(yaml.contains("input_source: guitar"));
        assert!(yaml.contains("ctl1_function: memory_down"));
        assert!(yaml.contains("reverb_type: spring"));
        assert!(yaml.contains("tx_channel: rx"));
    }
}
