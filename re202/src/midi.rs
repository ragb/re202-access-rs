//! MIDI plumbing — open input + output, send Frames, wait for matching responses.
//!
//! Handles the fact that the device echoes our outgoing RQ1 back via MIDI Thru
//! (if Thru is on) — when we wait for a response, we only accept DT1 frames at
//! the address we requested.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use re202_core::{classify_inbound, Frame, InboundMessage};
use thiserror::Error;

/// midir's ALSA-backed `ConnectError` contains a `Cell<bool>` and is `!Sync`.
/// Naming this field `source` would also trip thiserror's auto-`#[source]`
/// machinery and demand `Error + Sized`. So we stringify the cause at the
/// boundary and keep our own error `Send + Sync + 'static`.
#[derive(Debug, Error)]
pub enum MidiError {
    #[error("failed to open MIDI port: {reason}")]
    Open { reason: String },
    #[error("no MIDI {kind} port matches {needle:?}")]
    PortNotFound { kind: &'static str, needle: String },
    #[error("MIDI send failed: {reason}")]
    Send { reason: String },
    #[error("timed out after {0:?} waiting for response from device")]
    Timeout(Duration),
}

pub fn list_ports() -> Result<()> {
    let mi = MidiInput::new("re202-list-in").map_err(|e| MidiError::Open {
        reason: e.to_string(),
    })?;
    let mo = MidiOutput::new("re202-list-out").map_err(|e| MidiError::Open {
        reason: e.to_string(),
    })?;

    println!("Input ports:");
    for p in mi.ports() {
        let name = mi.port_name(&p).unwrap_or_else(|_| "<unknown>".to_string());
        println!("  - {name}");
    }
    println!();
    println!("Output ports:");
    for p in mo.ports() {
        let name = mo.port_name(&p).unwrap_or_else(|_| "<unknown>".to_string());
        println!("  - {name}");
    }
    Ok(())
}

/// Active connection to one RE-202: keeps both input and output ports open and
/// owns a background callback that forwards inbound bytes to a channel.
pub struct MidiSession {
    output: MidiOutputConnection,
    rx: mpsc::Receiver<Vec<u8>>,
    /// Keeps the input port alive — dropping it closes the callback.
    _input: MidiInputConnection<()>,
    pub device_id: u8,
}

impl MidiSession {
    /// Open input and output ports by substring match.
    pub fn open_with(
        input_substring: &str,
        output_substring: &str,
        device_id: u8,
    ) -> Result<Self, MidiError> {
        let mi = MidiInput::new("re202-cli-in").map_err(|e| MidiError::Open {
            reason: e.to_string(),
        })?;
        let mo = MidiOutput::new("re202-cli-out").map_err(|e| MidiError::Open {
            reason: e.to_string(),
        })?;

        let in_port = mi
            .ports()
            .into_iter()
            .find(|p| {
                mi.port_name(p)
                    .map(|n| n.to_lowercase().contains(&input_substring.to_lowercase()))
                    .unwrap_or(false)
            })
            .ok_or_else(|| MidiError::PortNotFound {
                kind: "input",
                needle: input_substring.to_string(),
            })?;
        let out_port = mo
            .ports()
            .into_iter()
            .find(|p| {
                mo.port_name(p)
                    .map(|n| n.to_lowercase().contains(&output_substring.to_lowercase()))
                    .unwrap_or(false)
            })
            .ok_or_else(|| MidiError::PortNotFound {
                kind: "output",
                needle: output_substring.to_string(),
            })?;

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let input = mi
            .connect(
                &in_port,
                "re202",
                move |_t, msg, _| {
                    let _ = tx.send(msg.to_vec());
                },
                (),
            )
            .map_err(|e| MidiError::Open {
                reason: e.to_string(),
            })?;
        let output = mo
            .connect(&out_port, "re202")
            .map_err(|e| MidiError::Open {
                reason: e.to_string(),
            })?;

        Ok(Self {
            output,
            rx,
            _input: input,
            device_id,
        })
    }

    /// Drain any pending inbound messages (echo, clock spam, etc.).
    fn drain(&self) {
        while self.rx.try_recv().is_ok() {}
    }

    /// Send a Data Set 1.
    pub fn send_dt1(&mut self, address: [u8; 4], data: &[u8]) -> Result<(), MidiError> {
        let frame = Frame::data_set(self.device_id, address, data.to_vec());
        self.output
            .send(&frame.encode())
            .map_err(|e| MidiError::Send {
                reason: e.to_string(),
            })
    }

    /// Send raw MIDI bytes. Used for channel messages (PC, CC, etc.) that aren't SysEx.
    pub fn send_raw(&mut self, bytes: &[u8]) -> Result<(), MidiError> {
        self.output.send(bytes).map_err(|e| MidiError::Send {
            reason: e.to_string(),
        })
    }

    /// Send a Data Request 1 and wait up to `timeout` for the matching DT1 reply.
    ///
    /// `size` is encoded as a 4-byte 7-bit-safe big-endian value (Roland convention).
    pub fn request(
        &mut self,
        address: [u8; 4],
        size: u32,
        timeout: Duration,
    ) -> Result<Frame, MidiError> {
        self.drain();
        let size_bytes = [
            ((size >> 21) & 0x7F) as u8,
            ((size >> 14) & 0x7F) as u8,
            ((size >> 7) & 0x7F) as u8,
            (size & 0x7F) as u8,
        ];
        let frame = Frame::data_request(self.device_id, address, size_bytes);
        self.output
            .send(&frame.encode())
            .map_err(|e| MidiError::Send {
                reason: e.to_string(),
            })?;
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(MidiError::Timeout(timeout));
            }
            let raw = match self.rx.recv_timeout(remaining) {
                Ok(b) => b,
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(MidiError::Timeout(timeout)),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(MidiError::Send {
                        reason: "input channel disconnected".to_string(),
                    });
                }
            };
            match classify_inbound(&raw) {
                InboundMessage::SystemDataSet {
                    address: a, data, ..
                }
                | InboundMessage::MemoryDataSet {
                    address: a, data, ..
                }
                | InboundMessage::UnknownDataSet {
                    address: a, data, ..
                } if a == address => {
                    return Ok(Frame::data_set(self.device_id, a, data));
                }
                // Echoes of our outgoing RQ1, MIDI clock, PCs we don't care about — keep listening.
                _ => continue,
            }
        }
    }
}
