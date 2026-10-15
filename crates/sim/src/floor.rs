//! Dungeon floor map and the first steps of floor generation.
//!
//! The layout follows the game's (see `docs/re/structs.md`, "Tile", and
//! `docs/re/dungeon-generation.md`), but the types are our own: a floor is a
//! 56 x 32 grid of [`Tile`]s. Generation is being reimplemented step by step and
//! checked against `verification/v7-trace/floor_stages.py`, which records the
//! dungeon RNG state at every step of the game's generator.

use crate::rng::DungeonRng;

/// Map width in tiles.
pub const WIDTH: usize = 56;
/// Map height in tiles.
pub const HEIGHT: usize = 32;

/// Terrain flag: impassable wall (the map border is always one).
pub const TERRAIN_IMPASSABLE_WALL: u16 = 0x10;
/// Mask of the terrain type in [`Tile::terrain`].
pub const TERRAIN_TYPE_MASK: u16 = 0x3;
/// Terrain type of open ground (rooms and hallways).
pub const TERRAIN_FLOOR: u16 = 0x1;
/// Room number of a tile outside every room.
pub const NO_ROOM: u8 = 0xFF;
/// Room number of a hallway anchor: the single open tile that stands in for a
/// grid cell without a room, for hallways to connect to.
pub const ANCHOR_ROOM: u8 = 0xFE;

/// One map tile. Entity links (the monster and object on the tile) live
/// elsewhere in the simulation and are not part of the tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    /// Terrain type and flags (bits 0-1 terrain type, `0x10` impassable wall, ...).
    pub terrain: u16,
    /// Spawn markers during generation, visibility afterwards.
    pub spawn_or_visibility: u16,
    /// Graphics variant; cosmetic, chosen with the global RNG.
    pub graphics_variant: u8,
    /// Room index, or [`NO_ROOM`].
    pub room: u8,
    /// Per crossable-terrain kind, one bit per direction a Pokemon may move to.
    pub walkable_neighbors: [u8; 4],
    /// Purpose not known yet; cleared on reset.
    pub unk_e: u16,
}

impl Tile {
    /// A cleared tile, as the game resets it before generating a floor.
    pub const EMPTY: Self = Self {
        terrain: 0,
        spawn_or_visibility: 0,
        graphics_variant: 0,
        room: NO_ROOM,
        walkable_neighbors: [0; 4],
        unk_e: 0,
    };
}

/// A floor map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Floor {
    tiles: Vec<Tile>,
    /// Stairs position, if placed.
    pub stairs: Option<(i16, i16)>,
}

impl Default for Floor {
    fn default() -> Self {
        Self::new()
    }
}

impl Floor {
    /// A reset floor: every tile cleared, the outermost ring of tiles marked as
    /// impassable wall, no stairs. This is the state generation starts from.
    #[must_use]
    pub fn new() -> Self {
        let mut floor = Self {
            tiles: vec![Tile::EMPTY; WIDTH * HEIGHT],
            stairs: None,
        };
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if x == 0 || y == 0 || x == WIDTH - 1 || y == HEIGHT - 1 {
                    floor.tiles[y * WIDTH + x].terrain |= TERRAIN_IMPASSABLE_WALL;
                }
            }
        }
        floor
    }

    /// The tile at `(x, y)`, or `None` outside the map.
    #[must_use]
    pub fn tile(&self, x: i32, y: i32) -> Option<&Tile> {
        Self::index(x, y).map(|i| &self.tiles[i])
    }

    /// Mutable access to the tile at `(x, y)`, or `None` outside the map.
    pub fn tile_mut(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        Self::index(x, y).map(|i| &mut self.tiles[i])
    }

    fn index(x: i32, y: i32) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < WIDTH && y < HEIGHT).then_some(y * WIDTH + x)
    }
}

/// Generation step 1 of a standard floor: the map split into `grid_x` by
/// `grid_y` cells. Returns the cell boundaries along each axis (`grid_x + 1`
/// and `grid_y + 1` values, starting at 0). Every cell spans `WIDTH / grid_x`
/// by `HEIGHT / grid_y` tiles, rounded down, so the last boundary can fall
/// short of the map edge. Uses no randomness.
///
/// # Panics
/// If `grid_x` or `grid_y` is zero.
#[must_use]
pub fn grid_bounds(grid_x: usize, grid_y: usize) -> (Vec<i32>, Vec<i32>) {
    let axis = |cells: usize, length: usize| -> Vec<i32> {
        let step = i32::try_from(length / cells).unwrap_or(i32::MAX);
        (0..=cells)
            .map(|i| i32::try_from(i).unwrap_or(i32::MAX) * step)
            .collect()
    };
    (axis(grid_x, WIDTH), axis(grid_y, HEIGHT))
}

/// Largest grid height the game's cell array holds (each grid column is 15
/// cells deep).
pub const MAX_GRID_Y: usize = 15;

/// Which grid cells a standard floor may use. Small and medium layouts run the
/// standard generator on a 4-wide grid and leave its right part unused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridMode {
    /// Every cell (large layout and the other generators).
    Full,
    /// Columns `x >= grid_x / 2` unused (small layout).
    LeftHalf,
    /// Columns `x >= grid_x * 3 / 4` unused (medium layout).
    LeftThreeQuarters,
}

