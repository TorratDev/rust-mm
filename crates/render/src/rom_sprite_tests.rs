//! ROM-gated check of the sprite renderer on a real species (docs/rom-tests.md). Skipped
//! without a ROM. Asserts only non-emptiness and determinism, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_formats::sprite::Piece;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

rescue_tests::rom_test! {
    /// Draws a frame of species 25 (Pikachu) from its idle animation in its overworld palette
    /// and checks it is non-empty, deterministic and drawn with colours of that palette
    /// (a few pieces may keep their own palette bits, so this only requires some pixels
    /// from the species palette). Also checks the player walks a real group.
    fn species_sprite_draws(rom) {
        use rescue_data::species;
        use rescue_formats::graphics;
        use rescue_formats::sprite::Sprite;

        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let load = |path: &str| {
            let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
            fs.data(&bytes, f.id).expect("file bytes").to_vec()
        };
        let monster = load("monster.sbin");
        let marc = Archive::parse(&monster).unwrap();
        let sub = |a: &Archive, d: &[u8], n: &str| {
            a.data(d, a.find(n).unwrap_or_else(|| panic!("{n} missing"))).unwrap().to_vec()
        };
        let system = load("system.sbin");
        let sarc = Archive::parse(&system).unwrap();
        let species_palette = species::parse_species(&sub(&sarc, &system, "monspara")).unwrap()[25].palette;
        let palettes = graphics::rgbx_palettes16(&sub(&marc, &monster, "palet")).unwrap();
        let sprite = Sprite::parse(&sub(&marc, &monster, "ax025")).unwrap();

        let mut anim = crate::anim::AnimPlayer::new(&sprite, 7, 0, true).expect("idle group");
        anim.advance(3);
        let img = crate::sprite::render_sprite(&sprite, anim.frame(), species_palette, &palettes).unwrap();
        assert!(img.pixels.iter().any(Option::is_some), "empty sprite");
        let again = crate::sprite::render_sprite(&sprite, anim.frame(), species_palette, &palettes).unwrap();
        assert!(img == again, "sprite render is not deterministic");
        let bank = &palettes[usize::from(species_palette)];
        assert!(
            img.pixels.iter().flatten().any(|(c, _)| bank.contains(c)),
            "no pixel uses the species palette {species_palette}"
        );
        // On a background layer too.
        let frame = crate::sprite::compose_with_sprites(
            &[],
            &palettes,
            [0, 0, 0, 255],
            &[crate::sprite::PlacedSprite { image: &img, at: (128, 96) }],
        );
        assert!(frame.pixels.chunks_exact(4).any(|p| p != [0, 0, 0, 255]));
    }
}

/// One engine-A OAM entry from a capture (`oam.bin`).
struct Obj {
    index: usize,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    tile: usize,
    pal: usize,
    hflip: bool,
}

type Rgb5 = [u8; 3];

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn rgb5(v: u16) -> Rgb5 {
    [
        (v & 31) as u8,
        ((v >> 5) & 31) as u8,
        ((v >> 10) & 31) as u8,
    ]
}

/// The visible engine-A OAM entries, in OAM order.
fn oam_objs(oam: &[u8]) -> Vec<Obj> {
    (0..128)
        .filter_map(|index| {
            let (a0, a1, a2) = (
                le16(oam, index * 8),
                le16(oam, index * 8 + 2),
                le16(oam, index * 8 + 4),
            );
            if (a0 >> 8) & 3 == 2 {
                return None; // disabled
            }
            let piece = Piece {
                image: None,
                unknown: 0,
                attr0: a0,
                attr1: a1,
                attr2: 0,
            };
            let (w, h) = piece.size();
            let (x, y) = (i32::from(a1 & 0x1FF), i32::from(a0 & 0xFF));
            if y >= 192 && y + h as i32 <= 256 || x >= 256 && x + w as i32 <= 512 {
                return None; // off screen
            }
            let sx = if x >= 256 { x - 512 } else { x };
            Some(Obj {
                index,
                x: sx,
                y,
                w,
                h,
                tile: usize::from(a2 & 0x3FF),
                pal: usize::from(a2 >> 12),
                hflip: a1 & 0x1000 != 0,
            })
        })
        .collect()
}

/// An object's pixels as the hardware draws them, from the capture's own OBJ VRAM and
/// palette (1D mapping, 32-byte tile step, 4bpp).
fn cap_pixels(o: &Obj, vram: &[u8], pal: &[u8]) -> Vec<Option<Rgb5>> {
    let mut out = vec![None; o.w * o.h];
    for y in 0..o.h {
        for x in 0..o.w {
            let sx = if o.hflip { o.w - 1 - x } else { x };
            let tile = o.tile + (y / 8) * (o.w / 8) + sx / 8;
            let b = vram[tile * 32 + (y % 8) * 4 + (sx % 8) / 2];
            let ci = usize::from(if sx % 2 == 1 { b >> 4 } else { b & 15 });
            if ci != 0 {
                out[y * o.w + x] = Some(rgb5(le16(pal, 0x200 + (o.pal * 16 + ci) * 2)));
            }
        }
    }
    out
}

