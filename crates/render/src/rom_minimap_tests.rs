//! ROM-gated check of the minimap rule against the dungeon captures (docs/rom-tests.md).
//! Skipped without a ROM or without `RESCUE_MINIMAP_CAPTURE`.
//!
//! `RESCUE_MINIMAP_CAPTURE` is the directory of per-dump capture folders (`dng1_run1`,
//! `w4_run1`, `w4xy4_run1`). Each holds `cells.json` (the floor cells and the switches read
//! at the time) and `canvas_15FC4.bin` (the minimap canvas, 4bpp, game order). Nothing from
//! the captures is committed.
//!
//! What is not checked. `cells.json` has no monster info fields, no trap ids and no object
//! flags, and no monster visibility. A quadrant of a cell that holds a monster (`mon_type` 1)
//! is therefore left out of the comparison. Also not checked here: terrain 2 and 3, flags 1
//! and 2 alone during play, the trap ids other than the observed 17, and layout options 4
//! and 6 with an object. The dungeon predicate `FUN_0206db90` is taken as 0 (the findings'
//! dng1 runtime value); the captures do not record it.

use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

use crate::minimap::{
    CANVAS_BYTES, CANVAS_COLS, COLS, Cell, Object, Patterns, ROWS, Settings, render,
};

/// The captures checked. Each is a directory of `cells.json` and `canvas_15FC4.bin`.
const RUNS: [&str; 3] = ["dng1_run1", "w4_run1", "w4xy4_run1"];

/// The value of the scalar `"key": N` (or `null`) after `from` in `text`.
fn scalar(text: &str, key: &str) -> Option<Option<i64>> {
    let pat = format!("\"{key}\": ");
    let at = text.find(&pat)? + pat.len();
    let rest = &text[at..];
    if rest.starts_with("null") {
        return Some(None);
    }
    let end = rest
        .find(|c: char| !(c == '-' || c.is_ascii_digit()))
        .unwrap_or(rest.len());
    rest[..end].parse().ok().map(Some)
}

/// The switches object: `"switches_15fa0": {"0": 3, ...}`.
fn switches(text: &str) -> Option<Vec<(u8, i64)>> {
    let start = text.find("\"switches_15fa0\": {")? + "\"switches_15fa0\": {".len();
    let body = &text[start..start + text[start..].find('}')?];
    body.split(',')
        .map(|pair| {
            let (k, v) = pair.split_once(':')?;
            let k = k.trim().trim_matches('"');
            let idx = u8::from_str_radix(k, 16).ok()?;
            Some((idx, v.trim().parse().ok()?))
        })
        .collect()
}

/// One captured cell from `cells.json`.
struct CapturedCell {
    x: i64,
    y: i64,
    cell: Cell,
    /// A monster is present: its info is not captured, so the quadrant is not compared.
    has_monster: bool,
}

/// Parses the `cells` array: each object is `{"x": .., "y": .., "terrain": .., ...}`.
fn captured_cells(text: &str) -> Option<Vec<CapturedCell>> {
    let start = text.find("\"cells\": [")?;
    text[start..]
        .split("{\"x\": ")
        .skip(1)
        .map(|piece| {
            let piece = format!("{{\"x\": {piece}");
            let x = scalar(&piece, "x")??;
            let y = scalar(&piece, "y")??;
            let terrain = u16::try_from(scalar(&piece, "terrain")??).ok()?;
            let flags = u16::try_from(scalar(&piece, "flags")??).ok()?;
            let obj_kind = scalar(&piece, "obj_kind")?;
            let mon_type = scalar(&piece, "mon_type")?;
            Some(CapturedCell {
                x,
                y,
                cell: Cell {
                    terrain,
                    flags,
                    monster: None,
                    object: obj_kind.map(|k| Object {
                        kind: k as u8,
                        trap_id: 0,
                        flag20: false,
                    }),
                },
                has_monster: mon_type.is_some(),
            })
        })
        .collect()
}

