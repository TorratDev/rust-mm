//! Deterministic, headless dungeon and game simulation: turns, combat, AI, RNG. No SDL, no I/O.

#![forbid(unsafe_code)]
// Determinism: integer/fixed-point math only. Banned types and methods
// (HashMap/HashSet, f32/f64, SystemTime/Instant, ...) are listed in this
// crate's clippy.toml; scripts/check-determinism.sh backs this up in CI.
#![deny(clippy::float_arithmetic)]
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

mod ai;
pub mod combat;
pub mod command;
pub mod floor;
pub mod floor_gen;
pub mod item_use;
pub mod items;
mod move_effects;
pub mod rng;
pub mod shop;
pub mod state;
pub mod status;
mod targeting;
pub mod turn;
pub mod visibility;