/// One cell of the generation grid. Fields are added as the steps that use
/// them are reimplemented; the game's 30-byte record also holds flags that
/// step 2 clears and later steps set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridCell {
    /// Room bounds, or the anchor tile `(start, start + 1)` for a cell without
    /// a room; set in step 4. Ends are exclusive.
    pub start_x: i16,
    pub start_y: i16,
    pub end_x: i16,
    pub end_y: i16,
    /// The cell is outside the area this floor uses.
    pub invalid: bool,
    /// The cell holds a room (otherwise a hallway anchor).
    pub has_room: bool,
    /// Room flag set in step 4 when floor-properties byte `+0x0D` has bit 2;
    /// role not known yet.
    pub room_flag_a: bool,
    /// Room flag set in step 4 (80 % chance) when the floor-properties byte
    /// `+0x0C` is non-zero; role not known yet.
    pub room_flag_b: bool,
    /// `+0x13`: the cell is linked to the cell north of it (`-y`). Step 5.
    pub north: bool,
    /// `+0x14`: linked to the cell south of it (`+y`). Step 5.
    pub south: bool,
    /// `+0x15`: linked to the cell west of it (`-x`). Step 5.
    pub west: bool,
    /// `+0x16`: linked to the cell east of it (`+x`). Step 5.
    pub east: bool,
}

/// A side of a grid cell, numbered as `ov_0001::0214F224` numbers its walk
/// directions: 0 east, 1 north, 2 west, 3 south. Turning to the next
/// direction adds 1 (mod 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    East,
    North,
    West,
    South,
}

impl Dir {
    /// The direction with index `index` (taken mod 4).
    const fn from_index(index: i32) -> Self {
        match index & 3 {
            0 => Self::East,
            1 => Self::North,
            2 => Self::West,
            _ => Self::South,
        }
    }

    const fn next(self) -> Self {
        Self::from_index(self as i32 + 1)
    }

    const fn opposite(self) -> Self {
        match self {
            Self::East => Self::West,
            Self::North => Self::South,
            Self::West => Self::East,
            Self::South => Self::North,
        }
    }
}

impl GridCell {
    fn link(&self, dir: Dir) -> bool {
        match dir {
            Dir::East => self.east,
            Dir::North => self.north,
            Dir::West => self.west,
            Dir::South => self.south,
        }
    }

    fn set_link(&mut self, dir: Dir) {
        match dir {
            Dir::East => self.east = true,
            Dir::North => self.north = true,
            Dir::West => self.west = true,
            Dir::South => self.south = true,
        }
    }

    /// How many of the four sides are linked.
    fn link_count(&self) -> usize {
        [self.north, self.south, self.west, self.east]
            .into_iter()
            .filter(|&linked| linked)
            .count()
    }
}

/// The generation grid: `grid_x` by `grid_y` [`GridCell`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    grid_x: usize,
    grid_y: usize,
    cells: Vec<GridCell>,
}

impl Grid {
    /// Generation step 2 of a standard floor (`ov_0001::0214FD7C`): every
    /// cell reset, marked as holding a room, and marked invalid when `mode`
    /// leaves its column unused. Uses no randomness.
    ///
    /// # Panics
    /// If `grid_y` is larger than [`MAX_GRID_Y`].
    #[must_use]
    pub fn new(grid_x: usize, grid_y: usize, mode: GridMode) -> Self {
        assert!(grid_y <= MAX_GRID_Y, "grid height {grid_y} > {MAX_GRID_Y}");
        let first_unused = match mode {
            GridMode::Full => usize::MAX,
            GridMode::LeftHalf => grid_x / 2,
            GridMode::LeftThreeQuarters => grid_x * 3 / 4,
        };
        let mut cells = Vec::with_capacity(grid_x * grid_y);
        for x in 0..grid_x {
            for _ in 0..grid_y {
                cells.push(GridCell {
                    invalid: x >= first_unused,
                    has_room: true,
                    ..GridCell::default()
                });
            }
        }
        Self {
            grid_x,
            grid_y,
            cells,
        }
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.grid_x
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.grid_y
    }

    /// The cell at column `x`, row `y`.
    ///
    /// # Panics
    /// If `(x, y)` is outside the grid.
    #[must_use]
    pub fn cell(&self, x: usize, y: usize) -> &GridCell {
        assert!(x < self.grid_x && y < self.grid_y);
        &self.cells[x * self.grid_y + y]
    }

    /// Mutable access to the cell at column `x`, row `y`.
    ///
    /// # Panics
    /// If `(x, y)` is outside the grid.
    pub fn cell_mut(&mut self, x: usize, y: usize) -> &mut GridCell {
        assert!(x < self.grid_x && y < self.grid_y);
        &mut self.cells[x * self.grid_y + y]
    }

    /// The cell next to `(x, y)` in `dir`, if it is inside the grid.
    fn neighbour(&self, x: usize, y: usize, dir: Dir) -> Option<(usize, usize)> {
        match dir {
            Dir::East if x + 1 < self.grid_x => Some((x + 1, y)),
            Dir::North if y > 0 => Some((x, y - 1)),
            Dir::West if x > 0 => Some((x - 1, y)),
            Dir::South if y + 1 < self.grid_y => Some((x, y + 1)),
            _ => None,
        }
    }

