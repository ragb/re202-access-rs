//! Roland SysEx framing for the RE-202.
//!
//! Frame layout (DT1, write):
//!
//! ```text
//! F0 41 [dev] 00 00 00 00 18 12 [a a a a] [d d d ...] [chk] F7
//! └ SOX                       └ DT1
//!    └ Roland         └ Model ID (5 bytes, modern Roland form)
//!       └ Device ID (0x10..=0x1F)
//! ```
//!
//! Checksum is the Roland standard: `(128 - ((sum of addr + data) mod 128)) mod 128`.
//! The checksum does NOT include the F0, manufacturer, device, model, or command bytes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SYSEX_START: u8 = 0xF0;
pub const SYSEX_END: u8 = 0xF7;
pub const ROLAND_ID: u8 = 0x41;

/// 5-byte Roland model ID for the RE-202.
pub const RE202_MODEL_ID: [u8; 5] = [0x00, 0x00, 0x00, 0x00, 0x18];

/// Data Set 1 — write data to the device.
pub const CMD_DT1: u8 = 0x12;
/// Data Request 1 — ask the device to send data back. Suspected, not yet device-verified.
pub const CMD_RQ1: u8 = 0x11;

/// Minimum frame length: F0 41 dev 00 00 00 00 18 cmd a a a a chk F7 = 15 bytes.
const MIN_FRAME_LEN: usize = 15;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SysExError {
    #[error("frame too short ({0} bytes, need at least {MIN_FRAME_LEN})")]
    TooShort(usize),
    #[error("missing F0 start sentinel")]
    MissingStart,
    #[error("missing F7 end sentinel")]
    MissingEnd,
    #[error("not a Roland frame (manufacturer id {0:#04x})")]
    NotRoland(u8),
    #[error("invalid device id {0:#04x} (must be 0x10..=0x1F)")]
    InvalidDeviceId(u8),
    #[error("wrong model id (expected RE-202 {RE202_MODEL_ID:02x?})")]
    WrongModel,
    #[error("unknown command byte {0:#04x}")]
    UnknownCommand(u8),
    #[error("checksum mismatch: expected {expected:#04x}, got {actual:#04x}")]
    ChecksumMismatch { expected: u8, actual: u8 },
    #[error("data byte out of range (>= 0x80) at index {0}")]
    DataByteOutOfRange(usize),
}

/// Compute the Roland SysEx checksum over the address + data section.
pub fn checksum(addr_and_data: &[u8]) -> u8 {
    let sum: u32 = addr_and_data.iter().map(|&b| b as u32).sum();
    ((128 - (sum % 128)) % 128) as u8
}

/// A decoded Roland SysEx frame addressed to / from the RE-202.
///
/// The `data` field is the raw payload between the address and the checksum.
/// For RQ1 frames it is conventionally a 4-byte "size" field.
/// For DT1 frames it is the value(s) being written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub device_id: u8,
    pub command: u8,
    pub address: [u8; 4],
    pub data: Vec<u8>,
}

impl Frame {
    pub fn data_set(device_id: u8, address: [u8; 4], data: Vec<u8>) -> Self {
        Self {
            device_id,
            command: CMD_DT1,
            address,
            data,
        }
    }

