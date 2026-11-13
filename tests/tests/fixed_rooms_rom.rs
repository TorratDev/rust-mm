//! ROM-gated whole-floor checks of the fixed rooms 1 to 66 (#342, P5): with the
//! fixed-room number forced at `GenerateFloor`'s entry, `generate_floor` with the
//! ported steps must leave the same stairs, rooms, terrain counts, tiles and RNG
//! state as the retail game at `GenerateFloor`'s return.
//!
//! The rows come from the probe that writes dungeon `+0x34B4` at `GenerateFloor`'s
//! entry (`docs/re/fixed-rooms.md`, "Oracle rows"). The tile hash excludes spawns,
//! so these rows do not check the spawn RNG. Skipped without a ROM (`docs/rom-tests.md`).

use rescue_formats::archive::Archive;
use rescue_formats::fixedmap::FixedMap;
use rescue_nds::binary::{overlay_data, read_overlay_tables};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::floor::{FIXED_TILE_ENTRY_SIZE, Floor, HEIGHT, NO_ROOM, WIDTH, layout_fnv1a};
use rescue_sim::floor_gen::{FloorProperties, GenState, PortedSteps, generate_floor};
use rescue_sim::rng::DungeonRng;

/// One retail floor at `GenerateFloor`'s return, for a forced fixed-room number.
struct FixedOracleFloor {
    /// The fixed-room number forced at `GenerateFloor`'s entry (dungeon `+0x34B4`).
    number: i16,
    /// Dungeon RNG state at `GenerateFloor`'s entry.
    entry: u32,
    /// Dungeon RNG steps from `GenerateFloor`'s entry to its return.
    steps: u32,
    /// [`layout_fnv1a`] of the tiles at the return.
    layout_fnv1a: u64,
    /// Stairs pair B (dungeon `+0xC0BC`); `(-1, -1)` when none is set.
    stairs: (i32, i32),
    /// Distinct room numbers other than `0xFF`.
    rooms: usize,
    /// Tiles of terrain type 0, 1, 2 and 3.
    terrain: (usize, usize, usize, usize),
}

/// Layout byte of the forced floors (layout 1, as the probe ran it).
const FIXED_LAYOUT: u8 = 1;

/// Tile-code table `021AC208`: 220 entries of 16 bytes (`PortedSteps::fixed_table`).
const FIXED_TABLE_ADDR: u32 = 0x021A_C208;
const FIXED_TABLE_ENTRIES: usize = 220;

/// The first floor of a new game as the whole-floor oracle test sets it
/// (`crates/sim/src/floor.rs`, `first_floor_props`).
fn first_floor_props() -> FloorProperties {
    FloorProperties {
        layout: FIXED_LAYOUT,
        room_density: 5,
        byte_05: 15,
        byte_06: 4,
        byte_0f: 2,
        byte_13: 5,
        ..Default::default()
    }
}

/// The bytes of `dungeon.sbin`, the archive that holds `fixedmap` and `mapparam`.
fn dungeon_file(rom: &[u8], fs: &Filesystem) -> Vec<u8> {
    let f = fs.get("dungeon.sbin").expect("dungeon.sbin present");
    fs.data(rom, f.id).expect("dungeon.sbin bytes").to_vec()
}

/// The tile-code table at `FIXED_TABLE_ADDR`, read from the overlay whose RAM range
/// holds it.
fn fixed_table(rom: &[u8], header: &Header, fs: &Filesystem) -> Vec<u8> {
    let (arm9, _) = read_overlay_tables(rom, header).expect("ARM9 overlay table");
    let len = FIXED_TABLE_ENTRIES * FIXED_TILE_ENTRY_SIZE;
    for ov in &arm9 {
        let end = ov.ram_addr.saturating_add(ov.ram_size);
        if (ov.ram_addr..end).contains(&FIXED_TABLE_ADDR) {
            let data = overlay_data(rom, fs, ov).expect("overlay bytes");
            let at = usize::try_from(FIXED_TABLE_ADDR - ov.ram_addr).expect("offset fits");
            return data
                .get(at..at + len)
                .expect("table inside the overlay")
                .to_vec();
        }
    }
    panic!("no ARM9 overlay holds {FIXED_TABLE_ADDR:#x}");
}

