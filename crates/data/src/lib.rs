//! Typed game data: species, moves, items, dungeons. No SDL, no platform code.
//!
//! Every table parses from the bytes of its ROM sub-file (`system.sbin` `monspara`,
//! `wazapara`, `itempara`, `lvmpNNN`; `dungeon.sbin` `mapparam`; the type chart in
//! decoded ARM9 overlay 1) and returns [`FormatError`] on malformed input. Layouts match
//! the Red decomp's structs (`pmd-red` `MonsterDataEntry`, `MoveDataEntry`,
//! `ItemDataEntry`, `DungeonMapParam2`, `FloorProperties`); values are Blue's. Spec and
//! the Red/Blue deltas: `docs/formats/data.md`, `docs/blue-vs-red.md`.

#![forbid(unsafe_code)]
// Determinism: integer/fixed-point math only. Banned types and methods
// (HashMap/HashSet, f32/f64, SystemTime/Instant, ...) are listed in this
// crate's clippy.toml; scripts/check-determinism.sh backs this up in CI.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

pub mod dungeon;
pub mod item;
pub mod moves;
pub mod species;
pub mod types;

pub use rescue_formats::error::FormatError;

/// Result alias.
pub type Result<T> = std::result::Result<T, FormatError>;

/// Decodes a Latin-1 game string.
pub(crate) fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// Number of `size`-byte records between the SIR0 content header and the pointer list.
pub(crate) fn record_count(s: &rescue_formats::sir0::Sir0<'_>, start: usize, size: usize) -> usize {
    s.pointer_list_offset.saturating_sub(start) / size
}
