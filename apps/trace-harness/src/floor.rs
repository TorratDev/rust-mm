//! The replay's first floor, generated from the row's `floorSeed` and the dungeon's
//! `FloorProperties` (#371, P2 of #332). The row carries neither the properties nor the
//! dungeon and floor ids: the scenario file gives the ids, and the properties are read from
//! the ROM's `dungeon.sbin` `mapparam`.

use std::fmt;

use rescue_data::dungeon::{FLOOR_PROPERTIES, MapParam};
use rescue_data::item::parse_items;
use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::floor::Floor;
use rescue_sim::floor_gen::{FloorProperties, GenState, PortedSteps, generate_floor};
use rescue_sim::rng::DungeonRng;

use crate::scenario::Scenario;

/// Layout cases the port generates (#111). The sim panics on the others, so they are refused
/// before generation, by name.
pub const PORTED_LAYOUTS: &[u8] = &[1];

/// LCG steps from `floorSeed | 1` to the entry of `GenerateFloor`.
// shortcut: 10 steps checked by arithmetic on floor 1 only (#332 P2), upgrade when another
// floor's entry state is checked.
const ENTRY_STEPS: u32 = 10;

/// Why the replay's floor cannot be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloorError {
    /// The floor's layout case is not ported (see [`PORTED_LAYOUTS`]).
    UnportedLayout { layout: u8 },
    /// The scenario's dungeon or floor is not in the ROM's `mapparam`.
    NoFloor { dungeon: u8, floor: u8 },
    /// The ROM could not be read or parsed. The message is the cause.
    Rom(String),
    /// The generator stopped before the floor was complete.
    Generation,
}

impl fmt::Display for FloorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnportedLayout { layout } => write!(
                f,
                "floor layout {layout} is not ported yet (#111); only layout 1 is"
            ),
            Self::NoFloor { dungeon, floor } => {
                write!(
                    f,
                    "dungeon {dungeon} floor {floor} is not in the ROM's mapparam"
                )
            }
            Self::Rom(message) => write!(f, "cannot read the floor data from the ROM: {message}"),
            Self::Generation => write!(f, "the floor generator stopped early"),
        }
    }
}

impl std::error::Error for FloorError {}

fn rom_error(e: impl fmt::Display) -> FloorError {
    FloorError::Rom(e.to_string())
}

/// Reads one `FloorProperties` from its raw record (Red's struct, the offsets named on
/// [`FloorProperties`]). Every field is one byte.
#[must_use]
pub fn properties_from_record(record: &[u8; FLOOR_PROPERTIES]) -> FloorProperties {
    FloorProperties {
        layout: record[0x00],
        room_density: record[0x01].cast_signed(),
        byte_05: record[0x05],
        byte_06: record[0x06],
        byte_07: record[0x07],
        byte_08: record[0x08],
        byte_09: record[0x09],
        byte_0b: record[0x0B],
        byte_0c: record[0x0C],
        byte_0d: record[0x0D],
        byte_0f: record[0x0F],
        byte_10: record[0x10],
        byte_13: record[0x13],
        byte_14: record[0x14],
        byte_15: record[0x15],
        itemless_chance: record[0x19],
    }
}

/// The properties of the scenario's floor: its dungeon's floor entry names a record in
/// `floor_properties`.
pub fn properties_for(map: &MapParam, scenario: Scenario) -> Result<FloorProperties, FloorError> {
    let missing = FloorError::NoFloor {
        dungeon: scenario.dungeon_id,
        floor: scenario.floor_id,
    };
    let entry = map
        .dungeons
        .get(usize::from(scenario.dungeon_id))
        .and_then(|floors| floors.get(usize::from(scenario.floor_id)))
        .ok_or_else(|| missing.clone())?;
    let record = map
        .floor_properties
        .get(usize::from(entry.properties))
        .ok_or(missing)?;
    Ok(properties_from_record(record))
}

/// Generates the floor of a row from its `floorSeed`. Returns the floor and the layout byte
/// the generator recorded.
pub fn generate(floor_seed: u32, props: &FloorProperties) -> Result<(Floor, u8), FloorError> {
    let layout = props.layout & 0xF;
    if !PORTED_LAYOUTS.contains(&layout) {
        return Err(FloorError::UnportedLayout { layout });
    }
    let mut rng = DungeonRng::new(floor_seed);
    for _ in 0..ENTRY_STEPS {
        rng.next_u16();
    }
    let mut floor = Floor::default();
    let mut g = GenState::default();
    generate_floor(
        props,
        false,
        &mut floor,
        &mut rng,
        &mut g,
        &mut PortedSteps::default(),
        false,
    )
    .map_err(|_| FloorError::Generation)?;
    Ok((floor, g.layout_byte))
}

