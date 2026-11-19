//! Title-menu backdrop screens (`titlemenu.sbin` `titlenK` and `subdef<lang>`) and the title
//! fade colour formula. Spec: `docs/formats/graphics.md#title-menu-screens`.
//!
//! The file is AT4PX; the decoded blob holds two 32×32 tilemaps (0x800 bytes each) and then
//! 4bpp tiles. The matching `...p` file is 224 RGBX colours.

use crate::gfx::{self, MapEntry, TILE_PIXELS};
use crate::graphics::{self, Palette};
use crate::px;
use crate::read::{self as bytes, Error, Result};

/// Entries in one screen map (32×32).
pub const MAP_ENTRIES: usize = 32 * 32;
/// Bytes in one screen map.
pub const MAP_BYTES: usize = MAP_ENTRIES * 2;
/// Offset of the tile data in the decoded blob.
pub const TILES_OFFSET: usize = 2 * MAP_BYTES;
/// Colours in a title palette file.
pub const PALETTE_COLORS: usize = 224;
/// Highest fade level; the fade counter runs to 32 but the level is held at 31.
pub const FADE_MAX: u32 = 31;

/// A decoded title screen: two maps and the shared tile set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleScreen {
    /// Map at blob offset 0 (engine A BG2 for `titlenK`, engine B BG2 for `subdef`).
    pub map0: Vec<MapEntry>,
    /// Map at blob offset 0x800 (engine A BG3 for `titlenK`, engine B BG3 for `subdef`).
    pub map1: Vec<MapEntry>,
    /// 4bpp tiles from blob offset 0x1000 (a trailing partial tile is dropped).
    pub tiles: Vec<[u8; TILE_PIXELS]>,
}

/// Parses an already PX-decoded title screen blob.
pub fn parse_blob(blob: &[u8]) -> Result<TitleScreen> {
    let maps = bytes::slice(blob, 0, TILES_OFFSET, "title screen maps")?;
    let (m0, m1) = maps.split_at(MAP_BYTES);
    let tile_bytes = blob.get(TILES_OFFSET..).unwrap_or_default();
    let whole = tile_bytes.len() / 32 * 32;
    Ok(TitleScreen {
        map0: gfx::decode_tilemap(m0)?,
        map1: gfx::decode_tilemap(m1)?,
        tiles: gfx::decode_4bpp_tiles(tile_bytes.get(..whole).unwrap_or_default())?,
    })
}

/// Parses a title screen file: PX-decodes it, then [`parse_blob`].
pub fn parse(file: &[u8]) -> Result<TitleScreen> {
    parse_blob(&px::decompress(file, px::DEFAULT_LIMIT)?)
}

/// Parses a title palette file: 224 RGBX colours (extra bytes are ignored).
pub fn parse_palette(data: &[u8]) -> Result<Palette> {
    graphics::rgbx_palette(data, PALETTE_COLORS)
}

/// One colour of the title fade as a BGR555 word. `rgb` are the RGBX bytes, `n` the fade
/// counter (clamped to 31): each channel is `c * n >> 8`, masked to 5 bits.
pub fn fade_color(rgb: [u8; 3], n: u32) -> u16 {
    let lvl = n.min(FADE_MAX);
    let ch = |c: u8| ((u32::from(c) * lvl) >> 8) as u16 & 31;
    ch(rgb[0]) | ch(rgb[2]) << 10 | ch(rgb[1]) << 5
}

/// Applies [`fade_color`] to every colour of a palette (alpha is ignored).
pub fn fade_palette(palette: &[[u8; 4]], n: u32) -> Vec<u16> {
    palette
        .iter()
        .map(|c| fade_color([c[0], c[1], c[2]], n))
        .collect()
}

/// Reports a decoded screen whose map points past its tile set, if any entry does.
pub fn check_tiles(screen: &TitleScreen) -> Result<()> {
    let n = screen.tiles.len();
    let bad = screen
        .map0
        .iter()
        .chain(&screen.map1)
        .any(|e| usize::from(e.tile) >= n.max(1) && e.tile != 0);
    if bad {
        return Err(Error::invalid("title screen", "map entry beyond tile data"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_layout() {
        let mut blob = vec![0u8; TILES_OFFSET + 64];
        blob[0] = 0x05;
        blob[1] = 0x14; // tile 5, hflip, palette 1
        blob[MAP_BYTES] = 0x01;
        blob[TILES_OFFSET] = 0x21;
        blob.push(0xFF); // partial tile dropped
        let s = parse_blob(&blob).unwrap();
        assert_eq!(s.map0.len(), MAP_ENTRIES);
        assert_eq!(s.map1.len(), MAP_ENTRIES);
        assert_eq!(
            (s.map0[0].tile, s.map0[0].hflip, s.map0[0].palette),
            (5, true, 1)
        );
        assert_eq!(s.map1[0].tile, 1);
        assert_eq!(s.tiles.len(), 2);
        assert_eq!(&s.tiles[0][..2], &[1, 2]);
        assert!(check_tiles(&s).is_err());
    }

    #[test]
    fn short_inputs_are_errors() {
        assert!(parse_blob(&[0; TILES_OFFSET - 1]).is_err());
        assert!(parse_blob(&[]).is_err());
        assert!(parse(b"AT4PX").is_err());
        assert!(parse_palette(&[0; 895]).is_err());
        assert_eq!(parse_palette(&[0x80; 900]).unwrap().len(), PALETTE_COLORS);
        assert_eq!(parse_blob(&[0; TILES_OFFSET]).unwrap().tiles.len(), 0);
    }

    #[test]
    fn fade_formula() {
        assert_eq!(fade_color([255, 255, 255], 31), 0x7BDE);
        assert_eq!(fade_color([255, 255, 255], 32), 0x7BDE);
        assert_eq!(fade_color([255, 255, 255], 100), 0x7BDE);
        assert_eq!(fade_color([255, 255, 255], 0), 0);
        assert_eq!(fade_color([255, 0, 0], 31), 30);
        assert_eq!(fade_color([0, 255, 0], 31), 30 << 5);
        assert_eq!(fade_color([0, 0, 255], 31), 30 << 10);
        assert_eq!(fade_color([255, 255, 255], 1), 0);
        assert_eq!(fade_color([255, 255, 255], 2), 0x0421);
        assert_eq!(fade_palette(&[[255, 255, 255, 0x80]; 2], 31), [0x7BDE; 2]);
    }
}
