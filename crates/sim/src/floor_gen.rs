//! `GenerateFloor` (`ov_0001::02151E7C`): the entry reset, the grid and layout
//! draws, the fixed-room branch and the two retry loops, in the order of
//! Blue's code. See `docs/re/dungeon-generation.md`, "Prologue and retry loops".
//!
//! The steps this function calls are behind [`FloorSteps`]; this module
//! decides when each runs and which RNG draws happen around it.
//! [`PortedSteps`] implements them with the ported functions of
//! [`crate::floor`]; the fixed rooms and layout cases 3 to 7, 9 and 10 are not
//! ported yet.

use crate::floor::{
    self, EnemySpawn, Floor, Grid, GridMode, Layout, LayoutPlan, MAX_LAYOUT_ATTEMPTS,
    MAX_STAIRS_ATTEMPTS, RoomFeatures, SpawnCounts, SpawnState, attempt_has_rooms, plan_layout,
    stairs_check,
};
use crate::rng::DungeonRng;

/// "Unset" stairs position, as the game stores it.
pub const UNSET: (i16, i16) = (-1, -1);

/// Floor-properties bytes that `GenerateFloor` and the steps it calls read,
/// named by offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloorProperties {
    /// `+0x00`: the layout byte. `& 0xF` picks the layout case; the value 8
    /// changes the grid draw.
    pub layout: u8,
    /// The room density of step 3 (`+0x01`, from `pmd-red`'s layout of the
    /// struct; a `[red-hypothesis]` offset).
    pub room_density: i8,
    /// `+0x05`: walk length of step 5.
    pub byte_05: u8,
    /// `+0x06`: its value is stored in dungeon `+0x654`; the monster density
    /// of `SpawnEnemies`.
    pub byte_06: u8,
    /// `+0x07`: copied to `021B45B0`, used by steps 9 and 10.
    pub byte_07: u8,
    /// `+0x08`: copied to `021B45AC`, used by steps 9 and 10.
    pub byte_08: u8,
    /// `+0x09`: maze-room chance of step 8.
    pub byte_09: u8,
    /// `+0x0B`: non-zero skips the dead-end pass of step 5.
    pub byte_0b: u8,
    /// `+0x0C`: copied to `gFloorPropsByteC`; gates a room flag.
    pub byte_0c: u8,
    /// `+0x0D`: bit 0 turns the secondary terrain on, bit 2 marks room imperfections.
    pub byte_0d: u8,
    /// `+0x0F`: average number of items (`SpawnNonEnemies`).
    pub byte_0f: u8,
    /// `+0x10`: most traps (`SpawnNonEnemies`).
    pub byte_10: u8,
    /// `+0x13`: extra-hallway attempts of step 11.
    pub byte_13: u8,
    /// `+0x14`: average number of buried items (`SpawnNonEnemies`).
    pub byte_14: u8,
    /// `+0x15`: number of secondary-terrain ponds.
    pub byte_15: u8,
    /// `+0x19`: percent chance of an itemless monster house.
    pub itemless_chance: u8,
}

/// The globals and dungeon fields `GenerateFloor` sets, with the names used in
/// the table of `docs/re/dungeon-generation.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenState {
    /// Fixed room number (`+0x34B4`); 0 when the floor is not a fixed room.
    pub fixed_room: i16,
    /// `+0x654`: the absolute value of props byte `+0x06`.
    pub field_654: i16,
    /// `+0x34A8`: monster house flag.
    pub monster_house: u8,
    /// `+0x34A9`: cleared on entry.
    pub field_34a9: u8,
    /// `+0x34AA`: set after the retry loops.
    pub field_34aa: u8,
    /// `+0x34AC`: `0xFF` at each layout attempt.
    pub field_34ac: u8,
    /// `+0x34B6`: index of the current layout attempt.
    pub attempt: i16,
    /// Stairs pair A (`+0xC0B8`, `+0xC0BA`): the position the floor must have.
    pub stairs_a: (i16, i16),
    /// Stairs pair B (`+0xC0BC`, `+0xC0BE`): the position `StairsAlwaysReachable` checks.
    pub stairs_b: (i16, i16),
    /// The pair at `021B45C8`.
    pub pair_c: (i16, i16),
    /// `021B45A4`: when set after a layout attempt, that attempt is retried.
    /// Nothing in overlay 1 sets it.
    pub retry_flag: u8,
    /// `gFloorPropsByteC` (`021B45C0`); cleared from the second layout attempt on.
    pub props_byte_c: u8,
    /// `021B45B0`: copy of props byte `+0x07`.
    pub props_copy_07: i16,
    /// `021B45AC`: copy of props byte `+0x08`.
    pub props_copy_08: i16,
    /// `021B4584`: set to 1 on entry.
    pub byte_4584: u8,
    /// `021B4598`: `0xFF` on entry.
    pub byte_4598: u8,
    /// `021B45B8`: the floor's layout byte, stored at each layout attempt.
    pub layout_byte: u8,
    /// `021B45CC`: the first of four words set to `-1` on entry; `GenerateFloor`
    /// later tests its sign.
    pub word_45cc: i32,
    /// `021B458C`, `021B4590`, `021B4594`: cleared on entry, not read here.
    pub bytes_458c_4594: [u8; 3],
    /// `gStandardGridMode` (`021B4588`): the grid mode the standard generator uses.
    pub grid_mode: GridMode,
    /// `021B45A8`: set on entry when the tileset is type 2 (the caller passes it).
    pub tileset_type2: bool,
}

