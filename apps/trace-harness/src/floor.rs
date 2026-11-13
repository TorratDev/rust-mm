//! The replay's first floor, generated from the row's `floorSeed` and the dungeon's
//! `FloorProperties` (#371, P2 of #332). The row carries neither the properties nor the
//! dungeon and floor ids: the scenario file gives the ids, and the properties are read from
//! the ROM's `dungeon.sbin` `mapparam`.

use std::fmt;

use rescue_data::dungeon::MapParam;
use rescue_data::item::parse_items;
use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::floor::Floor;
use rescue_sim::floor_gen::{
    FloorProperties, GenState, PortedSteps, generate_floor, properties_for,
};
use rescue_sim::rng::DungeonRng;

use crate::scenario::Scenario;

/// Layout cases the port generates (#111). The sim panics on the others, so they are refused
/// before generation, by name.
pub const PORTED_LAYOUTS: &[u8] = &[1, 10];

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
    let map = load_map_param(rom)?;
    properties_for(&map, scenario.dungeon_id, scenario.floor_id).map_err(|e| FloorError::NoFloor {
        dungeon: e.dungeon,
        floor: e.floor,
    })
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
    use rescue_data::dungeon::FLOOR_PROPERTIES;
    use rescue_sim::floor_gen::properties_from_record;

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
    fn outer_rooms_layout_is_accepted() {
        let mut props = first_floor_properties();
        props.layout = 10;
        let (_, layout) = generate(0x1234_5678, &props).expect("layout 10 is ported");
        assert_eq!(layout, 10);
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