/// Runs one forced floor and returns what differs from the oracle row, one line per
/// field, with the port's value and the oracle's.
fn check_floor(want: &FixedOracleFloor, fixed_map: &[u8], table: &[u8]) -> Vec<String> {
    let props = first_floor_props();
    let mut floor = Floor::new();
    let mut rng = DungeonRng::from_state(want.entry);
    let mut g = GenState {
        fixed_room: want.number,
        ..Default::default()
    };
    let mut steps = PortedSteps {
        fixed_map: fixed_map.to_vec(),
        fixed_table: table.to_vec(),
        ..Default::default()
    };
    generate_floor(
        &props, false, &mut floor, &mut rng, &mut g, &mut steps, false,
    )
    .expect("runs to the end");

    let mut end = want.entry;
    for _ in 0..want.steps {
        end = rescue_sim::rng::lcg_step(end);
    }
    let mut rooms = std::collections::BTreeSet::new();
    let mut terrain = [0usize; 4];
    for y in 0..HEIGHT as i32 {
        for x in 0..WIDTH as i32 {
            let tile = floor.tile(x, y).expect("in-map tile");
            terrain[usize::from(tile.terrain & 3)] += 1;
            if tile.room != NO_ROOM {
                rooms.insert(tile.room);
            }
        }
    }
    let got_stairs = (i32::from(g.stairs_b.0), i32::from(g.stairs_b.1));
    let want_terrain = [
        want.terrain.0,
        want.terrain.1,
        want.terrain.2,
        want.terrain.3,
    ];
    let mut bad = Vec::new();
    let mut field = |name: &str, port: String, oracle: String, same: bool| {
        if !same {
            bad.push(format!(
                "number {} entry {:#010x}: {name}: port {port}, oracle {oracle}",
                want.number, want.entry
            ));
        }
    };
    field(
        "RNG state",
        format!("{:#010x}", rng.state()),
        format!("{end:#010x}"),
        rng.state() == end,
    );
    field(
        "layout_fnv1a",
        format!("{:#018x}", layout_fnv1a(&floor)),
        format!("{:#018x}", want.layout_fnv1a),
        layout_fnv1a(&floor) == want.layout_fnv1a,
    );
    field(
        "stairs",
        format!("{got_stairs:?}"),
        format!("{:?}", want.stairs),
        got_stairs == want.stairs,
    );
    field(
        "rooms",
        rooms.len().to_string(),
        want.rooms.to_string(),
        rooms.len() == want.rooms,
    );
    field(
        "terrain",
        format!("{terrain:?}"),
        format!("{want_terrain:?}"),
        terrain == want_terrain,
    );
    bad
}

