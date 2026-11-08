//! ROM-gated check of the lower-screen chrome against the dng1 dungeon capture
//! (docs/rom-tests.md). Skipped without a ROM or without `RESCUE_DUNGEON_CAPTURE`.

use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

use crate::bg::Frame;
use crate::lower::{BAR_TILES, LowerTiles, Status, draw_lower, hp_bar, panel_cells, status_row};
use crate::text::Canvas;
use crate::ui::TEXT_ORIGIN;

/// Canvas size in tiles (BG0 cols 3 to 28, rows 17 to 21).
const CANVAS_TILES: (usize, usize) = (26, 5);
/// BG0 map base in VRAM (screen base +0x10000) and the BG char base (0).
const MAP_BASE: usize = 0x10000;
const TILE_BYTES: usize = 32;

/// Decodes one 4bpp tile (low nibble left) to palette indices.
fn tile_indices(tile: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    for (i, &b) in tile.iter().take(32).enumerate() {
        out[i * 2] = b & 15;
        out[i * 2 + 1] = b >> 4;
    }
    out
}

rescue_tests::rom_test! {
    /// Checks the dng1 lower screen: the status row and the touch-button panel entries cell by
    /// cell against the capture's BG0 map, the HP bar tiles against the captured bar tiles, and
    /// the message log's text pixels against `screen_bottom.rgb`. The canvas is the captured
    /// text tiles 0x002 to 0x083. Needs `RESCUE_DUNGEON_CAPTURE`, the dng1 capture directory
    /// (`vram_a_bg.bin`, `screen_bottom.rgb`).
    fn dng1_lower_screen_matches_capture(rom) {
        let Some(dir) = std::env::var_os("RESCUE_DUNGEON_CAPTURE") else {
            eprintln!("SKIPPED dng1_lower_screen_matches_capture: RESCUE_DUNGEON_CAPTURE not set");
            return;
        };
        let cap = std::path::PathBuf::from(dir);
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let load = |path: &str| {
            let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
            fs.data(&bytes, f.id).expect("file bytes").to_vec()
        };
        let system = load("system.sbin");
        let sarc = Archive::parse(&system).unwrap();
        let tiles = LowerTiles::new(
            sarc.get(&system, "font").unwrap(),
            sarc.get(&system, "fontpal").unwrap(),
            sarc.get(&system, "panelnn").unwrap(),
        )
        .unwrap();

        let vram = std::fs::read(cap.join("vram_a_bg.bin")).expect("vram_a_bg.bin");
        let map_entry = |row: usize, col: usize| {
            let i = MAP_BASE + (row * 32 + col) * 2;
            u16::from_le_bytes([vram[i], vram[i + 1]])
        };

        // The dng1 status: B1F, level 8, HP 34 of 34 (docs/re/ui-screens.md, status line).
        let status = Status { basement: true, floor: 1, level: 8, hp: 34, max_hp: 34, flag: true };
        let want_row: Vec<u16> = (0..32).map(|c| map_entry(0, c)).collect();
        let got_row = status_row(&status);
        assert_eq!(got_row.to_vec(), want_row, "status row differs from the BG0 map");

        // The touch-button panel, rows 22 and 23: every column.
        let mut got_panel = [[0u16; 32]; 2];
        for (col, row, entry) in panel_cells() {
            if let Some(c) = got_panel
                .get_mut(row as usize - 22)
                .and_then(|r| r.get_mut(col as usize))
            {
                *c = entry;
            }
        }
        for (k, row) in [22usize, 23].into_iter().enumerate() {
            let want: Vec<u16> = (0..32).map(|c| map_entry(row, c)).collect();
            assert_eq!(got_panel[k].to_vec(), want, "panel row {row} differs from the BG0 map");
        }

        // The HP bar tiles at BG tile 0x2CC: VRAM char base 0, 32 bytes per tile.
        let bar = hp_bar(34, 34, true);
        for (i, tile) in bar.iter().enumerate().take(BAR_TILES) {
            let at = (0x2CC + i) * TILE_BYTES;
            let want = tile_indices(&vram[at..at + TILE_BYTES]);
            assert_eq!(tile.to_vec(), want, "HP bar tile {i} differs from VRAM");
        }

        // The message log canvas: captured text tiles 0x002 onward, 26 by 5.
        let mut canvas = Canvas::window();
        for t in 0..CANVAS_TILES.0 * CANVAS_TILES.1 {
            let at = (2 + t) * TILE_BYTES;
            let indices = tile_indices(&vram[at..at + TILE_BYTES]);
            for y in 0..8 {
                for x in 0..8 {
                    let (tx, ty) = (t % CANVAS_TILES.0, t / CANVAS_TILES.0);
                    canvas.pixels[(ty * 8 + y) * canvas.width + tx * 8 + x] = indices[y * 8 + x];
                }
            }
        }

        // Draw over the captured screen. The log box fill is already blended in that capture, and
        // a blend cannot be undone, so the fill is not compared here: `blend_log_fill` is checked
        // only by its unit test, against the doc's captured values. Only text pixels are compared.
        let shot = std::fs::read(cap.join("screen_bottom.rgb")).expect("screen_bottom.rgb");
        assert_eq!(shot.len(), 256 * 192 * 3, "screen_bottom.rgb is 256x192 RGB");
        let mut frame = Frame::filled([0, 0, 0, 255]);
        for (i, px) in frame.pixels.chunks_exact_mut(4).enumerate() {
            px[0..3].copy_from_slice(&shot[i * 3..i * 3 + 3]);
        }
        draw_lower(&mut frame, &tiles, &status, &canvas);

        // The message text: every non-empty canvas pixel must match the capture.
        let (mut drawn, mut bad) = (0usize, 0usize);
        for y in 0..canvas.height {
            for x in 0..canvas.width {
                if canvas.get(x, y) == 0 {
                    continue;
                }
                let (sx, sy) = (TEXT_ORIGIN.0 as u32 + x as u32, TEXT_ORIGIN.1 as u32 + y as u32);
                let ours = frame.get(sx, sy);
                let i = (sy as usize * 256 + sx as usize) * 3;
                let theirs = &shot[i..i + 3];
                drawn += 1;
                if !(0..3).all(|k| (ours[k] >> 3).abs_diff(theirs[k] >> 3) <= 1) {
                    bad += 1;
                    eprintln!("text pixel ({sx},{sy}): ours {ours:?} capture {theirs:?}");
                }
            }
        }
        eprintln!("log text: {bad} of {drawn} text pixels differ");
        assert!(drawn > 0, "the capture has no text pixels");
        assert_eq!(bad, 0, "message log text differs from the capture");
    }
}
