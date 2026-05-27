//! Command-line surface for the RE-202 CLI.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use re202_core::address::{MemorySlot, EDIT_BUFFER_BASE, MEMORY_BLOCK_LEN, SYSTEM_BASE};
use re202_core::system::SYSTEM_AREA_LEN;
use re202_core::{Memory, SystemArea};

use crate::midi::MidiSession;
use crate::yaml_io;

const REQUEST_TIMEOUT: Duration = Duration::from_millis(2500);

#[derive(Parser, Debug)]
#[command(name = "re202", version, about)]
pub struct Cli {
    /// MIDI port name substring used for both input and output. Use `--input-port` and
    /// `--output-port` to override one direction independently.
    #[arg(long, global = true)]
    pub port: Option<String>,

    /// MIDI input port name (substring match). Overrides `--port`.
    #[arg(long, global = true)]
    pub input_port: Option<String>,

    /// MIDI output port name (substring match). Overrides `--port`.
    #[arg(long, global = true)]
    pub output_port: Option<String>,

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

    /// Read settings off the device into a YAML file (or directory, for `--all`).
    Dump {
        #[command(flatten)]
        target: Target,
        /// Output path: a YAML file for single-target dumps, a directory for `--all`.
        #[arg(short = 'o', long)]
        output: PathBuf,
    },

    /// Send YAML settings to the device.
    Sync {
        #[command(flatten)]
        target: Target,
        /// Input path: a YAML file, or a directory previously written by `dump --all`.
        #[arg(short = 'i', long)]
        input: PathBuf,
        /// After writing, read the same address back and verify the bytes match.
        /// Catches silent write failures and confirms the device accepted the data.
        #[arg(long)]
        verify: bool,
    },

    /// Compare two configurations (file vs. file). YAML on both sides.
    Diff { left: PathBuf, right: PathBuf },

    /// Pretty-print a YAML file.
    Show { path: PathBuf },

    /// Validate a YAML file (decodes it through the typed model).
    Lint { path: PathBuf },

    /// Set the active memory slot via Program Change.
    Select {
        /// Slot: `manual` or 1..=127.
        slot: String,
    },

    /// Print the JSON Schema for one of the typed models.
    Schema {
        /// Which schema: `system` or `memory`.
        kind: String,
    },

    /// Send a Universal Identity Request and print the device's reply.
    Identity,
}