/// Parses the ROM's `mapparam` (in `dungeon.sbin`). The item count the parser needs is the
/// length of `itempara` (in `system.sbin`).
pub fn load_map_param(rom: &[u8]) -> Result<MapParam, FloorError> {
    let header = Header::parse(rom).map_err(rom_error)?;
    let fs = Filesystem::parse(rom, &header).map_err(rom_error)?;
    let items = parse_items(sub_file(rom, &fs, "system.sbin", "itempara")?).map_err(rom_error)?;
    let mapparam = sub_file(rom, &fs, "dungeon.sbin", "mapparam")?;
    rescue_data::dungeon::parse_mapparam(mapparam, items.len()).map_err(rom_error)
}

/// The properties of the scenario's floor, read from the ROM.
pub fn start_properties(rom: &[u8], scenario: Scenario) -> Result<FloorProperties, FloorError> {
    properties_for(&load_map_param(rom)?, scenario)
}

/// The bytes of sub-file `name` in the pack file `pack` of the ROM.
fn sub_file<'a>(
    rom: &'a [u8],
    fs: &Filesystem,
    pack: &str,
    name: &str,
) -> Result<&'a [u8], FloorError> {
    let file = fs
        .get(pack)
        .ok_or_else(|| FloorError::Rom(format!("no {pack} in the ROM")))?;
    let data = fs.data(rom, file.id).map_err(rom_error)?;
    let archive = Archive::parse(data).map_err(rom_error)?;
    archive.get(data, name).map_err(rom_error)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rescue_data::dungeon::Floor as DungeonFloor;

    /// A hand-written first-floor record: layout 1, room density 5, walk 15, monster
    /// density 4, two items, five extra hallways. Not game data.
    pub(crate) fn first_floor_record() -> [u8; FLOOR_PROPERTIES] {
        let mut r = [0u8; FLOOR_PROPERTIES];
        r[0x00] = 1;
        r[0x01] = 5;
        r[0x05] = 15;
        r[0x06] = 4;
        r[0x0F] = 2;
        r[0x13] = 5;
        r
    }

    pub(crate) fn first_floor_properties() -> FloorProperties {
        properties_from_record(&first_floor_record())
    }

    #[test]
    fn record_bytes_map_to_their_fields() {
        let mut r = [0u8; FLOOR_PROPERTIES];
        r[0x00] = 0x11;
        r[0x01] = 0xFB; // -5 as a signed byte
        r[0x05] = 15;
        r[0x06] = 4;
        r[0x07] = 7;
        r[0x08] = 8;
        r[0x09] = 9;
        r[0x0B] = 11;
        r[0x0C] = 12;
        r[0x0D] = 13;
        r[0x0F] = 2;
        r[0x10] = 3;
        r[0x13] = 5;
        r[0x14] = 6;
        r[0x15] = 1;
        r[0x19] = 10;
        assert_eq!(
            properties_from_record(&r),
            FloorProperties {
                layout: 0x11,
                room_density: -5,
                byte_05: 15,
                byte_06: 4,
                byte_07: 7,
                byte_08: 8,
                byte_09: 9,
                byte_0b: 11,
                byte_0c: 12,
                byte_0d: 13,
                byte_0f: 2,
                byte_10: 3,
                byte_13: 5,
                byte_14: 6,
                byte_15: 1,
                itemless_chance: 10,
            }
        );
    }

    #[test]
    fn scenario_floor_reads_its_record() {
        let map = MapParam {
            dungeons: vec![vec![DungeonFloor {
                properties: 1,
                monsters: 0,
                traps: 0,
                items: [0; 4],
            }]],
            floor_properties: vec![[0; FLOOR_PROPERTIES], first_floor_record()],
            monster_spawns: Vec::new(),
            trap_chances: Vec::new(),
            item_spawns: Vec::new(),
        };
        let scenario = Scenario {
            dungeon_id: 0,
            floor_id: 0,
        };
        assert_eq!(properties_for(&map, scenario), Ok(first_floor_properties()));
        let missing = Scenario {
            dungeon_id: 0,
            floor_id: 1,
        };
        assert_eq!(
            properties_for(&map, missing),
            Err(FloorError::NoFloor {
                dungeon: 0,
                floor: 1
            })
        );
    }

    #[test]
    fn first_floor_is_generated_with_layout_one() {
        let (floor, layout) =
            generate(0x1234_5678, &first_floor_properties()).expect("layout 1 is ported");
        assert_eq!(layout, 1);
        let empty = Floor::default();
        let written = (1..rescue_sim::floor::HEIGHT as i32 - 1)
            .flat_map(|y| (1..rescue_sim::floor::WIDTH as i32 - 1).map(move |x| (x, y)))
            .any(|(x, y)| floor.tile(x, y) != empty.tile(x, y));
        assert!(
            written,
            "the interior of a generated floor is not the empty floor"
        );
    }

    #[test]
    fn unported_layout_is_refused_by_name() {
        let mut props = first_floor_properties();
        props.layout = 3;
        let err = generate(0x1234_5678, &props).expect_err("layout 3 is not ported");
        assert_eq!(err, FloorError::UnportedLayout { layout: 3 });
        assert!(err.to_string().contains("layout 3"), "{err}");
    }
}
