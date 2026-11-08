//! ROM-gated check of the dialogue box, portrait and marker against the post_accept capture
//! (docs/rom-tests.md). Skipped without a ROM or without `RESCUE_TEXT_CAPTURE`.

use rescue_formats::archive::Archive;
use rescue_formats::{font, portrait};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

use crate::bg::Frame;
use crate::text::{Dialogue, Font, State};
use crate::ui::{TileSet, draw_dialogue_box, draw_marker, draw_portrait, portrait_slot};

/// Pixel rectangles compared with the capture: the box, and the portrait frame and image.
const BOX: (std::ops::Range<u32>, std::ops::Range<u32>) = (16..240, 136..176);
const PORTRAIT: (std::ops::Range<u32>, std::ops::Range<u32>) = (184..240, 72..128);
/// Colour that marks a pixel the renderer did not draw.
const UNDRAWN: [u8; 4] = [1, 2, 3, 255];
/// Pixels of the portrait that may differ: the 12 dithered pixels of the mirrored image
/// (`docs/formats/portraits.md`, residual).
const PORTRAIT_RESIDUAL: usize = 12;
/// Box pixels that may differ inside the corner tiles: the transparent corner pixels, where
/// the capture shows something the notes do not record (`[hypothesis]`: the scene behind
/// them). Observed: 24, all in the top and bottom corner rows.
const CORNER_RESIDUAL: usize = 24;

/// Whether screen pixel (`x`, `y`) lies in one of the four 8x8 corner tiles of the box
/// (box corners at x 16 and 232, y 136 and 168).
fn in_box_corner(x: u32, y: u32) -> bool {
    let col_ok = (16..24).contains(&x) || (232..240).contains(&x);
    let row_ok = (136..144).contains(&y) || (168..176).contains(&y);
    col_ok && row_ok
}

rescue_tests::rom_test! {
    /// Draws the post_accept dialogue box, the Charmander portrait (position 3, mirrored) and
    /// the wait marker, and compares them with `screen_bottom.rgb` in the capture directory.
    /// The line body is `dialogue_body.bin`; the speaker prefix is inferred, as in the text
    /// test. Needs `RESCUE_TEXT_CAPTURE` (`screen_bottom.rgb`, `dialogue_body.bin`).
    fn post_accept_box_and_portrait_match_capture(rom) {
        let Some(dir) = std::env::var_os("RESCUE_TEXT_CAPTURE") else {
            eprintln!("SKIPPED post_accept_box_and_portrait_match_capture: RESCUE_TEXT_CAPTURE not set");
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
        let glyphs = Font::new(font::parse_font(sarc.get(&system, "kanji_a").unwrap()).unwrap());
        let tiles = TileSet::new(
            sarc.get(&system, "font").unwrap(),
            sarc.get(&system, "fontpal").unwrap(),
        )
        .unwrap();
        let sprites = TileSet::new(
            sarc.get(&system, "fontsp").unwrap(),
            sarc.get(&system, "fontsppa").unwrap(),
        )
        .unwrap();
        let monster = load("monster.sbin");
        let marc = Archive::parse(&monster).unwrap();
        let kao = portrait::parse(marc.get(&monster, "kao004").unwrap()).unwrap();
        let charmander = kao
            .into_iter()
            .find(|p| p.emotion == 0)
            .expect("kao004 has emotion 0");

        // The line: the inferred speaker prefix, then the body read from game RAM.
        let body = std::fs::read(cap.join("dialogue_body.bin")).expect("dialogue_body.bin");
        let mut line = b"#C6Charmander#R:".to_vec();
        line.extend_from_slice(&body);
        let tokens = rescue_formats::text::decode(&line);
        let mut d = Dialogue::new(&tokens, &glyphs);
        d.tick(false, true);
        assert!(matches!(d.state(), State::Done), "state {:?}", d.state());

        // Draw the frame in the order the screen does: box, portrait, marker.
        let mut frame = Frame::filled(UNDRAWN);
        draw_dialogue_box(&mut frame, &tiles, d.canvas());
        let slot = portrait_slot(3).expect("position 3");
        draw_portrait(&mut frame, &tiles, &charmander, slot);
        draw_marker(&mut frame, &sprites, 120, 171);

        let shot = std::fs::read(cap.join("screen_bottom.rgb")).expect("screen_bottom.rgb");
        assert_eq!(shot.len(), 256 * 192 * 3, "screen_bottom.rgb is 256x192 RGB");
        // Compare after dropping the low 3 bits of each channel, with one step of slack
        // (the capture's colours are 5-bit values widened to 8 bits).
        // Returns (drawn, differing outside the corners, differing in the corners).
        let compare = |(xs, ys): &(std::ops::Range<u32>, std::ops::Range<u32>)| {
            let (mut drawn, mut bad, mut corner_bad) = (0usize, 0usize, 0usize);
            for y in ys.clone() {
                for x in xs.clone() {
                    let ours = frame.get(x, y);
                    if ours == UNDRAWN {
                        continue;
                    }
                    drawn += 1;
                    let i = (y as usize * 256 + x as usize) * 3;
                    let theirs = &shot[i..i + 3];
                    let same = (0..3).all(|k| (ours[k] >> 3).abs_diff(theirs[k] >> 3) <= 1);
                    if !same {
                        if in_box_corner(x, y) {
                            corner_bad += 1;
                        } else {
                            bad += 1;
                        }
                        eprintln!("differs at ({x},{y}): ours {ours:?} capture {theirs:?}");
                    }
                }
            }
            (drawn, bad, corner_bad)
        };
        let (box_drawn, box_bad, box_corner_bad) = compare(&BOX);
        let (por_drawn, por_bad, _) = compare(&PORTRAIT);
        eprintln!(
            "box: {box_bad} outside the corners and {box_corner_bad} in them, of {box_drawn} drawn pixels; portrait: {por_bad} of {por_drawn}"
        );
        assert!(box_drawn > 0 && por_drawn > 0, "nothing was drawn");
        assert_eq!(box_bad, 0, "dialogue box differs from the capture outside the corners");
        assert!(
            box_corner_bad <= CORNER_RESIDUAL,
            "box corners differ in {box_corner_bad} pixels (residual is {CORNER_RESIDUAL})"
        );
        assert!(
            por_bad <= PORTRAIT_RESIDUAL,
            "portrait differs in {por_bad} pixels (residual is {PORTRAIT_RESIDUAL})"
        );
    }
}
