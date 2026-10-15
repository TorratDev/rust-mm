//! Graphics primitives shared by every image format: BGR555 palettes, 4bpp and 8bpp
//! tiles, and tilemap entries.
//!
//! Tiles are 8×8 pixels stored as palette indices, row-major. 4bpp tiles are 32 bytes
//! (the low nibble is the left pixel). 8bpp tiles are 64 bytes. Colors are RGBA8.

use crate::error::FormatError;

/// One color.
pub type Rgba = [u8; 4];

/// Pixels in one tile (8×8).
pub const TILE_PIXELS: usize = 64;

/// Converts a BGR555 color (red in bits 0–4, blue in bits 10–14) to RGBA8 with alpha 255.
/// Each 5-bit channel is widened by replicating its top bits, so 31 maps to 255.
pub fn bgr555(c: u16) -> Rgba {
    let widen = |v: u16| {
        let v = (v & 0x1F) as u8;
        (v << 3) | (v >> 2)
    };
    [widen(c), widen(c >> 5), widen(c >> 10), 255]
}

/// Decodes a palette of little-endian BGR555 colors. With `transparent_zero`, index 0 gets
/// alpha 0, as sprite palettes do.
pub fn decode_palette(data: &[u8], transparent_zero: bool) -> Result<Vec<Rgba>, FormatError> {
    if !data.len().is_multiple_of(2) {
        return Err(FormatError::Malformed("palette length is odd"));
    }
    let mut colors: Vec<Rgba> = data
        .chunks_exact(2)
        .map(|c| bgr555(u16::from_le_bytes([c[0], c[1]])))
        .collect();
    if let Some(first) = colors.first_mut().filter(|_| transparent_zero) {
        first[3] = 0;
    }
    Ok(colors)
}

/// Decodes one 4bpp tile (32 bytes) into 64 palette indices, each in `0..16`.
pub fn decode_4bpp_tile(data: &[u8]) -> Result<[u8; TILE_PIXELS], FormatError> {
    if data.len() < 32 {
        return Err(truncated("4bpp tile", 32, data.len()));
    }
    let mut px = [0u8; TILE_PIXELS];
    for (i, &b) in data.iter().take(32).enumerate() {
        px[2 * i] = b & 0x0F;
        px[2 * i + 1] = b >> 4;
    }
    Ok(px)
}

/// Decodes one 8bpp tile (64 bytes) into 64 palette indices.
pub fn decode_8bpp_tile(data: &[u8]) -> Result<[u8; TILE_PIXELS], FormatError> {
    let mut px = [0u8; TILE_PIXELS];
    px.copy_from_slice(
        data.get(..TILE_PIXELS)
            .ok_or_else(|| truncated("8bpp tile", 64, data.len()))?,
    );
    Ok(px)
}

/// Decodes a run of 4bpp tiles. The length must be a multiple of 32.
pub fn decode_4bpp_tiles(data: &[u8]) -> Result<Vec<[u8; TILE_PIXELS]>, FormatError> {
    if !data.len().is_multiple_of(32) {
        return Err(FormatError::Malformed(
            "4bpp tile data is not a multiple of 32 bytes",
        ));
    }
    data.chunks_exact(32).map(decode_4bpp_tile).collect()
}

/// Decodes a run of 8bpp tiles. The length must be a multiple of 64.
pub fn decode_8bpp_tiles(data: &[u8]) -> Result<Vec<[u8; TILE_PIXELS]>, FormatError> {
    if !data.len().is_multiple_of(TILE_PIXELS) {
        return Err(FormatError::Malformed(
            "8bpp tile data is not a multiple of 64 bytes",
        ));
    }
    data.chunks_exact(TILE_PIXELS)
        .map(decode_8bpp_tile)
        .collect()
}

/// One tilemap cell: which tile, how it is flipped, and which palette bank it uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapEntry {
    /// Tile index (bits 0–9).
    pub tile: u16,
    /// Horizontal flip (bit 10).
    pub hflip: bool,
    /// Vertical flip (bit 11).
    pub vflip: bool,
    /// Palette bank (bits 12–15).
    pub palette: u8,
}

impl MapEntry {
    /// Splits a raw 16-bit tilemap cell into its fields.
    pub fn from_raw(raw: u16) -> Self {
        MapEntry {
            tile: raw & 0x03FF,
            hflip: raw & 0x0400 != 0,
            vflip: raw & 0x0800 != 0,
            palette: (raw >> 12) as u8,
        }
    }
}

