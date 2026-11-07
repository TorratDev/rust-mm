//! Tilemaps, sprites, palettes, animation and screen composition.
//!
//! API-agnostic (no SDL): [`screens`] holds the layout, integer-scale, letterbox and
//! screen-gap math, [`bg`] composes background layers into a 256x192 RGBA frame and
//! [`palette`] animates and fades palettes. The SDL3 render targets live in
//! `rescue-platform`, which uses this layout from #297.

pub mod bg;
pub mod palette;
pub mod screens;

pub use bg::{BgLayer, Frame, compose, ground_layer};
pub use palette::{Fade, FadeTarget, PaletteAnimator};
pub use screens::{Rect, SCREEN_HEIGHT, SCREEN_WIDTH, ScreenLayout};

#[cfg(test)]
mod rom_tests;
