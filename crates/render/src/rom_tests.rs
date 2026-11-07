//! ROM-gated check of the BG renderer on a real ground map (docs/rom-tests.md). Skipped
//! without a ROM. Asserts only that the frame is non-empty and deterministic, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_formats::ground;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

use rescue_formats::tileset;

use crate::autotile::{GRID_H, GRID_W, autotile};
use crate::bg::{compose, ground_layer};

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
