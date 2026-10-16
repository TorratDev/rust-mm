//! Ground (town / story scene) maps in `ground.sbin`: for a map `X`, `X` is the palette
//! file (BPL), `Xc` the tiles and chunks (BPC), `Xm` the chunk map (BMA). The names
//! and layouts follow Red's `ground_bg.c`; Blue keeps BPL and BMA as Red has them.
//! Spec: `docs/formats/maps.md`.
//!
//! Open: Blue's BPC tile/chunk data is compressed (Red stores it raw) with a scheme not
//! yet identified, so [`Bpc`] exposes the header and the still-compressed bytes. Chunk
//! maps therefore decode, but maps do not render yet.

use crate::graphics::{self, Palette};
use crate::read::{self as bytes, Error, Result};

/// Colors stored per BPL palette (index 0 is always transparent and not stored).
pub const BPL_COLORS: usize = 15;

/// One palette animation (BPL).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteAnim {
    /// Game frames per palette frame.
    pub duration: i16,
    /// Frames: each a 15-color palette.
    pub frames: Vec<Palette>,
}

/// BPL: the map's palettes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bpl {
    /// 16-color palettes (color 0 transparent).
    pub palettes: Vec<Palette>,
    /// Per palette, its animation if it has one.
    pub animations: Vec<Option<PaletteAnim>>,
}

/// Parses a BPL file: `u16 palette count`, `u16 has animations`, 15 RGBX colors per
/// palette, then (with animations) `{s16 duration, s16 frames}` per palette followed by
/// the frames' colors.
pub fn parse_bpl(data: &[u8]) -> Result<Bpl> {
    let count = usize::from(bytes::u16_at(data, 0, "BPL header")?);
    let animated = bytes::u16_at(data, 2, "BPL header")? != 0;
    if count > 16 {
        return Err(Error::invalid("BPL", format!("{count} palettes")));
    }
    let with_clear = |p: Palette| {
        let mut v = vec![[0, 0, 0, 0]];
        v.extend(p);
        v
    };
    let mut at = 4;
    let mut palettes = Vec::with_capacity(count);
    for _ in 0..count {
        palettes.push(with_clear(graphics::rgbx_palette(
            bytes::slice(data, at, BPL_COLORS * 4, "BPL palette")?,
            BPL_COLORS,
        )?));
        at += BPL_COLORS * 4;
    }
    let mut animations = vec![None; count];
    if animated {
        let specs = at;
        let mut colors = specs + count * 4;
        for (i, a) in animations.iter_mut().enumerate() {
            let duration = bytes::i16_at(data, specs + i * 4, "BPL animation")?;
            let n = bytes::i16_at(data, specs + i * 4 + 2, "BPL animation")?;
            if n <= 0 {
                continue;
            }
            let mut frames = Vec::new();
            for _ in 0..n {
                frames.push(with_clear(graphics::rgbx_palette(
                    bytes::slice(data, colors, BPL_COLORS * 4, "BPL frame")?,
                    BPL_COLORS,
                )?));
                colors += BPL_COLORS * 4;
            }
            *a = Some(PaletteAnim { duration, frames });
        }
    }
    Ok(Bpl {
        palettes,
        animations,
    })
}

/// BPC header (16 bytes) plus its still-compressed payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bpc<'a> {
    /// Chunk width in tiles (2 or 3).
    pub chunk_width: u16,
    /// Chunk height in tiles.
    pub chunk_height: u16,
    /// Tiles + 1 (tile 0 is the implicit empty tile).
    pub tiles: u16,
    /// Tiles in each of the 4 animated-tile (BPA) slots.
    pub bpa_tiles: [u16; 4],
    /// Chunks + 1 (chunk 0 is empty).
    pub chunks: u16,
    /// Compressed tiles and chunk tilemaps.
    pub payload: &'a [u8],
}

/// Parses the BPC header.
pub fn parse_bpc(data: &[u8]) -> Result<Bpc<'_>> {
    let h = |i: usize| bytes::u16_at(data, i * 2, "BPC header");
    let b = Bpc {
        chunk_width: h(0)?,
        chunk_height: h(1)?,
        tiles: h(2)?,
        bpa_tiles: [h(3)?, h(4)?, h(5)?, h(6)?],
        chunks: h(7)?,
        payload: &data[16..],
    };
    if !matches!((b.chunk_width, b.chunk_height), (2, 2) | (3, 3)) {
        return Err(Error::invalid(
            "BPC",
            format!("chunk size {}x{}", b.chunk_width, b.chunk_height),
        ));
    }
    Ok(b)
}

/// BMA: the chunk map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bma {
    /// Map width in tiles.
    pub width_tiles: u8,
    /// Map height in tiles.
    pub height_tiles: u8,
    /// Chunk width in tiles.
    pub tiling_width: u8,
    /// Chunk height in tiles.
    pub tiling_height: u8,
    /// Map width in chunks.
    pub width_chunks: u8,
    /// Map height in chunks.
    pub height_chunks: u8,
    /// Has a data layer (flag as stored).
    pub has_data_layer: bool,
    /// Has a collision layer (flag as stored).
    pub has_collision: bool,
    /// Layers: `height_chunks` rows of `width_chunks` chunk numbers.
    pub layers: Vec<Vec<Vec<u16>>>,
    /// Bytes after the layers (collision/data layers; format not decoded yet).
    pub extra: Vec<u8>,
}

