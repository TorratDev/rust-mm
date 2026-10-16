//! Dungeon tilesets (`dungeon.sbin` `bNN*`). Spec: `docs/formats/tilesets.md`.
//!
//! A tileset `NN` has up to five parts:
//!
//! | Part      | Holds |
//! |-----------|-------|
//! | `bNNfon`  | AT4PX: 4bpp 8x8 tiles |
//! | `bNNcel`  | AT4PX: 24x24 metatiles, 9 tilemap entries (3x3) each |
//! | `bNNpal`  | 12 palettes of 16 RGBX colors |
//! | `bNNcex`  | SIR0 + AT4PX: autotile table, metatile per (terrain, neighbour mask, variant) |
//! | `bNNcanm` | SIR0: palette animation, one color cycle per slot |
//!
//! Some numbers (22, 26, 27, ...) only have `pal` and `canm`; 64-75 have no `cex`.

use crate::archive::Archive;
use crate::gfx::MapEntry;
use crate::graphics::{self, Depth, IndexedImage, Palette};
use crate::px;
use crate::read::{self as bytes, Error, Result};
use crate::sir0::Sir0;

/// Metatile size in pixels.
pub const METATILE: usize = 24;
/// Autotile terrains: 0 wall, 1 secondary (water/lava), 2 floor (`[hypothesis]` from
/// the pictures: the single-bit masks extend each terrain's blob towards one side).
pub const TERRAINS: usize = 3;
/// Variants per (terrain, mask).
pub const VARIANTS: usize = 3;

/// One palette color cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorCycle {
    /// Game frames each color is shown.
    pub duration: u16,
    /// Colors in order.
    pub colors: Palette,
}

/// A decoded tileset. `chr` is empty for palette-only tilesets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tileset {
    /// 4bpp tile data.
    pub chr: Vec<u8>,
    /// Metatiles, 3x3 entries row-major.
    pub metatiles: Vec<[MapEntry; 9]>,
    /// Palettes.
    pub palettes: Vec<Palette>,
    /// Autotile table: `[terrain][mask][variant]` metatile numbers, flattened.
    pub autotile: Option<Vec<u8>>,
    /// Palette animation slots (`None` = slot not animated).
    pub palette_anim: Vec<Option<ColorCycle>>,
}

/// Tileset numbers present in `dungeon.sbin` (those with a `bNNpal`).
pub fn indices(arc: &Archive) -> Vec<u8> {
    arc.entries()
        .iter()
        .filter_map(|e| e.name.strip_prefix('b')?.strip_suffix("pal")?.parse().ok())
        .collect()
}

impl Tileset {
    /// Loads tileset `index` from the `dungeon.sbin` archive `data`.
    pub fn load(arc: &Archive, data: &[u8], index: u8) -> Result<Self> {
        let part = |suffix: &str| {
            let name = format!("b{index:02}{suffix}");
            arc.find(&name).map(|e| arc.data(data, e)).transpose()
        };
        let pal =
            part("pal")?.ok_or_else(|| Error::invalid("tileset", format!("no tileset {index}")))?;
        let mut t = Tileset {
            palettes: graphics::rgbx_palettes16(pal)?,
            ..Self::default()
        };
        if let Some(fon) = part("fon")? {
            t.chr = px::decompress(fon, px::DEFAULT_LIMIT)?;
        }
        if let Some(cel) = part("cel")? {
            t.metatiles = parse_metatiles(&px::decompress(cel, px::DEFAULT_LIMIT)?);
        }
        if let Some(cex) = part("cex")? {
            t.autotile = Some(parse_autotile(cex)?);
        }
        if let Some(canm) = part("canm")? {
            t.palette_anim = parse_palette_anim(canm)?;
        }
        Ok(t)
    }

    /// Renders metatile `m` (24x24).
    pub fn render_metatile(&self, m: usize) -> Result<IndexedImage> {
        let entries = self
            .metatiles
            .get(m)
            .ok_or_else(|| Error::invalid("tileset", format!("no metatile {m}")))?;
        let mut img = IndexedImage::new(METATILE, METATILE);
        for (k, e) in entries.iter().enumerate() {
            // A few tilesets (4, 64, ...) have one entry past their tile data, in a
            // metatile the autotile table never names; draw it blank.
            if (usize::from(e.tile) + 1) * 32 > self.chr.len() {
                continue;
            }
            img.draw_tile(
                &self.chr,
                Depth::Bpp4,
                *e,
                (k % 3 * 8) as isize,
                (k / 3 * 8) as isize,
                false,
            )?;
        }
        Ok(img)
    }

