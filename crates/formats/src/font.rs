//! Fonts in `system.sbin`. Spec: `docs/formats/text.md#fonts`.
//!
//! * `kanji_a` / `kanji_b`: the variable-width dialogue fonts. SIR0 content header
//!   `{u32 glyph count, u32 table pointer}`; table entries are 12 bytes
//!   `{u32 bitmap pointer, u16 char code, u16 advance width, u16 0, u16 flags}`, sorted
//!   by code. A bitmap is 12x12 pixels, 4bpp, linear rows of 6 bytes (low nibble left).
//! * `font`, `fontF`, ...: fixed 8x8 fonts, `u32` count then 4bpp tiles.

use crate::graphics::{self, Depth, IndexedImage};
use crate::read::{self as bytes, Error, Result};
use crate::sir0::Sir0;

/// Glyph cell size of the variable-width fonts.
pub const GLYPH: usize = 12;
const GLYPH_BYTES: usize = GLYPH * GLYPH / 2;

/// One glyph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// Character code: one byte for Latin-1, two for game glyphs (`0x81xx`...).
    pub code: u16,
    /// Advance width in pixels.
    pub width: u16,
    /// Flags word (0xFF02 / 0xFF03 seen; meaning unknown).
    pub flags: u16,
    /// 12x12 color indices, row-major.
    pub pixels: Vec<u8>,
}

/// Decodes a variable-width font.
pub fn parse_font(data: &[u8]) -> Result<Vec<Glyph>> {
    let s = Sir0::parse(data)?;
    let count = s.u32(s.data_offset, "font header")? as usize;
    let table = s.ptr(s.data_offset + 4, "font header")?;
    if count > 0x1_0000 {
        return Err(Error::invalid("font", format!("{count} glyphs")));
    }
    (0..count)
        .map(|i| {
            let e = table + i * 12;
            let bmp = s.ptr(e, "font glyph")?;
            let raw = bytes::slice(data, bmp, GLYPH_BYTES, "font bitmap")?;
            Ok(Glyph {
                code: bytes::u16_at(data, e + 4, "font glyph")?,
                width: bytes::u16_at(data, e + 6, "font glyph")?,
                flags: bytes::u16_at(data, e + 10, "font glyph")?,
                pixels: raw.iter().flat_map(|b| [b & 0xF, b >> 4]).collect(),
            })
        })
        .collect()
}

/// Decodes a fixed 8x8 font into one image per glyph (palette 0).
pub fn parse_fixed_font(data: &[u8]) -> Result<Vec<IndexedImage>> {
    let count = bytes::offset_at(data, 0, "fixed font")?;
    let tiles = bytes::slice(data, 4, count.saturating_mul(32), "fixed font")?;
    (0..count)
        .map(|i| graphics::tile_sheet(&tiles[i * 32..i * 32 + 32], Depth::Bpp4, 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sir0::tests::wrap;

    #[test]
    fn variable_font() {
        let mut body = vec![0x21u8; GLYPH_BYTES];
        let table = body.len();
        body.extend(0x10u32.to_le_bytes());
        body.extend(0x41u16.to_le_bytes());
        body.extend(6u16.to_le_bytes());
        body.extend(0u16.to_le_bytes());
        body.extend(0xFF02u16.to_le_bytes());
        let hdr = body.len();
        body.extend(1u32.to_le_bytes());
        body.extend((0x10 + table as u32).to_le_bytes());
        let g = parse_font(&wrap(&body, hdr, &[table, hdr + 4])).unwrap();
        assert_eq!(
            (g[0].code, g[0].width, &g[0].pixels[..2]),
            (0x41, 6, &[1u8, 2][..])
        );
    }

    #[test]
    fn fixed_font() {
        let mut d = 2u32.to_le_bytes().to_vec();
        d.extend([0x11; 64]);
        assert_eq!(parse_fixed_font(&d).unwrap().len(), 2);
        assert!(parse_fixed_font(&[9, 0, 0, 0]).is_err());
    }
}
