//! ROM-gated check of the text renderer against a captured dialogue (docs/rom-tests.md).
//! Skipped without a ROM or without `RESCUE_TEXT_CAPTURE`.

use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

/// Canvas pixel rows that hold the box's top edge (0 to 2) and bottom edge (37 to 39):
/// the frame is baked into the canvas tiles, and the window frame is not drawn by the
/// text renderer, so these rows are excluded from the comparison.
const EDGE_ROWS: [std::ops::Range<usize>; 2] = [0..3, 37..40];

rescue_tests::rom_test! {
    /// Renders the post_accept dialogue line ("Charmander: See? There were jobs listed,
    /// right?") from `message_e.str` with `kanji_a`, and compares the text canvas with the
    /// captured BG0 canvas tiles 0x002 to 0x083 (26x5 tiles, BG char base 0). Needs
    /// `RESCUE_TEXT_CAPTURE`, the `post_accept` capture directory (`vram_a_bg.bin`,
    /// `palette.bin`).
    fn post_accept_text_matches_capture(rom) {
        use crate::text::{Canvas, Dialogue, Font, State};
        use rescue_formats::{font, graphics, text};

        let Some(dir) = std::env::var_os("RESCUE_TEXT_CAPTURE") else {
            eprintln!("SKIPPED post_accept_text_matches_capture: RESCUE_TEXT_CAPTURE not set");
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
        let str_file = load("message_e.str");
        let strings = text::parse_str(&str_file).unwrap();
        let system = load("system.sbin");
        let sarc = Archive::parse(&system).unwrap();
        let font = Font::new(font::parse_font(sarc.get(&system, "kanji_a").unwrap()).unwrap());

        // The line as the script engine draws it. The body (" See?\nThere were jobs listed~2c
        // right?", with its leading space) was read from the game's RAM in the savestate; it
        // is not in message_e.str or message_e.bin. The speaker prefix "#C6Charmander#R:" is
        // inferred from the pixels (yellow name, white colon): the script's format is not
        // decoded.
        let line: &[u8] = b"#C6Charmander#R:  See?\nThere were jobs listed~2c right?";
        let _ = &strings;

        // The text palette: fontpal bank 0 is BG palette 15 in the capture.
        let fontpal = graphics::rgbx_palettes16(sarc.get(&system, "fontpal").unwrap()).unwrap();
        let cap_pal = std::fs::read(cap.join("palette.bin")).expect("palette.bin");
        for i in 0..16 {
            let v = u16::from_le_bytes([cap_pal[15 * 32 + i * 2], cap_pal[15 * 32 + i * 2 + 1]]);
            let c5 = [(v & 31) as u8, ((v >> 5) & 31) as u8, ((v >> 10) & 31) as u8];
            let rom = fontpal[0][i];
            assert!(
                (0..3).all(|k| c5[k].abs_diff(rom[k] >> 3) <= 1),
                "fontpal bank 0 colour {i} differs from BG palette 15"
            );
        }

        // The captured canvas: tiles 0x002.. of BG char base 0, 4bpp.
        let vram = std::fs::read(cap.join("vram_a_bg.bin")).expect("vram_a_bg.bin");
        let mut want = Canvas::window();
        for t in 0..26 * 5 {
            let tile = &vram[(2 + t) * 32..(3 + t) * 32];
            for y in 0..8 {
                for x in 0..8 {
                    let b = tile[y * 4 + x / 2];
                    let v = if x % 2 == 1 { b >> 4 } else { b & 15 };
                    want.pixels[((t / 26) * 8 + y) * want.width + (t % 26) * 8 + x] = v;
                }
            }
        }

        let tokens = text::decode(line);
        let mut d = Dialogue::new(&tokens, &font);
        // Type the page: fast-forward until the dialogue is done or waits.
        d.tick(false, true);
        assert!(matches!(d.state(), State::Done), "state {:?}", d.state());
        let got = d.canvas();
        let bad = (0..got.height)
            .filter(|y| !EDGE_ROWS.iter().any(|r| r.contains(y)))
            .flat_map(|y| (0..got.width).map(move |x| (x, y)))
            .filter(|&(x, y)| got.get(x, y) != want.get(x, y))
            .count();
        let drawn = want.pixels.iter().filter(|&&p| p != 0 && p < 8).count();
        eprintln!("{bad} of {} canvas pixels differ ({drawn} text pixels in the capture)", 208 * 34);
        assert_eq!(bad, 0, "text canvas differs from the capture");
        // The text is on rows 3..37 only: the edge rows hold the baked frame.
        assert!(got.pixels[..3 * 208].iter().all(|&p| p == 0));
    }
}
