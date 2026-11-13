//! ROM-gated check of the fixed-room file (#397): every retail entry parses, and decoding
//! exactly `width * height` codes uses exactly the bytes up to the next entry (entry 139:
//! up to the table). Spec: `docs/formats/fixedmap.md`. Skipped without a ROM
//! (`docs/rom-tests.md`).

use rescue_formats::archive::Archive;
use rescue_formats::fixedmap::{ENTRY_COUNT, FixedMap, TileStream};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

rescue_tests::rom_test! {
    /// All 140 retail entries of `fixedmap` (#397).
    fn fixedmap_entries_decode_to_their_size(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let f = fs.get("dungeon.sbin").expect("dungeon.sbin present");
        let dun = fs.data(&bytes, f.id).expect("dungeon.sbin bytes").to_vec();
        let darc = Archive::parse(&dun).expect("dungeon.sbin archive parses");
        let fixed = darc.get(&dun, "fixedmap").expect("fixedmap in dungeon.sbin");
        let map = FixedMap::parse(fixed).expect("fixedmap parses");
        assert!(map.entry(-1).is_none());
        assert!(map.entry(i16::try_from(ENTRY_COUNT).expect("fits")).is_none());

        for n in 0..ENTRY_COUNT {
            let num = i16::try_from(n).expect("fits");
            let e = map.entry(num).unwrap_or_else(|| panic!("entry {n} parses"));
            assert!(
                (5..=17).contains(&e.width) && (5..=17).contains(&e.height),
                "entry {n}: size {}x{}",
                e.width,
                e.height
            );
            assert!(e.flags <= 1, "entry {n}: flags {:#x}", e.flags);
            let mut tiles = TileStream::new(e.stream);
            for _ in 0..usize::from(e.width) * usize::from(e.height) {
                assert!(tiles.try_next().is_some(), "entry {n}: stream ends early");
            }
            if n + 1 < ENTRY_COUNT {
                assert!(tiles.is_exhausted(), "entry {n}: bytes left after its codes");
            } else {
                // Entry 139 is followed by two 0xAA alignment bytes before the table.
                let rest = tiles.rest();
                assert!(
                    tiles.is_exhausted() || (rest.len() == 2 && rest == [0xAA, 0xAA]),
                    "entry {n}: {rest:02x?} left after its codes"
                );
            }
        }
    }
}