    /// Request `size` bytes from the device starting at `address`.
    /// RQ1's data payload is conventionally a 4-byte big-endian (7-bit safe) size.
    pub fn data_request(device_id: u8, address: [u8; 4], size: [u8; 4]) -> Self {
        Self {
            device_id,
            command: CMD_RQ1,
            address,
            data: size.to_vec(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(MIN_FRAME_LEN + self.data.len());
        buf.push(SYSEX_START);
        buf.push(ROLAND_ID);
        buf.push(self.device_id);
        buf.extend_from_slice(&RE202_MODEL_ID);
        buf.push(self.command);
        buf.extend_from_slice(&self.address);
        buf.extend_from_slice(&self.data);

        let mut cksum_input = Vec::with_capacity(4 + self.data.len());
        cksum_input.extend_from_slice(&self.address);
        cksum_input.extend_from_slice(&self.data);
        buf.push(checksum(&cksum_input));
        buf.push(SYSEX_END);
        buf
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SysExError> {
        if bytes.len() < MIN_FRAME_LEN {
            return Err(SysExError::TooShort(bytes.len()));
        }
        if bytes[0] != SYSEX_START {
            return Err(SysExError::MissingStart);
        }
        if *bytes.last().unwrap() != SYSEX_END {
            return Err(SysExError::MissingEnd);
        }
        if bytes[1] != ROLAND_ID {
            return Err(SysExError::NotRoland(bytes[1]));
        }
        let device_id = bytes[2];
        if !(0x10..=0x1F).contains(&device_id) {
            return Err(SysExError::InvalidDeviceId(device_id));
        }
        if bytes[3..8] != RE202_MODEL_ID {
            return Err(SysExError::WrongModel);
        }
        let command = bytes[8];
        if command != CMD_DT1 && command != CMD_RQ1 {
            return Err(SysExError::UnknownCommand(command));
        }
        let address: [u8; 4] = bytes[9..13].try_into().unwrap();

        let payload_end = bytes.len() - 2; // exclude checksum + F7
        let data = bytes[13..payload_end].to_vec();
        for (i, &b) in data.iter().enumerate() {
            if b >= 0x80 {
                return Err(SysExError::DataByteOutOfRange(i));
            }
        }

        let expected = bytes[payload_end];
        let mut cksum_input = Vec::with_capacity(4 + data.len());
        cksum_input.extend_from_slice(&address);
        cksum_input.extend_from_slice(&data);
        let actual = checksum(&cksum_input);
        if expected != actual {
            return Err(SysExError::ChecksumMismatch { expected, actual });
        }

        Ok(Self {
            device_id,
            command,
            address,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-good frame derived from the Electra One forum thread:
    /// DT1 to System / Input Source (`10 00 00 00`) = Guitar (0x00).
    /// addr+data sum = 0x10 = 16. checksum = (128 - 16) % 128 = 112 = 0x70.
    const SYSTEM_INPUT_SOURCE_GUITAR: &[u8] = &[
        0xF0, 0x41, 0x10, 0x00, 0x00, 0x00, 0x00, 0x18, 0x12, 0x10, 0x00, 0x00, 0x00, 0x00, 0x70,
        0xF7,
    ];

    #[test]
    fn checksum_matches_roland_spec() {
        // sum = 0x10 → checksum = 0x70
        assert_eq!(checksum(&[0x10, 0x00, 0x00, 0x00, 0x00]), 0x70);

        // sum = 0 → checksum = 0
        assert_eq!(checksum(&[]), 0);
        assert_eq!(checksum(&[0, 0, 0, 0]), 0);

        // sum = 127 → checksum = 1
        assert_eq!(checksum(&[127]), 1);

        // sum = 128 → checksum = 0
        assert_eq!(checksum(&[64, 64]), 0);
    }

    #[test]
    fn encode_input_source_guitar() {
        let f = Frame::data_set(0x10, [0x10, 0x00, 0x00, 0x00], vec![0x00]);
        assert_eq!(f.encode(), SYSTEM_INPUT_SOURCE_GUITAR);
    }

    #[test]
    fn decode_input_source_guitar() {
        let f = Frame::decode(SYSTEM_INPUT_SOURCE_GUITAR).unwrap();
        assert_eq!(f.device_id, 0x10);
        assert_eq!(f.command, CMD_DT1);
        assert_eq!(f.address, [0x10, 0x00, 0x00, 0x00]);
        assert_eq!(f.data, vec![0x00]);
    }

    #[test]
    fn round_trip_random_dt1() {
        let f = Frame::data_set(0x1F, [0x20, 0x01, 0x02, 0x03], vec![0x5A, 0x33, 0x7F, 0x00]);
        let bytes = f.encode();
        let decoded = Frame::decode(&bytes).unwrap();
        assert_eq!(f, decoded);
    }

    #[test]
    fn round_trip_rq1() {
        let f = Frame::data_request(0x10, [0x20, 0x00, 0x00, 0x00], [0x00, 0x00, 0x00, 0x10]);
        let bytes = f.encode();
        let decoded = Frame::decode(&bytes).unwrap();
        assert_eq!(decoded.command, CMD_RQ1);
        assert_eq!(decoded.data, vec![0x00, 0x00, 0x00, 0x10]);
        assert_eq!(f, decoded);
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut bytes = SYSTEM_INPUT_SOURCE_GUITAR.to_vec();
        bytes[14] = 0x71; // wrong checksum
        let err = Frame::decode(&bytes).unwrap_err();
        assert!(matches!(err, SysExError::ChecksumMismatch { .. }));
    }

    #[test]
    fn rejects_wrong_model() {
        let mut bytes = SYSTEM_INPUT_SOURCE_GUITAR.to_vec();
        bytes[7] = 0x19; // wrong model byte
        let err = Frame::decode(&bytes).unwrap_err();
        assert_eq!(err, SysExError::WrongModel);
    }

    #[test]
    fn rejects_non_roland() {
        let mut bytes = SYSTEM_INPUT_SOURCE_GUITAR.to_vec();
        bytes[1] = 0x42; // Korg
        let err = Frame::decode(&bytes).unwrap_err();
        assert_eq!(err, SysExError::NotRoland(0x42));
    }

    #[test]
    fn rejects_invalid_device_id() {
        let mut bytes = SYSTEM_INPUT_SOURCE_GUITAR.to_vec();
        bytes[2] = 0x20; // out of 0x10..=0x1F
        let err = Frame::decode(&bytes).unwrap_err();
        assert_eq!(err, SysExError::InvalidDeviceId(0x20));
    }

    #[test]
    fn rejects_data_byte_out_of_range() {
        // Construct a frame where data has a >=0x80 byte (manually, because encode() never produces one).
        let bytes = [
            0xF0, 0x41, 0x10, 0x00, 0x00, 0x00, 0x00, 0x18, 0x12, 0x10, 0x00, 0x00, 0x00, 0x80,
            0x00, // bad data byte 0x80, placeholder checksum (will be checked first)
            0xF7,
        ];
        let err = Frame::decode(&bytes).unwrap_err();
        assert!(matches!(err, SysExError::DataByteOutOfRange(0)));
    }

    #[test]
    fn rejects_too_short() {
        let bytes = [0xF0, 0x41, 0x10, 0xF7];
        let err = Frame::decode(&bytes).unwrap_err();
        assert!(matches!(err, SysExError::TooShort(4)));
    }
}
