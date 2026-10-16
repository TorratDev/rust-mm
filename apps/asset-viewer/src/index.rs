//! Searchable resource index (#40): one row per ROM file, archive sub-file and decoded
//! resource, with its format, a decoded name (species, item, move) and size facts.
//!
//! Built from the user's dump, so it holds game data: it is written locally
//! ([`DEFAULT_PATH`], git-ignored) and never committed; only this generator is.

use std::fmt::Write as _;

use rescue_data::{dungeon, species};
use rescue_formats::{effect, font, ground, portrait, px, sir0::Sir0, sprite, text, tileset};

use crate::game::Game;

/// Default location of the generated index (git-ignored).
pub const DEFAULT_PATH: &str = "rescue-index.tsv";

/// One indexed resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    /// ROM file path.
    pub file: String,
    /// Archive sub-file name, or empty for a whole ROM file.
    pub sub: String,
    /// Format (`portrait`, `sprite`, `tileset`, `sir0`, `at4px`, ...).
    pub format: String,
    /// Decoded name, if any.
    pub name: String,
    /// Size facts: dimensions, frame/entry counts.
    pub info: String,
}

impl Resource {
    fn matches(&self, term: &str) -> bool {
        [&self.file, &self.sub, &self.format, &self.name, &self.info]
            .iter()
            .any(|f| f.to_lowercase().contains(term))
    }
}

/// Builds the index.
pub fn build(g: &Game) -> Vec<Resource> {
    let mut out = Vec::new();
    for f in g.fs.files() {
        let format = if g.pack(&f.path).is_some() {
            "archive"
        } else if f.path.ends_with(".str") || f.path.ends_with(".bin") {
            "text"
        } else if f.path == "sound.sbin" {
            "sdat"
        } else {
            "file"
        };
        let mut info = format!("{} bytes, file ID {}", f.len(), f.id);
        if f.path.ends_with(".str")
            && let Some(d) = g.file(&f.path)
            && let Ok(s) = text::parse_str(d)
        {
            let _ = write!(info, ", {} strings", s.len());
        }
        out.push(Resource {
            file: f.path.clone(),
            sub: String::new(),
            format: format.into(),
            name: String::new(),
            info,
        });
    }
    for p in &g.packs {
        for e in p.archive.entries() {
            let d = p.archive.data(&p.data, e).unwrap_or_default();
            let (format, name, info) = classify(g, &p.path, &e.name, d);
            out.push(Resource {
                file: p.path.clone(),
                sub: e.name.clone(),
                format,
                name,
                info,
            });
        }
    }
    for (i, n) in g.moves.iter().enumerate() {
        out.push(Resource {
            file: "system.sbin".into(),
            sub: "wazapara".into(),
            format: "move".into(),
            name: n.clone(),
            info: format!("move {i}"),
        });
    }
    for (i, n) in g.items.iter().enumerate() {
        out.push(Resource {
            file: "system.sbin".into(),
            sub: "itempara".into(),
            format: "item".into(),
            name: n.clone(),
            info: format!("item {i}"),
        });
    }
    for (i, n) in g.species.iter().enumerate() {
        out.push(Resource {
            file: "system.sbin".into(),
            sub: "monspara".into(),
            format: "species".into(),
            name: n.clone(),
            info: format!("species {i}"),
        });
    }
    if let Some(p) = g.pack("dungeon.sbin")
        && let Some(d) = p.archive.get(&p.data, "mapparam").ok()
        && let Ok(mp) = dungeon::parse_mapparam(d, g.items.len())
    {
        for (i, floors) in mp.dungeons.iter().enumerate() {
            out.push(Resource {
                file: "dungeon.sbin".into(),
                sub: "mapparam".into(),
                format: "dungeon".into(),
                name: String::new(),
                info: format!("dungeon {i}, {} floors", floors.len()),
            });
        }
    }
    out
}

