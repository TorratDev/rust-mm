//! Tilemaps, sprites, palettes, animation and screen composition.
//!
//! API-agnostic (no SDL): [`screens`] holds the layout, integer-scale, letterbox and
//! screen-gap math. The SDL3 render targets live in `rescue-platform`, which uses this layout from #297.

pub mod screens;

pub use screens::{Rect, SCREEN_HEIGHT, SCREEN_WIDTH, ScreenLayout};
