//! Classify any byte sequence that arrived from the device.
//!
//! Owning this routing in one function (per the ml10x lesson) keeps callers
//! from having to know about command bytes and address prefixes.

use crate::address::AddressSpace;
use crate::sysex::{Frame, SysExError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundMessage {
    /// A well-formed DT1 frame addressed to the System area.
    SystemDataSet {
        device_id: u8,
        address: [u8; 4],
        data: Vec<u8>,
    },
    /// A well-formed DT1 frame addressed to a Memory slot.
    MemoryDataSet {
        device_id: u8,
        address: [u8; 4],
        data: Vec<u8>,
    },
    /// A well-formed DT1 frame at an unclassified address.
    UnknownDataSet {
        device_id: u8,
        address: [u8; 4],
        data: Vec<u8>,
    },
    /// A well-formed RQ1 frame (we don't normally receive these, but support it).
    DataRequest {
        device_id: u8,
        address: [u8; 4],
        size: Vec<u8>,
    },
    /// SysEx that we couldn't decode as a Roland RE-202 frame.
    UnparseableSysEx { bytes: Vec<u8>, error: SysExError },
    /// Bytes that weren't a SysEx frame at all (Note On/Off, CC, etc.).
    NonSysEx(Vec<u8>),
}

pub fn classify_inbound(bytes: &[u8]) -> InboundMessage {
    let starts_sysex = bytes.first() == Some(&crate::sysex::SYSEX_START);
    if !starts_sysex {
        return InboundMessage::NonSysEx(bytes.to_vec());
    }
    match Frame::decode(bytes) {
        Ok(frame) => match frame.command {
            crate::sysex::CMD_DT1 => match AddressSpace::classify(frame.address) {
                AddressSpace::System => InboundMessage::SystemDataSet {
                    device_id: frame.device_id,
                    address: frame.address,
                    data: frame.data,
                },
                AddressSpace::Memory => InboundMessage::MemoryDataSet {
                    device_id: frame.device_id,
                    address: frame.address,
                    data: frame.data,
                },
                AddressSpace::Unknown => InboundMessage::UnknownDataSet {
                    device_id: frame.device_id,
                    address: frame.address,
                    data: frame.data,
                },
            },
            crate::sysex::CMD_RQ1 => InboundMessage::DataRequest {
                device_id: frame.device_id,
                address: frame.address,
                size: frame.data,
            },
            _ => unreachable!("Frame::decode rejected unknown commands"),
        },
        Err(error) => InboundMessage::UnparseableSysEx {
            bytes: bytes.to_vec(),
            error,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_system_dt1() {
        let bytes = [
            0xF0, 0x41, 0x10, 0x00, 0x00, 0x00, 0x00, 0x18, 0x12, 0x10, 0x00, 0x00, 0x00, 0x00,
            0x70, 0xF7,
        ];
        match classify_inbound(&bytes) {
            InboundMessage::SystemDataSet {
                device_id,
                address,
                data,
            } => {
                assert_eq!(device_id, 0x10);
                assert_eq!(address, [0x10, 0x00, 0x00, 0x00]);
                assert_eq!(data, vec![0x00]);
            }
            other => panic!("expected SystemDataSet, got {other:?}"),
        }
    }

    #[test]
    fn classifies_non_sysex() {
        let bytes = [0x90, 0x40, 0x7F]; // Note On
        assert!(matches!(
            classify_inbound(&bytes),
            InboundMessage::NonSysEx(_)
        ));
    }

    #[test]
    fn classifies_unparseable_sysex() {
        // Starts F0 but with wrong manufacturer.
        let bytes = [0xF0, 0x42, 0x00, 0xF7];
        assert!(matches!(
            classify_inbound(&bytes),
            InboundMessage::UnparseableSysEx { .. }
        ));
    }
}