impl Default for GenState {
    fn default() -> Self {
        Self {
            fixed_room: 0,
            field_654: 0,
            monster_house: 0,
            field_34a9: 0,
            field_34aa: 0,
            field_34ac: 0,
            attempt: 0,
            stairs_a: UNSET,
            stairs_b: UNSET,
            pair_c: UNSET,
            retry_flag: 0,
            props_byte_c: 0,
            props_copy_07: 0,
            props_copy_08: 0,
            byte_4584: 0,
            byte_4598: 0,
            layout_byte: 0,
            word_45cc: -1,
            bytes_458c_4594: [0; 3],
            grid_mode: GridMode::Full,
            tileset_type2: false,
        }
    }
}

/// A fixed room of `ProcessFixedRoom` for numbers from 50 on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedRoom {
    /// The fixed room number.
    pub number: i16,
    /// Room grid size: `56 / (w + 4)` by `32 / (h + 4)`, at least 1.
    pub grid: (usize, usize),
    /// The entry's width and height bytes.
    pub size: (u8, u8),
}

/// Steps `GenerateFloor` calls that are outside this module. Each method is
/// one Blue function; see the name in its doc comment.
pub trait FloorSteps {
    /// `ResetFloor` (`0x0214BDBC`).
    fn reset_floor(&mut self, floor: &mut Floor, g: &mut GenState);
    /// Width and height bytes of fixed room `number` (entry bytes 0 and 1 of
    /// the fixed-room file's table).
    fn fixed_room_size(&mut self, number: i16) -> (u8, u8);
    /// `0x0214A0D4`: places a fixed room numbered below 50. Returns nothing;
    /// `ProcessFixedRoom` then reports success.
    fn place_fixed_room(
        &mut self,
        number: i16,
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    );
    /// The last call of `ProcessFixedRoom` for numbers from 50 on, with the
    /// room grid size `(grid_w, grid_h)` and the entry bytes `size`.
    fn place_fixed_room_large(
        &mut self,
        room: FixedRoom,
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    );
    /// `GenerateOneRoomMonsterHouseFloor` (`0x021506B0`); see
    /// [`crate::floor::generate_one_room_monster_house`].
    fn one_room_monster_house(&mut self, floor: &mut Floor, rng: &mut DungeonRng, g: &mut GenState);
    /// The layout generator for `plan`: the standard generator, `GenerateOuterRoomsFloor`
    /// or one of the other `Layout` generators.
    fn generate_layout(
        &mut self,
        plan: LayoutPlan,
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    );
    /// `ResetInnerBoundaryTileRows` (`0x02149A70`).
    fn reset_inner_boundary(&mut self, floor: &mut Floor);
    /// `EnsureImpassableTilesAreWalls` (`0x0214C0E0`).
    fn ensure_impassable(&mut self, floor: &mut Floor);
    /// `FinalizeJunctions` (`0x0214D900`).
    fn finalize_junctions(&mut self, floor: &mut Floor);
    /// `GenerateSecondaryTerrainFormations(1, props)` (`0x0214AAF8`).
    fn secondary_terrain(
        &mut self,
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
    );
    /// `SpawnNonEnemies` (`0x0214B650`).
    fn spawn_non_enemies(
        &mut self,
        props: &FloorProperties,
        itemless: bool,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    );
    /// `SpawnEnemies` (`0x0214B338`).
    fn spawn_enemies(
        &mut self,
        props: &FloorProperties,
        itemless: bool,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    );
    /// `ResolveInvalidSpawns` (`0x0214C1A8`); see [`crate::floor::resolve_invalid_spawns`].
    fn resolve_invalid_spawns(&mut self, floor: &mut Floor, rng: &mut DungeonRng);
    /// `GetFloorType` (`0x0206DB90`); see [`crate::floor::floor_type`].
    fn floor_type(&mut self, g: &GenState) -> u8;
    /// `StairsAlwaysReachable(x, y, FALSE)` (`0x0214A760`); see
    /// [`crate::floor::stairs_always_reachable`]. `stored` is stairs pair B.
    fn stairs_reachable(&mut self, floor: &mut Floor, x: i16, y: i16, stored: (i16, i16)) -> bool;
    /// The call with `pair_c`, the shop's centre, as its position
    /// (`0x02179060`, arguments `x, y, 0x17C, 0`): it adds or updates an
    /// entry `(x, y, 0x17C, 0)` in an 8-entry list of the dungeon (likely a
    /// monster to spawn there). No tiles, no RNG.
    fn pair_c_marker(&mut self, pos: (i16, i16));
    /// The call with the props pointer (`0x02149AF8`) when a shop was made;
    /// see [`crate::floor::finish_shop`].
    fn props_marker(&mut self, props: &FloorProperties, floor: &mut Floor, rng: &mut DungeonRng);
    /// The steps after the stairs fallbacks: `0x02151D4C(0, 0, 56, 32)`
    /// ([`crate::floor::mark_hallway_junctions`]), the optional `0x0214C13C`
    /// when `tileset_type2` ([`crate::floor::secondary_to_terrain_3`]), and
    /// `CloseFile`. None of them draws from the RNG.
    fn final_steps(&mut self, floor: &mut Floor, rng: &mut DungeonRng, tileset_type2: bool);
}

