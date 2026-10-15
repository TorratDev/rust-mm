//! Palette, tile and tilemap decoders on arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_formats::gfx::{
    MapEntry, decode_4bpp_tiles, decode_8bpp_tiles, decode_palette, decode_tilemap, render_tile,
};

fuzz_target!(|data: &[u8]| {
    let palette = decode_palette(data, true).unwrap_or_default();
    if let Ok(tiles) = decode_4bpp_tiles(data) {
        if let Some(tile) = tiles.first() {
            let _ = render_tile(tile, &palette, MapEntry::from_raw(0xFFFF));
        }
    }
    let _ = decode_8bpp_tiles(data);
    let _ = decode_tilemap(data);
});
