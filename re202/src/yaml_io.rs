//! Path-aware YAML helpers. Wraps `re202_core::yaml` with file I/O.
//!
//! Adds the `# yaml-language-server: $schema=...` header on write so the user
//! gets schema validation in their editor of choice.

#![allow(dead_code)]

use std::path::Path;

use anyhow::{Context, Result};
use re202_core::yaml::YAML_SCHEMA_HEADER;

pub fn write_with_schema_header(path: &Path, body: &str) -> Result<()> {
    let mut out = String::new();
    out.push_str(YAML_SCHEMA_HEADER);
    out.push('\n');
    out.push_str(body);
    std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))
}

pub fn read_to_string(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}
