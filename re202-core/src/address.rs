//! Parameter address map for the RE-202.
//!
//! Layout (from the official MIDI Implementation v1.00, see `docs/sysex-notes.md`):
//!
//! | Prefix              | Area                 |
//! |---------------------|----------------------|
//! | `10 00 00 00`       | System (18 params)   |
//! | `20 10 00 00`       | Memory MANUAL        |
//! | `20 20 00 00`       | Memory 1             |
//! | `30 00 00 00`       | Memory 127           |
//!
//! Memory stride between consecutive slots is `00 10 00 00` (with carry).

#![allow(dead_code)]

/// Base address of the System area.
pub const SYSTEM_BASE: [u8; 4] = [0x10, 0x00, 0x00, 0x00];

/// Base address of MEMORY MANUAL (the live "manual mode" slot).
pub const MEMORY_MANUAL_BASE: [u8; 4] = [0x20, 0x10, 0x00, 0x00];

/// Base address of the **edit-buffer mirror** — a live, read/write mirror of the
/// currently-active memory's 33-byte block.
///
/// Not in the official MIDI Implementation PDF — discovered by device probing on
/// 2026-05-27. RQ1 to this address returns the same 33 bytes as RQ1 to the
/// currently-active slot's base address, and the contents update when the user
/// switches slots via the MEMORY footswitch. Verified: changing from MEMORY 1
/// to MEMORY 2 caused the mirror's contents to swap to match MEMORY 2.
///
/// Useful for live editing without committing to a slot.
pub const EDIT_BUFFER_BASE: [u8; 4] = [0x20, 0x00, 0x00, 0x00];

/// Per-memory block size in bytes (offsets `00 00`..`00 20`, inclusive).
pub const MEMORY_BLOCK_LEN: usize = 33;

/// Highest user memory slot number.
pub const MEMORY_SLOT_MAX: u8 = 127;

/// Top-level partition of the RE-202 address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpace {
    /// `10 xx xx xx` — global settings.
    System,
    /// `20 xx xx xx` or `30 00 00 00` — manual + 127 saved memory slots.
    Memory,
    /// Anything we haven't classified yet.
    Unknown,
}

impl AddressSpace {
    pub fn classify(address: [u8; 4]) -> Self {
        match address[0] {
            0x10 => Self::System,
            // 0x20 0x00 ... = edit-buffer mirror;
            // 0x20 0x10 ... = MEMORY MANUAL;
            // 0x20 0x20 ... through 0x30 0x00 ... = MEMORY 1..=127.
            0x20 | 0x30 => Self::Memory,
            _ => Self::Unknown,
        }
    }
}

/// Identifies a memory slot: MANUAL (the live patch) or one of 127 user memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySlot {
    Manual,
    User(u8),
}

impl MemorySlot {
    /// Base address of this slot's 33-byte block.
    ///
    /// Encoding: starting at `20 10 00 00` (MANUAL), advance by `00 10 00 00`
    /// per slot, carrying from the second byte into the first when it exceeds `0x7F`.
    /// Slot 127 lands at `30 00 00 00`.
    pub fn base_address(self) -> [u8; 4] {
        let index: u16 = match self {
            MemorySlot::Manual => 0,
            MemorySlot::User(n) => n as u16,
        };
        // We've used `0x10` blocks of 0x10 bytes per slot in the second byte;
        // carrying into the first byte when the second wraps past 0x7F.
        let offset_2nd = ((index + 1) * 0x10) as u32;
        let high = 0x20 + (offset_2nd >> 7);
        let low = offset_2nd & 0x7F;
        [high as u8, low as u8, 0x00, 0x00]
    }

    /// Try to construct from a user-facing index. `0` means MANUAL, `1..=127`
    /// means User slots. Returns `None` for out-of-range.
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(MemorySlot::Manual),
            n if n <= MEMORY_SLOT_MAX => Some(MemorySlot::User(n)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_known_addresses() {
        assert_eq!(AddressSpace::classify(SYSTEM_BASE), AddressSpace::System);
        assert_eq!(
            AddressSpace::classify(MEMORY_MANUAL_BASE),
            AddressSpace::Memory
        );
        assert_eq!(
            AddressSpace::classify([0x30, 0x00, 0x00, 0x00]),
            AddressSpace::Memory
        );
        assert_eq!(
            AddressSpace::classify([0x40, 0x00, 0x00, 0x00]),
            AddressSpace::Unknown
        );
    }

    /// Spec anchors: MANUAL at 20 10 00 00, MEMORY 1 at 20 20 00 00, MEMORY 127 at 30 00 00 00.
    #[test]
    fn memory_slot_addresses_match_spec() {
        assert_eq!(MemorySlot::Manual.base_address(), [0x20, 0x10, 0x00, 0x00]);
        assert_eq!(MemorySlot::User(1).base_address(), [0x20, 0x20, 0x00, 0x00]);
        assert_eq!(
            MemorySlot::User(127).base_address(),
            [0x30, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn memory_stride_is_0x10_with_carry() {
        // Walk a few addresses to make sure the carry into the high byte works.
        // From MANUAL (0,0x10) to slot 7 should advance through 0x80; (0+1)*0x10..(7+1)*0x10
        // = 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80. The 0x80 step requires carry.
        assert_eq!(MemorySlot::User(6).base_address(), [0x20, 0x70, 0x00, 0x00]);
        assert_eq!(MemorySlot::User(7).base_address(), [0x21, 0x00, 0x00, 0x00]);
        assert_eq!(MemorySlot::User(8).base_address(), [0x21, 0x10, 0x00, 0x00]);
    }

    #[test]
    fn from_index_rejects_out_of_range() {
        assert!(matches!(
            MemorySlot::from_index(0),
            Some(MemorySlot::Manual)
        ));
        assert!(matches!(
            MemorySlot::from_index(127),
            Some(MemorySlot::User(127))
        ));
        assert!(MemorySlot::from_index(128).is_none());
    }
}
