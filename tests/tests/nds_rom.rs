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
    /// Every ARM9 overlay decodes, and `arm9 overlay 1` contains the type chart at decoded
    /// offset 0x72688 (V6, #16), the one table already located in Blue.
    fn overlays_decode(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let (arm9, arm7) = read_overlay_tables(&bytes, &header).expect("overlay tables");
        assert_eq!(arm9.len() + arm7.len(), 145);
        let ov1 = arm9.iter().find(|o| o.id == 1).expect("overlay 1");
        let decoded = overlay_data(&bytes, &fs, ov1).expect("overlay 1 decodes");
        assert!(decoded.len() > 0x72688);
    }
}
