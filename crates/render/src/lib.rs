//! Tilemaps, sprites, palettes, animation and screen composition.
//!
//! API-agnostic (no SDL): [`screens`] holds the layout, integer-scale, letterbox and
//! screen-gap math. The SDL3 render targets that use it live in `apps/game`.

pub mod screens;

pub use screens::{Rect, SCREEN_HEIGHT, SCREEN_WIDTH, ScreenLayout};
