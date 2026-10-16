//! ROM-gated checks of Blue's own formats (#33, #34) against the configured dump: every
//! `.sbin` archive parses, every portrait and sprite decodes. Skipped without a ROM
//! (docs/rom-tests.md). Asserts counts and sizes only, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_formats::gfx::MapEntry;
use rescue_formats::{
    effect, font, ground, portrait, px, sir0::Sir0, sprite::Sprite, text, tileset,
};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

/// Bytes of the named ROM file.
fn rom_file(rom: &[u8], path: &str) -> Vec<u8> {
    let header = Header::parse(rom).expect("header parses");
    let fs = Filesystem::parse(rom, &header).expect("FNT/FAT parse");
    let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
    fs.data(rom, f.id).expect("file bytes").to_vec()
}

rescue_tests::rom_test! {
    /// Every `.sbin` except the SDAT parses as an archive, and every AT4PX sub-file and
    /// every SIR0 sub-file is well formed.
    fn archives_and_wrappers_parse(rom) {
        let bytes = rom.read();
        let mut px_count = 0;
        let mut sir0_count = 0;
        for name in ["dungeon", "effect", "ground", "monster", "ornament", "sample", "system", "titlemenu"] {
            let data = rom_file(&bytes, &format!("{name}.sbin"));
            let arc = Archive::parse(&data).unwrap_or_else(|e| panic!("{name}.sbin: {e}"));
            for e in arc.entries() {
                let sub = arc.data(&data, e).unwrap();
                if px::kind(sub).is_some() {
                    px::decompress(sub, px::DEFAULT_LIMIT).unwrap_or_else(|err| panic!("{name}/{}: {err}", e.name));
                    px_count += 1;
                } else if Sir0::is_sir0(sub) {
                    Sir0::parse(sub).unwrap_or_else(|err| panic!("{name}/{}: {err}", e.name));
                    sir0_count += 1;
                }
            }
        }
        assert_eq!((px_count, sir0_count), (154, 1715));
    }
}

rescue_tests::rom_test! {
    /// 73 portrait files, 13 emotion slots each, all 40x40.
    fn portraits_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "monster.sbin");
        let arc = Archive::parse(&data).unwrap();
        let mut files = 0;
        let mut faces = 0;
        for e in arc.entries().iter().filter(|e| e.name.starts_with("kao")) {
            assert!(portrait::species_of(&e.name).is_some());
            let p = portrait::parse(arc.data(&data, e).unwrap()).unwrap_or_else(|err| panic!("{}: {err}", e.name));
            assert!(!p.is_empty() && p.iter().all(|f| f.image.width == 40 && f.image.height == 40));
            files += 1;
            faces += p.len();
        }
        assert_eq!(files, 73);
        assert!(faces > 150, "{faces} portraits");
    }
}

rescue_tests::rom_test! {
    /// 423 sprite files; every frame of every sprite renders and every animation step
    /// names a frame that exists.
    fn sprites_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "monster.sbin");
        let arc = Archive::parse(&data).unwrap();
        let palettes = rescue_formats::graphics::rgbx_palettes16(arc.get(&data, "palet").unwrap()).unwrap();
        assert_eq!(palettes.len(), 14);
        let mut count = 0;
        for e in arc.entries().iter().filter(|e| e.name.starts_with("ax")) {
            let s = Sprite::parse(arc.data(&data, e).unwrap()).unwrap_or_else(|err| panic!("{}: {err}", e.name));
            assert!(s.groups.len() >= 13, "{}: {} groups", e.name, s.groups.len());
            for i in 0..s.frames.len() {
                let f = s.render_frame(i).unwrap_or_else(|err| panic!("{} frame {i}: {err}", e.name));
                assert!(f.image.pixels.iter().flatten().all(|&(p, _)| usize::from(p) < palettes.len()));
            }
            count += 1;
        }
        assert_eq!(count, 423);
    }
}

rescue_tests::rom_test! {
    /// Every dungeon tileset loads; full ones render all metatiles and have an
    /// autotile table whose entries name existing metatiles.
    fn tilesets_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "dungeon.sbin");
        let arc = Archive::parse(&data).unwrap();
        let ids = tileset::indices(&arc);
        assert_eq!(ids.len(), 76);
        for id in ids {
            let t = tileset::Tileset::load(&arc, &data, id).unwrap_or_else(|e| panic!("tileset {id}: {e}"));
            assert_eq!(t.palettes.len(), 12, "tileset {id}");
            for m in 0..t.metatiles.len() {
                t.render_metatile(m).unwrap_or_else(|e| panic!("tileset {id} metatile {m}: {e}"));
            }
            if let Some(table) = &t.autotile {
                assert!(table[..2304].iter().all(|&m| usize::from(m) < t.metatiles.len()), "tileset {id}");
            }
        }
    }
}