#[derive(clap::Args, Debug)]
#[group(required = true, multiple = false)]
pub struct Target {
    /// Target the System area (global settings).
    #[arg(long)]
    pub system: bool,
    /// Target memory slot: `manual` or 1..=127.
    #[arg(long, value_name = "SLOT")]
    pub memory: Option<String>,
    /// Target the edit-buffer mirror (the currently-active memory).
    #[arg(long)]
    pub edit: bool,
    /// Target everything: system + edit + memory_manual + memory_001..=127.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Clone, Copy)]
enum SingleTarget {
    System,
    Memory(MemorySlot),
    EditBuffer,
}

impl SingleTarget {
    fn parse(target: &Target) -> Result<Option<Self>> {
        if target.all {
            return Ok(None);
        }
        if target.system {
            return Ok(Some(Self::System));
        }
        if target.edit {
            return Ok(Some(Self::EditBuffer));
        }
        if let Some(s) = &target.memory {
            return Ok(Some(Self::Memory(parse_slot(s)?)));
        }
        bail!("no target selected — pass --system, --memory N, --edit, or --all")
    }
}

fn parse_slot(s: &str) -> Result<MemorySlot> {
    if s.eq_ignore_ascii_case("manual") {
        return Ok(MemorySlot::Manual);
    }
    let n: u8 = s
        .parse()
        .with_context(|| format!("expected `manual` or 1..=127, got {s:?}"))?;
    MemorySlot::from_index(n)
        .filter(|s| matches!(s, MemorySlot::User(_)))
        .ok_or_else(|| anyhow!("memory slot {n} out of range (valid: manual, 1..=127)"))
}

pub fn run(cli: Cli) -> Result<()> {
    let Cli {
        port,
        input_port,
        output_port,
        device_id,
        command,
    } = cli;
    let session_args = SessionArgs {
        port,
        input_port,
        output_port,
        device_id,
    };

    match command {
        Command::Ports => crate::midi::list_ports(),

        Command::Dump { target, output } => {
            let mut session = open_session(&session_args)?;
            match SingleTarget::parse(&target)? {
                Some(t) => dump_single(&mut session, t, &output),
                None => dump_all(&mut session, &output),
            }
        }

        Command::Sync {
            target,
            input,
            verify,
        } => {
            let mut session = open_session(&session_args)?;
            match SingleTarget::parse(&target)? {
                Some(t) => sync_single(&mut session, t, &input, verify),
                None => sync_all(&mut session, &input, verify),
            }
        }

        Command::Diff { left, right } => diff_files(&left, &right),

        Command::Show { path } => show(&path),

        Command::Lint { path } => lint(&path),

        Command::Select { slot } => {
            let mut session = open_session(&session_args)?;
            select(&mut session, &slot)
        }

        Command::Schema { kind } => emit_schema(&kind),

        Command::Identity => {
            let mut session = open_session(&session_args)?;
            identity(&mut session)
        }
    }
}

fn identity(session: &mut MidiSession) -> Result<()> {
    let reply = session.identity_request(REQUEST_TIMEOUT)?;
    let hex: String = reply
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("reply: {hex}");
    // Parse the standard fields:
    //   F0 7E [dev] 06 02 [manuf] [family-lo family-hi] [number-lo number-hi]
    //   [sw1 sw2 sw3 sw4] F7
    if reply.len() < 15 {
        anyhow::bail!("identity reply too short ({} bytes)", reply.len());
    }
    let dev = reply[2];
    let manuf = reply[5];
    let family = [reply[6], reply[7]];
    let number = [reply[8], reply[9]];
    let sw = [reply[10], reply[11], reply[12], reply[13]];
    println!("  device id:        0x{dev:02X}");
    println!(
        "  manufacturer:     0x{manuf:02X} ({})",
        manufacturer_name(manuf)
    );
    println!("  family code:      {family:02X?}");
    println!("  family number:    {number:02X?}");
    println!(
        "  software version: {sw:02X?} (decimal: {} {} {} {})",
        sw[0], sw[1], sw[2], sw[3]
    );
    if manuf == 0x41 && family == [0x18, 0x04] {
        println!("  -> matches RE-202 (Roland family 0x0418)");
    }
    Ok(())
}

fn manufacturer_name(id: u8) -> &'static str {
    match id {
        0x41 => "Roland",
        0x42 => "Korg",
        0x47 => "Akai",
        _ => "unknown",
    }
}

fn emit_schema(kind: &str) -> Result<()> {
    let schema = match kind.to_ascii_lowercase().as_str() {
        "system" => re202_core::schema::system_area_schema(),
        "memory" => re202_core::schema::memory_schema(),
        other => bail!("unknown schema kind {other:?} (valid: system, memory)"),
    };
    let json = serde_json::to_string_pretty(&schema).context("serialize schema")?;
    println!("{json}");
    Ok(())
}

struct SessionArgs {
    port: Option<String>,
    input_port: Option<String>,
    output_port: Option<String>,
    device_id: u8,
}

fn open_session(args: &SessionArgs) -> Result<MidiSession> {
    let port = args.port.as_deref();
    let input = args.input_port.as_deref().or(port).ok_or_else(|| {
        anyhow!("--input-port or --port required (use `re202 ports` to list available)")
    })?;
    let output = args.output_port.as_deref().or(port).ok_or_else(|| {
        anyhow!("--output-port or --port required (use `re202 ports` to list available)")
    })?;
    if !(0x10..=0x1F).contains(&args.device_id) {
        bail!(
            "--device-id {:#04X} out of range (must be 0x10..=0x1F)",
            args.device_id
        );
    }
    log::info!(
        "opening MIDI: input ~{input:?}, output ~{output:?}, device id {:#04X}",
        args.device_id
    );
    Ok(MidiSession::open_with(input, output, args.device_id)?)
}

// === dump ===

fn dump_single(session: &mut MidiSession, target: SingleTarget, output: &Path) -> Result<()> {
    match target {
        SingleTarget::System => {
            let frame = session.request(SYSTEM_BASE, SYSTEM_AREA_LEN as u32, REQUEST_TIMEOUT)?;
            let system = SystemArea::from_bytes(&frame.data)?;
            yaml_io::write_system(output, &system)?;
            println!("wrote {}", output.display());
        }
        SingleTarget::EditBuffer => {
            let frame =
                session.request(EDIT_BUFFER_BASE, MEMORY_BLOCK_LEN as u32, REQUEST_TIMEOUT)?;
            let memory = Memory::from_bytes(&frame.data)?;
            yaml_io::write_memory(output, &memory)?;
            println!("wrote {}", output.display());
        }
        SingleTarget::Memory(slot) => {
            let frame = session.request(
                slot.base_address(),
                MEMORY_BLOCK_LEN as u32,
                REQUEST_TIMEOUT,
            )?;
            let memory = Memory::from_bytes(&frame.data)?;
            yaml_io::write_memory(output, &memory)?;
            println!("wrote {} ({})", output.display(), slot_label(slot));
        }
    }
    Ok(())
}