/// The [`FloorSteps`] made of the ported functions of [`crate::floor`]. The
/// fixed rooms (they read the fixed-room file, game data) and the layouts of
/// cases 3 to 7, 9 and 10 (#109) are not ported: they panic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortedSteps {
    /// Dungeon byte `+0x668`, read by `GetFloorType`.
    pub byte_668: u8,
    /// Dungeon byte `+0x641`, read by `GetFloorType`.
    pub byte_641: u8,
    /// Dungeon byte `+0x63D`, read by `GetFloorType`.
    pub byte_63d: u8,
    /// Dungeon `+0x64D`: monster-house tiles may get traps.
    pub byte_64d: u8,
    /// Dungeon `+0x678` (16 bit): blocks the monster house, adds an enemy spot.
    pub word_678: i16,
    /// The 3 x 3 shop item chances (the table at `021AAA38` that props byte
    /// `+0x18` picks; game data, so the caller supplies it).
    pub shop_chances: [i16; 9],
    /// The shop area at `021B45CC`; [`GenState::word_45cc`] mirrors its first word.
    pub shop_rect: [i32; 4],
    /// Dungeon `+0xC0E0`: the shop area bounds.
    pub shop_bounds: [i32; 4],
}

impl PortedSteps {
    /// The floor-wide data of steps 8 to 10, as `GenerateFloor` left it in `g`.
    fn features(&self, g: &GenState) -> RoomFeatures {
        let [shop, maze, house] = g.bytes_458c_4594;
        RoomFeatures {
            maze: maze != 0,
            monster_house: house != 0,
            shop: shop != 0,
            shop_rect: if g.word_45cc < 0 {
                [-1; 4]
            } else {
                self.shop_rect
            },
            shop_centre: (g.pair_c.0.cast_unsigned(), g.pair_c.1.cast_unsigned()),
            shop_bounds: self.shop_bounds,
            monster_house_room: g.field_34ac,
        }
    }

    /// Writes the data of steps 8 to 10 back to `g` and `self`.
    fn store(&mut self, f: &RoomFeatures, g: &mut GenState) {
        g.bytes_458c_4594 = [
            u8::from(f.shop),
            u8::from(f.maze),
            u8::from(f.monster_house),
        ];
        g.word_45cc = f.shop_rect[0];
        g.pair_c = (f.shop_centre.0.cast_signed(), f.shop_centre.1.cast_signed());
        g.field_34ac = f.monster_house_room;
        self.shop_rect = f.shop_rect;
        self.shop_bounds = f.shop_bounds;
    }

    /// `GenerateStandardFloor(grid_x, grid_y, props)`: steps 1 to 13.
    fn standard(
        &mut self,
        (grid_x, grid_y, mode): (usize, usize, GridMode),
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    ) {
        let (xs, ys) = floor::grid_bounds(grid_x, grid_y);
        let mut grid = Grid::new(grid_x, grid_y, mode);
        if floor::assign_rooms(&mut grid, props.room_density, rng) < 2 {
            g.byte_4584 = 0;
        }
        let byte_c = g.props_byte_c != 0;
        floor::place_rooms(floor, &mut grid, &xs, &ys, props.byte_0d, byte_c, rng);
        floor::assign_connections(&mut grid, props.byte_05, props.byte_0b != 0, rng);
        floor::create_hallways(floor, &mut grid, &xs, &ys, false, rng);
        floor::ensure_connected_grid(floor, &mut grid, &xs, &ys, rng);
        let ty = self.floor_type(g);
        let mut f = self.features(g);
        let maze = i32::from(props.byte_09);
        floor::generate_maze_room(floor, &mut grid, maze, g.attempt < 0, &mut f, rng);
        let shop = i32::from(g.props_copy_07);
        floor::generate_shop(floor, &mut grid, shop, ty, &mut f, rng);
        let house = i32::from(g.props_copy_08);
        let blocked = self.word_678 != 0;
        floor::generate_monster_house(floor, &mut grid, house, ty, blocked, &mut f, rng);
        floor::generate_extra_hallways(floor, &grid, mode, i32::from(props.byte_13), rng);
        floor::generate_room_imperfections(floor, &grid, rng);
        floor::generate_secondary_structures(floor, &mut grid, &mut g.props_byte_c, &mut f, rng);
        self.store(&f, g);
    }
}

