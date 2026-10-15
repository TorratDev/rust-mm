//! NDS container: cartridge header, filesystem (FNT/FAT), ARM9/ARM7 binaries, overlay table.

#![forbid(unsafe_code)]

pub mod binary;
pub mod blz;
pub mod error;
pub mod fs;
pub mod header;
pub mod rom;

mod read;

/// Synthetic ROM builder for tests in this and other crates. Not part of the parser API.
#[cfg(any(test, feature = "test-support"))]
pub mod synth;