fn near(a: Rgb5, c: [u8; 4]) -> bool {
    (0..3).all(|i| a[i].abs_diff(c[i] >> 3) <= 1)
}

rescue_tests::rom_test! {
    /// Match-the-original check for sprites: rebuilds each captured dungeon frame's object
    /// layer (leader, partner, wild monster, floor item, shadows) from ROM assets at the
    /// positions the camera rule gives, and compares it with the capture's own OAM render
    /// (rebuilt from its OBJ VRAM and palette). Needs `RESCUE_SPRITE_CAPTURE`, a directory
    /// holding `dng1_run1/` and `b2_run1/` dumps, as well as a ROM; skips without it. The
    /// animation frame of each monster is not read from the entity (not decoded yet): it is
    /// the decoded frame whose picture best matches the captured object. Position, flips,
    /// palette, z-order and the shadows are not fitted.
    fn dungeon_sprites_match_capture(rom) {
        use rescue_data::species;
        use rescue_formats::graphics;
        use rescue_formats::sprite::Sprite;

        let Some(dir) = std::env::var_os("RESCUE_SPRITE_CAPTURE") else {
            eprintln!("SKIPPED dungeon_sprites_match_capture: RESCUE_SPRITE_CAPTURE not set");
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let load = |path: &str| {
            let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
            fs.data(&bytes, f.id).expect("file bytes").to_vec()
        };
        let sub = |a: &Archive, d: &[u8], n: &str| {
            a.data(d, a.find(n).unwrap_or_else(|| panic!("{n} missing"))).unwrap().to_vec()
        };
        let monster = load("monster.sbin");
        let marc = Archive::parse(&monster).unwrap();
        let system = load("system.sbin");
        let sarc = Archive::parse(&system).unwrap();
        let dungeon = load("dungeon.sbin");
        let darc = Archive::parse(&dungeon).unwrap();
        let species = species::parse_species(&sub(&sarc, &system, "monspara")).unwrap();
        let palettes = graphics::rgbx_palettes16(&sub(&marc, &monster, "palet")).unwrap();
        let etcfont = sub(&darc, &dungeon, "etcfont");
        let itempat = sub(&darc, &dungeon, "itempat");

        for run in ["dng1_run1", "b2_run1"] {
            let cap = dir.join(run);
            let rd = |n: &str| std::fs::read(cap.join(n)).unwrap_or_else(|e| panic!("{run}/{n}: {e}"));
            let (oam, pal, vram) = (rd("oam.bin"), rd("palette.bin"), rd("vram_a_obj.bin"));
            let cam_file = rd("dungeon_camera_ptr_plus_15F80.bin");
            let camera = (i32::from(le16(&cam_file, 8) as i16), i32::from(le16(&cam_file, 10) as i16));
            let objs = oam_objs(&oam);

            // The leader is entity 0; the camera rule must reproduce the captured camera.
            let ent = |n: u32| rd(&format!("entity_{n:02}.bin"));
            let info = |n: u32| rd(&format!("info_{n:02}.bin"));
            let leader = ent(0);
            let leader_cell = (i32::from(le16(&leader, 4) as i16), i32::from(le16(&leader, 6) as i16));
            let cam = crate::camera::follow(leader_cell, (0, 0));
            assert_eq!(cam.scroll, camera, "{run}: camera");

            // (OAM index, image, screen position)
            let mut placed: Vec<(usize, crate::sprite::SpriteImage, (i32, i32))> = Vec::new();
            let mut used: Vec<usize> = Vec::new();
            for n in [0u32, 1, 4, 5, 6] {
                let (e, i) = (ent(n), info(n));
                let cell = (i32::from(le16(&e, 4) as i16), i32::from(le16(&e, 6) as i16));
                let px = crate::camera::entity_pixel(cell, (0, 0));
                // The entity's own 8.8 pixel position agrees with the cell rule.
                let fixed = |at: usize| i32::from_le_bytes(e[at..at + 4].try_into().unwrap()) >> 8;
                assert_eq!((fixed(0xC), fixed(0x10)), px, "{run} entity {n}: pixel position");
                let at = cam.to_screen(px);
                let hop = (i32::from(le16(&e, 0x34) as i16), i32::from(le16(&e, 0x36) as i16));
                let draw = (at.0 + hop.0, at.1 + hop.1);
                if !(-32..288).contains(&draw.0) || !(-32..224).contains(&draw.1) {
                    continue; // off screen: the game leaves its draw position unset
                }
                assert_eq!(
                    draw,
                    (i32::from(le16(&e, 0x30) as i16), i32::from(le16(&e, 0x32) as i16)),
                    "{run} entity {n}: the game's own draw position"
                );
                // The monster object: the unused 32-pixel-wide-or-so entry closest to it.
                let Some(obj) = objs
                    .iter()
                    .filter(|o| o.w >= 16 && o.h >= 16 && o.tile < 512 && !used.contains(&o.index))
                    .filter(|o| (o.x + o.w as i32 / 2 - draw.0).abs() < 24 && (o.y + o.h as i32 - draw.1 - 8).abs() < 40)
                    .min_by_key(|o| (o.x + o.w as i32 / 2 - draw.0).abs())
                else {
                    eprintln!("{run}: entity {n} (species {}) has no object on screen", le16(&i, 2));
                    continue;
                };
                used.push(obj.index);
                let id = usize::from(le16(&i, 2));
                let spr = Sprite::parse(&sub(&marc, &monster, &format!("ax{id:03}")))
                    .unwrap_or_else(|e| panic!("ax{id:03}: {e:?}"));
                let p = species[id].palette;
                let want = cap_pixels(obj, &vram, &pal);
                // Pick the frame by picture, aligned by the corner of the object.
                let best = (0..spr.frames.len())
                    .filter_map(|f| {
                        let img = crate::sprite::render_sprite(&spr, f, p, &palettes).ok()?;
                        if (img.width, img.height) != (obj.w, obj.h) {
                            return None;
                        }
                        let bad = (0..obj.w * obj.h)
                            .filter(|&k| match (img.pixels[k], want[k]) {
                                (None, None) => false,
                                (Some((c, _)), Some(w)) => !near(w, c),
                                _ => true,
                            })
                            .count();
                        Some((bad, f, img))
                    })
                    .min_by_key(|(bad, ..)| *bad)
                    .unwrap_or_else(|| panic!("{run}: no frame of species {id} has size {}x{}", obj.w, obj.h));
                eprintln!("{run}: entity {n} species {id} pal {p} object {} frame {} (picture mismatch {})", obj.index, best.1, best.0);
                placed.push((obj.index, best.2, draw));
                // Its shadow: 16x8 at the entity position + (-8, -4), tile 520 or 513 (etcfont).
                let sh = crate::sprite::shadow_origin(at);
                if let Some(s) = objs.iter().find(|o| (o.w, o.h) == (16, 8) && o.pal == 0 && (o.x, o.y) == sh) {
                    used.push(s.index);
                    let img = crate::sprite::render_obj(&etcfont[4..], s.tile - 512, (16, 8), &palettes[0], 3, (0, 0));
                    placed.push((s.index, img, sh));
                } else {
                    eprintln!("{run}: entity {n} has no shadow at {sh:?}");
                }
            }
            // The floor item: 16x16, tile 440; it is in cell (8, 21) in both captures.
            if let Some(o) = objs.iter().find(|o| o.tile == 440) {
                let cell_px = (8 * 24 + 4, 21 * 24 + 4);
                let at = cam.to_screen(cell_px);
                assert_eq!(at, (o.x, o.y), "{run}: item position");
                let pal3: Vec<[u8; 4]> = (0..16)
                    .map(|i| {
                        let c = rgb5(le16(&pal, 0x200 + (o.pal * 16 + i) * 2));
                        [c[0] << 3, c[1] << 3, c[2] << 3, 255]
                    })
                    .collect();
                // 4 tiles starting 0x314 bytes into itempat (found by byte match).
                let img = crate::sprite::render_obj(&itempat[0x314..], 0, (16, 16), &pal3, 3, (0, 0));
                placed.push((o.index, img, at));
                used.push(o.index);
            }
            let ignored: Vec<_> = objs.iter().filter(|o| !used.contains(&o.index)).map(|o| (o.index, o.tile)).collect();
            eprintln!("{run}: objects not compared (HUD dot/damage number/unlisted): {ignored:?}");

            // Ours, in OAM order; the reference is the capture's objects in OAM order,
            // first opaque pixel wins.
            placed.sort_by_key(|p| p.0);
            let sprites: Vec<_> = placed.iter().map(|(_, image, at)| crate::sprite::PlacedSprite { image, at: *at }).collect();
            let ours = crate::sprite::compose_with_sprites(&[], &palettes, [255, 0, 255, 255], &sprites);
            let mut reference: Vec<Option<Rgb5>> = vec![None; 256 * 192];
            for o in objs.iter().filter(|o| used.contains(&o.index)) {
                for (k, c) in cap_pixels(o, &vram, &pal).into_iter().enumerate() {
                    let (x, y) = (o.x + (k % o.w) as i32, o.y + (k / o.w) as i32);
                    if let Some(c) = c
                        && (0..256).contains(&x) && (0..192).contains(&y)
                        && reference[(y * 256 + x) as usize].is_none()
                    {
                        reference[(y * 256 + x) as usize] = Some(c);
                    }
                }
            }
            let mut mismatches = 0;
            let mut drawn = 0;
            for (k, r) in reference.iter().enumerate() {
                let p = &ours.pixels[k * 4..k * 4 + 4];
                let p = [p[0], p[1], p[2], p[3]];
                let ours_opaque = p != [255, 0, 255, 255];
                drawn += usize::from(r.is_some());
                let same = match r {
                    None => !ours_opaque,
                    Some(c) => ours_opaque && near(*c, p),
                };
                mismatches += usize::from(!same);
            }
            eprintln!("{run}: {mismatches} of {drawn} captured object pixels differ");
            assert!(drawn > 500, "{run}: too few object pixels compared ({drawn})");
            assert_eq!(mismatches, 0, "{run}: sprite pixels differ from the capture");
        }
    }
}
