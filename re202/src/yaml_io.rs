//! Path-aware YAML helpers for the typed core models.
//!
//! The string codec lives in `re202_core::yaml`; this module adds the file I/O
//! plus the `# yaml-language-server: $schema=...` header line on write.

use std::path::Path;

use anyhow::{Context, Result};
use re202_core::yaml::{
    memory_from_yaml_str, memory_to_yaml_string, system_from_yaml_str, system_to_yaml_string,
    MEMORY_YAML_HEADER, SYSTEM_YAML_HEADER,
};
use re202_core::{Memory, SystemArea};

fn write_yaml(path: &Path, header: &str, body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    let mut out = String::new();
    out.push_str(header);
    out.push('\n');
    out.push_str(body);
    std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))
}

pub fn write_system(path: &Path, system: &SystemArea) -> Result<()> {
    let body = system_to_yaml_string(system).context("serialize SystemArea")?;
    write_yaml(path, SYSTEM_YAML_HEADER, &body)
}

pub fn write_memory(path: &Path, memory: &Memory) -> Result<()> {
    let body = memory_to_yaml_string(memory).context("serialize Memory")?;
    write_yaml(path, MEMORY_YAML_HEADER, &body)
}

pub fn read_system(path: &Path) -> Result<SystemArea> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    system_from_yaml_str(&s).with_context(|| format!("parsing SystemArea from {}", path.display()))
}

pub fn read_memory(path: &Path) -> Result<Memory> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    memory_from_yaml_str(&s).with_context(|| format!("parsing Memory from {}", path.display()))
}