fn dump_all(session: &mut MidiSession, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    dump_single(session, SingleTarget::System, &dir.join("system.yaml"))?;
    dump_single(session, SingleTarget::EditBuffer, &dir.join("edit.yaml"))?;
    dump_single(
        session,
        SingleTarget::Memory(MemorySlot::Manual),
        &dir.join("memory_manual.yaml"),
    )?;
    for n in 1u8..=127 {
        let slot = MemorySlot::User(n);
        let path = dir.join(format!("memory_{n:03}.yaml"));
        dump_single(session, SingleTarget::Memory(slot), &path)?;
    }
    Ok(())
}

// === sync ===

fn sync_single(
    session: &mut MidiSession,
    target: SingleTarget,
    input: &Path,
    verify: bool,
) -> Result<()> {
    let (address, bytes, label): ([u8; 4], Vec<u8>, String) = match target {
        SingleTarget::System => {
            let system = yaml_io::read_system(input)?;
            (
                SYSTEM_BASE,
                system.to_bytes()?.to_vec(),
                "System area".to_string(),
            )
        }
        SingleTarget::EditBuffer => {
            let memory = yaml_io::read_memory(input)?;
            (
                EDIT_BUFFER_BASE,
                memory.to_bytes()?.to_vec(),
                "edit buffer".to_string(),
            )
        }
        SingleTarget::Memory(slot) => {
            let memory = yaml_io::read_memory(input)?;
            (
                slot.base_address(),
                memory.to_bytes()?.to_vec(),
                slot_label(slot),
            )
        }
    };

    session.send_dt1(address, &bytes)?;
    println!("sent {label} from {}", input.display());

    if verify {
        // Brief pause to let the device finish processing the write before we
        // read back. Empirically the RE-202 is ready immediately, but a small
        // gap costs nothing and helps slower interfaces.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let frame = session.request(address, bytes.len() as u32, REQUEST_TIMEOUT)?;
        if frame.data == bytes {
            println!("  verified: read-back matches ({} bytes)", bytes.len());
        } else {
            let expected = hex_short(&bytes);
            let got = hex_short(&frame.data);
            bail!(
                "verify FAILED at {address:02X?}:\n    expected: {expected}\n    got:      {got}"
            );
        }
    }
    Ok(())
}

fn sync_all(session: &mut MidiSession, dir: &Path, verify: bool) -> Result<()> {
    sync_single(
        session,
        SingleTarget::System,
        &dir.join("system.yaml"),
        verify,
    )?;
    sync_single(
        session,
        SingleTarget::Memory(MemorySlot::Manual),
        &dir.join("memory_manual.yaml"),
        verify,
    )?;
    for n in 1u8..=127 {
        let path = dir.join(format!("memory_{n:03}.yaml"));
        if path.exists() {
            sync_single(
                session,
                SingleTarget::Memory(MemorySlot::User(n)),
                &path,
                verify,
            )?;
        }
    }
    // edit.yaml deliberately not synced as part of --all: it's a snapshot of
    // the active memory at dump time, not a separate stored value.
    Ok(())
}

fn hex_short(bytes: &[u8]) -> String {
    const MAX: usize = 24;
    let parts: Vec<String> = bytes.iter().take(MAX).map(|b| format!("{b:02X}")).collect();
    let mut s = parts.join(" ");
    if bytes.len() > MAX {
        s.push_str(&format!(" ... ({} more)", bytes.len() - MAX));
    }
    s
}

// === show, lint, diff, select ===

fn show(path: &Path) -> Result<()> {
    let yaml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    // First try System, then Memory, to give a helpful preamble.
    if serde_yaml::from_str::<SystemArea>(&yaml).is_ok() {
        println!("# {} (System area)", path.display());
    } else if serde_yaml::from_str::<Memory>(&yaml).is_ok() {
        println!("# {} (Memory block)", path.display());
    } else {
        println!("# {} (unrecognized — printing raw)", path.display());
    }
    print!("{yaml}");
    Ok(())
}

