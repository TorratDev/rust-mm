//! ROM-gated checks of the `rescue-data` tables (#37) against the configured dump.
//! Asserts counts, cross-table consistency and a few well-known facts; the snapshot
//! records only a SHA-1 of each parsed table, never the values.

use rescue_data::{dungeon, item, moves, recruit, species, types};
use rescue_formats::archive::Archive;
use rescue_nds::binary::{overlay_data, read_overlay_tables};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_nds::rom::Sha1;

fn rom_file(rom: &[u8], path: &str) -> Vec<u8> {
    let header = Header::parse(rom).unwrap();
    let fs = Filesystem::parse(rom, &header).unwrap();
    fs.data(rom, fs.get(path).unwrap().id).unwrap().to_vec()
}

fn digest(v: &impl std::fmt::Debug) -> String {
    Sha1::of_bytes(format!("{v:?}").as_bytes()).to_string()
}

rescue_tests::rom_test! {
    fn tables_parse_and_agree(rom) {
        let bytes = rom.read();
        let sys = rom_file(&bytes, "system.sbin");
        let arc = Archive::parse(&sys).unwrap();
        let mons = species::parse_species(arc.get(&sys, "monspara").unwrap()).unwrap();
        assert_eq!(mons.len(), 424);
        assert_eq!((mons[1].name.as_str(), mons[1].types), ("Bulbasaur", [4, 8]));
        let mv = moves::parse_moves(arc.get(&sys, "wazapara").unwrap()).unwrap();
        assert_eq!((mv.moves.len(), mv.learnsets.len()), (413, 424));
        // Blue reads an empty learnset for species 420 and 421, whatever the table holds.
        assert_eq!(mv.learnsets[420], moves::Learnset::default());
        assert_eq!(mv.learnsets[421], moves::Learnset::default());
        assert!(mv.learnsets.iter().flat_map(|l| l.level_up.iter().map(|m| m.0).chain(l.tm_hm.iter().copied()))
            .all(|m| usize::from(m) < mv.moves.len()));
        let items = item::parse_items(arc.get(&sys, "itempara").unwrap()).unwrap();
        assert_eq!(items[0].name, "Nothing");
        for lang in ["f", "g", "i", "s"] {
            assert_eq!(item::parse_items(arc.get(&sys, &format!("itempar{lang}")).unwrap()).unwrap().len(), items.len());
            assert_eq!(moves::parse_moves(arc.get(&sys, &format!("wazapar{lang}")).unwrap()).unwrap().moves.len(), 413);
        }
        let mut levels = Vec::new();
        for e in arc.entries().iter().filter(|e| e.name.starts_with("lvmp")) {
            let l = species::parse_level_data(arc.data(&sys, e).unwrap()).unwrap();
            assert_eq!(l.len(), 100, "{}", e.name);
            levels.push(l);
        }
        assert_eq!(levels.len(), 421);

        let dun = rom_file(&bytes, "dungeon.sbin");
        let darc = Archive::parse(&dun).unwrap();
        let mp = dungeon::parse_mapparam(darc.get(&dun, "mapparam").unwrap(), items.len()).unwrap();
        assert_eq!((mp.dungeons.len(), mp.floor_properties.len()), (64, 1764));
        assert_eq!((mp.monster_spawns.len(), mp.trap_chances.len(), mp.item_spawns.len()), (839, 148, 178));
        // 8-byte records; each list's terminator ends where the next list starts (checked by
        // the parser, which fails otherwise).
        assert_eq!(mp.monster_spawns.iter().map(Vec::len).sum::<usize>(), 5632);
        for f in mp.dungeons.iter().flatten() {
            assert!(usize::from(f.properties) < mp.floor_properties.len());
            assert!(usize::from(f.monsters) < mp.monster_spawns.len());
            assert!(usize::from(f.traps) < mp.trap_chances.len());
            assert!(f.items.iter().all(|&i| usize::from(i) < mp.item_spawns.len()));
        }
        assert!(mp.monster_spawns.iter().flatten().all(|m| usize::from(m.species) < mons.len()));

        let header = Header::parse(&bytes).unwrap();
        let fs = Filesystem::parse(&bytes, &header).unwrap();
        let (arm9, _) = read_overlay_tables(&bytes, &header).unwrap();
        let ov1 = overlay_data(&bytes, &fs, arm9.iter().find(|o| o.id == 1).unwrap()).unwrap();
        let chart = types::type_chart(&ov1).unwrap();
        assert!(chart.iter().flatten().all(|v| (0..=3).contains(v)));
        let recruit = recruit::recruit_rate_by_level(&ov1).unwrap();
        assert!(recruit[90..].iter().all(|&r| r == 240));

        let summary = format!(
            "species {}\nmoves {}\nitems {} ({})\nlevels {}\nmapparam {}\ntype chart {}\nrecruit {}\n",
            digest(&mons), digest(&mv), items.len(), digest(&items), digest(&levels), digest(&mp), digest(&chart), digest(&recruit)
        );
        insta::assert_snapshot!(summary);
    }
}
