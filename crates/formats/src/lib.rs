//! Game-specific binary formats: compression, graphics, text, data tables.
//!
//! Every decoder takes `&[u8]`, bounds-checks each read and returns [`error::FormatError`]
//! on malformed input instead of panicking.

#![forbid(unsafe_code)]

pub mod archive;
pub mod compress;
pub mod error;
pub mod font;
pub mod gfx;
pub mod graphics;
pub mod ground;
pub mod portrait;
pub mod px;
pub mod read;
pub mod sir0;
pub mod sprite;
pub mod text;
pub mod tileset;
