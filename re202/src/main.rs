//! RE-202 CLI entry point.
//!
//! The whole command surface — ports / identity / dump / sync / show / lint /
//! diff / schema / catalog / resolve — is the generic engine in
//! `midi-access-cli`, dispatched through [`re202_core::Re202`]'s [`Device`] impl.
//!
//! [`Device`]: midi_access_core::Device

use std::process::ExitCode;

fn main() -> ExitCode {
    midi_access_cli::run::<re202_core::Re202>()
}
