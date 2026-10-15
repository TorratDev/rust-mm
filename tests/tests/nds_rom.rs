//! ROM-gated checks of the NDS container against the configured dump (#28–#30).
//! Counts come from `docs/file-survey.md`. Skipped unless a supported dump is configured.

use rescue_nds::binary::{overlay_data, read_overlay_tables};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

rescue_tests::rom_test! {
    /// The header parses, its CRC checks out, and the filesystem lists the 24 named files
    /// over 169 FAT entries (145 overlays plus 24 named files).
    fn container_matches_survey(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header of a supported dump parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        assert_eq!(fs.files().len(), 24);
        assert_eq!(fs.fat_len(), 169);
    }
}

rescue_tests::rom_test! {
    /// Every overlay decodes (145 in total), and `arm9 overlay 1` holds the type chart at
    /// decoded offset 0x72688 (V6, #16): 18×18 `s16` cells, all in 0..=3 in this dump.
    fn overlays_decode(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let (arm9, arm7) = read_overlay_tables(&bytes, &header).expect("overlay tables");
        assert_eq!(arm9.len() + arm7.len(), 145);
        for ov in arm9.iter().chain(arm7.iter()) {
            overlay_data(&bytes, &fs, ov)
                .unwrap_or_else(|e| panic!("overlay {} failed: {e}", ov.id));
        }
        let ov1 = arm9.iter().find(|o| o.id == 1).expect("overlay 1");
        let decoded = overlay_data(&bytes, &fs, ov1).expect("overlay 1 decodes");
        let chart = &decoded[0x72688..0x72688 + 18 * 18 * 2];
        for cell in chart.chunks_exact(2) {
            let v = i16::from_le_bytes([cell[0], cell[1]]);
            assert!((0..=3).contains(&v), "type chart cell {v} at {cell:?}");
        }
    }
}