/// Parses a BMA file. Layers use Red's "NRL" row coding: per row, commands until the row
/// is full; `0xC0+n` reads n+1 literal pairs, `0x80+n` repeats one pair n+1 times,
/// `n < 0x80` emits n+1 zero (or, after row 0, unchanged) pairs. A pair is 3 bytes = two
/// 12-bit chunk numbers. Rows after the first are XORed with the row above.
pub fn parse_bma(data: &[u8]) -> Result<Bma> {
    let b = |i| bytes::u8_at(data, i, "BMA header");
    let (wc, hc) = (usize::from(b(4)?), usize::from(b(5)?));
    let layer_count = usize::from(bytes::u16_at(data, 6, "BMA header")?);
    if layer_count > 4 {
        return Err(Error::invalid("BMA", format!("{layer_count} layers")));
    }
    let mut at = 12;
    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        let mut rows: Vec<Vec<u16>> = Vec::with_capacity(hc);
        for _ in 0..hc {
            let mut row: Vec<u16> = Vec::with_capacity(wc + 1);
            let above = rows.last();
            let prev = |row: &Vec<u16>, k: usize| {
                above
                    .and_then(|r| r.get(row.len() + k))
                    .copied()
                    .unwrap_or(0)
            };
            while row.len() < wc {
                let cmd = bytes::u8_at(data, at, "BMA layer")?;
                at += 1;
                let pair = |at: &mut usize| -> Result<[u16; 2]> {
                    let v = bytes::array::<3>(data, *at, "BMA layer")?;
                    *at += 3;
                    let v = u32::from(v[0]) | u32::from(v[1]) << 8 | u32::from(v[2]) << 16;
                    Ok([(v & 0xFFF) as u16, (v >> 12 & 0xFFF) as u16])
                };
                if cmd >= 0x80 {
                    let repeated = (cmd < 0xC0).then(|| pair(&mut at)).transpose()?;
                    for _ in 0..usize::from(cmd & 0x3F) + 1 {
                        let p = match repeated {
                            Some(p) => p,
                            None => pair(&mut at)?,
                        };
                        let (a, b) = (prev(&row, 0), prev(&row, 1));
                        row.extend([p[0] ^ a, p[1] ^ b]);
                    }
                } else {
                    for _ in 0..usize::from(cmd) + 1 {
                        let (a, b) = (prev(&row, 0), prev(&row, 1));
                        row.extend([a, b]);
                    }
                }
            }
            rows.push(row);
        }
        for r in &mut rows {
            r.truncate(wc);
        }
        layers.push(rows);
    }
    Ok(Bma {
        width_tiles: b(0)?,
        height_tiles: b(1)?,
        tiling_width: b(2)?,
        tiling_height: b(3)?,
        width_chunks: b(4)?,
        height_chunks: b(5)?,
        has_data_layer: bytes::u16_at(data, 8, "BMA header")? != 0,
        has_collision: bytes::u16_at(data, 10, "BMA header")? != 0,
        layers,
        extra: data.get(at..).unwrap_or_default().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bpl_with_animation() {
        let mut d = vec![1, 0, 1, 0];
        d.extend([7u8; 60]);
        d.extend(4i16.to_le_bytes());
        d.extend(1i16.to_le_bytes());
        d.extend([9u8; 60]);
        let b = parse_bpl(&d).unwrap();
        assert_eq!(
            (b.palettes[0].len(), b.palettes[0][1]),
            (16, [7, 7, 7, 255])
        );
        let a = b.animations[0].as_ref().unwrap();
        assert_eq!((a.duration, a.frames[0][15]), (4, [9, 9, 9, 255]));
        assert!(parse_bpl(&[1, 0, 0, 0]).is_err());
    }

    #[test]
    fn bma_rows_and_xor() {
        // 2x2 chunks, 1 layer. Row 0: 0x80 repeats pair (1, 2). Row 1: 0x80 with pair
        // (1, 1) XORs the row above -> (0, 3).
        let mut d = vec![6, 6, 3, 3, 2, 2, 1, 0, 0, 0, 0, 0];
        d.extend([0x80, 0x01, 0x20, 0x00]);
        d.extend([0x80, 0x01, 0x10, 0x00]);
        let m = parse_bma(&d).unwrap();
        assert_eq!(m.layers[0], [vec![1, 2], vec![0, 3]]);
        assert!(m.extra.is_empty());
        assert!(parse_bma(&d[..14]).is_err());
    }

    #[test]
    fn bpc_header() {
        let mut d: Vec<u8> = [3u16, 3, 10, 0, 0, 0, 0, 5]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        d.push(0xAB);
        let b = parse_bpc(&d).unwrap();
        assert_eq!((b.tiles, b.chunks, b.payload), (10, 5, &[0xAB][..]));
        d[0] = 4;
        assert!(parse_bpc(&d).is_err());
    }
}
