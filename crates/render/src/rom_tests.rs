//! ROM-gated check of the BG renderer on a real ground map (docs/rom-tests.md). Skipped
//! without a ROM. Asserts only that the frame is non-empty and deterministic, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_formats::ground;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

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