    /// Metatile for `terrain`, neighbour `mask` (bit per neighbour of the same terrain;
    /// `[hypothesis]` order S, SE, E, NE, N, NW, W, SW) and `variant`.
    pub fn autotile(&self, terrain: usize, mask: u8, variant: usize) -> Option<u8> {
        if terrain >= TERRAINS || variant >= VARIANTS {
            return None;
        }
        let table = self.autotile.as_ref()?;
        table
            .get(terrain * 256 * VARIANTS + usize::from(mask) * VARIANTS + variant)
            .copied()
    }
}

/// Splits decoded `cel` data into 3x3 metatiles.
pub fn parse_metatiles(cel: &[u8]) -> Vec<[MapEntry; 9]> {
    cel.chunks_exact(18)
        .map(|c| {
            std::array::from_fn(|k| {
                MapEntry::from_raw(u16::from_le_bytes([c[k * 2], c[k * 2 + 1]]))
            })
        })
        .collect()
}

/// Decodes the autotile table (`cex`): SIR0 whose content is one AT4PX.
pub fn parse_autotile(cex: &[u8]) -> Result<Vec<u8>> {
    let s = Sir0::parse(cex)?;
    let table = px::decompress(s.tail(s.data_offset, "autotile table")?, px::DEFAULT_LIMIT)?;
    if table.len() < TERRAINS * 256 * VARIANTS {
        return Err(Error::invalid(
            "autotile table",
            format!("{} bytes, need 2304", table.len()),
        ));
    }
    Ok(table)
}

/// Decodes palette animation (`canm`): a pointer table, each record
/// `{u16 color count, u16 duration, RGBX colors}`.
pub fn parse_palette_anim(canm: &[u8]) -> Result<Vec<Option<ColorCycle>>> {
    let s = Sir0::parse(canm)?;
    let end = s.pointer_list_offset.min(canm.len());
    let mut out = Vec::new();
    let mut at = s.data_offset;
    while at + 4 <= end {
        if !s.pointers.contains(&at) {
            let v = bytes::u32_at(canm, at, "palette animation table")?;
            if v != 0 {
                break; // padding after the table
            }
            out.push(None);
        } else {
            let rec = s.ptr(at, "palette animation table")?;
            let count = usize::from(bytes::u16_at(canm, rec, "palette animation")?);
            let duration = bytes::u16_at(canm, rec + 2, "palette animation")?;
            let colors = graphics::rgbx_palette(s.tail(rec + 4, "palette animation")?, count)?;
            out.push(Some(ColorCycle { duration, colors }));
        }
        at += 4;
    }
    while out.last() == Some(&None) {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px::tests::literal_at4px;
    use crate::sir0::tests::wrap;

    #[test]
    fn metatiles_and_autotile() {
        let mut cel = Vec::new();
        for k in 0..9u16 {
            cel.extend((k | 0x1000).to_le_bytes());
        }
        let m = parse_metatiles(&cel);
        assert_eq!((m.len(), m[0][8].tile, m[0][8].palette), (1, 8, 1));
        let table: Vec<u8> = (0..2352).map(|i| (i % 7) as u8).collect();
        let cex = wrap(&literal_at4px(&table), 0, &[]);
        let t = Tileset {
            autotile: Some(parse_autotile(&cex).unwrap()),
            ..Tileset::default()
        };
        assert_eq!(t.autotile(1, 2, 1), Some(((768 + 6 + 1) % 7) as u8));
        assert_eq!(t.autotile(3, 0, 0), None);
        assert!(parse_autotile(&wrap(&literal_at4px(&[0; 10]), 0, &[])).is_err());
    }

    #[test]
    fn palette_animation() {
        let mut body = vec![];
        body.extend(2u16.to_le_bytes());
        body.extend(5u16.to_le_bytes());
        body.extend([1, 2, 3, 0x80, 4, 5, 6, 0x80]);
        let hdr = body.len();
        body.extend(0x10u32.to_le_bytes());
        body.extend(0u32.to_le_bytes());
        let a = parse_palette_anim(&wrap(&body, hdr, &[hdr])).unwrap();
        assert_eq!(a.len(), 1);
        let c = a[0].as_ref().unwrap();
        assert_eq!((c.duration, c.colors[1]), (5, [4, 5, 6, 255]));
    }
}
