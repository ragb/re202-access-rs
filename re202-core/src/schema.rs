//! JSON Schema generation for the typed System / Memory models.
//!
//! Gated behind `feature = "schema"`. The CLI enables this and exposes the
//! schema via `re202 schema [system|memory]`.

use schemars::schema::RootSchema;

use crate::{Memory, SystemArea};

pub fn system_area_schema() -> RootSchema {
    schemars::schema_for!(SystemArea)
}

pub fn memory_schema() -> RootSchema {
    schemars::schema_for!(Memory)
}
