//! Turns a selected resource into something to show: an RGBA image, text lines, or raw
//! bytes for the hex view. One function per format; all of them are GUI-free so they
//! can be tested and reused by exports.

use rescue_data::{item, moves, species};
use rescue_formats::graphics::{self, Depth, IndexedImage, Palette};
use rescue_formats::{effect, font, ground, portrait, px, sprite, text, tileset};

use crate::game::Game;

/// An RGBA8 image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    /// Width.
    pub width: usize,
    /// Height.
    pub height: usize,
    /// Pixels, 4 bytes each.
    pub pixels: Vec<u8>,
}

/// What to show for a resource.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// Format label.
    pub kind: String,
    /// Image, if the format has one.
    pub image: Option<Rgba>,
    /// Text lines (strings, table rows, notes).
    pub lines: Vec<String>,
    /// Bytes for the hex view (raw or decoded).
    pub bytes: Vec<u8>,
}

/// Viewer choices that change what a sprite or tileset shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Sprite animation group.
    pub group: usize,
    /// Sprite direction (0..8).
    pub direction: usize,
    /// Sprite animation time in game frames.
    pub tick: u32,
    /// Palette number for raw tile sheets.
    pub palette: usize,
}

/// Grayscale ramp used for tiles with no known palette.
pub fn gray16() -> Palette {
    (0..16u8).map(|i| [i * 17, i * 17, i * 17, 255]).collect()
}

fn rgba(img: &IndexedImage, palettes: &[Palette]) -> Rgba {
    Rgba {
        width: img.width,
        height: img.height,
        pixels: img.to_rgba(palettes),
    }
}

/// Lays images out in a grid, `per_row` per row, with a 2-pixel gap.
pub fn grid(images: &[Rgba], per_row: usize) -> Rgba {
    let per_row = per_row.max(1);
    let cw = images.iter().map(|i| i.width).max().unwrap_or(0) + 2;
    let ch = images.iter().map(|i| i.height).max().unwrap_or(0) + 2;
    let rows = images.len().div_ceil(per_row);
    let (w, h) = (cw * per_row.min(images.len()).max(1), ch * rows.max(1));
    let mut out = Rgba {
        width: w,
        height: h,
        pixels: vec![0; w * h * 4],
    };
    for (k, im) in images.iter().enumerate() {
        let (x0, y0) = (k % per_row * cw, k / per_row * ch);
        for y in 0..im.height {
            let src = &im.pixels[y * im.width * 4..(y + 1) * im.width * 4];
            let at = ((y0 + y) * w + x0) * 4;
            out.pixels[at..at + src.len()].copy_from_slice(src);
        }
    }
    out
}

/// Color swatches, 16 per row, 8x8 each.
pub fn swatches(palettes: &[Palette]) -> Rgba {
    let cells: Vec<Rgba> = palettes
        .iter()
        .flatten()
        .map(|c| Rgba {
            width: 8,
            height: 8,
            pixels: c.repeat(64),
        })
        .collect();
    grid(&cells, 16)
}

/// The view for sub-file `name` of pack `pack`, or for a whole ROM file when `name` is
/// `None`.
pub fn view(g: &Game, pack: &str, name: Option<&str>, opt: Options) -> View {
    let data = match name {
        Some(n) => g.sub(pack, n),
        None => g.file(pack),
    }
    .unwrap_or_default();
    let mut v = View {
        bytes: data.to_vec(),
        ..View::default()
    };
    let r = match name {
        None => whole_file(g, pack, data, &mut v),
        Some(n) => sub_file(g, pack, n, data, opt, &mut v),
    };
    if let Err(e) = r {
        v.lines.push(format!("decode error: {e}"));
    }
    if v.kind.is_empty() {
        v.kind = "raw".into();
    }
    v
}

type R = Result<(), rescue_formats::error::FormatError>;

