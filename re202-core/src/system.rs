//! System area (global settings) — `0x10 xx xx xx`.
//!
//! Empty until we have device-captured fixtures.
//! Expected contents (per the RE-202 reference manual, MIDI section):
//!   - Input Source (Guitar / Line)            confirmed addr: 0x10 0x00 0x00 0x00
//!   - MIDI RX channel
//!   - MIDI TX channel
//!   - MIDI Clock source / sync
//!   - Device ID
//!   - Tempo source, Bypass behavior, etc.
