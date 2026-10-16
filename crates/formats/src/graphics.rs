//! Blue's own palette layout and an indexed image type the sprite, portrait and tileset
//! decoders draw into. Tile and tilemap primitives live in [`crate::gfx`]. Spec:
//! `docs/formats/graphics.md`.
//!
//! Blue stores palettes as 4-byte RGBX entries (`r, g, b, x`, `x` is 0x80 or 0), not the
//! hardware's BGR555.

use crate::gfx::{self, MapEntry, Rgba};
use crate::read::{self as bytes, Error, Result};

/// Bytes per 4bpp tile.
pub const TILE_4BPP: usize = 32;

/// A palette of RGBA colors.
pub type Palette = Vec<Rgba>;

/// Decodes `count` RGBX colors (4 bytes each), all opaque.
pub fn rgbx_palette(data: &[u8], count: usize) -> Result<Palette> {
    let len = count
        .checked_mul(4)
        .ok_or_else(|| Error::invalid("RGBX palette", "too many colors"))?;
    let s = bytes::slice(data, 0, len, "RGBX palette")?;
    Ok(s.chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], 0xFF])
        .collect())
}

/// Splits RGBX data into 16-color palettes (a trailing partial palette is ignored).
pub fn rgbx_palettes16(data: &[u8]) -> Result<Vec<Palette>> {
    data.chunks_exact(64).map(|c| rgbx_palette(c, 16)).collect()
}

/// Pixel depth of tile data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Depth {
    /// 4 bits per pixel.
    Bpp4,
    /// 8 bits per pixel.
    Bpp8,
}

impl Depth {
    /// Bytes per 8x8 tile.
    pub fn tile_bytes(self) -> usize {
        match self {
            Depth::Bpp4 => TILE_4BPP,
            Depth::Bpp8 => 64,
        }
    }
}

/// Palette indices of tile `index` in `data`.
pub fn tile_pixels(data: &[u8], depth: Depth, index: usize) -> Result<[u8; 64]> {
    let at = index
        .checked_mul(depth.tile_bytes())
        .ok_or_else(|| Error::invalid("tile", "index too large"))?;
    let t = bytes::slice(data, at, depth.tile_bytes(), "tile")?;
    match depth {
        Depth::Bpp4 => gfx::decode_4bpp_tile(t),
        Depth::Bpp8 => gfx::decode_8bpp_tile(t),
    }
}

/// An image of `(palette number, color index)` pixels; `None` is transparent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexedImage {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Row-major pixels.
    pub pixels: Vec<Option<(u8, u8)>>,
}

impl IndexedImage {
    /// A transparent image.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; width * height],
        }
    }

    /// Sets pixel `(x, y)` if it is inside the image.
    pub fn set(&mut self, x: isize, y: isize, v: Option<(u8, u8)>) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = v;
        }
    }

    /// Draws tile `entry.tile` of `tiles` at `(x, y)` with its flips and palette. With
    /// `transparent0`, color index 0 is not drawn.
    pub fn draw_tile(
        &mut self,
        tiles: &[u8],
        depth: Depth,
        entry: MapEntry,
        x: isize,
        y: isize,
        transparent0: bool,
    ) -> Result<()> {
        let px = tile_pixels(tiles, depth, usize::from(entry.tile))?;
        for ty in 0..8 {
            for tx in 0..8 {
                let sx = if entry.hflip { 7 - tx } else { tx };
                let sy = if entry.vflip { 7 - ty } else { ty };
                let c = px[sy * 8 + sx];
                if !(transparent0 && c == 0) {
                    self.set(x + tx as isize, y + ty as isize, Some((entry.palette, c)));
                }
            }
        }
        Ok(())
    }

    /// RGBA8 pixels, row-major. A missing palette or color renders magenta so decoding
    /// errors stand out.
    pub fn to_rgba(&self, palettes: &[Palette]) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for p in &self.pixels {
            let c = match p {
                None => [0, 0, 0, 0],
                Some((pal, idx)) => palettes
                    .get(usize::from(*pal))
                    .and_then(|pl| pl.get(usize::from(*idx)))
                    .copied()
                    .unwrap_or([0xFF, 0, 0xFF, 0xFF]),
            };
            out.extend_from_slice(&c);
        }
        out
    }
}

/// Lays out every tile of `tiles` row-major, `width_tiles` per row, with palette 0.
pub fn tile_sheet(tiles: &[u8], depth: Depth, width_tiles: usize) -> Result<IndexedImage> {
    if width_tiles == 0 {
        return Err(Error::invalid("tile sheet", "zero width"));
    }
    let count = tiles.len() / depth.tile_bytes();
    let mut img = IndexedImage::new(width_tiles * 8, count.div_ceil(width_tiles) * 8);
    for t in 0..count {
        let e = MapEntry::from_raw(0);
        let (x, y) = (
            (t % width_tiles * 8) as isize,
            (t / width_tiles * 8) as isize,
        );
        // A one-tile window, so tile numbers above 0x3FF still work.
        img.draw_tile(&tiles[t * depth.tile_bytes()..], depth, e, x, y, false)?;
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgbx_palettes() {
        assert_eq!(rgbx_palette(&[1, 2, 3, 0x80], 1).unwrap(), [[1, 2, 3, 255]]);
        assert!(rgbx_palette(&[1, 2, 3], 1).is_err());
        assert_eq!(rgbx_palettes16(&[0; 130]).unwrap().len(), 2);
    }

    #[test]
    fn draws_flipped_tiles() {
        let mut t = [0u8; 32];
        t[0] = 0x21; // pixel 0 = 1, pixel 1 = 2
        let mut img = IndexedImage::new(8, 8);
        img.draw_tile(&t, Depth::Bpp4, MapEntry::from_raw(0x2400), 0, 0, true)
            .unwrap();
        assert_eq!(img.pixels[7], Some((2, 1)));
        assert_eq!(img.pixels[6], Some((2, 2)));
        assert_eq!(img.pixels[0], None);
        let rgba = img.to_rgba(&[vec![], vec![], vec![[0; 4], [9, 9, 9, 255]]]);
        assert_eq!(&rgba[28..32], &[9, 9, 9, 255]);
        assert_eq!(&rgba[24..28], &[255, 0, 255, 255]);
        assert_eq!(tile_sheet(&[0; 96], Depth::Bpp4, 2).unwrap().height, 16);
    }
}
