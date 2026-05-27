//! Command-line surface for the RE-202 CLI.
//!
//! Subcommands:
//!   - `ports`           — list MIDI input/output ports
//!   - `dump`            — read System / Memory off the device into YAML
//!   - `sync`            — write YAML back to the device
//!   - `diff`            — compare two YAML files (or device vs. file)
//!   - `show`            — pretty-print a YAML file
//!   - `lint`            — validate a YAML file against the address map
//!   - `select N`        — set the active memory slot on the device

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "re202", version, about)]
pub struct Cli {
    /// MIDI input port name (substring match). If omitted, picks the first matching RE-202.
    #[arg(long, global = true)]
    pub input: Option<String>,

    /// MIDI output port name (substring match).
    #[arg(long, global = true)]
    pub output: Option<String>,

    /// Roland device id (0x10..=0x1F, decimal 16..=31).
    #[arg(long, global = true, default_value_t = 0x10)]
    pub device_id: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List available MIDI input and output ports.
    Ports,

    /// Read settings off the device into a YAML file.
    Dump {
        #[command(flatten)]
        target: DumpTarget,

        /// Output path. For `--all`, a directory; otherwise a YAML file.
        #[arg(short = 'o', long)]
        output: PathBuf,
    },

    /// Send YAML settings to the device.
    Sync {
        #[command(flatten)]
        target: SyncTarget,

        /// Input path. For `--all`, a directory; otherwise a YAML file.
        #[arg(short = 'i', long)]
        input: PathBuf,
    },

    /// Compare two configurations.
    Diff {
        /// Left side. Use `--device` to capture live, otherwise a path.
        left: String,
        /// Right side. Use `--device` to capture live, otherwise a path.
        right: String,
    },

    /// Pretty-print a YAML file as a human-readable summary.
    Show { path: PathBuf },

    /// Validate a YAML file (ranges, enums, address map coverage).
    Lint { path: PathBuf },

    /// Set the active memory slot on the device (1..=127).
    Select { slot: u8 },
}

#[derive(clap::Args, Debug)]
#[group(required = true, multiple = false)]
pub struct DumpTarget {
    /// Dump the System area (global settings).
    #[arg(long)]
    pub system: bool,
    /// Dump memory slot N (1..=127).
    #[arg(long, value_name = "N")]
    pub memory: Option<u8>,
    /// Dump everything: system + all memory slots, into a directory.
    #[arg(long)]
    pub all: bool,
}

#[derive(clap::Args, Debug)]
#[group(required = true, multiple = false)]
pub struct SyncTarget {
    /// Sync System area only.
    #[arg(long)]
    pub system: bool,
    /// Sync memory slot N (1..=127).
    #[arg(long, value_name = "N")]
    pub memory: Option<u8>,
    /// Sync everything in a dump directory.
    #[arg(long)]
    pub all: bool,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Ports => crate::midi::list_ports(),
        Command::Dump { target, output } => {
            log::info!("dump {target:?} -> {}", output.display());
            anyhow::bail!("dump: not implemented yet — Day 5 milestone")
        }
        Command::Sync { target, input } => {
            log::info!("sync {target:?} <- {}", input.display());
            anyhow::bail!("sync: not implemented yet — Day 6 milestone")
        }
        Command::Diff { left, right } => {
            log::info!("diff {left} vs {right}");
            anyhow::bail!("diff: not implemented yet — Day 6 milestone")
        }
        Command::Show { path } => {
            let yaml = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            print!("{yaml}");
            Ok(())
        }
        Command::Lint { path } => {
            log::info!("lint {}", path.display());
            anyhow::bail!("lint: not implemented yet — Day 6+ milestone")
        }
        Command::Select { slot } => {
            log::info!("select slot {slot}");
            anyhow::bail!("select: not implemented yet — needs address map")
        }
    }
}