    /// Links `(x, y)` and its neighbour in `dir` to each other.
    ///
    /// # Panics
    /// If the neighbour is outside the grid.
    fn connect(&mut self, x: usize, y: usize, dir: Dir) {
        let (nx, ny) = self
            .neighbour(x, y, dir)
            .expect("connect: neighbour outside the grid");
        self.cell_mut(x, y).set_link(dir);
        self.cell_mut(nx, ny).set_link(dir.opposite());
    }

    /// Whether the cell east of `(x, y)` is marked invalid. The game reads
    /// this flag for every direction in the dead-end pass (see
    /// [`fill_dead_ends`]). Past the last column it reads the cleared cell
    /// that `ResetFloor` leaves there, so the answer is `false`.
    fn east_neighbour_invalid(&self, x: usize, y: usize) -> bool {
        x + 1 < self.grid_x && self.cell(x + 1, y).invalid
    }
}

/// Generation step 3 of a standard floor (`ov_0001::0214FB4C`): decides which
/// valid cells hold a room. Returns the number of rooms the first pass marked.
///
/// The target is `-room_density` rooms when the density is negative, else
/// `room_density` plus 0 to 2. A list of that many "room" slots, padded to 256,
/// is shuffled with 64 random swaps among the first `grid_x * grid_y` entries;
/// valid cells then take slots in column order. With an odd grid width, the
/// cell at row 1 of the middle column never holds a room (it still counts).
/// When fewer than two rooms result, valid cells are visited again in column
/// order, each becoming a room with a 60 % chance, until one does (at most 200
/// passes).
pub fn assign_rooms(grid: &mut Grid, room_density: i8, rng: &mut DungeonRng) -> usize {
    let bonus = rng.int(3);
    let target = if room_density < 0 {
        -i32::from(room_density)
    } else {
        i32::from(room_density) + bonus
    };
    let mut slots = [false; 256];
    for slot in slots.iter_mut().take(usize::try_from(target).unwrap_or(0)) {
        *slot = true;
    }
    let cells = i32::try_from(grid.grid_x * grid.grid_y).unwrap_or(i32::MAX);
    for _ in 0..64 {
        let a = usize::try_from(rng.int(cells)).unwrap_or(0);
        let b = usize::try_from(rng.int(cells)).unwrap_or(0);
        slots.swap(a, b);
    }

    let middle = (grid.grid_x.saturating_sub(1)) / 2;
    let mut rooms = 0;
    let mut next_slot = 0;
    for x in 0..grid.grid_x {
        for y in 0..grid.grid_y {
            let odd_width = grid.grid_x % 2 == 1;
            let cell = grid.cell_mut(x, y);
            if cell.invalid {
                continue;
            }
            cell.has_room = slots[next_slot];
            if cell.has_room {
                rooms += 1;
                if odd_width && x == middle && y == 1 {
                    cell.has_room = false;
                }
            }
            next_slot += 1;
        }
    }

    if rooms < 2 {
        'passes: for _ in 0..200 {
            for x in 0..grid.grid_x {
                for y in 0..grid.grid_y {
                    if !grid.cell(x, y).invalid && rng.outcome(60) {
                        grid.cell_mut(x, y).has_room = true;
                        break 'passes;
                    }
                }
            }
        }
    }
    rooms
}

/// Generation step 4 of a standard floor (`ov_0001::0214F7A8`): places a room
/// in every valid cell marked as holding one, and a hallway anchor in every
/// other valid cell. Cells are visited row by row. `xs` and `ys` are the cell
/// boundaries from [`grid_bounds`]; `props_0d` is floor-properties byte
/// `+0x0D` and `props_0c_set` whether floor-properties byte `+0x0C` is
/// non-zero (the game clears that one after the first generation attempt).
///
/// Inside a cell, the usable area starts 2 tiles in and is 4 tiles narrower
/// and 3 tiles shorter than the cell. A room is 5 or more tiles wide and 4 or
/// more tall, rounded up to an odd size when that still fits, and at most
/// 3 / 2 as long as it is wide either way; its tiles become floor numbered with
/// the room's index (rooms count from 0 in visiting order). An anchor is a
/// single floor tile, kept 2 tiles (1 at the map's top and left edges) from
/// the area's start and 4 tiles (2 at the bottom and right edges) from its end.
///
/// # Panics
/// If `xs` or `ys` is shorter than the grid needs.
pub fn place_rooms(
    floor: &mut Floor,
    grid: &mut Grid,
    xs: &[i32],
    ys: &[i32],
    props_0d: u8,
    props_0c_set: bool,
    rng: &mut DungeonRng,
) {
    let (grid_x, grid_y) = (grid.grid_x, grid.grid_y);
    let mut room_index: u8 = 0;
    for y in 0..grid_y {
        for x in 0..grid_x {
            let cell = *grid.cell(x, y);
            if cell.invalid {
                continue;
            }
            let (left, top) = (xs[x] + 2, ys[y] + 2);
            let width = xs[x + 1] - xs[x] - 4;
            let height = ys[y + 1] - ys[y] - 3;
            if !cell.has_room {
                let margin_left = if x == 0 { 1 } else { 2 };
                let margin_top = if y == 0 { 1 } else { 2 };
                let margin_right = if x == grid_x - 1 { 2 } else { 4 };
                let margin_bottom = if y == grid_y - 1 { 2 } else { 4 };
                let ax = rng.range(left + margin_left, left + width - margin_right);
                let ay = rng.range(top + margin_top, top + height - margin_bottom);
                set_bounds(grid.cell_mut(x, y), ax, ay, ax + 1, ay + 1);
                open_tile(floor, ax, ay, ANCHOR_ROOM);
                continue;
            }

            let mut room_w = rng.range(5, width);
            let mut room_h = rng.range(4, height);
            if room_w | 1 < width {
                room_w |= 1;
            }
            if room_h | 1 < height {
                room_h |= 1;
            }
            room_w = room_w.min(room_h * 3 / 2);
            room_h = room_h.min(room_w * 3 / 2);
            let start_x = left + rng.int(width - room_w);
            let start_y = top + rng.int(height - room_h);
            let (end_x, end_y) = (start_x + room_w, start_y + room_h);
            set_bounds(grid.cell_mut(x, y), start_x, start_y, end_x, end_y);
            for tx in start_x..end_x {
                for ty in start_y..end_y {
                    open_tile(floor, tx, ty, room_index);
                }
            }

            let mut flag_b = rng.int(100) < 80 && props_0c_set;
            let mut flag_a = props_0d & 0x4 != 0;
            if flag_a && flag_b {
                if rng.int(100) < 50 {
                    flag_a = false;
                } else {
                    flag_b = false;
                }
            }
            let cell = grid.cell_mut(x, y);
            cell.room_flag_a |= flag_a;
            cell.room_flag_b |= flag_b;
            room_index = room_index.wrapping_add(1);
        }
    }
}

