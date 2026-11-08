//! ROM-gated check of the BG renderer on a real ground map (docs/rom-tests.md). Skipped
//! without a ROM. Asserts only that the frame is non-empty and deterministic, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_formats::graphics::Palette;
use rescue_formats::ground;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

use rescue_formats::tileset;

use crate::autotile::{GRID_H, GRID_W, autotile};
use crate::bg::{BgLayer, compose, ground_layer, ground_layer_at};

rescue_tests::rom_test! {
    /// Renders layer 0 of the first town map (`T*`) that has tiles and palettes, twice, and
    /// checks the frame is not just the backdrop and both renders agree.
    fn town_map_bg_renders_non_empty_and_deterministic(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let f = fs.get("ground.sbin").expect("ground.sbin present");
        let data = fs.data(&bytes, f.id).expect("file bytes").to_vec();
        let arc = Archive::parse(&data).unwrap();
        let mut rendered = 0;
        for e in arc
            .entries()
            .iter()
            .filter(|e| e.name.starts_with('T') && e.name.len() > 1 && e.name.ends_with('m'))
        {
            let base = &e.name[..e.name.len() - 1];
            let (Some(c), Some(p)) = (arc.find(&format!("{base}c")), arc.find(base)) else {
                continue;
            };
            let bma = ground::parse_bma(arc.data(&data, e).unwrap()).unwrap();
            let bpc = ground::parse_bpc(arc.data(&data, c).unwrap()).unwrap();
            let bpl = ground::parse_bpl(arc.data(&data, p).unwrap()).unwrap();
            let bpa: Vec<_> = (1..=4)
                .map(|i| {
                    arc.find(&format!("{base}{i}"))
                        .map(|f| ground::parse_bpa(arc.data(&data, f).unwrap()).unwrap())
                })
                .collect();
            let backdrop = [1, 2, 3, 255];
            for layer in 0..bma.layers.len() {
                let bg = ground_layer(&bpc, &bpa, &bma, layer).unwrap();
                let a = compose(std::slice::from_ref(&bg), &bpl.palettes, backdrop);
                if a.pixels.chunks_exact(4).all(|px| px == backdrop) {
                    continue;
                }
                assert_eq!(a, compose(std::slice::from_ref(&bg), &bpl.palettes, backdrop), "{base} layer {layer}");
                rendered += 1;
            }
            if rendered > 0 {
                break;
            }
        }
        assert!(rendered > 0, "no town map layer rendered anything");
    }
}

/// Parses `floor_dump.txt` (from a runtime dump of the original's autotile pass): the
/// tileset, the fixed-room flag, and per cell the terrain word and stored metatile.
fn parse_floor_dump(text: &str) -> (u16, bool, Vec<u16>, Vec<u8>) {
    let mut lines = text.lines();
    let mut field = |name: &str| -> u32 {
        let l = lines.next().expect("dump header line");
        l.strip_prefix(name)
            .unwrap_or_else(|| panic!("expected {name}, got {l}"))
            .trim()
            .parse()
            .unwrap()
    };
    let tileset = field("tileset") as u16;
    let fixed = field("fixed_flag") != 0;
    // frame, dungeon_ptr, randint_calls_in_pass, the comment line.
    for _ in 0..4 {
        lines.next();
    }
    let (mut terrain, mut metatile) = (Vec::new(), Vec::new());
    for line in lines.filter(|l| !l.trim().is_empty()) {
        for cell in line.split_whitespace() {
            assert_eq!(cell.len(), 6, "cell {cell}");
            terrain.push(u16::from_str_radix(&cell[..4], 16).unwrap());
            metatile.push(u8::from_str_radix(&cell[4..], 16).unwrap());
        }
    }
    (tileset, fixed, terrain, metatile)
}

