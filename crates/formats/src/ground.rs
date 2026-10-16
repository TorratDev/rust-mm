//! Ground (town / story scene) maps in `ground.sbin`: for a map `X`, `X` is the palette
//! file (BPL), `Xc` the tiles and chunks (BPC), `Xm` the chunk map (BMA). The names
//! and layouts follow Red's `ground_bg.c`; Blue keeps BPL and BMA as Red has them.
//! Spec: `docs/formats/maps.md`.
//!
//! Blue stores BPC tiles and chunks compressed (Red stores them raw); the decoders
//! follow overlay 0's loader (`docs/re/ground-bpc.md`).

use crate::gfx::MapEntry;
use crate::graphics::{self, Depth, IndexedImage, Palette};
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

/// BPC: the map's tiles and chunks, decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bpc {
    /// Chunk width in tiles (2 or 3).
    pub chunk_width: u16,
    /// Chunk height in tiles.
    pub chunk_height: u16,
    /// Tiles + 1 (tile 0 is the implicit empty tile).
    pub tiles: u16,
    /// Tiles in each of the 4 animated-tile (BPA) slots. They follow the BPC tiles in
    /// VRAM, so chunk entries at or past `tiles` name them.
    pub bpa_tiles: [u16; 4],
    /// Chunks + 1 (chunk 0 is empty).
    pub chunks: u16,
    /// 4bpp tiles, 32 bytes each, `tiles` of them (tile 0 blank).
    pub chr: Vec<u8>,
    /// Tilemap entries, `chunk_width * chunk_height` per chunk row-major, `chunks` of
    /// them (chunk 0 all zero).
    pub chunk_map: Vec<u16>,
}

impl Bpc {
    /// Entries of chunk `c`.
    pub fn chunk(&self, c: u16) -> Option<&[u16]> {
        let n = usize::from(self.chunk_width * self.chunk_height);
        self.chunk_map
            .get(usize::from(c) * n..(usize::from(c) + 1) * n)
    }
}

/// Parses a BPC file: the 16-byte header (8 `u16`), the tiles in the coding of
/// [`decode_bpc_tiles`], padding to an even offset, then the chunk entries in the
/// coding of [`decode_bpc_chunks`].
pub fn parse_bpc(data: &[u8]) -> Result<Bpc> {
    let h = |i: usize| bytes::u16_at(data, i * 2, "BPC header");
    let (chunk_width, chunk_height) = (h(0)?, h(1)?);
    if !matches!((chunk_width, chunk_height), (2, 2) | (3, 3)) {
        return Err(Error::invalid(
            "BPC",
            format!("chunk size {chunk_width}x{chunk_height}"),
        ));
    }
    let (tiles, chunks) = (h(2)?, h(7)?);
    let payload = &data[16..];
    let (mut chr, used) = decode_bpc_tiles(payload, usize::from(tiles.saturating_sub(1)) * 32)?;
    chr.splice(0..0, [0; 32]);
    let per = usize::from(chunk_width * chunk_height);
    let mut chunk_map = vec![0; per];
    chunk_map.extend(decode_bpc_chunks(
        payload.get(used + (used & 1)..).unwrap_or_default(),
        per * usize::from(chunks.saturating_sub(1)),
    )?);
    Ok(Bpc {
        chunk_width,
        chunk_height,
        tiles,
        bpa_tiles: [h(3)?, h(4)?, h(5)?, h(6)?],
        chunks,
        chr,
        chunk_map,
    })
}

/// Decodes `len` bytes of BPC tile data (overlay 0 `0x02149fc8`); returns them and the
/// input bytes used. Each command emits n+1 bytes:
///
/// - `0x00-0x7D` literal, n = cmd; `0x7E` n = next byte; `0x7F` n = next `u16`.
/// - `0x80-0xBE` n = cmd - 0x80 (`0xBF`: next byte), then a new fill byte.
/// - `0xC0-0xDE` n = cmd - 0xC0 (`0xDF`: next byte): the last fill byte again.
/// - `0xE0-0xFE` n = cmd - 0xE0 (`0xFF`: next byte): the fill byte before the last
///   (the two swap).
pub fn decode_bpc_tiles(src: &[u8], len: usize) -> Result<(Vec<u8>, usize)> {
    let mut out = Vec::with_capacity(len + 0x100);
    let (mut at, mut last, mut before) = (0, 0u8, 0u8);
    let byte = |at: &mut usize| {
        *at += 1;
        bytes::u8_at(src, *at - 1, "BPC tiles")
    };
    while out.len() < len {
        let cmd = byte(&mut at)?;
        // Low bits of the count field; all ones means the count follows in a byte.
        let field = match cmd {
            0x00..=0x7F => 0x7F,
            0x80..=0xBF => 0x3F,
            _ => 0x1F,
        };
        let n = 1 + match cmd {
            0x7F => {
                at += 2;
                usize::from(bytes::u16_at(src, at - 2, "BPC tiles")?)
            }
            0x7E => usize::from(byte(&mut at)?),
            _ if cmd & field == field => usize::from(byte(&mut at)?),
            _ => usize::from(cmd & field),
        };
        let fill = match cmd {
            0x00..=0x7F => {
                out.extend_from_slice(bytes::slice(src, at, n, "BPC tiles")?);
                at += n;
                continue;
            }
            0x80..=0xBF => {
                before = last;
                byte(&mut at)?
            }
            0xC0..=0xDF => last,
            _ => std::mem::replace(&mut before, last),
        };
        last = fill;
        out.resize(out.len() + n, fill);
    }
    out.truncate(len);
    Ok((out, at))
}

