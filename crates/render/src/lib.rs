//! Tilemaps, sprites, palettes, animation and screen composition.
//!
//! API-agnostic (no SDL): [`screens`] holds the layout, integer-scale, letterbox and
//! screen-gap math, [`bg`] composes background layers into a 256x192 RGBA frame,
//! [`autotile`] picks dungeon metatiles from the terrain grid and [`palette`] animates
//! and fades palettes, [`sprite`] draws multi-piece sprites and mixes them with the layers
//! by priority, [`anim`] plays their animations and [`camera`] follows the leader. The SDL3 render targets live in `rescue-platform`, which uses this layout from #297.

pub mod anim;
pub mod autotile;
pub mod bg;
pub mod camera;
pub mod palette;
pub mod screens;
pub mod sprite;
pub mod text;

pub use anim::AnimPlayer;
pub use autotile::{AutotileError, GRID_H, GRID_W, autotile, cell_key};
pub use bg::{BgLayer, Frame, compose, ground_layer, ground_layer_at};
pub use camera::{Camera, entity_pixel, follow};
pub use palette::{Fade, FadeTarget, PaletteAnimator};
pub use screens::{Rect, SCREEN_HEIGHT, SCREEN_WIDTH, ScreenLayout};
pub use sprite::{
    PlacedSprite, SpriteImage, compose_with_sprites, render_obj, render_sprite, shadow_origin,
};
pub use text::{Canvas, Dialogue, Font, State};

#[cfg(test)]
mod rom_sprite_tests;
#[cfg(test)]
mod rom_tests;
#[cfg(test)]
mod rom_text_tests;