rescue_tests::rom_test! {
    /// Ground truth for the autotile rules: feeds the terrain of a runtime floor dump and
    /// its logged `RandInt(4)` draws (not a replay of the LCG, which VBlank interrupts
    /// disturb) to `autotile` with the tileset's `cex` table from the ROM, and requires
    /// every cell's metatile to match. Needs `RESCUE_FLOOR_DUMP` (a directory holding
    /// `floor_dump.txt` and `randint.txt`) as well as a ROM; skips without it.
    fn autotile_matches_runtime_floor_dump(rom) {
        let Some(dir) = std::env::var_os("RESCUE_FLOOR_DUMP") else {
            eprintln!("SKIPPED autotile_matches_runtime_floor_dump: RESCUE_FLOOR_DUMP not set");
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let dump = std::fs::read_to_string(dir.join("floor_dump.txt")).expect("floor_dump.txt");
        let draws: Vec<u32> = std::fs::read_to_string(dir.join("randint.txt"))
            .expect("randint.txt")
            .lines()
            .map(|l| l.trim().parse().unwrap())
            .collect();
        let (tileset, fixed, terrain, expected) = parse_floor_dump(&dump);
        assert_eq!(draws.len(), GRID_W * GRID_H);

        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let f = fs.get("dungeon.sbin").expect("dungeon.sbin present");
        let data = fs.data(&bytes, f.id).expect("file bytes").to_vec();
        let arc = Archive::parse(&data).unwrap();
        let t = tileset::Tileset::load(&arc, &data, tileset as u8).expect("tileset loads");
        let cex = t.autotile.expect("tileset has an autotile table");

        let mut next = draws.iter().copied();
        let got = autotile(&terrain, tileset, &cex, &[], fixed, || next.next().unwrap()).unwrap();
        let bad: Vec<_> = (0..got.len())
            .filter(|&i| got[i] != expected[i])
            .map(|i| (i % GRID_W, i / GRID_W))
            .collect();
        assert!(bad.is_empty(), "{} cells differ, first at (x, y) {:?}", bad.len(), &bad[..bad.len().min(5)]);
        assert!(next.next().is_none(), "unused draws");
    }
}

/// One captured ground map: the capture directory prefix and the ground.sbin base name.
const GROUND_CAPTURES: [(&str, &str, i16); 3] = [
    ("square", "T01P01", 2),
    ("free", "T01P03", 5),
    ("base", "B12P01A", 77),
];

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// BGR555 palette RAM colour to RGBA the way BPL colours are stored (`5-bit * 8 + 7`).
fn bgr555(c: u16) -> [u8; 4] {
    let ch = |v: u16| ((v & 31) as u8) * 8 + 7;
    [ch(c), ch(c >> 5), ch(c >> 10), 255]
}

/// A colour as the DS stores it: 5 bits per channel (the BPL's 8-bit values are `bits << 3`).
fn five(c: [u8; 4]) -> [u8; 3] {
    [c[0] >> 3, c[1] >> 3, c[2] >> 3]
}

/// The BPL palettes as they stand in the capture's palette RAM. Each colour keeps its base
/// value if that equals RAM, else takes the value from the animation frame of its row that
/// equals RAM (animations touch single colours, so rows are matched colour by colour); a
/// colour that no frame explains stays at its base value and shows up as a mismatch.
fn palettes_as_captured(bpl: &ground::Bpl, pal_ram: &[u8]) -> Vec<Palette> {
    let mut out = Vec::new();
    for (row, base) in bpl.palettes.iter().enumerate() {
        let anim = bpl.animations.get(row).and_then(Option::as_ref);
        let mut p = base.clone();
        for (i, slot) in p.iter_mut().enumerate().skip(1) {
            let ram = five(bgr555(le16(pal_ram, (row * 16 + i) * 2)));
            if five(*slot) == ram {
                continue;
            }
            let hit = anim.and_then(|a| {
                a.frames
                    .iter()
                    .filter_map(|f| f.get(i))
                    .find(|c| five(**c) == ram)
            });
            if let Some(c) = hit {
                *slot = *c;
            }
        }
        out.push(p);
    }
    out
}

/// Software render of engine A BG2 and BG3 from a capture's VRAM, palette RAM and I/O dump
/// (text BGs, 4bpp, 512x256). Returns the frame and a per-pixel flag: true where the front
/// pixel comes from BG2 with palette row 13 (the overlay that is not ground-map data).
fn render_captured_bg23(dir: &std::path::Path) -> (Vec<[u8; 4]>, Vec<bool>) {
    let rd = |n: &str| std::fs::read(dir.join(n)).unwrap_or_else(|e| panic!("{}: {e}", n));
    let (io, vram, pal, sh) = (
        rd("io.bin"),
        rd("vram_a_bg.bin"),
        rd("palette.bin"),
        rd("scroll_shadow_020F38A0.bin"),
    );
    let dispcnt = le32(&io, 0);
    let char_off = ((dispcnt >> 24) & 7) as usize * 0x10000;
    let screen_off = ((dispcnt >> 27) & 7) as usize * 0x10000;
    let colour = |row: usize, i: usize| bgr555(le16(&pal, (row * 16 + i) * 2));
    let backdrop = bgr555(le16(&pal, 0));
    // (priority, bg number); a smaller priority is in front, then the lower BG number.
    let mut bgs: Vec<(u16, usize)> = [2usize, 3]
        .iter()
        .map(|&n| (le16(&io, 8 + 2 * n) & 3, n))
        .collect();
    bgs.sort_by_key(|&(p, n)| std::cmp::Reverse((p, n)));
    let mut out = vec![backdrop; 256 * 192];
    let mut overlay = vec![false; 256 * 192];
    for (_, n) in bgs {
        let cnt = le16(&io, 8 + 2 * n);
        assert_eq!(cnt >> 14, 1, "BG{n} is not a 512x256 text BG");
        let chr = char_off + usize::from((cnt >> 2) & 15) * 0x4000;
        let scr = screen_off + usize::from((cnt >> 8) & 31) * 0x800;
        let hofs = i32::from(le16(&sh, 0x30 + 4 * n) as i16);
        let vofs = i32::from(le16(&sh, 0x30 + 4 * n + 2) as i16);
        for y in 0..192i32 {
            for x in 0..256i32 {
                let (bx, by) = (
                    (x + hofs).rem_euclid(512) as usize,
                    (y + vofs).rem_euclid(256) as usize,
                );
                let block = (bx / 8) / 32;
                let entry = le16(
                    &vram,
                    scr + block * 0x800 + (((by / 8) * 32 + (bx / 8) % 32) * 2),
                );
                let (tile, hf, vf, row) = (
                    usize::from(entry & 0x3FF),
                    entry & 0x400 != 0,
                    entry & 0x800 != 0,
                    usize::from(entry >> 12),
                );
                let (tx, ty) = (
                    if hf { 7 - bx % 8 } else { bx % 8 },
                    if vf { 7 - by % 8 } else { by % 8 },
                );
                let byte = vram[chr + tile * 32 + ty * 4 + tx / 2];
                let idx = usize::from(if tx % 2 == 0 { byte & 15 } else { byte >> 4 });
                if idx != 0 {
                    let at = y as usize * 256 + x as usize;
                    out[at] = colour(row, idx);
                    overlay[at] = n == 2 && row == 13;
                }
            }
        }
    }
    (out, overlay)
}

rescue_tests::rom_test! {
    /// Match-the-original check for ground (town) maps: renders each captured map from the
    /// ROM's ground assets with `ground_layer_at` + `compose` at the captured camera, and
    /// compares it with the capture's own BG2+BG3 render (rebuilt from its VRAM, no
    /// sprites). Needs `RESCUE_GROUND_CAPTURE`, a directory holding `<name>_run<k>/` dumps
    /// (square, free, base; k = 1, 2), as well as a ROM; skips without it. Pixels where the
    /// capture shows the pal-row-13 BG2 overlay are not ground-map data and are excluded.
    fn ground_maps_match_captured_bg_render(rom) {
        let Some(dir) = std::env::var_os("RESCUE_GROUND_CAPTURE") else {
            eprintln!("SKIPPED ground_maps_match_captured_bg_render: RESCUE_GROUND_CAPTURE not set");
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let f = fs.get("ground.sbin").expect("ground.sbin present");
        let data = fs.data(&bytes, f.id).expect("file bytes").to_vec();
        let arc = Archive::parse(&data).unwrap();
        let get = |name: &str| {
            let e = arc.find(name).unwrap_or_else(|| panic!("{name} not in ground.sbin"));
            arc.data(&data, e).unwrap()
        };
        for (name, base, map_id) in GROUND_CAPTURES {
            let bpl = ground::parse_bpl(get(base)).unwrap();
            let bpc = ground::parse_bpc(get(&format!("{base}c"))).unwrap();
            let bma = ground::parse_bma(get(&format!("{base}m"))).unwrap();
            let bpa: Vec<_> = (1..=4)
                .map(|i| arc.find(&format!("{base}{i}")).map(|e| ground::parse_bpa(arc.data(&data, e).unwrap()).unwrap()))
                .collect();
            for run in [1, 2] {
                let cap = dir.join(format!("{name}_run{run}"));
                let st = std::fs::read(cap.join("struct_0212913c.bin")).expect("struct dump");
                assert_eq!(le16(&st, 0x444) as i16, map_id, "{name}: map id");
                let (cam_x, cam_y) = (le32(&st, 0x478) as i32, le32(&st, 0x47C) as i32);
                let vram = std::fs::read(cap.join("vram_a_bg.bin")).expect("vram dump");
                // The BPA frame in VRAM: the one whose tiles equal the block after the BPC tiles.
                let mut frames = [0usize; 4];
                let mut at = 0x8000 + usize::from(bpc.tiles) * 32;
                for (i, &n) in bpc.bpa_tiles.iter().enumerate() {
                    let len = usize::from(n) * 32;
                    if let Some(b) = bpa.get(i).and_then(Option::as_ref) {
                        frames[i] = (0..b.frame_info.len())
                            .find(|&k| b.frame(k) == vram.get(at..at + len))
                            .unwrap_or_else(|| panic!("{name} run{run}: no BPA{} frame in VRAM", i + 1));
                    }
                    at += len;
                }
                // 1-layer maps: layer 0 on BG3. 2-layer maps: layer 0 on BG2, layer 1 on BG3.
                let place: &[(usize, u8)] = if bma.layers.len() == 1 { &[(0, 3)] } else { &[(0, 2), (1, 3)] };
                let layers: Vec<BgLayer> = place
                    .iter()
                    .map(|&(l, prio)| {
                        let mut bg = ground_layer_at(&bpc, &bpa, &bma, l, frames).unwrap();
                        bg.scroll_x = cam_x;
                        bg.scroll_y = cam_y;
                        bg.priority = prio;
                        bg
                    })
                    .collect();
                let backdrop = {
                    let pal = std::fs::read(cap.join("palette.bin")).unwrap();
                    bgr555(le16(&pal, 0))
                };
                let pal_ram = std::fs::read(cap.join("palette.bin")).unwrap();
                let palettes = palettes_as_captured(&bpl, &pal_ram);
                let mine = compose(&layers, &palettes, backdrop);
                let (want, overlay) = render_captured_bg23(&cap);
                let (mut bad, mut skipped) = (Vec::new(), 0usize);
                for i in 0..256 * 192 {
                    if overlay[i] {
                        skipped += 1;
                    } else if five(mine.get((i % 256) as u32, (i / 256) as u32)) != five(want[i]) {
                        bad.push((i % 256, i / 256));
                    }
                }
                let compared = 256 * 192 - skipped;
                let pct = 100.0 * (compared - bad.len()) as f64 / compared as f64;
                eprintln!(
                    "{name} run{run}: camera ({cam_x}, {cam_y}), BPA frames {frames:?}, overlay pixels excluded {skipped}, \
                     matched {}/{compared} ({pct:.2}%)",
                    compared - bad.len()
                );
                assert!(
                    bad.is_empty(),
                    "{name} run{run}: {} of {compared} pixels differ, first at (x, y) {:?}",
                    bad.len(),
                    &bad[..bad.len().min(5)]
                );
            }
        }
    }
}