/// Decodes `count` BPC chunk entries (overlay 0 `0x02149d30`): two passes over the
/// entries, first their high bytes, then their low bytes. Per pass, commands:
/// `n < 0x80` skips n+1 entries (they keep 0 or their high byte); `0x80 + n` one byte
/// for n+1 entries; `0xC0 + n` n+1 literal bytes.
pub fn decode_bpc_chunks(src: &[u8], count: usize) -> Result<Vec<u16>> {
    let mut out = vec![0u16; count];
    let mut at = 0;
    for shift in [8, 0] {
        let mut k = 0;
        while k < count {
            let cmd = bytes::u8_at(src, at, "BPC chunks")?;
            at += 1;
            let n = usize::from(cmd & 0x3F) + 1;
            match cmd {
                0x00..=0x7F => {
                    k += usize::from(cmd) + 1;
                    continue;
                }
                0x80..=0xBF => {
                    let v = u16::from(bytes::u8_at(src, at, "BPC chunks")?) << shift;
                    at += 1;
                    out.iter_mut().skip(k).take(n).for_each(|e| *e |= v);
                }
                _ => {
                    let lit = bytes::slice(src, at, n, "BPC chunks")?;
                    at += n;
                    for (e, &b) in out.iter_mut().skip(k).zip(lit) {
                        *e |= u16::from(b) << shift;
                    }
                }
            }
            k += n;
        }
    }
    Ok(out)
}

/// BPA: one animated-tile slot, `X1`..`X4` for slots 0..3 of map `X`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bpa {
    /// Tiles per frame.
    pub tiles: u16,
    /// Per frame, its 4 info bytes (timing; layout not decoded).
    pub frame_info: Vec<[u8; 4]>,
    /// 4bpp tiles, frame after frame.
    pub chr: Vec<u8>,
}

impl Bpa {
    /// Tiles of frame `f`.
    pub fn frame(&self, f: usize) -> Option<&[u8]> {
        let n = usize::from(self.tiles) * 32;
        self.chr.get(f * n..(f + 1) * n)
    }
}

/// Parses a BPA file: `u16 tiles`, `u16 frames`, 4 bytes per frame, then raw 4bpp tiles
/// for every frame (overlay 0 `0x0214a8f4` copies frame 0 for slots 2 and 3).
pub fn parse_bpa(data: &[u8]) -> Result<Bpa> {
    let tiles = bytes::u16_at(data, 0, "BPA header")?;
    let frames = usize::from(bytes::u16_at(data, 2, "BPA header")?);
    let info = bytes::slice(data, 4, frames * 4, "BPA frames")?;
    let chr = bytes::slice(
        data,
        4 + frames * 4,
        frames * usize::from(tiles) * 32,
        "BPA tiles",
    )?;
    Ok(Bpa {
        tiles,
        frame_info: info
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect(),
        chr: chr.to_vec(),
    })
}