fn whole_file(g: &Game, path: &str, data: &[u8], v: &mut View) -> R {
    if path.ends_with(".str") {
        v.kind = "string table".into();
        for (i, s) in text::parse_str(data)?.iter().enumerate() {
            v.lines.push(format!(
                "{i:4}  {}",
                text::to_plain(&text::decode(s)).replace('\n', " / ")
            ));
        }
    } else if g.pack(path).is_some() {
        v.kind = "archive".into();
        v.lines.push("select a sub-file in the tree".into());
    }
    Ok(())
}

/// Plays `s` at the animation group and direction in `opt`: the frame that `opt.tick`
/// lands on, with its timing, plus counts.
fn sprite_view(s: &sprite::Sprite, palettes: &[Palette], opt: Options, v: &mut View) -> R {
    let group = opt.group.min(s.groups.len().saturating_sub(1));
    let seq = s
        .groups
        .get(group)
        .map(|d| &d[opt.direction % sprite::DIRECTIONS]);
    let total: u32 = seq
        .map(|q| q.iter().map(|st| u32::from(st.duration)).sum())
        .unwrap_or(0);
    let mut t = if total > 0 { opt.tick % total } else { 0 };
    let step = seq.and_then(|q| {
        q.iter().find(|st| {
            let hit = t < u32::from(st.duration);
            t = t.saturating_sub(u32::from(st.duration));
            hit
        })
    });
    if let Some(st) = step {
        let f = s.render_frame(usize::from(st.frame))?;
        v.image = Some(rgba(&f.image, palettes));
        v.lines.push(format!(
            "frame {} for {} ticks, offset {:?}, shadow {:?}",
            st.frame, st.duration, st.offset, st.shadow
        ));
    }
    v.lines.push(format!(
        "{} groups, {} frames, {} images",
        s.groups.len(),
        s.frames.len(),
        s.images.len()
    ));
    Ok(())
}