impl FloorSteps for PortedSteps {
    fn reset_floor(&mut self, floor: &mut Floor, g: &mut GenState) {
        *floor = Floor::new();
        g.stairs_b = UNSET;
    }
    fn fixed_room_size(&mut self, number: i16) -> (u8, u8) {
        panic!("fixed room {number}: the fixed-room file is not ported")
    }
    fn place_fixed_room(
        &mut self,
        number: i16,
        _props: &FloorProperties,
        _floor: &mut Floor,
        _rng: &mut DungeonRng,
        _g: &mut GenState,
    ) {
        panic!("fixed room {number}: not ported")
    }
    fn place_fixed_room_large(
        &mut self,
        room: FixedRoom,
        _props: &FloorProperties,
        _floor: &mut Floor,
        _rng: &mut DungeonRng,
        _g: &mut GenState,
    ) {
        panic!("fixed room {}: not ported", room.number)
    }
    fn one_room_monster_house(
        &mut self,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    ) {
        let ty = self.floor_type(g);
        let f = floor::generate_one_room_monster_house(floor, ty, self.word_678 != 0, rng);
        // Step 10 sets only the house flag and room; the other globals stay.
        if f.monster_house {
            g.bytes_458c_4594[2] = 1;
            g.field_34ac = f.monster_house_room;
        }
    }
    fn generate_layout(
        &mut self,
        plan: LayoutPlan,
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    ) {
        match plan {
            LayoutPlan::Standard {
                grid_x,
                grid_y,
                mode,
            } => self.standard((grid_x, grid_y, mode), props, floor, rng, g),
            LayoutPlan::Other(Layout::OneRoomMonsterHouse) => {
                self.one_room_monster_house(floor, rng, g);
            }
            _ => panic!("layout {plan:?}: not ported (#109)"),
        }
    }
    fn reset_inner_boundary(&mut self, floor: &mut Floor) {
        floor::reset_inner_boundary_tile_rows(floor);
    }
    fn ensure_impassable(&mut self, floor: &mut Floor) {
        floor::ensure_impassable_tiles_are_walls(floor);
    }
    fn finalize_junctions(&mut self, floor: &mut Floor) {
        floor::finalize_junctions(floor);
    }
    fn secondary_terrain(
        &mut self,
        props: &FloorProperties,
        floor: &mut Floor,
        rng: &mut DungeonRng,
    ) {
        floor::generate_secondary_terrain(floor, props.byte_0d, props.byte_15, rng);
    }
    fn spawn_non_enemies(
        &mut self,
        props: &FloorProperties,
        itemless: bool,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    ) {
        let counts = SpawnCounts {
            items: props.byte_0f,
            traps: props.byte_10,
            buried: props.byte_14,
        };
        let ty = self.floor_type(g);
        let mut s = SpawnState {
            leader: g.stairs_a,
            stairs: g.stairs_b,
            stairs_room: g.byte_4598,
            house_room: g.field_34ac,
        };
        floor::spawn_non_enemies(floor, counts, itemless, ty, self.byte_64d != 0, &mut s, rng);
        g.stairs_a = s.leader;
        g.stairs_b = s.stairs;
        g.byte_4598 = s.stairs_room;
        g.field_34ac = s.house_room;
    }
    fn spawn_enemies(
        &mut self,
        props: &FloorProperties,
        itemless: bool,
        floor: &mut Floor,
        rng: &mut DungeonRng,
        g: &mut GenState,
    ) {
        let spawn = EnemySpawn {
            density: props.byte_06,
            empty_house: itemless,
            has_house: g.monster_house != 0,
            extra: self.word_678 != 0,
            leader: g.stairs_a,
        };
        floor::spawn_enemies(floor, spawn, rng);
    }
    fn resolve_invalid_spawns(&mut self, floor: &mut Floor, _rng: &mut DungeonRng) {
        floor::resolve_invalid_spawns(floor);
    }
    fn floor_type(&mut self, g: &GenState) -> u8 {
        floor::floor_type(self.byte_668, self.byte_641, self.byte_63d, g.fixed_room)
    }
    fn stairs_reachable(&mut self, floor: &mut Floor, x: i16, y: i16, stored: (i16, i16)) -> bool {
        floor::stairs_always_reachable(floor, (x, y), stored, false)
    }
    // shortcut: the 8-entry list of `0x02179060` is not modelled; it touches no tiles and no RNG.
    fn pair_c_marker(&mut self, _pos: (i16, i16)) {}
    fn props_marker(&mut self, _props: &FloorProperties, floor: &mut Floor, rng: &mut DungeonRng) {
        floor::finish_shop(floor, &mut self.shop_rect, &self.shop_chances, rng);
    }
    fn final_steps(&mut self, floor: &mut Floor, _rng: &mut DungeonRng, tileset_type2: bool) {
        floor::mark_hallway_junctions(floor);
        if tileset_type2 {
            floor::secondary_to_terrain_3(floor);
        }
    }
}