rescue_tests::rom_test! {
    /// All five languages: .str and .bin hold the same 3335 strings (the .str has one
    /// trailing empty entry), every string decodes, English starts with the type names.
    fn messages_decode(rom) {
        let bytes = rom.read();
        for lang in ["e", "f", "g", "i", "s"] {
            let s = rom_file(&bytes, &format!("message_{lang}.str"));
            let b = rom_file(&bytes, &format!("message_{lang}.bin"));
            let strs = text::parse_str(&s).unwrap();
            let bins = text::parse_bin(&b).unwrap();
            assert_eq!((strs.len(), bins.len()), (3336, 3335), "{lang}");
            assert!(strs.iter().zip(&bins).all(|(a, b)| a == b), "{lang}");
            for st in &strs {
                text::decode(st);
            }
        }
        let s = rom_file(&bytes, "message_e.str");
        let strs = text::parse_str(&s).unwrap();
        assert_eq!(text::to_plain(&text::decode(strs[2])), "Fire");
    }
}

rescue_tests::rom_test! {
    /// The dialogue fonts decode; kanji_a covers printable ASCII.
    fn fonts_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "system.sbin");
        let arc = Archive::parse(&data).unwrap();
        let a = font::parse_font(arc.get(&data, "kanji_a").unwrap()).unwrap();
        let b = font::parse_font(arc.get(&data, "kanji_b").unwrap()).unwrap();
        assert_eq!(a.len(), 473);
        assert!(!b.is_empty());
        assert!((0x20..0x7F).all(|c| a.iter().any(|g| g.code == c)));
        assert_eq!(font::parse_fixed_font(arc.get(&data, "font").unwrap()).unwrap().len(), 136);
    }
}

rescue_tests::rom_test! {
    /// Every ground map's palette (BPL), tiles and chunks (BPC) and chunk map (BMA)
    /// decode, the map only names chunks its BPC declares, chunks only name tiles of the
    /// BPC or its animated (BPA) slots and palettes the BPL has, and every map renders. A
    /// collision/data tail is present exactly when the header flag says so.
    fn ground_maps_parse(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "ground.sbin");
        let arc = Archive::parse(&data).unwrap();
        let (mut maps, mut rendered) = (0, 0);
        for e in arc.entries().iter().filter(|e| e.name.len() > 1 && e.name.ends_with('m')) {
            let base = &e.name[..e.name.len() - 1];
            let bma = ground::parse_bma(arc.data(&data, e).unwrap()).unwrap_or_else(|err| panic!("{}: {err}", e.name));
            assert_eq!(bma.has_collision || bma.has_data_layer, !bma.extra.is_empty(), "{}", e.name);
            maps += 1;
            // A few maps (`B01P02A`, ...) have no tiles or palettes of their own.
            let (Some(c), Some(p)) = (arc.find(&format!("{base}c")), arc.find(base)) else {
                continue;
            };
            let bpc = ground::parse_bpc(arc.data(&data, c).unwrap()).unwrap_or_else(|err| panic!("{base}c: {err}"));
            assert!(bma.layers.iter().flatten().flatten().all(|&ch| ch < bpc.chunks), "{}", e.name);
            let bpl = ground::parse_bpl(arc.data(&data, p).unwrap()).unwrap_or_else(|err| panic!("{base}: {err}"));
            let vram_tiles = bpc.tiles + bpc.bpa_tiles.iter().sum::<u16>();
            for &raw in &bpc.chunk_map {
                let t = MapEntry::from_raw(raw);
                assert!(t.tile < vram_tiles && (t.tile == 0 || usize::from(t.palette) < bpl.palettes.len()), "{base}c: entry {raw:#06x}");
            }
            // Each declared animated-tile slot `n` is part `X{n+1}` with that many tiles.
            let bpa: Vec<_> = (1..=4)
                .map(|i| arc.find(&format!("{base}{i}")).map(|f| ground::parse_bpa(arc.data(&data, f).unwrap()).unwrap()))
                .collect();
            for (slot, &n) in bpc.bpa_tiles.iter().enumerate() {
                assert_eq!(bpa[slot].as_ref().map_or(0, |b| b.tiles), n, "{base} BPA slot {slot}");
            }
            let drawn = (0..bma.layers.len()).any(|l| {
                let img = ground::render_layer(&bpc, &bpa, &bma, l).unwrap_or_else(|err| panic!("{}: {err}", e.name));
                img.pixels.iter().any(Option::is_some)
            });
            assert!(drawn, "{} renders empty", e.name);
            rendered += 1;
        }
        assert!(maps > 190 && rendered > 180, "{maps} maps, {rendered} rendered");
        eprintln!("{maps} maps, {rendered} rendered");
    }
}

