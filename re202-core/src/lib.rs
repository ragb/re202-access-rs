//! Pure codec for the BOSS RE-202 Space Echo SysEx protocol.
//!
//! No I/O, no threads, no MIDI backend — compiles for `wasm32-unknown-unknown`.

#![forbid(unsafe_code)]

pub mod address;
pub mod inbound;
pub mod memory;
pub mod sysex;
pub mod system;
pub mod yaml;

pub use inbound::{classify_inbound, InboundMessage};
pub use sysex::{Frame, SysExError, CMD_DT1, CMD_RQ1, RE202_MODEL_ID, ROLAND_ID};