fn sub_file(g: &Game, pack: &str, name: &str, data: &[u8], opt: Options, v: &mut View) -> R {
    match (pack, name) {
        ("monster.sbin", n) if portrait::species_of(n).is_some() => {
            v.kind = format!(
                "portraits: {}",
                g.species_name(portrait::species_of(n).unwrap_or(0))
            );
            let faces = portrait::parse(data)?;
            v.image = Some(grid(
                &faces
                    .iter()
                    .map(|p| rgba(&p.image, std::slice::from_ref(&p.palette)))
                    .collect::<Vec<_>>(),
                7,
            ));
            v.lines = faces
                .iter()
                .map(|p| {
                    format!(
                        "slot {:2}: {}",
                        p.emotion,
                        portrait::EMOTION_NAMES[p.emotion]
                    )
                })
                .collect();
        }
        ("monster.sbin", n) if sprite::species_of(n).is_some() => {
            v.kind = format!(
                "sprite: {}",
                g.species_name(sprite::species_of(n).unwrap_or(0))
            );
            sprite_view(&sprite::Sprite::parse(data)?, &g.sprite_palettes, opt, v)?;
        }
        ("effect.sbin", n) if n.starts_with("efob") => {
            v.kind = format!("effect object {n}");
            let o = effect::parse_object(data)?;
            sprite_view(&o.sprite, std::slice::from_ref(&o.palette), opt, v)?;
        }
        ("effect.sbin", n) if n.starts_with("efbg") => {
            v.kind = format!("effect background {n}");
            let b = effect::parse_background(data)?;
            let banks: Vec<Palette> = b.palette.chunks(16).map(<[_]>::to_vec).collect();
            let sheet = graphics::tile_sheet(&b.tiles, Depth::Bpp4, 16)?;
            v.image = Some(rgba(&sheet, &banks));
            for (i, f) in b.frames.iter().enumerate() {
                v.lines.push(match f {
                    Some(f) => format!(
                        "frame {i}: header {:?}, {} map entries",
                        f.header,
                        f.map.len()
                    ),
                    None => format!("frame {i}: null"),
                });
            }
        }
        ("monster.sbin", "palet")
        | ("titlemenu.sbin", _)
        | ("ornament.sbin", _)
        | ("sample.sbin", _)
        | ("system.sbin", "fontpal" | "fontsppa")
            if is_rgbx(data) =>
        {
            v.kind = "palettes".into();
            v.image = Some(swatches(&graphics::rgbx_palettes16(data)?));
        }
        ("dungeon.sbin", n)
            if n.starts_with('b') && n.len() >= 5 && n[1..3].parse::<u8>().is_ok() =>
        {
            let id: u8 = n[1..3].parse().unwrap_or(0);
            let p = g.pack(pack).ok_or_else(|| {
                rescue_formats::error::FormatError::invalid("viewer", "no dungeon.sbin")
            })?;
            let t = tileset::Tileset::load(&p.archive, &p.data, id)?;
            v.kind = format!("tileset {id}");
            if t.metatiles.is_empty() {
                v.image = Some(swatches(&t.palettes));
                v.lines.push("palette-only tileset".into());
            } else {
                let tiles: Vec<Rgba> = (0..t.metatiles.len())
                    .filter_map(|m| t.render_metatile(m).ok())
                    .map(|i| rgba(&i, &t.palettes))
                    .collect();
                v.image = Some(grid(&tiles, 16));
            }
            v.lines.push(format!(
                "{} metatiles, {} tiles, autotile table: {}, {} animated palette slots",
                t.metatiles.len(),
                t.chr.len() / 32,
                t.autotile.is_some(),
                t.palette_anim.iter().flatten().count()
            ));
        }
        ("system.sbin", "kanji_a" | "kanji_b") => {
            v.kind = "font".into();
            let glyphs = font::parse_font(data)?;
            let imgs: Vec<Rgba> = glyphs
                .iter()
                .map(|gl| {
                    let px: Vec<u8> = gl
                        .pixels
                        .iter()
                        .flat_map(|&c| [c * 17, c * 17, c * 17, 255])
                        .collect();
                    Rgba {
                        width: font::GLYPH,
                        height: font::GLYPH,
                        pixels: px,
                    }
                })
                .collect();
            v.image = Some(grid(&imgs, 16));
            v.lines = glyphs
                .iter()
                .map(|gl| format!("{:#06x} width {}", gl.code, gl.width))
                .collect();
        }
        ("system.sbin", "font" | "fontF" | "fontG" | "fontI" | "fontS") => {
            v.kind = "8x8 font".into();
            let imgs: Vec<Rgba> = font::parse_fixed_font(data)?
                .iter()
                .map(|i| rgba(i, &[gray16()]))
                .collect();
            v.image = Some(grid(&imgs, 16));
        }
        ("system.sbin", "monspara") => {
            v.kind = "species".into();
            v.lines = species::parse_species(data)?
                .iter()
                .map(|s| {
                    format!(
                        "{:3} {:12} {:14} types {:?} HP {} atk {:?} def {:?} recruit {}",
                        s.id,
                        s.name,
                        s.category,
                        s.types,
                        s.base_hp,
                        s.base_attack,
                        s.base_defense,
                        s.recruit_rate
                    )
                })
                .collect();
        }
        ("system.sbin", n) if n.starts_with("wazapar") => {
            v.kind = "moves".into();
            let m = moves::parse_moves(data)?;
            v.lines = m
                .moves
                .iter()
                .map(|m| {
                    format!(
                        "{:3} {:16} power {:3} PP {:2} type {:2} acc {:?}",
                        m.id, m.name, m.base_power, m.base_pp, m.kind, m.accuracy
                    )
                })
                .collect();
        }
        ("system.sbin", n) if n.starts_with("itempar") => {
            v.kind = "items".into();
            v.lines = item::parse_items(data)?
                .iter()
                .map(|i| {
                    format!(
                        "{:3} {:20} buy {:5} sell {:5} cat {}",
                        i.id, i.name, i.buy_price, i.sell_price, i.category
                    )
                })
                .collect();
        }
        ("system.sbin", n) if n.starts_with("lvmp") => {
            v.kind = "level growth".into();
            v.lines = species::parse_level_data(data)?
                .iter()
                .enumerate()
                .map(|(l, d)| {
                    format!(
                        "Lv {:3} exp {:8} HP +{} atk +{:?} def +{:?}",
                        l + 1,
                        d.exp_required,
                        d.hp,
                        d.attack,
                        d.defense
                    )
                })
                .collect();
        }
        ("ground.sbin", n) if n.ends_with('m') => {
            let m = ground::parse_bma(data)?;
            let base = &n[..n.len() - 1];
            v.kind = "ground map".into();
            v.lines.push(format!(
                "{}x{} tiles, {}x{} chunks, {} layers, collision {}",
                m.width_tiles,
                m.height_tiles,
                m.width_chunks,
                m.height_chunks,
                m.layers.len(),
                m.has_collision
            ));
            let (Some(c), Some(p)) = (g.sub(pack, &format!("{base}c")), g.sub(pack, base)) else {
                v.kind = "chunk map without tiles or palettes of its own".into();
                return Ok(());
            };
            let (bpc, bpl) = (ground::parse_bpc(c)?, ground::parse_bpl(p)?);
            let bpa: Vec<_> = (1..=4)
                .map(|i| {
                    g.sub(pack, &format!("{base}{i}"))
                        .and_then(|d| ground::parse_bpa(d).ok())
                })
                .collect();
            // One image per layer, top to bottom; layer priority is not decoded.
            let layers = (0..m.layers.len())
                .map(|l| {
                    Ok(rgba(
                        &ground::render_layer(&bpc, &bpa, &m, l)?,
                        &bpl.palettes,
                    ))
                })
                .collect::<Result<Vec<_>, rescue_formats::error::FormatError>>()?;
            v.image = Some(grid(&layers, 1));
            v.lines.push(format!(
                "{} tiles, {} chunks, animated (BPA) tiles at frame 0",
                bpc.tiles, bpc.chunks
            ));
        }
        ("ground.sbin", n) if n.ends_with('c') => {
            let b = ground::parse_bpc(data)?;
            v.kind = "ground tiles (gray)".into();
            v.image = Some(rgba(
                &graphics::tile_sheet(&b.chr, Depth::Bpp4, 16)?,
                &[gray16()],
            ));
            v.lines
                .push(format!("{} tiles, {} chunks", b.tiles, b.chunks));
            v.bytes = b.chr;
        }
        ("ground.sbin", n) if !n.ends_with('c') => {
            if let Ok(b) = ground::parse_bpl(data) {
                v.kind = "ground palettes".into();
                v.image = Some(swatches(&b.palettes));
                v.lines.push(format!(
                    "{} animated",
                    b.animations.iter().flatten().count()
                ));
            }
        }
        _ => {
            if px::kind(data).is_some() {
                let raw = px::decompress(data, px::DEFAULT_LIMIT)?;
                v.kind = "AT4PX (shown as 4bpp tiles, gray)".into();
                v.image = Some(rgba(
                    &graphics::tile_sheet(&raw, Depth::Bpp4, 16)?,
                    &[gray16()],
                ));
                v.bytes = raw;
            } else if is_rgbx(data) {
                v.kind = "palettes".into();
                v.image = Some(swatches(&graphics::rgbx_palettes16(data)?));
            }
        }
    }
    Ok(())
}

/// RGBX palette data: whole palettes, every 4th byte 0x80 or 0.
fn is_rgbx(d: &[u8]) -> bool {
    !d.is_empty()
        && d.len().is_multiple_of(64)
        && d.chunks_exact(4).all(|c| c[3] == 0x80 || c[3] == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_swatches() {
        let a = Rgba {
            width: 2,
            height: 1,
            pixels: vec![255; 8],
        };
        let g = grid(&[a.clone(), a], 2);
        assert_eq!((g.width, g.height), (8, 3));
        assert_eq!(&g.pixels[..4], &[255; 4]);
        let s = swatches(&[gray16()]);
        assert_eq!((s.width, s.height), (160, 10));
        assert!(is_rgbx(&[1, 2, 3, 0x80].repeat(16)));
        assert!(!is_rgbx(&[1, 2, 3, 4].repeat(16)));
    }
}
