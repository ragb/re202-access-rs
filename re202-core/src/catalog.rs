//! Value catalogs for the RE-202.
//!
//! The RE-202 exposes **no** name↔number lookup tables: every "named" value in
//! its model (input source, head mode, reverb type, MIDI channel, …) is already
//! a serde enum that serializes as its own name, so there is nothing to resolve.
//! This [`Catalogs`] implementation is therefore empty — name resolution is a
//! no-op for this device — but it still satisfies the [`Device`] contract and
//! gives the `catalog` command its standard `{device, params, catalogs,
//! defaults}` bundle shape.
//!
//! (Editor-facing [`ParamMeta`] for the System / Memory fields could be added
//! here in a later pass; the codec has none today, so the param table is empty.)
//!
//! [`Catalogs`]: midi_access_core::Catalogs
//! [`Device`]: midi_access_core::Device
//! [`ParamMeta`]: midi_access_core::ParamMeta

use midi_access_core::{Catalogs, Params};
use serde_yaml::{Mapping, Value};

/// The RE-202's (empty) value catalogs — the device-facing half of name
/// resolution. The device has no name↔number tables, so every lookup returns
/// `None` and the walk leaves documents untouched.
pub struct Re202Catalogs;

/// The shared singleton, referenced by the [`Device`](midi_access_core::Device)
/// impl.
pub static RE202_CATALOGS: Re202Catalogs = Re202Catalogs;

impl Catalogs for Re202Catalogs {
    fn resolve(&self, _cat: &str, _name: &str) -> Option<i64> {
        None
    }

    fn label(&self, _cat: &str, _value: i64) -> Option<String> {
        None
    }

    fn names(&self) -> &[&str] {
        &[]
    }

    fn as_value(&self) -> Value {
        // No catalogs: an empty mapping keeps the bundle's `catalogs` field a
        // consistent object across devices.
        Value::Mapping(Mapping::new())
    }
}

/// The RE-202's editor-facing parameter metadata table.
///
/// Empty for now: the codec carries no label/group/help/catalog metadata. The
/// `catalog` command still emits a valid (empty) `params` array.
pub fn params() -> Params {
    Params(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_are_empty_but_well_formed() {
        let c = Re202Catalogs;
        assert!(c.resolve("anything", "name").is_none());
        assert!(c.label("anything", 0).is_none());
        assert!(c.names().is_empty());
        assert_eq!(c.as_value(), Value::Mapping(Mapping::new()));
        assert!(params().as_slice().is_empty());
    }
}