rescue_tests::rom_test! {
    /// Mutation check (#38): real sub-files with a few bytes flipped must give `Ok` or
    /// `Err`, never a panic, hang or runaway allocation. Deterministic (fixed LCG).
    fn mutated_assets_never_panic(rom) {
        let bytes = rom.read();
        let mut seed: u64 = 0x5EED;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let mon = rom_file(&bytes, "monster.sbin");
        let marc = Archive::parse(&mon).unwrap();
        let sys = rom_file(&bytes, "system.sbin");
        let sarc = Archive::parse(&sys).unwrap();
        let dun = rom_file(&bytes, "dungeon.sbin");
        let darc = Archive::parse(&dun).unwrap();
        let picks: Vec<(&str, Vec<u8>)> = vec![
            ("ax001", marc.get(&mon, "ax001").unwrap().to_vec()),
            ("ax150", marc.get(&mon, "ax150").unwrap().to_vec()),
            ("kao001", marc.get(&mon, "kao001").unwrap().to_vec()),
            ("kanji_a", sarc.get(&sys, "kanji_a").unwrap().to_vec()),
            ("wazapara", sarc.get(&sys, "wazapara").unwrap().to_vec()),
            ("monspara", sarc.get(&sys, "monspara").unwrap().to_vec()),
            ("lvmp001", sarc.get(&sys, "lvmp001").unwrap().to_vec()),
            ("mapparam", darc.get(&dun, "mapparam").unwrap().to_vec()),
            ("b00cex", darc.get(&dun, "b00cex").unwrap().to_vec()),
            ("b00canm", darc.get(&dun, "b00canm").unwrap().to_vec()),
        ];
        for (_name, base) in &picks {
            for _ in 0..150 {
                let mut d = base.clone();
                for _ in 0..1 + next() % 4 {
                    let i = next() % d.len();
                    d[i] = next() as u8;
                }
                if let Ok(s) = Sprite::parse(&d) {
                    for f in 0..s.frames.len().min(8) {
                        let _ = s.render_frame(f);
                    }
                }
                let _ = portrait::parse(&d);
                let _ = font::parse_font(&d);
                let _ = rescue_data::moves::parse_moves(&d);
                let _ = rescue_data::species::parse_species(&d);
                let _ = rescue_data::species::parse_level_data(&d);
                let _ = rescue_data::dungeon::parse_mapparam(&d, 240);
                let _ = tileset::parse_autotile(&d);
                let _ = tileset::parse_palette_anim(&d);
            }
        }
    }
}

rescue_tests::rom_test! {
    /// 139 effect objects and 8 backgrounds parse; every object pose renders (empty
    /// poses are null pointers and are skipped), every background frame has a tilemap.
    fn effects_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "effect.sbin");
        let arc = Archive::parse(&data).unwrap();
        let (mut objects, mut backgrounds) = (0, 0);
        for e in arc.entries() {
            let sub = arc.data(&data, e).unwrap();
            if e.name.starts_with("efob") {
                let o = effect::parse_object(sub).unwrap_or_else(|err| panic!("{}: {err}", e.name));
                for i in 0..o.sprite.frames.len() {
                    if !o.sprite.frames[i].is_empty() {
                        o.sprite.render_frame(i).unwrap_or_else(|err| panic!("{} pose {i}: {err}", e.name));
                    }
                }
                objects += 1;
            } else if e.name.starts_with("efbg") {
                let b = effect::parse_background(sub).unwrap_or_else(|err| panic!("{}: {err}", e.name));
                assert!(b.frames.iter().flatten().all(|f| !f.map.is_empty()), "{}", e.name);
                backgrounds += 1;
            }
        }
        assert_eq!((objects, backgrounds), (139, 8));
    }
}