/// Generation step 5 of a standard floor (`ov_0001::0214F6A0`, which calls
/// `ov_0001::0214F224`): links the valid cells into connected paths. Draws a
/// random start cell (`x` from `grid_x`, then `y` from `grid_y`) and walks from
/// it for `walk_steps` steps (floor-properties byte `+0x05`), then, unless
/// `skip_dead_ends` (byte `+0x0B` non-zero), links every dead end. Written from
/// the disassembly; see `docs/re/dungeon-generation.md`.
pub fn assign_connections(
    grid: &mut Grid,
    walk_steps: u8,
    skip_dead_ends: bool,
    rng: &mut DungeonRng,
) {
    let x = rng.int(i32::try_from(grid.grid_x).unwrap_or(i32::MAX));
    let y = rng.int(i32::try_from(grid.grid_y).unwrap_or(i32::MAX));
    let start = (
        usize::try_from(x).unwrap_or(0),
        usize::try_from(y).unwrap_or(0),
    );
    walk(grid, start, walk_steps, rng);
    if !skip_dead_ends {
        fill_dead_ends(grid, rng);
    }
}

/// The walk of step 5. Each step draws `DungeonRandInt(8)` and then
/// `DungeonRandInt(4)`; the new direction is used when the first draw is below
/// 4 (half the steps). The walker turns clockwise while the way is blocked by
/// the grid edge or an invalid cell, then links to the cell it moves to.
fn walk(grid: &mut Grid, start: (usize, usize), steps: u8, rng: &mut DungeonRng) {
    let (mut x, mut y) = start;
    let mut dir = Dir::from_index(rng.int(4));
    for _ in 0..steps {
        let turn = rng.int(8) < 4;
        let drawn = Dir::from_index(rng.int(4));
        if turn {
            dir = drawn;
        }
        // Four blocked sides at most; the game loops forever on a cell with
        // none. The grids the draw produces always have a usable neighbour.
        for _ in 0..4 {
            if let Some((nx, ny)) = grid.neighbour(x, y, dir)
                && !grid.cell(nx, ny).invalid
            {
                grid.connect(x, y, dir);
                (x, y) = (nx, ny);
                break;
            }
            dir = dir.next();
        }
    }
}

