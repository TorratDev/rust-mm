//! Typed game data: species, moves, items, dungeons. No SDL, no platform code.

#![forbid(unsafe_code)]
// Determinism: integer/fixed-point math only. Banned types and methods
// (HashMap/HashSet, f32/f64, SystemTime/Instant, ...) are listed in this
// crate's clippy.toml; scripts/check-determinism.sh backs this up in CI.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]
