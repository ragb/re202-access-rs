//! Thin midir wrapper. The CLI does not own MIDI lifetime nuances yet — Day 5.

use anyhow::Result;
use midir::{MidiInput, MidiOutput};
use thiserror::Error;

/// midir's ALSA-backed `ConnectError` contains a `Cell<bool>` and is `!Sync`.
/// Naming this field `source` would also trip thiserror's auto-`#[source]`
/// machinery and demand `Error + Sized`. So we stringify the cause at the
/// boundary and keep our own error `Send + Sync + 'static`.
#[derive(Debug, Error)]
pub enum MidiError {
    #[error("failed to open MIDI port: {reason}")]
    Open { reason: String },
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
