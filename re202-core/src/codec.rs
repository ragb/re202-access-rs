//! Errors shared by the typed System / Memory codecs.

use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("expected {expected} bytes, got {actual}")]
    WrongLength { expected: usize, actual: usize },

    #[error("invalid {field} value 0x{value:02X} (valid: {valid})")]
    InvalidValue {
        field: &'static str,
        value: u8,
        valid: &'static str,
    },

    #[error("{field} out of range: {value} (valid: {valid})")]
    OutOfRange {
        field: &'static str,
        value: u16,
        valid: &'static str,
    },
}