/// Detected format, name and info for an archive sub-file.
pub fn classify(g: &Game, pack: &str, name: &str, d: &[u8]) -> (String, String, String) {
    let size = d.len();
    if pack == "monster.sbin"
        && let Some(sp) = portrait::species_of(name)
        && let Ok(p) = portrait::parse(d)
    {
        return (
            "portrait".into(),
            g.species_name(sp),
            format!("{} emotions, 40x40", p.len()),
        );
    }
    if pack == "monster.sbin"
        && let Some(sp) = sprite::species_of(name)
        && let Ok(s) = sprite::Sprite::parse(d)
    {
        let info = format!(
            "{} frames, {} images, {} animations",
            s.frames.len(),
            s.images.len(),
            s.groups.len()
        );
        return ("sprite".into(), g.species_name(sp), info);
    }
    if pack == "monster.sbin" && name == "palet" {
        return (
            "palettes".into(),
            String::new(),
            format!("{} palettes", size / 64),
        );
    }
    if pack == "dungeon.sbin"
        && let Some(n) = name
            .strip_prefix('b')
            .and_then(|r| r.strip_suffix("pal"))
            .and_then(|n| n.parse::<u8>().ok())
        && let Some(p) = g.pack(pack)
        && let Ok(t) = tileset::Tileset::load(&p.archive, &p.data, n)
    {
        let info = format!(
            "{} metatiles, {} tiles, autotile {}",
            t.metatiles.len(),
            t.chr.len() / 32,
            t.autotile.is_some()
        );
        return ("tileset".into(), format!("tileset {n}"), info);
    }
    if pack == "ground.sbin" {
        if let Some(base) = name.strip_suffix('m')
            && let Ok(m) = ground::parse_bma(d)
        {
            let info = format!(
                "{}x{} chunks, {} layers",
                m.width_chunks,
                m.height_chunks,
                m.layers.len()
            );
            return ("ground map".into(), base.into(), info);
        }
        if let Some(base) = name.strip_suffix('c')
            && let Ok(b) = ground::parse_bpc(d)
        {
            return (
                "ground tiles".into(),
                base.into(),
                format!("{} tiles, {} chunks", b.tiles, b.chunks),
            );
        }
        if let Ok(b) = ground::parse_bpl(d) {
            return (
                "ground palettes".into(),
                name.into(),
                format!("{} palettes", b.palettes.len()),
            );
        }
    }
    if pack == "system.sbin"
        && name.starts_with("kanji")
        && let Ok(f) = font::parse_font(d)
    {
        return (
            "font".into(),
            name.into(),
            format!("{} glyphs, 12x12", f.len()),
        );
    }
    if pack == "system.sbin"
        && let Some(sp) = name
            .strip_prefix("lvmp")
            .and_then(|n| n.parse::<u16>().ok())
        && let Ok(levels) = species::parse_level_data(d)
    {
        return (
            "level growth".into(),
            g.species_name(sp),
            format!("{} levels", levels.len()),
        );
    }
    if pack == "effect.sbin"
        && name.starts_with("efob")
        && let Ok(o) = effect::parse_object(d)
    {
        let info = format!(
            "{} poses, {} animations, {} tiles",
            o.sprite.frames.len(),
            o.sprite.groups.len(),
            o.sprite.tiles.len() / 32
        );
        return ("effect object".into(), String::new(), info);
    }
    if pack == "effect.sbin"
        && name.starts_with("efbg")
        && let Ok(b) = effect::parse_background(d)
    {
        let info = format!("{} frames, {} tiles", b.frames.len(), b.tiles.len() / 32);
        return ("effect background".into(), String::new(), info);
    }
    if pack == "dungeon.sbin"
        && let Some((n, part)) = tileset_part(name)
    {
        match part {
            "canm" => {
                if let Ok(slots) = tileset::parse_palette_anim(d) {
                    let animated = slots.iter().flatten().count();
                    return (
                        "tileset animation".into(),
                        format!("tileset {n}"),
                        format!("{animated} animated slots"),
                    );
                }
            }
            "cex" => {
                if let Ok(table) = tileset::parse_autotile(d) {
                    return (
                        "autotile table".into(),
                        format!("tileset {n}"),
                        format!("{} entries", table.len()),
                    );
                }
            }
            "fon" | "cel" => {
                if let Ok(raw) = px::decompress(d, px::DEFAULT_LIMIT) {
                    let info = if part == "cel" {
                        format!("{} metatiles", tileset::parse_metatiles(&raw).len())
                    } else {
                        format!("{} tiles", raw.len() / 32)
                    };
                    let format = if part == "cel" {
                        "tileset metatiles"
                    } else {
                        "tileset tiles"
                    };
                    return (format.into(), format!("tileset {n}"), info);
                }
            }
            _ => {}
        }
    }
    if px::kind(d).is_some() {
        let n = px::header(d).map(|h| h.decoded_size).unwrap_or(0);
        return (
            "at4px".into(),
            String::new(),
            format!("{size} bytes, {n} decoded"),
        );
    }
    if Sir0::is_sir0(d) {
        return ("sir0".into(), String::new(), format!("{size} bytes"));
    }
    ("raw".into(), String::new(), format!("{size} bytes"))
}

/// Tileset number and part (`canm`, `cex`, `fon`, `cel`, ...) of a `bNN<part>` sub-file.
fn tileset_part(name: &str) -> Option<(u8, &str)> {
    let rest = name.strip_prefix('b')?;
    let split = rest.find(|c: char| !c.is_ascii_digit())?;
    Some((rest[..split].parse().ok()?, &rest[split..]))
}

/// Case-insensitive substring search over every field.
pub fn search<'a>(index: &'a [Resource], term: &str) -> Vec<&'a Resource> {
    let t = term.to_lowercase();
    index.iter().filter(|r| r.matches(&t)).collect()
}

/// Tab-separated text, one resource per line, with a header row.
pub fn to_tsv(index: &[Resource]) -> String {
    let mut s = String::from("file\tsub\tformat\tname\tinfo\n");
    for r in index {
        let clean = |x: &str| x.replace(['\t', '\n'], " ");
        let _ = writeln!(
            s,
            "{}\t{}\t{}\t{}\t{}",
            clean(&r.file),
            clean(&r.sub),
            clean(&r.format),
            clean(&r.name),
            clean(&r.info)
        );
    }
    s
}

/// Parses [`to_tsv`] output (lines with too few fields are skipped).
pub fn from_tsv(s: &str) -> Vec<Resource> {
    s.lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.splitn(5, '\t').map(str::to_owned);
            Some(Resource {
                file: f.next()?,
                sub: f.next()?,
                format: f.next()?,
                name: f.next()?,
                info: f.next()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsv_round_trip_and_search() {
        let r = vec![
            Resource {
                file: "monster.sbin".into(),
                sub: "kao001".into(),
                format: "portrait".into(),
                name: "Bulba\tsaur".into(),
                info: "13".into(),
            },
            Resource {
                file: "a".into(),
                sub: String::new(),
                format: "raw".into(),
                name: String::new(),
                info: String::new(),
            },
        ];
        let back = from_tsv(&to_tsv(&r));
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "Bulba saur");
        assert_eq!(search(&back, "PORTRAIT").len(), 1);
        assert_eq!(search(&back, "").len(), 2);
    }
}