/// A reason `generate_floor` returned before the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenStop {
    /// `GenerateStandardFloor` is the next call, and the caller asked to stop
    /// there. The RNG is in the state at the call's entry.
    StandardFloor {
        grid_x: usize,
        grid_y: usize,
        mode: GridMode,
    },
}

/// Whether `GenerateFloor` sets its secondary-terrain flag after `plan`:
/// the standard generator (cases 0, 8, 1, 11 and 12–15), outer rooms, outer
/// ring, crossroads and line.
#[must_use]
pub const fn sets_secondary_flag(plan: LayoutPlan) -> bool {
    matches!(
        plan,
        LayoutPlan::Standard { .. }
            | LayoutPlan::OuterRooms { .. }
            | LayoutPlan::Other(Layout::OuterRing | Layout::Crossroads | Layout::Line)
    )
}

/// `ProcessFixedRoom` (`0x02150958`): returns whether the fixed room ends
/// generation (`true`) or the floor goes on to the finalising steps (`false`).
pub fn process_fixed_room<S: FloorSteps>(
    number: i16,
    props: &FloorProperties,
    floor: &mut Floor,
    rng: &mut DungeonRng,
    g: &mut GenState,
    steps: &mut S,
) -> bool {
    let (w, h) = steps.fixed_room_size(number);
    if w == 0 || h == 0 {
        steps.one_room_monster_house(floor, rng, g);
        return false;
    }
    if number < 50 {
        steps.place_fixed_room(number, props, floor, rng, g);
        return true;
    }
    // Integer division of the map by the room size plus a margin of 4; a
    // result under 2 becomes 1.
    let grid_w = (56 / (usize::from(w) + 4)).max(1);
    let grid_h = (32 / (usize::from(h) + 4)).max(1);
    steps.place_fixed_room_large(
        FixedRoom {
            number,
            grid: (grid_w, grid_h),
            size: (w, h),
        },
        props,
        floor,
        rng,
        g,
    );
    false
}