/// The retail rows: copied from the probe output by `cat`, not typed. Numbers 1 to 49
/// have one row (the same for every seed); numbers 50 to 66 have one row per seed
/// (`0x12345678`, `0xCAFEBABF`, `0x0BADF00D`).
#[rustfmt::skip]
const FIXED_ORACLE_FLOORS: &[FixedOracleFloor] = &[
        FixedOracleFloor { number: 1, entry: 0x0BAD_F00D, steps: 4, layout_fnv1a: 0xb2d0_8f29_d84e_66a9, stairs: (-1, -1), rooms: 1, terrain: (1698, 53, 0, 41) },
        FixedOracleFloor { number: 2, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x910e_b848_f667_48aa, stairs: (-1, -1), rooms: 1, terrain: (1712, 80, 0, 0) },
        FixedOracleFloor { number: 3, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0xf8fa_80ff_7fee_83f9, stairs: (-1, -1), rooms: 1, terrain: (11, 77, 0, 1704) },
        FixedOracleFloor { number: 4, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0xe6be_8f47_953f_4176, stairs: (-1, -1), rooms: 1, terrain: (90, 129, 0, 1573) },
        FixedOracleFloor { number: 5, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x2468_2a35_2971_3c95, stairs: (-1, -1), rooms: 1, terrain: (0, 84, 0, 1708) },
        FixedOracleFloor { number: 6, entry: 0x0BAD_F00D, steps: 2, layout_fnv1a: 0x39b3_11ee_6063_af49, stairs: (-1, -1), rooms: 1, terrain: (1722, 70, 0, 0) },
        FixedOracleFloor { number: 7, entry: 0x0BAD_F00D, steps: 4, layout_fnv1a: 0x74c6_09ab_b3a3_fd7e, stairs: (-1, -1), rooms: 1, terrain: (1713, 71, 8, 0) },
        FixedOracleFloor { number: 8, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x10d1_9b0d_2e4d_10d6, stairs: (-1, -1), rooms: 1, terrain: (0, 93, 0, 1699) },
        FixedOracleFloor { number: 9, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x742a_9900_35b8_4f1a, stairs: (-1, -1), rooms: 1, terrain: (1747, 33, 12, 0) },
        FixedOracleFloor { number: 10, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x0a61_7f63_1b06_d7b1, stairs: (-1, -1), rooms: 1, terrain: (1735, 57, 0, 0) },
        FixedOracleFloor { number: 11, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xf7fb_bcb5_aaa0_d4bd, stairs: (-1, -1), rooms: 1, terrain: (1727, 57, 8, 0) },
        FixedOracleFloor { number: 12, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x8430_98a4_c7d0_01ed, stairs: (-1, -1), rooms: 1, terrain: (1726, 56, 10, 0) },
        FixedOracleFloor { number: 13, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x6b3d_21a5_13a2_33b9, stairs: (-1, -1), rooms: 1, terrain: (1719, 63, 10, 0) },
        FixedOracleFloor { number: 14, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x7a89_d2ca_6c33_d0dd, stairs: (-1, -1), rooms: 1, terrain: (1750, 42, 0, 0) },
        FixedOracleFloor { number: 15, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x0e82_ed9d_a4e1_cb92, stairs: (-1, -1), rooms: 1, terrain: (0, 106, 0, 1686) },
        FixedOracleFloor { number: 16, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x61f2_0514_79ce_b929, stairs: (-1, -1), rooms: 1, terrain: (58, 50, 0, 1684) },
        FixedOracleFloor { number: 17, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x3f9c_a508_2cc8_fba9, stairs: (-1, -1), rooms: 1, terrain: (1756, 36, 0, 0) },
        FixedOracleFloor { number: 18, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x36ab_cb21_4e8e_aace, stairs: (-1, -1), rooms: 1, terrain: (1752, 34, 6, 0) },
        FixedOracleFloor { number: 19, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x520b_3ad8_7784_8b4a, stairs: (-1, -1), rooms: 1, terrain: (1744, 48, 0, 0) },
        FixedOracleFloor { number: 20, entry: 0x0BAD_F00D, steps: 4, layout_fnv1a: 0xcd27_d4ae_85d6_1415, stairs: (9, 5), rooms: 1, terrain: (1713, 74, 5, 0) },
        FixedOracleFloor { number: 21, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x4e5d_99d6_a87c_6642, stairs: (-1, -1), rooms: 1, terrain: (1683, 109, 0, 0) },
        FixedOracleFloor { number: 22, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x8221_71fe_87a4_05a1, stairs: (-1, -1), rooms: 1, terrain: (1686, 106, 0, 0) },
        FixedOracleFloor { number: 23, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0xe694_f0c3_dac0_adbe, stairs: (-1, -1), rooms: 1, terrain: (1736, 53, 3, 0) },
        FixedOracleFloor { number: 24, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x5446_084c_8a7c_f1f9, stairs: (-1, -1), rooms: 1, terrain: (1735, 54, 3, 0) },
        FixedOracleFloor { number: 25, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0xdc13_326c_ddde_608d, stairs: (-1, -1), rooms: 1, terrain: (1722, 54, 16, 0) },
        FixedOracleFloor { number: 26, entry: 0x0BAD_F00D, steps: 3, layout_fnv1a: 0x4a23_a173_48a6_375e, stairs: (9, 6), rooms: 1, terrain: (1738, 48, 6, 0) },
        FixedOracleFloor { number: 27, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xb5e8_f9ad_ae60_5539, stairs: (-1, -1), rooms: 1, terrain: (1750, 42, 0, 0) },
        FixedOracleFloor { number: 28, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xeaf9_68df_c0dd_c575, stairs: (-1, -1), rooms: 1, terrain: (1743, 32, 17, 0) },
        FixedOracleFloor { number: 29, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x82c6_0e83_cf3d_5bee, stairs: (-1, -1), rooms: 1, terrain: (1743, 33, 16, 0) },
        FixedOracleFloor { number: 30, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0xfa2e_b33a_0085_f7ea, stairs: (-1, -1), rooms: 1, terrain: (1750, 42, 0, 0) },
        FixedOracleFloor { number: 31, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0x758b_eed9_ac68_4dbe, stairs: (-1, -1), rooms: 1, terrain: (1750, 42, 0, 0) },
        FixedOracleFloor { number: 32, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x66f0_253b_3c20_f4d1, stairs: (-1, -1), rooms: 1, terrain: (1743, 42, 7, 0) },
        FixedOracleFloor { number: 33, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x2d9f_d018_f9ae_ceee, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 34, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0x1b5b_6a61_382d_4326, stairs: (-1, -1), rooms: 1, terrain: (1749, 43, 0, 0) },
        FixedOracleFloor { number: 35, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x2d9f_d018_f9ae_ceee, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 36, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x2d9f_d018_f9ae_ceee, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 37, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0xdc62_705b_a2f4_1416, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 38, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x2d9f_d018_f9ae_ceee, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 39, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xbcbd_d8e4_c0f6_4bb5, stairs: (-1, -1), rooms: 1, terrain: (1752, 40, 0, 0) },
        FixedOracleFloor { number: 40, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xc379_8c9c_3931_ce3d, stairs: (-1, -1), rooms: 1, terrain: (1754, 38, 0, 0) },
        FixedOracleFloor { number: 41, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0xdc62_705b_a2f4_1416, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 42, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0xdc62_705b_a2f4_1416, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 43, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0xdc62_705b_a2f4_1416, stairs: (-1, -1), rooms: 1, terrain: (1743, 49, 0, 0) },
        FixedOracleFloor { number: 44, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xa976_6350_521a_5a26, stairs: (-1, -1), rooms: 1, terrain: (1747, 39, 6, 0) },
        FixedOracleFloor { number: 45, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0x96dd_86cf_cc7f_da1e, stairs: (-1, -1), rooms: 1, terrain: (1747, 33, 12, 0) },
        FixedOracleFloor { number: 46, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xdb37_1800_eb5f_87b6, stairs: (-1, -1), rooms: 1, terrain: (1747, 31, 14, 0) },
        FixedOracleFloor { number: 47, entry: 0x0BAD_F00D, steps: 5, layout_fnv1a: 0xb73f_8371_0bf1_6659, stairs: (-1, -1), rooms: 1, terrain: (1758, 34, 0, 0) },
        FixedOracleFloor { number: 48, entry: 0x0BAD_F00D, steps: 4, layout_fnv1a: 0xf74e_9067_7cde_6536, stairs: (-1, -1), rooms: 1, terrain: (1751, 41, 0, 0) },
        FixedOracleFloor { number: 49, entry: 0x0BAD_F00D, steps: 6, layout_fnv1a: 0xac2b_48cd_4621_decd, stairs: (-1, -1), rooms: 1, terrain: (1736, 36, 20, 0) },
        FixedOracleFloor { number: 50, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0xbb4d_5ff2_4572_615e, stairs: (50, 24), rooms: 4, terrain: (1598, 194, 0, 0) },
        FixedOracleFloor { number: 50, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x8c81_419f_67d7_1c2d, stairs: (6, 2), rooms: 7, terrain: (1560, 232, 0, 0) },
        FixedOracleFloor { number: 50, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xdf0a_60ae_5d88_0e58, stairs: (39, 13), rooms: 7, terrain: (1525, 267, 0, 0) },
        FixedOracleFloor { number: 51, entry: 0x0BAD_F00D, steps: 978, layout_fnv1a: 0x1372_ee13_f020_926e, stairs: (17, 19), rooms: 4, terrain: (1519, 273, 0, 0) },
        FixedOracleFloor { number: 51, entry: 0x1234_5678, steps: 1756, layout_fnv1a: 0xda1d_2c16_eff4_396b, stairs: (49, 3), rooms: 6, terrain: (1451, 341, 0, 0) },
        FixedOracleFloor { number: 51, entry: 0xCAFE_BABF, steps: 1672, layout_fnv1a: 0xc522_d25d_a327_9b67, stairs: (49, 24), rooms: 6, terrain: (1452, 340, 0, 0) },
        FixedOracleFloor { number: 52, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0xbb4d_5ff2_4572_615e, stairs: (50, 24), rooms: 4, terrain: (1598, 194, 0, 0) },
        FixedOracleFloor { number: 52, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x8c81_419f_67d7_1c2d, stairs: (6, 2), rooms: 7, terrain: (1560, 232, 0, 0) },
        FixedOracleFloor { number: 52, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xdf0a_60ae_5d88_0e58, stairs: (39, 13), rooms: 7, terrain: (1525, 267, 0, 0) },
        FixedOracleFloor { number: 53, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0x2189_3fe6_ed5d_258d, stairs: (50, 24), rooms: 4, terrain: (1589, 195, 8, 0) },
        FixedOracleFloor { number: 53, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x3046_c99d_5cbb_7afa, stairs: (6, 2), rooms: 7, terrain: (1551, 233, 8, 0) },
        FixedOracleFloor { number: 53, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xd1af_01da_8c8f_af67, stairs: (39, 13), rooms: 7, terrain: (1516, 268, 8, 0) },
        FixedOracleFloor { number: 54, entry: 0x0BAD_F00D, steps: 980, layout_fnv1a: 0x6dd4_0bb3_8bb9_f5a7, stairs: (48, 13), rooms: 5, terrain: (1530, 262, 0, 0) },
        FixedOracleFloor { number: 54, entry: 0x1234_5678, steps: 1533, layout_fnv1a: 0x1489_add2_0d67_9a3d, stairs: (15, 12), rooms: 7, terrain: (1478, 314, 0, 0) },
        FixedOracleFloor { number: 54, entry: 0xCAFE_BABF, steps: 1215, layout_fnv1a: 0x8feb_b858_3bf3_0683, stairs: (37, 4), rooms: 6, terrain: (1558, 234, 0, 0) },
        FixedOracleFloor { number: 55, entry: 0x0BAD_F00D, steps: 980, layout_fnv1a: 0x378f_59b6_eedb_e367, stairs: (48, 13), rooms: 5, terrain: (1530, 262, 0, 0) },
        FixedOracleFloor { number: 55, entry: 0x1234_5678, steps: 1533, layout_fnv1a: 0x46c1_255c_9792_9f5d, stairs: (15, 12), rooms: 7, terrain: (1478, 314, 0, 0) },
        FixedOracleFloor { number: 55, entry: 0xCAFE_BABF, steps: 1215, layout_fnv1a: 0x1eb5_f609_31e4_cf43, stairs: (37, 4), rooms: 6, terrain: (1558, 234, 0, 0) },
        FixedOracleFloor { number: 56, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0x5010_2ffe_8da6_4d3c, stairs: (50, 24), rooms: 4, terrain: (1596, 196, 0, 0) },
        FixedOracleFloor { number: 56, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0xf56b_729c_6981_47b4, stairs: (6, 2), rooms: 7, terrain: (1558, 234, 0, 0) },
        FixedOracleFloor { number: 56, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0x56a1_a57e_2e43_471a, stairs: (39, 13), rooms: 7, terrain: (1522, 270, 0, 0) },
        FixedOracleFloor { number: 57, entry: 0x0BAD_F00D, steps: 980, layout_fnv1a: 0x0369_d802_5be3_cc4b, stairs: (48, 13), rooms: 5, terrain: (1528, 264, 0, 0) },
        FixedOracleFloor { number: 57, entry: 0x1234_5678, steps: 1533, layout_fnv1a: 0xdbee_5ab8_4206_1651, stairs: (15, 12), rooms: 7, terrain: (1476, 316, 0, 0) },
        FixedOracleFloor { number: 57, entry: 0xCAFE_BABF, steps: 1215, layout_fnv1a: 0x1ada_bad4_03ee_28ab, stairs: (37, 4), rooms: 6, terrain: (1556, 236, 0, 0) },
        FixedOracleFloor { number: 58, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0xbb4d_5ff2_4572_615e, stairs: (50, 24), rooms: 4, terrain: (1598, 194, 0, 0) },
        FixedOracleFloor { number: 58, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x8c81_419f_67d7_1c2d, stairs: (6, 2), rooms: 7, terrain: (1560, 232, 0, 0) },
        FixedOracleFloor { number: 58, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xdf0a_60ae_5d88_0e58, stairs: (39, 13), rooms: 7, terrain: (1525, 267, 0, 0) },
        FixedOracleFloor { number: 59, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0xbb4d_5ff2_4572_615e, stairs: (50, 24), rooms: 4, terrain: (1598, 194, 0, 0) },
        FixedOracleFloor { number: 59, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x8c81_419f_67d7_1c2d, stairs: (6, 2), rooms: 7, terrain: (1560, 232, 0, 0) },
        FixedOracleFloor { number: 59, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xdf0a_60ae_5d88_0e58, stairs: (39, 13), rooms: 7, terrain: (1525, 267, 0, 0) },
        FixedOracleFloor { number: 60, entry: 0x0BAD_F00D, steps: 978, layout_fnv1a: 0x558a_e6e9_665a_942b, stairs: (17, 19), rooms: 4, terrain: (1517, 275, 0, 0) },
        FixedOracleFloor { number: 60, entry: 0x1234_5678, steps: 1756, layout_fnv1a: 0x4253_5d18_1f5e_22ad, stairs: (49, 3), rooms: 6, terrain: (1450, 342, 0, 0) },
        FixedOracleFloor { number: 60, entry: 0xCAFE_BABF, steps: 1672, layout_fnv1a: 0xc5cc_698c_3092_6ca8, stairs: (49, 24), rooms: 6, terrain: (1452, 340, 0, 0) },
        FixedOracleFloor { number: 61, entry: 0x0BAD_F00D, steps: 978, layout_fnv1a: 0x3684_7982_3c43_7375, stairs: (17, 19), rooms: 4, terrain: (1518, 274, 0, 0) },
        FixedOracleFloor { number: 61, entry: 0x1234_5678, steps: 1756, layout_fnv1a: 0x33f9_9377_376e_5224, stairs: (49, 3), rooms: 6, terrain: (1450, 342, 0, 0) },
        FixedOracleFloor { number: 61, entry: 0xCAFE_BABF, steps: 1672, layout_fnv1a: 0x76fa_2360_0c55_81b4, stairs: (49, 24), rooms: 6, terrain: (1451, 341, 0, 0) },
        FixedOracleFloor { number: 62, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0xbb4d_5ff2_4572_615e, stairs: (50, 24), rooms: 4, terrain: (1598, 194, 0, 0) },
        FixedOracleFloor { number: 62, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x8c81_419f_67d7_1c2d, stairs: (6, 2), rooms: 7, terrain: (1560, 232, 0, 0) },
        FixedOracleFloor { number: 62, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xdf0a_60ae_5d88_0e58, stairs: (39, 13), rooms: 7, terrain: (1525, 267, 0, 0) },
        FixedOracleFloor { number: 63, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0xbb4d_5ff2_4572_615e, stairs: (50, 24), rooms: 4, terrain: (1598, 194, 0, 0) },
        FixedOracleFloor { number: 63, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x8c81_419f_67d7_1c2d, stairs: (6, 2), rooms: 7, terrain: (1560, 232, 0, 0) },
        FixedOracleFloor { number: 63, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xdf0a_60ae_5d88_0e58, stairs: (39, 13), rooms: 7, terrain: (1525, 267, 0, 0) },
        FixedOracleFloor { number: 64, entry: 0x0BAD_F00D, steps: 980, layout_fnv1a: 0xec6a_e94e_0480_44d0, stairs: (48, 13), rooms: 5, terrain: (1512, 269, 11, 0) },
        FixedOracleFloor { number: 64, entry: 0x1234_5678, steps: 1533, layout_fnv1a: 0xe5fd_3070_624f_3a32, stairs: (15, 12), rooms: 7, terrain: (1460, 321, 11, 0) },
        FixedOracleFloor { number: 64, entry: 0xCAFE_BABF, steps: 1215, layout_fnv1a: 0x8db5_a0ba_4b6a_fd38, stairs: (37, 4), rooms: 6, terrain: (1540, 241, 11, 0) },
        FixedOracleFloor { number: 65, entry: 0x0BAD_F00D, steps: 980, layout_fnv1a: 0xc08c_ddd4_455b_43a8, stairs: (48, 13), rooms: 5, terrain: (1506, 275, 11, 0) },
        FixedOracleFloor { number: 65, entry: 0x1234_5678, steps: 1533, layout_fnv1a: 0xe56b_5233_fdd9_e513, stairs: (15, 12), rooms: 7, terrain: (1455, 326, 11, 0) },
        FixedOracleFloor { number: 65, entry: 0xCAFE_BABF, steps: 1215, layout_fnv1a: 0xe547_64f0_4266_15d8, stairs: (37, 4), rooms: 6, terrain: (1534, 247, 11, 0) },
        FixedOracleFloor { number: 66, entry: 0x0BAD_F00D, steps: 774, layout_fnv1a: 0x2189_3fe6_ed5d_258d, stairs: (50, 24), rooms: 4, terrain: (1589, 195, 8, 0) },
        FixedOracleFloor { number: 66, entry: 0x1234_5678, steps: 1331, layout_fnv1a: 0x3046_c99d_5cbb_7afa, stairs: (6, 2), rooms: 7, terrain: (1551, 233, 8, 0) },
        FixedOracleFloor { number: 66, entry: 0xCAFE_BABF, steps: 1319, layout_fnv1a: 0xd1af_01da_8c8f_af67, stairs: (39, 13), rooms: 7, terrain: (1516, 268, 8, 0) },
];

rescue_tests::rom_test! {
    /// Forced fixed rooms 1 to 66 against the retail floors (#342 P5). Every row is
    /// checked; the test lists each mismatch (number, seed, port and oracle value)
    /// before it fails.
    fn fixed_rooms_match_the_retail_floors(rom) {
        assert_eq!(FIXED_ORACLE_FLOORS.len(), 100);
        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let dun = dungeon_file(&bytes, &fs);
        let darc = Archive::parse(&dun).expect("dungeon.sbin archive parses");
        let fixed_map = darc.get(&dun, "fixedmap").expect("fixedmap in dungeon.sbin").to_vec();
        let table = fixed_table(&bytes, &header, &fs);
        let map = FixedMap::parse(&fixed_map).expect("fixedmap parses");
        assert!(map.entry(1).is_some(), "fixedmap has entry 1");

        let mut bad = Vec::new();
        for want in FIXED_ORACLE_FLOORS {
            bad.extend(check_floor(want, &fixed_map, &table));
        }
        assert!(
            bad.is_empty(),
            "{} mismatches in {} rows:\n{}",
            bad.len(),
            FIXED_ORACLE_FLOORS.len(),
            bad.join("\n")
        );
    }
}