fn lint(path: &Path) -> Result<()> {
    let yaml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let sys_err = serde_yaml::from_str::<SystemArea>(&yaml).err();
    let mem_err = serde_yaml::from_str::<Memory>(&yaml).err();
    match (sys_err, mem_err) {
        (None, _) => {
            let s: SystemArea = serde_yaml::from_str(&yaml)?;
            s.to_bytes()?;
            println!("OK: {} is a valid SystemArea", path.display());
        }
        (_, None) => {
            let m: Memory = serde_yaml::from_str(&yaml)?;
            m.to_bytes()?;
            println!("OK: {} is a valid Memory block", path.display());
        }
        (Some(s), Some(m)) => {
            bail!(
                "{} doesn't parse as either:\n  - as SystemArea: {s}\n  - as Memory: {m}",
                path.display()
            )
        }
    }
    Ok(())
}

fn diff_files(left: &Path, right: &Path) -> Result<()> {
    let left_text =
        std::fs::read_to_string(left).with_context(|| format!("reading {}", left.display()))?;
    let right_text =
        std::fs::read_to_string(right).with_context(|| format!("reading {}", right.display()))?;

    // Try System first.
    if let (Ok(a), Ok(b)) = (
        serde_yaml::from_str::<SystemArea>(&left_text),
        serde_yaml::from_str::<SystemArea>(&right_text),
    ) {
        return print_diff_system(&a, &b);
    }
    if let (Ok(a), Ok(b)) = (
        serde_yaml::from_str::<Memory>(&left_text),
        serde_yaml::from_str::<Memory>(&right_text),
    ) {
        return print_diff_memory(&a, &b);
    }
    bail!("both files must be either SystemArea or Memory YAML (and of the same kind)");
}

fn print_diff_system(a: &SystemArea, b: &SystemArea) -> Result<()> {
    if a == b {
        println!("(no differences)");
        return Ok(());
    }
    // Serialize each to YAML and compare line-by-line.
    let ya = serde_yaml::to_string(a)?;
    let yb = serde_yaml::to_string(b)?;
    print_textual_diff(&ya, &yb);
    Ok(())
}

fn print_diff_memory(a: &Memory, b: &Memory) -> Result<()> {
    if a == b {
        println!("(no differences)");
        return Ok(());
    }
    let ya = serde_yaml::to_string(a)?;
    let yb = serde_yaml::to_string(b)?;
    print_textual_diff(&ya, &yb);
    Ok(())
}

/// Side-by-side line diff. Lines that only differ in value (same key) print as
/// `key: a  →  b`. Other unchanged context is suppressed.
fn print_textual_diff(left: &str, right: &str) {
    let la: Vec<&str> = left.lines().collect();
    let lb: Vec<&str> = right.lines().collect();
    let n = la.len().max(lb.len());
    for i in 0..n {
        let a = la.get(i).copied().unwrap_or("");
        let b = lb.get(i).copied().unwrap_or("");
        if a == b {
            continue;
        }
        let key_a = a.split_once(':').map(|(k, _)| k.trim());
        let key_b = b.split_once(':').map(|(k, _)| k.trim());
        match (key_a, key_b) {
            (Some(ka), Some(kb)) if ka == kb => {
                let va = a.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
                let vb = b.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
                println!("  {ka}: {va}  →  {vb}");
            }
            _ => {
                println!("- {a}");
                println!("+ {b}");
            }
        }
    }
}

fn select(session: &mut MidiSession, slot: &str) -> Result<()> {
    let n: u8 = if slot.eq_ignore_ascii_case("manual") {
        0
    } else {
        let parsed: u8 = slot
            .parse()
            .with_context(|| format!("expected `manual` or 1..=127, got {slot:?}"))?;
        if !(1..=127).contains(&parsed) {
            bail!("slot {parsed} out of range (valid: manual, 1..=127)");
        }
        parsed
    };
    // Program Change on MIDI channel 1. The device must have MIDI PC In = ON
    // and Rx Channel = 1 for this to switch slots.
    let status: u8 = 0xC0;
    session.send_raw(&[status, n])?;
    println!(
        "sent Program Change {n} on channel 1 (slot: {})",
        if n == 0 {
            "MANUAL".to_string()
        } else {
            format!("MEMORY {n}")
        }
    );
    log::info!("ensure MIDI PC In is ON and Rx Channel matches if the slot doesn't change");
    Ok(())
}

fn slot_label(slot: MemorySlot) -> String {
    match slot {
        MemorySlot::Manual => "MEMORY MANUAL".to_string(),
        MemorySlot::User(n) => format!("MEMORY {n}"),
    }
}