/// The dead-end pass of step 5. Passes run over the grid, column by column,
/// until one adds no link. A valid cell with no room and exactly one link
/// gets one more: from a random direction it tries up to eight directions
/// (turning clockwise) for one that is in the grid and not linked yet, then
/// links it only if the cell east of it is valid. Blue reads that east cell
/// for all four directions, not the one it links to; the port keeps that.
fn fill_dead_ends(grid: &mut Grid, rng: &mut DungeonRng) {
    loop {
        let mut changed = false;
        for x in 0..grid.grid_x {
            for y in 0..grid.grid_y {
                let cell = *grid.cell(x, y);
                if cell.invalid || cell.has_room || cell.link_count() != 1 {
                    continue;
                }
                let mut dir = Dir::from_index(rng.int(4));
                let mut found = None;
                for _ in 0..8 {
                    let open = grid.neighbour(x, y, dir).is_some() && !cell.link(dir);
                    if open {
                        found = Some(dir);
                        break;
                    }
                    dir = dir.next();
                }
                if let Some(dir) = found
                    && !grid.east_neighbour_invalid(x, y)
                {
                    grid.connect(x, y, dir);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn set_bounds(cell: &mut GridCell, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
    // The game stores the bounds as 16-bit values.
    let short = |v: i32| i16::try_from(v).unwrap_or(i16::MAX);
    cell.start_x = short(start_x);
    cell.start_y = short(start_y);
    cell.end_x = short(end_x);
    cell.end_y = short(end_y);
}

/// Makes `(x, y)` open floor belonging to `room`. Out-of-map positions are
/// ignored; the grid bounds keep step 4 inside the map.
fn open_tile(floor: &mut Floor, x: i32, y: i32, room: u8) {
    if let Some(tile) = floor.tile_mut(x, y) {
        tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
        tile.room = room;
    }
}

/// Grid draws allowed before `GenerateFloor` gives up and uses 4 × 4.
const MAX_GRID_DRAWS: usize = 32;

/// Layout attempts (the inner retry loop of `GenerateFloor`) before it falls
/// back to a one-room monster house.
pub const MAX_LAYOUT_ATTEMPTS: usize = 10;

/// Stairs attempts (the outer retry loop) before the same fallback.
pub const MAX_STAIRS_ATTEMPTS: usize = 10;

/// Floor tiles that must belong to rooms for an attempt to count.
pub const MIN_ROOM_TILES: usize = 30;

/// Distinct rooms that must exist for an attempt to count.
pub const MIN_ROOMS: usize = 2;

/// A layout generator that `GenerateFloor` calls instead of the standard one.
/// The variants are named after the Blue function each case calls; their own
/// RNG calls are not part of the prologue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Case 2: `GenerateOneRoomMonsterHouseFloor`.
    OneRoomMonsterHouse,
    /// Case 3: `GenerateOuterRingFloor`.
    OuterRing,
    /// Case 4: `GenerateCrossroadsFloor`.
    Crossroads,
    /// Case 5: `GenerateTwoRoomsWithMonsterHouseFloor`.
    TwoRoomsMonsterHouse,
    /// Case 6: `GenerateLineFloor`.
    Line,
    /// Case 7: `GenerateCrossFloor`.
    Cross,
    /// Case 9: `GenerateBeetleFloor`.
    Beetle,
}

/// What `GenerateFloor` decides before it calls a layout generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutPlan {
    /// `GenerateStandardFloor(grid_x, grid_y, props)`, with the cells in
    /// `mode` in use (cases 0, 8, 1 and 11, and the nibbles 12 to 15, which
    /// the switch sends to the standard generator).
    Standard {
        grid_x: usize,
        grid_y: usize,
        mode: GridMode,
    },
    /// `GenerateOuterRoomsFloor(grid_x, grid_y, props)` (case 10).
    OuterRooms { grid_x: usize, grid_y: usize },
    /// Another layout generator (cases 2 to 7 and 9).
    Other(Layout),
}

/// Draws the grid size `GenerateFloor` uses for the layout byte `layout`:
/// `DungeonRandRange(2, 9)` and `(2, 8)`, or `(2, 5)` and `(2, 4)` when the
/// byte is exactly 8. A draw is kept when `grid_x < 7` and `grid_y < 5`. The
/// game makes at most [`MAX_GRID_DRAWS`] draws (a counter starts at 32 and
/// goes down on each rejected draw); when the last one is rejected the grid is
/// 4 × 4.
fn draw_grid_size(layout: u8, rng: &mut DungeonRng) -> (usize, usize) {
    let (x_range, y_range) = if layout == 8 {
        ((2, 5), (2, 4))
    } else {
        ((2, 9), (2, 8))
    };
    for _ in 0..MAX_GRID_DRAWS {
        let x = usize::try_from(rng.range(x_range.0, x_range.1)).unwrap_or(0);
        let y = usize::try_from(rng.range(y_range.0, y_range.1)).unwrap_or(0);
        if x < 7 && y < 5 {
            return shrink_thin_cells(x, y);
        }
    }
    shrink_thin_cells(4, 4)
}

/// `GenerateFloor` makes a cell count of 1 on an axis whose cells would be
/// under 8 tiles (`56 / grid_x` or `32 / grid_y` under 8). After the draw
/// limits above no drawn size reaches this, but the game checks it anyway.
fn shrink_thin_cells(x: usize, y: usize) -> (usize, usize) {
    let x = if WIDTH / x <= 7 { 1 } else { x };
    let y = if HEIGHT / y <= 7 { 1 } else { y };
    (x, y)
}

/// Runs `GenerateFloor`'s grid-size draw and layout switch for the layout
/// byte `layout` (the value of `layout & 0xF` picks the case).
///
/// The RNG calls, in order: the grid size (see [`draw_grid_size`]), then, for
/// cases 1 and 11, one `DungeonRandInt(2)` that picks the grid height
/// `r + 2`. The other cases make no call here. Nibbles 12 to 15 take the
/// standard generator with the large grid, as the game's jump table does.
#[must_use]
pub fn plan_layout(layout: u8, rng: &mut DungeonRng) -> LayoutPlan {
    let (grid_x, grid_y) = draw_grid_size(layout, rng);
    let small_or_medium = |mode: GridMode, rng: &mut DungeonRng| LayoutPlan::Standard {
        grid_x: 4,
        grid_y: usize::try_from(rng.int(2)).unwrap_or(0) + 2,
        mode,
    };
    match layout & 0xF {
        1 => small_or_medium(GridMode::LeftHalf, rng),
        2 => LayoutPlan::Other(Layout::OneRoomMonsterHouse),
        3 => LayoutPlan::Other(Layout::OuterRing),
        4 => LayoutPlan::Other(Layout::Crossroads),
        5 => LayoutPlan::Other(Layout::TwoRoomsMonsterHouse),
        6 => LayoutPlan::Other(Layout::Line),
        7 => LayoutPlan::Other(Layout::Cross),
        9 => LayoutPlan::Other(Layout::Beetle),
        10 => LayoutPlan::OuterRooms { grid_x, grid_y },
        11 => small_or_medium(GridMode::LeftThreeQuarters, rng),
        _ => LayoutPlan::Standard {
            grid_x,
            grid_y,
            mode: GridMode::Full,
        },
    }
}

/// Whether a generated floor counts as an attempt: at least
/// [`MIN_ROOM_TILES`] floor tiles with a room number, and at least
/// [`MIN_ROOMS`] distinct room numbers below 64.
///
/// Read from `GenerateFloor`: a tile counts when its terrain type is floor
/// and its room byte is at most `0xF0` (so anchors, `0xFE`, and `NO_ROOM` do
/// not). Room numbers of 64 and above count as tiles but not as rooms.
#[must_use]
pub fn attempt_has_rooms(floor: &Floor) -> bool {
    let mut room_tiles = 0usize;
    let mut rooms = [false; 64];
    for y in 0..HEIGHT as i32 {
        for x in 0..WIDTH as i32 {
            let Some(tile) = floor.tile(x, y) else {
                continue;
            };
            if tile.terrain & TERRAIN_TYPE_MASK == TERRAIN_FLOOR && tile.room <= 0xF0 {
                room_tiles += 1;
                if let Some(seen) = rooms.get_mut(usize::from(tile.room)) {
                    *seen = true;
                }
            }
        }
    }
    let distinct = rooms.iter().filter(|seen| **seen).count();
    room_tiles >= MIN_ROOM_TILES && distinct >= MIN_ROOMS
}

/// Whether the stairs check of `GenerateFloor` accepts the floor. `first` and
/// `second` are the two stored positions (`-1` when unset, as the game
/// stores them); `floor_type` is `GetFloorType()`. A set first position with
/// floor type 1 is accepted outright; otherwise the second position must be
/// set and `reachable` (`StairsAlwaysReachable` with `markUnreachable` off)
/// must say yes.
pub fn stairs_check(
    first: (i16, i16),
    second: (i16, i16),
    floor_type: u8,
    reachable: impl FnOnce(i16, i16) -> bool,
) -> bool {
    let unset = |(x, y): (i16, i16)| x == -1 || y == -1;
    if unset(first) {
        return false;
    }
    if floor_type == 1 {
        return true;
    }
    !unset(second) && reachable(second.0, second.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_floor_has_a_wall_border() {
        let floor = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                let t = floor.tile(x, y).unwrap();
                let border = x == 0 || y == 0 || x == WIDTH as i32 - 1 || y == HEIGHT as i32 - 1;
                assert_eq!(
                    t.terrain & TERRAIN_IMPASSABLE_WALL != 0,
                    border,
                    "({x}, {y})"
                );
                assert_eq!(t.room, NO_ROOM);
            }
        }
        assert!(floor.tile(-1, 0).is_none());
        assert!(floor.tile(WIDTH as i32, 0).is_none());
        assert!(floor.stairs.is_none());
    }

    #[test]
    fn grid_bounds_small_layout() {
        // The first floor of a new game uses a 4 x 2 grid (verification/v7-trace/floor_stages.py).
        let (xs, ys) = grid_bounds(4, 2);
        assert_eq!(xs, [0, 14, 28, 42, 56]);
        assert_eq!(ys, [0, 16, 32]);
    }

    /// Dungeon RNG states measured on the retail ROM by
    /// `verification/v7-trace/floor_stages.py` on the first floor of a new
    /// game (small layout, 4-wide grid, room density 5), with the game's own
    /// seed or a seed forced at `GenerateFloor`: the grid height and the state
    /// at the entry of steps 3, 4 and 5 of the standard generator.
    struct Stages {
        grid_y: usize,
        std3: u32,
        std4: u32,
        std5: u32,
    }

    const NATURAL_SEED: Stages = Stages {
        grid_y: 2,
        std3: 1_410_305_108,
        std4: 2_131_318_437,
        std5: 2_134_860_938,
    };

    const FORCED_SEEDS: [Stages; 3] = [
        // 0x12345678
        Stages {
            grid_y: 2,
            std3: 985_678_915,
            std4: 1_990_207_216,
            std5: 3_860_814_012,
        },
        // 0xCAFEBABF
        Stages {
            grid_y: 2,
            std3: 1_477_956_308,
            std4: 436_971_301,
            std5: 3_196_431_495,
        },
        // 0x0BADF00D
        Stages {
            grid_y: 3,
            std3: 3_889_763_896,
            std4: 4_124_795_289,
            std5: 2_379_107_274,
        },
    ];

    /// Steps 1 to 4 of the first floor, from the state at step 3's entry.
    /// Returns the RNG state after steps 3 and 4.
    fn first_floor_steps(grid_y: usize, std3: u32) -> (u32, u32, Floor, Grid) {
        let mut rng = DungeonRng::from_state(std3);
        let mut floor = Floor::new();
        let (xs, ys) = grid_bounds(4, grid_y);
        let mut grid = Grid::new(4, grid_y, GridMode::LeftHalf);
        assign_rooms(&mut grid, 5, &mut rng);
        let after_std3 = rng.state();
        place_rooms(&mut floor, &mut grid, &xs, &ys, 0, false, &mut rng);
        (after_std3, rng.state(), floor, grid)
    }

    #[test]
    fn standard_steps_match_the_retail_rng_budget() {
        for (i, stages) in std::iter::once(NATURAL_SEED)
            .chain(FORCED_SEEDS)
            .enumerate()
        {
            let (after_std3, after_std4, _, _) = first_floor_steps(stages.grid_y, stages.std3);
            assert_eq!(after_std3, stages.std4, "case {i}, step 3");
            assert_eq!(after_std4, stages.std5, "case {i}, step 4");
        }
    }

    /// Walk length (floor-properties byte `+0x05`) of the first floor, pinned by
    /// the RNG budget: the walk draws two numbers a step and the dead-end pass
    /// draws none on these grids, so 33 steps need 15 walk steps.
    const FIRST_FLOOR_WALK_STEPS: u8 = 15;

    #[test]
    fn connections_match_the_retail_rng_budget() {
        for (i, stages) in std::iter::once(NATURAL_SEED)
            .chain(FORCED_SEEDS)
            .enumerate()
        {
            let (_, _, _, mut grid) = first_floor_steps(stages.grid_y, stages.std3);
            let mut rng = DungeonRng::from_state(stages.std5);
            let mut expect = rng;
            assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
            // Step 5 is 33 RNG steps on the first floor (std 5 in the table).
            for _ in 0..33 {
                expect.next_u16();
            }
            assert_eq!(rng.state(), expect.state(), "case {i}");
        }
    }

    #[test]
    fn connections_are_symmetric_and_leave_invalid_cells_once() {
        for stages in std::iter::once(NATURAL_SEED).chain(FORCED_SEEDS) {
            let (_, _, _, mut grid) = first_floor_steps(stages.grid_y, stages.std3);
            let mut rng = DungeonRng::from_state(stages.std5);
            assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
            for x in 0..grid.width() {
                for y in 0..grid.height() {
                    let cell = *grid.cell(x, y);
                    // The start cell is drawn over the whole grid, so an invalid
                    // one can link out once; the walk never enters one.
                    if cell.invalid {
                        assert!(cell.link_count() <= 1, "({x}, {y})");
                    }
                    for dir in [Dir::East, Dir::North, Dir::West, Dir::South] {
                        if !cell.link(dir) {
                            continue;
                        }
                        let (nx, ny) = grid.neighbour(x, y, dir).expect("link leaves the grid");
                        let other = *grid.cell(nx, ny);
                        assert!(other.link(dir.opposite()), "({x}, {y}) {dir:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn layout_prologue_reaches_standard_floor_entry() {
        // A forced seed is the dungeon RNG state at `GenerateFloor`'s entry, so
        // the prologue (grid size and the small-layout draw) must end in the
        // state recorded at `GenerateStandardFloor`'s entry (std3), with the
        // same grid height. The first floor of a new game uses layout byte 1.
        let cases = [
            (0x1234_5678, 2, 985_678_915, Some(7)),
            (0xCAFE_BABF, 2, 1_477_956_308, None),
            (0x0BAD_F00D, 3, 3_889_763_896, None),
        ];
        for (seed, grid_y, std3, steps) in cases {
            let mut rng = DungeonRng::from_state(seed);
            let plan = plan_layout(1, &mut rng);
            assert_eq!(
                plan,
                LayoutPlan::Standard {
                    grid_x: 4,
                    grid_y,
                    mode: GridMode::LeftHalf,
                },
                "seed {seed:#x}"
            );
            assert_eq!(rng.state(), std3, "seed {seed:#x}");
            if let Some(steps) = steps {
                // Count the RNG steps the prologue took.
                let mut counted = DungeonRng::from_state(seed);
                let mut n = 0;
                while counted.state() != std3 {
                    counted.next_u16();
                    n += 1;
                }
                assert_eq!(n, steps, "seed {seed:#x}");
            }
        }
    }

    #[test]
    fn layout_switch_cases() {
        // Cases that make no call beyond the grid draw.
        let mut rng = DungeonRng::new(1);
        assert_eq!(
            plan_layout(2, &mut rng),
            LayoutPlan::Other(Layout::OneRoomMonsterHouse)
        );
        assert_eq!(plan_layout(7, &mut rng), LayoutPlan::Other(Layout::Cross));
        assert!(matches!(
            plan_layout(10, &mut rng),
            LayoutPlan::OuterRooms { .. }
        ));
        // Nibbles 12 to 15 are not in the jump table; the game sends them to
        // the standard generator with the full grid.
        for nibble in 12..16u8 {
            assert!(matches!(
                plan_layout(nibble, &mut rng),
                LayoutPlan::Standard {
                    mode: GridMode::Full,
                    ..
                }
            ));
        }
    }

    #[test]
    fn grid_size_stays_small_and_large_layout_is_full() {
        for seed in 1..200u32 {
            let mut rng = DungeonRng::new(seed * 7919);
            match plan_layout(0, &mut rng) {
                LayoutPlan::Standard {
                    grid_x,
                    grid_y,
                    mode,
                } => {
                    assert!((2..9).contains(&grid_x) && (2..8).contains(&grid_y));
                    assert_eq!(mode, GridMode::Full);
                }
                other => panic!("seed {seed}: {other:?}"),
            }
            // Layout byte 8 draws from the smaller ranges.
            let mut rng = DungeonRng::new(seed * 7919);
            match plan_layout(8, &mut rng) {
                LayoutPlan::Standard { grid_x, grid_y, .. } => {
                    assert!((2..6).contains(&grid_x) && (2..5).contains(&grid_y));
                }
                other => panic!("seed {seed}: {other:?}"),
            }
        }
    }

    #[test]
    fn attempt_needs_rooms_and_room_tiles() {
        let mut floor = Floor::new();
        for x in 1..=15 {
            open_tile(&mut floor, x, 1, 0);
            open_tile(&mut floor, x, 2, 1);
        }
        // 30 tiles in two rooms: enough.
        assert!(attempt_has_rooms(&floor));

        // Anchors (0xFE) do not count as room tiles.
        let mut anchors = Floor::new();
        for x in 1..=15 {
            open_tile(&mut anchors, x, 1, ANCHOR_ROOM);
            open_tile(&mut anchors, x, 2, 0);
        }
        assert!(!attempt_has_rooms(&anchors));

        // One room only.
        let mut one = Floor::new();
        for x in 1..=15 {
            open_tile(&mut one, x, 1, 0);
            open_tile(&mut one, x, 2, 0);
        }
        assert!(!attempt_has_rooms(&one));

        // Two rooms but only 29 tiles.
        let mut short = Floor::new();
        for x in 1..=14 {
            open_tile(&mut short, x, 1, 0);
            open_tile(&mut short, x, 2, 1);
        }
        open_tile(&mut short, 15, 1, 0);
        assert!(!attempt_has_rooms(&short));
    }

    #[test]
    fn stairs_check_follows_the_game_order() {
        let unset = (-1, -1);
        let set = (5, 6);
        // A missing first position always fails, whatever the rest.
        assert!(!stairs_check(unset, set, 1, |_, _| true));
        // Floor type 1 accepts a set first position without a reachability test.
        assert!(stairs_check(set, unset, 1, |_, _| panic!("not called")));
        // Otherwise the second position must be set and reachable.
        assert!(!stairs_check(set, unset, 0, |_, _| true));
        assert!(!stairs_check(set, set, 0, |_, _| false));
        assert!(stairs_check(set, set, 0, |x, y| (x, y) == (5, 6)));
    }

    #[test]
    fn init_grid_marks_unused_columns() {
        let small = Grid::new(4, 2, GridMode::LeftHalf);
        let medium = Grid::new(4, 3, GridMode::LeftThreeQuarters);
        let large = Grid::new(5, 4, GridMode::Full);
        for y in 0..2 {
            let used: Vec<bool> = (0..4).map(|x| !small.cell(x, y).invalid).collect();
            assert_eq!(used, [true, true, false, false]);
        }
        for y in 0..3 {
            let used: Vec<bool> = (0..4).map(|x| !medium.cell(x, y).invalid).collect();
            assert_eq!(used, [true, true, true, false]);
        }
        assert!(large.cells.iter().all(|c| !c.invalid && c.has_room));
    }

    #[test]
    fn rooms_and_anchors_are_open_floor_inside_their_cells() {
        let (_, _, floor, grid) = first_floor_steps(2, NATURAL_SEED.std3);
        let (xs, ys) = grid_bounds(4, 2);
        let mut rooms = 0;
        for x in 0..grid.width() {
            for y in 0..grid.height() {
                let cell = grid.cell(x, y);
                if cell.invalid {
                    continue;
                }
                let (sx, sy) = (i32::from(cell.start_x), i32::from(cell.start_y));
                let (ex, ey) = (i32::from(cell.end_x), i32::from(cell.end_y));
                assert!(xs[x] < sx && ex < xs[x + 1] && ys[y] < sy && ey < ys[y + 1]);
                if cell.has_room {
                    rooms += 1;
                    assert!(ex - sx >= 5 && ey - sy >= 4, "({x}, {y})");
                } else {
                    assert_eq!((ex - sx, ey - sy), (1, 1));
                }
                let room = floor.tile(sx, sy).unwrap().room;
                assert_eq!(room == ANCHOR_ROOM, !cell.has_room);
                for tx in sx..ex {
                    for ty in sy..ey {
                        let tile = floor.tile(tx, ty).unwrap();
                        assert_eq!(tile.terrain & TERRAIN_TYPE_MASK, TERRAIN_FLOOR);
                        assert_eq!(tile.room, room);
                    }
                }
            }
        }
        // 17 RNG steps in step 4: three rooms (5 each) and one anchor (2).
        assert_eq!(rooms, 3);
    }

    #[test]
    fn too_few_rooms_forces_one() {
        // A density of -1 asks for exactly one room, so the fallback adds one.
        let mut grid = Grid::new(3, 3, GridMode::Full);
        let mut rng = DungeonRng::new(0x1234_5678);
        let first_pass = assign_rooms(&mut grid, -1, &mut rng);
        let rooms = grid.cells.iter().filter(|c| c.has_room).count();
        assert!(first_pass < 2);
        assert!(rooms >= 1 && rooms <= first_pass + 1, "{rooms}");
    }

    #[test]
    fn grid_bounds_round_down() {
        let (xs, ys) = grid_bounds(5, 3);
        assert_eq!(xs, [0, 11, 22, 33, 44, 55]);
        assert_eq!(ys, [0, 10, 20, 30]);
    }
}