/// Renders BMA layer `layer` with the BPC's chunks and frame 0 of each BPA slot (`bpa`,
/// slot order; tiles of a missing slot are left transparent). Palette numbers stay as
/// stored: BPL palette indices.
pub fn render_layer(
    bpc: &Bpc,
    bpa: &[Option<Bpa>],
    bma: &Bma,
    layer: usize,
) -> Result<IndexedImage> {
    let rows = bma
        .layers
        .get(layer)
        .ok_or_else(|| Error::invalid("BMA", format!("no layer {layer}")))?;
    // VRAM order: BPC tiles, then each slot's tiles.
    let mut chr = bpc.chr.clone();
    let mut present = vec![true; usize::from(bpc.tiles)];
    for (i, &n) in bpc.bpa_tiles.iter().enumerate() {
        let n = usize::from(n);
        match bpa.get(i).and_then(Option::as_ref).and_then(|b| b.frame(0)) {
            Some(f) if f.len() == n * 32 => chr.extend_from_slice(f),
            _ => chr.resize(chr.len() + n * 32, 0),
        }
        present.resize(present.len() + n, bpa.get(i).is_some_and(Option::is_some));
    }
    let (cw, ch) = (usize::from(bpc.chunk_width), usize::from(bpc.chunk_height));
    let mut img = IndexedImage::new(
        usize::from(bma.width_chunks) * cw * 8,
        usize::from(bma.height_chunks) * ch * 8,
    );
    for (cy, row) in rows.iter().enumerate() {
        for (cx, &c) in row.iter().enumerate() {
            let entries = bpc
                .chunk(c)
                .ok_or_else(|| Error::invalid("BMA", format!("chunk {c} out of range")))?;
            for (k, &raw) in entries.iter().enumerate() {
                let e = MapEntry::from_raw(raw);
                if e.tile == 0 || !present.get(usize::from(e.tile)).copied().unwrap_or(false) {
                    continue;
                }
                let x = (cx * cw + k % cw) * 8;
                let y = (cy * ch + k / cw) * 8;
                img.draw_tile(&chr, Depth::Bpp4, e, x as isize, y as isize, false)?;
            }
        }
    }
    Ok(img)
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
    fn bpc_tile_commands() {
        let src = [
            0x01, 0xAA, 0xBB, // 2 literal bytes
            0x81, 0x11, // fill 2 x 0x11
            0x82, 0x22, // fill 3 x 0x22
            0xE0, // 1 x the fill before (0x11); the two swap
            0xC1, // 2 x the last fill (0x11)
            0x7E, 0x00, 0xCC, // literal, count in the next byte
            0xDF, 0x01, // last fill, count in the next byte
            0x7F, 0x01, 0x00, 0xDD, 0xEE, // literal, u16 count
        ];
        let want = [
            0xAA, 0xBB, 0x11, 0x11, 0x22, 0x22, 0x22, 0x11, 0x11, 0x11, 0xCC, 0x11, 0x11, 0xDD,
            0xEE,
        ];
        assert_eq!(
            decode_bpc_tiles(&src, 15).unwrap(),
            (want.to_vec(), src.len())
        );
        assert_eq!(decode_bpc_tiles(&src, 3).unwrap(), (want[..3].to_vec(), 5));
        assert!(decode_bpc_tiles(&src[..4], 15).is_err());
    }

    #[test]
    fn bpc_chunk_planes() {
        // High bytes: skip 1, 2 literal. Low bytes: fill 2 x 0x05, skip 1.
        let src = [0x00, 0xC1, 0xA0, 0xB0, 0x81, 0x05, 0x00];
        assert_eq!(
            decode_bpc_chunks(&src, 3).unwrap(),
            [0x0005, 0xA005, 0xB000]
        );
        assert!(decode_bpc_chunks(&src[..5], 3).is_err());
    }

    /// One BPC tile (all color 7), one BPA tile (all 3), one 3x3 chunk naming both.
    fn tiny_map() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut bpc: Vec<u8> = [3u16, 3, 2, 1, 0, 0, 0, 2]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        // Tiles: 17 + 15 bytes of 0x77 (3 bytes used, so 1 pad byte), then chunks.
        bpc.extend([0x90, 0x77, 0xCE, 0xFF]);
        bpc.extend([0x08, 0xC8, 1, 2, 0, 0, 0, 0, 0, 0, 0]);
        let mut bpa = vec![1, 0, 1, 0, 9, 0, 0, 0];
        bpa.extend([0x33; 32]);
        let bma = vec![3, 3, 3, 3, 1, 1, 1, 0, 0, 0, 0, 0, 0xC0, 0x01, 0x00, 0x00];
        (bpc, bpa, bma)
    }

    #[test]
    fn bpc_parse_and_render() {
        let (bpc, bpa, bma) = tiny_map();
        let b = parse_bpc(&bpc).unwrap();
        assert_eq!((b.tiles, b.chunks, b.bpa_tiles), (2, 2, [1, 0, 0, 0]));
        assert_eq!(
            (&b.chr[..32], &b.chr[32..]),
            (&[0; 32][..], &[0x77; 32][..])
        );
        assert_eq!(b.chunk(0), Some(&[0; 9][..]));
        assert_eq!(b.chunk(1), Some(&[1, 2, 0, 0, 0, 0, 0, 0, 0][..]));
        let a = parse_bpa(&bpa).unwrap();
        assert_eq!((a.tiles, a.frame_info.len()), (1, 1));
        assert!(parse_bpa(&bpa[..20]).is_err());

        let m = parse_bma(&bma).unwrap();
        let img = render_layer(&b, &[Some(a)], &m, 0).unwrap();
        assert_eq!((img.width, img.height), (24, 24));
        assert_eq!(img.pixels[0], Some((0, 7)));
        assert_eq!(img.pixels[8], Some((0, 3)));
        assert_eq!(img.pixels[16], None);
        // Without the BPA its tile stays blank.
        assert_eq!(render_layer(&b, &[], &m, 0).unwrap().pixels[8], None);
        assert!(render_layer(&b, &[], &m, 1).is_err());
    }

    #[test]
    fn bpc_bad_chunk_size() {
        let (mut bpc, _, _) = tiny_map();
        bpc[0] = 4;
        assert!(parse_bpc(&bpc).is_err());
        assert!(parse_bpc(&bpc[..10]).is_err());
    }
}