/// Decodes a tilemap of little-endian 16-bit cells, in row-major order.
pub fn decode_tilemap(data: &[u8]) -> Result<Vec<MapEntry>, FormatError> {
    if !data.len().is_multiple_of(2) {
        return Err(FormatError::Malformed("tilemap length is odd"));
    }
    Ok(data
        .chunks_exact(2)
        .map(|c| MapEntry::from_raw(u16::from_le_bytes([c[0], c[1]])))
        .collect())
}

/// Renders one tile with its flips applied, using `bank` as the palette (index `n` gives
/// `bank[n]`). Indices outside the bank render as transparent black.
pub fn render_tile(
    tile: &[u8; TILE_PIXELS],
    bank: &[Rgba],
    entry: MapEntry,
) -> [Rgba; TILE_PIXELS] {
    let mut out = [[0u8; 4]; TILE_PIXELS];
    for y in 0..8 {
        for x in 0..8 {
            let sx = if entry.hflip { 7 - x } else { x };
            let sy = if entry.vflip { 7 - y } else { y };
            let idx = usize::from(tile[sy * 8 + sx]);
            out[y * 8 + x] = bank.get(idx).copied().unwrap_or([0, 0, 0, 0]);
        }
    }
    out
}

fn truncated(what: &'static str, need: usize, have: usize) -> FormatError {
    FormatError::Truncated { what, need, have }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgr555_channels_and_widening() {
        assert_eq!(bgr555(0x0000), [0, 0, 0, 255]);
        assert_eq!(bgr555(0x001F), [255, 0, 0, 255]); // red = 31
        assert_eq!(bgr555(0x03E0), [0, 255, 0, 255]); // green = 31
        assert_eq!(bgr555(0x7C00), [0, 0, 255, 255]); // blue = 31
        assert_eq!(bgr555(0x0010), [132, 0, 0, 255]); // 16<<3 | 16>>2
    }

    #[test]
    fn palette_transparency_and_odd_length() {
        let p = decode_palette(&[0x1F, 0x00, 0x00, 0x7C], true).unwrap();
        assert_eq!(p[0], [255, 0, 0, 0]);
        assert_eq!(p[1], [0, 0, 255, 255]);
        assert!(decode_palette(&[0x00], false).is_err());
    }

    #[test]
    fn tile_4bpp_low_nibble_is_left_pixel() {
        let mut data = [0u8; 32];
        data[0] = 0x21; // pixel 0 = 1, pixel 1 = 2
        let px = decode_4bpp_tile(&data).unwrap();
        assert_eq!(&px[..2], &[1, 2]);
        assert!(decode_4bpp_tile(&data[..31]).is_err());
    }

    #[test]
    fn tile_8bpp_and_runs() {
        let data: Vec<u8> = (0..128).map(|i| i as u8).collect();
        let tiles = decode_8bpp_tiles(&data).unwrap();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[1][0], 64);
        assert!(decode_8bpp_tiles(&data[..100]).is_err());
        assert_eq!(decode_4bpp_tiles(&[0; 64]).unwrap().len(), 2);
    }

    #[test]
    fn tilemap_fields() {
        let raw = 0x3000 | 0x0800 | 0x0400 | 0x0123;
        let e = MapEntry::from_raw(raw);
        assert_eq!(e.tile, 0x123);
        assert!(e.hflip && e.vflip);
        assert_eq!(e.palette, 3);
        let map = decode_tilemap(&raw.to_le_bytes()).unwrap();
        assert_eq!(map, vec![e]);
    }

    #[test]
    fn render_applies_flips() {
        let mut tile = [0u8; TILE_PIXELS];
        tile[0] = 1; // top-left pixel is index 1
        let bank = [[0, 0, 0, 0], [9, 9, 9, 255]];
        let plain = render_tile(&tile, &bank, MapEntry::from_raw(0));
        assert_eq!(plain[0], [9, 9, 9, 255]);
        assert_eq!(plain[7], [0, 0, 0, 0]);

        let flipped = render_tile(&tile, &bank, MapEntry::from_raw(0x0400 | 0x0800));
        assert_eq!(flipped[63], [9, 9, 9, 255]); // bottom-right after both flips
        assert_eq!(flipped[0], [0, 0, 0, 0]);
    }
}
