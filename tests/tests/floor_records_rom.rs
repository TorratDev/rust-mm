//! ROM-gated check of `floor_records` (#345): dungeon 0 has four records in `mapparam`
//! (the placeholder at index 0, then floors 1 to 3), and record 2 is the one
//! `properties_for` reads for floor 2. Skipped without a ROM (`docs/rom-tests.md`).

use rescue_data::dungeon::parse_mapparam;
use rescue_data::item::parse_items;
use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::floor_gen::{properties_for, properties_from_record};
use rescue_sim::state::floor_records;

rescue_tests::rom_test! {
    /// Dungeon 0's floor records (#345): four entries, and entry 2 is floor 2's properties.
    fn floor_records_of_dungeon_0_match_properties_for(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let sys = fs.data(&bytes, fs.get("system.sbin").expect("system.sbin").id).expect("file bytes");
        let items = parse_items(Archive::parse(sys).unwrap().get(sys, "itempara").expect("itempara"))
            .expect("items parse");
        let dun = fs.data(&bytes, fs.get("dungeon.sbin").expect("dungeon.sbin").id).expect("file bytes");
        let arc = Archive::parse(dun).unwrap();
        let map = parse_mapparam(arc.get(dun, "mapparam").expect("mapparam"), items.len())
            .expect("mapparam parses");

        let records = floor_records(&map, 0).expect("dungeon 0 has a floor list");
        assert_eq!(records.len(), 4, "placeholder plus floors 1 to 3");
        let floor2 = properties_for(&map, 0, 2).expect("floor 2 has a record");
        assert_eq!(properties_from_record(&records[2]), floor2, "entry 2 is floor 2");
    }
}
