//! YAML string codec (pure, no `Path`, no `fs::*`).
//!
//! The CLI layer wraps these with file I/O and the
//! `# yaml-language-server: $schema=...` header.

#![allow(dead_code)]

/// Schema comment the CLI layer prepends when writing YAML files.
pub const YAML_SCHEMA_HEADER: &str =
    "# yaml-language-server: $schema=https://example.invalid/re202-schema.json";
