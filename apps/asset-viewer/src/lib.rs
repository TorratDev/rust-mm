//! Asset viewer (#39) and resource index (#40) for the user's own dump.
//!
//! The library holds what the GUI and the `index` CLI share: ROM loading ([`game`]),
//! rendering each resource kind to RGBA ([`view`]), the index ([`index`]) and the PNG
//! writer for exports ([`png`]). Nothing here embeds ROM data.

#![forbid(unsafe_code)]

pub mod game;
pub mod index;
pub mod png;
pub mod view;
