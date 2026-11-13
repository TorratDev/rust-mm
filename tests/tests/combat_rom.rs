//! ROM-gated checks of the damage and hit tables (`rescue_data::combat`, #419) against the
//! configured dump. They check the shape of each table and the facts `docs/re/combat.md`
//! records; no table value is written into the repo.

use rescue_data::combat::{self, CombatTables};
use rescue_data::moves;
use rescue_formats::archive::Archive;
use rescue_nds::binary::{binary_bytes, overlay_data, read_overlay_tables};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

fn rom_file(rom: &[u8], path: &str) -> Vec<u8> {
    let header = Header::parse(rom).unwrap();
    let fs = Filesystem::parse(rom, &header).unwrap();
    fs.data(rom, fs.get(path).unwrap().id).unwrap().to_vec()
}

rescue_tests::rom_test! {
    fn combat_tables_load_with_the_documented_shape(rom) {
        let bytes = rom.read();
        let header = Header::parse(&bytes).unwrap();
        let fs = Filesystem::parse(&bytes, &header).unwrap();
        let (arm9_overlays, _) = read_overlay_tables(&bytes, &header).unwrap();
        let ov1_entry = arm9_overlays.iter().find(|o| o.id == 1).unwrap();
        // The load addresses the loader assumes.
        assert_eq!(header.arm9.ram_addr, combat::ARM9_RAM);
        assert_eq!(ov1_entry.ram_addr, combat::OVERLAY1_RAM);

        let ov1 = overlay_data(&bytes, &fs, ov1_entry).unwrap();
        // The ARM9 binary is BLZ-compressed; its RAM image (0x02000000 on) holds the physical flags.
        let arm9 = rescue_nds::blz::decompress(binary_bytes(&bytes, &header.arm9).unwrap()).unwrap();
        let t = CombatTables::load(&ov1, &arm9).unwrap();

        // Physical flags are 0 or 1, and Normal (type 1) is physical (P2).
        assert!(t.physical.iter().all(|&p| p <= 1));
        assert_eq!(t.physical[1], 1);
        // Chart cells and labels are 0 to 3.
        assert!(t.type_chart.iter().flatten().all(|c| (0..=3).contains(c)));
        assert!(t.effectiveness.iter().flatten().all(|l| (0..=3).contains(l)));
        // The pool orders resisted, neutral and super (cells 0, 2 and 3 are read this way).
        assert!(t.cell_multiplier[0] < t.cell_multiplier[2]);
        assert!(t.cell_multiplier[2] < t.cell_multiplier[3]);
        assert_eq!(t.cell_multiplier[2], 0x10000);
        // The stage tables rise (attack, defence, accuracy) or fall (evasion), and neutral (10)
        // is 1.0 (256 in the integer tables) (P4, P7).
        assert!(t.attack_stage.windows(2).all(|w| w[0] <= w[1]));
        assert!(t.defence_stage.windows(2).all(|w| w[0] <= w[1]));
        assert!(t.accuracy_stage.windows(2).all(|w| w[0] <= w[1]));
        assert!(t.evasion_stage.windows(2).all(|w| w[0] >= w[1]));
        assert_eq!(t.attack_stage[10], 0x100);
        assert_eq!(t.defence_stage[10], 0x100);
        assert_eq!(t.accuracy_stage[10], 256);
        assert_eq!(t.evasion_stage[10], 256);

        // The two damaging moves of the sim: Tackle (0x9A) and Scratch (0x104) target a foe on
        // the facing tile, class 0 area 0, as the notes record for the direct moves (#357 P3).
        let sys = rom_file(&bytes, "system.sbin");
        let arc = Archive::parse(&sys).unwrap();
        let mv = moves::parse_moves(arc.get(&sys, "wazapara").unwrap()).unwrap();
        assert_eq!(mv.moves[0x9A].name, "Tackle");
        assert_eq!(mv.moves[0x104].name, "Scratch");
        for id in [0x9A, 0x104] {
            assert_eq!(mv.moves[id].targeting[0] & 0xFF, 0, "{}", mv.moves[id].name);
        }
    }
}
