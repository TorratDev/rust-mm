//! Errors for ROM container parsing. Every parser returns these instead of panicking.

use std::fmt;

/// A malformed, truncated or out-of-range structure in a ROM.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NdsError {
    /// The input is shorter than the structure being read.
    Truncated {
        what: &'static str,
        need: usize,
        have: usize,
    },
    /// A stored offset and size point outside the ROM.
    OutOfBounds {
        what: &'static str,
        offset: u64,
        size: u64,
        rom_len: u64,
    },
    /// The header CRC-16 does not match the header bytes.
    BadCrc { stored: u16, computed: u16 },
    /// The structure is internally inconsistent.
    Malformed(&'static str),
    /// A decompressed size exceeds the caller's limit.
    TooLarge { size: usize, max: usize },
}

impl fmt::Display for NdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NdsError::Truncated { what, need, have } => {
                write!(f, "{what}: need {need} bytes, have {have}")
            }
            NdsError::OutOfBounds {
                what,
                offset,
                size,
                rom_len,
            } => write!(
                f,
                "{what}: range {offset:#x}+{size:#x} is outside the {rom_len:#x}-byte ROM"
            ),
            NdsError::BadCrc { stored, computed } => write!(
                f,
                "header CRC mismatch: stored {stored:#06x}, computed {computed:#06x}"
            ),
            NdsError::Malformed(what) => write!(f, "malformed: {what}"),
            NdsError::TooLarge { size, max } => {
                write!(f, "decompressed size {size} exceeds limit {max}")
            }
        }
    }
}

impl std::error::Error for NdsError {}