rescue_tests::rom_test! {
    /// Renders each capture's minimap with `zmappat` and compares each cell's canvas quadrant
    /// with `canvas_15FC4.bin`. The layout and switches come from the capture. Needs
    /// `RESCUE_MINIMAP_CAPTURE`, the directory holding the runs.
    fn captured_minimaps_match_the_cell_rule(rom) {
        let Some(dir) = std::env::var_os("RESCUE_MINIMAP_CAPTURE") else {
            eprintln!("SKIPPED captured_minimaps_match_the_cell_rule: RESCUE_MINIMAP_CAPTURE not set");
            return;
        };
        let base = std::path::PathBuf::from(dir);
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let load = |path: &str| {
            let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
            fs.data(&bytes, f.id).expect("file bytes").to_vec()
        };
        let dungeon = load("dungeon.sbin");
        let darc = Archive::parse(&dungeon).unwrap();
        let patterns = Patterns::decode(darc.get(&dungeon, "zmappat").unwrap()).unwrap();

        for run in RUNS {
            let dir = base.join(run);
            let Ok(text) = std::fs::read_to_string(dir.join("cells.json")) else {
                eprintln!("SKIPPED {run}: no cells.json");
                continue;
            };
            let want = std::fs::read(dir.join("canvas_15FC4.bin")).expect("canvas_15FC4.bin");
            assert_eq!(want.len(), CANVAS_BYTES, "{run}: canvas size");
            let layout = scalar(&text, "layout_t").flatten().expect("layout_t") as u8;
            let blank = scalar(&text, "d11308").flatten().unwrap_or(0) != 0;
            let sw = switches(&text).expect("switches");
            let on = |idx: u8| sw.iter().any(|&(k, v)| k == idx && v != 0);
            let settings = Settings {
                layout,
                // FUN_0206db90 is 0 in the dng1 runtime (findings, #313 P1.a).
                dungeon_special: false,
                show_monsters: true,
                leader_white: on(0x0C),
                no_entities: on(0x02),
                known_all: on(0x03),
                items_visible: on(0x06),
                traps_visible: on(0x07),
                marked: on(0x08),
                stairs_unknown: on(0x09),
                blank,
                enabled: true,
            };
            let captured = captured_cells(&text).expect("cells");
            let mut grid = vec![Cell { terrain: 0, flags: 0, monster: None, object: None }; COLS * ROWS];
            for c in &captured {
                let slot = usize::try_from(c.y)
                    .ok()
                    .zip(usize::try_from(c.x).ok())
                    .and_then(|(y, x)| grid.get_mut(y * COLS + x));
                if let Some(slot) = slot {
                    *slot = c.cell;
                }
            }
            let got = render(&patterns, &grid, &settings).canvas;

            // Compare each cell's quadrant: 4 rows of 2 bytes in its tile.
            let local = i64::from(layout == 6);
            let (mut checked, mut skipped, mut bad) = (0usize, 0usize, 0usize);
            for c in &captured {
                let ry = c.y - 2 + local;
                if !(0..COLS as i64).contains(&c.x) || ry < 0 {
                    continue;
                }
                if c.has_monster {
                    skipped += 1;
                    continue;
                }
                let (tile_col, tile_row) = ((c.x / 2) as usize, (ry / 2) as usize);
                let (qx, qy) = ((c.x & 1) as usize, (ry & 1) as usize);
                let tile = (tile_row * CANVAS_COLS + tile_col) * 32;
                for r in 0..4 {
                    let at = tile + (qy * 4 + r) * 4 + qx * 2;
                    checked += 1;
                    if got[at..at + 2] != want[at..at + 2] {
                        bad += 1;
                        eprintln!("{run}: cell ({},{}) row {r}: ours {:02x?} capture {:02x?}",
                            c.x, c.y, &got[at..at + 2], &want[at..at + 2]);
                    }
                }
            }
            eprintln!("{run}: {checked} quadrant rows checked, {skipped} cells with monsters skipped, {bad} differ");
            assert_eq!(bad, 0, "{run}: minimap differs from the capture");
        }
    }
}