/// Runs `GenerateFloor` for the floor `props` describes, starting from the
/// dungeon RNG state in `rng`.
///
/// With `stop_at_standard`, returns [`GenStop::StandardFloor`] as soon as the
/// layout draw picks the standard generator, before any of its calls. That
/// is the checkpoint `verification/v7-trace/floor_stages.py` records as
/// `GenerateStandardFloor`'s entry.
///
/// # Errors
/// [`GenStop::StandardFloor`] when `stop_at_standard` is set and the standard
/// generator would run.
pub fn generate_floor<S: FloorSteps>(
    props: &FloorProperties,
    tileset_type2: bool,
    floor: &mut Floor,
    rng: &mut DungeonRng,
    g: &mut GenState,
    steps: &mut S,
    stop_at_standard: bool,
) -> Result<(), GenStop> {
    // Entry: the globals and dungeon fields `GenerateFloor` sets once.
    g.bytes_458c_4594 = [0; 3];
    g.tileset_type2 = tileset_type2;
    g.byte_4598 = 0xFF;
    g.grid_mode = GridMode::Full;
    g.props_copy_07 = i16::from(props.byte_07);
    g.props_copy_08 = i16::from(props.byte_08);
    g.byte_4584 = 1;
    g.word_45cc = -1;
    steps.reset_floor(floor, g);
    g.field_654 = i16::from(props.byte_06);
    g.field_34a9 = 0;
    g.field_34aa = 0;
    g.props_byte_c = props.byte_0c;

    // Set once the layout runs a generator that leaves its flag for
    // `GenerateSecondaryTerrainFormations`. It persists across attempts.
    let mut secondary_flag = false;

    let mut stairs_ok = false;
    for _ in 0..MAX_STAIRS_ATTEMPTS {
        // Outer attempt: the stairs pairs start unset.
        g.stairs_a = UNSET;
        g.stairs_b = UNSET;

        let mut layout_ok = false;
        for attempt in 0..MAX_LAYOUT_ATTEMPTS {
            // Inner attempt.
            g.attempt = i16::try_from(attempt).unwrap_or(i16::MAX);
            if attempt >= 1 {
                g.props_byte_c = 0;
            }
            g.retry_flag = 0;
            g.pair_c = UNSET;
            steps.reset_floor(floor, g);
            g.stairs_a = UNSET;
            g.monster_house = 0;

            if g.fixed_room != 0 {
                if process_fixed_room(g.fixed_room, props, floor, rng, g, steps) {
                    // The fixed room is the floor: skip the rest of the attempt.
                    layout_ok = true;
                    break;
                }
            } else {
                let plan = plan_layout(props.layout, rng);
                if let LayoutPlan::Standard {
                    grid_x,
                    grid_y,
                    mode,
                } = plan
                {
                    g.grid_mode = mode;
                    if stop_at_standard {
                        return Err(GenStop::StandardFloor {
                            grid_x,
                            grid_y,
                            mode,
                        });
                    }
                }
                g.field_34ac = 0xFF;
                g.layout_byte = props.layout;
                steps.generate_layout(plan, props, floor, rng, g);
                if sets_secondary_flag(plan) {
                    secondary_flag = true;
                }
                if matches!(
                    plan,
                    LayoutPlan::Other(Layout::OneRoomMonsterHouse | Layout::TwoRoomsMonsterHouse)
                ) {
                    g.monster_house = 1;
                }
            }

            // Finalising steps of the attempt.
            steps.reset_inner_boundary(floor);
            steps.ensure_impassable(floor);
            if g.retry_flag != 0 {
                continue;
            }
            if attempt_has_rooms(floor) {
                layout_ok = true;
                break;
            }
        }

        if !layout_ok {
            // Ten failed layouts: the floor is one room with a monster house.
            g.pair_c = UNSET;
            steps.one_room_monster_house(floor, rng, g);
            g.monster_house = 1;
        }
        steps.finalize_junctions(floor);
        if secondary_flag {
            steps.secondary_terrain(props, floor, rng);
        }
        let itemless = rng.int(100) < i32::from(props.itemless_chance);
        steps.spawn_non_enemies(props, itemless, floor, rng, g);
        steps.spawn_enemies(props, itemless, floor, rng, g);
        steps.resolve_invalid_spawns(floor, rng);

        // `GetFloorType` is only called when pair A is set, as in the game.
        let a = g.stairs_a;
        let floor_type = if a.0 != -1 && a.1 != -1 {
            steps.floor_type(g)
        } else {
            0
        };
        let b = g.stairs_b;
        let accepted = stairs_check(a, b, floor_type, |x, y| {
            steps.stairs_reachable(floor, x, y, b)
        });
        if accepted {
            stairs_ok = true;
            break;
        }
    }

    if !stairs_ok {
        // Ten failed stairs checks: the same one-room fallback, with the
        // spawns made without an itemless house.
        g.pair_c = UNSET;
        steps.reset_floor(floor, g);
        steps.one_room_monster_house(floor, rng, g);
        g.monster_house = 1;
        steps.finalize_junctions(floor);
        steps.spawn_non_enemies(props, false, floor, rng, g);
        steps.spawn_enemies(props, false, floor, rng, g);
        steps.resolve_invalid_spawns(floor, rng);
    }

    if g.pair_c.0 >= 0 && g.pair_c.1 >= 0 {
        steps.pair_c_marker(g.pair_c);
    }
    if g.word_45cc < 0 {
        g.field_34aa = 0;
    } else {
        steps.props_marker(props, floor, rng);
        g.field_34aa = 1;
    }
    steps.final_steps(floor, rng, g.tileset_type2);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floor::{TERRAIN_FLOOR, TERRAIN_TYPE_MASK};

    /// Test double for [`FloorSteps`]. It counts calls and opens two rooms
    /// (30 floor tiles) from the layout call number `rooms_from` on.
    #[derive(Default)]
    struct Mock {
        layouts: u32,
        one_room: u32,
        resets: u32,
        non_enemy_spawns: u32,
        placed: u32,
        large: Option<(i16, (usize, usize))>,
        finals: u32,
        stairs_calls: u32,
        /// Stairs pair B is reachable from this `stairs_reachable` call on.
        reachable_from: u32,
        /// Layout call number (1-based) from which the layout opens rooms.
        rooms_from: u32,
        fixed_size: (u8, u8),
    }

    fn open_two_rooms(floor: &mut Floor) {
        for x in 1..=15 {
            for (y, room) in [(1, 0u8), (2, 1u8)] {
                if let Some(t) = floor.tile_mut(x, y) {
                    t.terrain = (t.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
                    t.room = room;
                }
            }
        }
    }

    impl FloorSteps for Mock {
        fn reset_floor(&mut self, floor: &mut Floor, _g: &mut GenState) {
            self.resets += 1;
            *floor = Floor::new();
        }
        fn fixed_room_size(&mut self, _number: i16) -> (u8, u8) {
            self.fixed_size
        }
        fn place_fixed_room(
            &mut self,
            _number: i16,
            _props: &FloorProperties,
            floor: &mut Floor,
            _rng: &mut DungeonRng,
            g: &mut GenState,
        ) {
            self.placed += 1;
            open_two_rooms(floor);
            g.stairs_a = (5, 5);
            g.stairs_b = (7, 7);
        }
        fn place_fixed_room_large(
            &mut self,
            room: FixedRoom,
            _props: &FloorProperties,
            _floor: &mut Floor,
            _rng: &mut DungeonRng,
            _g: &mut GenState,
        ) {
            self.large = Some((room.number, room.grid));
        }
        fn one_room_monster_house(
            &mut self,
            _floor: &mut Floor,
            _rng: &mut DungeonRng,
            _g: &mut GenState,
        ) {
            self.one_room += 1;
        }
        fn generate_layout(
            &mut self,
            _plan: LayoutPlan,
            _props: &FloorProperties,
            floor: &mut Floor,
            _rng: &mut DungeonRng,
            g: &mut GenState,
        ) {
            self.layouts += 1;
            if self.layouts >= self.rooms_from {
                open_two_rooms(floor);
                g.stairs_a = (5, 5);
                g.stairs_b = (7, 7);
            }
        }
        fn reset_inner_boundary(&mut self, _floor: &mut Floor) {}
        fn ensure_impassable(&mut self, _floor: &mut Floor) {}
        fn finalize_junctions(&mut self, _floor: &mut Floor) {}
        fn secondary_terrain(
            &mut self,
            _props: &FloorProperties,
            _floor: &mut Floor,
            _rng: &mut DungeonRng,
        ) {
        }
        fn spawn_non_enemies(
            &mut self,
            _props: &FloorProperties,
            _itemless: bool,
            _floor: &mut Floor,
            _rng: &mut DungeonRng,
            _g: &mut GenState,
        ) {
            self.non_enemy_spawns += 1;
        }
        fn spawn_enemies(
            &mut self,
            _props: &FloorProperties,
            _itemless: bool,
            _floor: &mut Floor,
            _rng: &mut DungeonRng,
            _g: &mut GenState,
        ) {
        }
        fn resolve_invalid_spawns(&mut self, _floor: &mut Floor, _rng: &mut DungeonRng) {}
        fn floor_type(&mut self, _g: &GenState) -> u8 {
            0
        }
        fn stairs_reachable(
            &mut self,
            _floor: &mut Floor,
            _x: i16,
            _y: i16,
            _stored: (i16, i16),
        ) -> bool {
            self.stairs_calls += 1;
            self.stairs_calls >= self.reachable_from
        }
        fn pair_c_marker(&mut self, _pos: (i16, i16)) {}
        fn props_marker(
            &mut self,
            _props: &FloorProperties,
            _floor: &mut Floor,
            _rng: &mut DungeonRng,
        ) {
        }
        fn final_steps(&mut self, _floor: &mut Floor, _rng: &mut DungeonRng, _t2: bool) {
            self.finals += 1;
        }
    }

    fn props(layout: u8) -> FloorProperties {
        FloorProperties {
            layout,
            itemless_chance: 10,
            ..FloorProperties::default()
        }
    }

    fn run(
        mock: &mut Mock,
        props: &FloorProperties,
        rng: &mut DungeonRng,
        g: &mut GenState,
        stop: bool,
    ) -> Result<(), GenStop> {
        let mut floor = Floor::new();
        generate_floor(props, false, &mut floor, rng, g, mock, stop)
    }

    #[test]
    fn forced_seeds_reach_the_standard_floor_entry() {
        // The checkpoints of `floor::tests::layout_prologue_reaches_standard_floor_entry`,
        // now through the entry sequence: grid draw, layout draw, stop at the
        // standard generator's entry.
        let cases = [
            (0x1234_5678u32, 2usize, 985_678_915u32),
            (0xCAFE_BABF, 2, 1_477_956_308),
            (0x0BAD_F00D, 3, 3_889_763_896),
        ];
        for (seed, grid_y, std3) in cases {
            let mut rng = DungeonRng::from_state(seed);
            let mut g = GenState::default();
            let mut mock = Mock::default();
            let err = run(&mut mock, &props(1), &mut rng, &mut g, true).unwrap_err();
            assert_eq!(
                err,
                GenStop::StandardFloor {
                    grid_x: 4,
                    grid_y,
                    mode: GridMode::LeftHalf,
                },
                "seed {seed:#x}"
            );
            assert_eq!(rng.state(), std3, "seed {seed:#x}");
            assert_eq!(
                mock.layouts, 0,
                "the generator must not run before the stop"
            );
        }
    }

    #[test]
    fn layout_retries_until_the_floor_has_rooms() {
        // Layouts 1 to 3 open no rooms; the fourth does.
        let mut mock = Mock {
            rooms_from: 4,
            reachable_from: 1,
            ..Mock::default()
        };
        let mut rng = DungeonRng::new(0x1234_5678);
        let mut g = GenState::default();
        run(&mut mock, &props(0), &mut rng, &mut g, false).unwrap();
        assert_eq!(mock.layouts, 4);
        assert_eq!(g.attempt, 3);
        assert_eq!(mock.one_room, 0);
        assert_eq!(mock.finals, 1);
        assert_eq!(mock.non_enemy_spawns, 1);
    }

    #[test]
    fn ten_failed_layouts_fall_back_to_one_room() {
        // No layout opens rooms and the stairs never pass: ten outer attempts
        // of ten layouts each, a one-room fallback after each inner loop, and
        // the outer fallback at the end.
        let mut mock = Mock {
            rooms_from: u32::MAX,
            reachable_from: u32::MAX,
            ..Mock::default()
        };
        let mut rng = DungeonRng::new(7);
        let mut g = GenState::default();
        run(&mut mock, &props(0), &mut rng, &mut g, false).unwrap();
        assert_eq!(mock.layouts, 100);
        assert_eq!(mock.one_room, 11);
        // The entry reset, one reset per layout attempt, and the outer fallback.
        assert_eq!(mock.resets, 1 + 100 + 1);
        assert_eq!(g.monster_house, 1);
        assert_eq!(mock.non_enemy_spawns, 11);
    }

    #[test]
    fn stairs_retry_repeats_the_outer_loop() {
        // Rooms are there at once; the stairs check passes on the third outer attempt.
        let mut mock = Mock {
            rooms_from: 1,
            reachable_from: 3,
            ..Mock::default()
        };
        let mut rng = DungeonRng::new(99);
        let mut g = GenState::default();
        run(&mut mock, &props(0), &mut rng, &mut g, false).unwrap();
        assert_eq!(mock.layouts, 3);
        assert_eq!(mock.stairs_calls, 3);
        assert_eq!(mock.one_room, 0);
    }

    #[test]
    fn fixed_room_ends_the_floor_without_a_layout() {
        let mut mock = Mock {
            fixed_size: (3, 3),
            ..Mock::default()
        };
        let mut rng = DungeonRng::new(5);
        let mut g = GenState {
            fixed_room: 7,
            ..GenState::default()
        };
        run(&mut mock, &props(0), &mut rng, &mut g, false).unwrap();
        assert_eq!(mock.placed, 1);
        assert_eq!(mock.layouts, 0);
        assert_eq!(mock.finals, 1);
    }

    #[test]
    fn fixed_room_with_no_size_takes_the_monster_house() {
        let mut mock = Mock::default();
        let mut rng = DungeonRng::new(5);
        let mut g = GenState::default();
        let mut floor = Floor::new();
        let props = props(0);
        assert!(!process_fixed_room(
            3, &props, &mut floor, &mut rng, &mut g, &mut mock
        ));
        assert_eq!(mock.one_room, 1);
    }

    #[test]
    fn large_fixed_room_grid_is_map_over_size_plus_four() {
        // Room 60 of size 4 by 6: grid 56 / (4 + 4) = 7 by 32 / (6 + 4) = 3.
        let mut mock = Mock {
            fixed_size: (4, 6),
            ..Mock::default()
        };
        let mut rng = DungeonRng::new(5);
        let mut g = GenState::default();
        let mut floor = Floor::new();
        let props = props(0);
        assert!(!process_fixed_room(
            60, &props, &mut floor, &mut rng, &mut g, &mut mock
        ));
        assert_eq!(mock.large, Some((60, (7, 3))));
    }

    #[test]
    fn entry_sets_the_globals_the_game_sets() {
        let mut mock = Mock {
            rooms_from: 1,
            reachable_from: 1,
            ..Mock::default()
        };
        let mut rng = DungeonRng::new(3);
        let mut g = GenState::default();
        let props = FloorProperties {
            layout: 0,
            byte_06: 9,
            byte_07: 4,
            byte_08: 6,
            byte_0c: 1,
            ..FloorProperties::default()
        };
        run(&mut mock, &props, &mut rng, &mut g, false).unwrap();
        assert_eq!(g.field_654, 9);
        assert_eq!(g.props_copy_07, 4);
        assert_eq!(g.props_copy_08, 6);
        assert_eq!(g.byte_4598, 0xFF);
        assert_eq!(g.byte_4584, 1);
        assert_eq!(g.field_34ac, 0xFF);
        assert_eq!(g.props_byte_c, 1);
        assert_eq!(g.layout_byte, 0);
        assert_eq!(g.fixed_room, 0);
    }

    #[test]
    fn secondary_flag_follows_the_layout_case() {
        assert!(sets_secondary_flag(LayoutPlan::Standard {
            grid_x: 4,
            grid_y: 2,
            mode: GridMode::LeftHalf,
        }));
        assert!(sets_secondary_flag(LayoutPlan::OuterRooms {
            grid_x: 4,
            grid_y: 4
        }));
        assert!(sets_secondary_flag(LayoutPlan::Other(Layout::Line)));
        assert!(!sets_secondary_flag(LayoutPlan::Other(Layout::Cross)));
        assert!(!sets_secondary_flag(LayoutPlan::Other(
            Layout::TwoRoomsMonsterHouse
        )));
    }
}
