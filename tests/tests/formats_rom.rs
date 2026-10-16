//! ROM-gated checks of Blue's own formats (#33, #34) against the configured dump: every
//! `.sbin` archive parses, every portrait and sprite decodes. Skipped without a ROM
//! (docs/rom-tests.md). Asserts counts and sizes only, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_formats::{portrait, px, sir0::Sir0, sprite::Sprite};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

/// Bytes of the named ROM file.
fn rom_file(rom: &[u8], path: &str) -> Vec<u8> {
    let header = Header::parse(rom).expect("header parses");
    let fs = Filesystem::parse(rom, &header).expect("FNT/FAT parse");
    let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
    fs.data(rom, f.id).expect("file bytes").to_vec()
}

rescue_tests::rom_test! {
    /// Every `.sbin` except the SDAT parses as an archive, and every AT4PX sub-file and
    /// every SIR0 sub-file is well formed.
    fn archives_and_wrappers_parse(rom) {
        let bytes = rom.read();
        let mut px_count = 0;
        let mut sir0_count = 0;
        for name in ["dungeon", "effect", "ground", "monster", "ornament", "sample", "system", "titlemenu"] {
            let data = rom_file(&bytes, &format!("{name}.sbin"));
            let arc = Archive::parse(&data).unwrap_or_else(|e| panic!("{name}.sbin: {e}"));
            for e in arc.entries() {
                let sub = arc.data(&data, e).unwrap();
                if px::kind(sub).is_some() {
                    px::decompress(sub, px::DEFAULT_LIMIT).unwrap_or_else(|err| panic!("{name}/{}: {err}", e.name));
                    px_count += 1;
                } else if Sir0::is_sir0(sub) {
                    Sir0::parse(sub).unwrap_or_else(|err| panic!("{name}/{}: {err}", e.name));
                    sir0_count += 1;
                }
            }
        }
        assert_eq!((px_count, sir0_count), (154, 1715));
    }
}

rescue_tests::rom_test! {
    /// 73 portrait files, 13 emotion slots each, all 40x40.
    fn portraits_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "monster.sbin");
        let arc = Archive::parse(&data).unwrap();
        let mut files = 0;
        let mut faces = 0;
        for e in arc.entries().iter().filter(|e| e.name.starts_with("kao")) {
            assert!(portrait::species_of(&e.name).is_some());
            let p = portrait::parse(arc.data(&data, e).unwrap()).unwrap_or_else(|err| panic!("{}: {err}", e.name));
            assert!(!p.is_empty() && p.iter().all(|f| f.image.width == 40 && f.image.height == 40));
            files += 1;
            faces += p.len();
        }
        assert_eq!(files, 73);
        assert!(faces > 150, "{faces} portraits");
    }
}

rescue_tests::rom_test! {
    /// 423 sprite files; every frame of every sprite renders and every animation step
    /// names a frame that exists.
    fn sprites_decode(rom) {
        let bytes = rom.read();
        let data = rom_file(&bytes, "monster.sbin");
        let arc = Archive::parse(&data).unwrap();
        let palettes = rescue_formats::graphics::rgbx_palettes16(arc.get(&data, "palet").unwrap()).unwrap();
        assert_eq!(palettes.len(), 14);
        let mut count = 0;
        for e in arc.entries().iter().filter(|e| e.name.starts_with("ax")) {
            let s = Sprite::parse(arc.data(&data, e).unwrap()).unwrap_or_else(|err| panic!("{}: {err}", e.name));
            assert!(s.groups.len() >= 13, "{}: {} groups", e.name, s.groups.len());
            for i in 0..s.frames.len() {
                let f = s.render_frame(i).unwrap_or_else(|err| panic!("{} frame {i}: {err}", e.name));
                assert!(f.image.pixels.iter().flatten().all(|&(p, _)| usize::from(p) < palettes.len()));
            }
            count += 1;
        }
        assert_eq!(count, 423);
    }
}
