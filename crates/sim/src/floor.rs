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
    /// `+0x1C`: set in step 4 when floor-properties byte `+0x0D` has bit 2.
    /// Step 12 may cut the corners of the room; step 13 skips the cell.
    pub imperfect: bool,
    /// `+0x1D`: set in step 4 (80 % chance) when the floor-properties byte
    /// `+0x0C` is non-zero. Step 13 may build a secondary structure in the room.
    pub structure: bool,
    /// `+0x13`: the cell is linked to the cell north of it (`-y`). Step 5.
    pub north: bool,
    /// `+0x14`: linked to the cell south of it (`+y`). Step 5.
    pub south: bool,
    /// `+0x15`: linked to the cell west of it (`-x`). Step 5.
    pub west: bool,
    /// `+0x16`: linked to the cell east of it (`+x`). Step 5.
    pub east: bool,
    /// `+0x09`: flag not known yet; the extra-link pass of step 6 skips cells
    /// with it set. Nothing in steps 1 to 5 sets it.
    pub flag_09: bool,
    /// `+0x0F`: flag not known yet; step 7 only drops the room of an unlinked
    /// cell when it is clear. Nothing in steps 1 to 7 sets it.
    pub flag_0f: bool,
    /// `+0x0B`: set by step 6 when a hallway is dug to or from the cell.
    /// The extra-link pass needs it set on both cells.
    pub hallway_mark: bool,
    /// `+0x11`: set on the cell that the extra-link pass of step 6 links
    /// (the role of the flag is not known yet).
    pub extra_mark: bool,
    /// `+0x12`: set by the extra-link pass on both linked cells; the pass
    /// skips cells with it set.
    pub extra_linked: bool,
    /// `+0x0C`: the cell is the shop room (step 9).
    pub shop: bool,
    /// `+0x0E`: the cell is the monster house (step 10).
    pub monster_house: bool,
    /// `+0x10`: the cell is a maze room (step 8).
    pub maze: bool,
    /// `+0x17`: a hallway still to dig to the north. Copied from `north` at the
    /// start of step 6 and cleared when the hallway is dug.
    pub dig_north: bool,
    /// `+0x18`: a hallway still to dig to the south (copy of `south`).
    pub dig_south: bool,
    /// `+0x19`: a hallway still to dig to the west (copy of `west`).
    pub dig_west: bool,
    /// `+0x1A`: a hallway still to dig to the east (copy of `east`).
    pub dig_east: bool,
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
            cell.imperfect |= flag_a;
            cell.structure |= flag_b;
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
/// the grid edge, then links to the cell it moves to. An invalid cell in that
/// direction ends the step without a move and without a turn.
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
            if let Some((nx, ny)) = grid.neighbour(x, y, dir) {
                if !grid.cell(nx, ny).invalid {
                    grid.connect(x, y, dir);
                    (x, y) = (nx, ny);
                }
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

/// Largest number of tiles one hallway segment digs (the game's counter).
const MAX_SEGMENT_TILES: usize = 56;

/// Generation step 6 of a standard floor (`ov_0001::0214E824`, which calls
/// `ov_0001::0214E0C4` to dig each hallway). Digs the hallways between cells
/// that step 5 linked, then (unless `skip_extra`) a random pass that links
/// some more pairs of room cells. `xs` and `ys` are the cell boundaries from
/// [`grid_bounds`]. Written from the disassembly; see
/// `docs/re/dungeon-generation.md`.
pub fn create_hallways(
    floor: &mut Floor,
    grid: &mut Grid,
    xs: &[i32],
    ys: &[i32],
    skip_extra: bool,
    rng: &mut DungeonRng,
) {
    let (grid_x, grid_y) = (grid.grid_x, grid.grid_y);
    // Each valid cell takes a copy of its links as the hallways still to dig,
    // after the map edge clears the links that point off the grid.
    for x in 0..grid_x {
        for y in 0..grid_y {
            let cell = grid.cell_mut(x, y);
            if cell.invalid {
                cell.dig_north = false;
                cell.dig_south = false;
                cell.dig_west = false;
                cell.dig_east = false;
                continue;
            }
            if y == 0 {
                cell.north = false;
            }
            if y + 1 >= grid_y {
                cell.south = false;
            }
            if x == 0 {
                cell.west = false;
            }
            if x + 1 >= grid_x {
                cell.east = false;
            }
            cell.dig_north = cell.north;
            cell.dig_south = cell.south;
            cell.dig_west = cell.west;
            cell.dig_east = cell.east;
        }
    }

    for x in 0..grid_x {
        for y in 0..grid_y {
            if grid.cell(x, y).invalid {
                continue;
            }
            let c = *grid.cell(x, y);
            let x0 = anchor_x(&c, rng);
            let y0 = anchor_y(&c, rng);
            if c.dig_north {
                let n = *grid.cell(x, y - 1);
                if !n.invalid {
                    let x1 = anchor_x(&n, rng);
                    dig_hallway(
                        floor,
                        (x0, i32::from(c.start_y)),
                        (x1, i32::from(n.end_y) - 1),
                        true,
                        xs[x],
                        ys[y],
                    );
                }
                grid.cell_mut(x, y).dig_north = false;
                grid.cell_mut(x, y).hallway_mark = true;
                let n = grid.cell_mut(x, y - 1);
                n.dig_south = false;
                n.hallway_mark = true;
            }
            if c.dig_south {
                let s = *grid.cell(x, y + 1);
                if !s.invalid {
                    let x1 = anchor_x(&s, rng);
                    dig_hallway(
                        floor,
                        (x0, i32::from(c.end_y) - 1),
                        (x1, i32::from(s.start_y)),
                        true,
                        xs[x],
                        ys[y + 1] - 1,
                    );
                }
                grid.cell_mut(x, y).dig_south = false;
                grid.cell_mut(x, y).hallway_mark = true;
                let s = grid.cell_mut(x, y + 1);
                s.dig_north = false;
                s.hallway_mark = true;
            }
            if c.dig_west {
                let w = *grid.cell(x - 1, y);
                if !w.invalid {
                    let y1 = anchor_y(&w, rng);
                    dig_hallway(
                        floor,
                        (i32::from(c.start_x), y0),
                        (i32::from(w.start_x) - 1, y1),
                        false,
                        xs[x],
                        ys[y],
                    );
                }
                grid.cell_mut(x, y).dig_west = false;
                grid.cell_mut(x, y).hallway_mark = true;
                let w = grid.cell_mut(x - 1, y);
                w.dig_east = false;
                w.hallway_mark = true;
            }
            if c.dig_east {
                let e = *grid.cell(x + 1, y);
                if !e.invalid {
                    let y1 = anchor_y(&e, rng);
                    dig_hallway(
                        floor,
                        (i32::from(c.end_x) - 1, y0),
                        (i32::from(e.start_x), y1),
                        false,
                        xs[x + 1] - 1,
                        ys[y],
                    );
                }
                grid.cell_mut(x, y).dig_east = false;
                grid.cell_mut(x, y).hallway_mark = true;
                let e = grid.cell_mut(x + 1, y);
                e.dig_west = false;
                e.hallway_mark = true;
            }
        }
    }

    if skip_extra {
        return;
    }
    for x in 0..grid_x {
        for y in 0..grid_y {
            // The game draws once per cell here, before it checks the cell.
            if !rng.outcome(5) {
                continue;
            }
            let c = *grid.cell(x, y);
            if c.invalid || !c.hallway_mark || c.extra_linked || c.flag_09 || !c.has_room {
                continue;
            }
            match rng.int(4) {
                0 => extra_west(floor, grid, x, y),
                1 => extra_north(floor, grid, x, y),
                2 => extra_east(floor, grid, x, y),
                _ => extra_south(floor, grid, x, y),
            }
        }
    }
}

/// Whether a neighbour may take an extra link in step 6 (the checks of
/// `ov_0001::0214E824`'s second part, shared by the four directions).
fn extra_link_allowed(cell: &GridCell) -> bool {
    !cell.invalid && cell.hallway_mark && cell.has_room && !cell.flag_09 && !cell.extra_linked
}

/// The room bound of a cell's start tile, read as the room that the extra
/// link paints over its rectangle.
fn room_at_cell_start(floor: &Floor, cell: &GridCell) -> u8 {
    floor
        .tile(i32::from(cell.start_x), i32::from(cell.start_y))
        .map_or(NO_ROOM, |tile| tile.room)
}

/// Paints the tiles `[x0, x1) x [y0, y1)` as floor of `room`.
fn paint_floor(floor: &mut Floor, (x0, x1): (i32, i32), (y0, y1): (i32, i32), room: u8) {
    for x in x0..x1 {
        for y in y0..y1 {
            if let Some(tile) = floor.tile_mut(x, y) {
                tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
                tile.room = room;
            }
        }
    }
}

/// The extra link to the west: cell `(x - 1, y)` takes the bounds of both
/// cells (the start column stays, the end moves to this cell's end).
fn extra_west(floor: &mut Floor, grid: &mut Grid, x: usize, y: usize) {
    if x < 1 {
        return;
    }
    let other = *grid.cell(x - 1, y);
    if !extra_link_allowed(&other) {
        return;
    }
    let c = *grid.cell(x, y);
    let room = room_at_cell_start(floor, &c);
    let y0 = i32::from(other.start_y).min(i32::from(c.start_y));
    let y1 = i32::from(other.end_y).max(i32::from(c.end_y));
    paint_floor(
        floor,
        (i32::from(other.start_x), i32::from(c.end_x)),
        (y0, y1),
        room,
    );
    let o = grid.cell_mut(x - 1, y);
    o.end_x = c.end_x;
    o.start_y = y0 as i16;
    o.end_y = y1 as i16;
    o.extra_linked = true;
    let c = grid.cell_mut(x, y);
    c.extra_linked = true;
    c.hallway_mark = false;
    c.extra_mark = true;
}

/// The extra link to the north: cell `(x, y - 1)` takes the bounds of both
/// cells.
fn extra_north(floor: &mut Floor, grid: &mut Grid, x: usize, y: usize) {
    if y < 1 {
        return;
    }
    let other = *grid.cell(x, y - 1);
    if !extra_link_allowed(&other) {
        return;
    }
    let c = *grid.cell(x, y);
    let room = room_at_cell_start(floor, &c);
    let x0 = i32::from(other.start_x).min(i32::from(c.start_x));
    let x1 = i32::from(other.end_x).max(i32::from(c.end_x));
    paint_floor(
        floor,
        (x0, x1),
        (i32::from(other.start_y), i32::from(c.end_y)),
        room,
    );
    let o = grid.cell_mut(x, y - 1);
    o.start_x = x0 as i16;
    o.end_x = x1 as i16;
    o.end_y = c.end_y;
    o.extra_linked = true;
    let c = grid.cell_mut(x, y);
    c.extra_linked = true;
    c.hallway_mark = false;
    c.extra_mark = true;
}

/// The extra link to the east: this cell takes the bounds of both cells.
fn extra_east(floor: &mut Floor, grid: &mut Grid, x: usize, y: usize) {
    if x + 1 >= grid.grid_x {
        return;
    }
    let other = *grid.cell(x + 1, y);
    if !extra_link_allowed(&other) {
        return;
    }
    let c = *grid.cell(x, y);
    let room = room_at_cell_start(floor, &c);
    let y0 = i32::from(c.start_y).min(i32::from(other.start_y));
    let y1 = i32::from(c.end_y).max(i32::from(other.end_y));
    paint_floor(
        floor,
        (i32::from(c.start_x), i32::from(other.end_x)),
        (y0, y1),
        room,
    );
    let c = grid.cell_mut(x, y);
    c.end_x = other.end_x;
    c.start_y = y0 as i16;
    c.end_y = y1 as i16;
    c.extra_linked = true;
    let o = grid.cell_mut(x + 1, y);
    o.extra_linked = true;
    o.hallway_mark = false;
    o.extra_mark = true;
}

/// The extra link to the south: this cell takes the bounds of both cells.
fn extra_south(floor: &mut Floor, grid: &mut Grid, x: usize, y: usize) {
    if y + 1 >= grid.grid_y {
        return;
    }
    let other = *grid.cell(x, y + 1);
    if !extra_link_allowed(&other) {
        return;
    }
    let c = *grid.cell(x, y);
    let room = room_at_cell_start(floor, &c);
    let x0 = i32::from(c.start_x).min(i32::from(other.start_x));
    let x1 = i32::from(c.end_x).max(i32::from(other.end_x));
    paint_floor(
        floor,
        (x0, x1),
        (i32::from(c.start_y), i32::from(other.end_y)),
        room,
    );
    let c = grid.cell_mut(x, y);
    c.start_x = x0 as i16;
    c.end_x = x1 as i16;
    c.end_y = other.end_y;
    c.extra_linked = true;
    let o = grid.cell_mut(x, y + 1);
    o.extra_linked = true;
    o.hallway_mark = false;
    o.extra_mark = true;
}

/// Whether a cell can take a hallway from an unlinked neighbour in step 7:
/// valid, already linked by step 6, and not part of an extra link.
fn can_join(cell: &GridCell) -> bool {
    !cell.invalid && cell.hallway_mark && !cell.extra_linked
}

/// A cell's anchor on the x axis for step 7, and its y axis draw.
/// A cell with a room draws x then y; one without takes its start tile.
fn join_anchor(cell: &GridCell, rng: &mut DungeonRng) -> (i32, i32) {
    if cell.has_room {
        let x = anchor_x(cell, rng);
        (x, anchor_y(cell, rng))
    } else {
        (i32::from(cell.start_x), i32::from(cell.start_y))
    }
}

/// Turns a tile into wall and clears its low spawn markers, as step 7 does to
/// the anchor of a cell that stays unlinked.
fn wall_off(floor: &mut Floor, x: i32, y: i32) {
    if let Some(tile) = floor.tile_mut(x, y) {
        tile.terrain &= !TERRAIN_TYPE_MASK;
        tile.spawn_or_visibility &= !0x7;
    }
}

/// Generation step 7 of a standard floor (`ov_0001::0214DB54`, `EnsureConnectedGrid`
/// in `pmd-red`). Finds valid cells that step 6 left without a hallway and joins
/// each to the first neighbour that has one (north, south, west, east), digging
/// a hallway with the same routine as step 6. A cell that cannot join keeps its
/// place: an anchor loses its tile, and (second pass) a room is erased unless
/// `flag_0f` is set. `xs` and `ys` are the cell boundaries from [`grid_bounds`].
/// Written from the disassembly; see `docs/re/dungeon-generation.md`.
pub fn ensure_connected_grid(
    floor: &mut Floor,
    grid: &mut Grid,
    xs: &[i32],
    ys: &[i32],
    rng: &mut DungeonRng,
) {
    let (grid_x, grid_y) = (grid.grid_x, grid.grid_y);
    for x in 0..grid_x {
        for y in 0..grid_y {
            let c = *grid.cell(x, y);
            if c.invalid || c.extra_mark || c.hallway_mark {
                continue;
            }
            if !c.has_room || c.flag_09 {
                // No room, or a flagged one: the cell's start tile goes back to wall.
                wall_off(floor, i32::from(c.start_x), i32::from(c.start_y));
                continue;
            }
            let x0 = anchor_x(&c, rng);
            let y0 = anchor_y(&c, rng);
            let join = if y > 0 && can_join(grid.cell(x, y - 1)) {
                let n = *grid.cell(x, y - 1);
                let (x1, _) = join_anchor(&n, rng);
                dig_hallway(
                    floor,
                    (x0, i32::from(c.start_y)),
                    (x1, i32::from(n.end_y) - 1),
                    true,
                    xs[x],
                    ys[y],
                );
                Some(Dir::North)
            } else if y + 1 < grid_y && can_join(grid.cell(x, y + 1)) {
                let s = *grid.cell(x, y + 1);
                let (x1, _) = join_anchor(&s, rng);
                dig_hallway(
                    floor,
                    (x0, i32::from(c.end_y) - 1),
                    (x1, i32::from(s.start_y)),
                    true,
                    xs[x],
                    ys[y + 1] - 1,
                );
                Some(Dir::South)
            } else if x > 0 && can_join(grid.cell(x - 1, y)) {
                let w = *grid.cell(x - 1, y);
                let (_, y1) = join_anchor(&w, rng);
                dig_hallway(
                    floor,
                    (i32::from(c.start_x), y0),
                    (i32::from(w.start_x) - 1, y1),
                    false,
                    xs[x],
                    ys[y],
                );
                Some(Dir::West)
            } else if x + 1 < grid_x && can_join(grid.cell(x + 1, y)) {
                let e = *grid.cell(x + 1, y);
                let (_, y1) = join_anchor(&e, rng);
                dig_hallway(
                    floor,
                    (i32::from(c.end_x) - 1, y0),
                    (i32::from(e.start_x), y1),
                    false,
                    xs[x + 1] - 1,
                    ys[y],
                );
                Some(Dir::East)
            } else {
                None
            };
            if let Some(dir) = join {
                let (nx, ny) = grid
                    .neighbour(x, y, dir)
                    .expect("a joinable neighbour is in the grid");
                let cell = grid.cell_mut(x, y);
                cell.hallway_mark = true;
                cell.set_link(dir);
                grid.cell_mut(nx, ny).set_link(dir.opposite());
            }
        }
    }

    // Cells still unlinked lose their room.
    for x in 0..grid_x {
        for y in 0..grid_y {
            let c = *grid.cell(x, y);
            if c.invalid || c.extra_mark || c.hallway_mark || c.flag_0f {
                continue;
            }
            for tx in i32::from(c.start_x)..i32::from(c.end_x) {
                for ty in i32::from(c.start_y)..i32::from(c.end_y) {
                    if let Some(tile) = floor.tile_mut(tx, ty) {
                        tile.terrain &= !TERRAIN_TYPE_MASK;
                        tile.spawn_or_visibility &= !0x7;
                        tile.room = NO_ROOM;
                    }
                }
            }
        }
    }
}

/// Start of a cell's anchor on the x axis: a random point inside the room, or
/// the cell's start when it has no room. Draws only for a room.
fn anchor_x(cell: &GridCell, rng: &mut DungeonRng) -> i32 {
    if cell.has_room {
        rng.range(i32::from(cell.start_x) + 1, i32::from(cell.end_x) - 1)
    } else {
        i32::from(cell.start_x)
    }
}

/// The y counterpart of [`anchor_x`].
fn anchor_y(cell: &GridCell, rng: &mut DungeonRng) -> i32 {
    if cell.has_room {
        rng.range(i32::from(cell.start_y) + 1, i32::from(cell.end_y) - 1)
    } else {
        i32::from(cell.start_y)
    }
}

/// Digs one hallway (`ov_0001::0214E0C4`). With `vertical_first` it goes down
/// the column of `from` to row `ty`, along that row to the column of `to`, then
/// down to `to`. Otherwise it goes along the row of `from` to column `tx`, down
/// that column, then along to `to`. Digging stops early at a floor tile that
/// is not the start point, and the rest of the hallway is left out.
fn dig_hallway(
    floor: &mut Floor,
    from: (i32, i32),
    to: (i32, i32),
    vertical_first: bool,
    tx: i32,
    ty: i32,
) {
    let (x0, y0) = from;
    let (x1, y1) = to;
    // The first segment that stops ends the hallway; `&&` skips the rest.
    let _ = if vertical_first {
        dig_segment(floor, from, (x0, ty), from)
            && dig_segment(floor, (x0, ty), (x1, ty), from)
            && dig_segment(floor, (x1, ty), to, from)
    } else {
        dig_segment(floor, from, (tx, y0), from)
            && dig_segment(floor, (tx, y0), (tx, y1), from)
            && dig_segment(floor, (tx, y1), to, from)
    };
}

/// One straight run of [`dig_hallway`], from `start` to `end` (exclusive of
/// `end`). Returns false when the run stopped on floor, which ends the whole
/// hallway.
fn dig_segment(floor: &mut Floor, start: (i32, i32), end: (i32, i32), origin: (i32, i32)) -> bool {
    let (mut x, mut y) = start;
    let mut dug = 0;
    while (x, y) != end {
        if dug >= MAX_SEGMENT_TILES {
            return false;
        }
        dug += 1;
        let is_floor = floor
            .tile(x, y)
            .is_some_and(|tile| tile.terrain & TERRAIN_TYPE_MASK == TERRAIN_FLOOR);
        if !is_floor {
            if let Some(tile) = floor.tile_mut(x, y) {
                tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
            }
        } else if (x, y) != origin {
            return false;
        }
        if x != end.0 {
            x += if x < end.0 { 1 } else { -1 };
        } else {
            y += if y < end.1 { 1 } else { -1 };
        }
    }
    true
}

/// Terrain flag set on the tiles inside the shop room (step 9).
pub const TERRAIN_SHOP: u16 = 0x20;
/// Terrain flag set on the tiles of the monster house (step 10).
pub const TERRAIN_MONSTER_HOUSE: u16 = 0x40;

/// Floor-wide state that steps 8 to 10 read and write besides the grid: bytes
/// and words in overlay 1's data (`021B458C`...) and the dungeon struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomFeatures {
    /// `021B4590`: step 8 made a maze room.
    pub maze: bool,
    /// `021B4594`: the floor has a monster house. Step 10 sets it and step 9
    /// skips the shop when it is set; the other writers are not known yet.
    pub monster_house: bool,
    /// `021B458C`: step 9 made a shop; step 10 then makes no monster house.
    pub shop: bool,
    /// `021B45CC`: the shop's open area as `[x0, y0, x1, y1]`, ends excluded.
    pub shop_rect: [i32; 4],
    /// `021B45C8`: the middle of the shop area, `0xFFFF` twice without a shop.
    pub shop_centre: (u16, u16),
    /// Dungeon `+0xC0E0`: smallest x and y, largest x and y of the shop area.
    pub shop_bounds: [i32; 4],
    /// Dungeon `+0x34AC`: room number of the monster house ([`NO_ROOM`] without one).
    pub monster_house_room: u8,
}

impl Default for RoomFeatures {
    fn default() -> Self {
        Self {
            maze: false,
            monster_house: false,
            shop: false,
            shop_rect: [-1; 4],
            shop_centre: (0xFFFF, 0xFFFF),
            shop_bounds: [0; 4],
            monster_house_room: NO_ROOM,
        }
    }
}

/// Index of the set slot of the shuffled one-hot list of steps 8 and 10: the
/// list starts with a single `true` at index 0, and 64 swaps of two draws of
/// `DungeonRandInt(count)` move it. The index picks among `count` candidates.
fn shuffled_pick(count: usize, rng: &mut DungeonRng) -> usize {
    let cap = i32::try_from(count).unwrap_or(i32::MAX);
    let mut slots = [false; 256];
    slots[0] = true;
    for _ in 0..64 {
        let a = usize::try_from(rng.int(cap)).unwrap_or(0);
        let b = usize::try_from(rng.int(cap)).unwrap_or(0);
        slots.swap(a, b);
    }
    slots.iter().position(|&set| set).expect("one slot is set")
}

/// Cells in column order (x outer, y inner) that pass `test`.
fn candidates(grid: &Grid, test: impl Fn(&GridCell) -> bool) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for x in 0..grid.grid_x {
        for y in 0..grid.grid_y {
            if test(grid.cell(x, y)) {
                found.push((x, y));
            }
        }
    }
    found
}

/// Width and height of a cell's bounds.
fn span(cell: &GridCell) -> (i32, i32) {
    (
        i32::from(cell.end_x) - i32::from(cell.start_x),
        i32::from(cell.end_y) - i32::from(cell.start_y),
    )
}

/// Terrain type at `(x, y)`; outside the map it reads as wall.
fn terrain_type(floor: &Floor, x: i32, y: i32) -> u16 {
    floor
        .tile(x, y)
        .map_or(0, |t| t.terrain & TERRAIN_TYPE_MASK)
}

fn make_wall(floor: &mut Floor, x: i32, y: i32) {
    if let Some(tile) = floor.tile_mut(x, y) {
        tile.terrain &= !TERRAIN_TYPE_MASK;
    }
}

/// Terrain type of the secondary terrain (water, lava or a chasm, by tileset).
const TERRAIN_SECONDARY: u16 = 0x2;

/// Makes `(x, y)` the secondary terrain, with the extra terrain bits `extra`
/// (`ov_0001::0214CA90`).
fn make_secondary(floor: &mut Floor, x: i32, y: i32, extra: u16) {
    if let Some(tile) = floor.tile_mut(x, y) {
        tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_SECONDARY | extra;
    }
}

/// How a maze turns its paths into wall: plain wall, or, when the maze belongs
/// to a secondary structure (step 13), the secondary terrain on the tiles of
/// the given room (`ov_0001::0214DB18`).
#[derive(Clone, Copy)]
enum MazeWall {
    Wall,
    SecondaryIn(u8),
}

impl MazeWall {
    fn build(self, floor: &mut Floor, x: i32, y: i32) {
        make_wall(floor, x, y);
        if let Self::SecondaryIn(room) = self
            && floor.tile(x, y).is_some_and(|t| t.room == room)
        {
            make_secondary(floor, x, y, 0);
        }
    }
}

/// Offsets of a maze step in the order of the direction numbers 0 to 3 (east,
/// north, west, south): two tiles, with the tile between walled.
const MAZE_STEPS: [(i32, i32); 4] = [(2, 0), (0, -2), (-2, 0), (0, 2)];

/// Walls off a path through a room (`ov_0001::0214CAB8`). From `start` it
/// draws a direction (`DungeonRandInt(4)`), turns the tile it stands on into
/// wall, and takes the first of the four directions, from the drawn one on and
/// turning by +1, whose tile two steps away is inside `bounds`
/// (`[x0, y0, x1, y1]`, ends excluded) and still floor. It walls the tile
/// between, moves there and draws again. It ends when no direction works.
fn carve_maze_walls(
    floor: &mut Floor,
    rng: &mut DungeonRng,
    wall: MazeWall,
    start: (i32, i32),
    bounds: [i32; 4],
) {
    let [x0, y0, x1, y1] = bounds;
    let (mut x, mut y) = start;
    loop {
        let first = rng.int(4);
        wall.build(floor, x, y);
        let step = (0..4)
            .map(|turn| MAZE_STEPS[usize::try_from((first + turn) & 3).unwrap_or(0)])
            .find(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                (x0..x1).contains(&nx)
                    && (y0..y1).contains(&ny)
                    && terrain_type(floor, nx, ny) == TERRAIN_FLOOR
            });
        let Some((dx, dy)) = step else { return };
        wall.build(floor, x + dx / 2, y + dy / 2);
        x += dx;
        y += dy;
    }
}

/// Turns the room of `cell` into a maze (`ov_0001::0214CC7C`; its second
/// argument is 0 for [`MazeWall::Wall`], 1 for [`MazeWall::SecondaryIn`]): marks
/// the cell, then starts a wall path from every
/// non-floor tile along a side of the room at every second position (north,
/// east, south, west), and from every floor tile of the room's inner lattice.
fn make_maze(
    floor: &mut Floor,
    cell: &mut GridCell,
    features: &mut RoomFeatures,
    wall: MazeWall,
    rng: &mut DungeonRng,
) {
    features.maze = true;
    cell.maze = true;
    let bounds = [
        i32::from(cell.start_x),
        i32::from(cell.start_y),
        i32::from(cell.end_x),
        i32::from(cell.end_y),
    ];
    let [x0, y0, x1, y1] = bounds;
    for x in (x0 + 1..x1 - 1).step_by(2) {
        if terrain_type(floor, x, y0 - 1) != TERRAIN_FLOOR {
            carve_maze_walls(floor, rng, wall, (x, y0 - 1), bounds);
        }
    }
    for y in (y0 + 1..y1 - 1).step_by(2) {
        if terrain_type(floor, x1, y) != TERRAIN_FLOOR {
            carve_maze_walls(floor, rng, wall, (x1, y), bounds);
        }
    }
    for x in (x0 + 1..x1 - 1).step_by(2) {
        if terrain_type(floor, x, y1) != TERRAIN_FLOOR {
            carve_maze_walls(floor, rng, wall, (x, y1), bounds);
        }
    }
    for y in (y0 + 1..y1 - 1).step_by(2) {
        if terrain_type(floor, x0 - 1, y) != TERRAIN_FLOOR {
            carve_maze_walls(floor, rng, wall, (x0 - 1, y), bounds);
        }
    }
    for x in (x0 + 3..x1 - 3).step_by(2) {
        for y in (y0 + 3..y1 - 3).step_by(2) {
            if terrain_type(floor, x, y) == TERRAIN_FLOOR {
                match wall {
                    MazeWall::Wall => make_wall(floor, x - 1, y),
                    MazeWall::SecondaryIn(_) => make_secondary(floor, x - 1, y, 0),
                }
                carve_maze_walls(floor, rng, wall, (x, y), bounds);
            }
        }
    }
}

/// Generation step 8 of a standard floor (`ov_0001::0214CF2C`,
/// `GenerateMazeRoom` in `pmd-red`): with `chance` percent (one draw of
/// `DungeonRandInt(100)`, none when `chance` is 0), and only if
/// `attempt_negative` (dungeon `+0x34B6 < 0`, checked after the draw), turns
/// one room into a maze. The candidates are valid, hallway-linked room cells
/// of odd width and height with no feature yet; a shuffled one-hot list picks
/// one. Returns whether a maze was made. Written from the disassembly; see
/// `docs/re/dungeon-generation.md`.
pub fn generate_maze_room(
    floor: &mut Floor,
    grid: &mut Grid,
    chance: i32,
    attempt_negative: bool,
    features: &mut RoomFeatures,
    rng: &mut DungeonRng,
) -> bool {
    if chance == 0 || !rng.outcome(chance) || !attempt_negative {
        return false;
    }
    let found = candidates(grid, |c| {
        let (w, h) = span(c);
        !c.invalid
            && !c.extra_mark
            && c.hallway_mark
            && c.has_room
            && !c.shop
            && !c.flag_0f
            && !c.monster_house
            && !c.flag_09
            && w & 1 != 0
            && h & 1 != 0
    });
    if found.is_empty() {
        return false;
    }
    let (x, y) = found[shuffled_pick(found.len(), rng)];
    let mut cell = *grid.cell(x, y);
    make_maze(floor, &mut cell, features, MazeWall::Wall, rng);
    *grid.cell_mut(x, y) = cell;
    true
}

/// Generation step 9 of a standard floor (`ov_0001::0214D448`,
/// `GenerateKecleonShop` in `pmd-red`). It always resets the shop centre to
/// `0xFFFF`. Then, unless the floor has a monster house or `floor_type` is 2,
/// and with `chance` percent (one draw), it shuffles two lists of the numbers
/// 0 to 14 (200 swaps each, two draws of `DungeonRandInt(15)` per swap) and
/// scans the pairs (column from the first list, row from the second, both
/// inside the grid) for the first valid, hallway-linked room cell more than 4
/// wide and 3 tall with no maze, `structure` or link marks. That cell
/// becomes the shop: its area (one tile in from the room's start and end, and
/// at least 3 rows tall) gets [`TERRAIN_SHOP`] and loses spawn bits `0x8` and
/// `0x1`, and every room tile gets spawn bit `0x10`. Returns whether a shop
/// was made. Written from the disassembly; see `docs/re/dungeon-generation.md`.
pub fn generate_shop(
    floor: &mut Floor,
    grid: &mut Grid,
    chance: i32,
    floor_type: u8,
    features: &mut RoomFeatures,
    rng: &mut DungeonRng,
) -> bool {
    features.shop_centre = (0xFFFF, 0xFFFF);
    if features.monster_house || floor_type == 2 || chance == 0 || !rng.outcome(chance) {
        return false;
    }
    let mut shuffled = || {
        let mut list: [usize; 15] = std::array::from_fn(|i| i);
        for _ in 0..200 {
            let a = usize::try_from(rng.int(15)).unwrap_or(0);
            let b = usize::try_from(rng.int(15)).unwrap_or(0);
            list.swap(a, b);
        }
        list
    };
    let columns = shuffled();
    let rows = shuffled();
    let fits = |c: &GridCell| {
        let (w, h) = span(c);
        !c.invalid
            && !c.extra_mark
            && !c.extra_linked
            && c.hallway_mark
            && c.has_room
            && !c.flag_09
            && !c.maze
            && !c.structure
            && w.abs() > 4
            && h.abs() > 3
    };
    let found = columns
        .iter()
        .filter(|&&x| x < grid.grid_x)
        .flat_map(|&x| rows.iter().map(move |&y| (x, y)))
        .find(|&(x, y)| y < grid.grid_y && fits(grid.cell(x, y)));
    let Some((x, y)) = found else { return false };

    let cell = grid.cell_mut(x, y);
    features.shop = true;
    cell.shop = true;
    let (x0, y0) = (i32::from(cell.start_x) + 1, i32::from(cell.start_y) + 1);
    let x1 = i32::from(cell.end_x) - 1;
    let mut y1 = i32::from(cell.end_y) - 1;
    if y1 - y0 < 3 {
        y1 = i32::from(cell.end_y);
    }
    features.shop_rect = [x0, y0, x1, y1];
    features.shop_bounds = [0x270F, 0x270F, -0x270F, -0x270F];
    for tx in x0..x1 {
        for ty in y0..y1 {
            if let Some(tile) = floor.tile_mut(tx, ty) {
                tile.terrain |= TERRAIN_SHOP;
                tile.spawn_or_visibility &= !0x9;
            }
            let b = &mut features.shop_bounds;
            b[0] = b[0].min(tx);
            b[1] = b[1].min(ty);
            b[2] = b[2].max(tx);
            b[3] = b[3].max(ty);
        }
    }
    for tx in i32::from(cell.start_x)..i32::from(cell.end_x) {
        for ty in i32::from(cell.start_y)..i32::from(cell.end_y) {
            if let Some(tile) = floor.tile_mut(tx, ty) {
                tile.spawn_or_visibility |= 0x10;
            }
        }
    }
    // Both halves of the centre are `(a + b) / 2` as the game's signed divide
    // computes it; the coordinates are positive.
    let centre = |a: i32, b: i32| u16::try_from((a + b) / 2).unwrap_or(0);
    features.shop_centre = (centre(x0, x1), centre(y0, y1));
    true
}

/// Generation step 10 of a standard floor (`ov_0001::0214D190`,
/// `GenerateMonsterHouse` in `pmd-red`): with `chance` percent (one draw, none
/// when `chance` is 0), and only if no shop was made, `house_blocked` is false
/// (dungeon `+0x678 == 0`) and `floor_type` is 0, turns one room into a
/// monster house. The candidates are valid, hallway-linked room cells with no
/// shop, maze or link marks; a shuffled one-hot list picks one, as in step 8.
/// The cell gets `monster_house`, its tiles [`TERRAIN_MONSTER_HOUSE`], and the
/// room number goes to [`RoomFeatures::monster_house_room`]. Returns whether a
/// house was made. Written from the disassembly; see
/// `docs/re/dungeon-generation.md`.
pub fn generate_monster_house(
    floor: &mut Floor,
    grid: &mut Grid,
    chance: i32,
    floor_type: u8,
    house_blocked: bool,
    features: &mut RoomFeatures,
    rng: &mut DungeonRng,
) -> bool {
    if chance == 0 || !rng.outcome(chance) || features.shop || house_blocked || floor_type != 0 {
        return false;
    }
    let found = candidates(grid, |c| {
        !c.invalid
            && !c.extra_mark
            && c.hallway_mark
            && c.has_room
            && !c.shop
            && !c.flag_0f
            && !c.maze
            && !c.flag_09
    });
    if found.is_empty() {
        return false;
    }
    let (x, y) = found[shuffled_pick(found.len(), rng)];
    let cell = grid.cell_mut(x, y);
    features.monster_house = true;
    cell.monster_house = true;
    for tx in i32::from(cell.start_x)..i32::from(cell.end_x) {
        for ty in i32::from(cell.start_y)..i32::from(cell.end_y) {
            if let Some(tile) = floor.tile_mut(tx, ty) {
                tile.terrain |= TERRAIN_MONSTER_HOUSE;
                features.monster_house_room = tile.room;
            }
        }
    }
    true
}

/// Walk steps `(dx, dy)` by direction number 0, 2, 4, 6 (south, east, north,
/// west) in the table `ov_0001::0214FF30` reads (odd entries, the diagonals,
/// are not used).
const EXTRA_STEPS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// Room number at `(x, y)`; outside the map it reads as [`NO_ROOM`].
fn room_at(floor: &Floor, x: i32, y: i32) -> u8 {
    floor.tile(x, y).map_or(NO_ROOM, |t| t.room)
}

/// Generation step 11 of a standard floor (`ov_0001::0214FF30`,
/// `GenerateExtraHallways` in `pmd-red`): `count` attempts to dig a winding
/// hallway out of a room. An attempt picks a random cell (`DungeonRandInt` on
/// each grid axis); the cell must be valid, hold a room, have a hallway and not
/// be a maze. It draws a point in the room and a direction (south, east, north
/// or west), turned away from the grid edge, walks out of the room and across
/// any floor beyond, and starts digging at the first tile that is not floor,
/// unless that tile is the secondary terrain, the 5 x 5 block around it is not
/// all inside the map, or either side of it is floor. The hallway advances in
/// runs of `DungeonRandInt(3) + 3` tiles and turns left or right (50 %) after
/// each run. Each tile becomes floor unless that completes a 2 x 2 block of
/// floor. It stops at floor beside the path, at a wall flag, near the map edge,
/// or, on the small and medium layouts, when a turn east passes the unused part
/// of the map. Written from the disassembly; see `docs/re/dungeon-generation.md`.
pub fn generate_extra_hallways(
    floor: &mut Floor,
    grid: &Grid,
    mode: GridMode,
    count: i32,
    rng: &mut DungeonRng,
) {
    let (gx, gy) = (grid.grid_x, grid.grid_y);
    let is_floor = |floor: &Floor, x: i32, y: i32| terrain_type(floor, x, y) == TERRAIN_FLOOR;
    let step = |dir: usize, x: i32, y: i32| {
        let (dx, dy) = EXTRA_STEPS[dir / 2];
        (x + dx, y + dy)
    };
    for _ in 0..count {
        let cx = usize::try_from(rng.int(i32::try_from(gx).unwrap_or(i32::MAX))).unwrap_or(0);
        let cy = usize::try_from(rng.int(i32::try_from(gy).unwrap_or(i32::MAX))).unwrap_or(0);
        let cell = *grid.cell(cx, cy);
        if !(cell.has_room && cell.hallway_mark && !cell.invalid && !cell.maze) {
            continue;
        }
        let mut x = rng.range(i32::from(cell.start_x), i32::from(cell.end_x));
        let mut y = rng.range(i32::from(cell.start_y), i32::from(cell.end_y));
        let mut dir = usize::try_from(rng.int(4)).unwrap_or(0) * 2;
        // Turn away from a grid edge; three rounds let a turn cascade.
        for _ in 0..3 {
            if dir == 0 && cy + 1 >= gy {
                dir = 2;
            }
            if dir == 2 && cx + 1 >= gx {
                dir = 4;
            }
            if dir == 4 && cy < 1 {
                dir = 6;
            }
            if dir == 6 && cx < 1 {
                dir = 0;
            }
        }
        let start_room = room_at(floor, x, y);
        while room_at(floor, x, y) == start_room {
            (x, y) = step(dir, x, y);
        }
        while is_floor(floor, x, y) {
            (x, y) = step(dir, x, y);
        }
        if terrain_type(floor, x, y) == 2 {
            continue;
        }
        // The whole 5 x 5 block around the tile must be inside the map.
        if x < 2 || x + 2 >= WIDTH as i32 || y < 2 || y + 2 >= HEIGHT as i32 {
            continue;
        }
        let (sx, sy) = step((dir + 2) & 6, x, y);
        let (tx, ty) = step((dir + 6) & 6, x, y);
        if is_floor(floor, sx, sy) || is_floor(floor, tx, ty) {
            continue;
        }
        let mut left = rng.int(3) + 3;
        while x > 1
            && y > 1
            && x < WIDTH as i32 - 1
            && y < HEIGHT as i32 - 1
            && !is_floor(floor, x, y)
            && floor
                .tile(x, y)
                .is_some_and(|t| t.terrain & TERRAIN_IMPASSABLE_WALL == 0)
        {
            // A 2 x 2 block of floor in any of the four corners blocks the tile.
            let block = |dx: i32, dy: i32| {
                is_floor(floor, x + dx, y)
                    && is_floor(floor, x + dx, y + dy)
                    && is_floor(floor, x, y + dy)
            };
            let blocked = block(1, 1) || block(1, -1) || block(-1, 1) || block(-1, -1);
            if let Some(tile) = floor.tile_mut(x, y).filter(|_| !blocked) {
                tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
            }
            let (sx, sy) = step((dir + 2) & 6, x, y);
            let (tx, ty) = step((dir + 6) & 6, x, y);
            if is_floor(floor, sx, sy) || is_floor(floor, tx, ty) {
                break;
            }
            left -= 1;
            if left == 0 {
                left = rng.int(3) + 3;
                dir = if rng.int(100) < 50 { dir + 2 } else { dir + 6 } & 6;
                let past_edge = match mode {
                    GridMode::LeftHalf => x > 0x1F,
                    GridMode::LeftThreeQuarters => x > 0x2F,
                    GridMode::Full => false,
                };
                if past_edge && dir == 2 {
                    break;
                }
            }
            (x, y) = step(dir, x, y);
        }
    }
}

/// The 8 directions of the game's direction table, starting south and turning
/// through east, north and west (`dx`, `dy`).
const DIRECTIONS_8: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

/// For each room corner (top left, top right, bottom right, bottom left) and
/// each of the two sides to walk along from it: the step.
const CORNER_WALKS: [[(i32, i32); 2]; 4] = [
    [(0, 1), (1, 0)],
    [(-1, 0), (0, 1)],
    [(0, -1), (-1, 0)],
    [(1, 0), (0, -1)],
];

/// For each room corner: whether the neighbours to the south, east, north and
/// west are floor when the corner is a right angle that may be cut.
const CORNER_SHAPES: [[bool; 4]; 4] = [
    [true, true, false, false],
    [true, false, false, true],
    [false, false, true, true],
    [false, true, true, false],
];

/// True if a hallway tile (floor with no room) is in the 3 x 3 block around
/// `(x, y)`.
fn hallway_in_block(floor: &Floor, x: i32, y: i32) -> bool {
    (-1..=1).any(|dy| {
        (-1..=1).any(|dx| {
            terrain_type(floor, x + dx, y + dy) == TERRAIN_FLOOR
                && room_at(floor, x + dx, y + dy) == NO_ROOM
        })
    })
}

/// True if a hallway tile is next to `(x, y)` or on it (`ov_0001::0214C994`).
fn hallway_in_cross(floor: &Floor, x: i32, y: i32) -> bool {
    [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)]
        .iter()
        .any(|&(dx, dy)| {
            terrain_type(floor, x + dx, y + dy) == TERRAIN_FLOOR
                && room_at(floor, x + dx, y + dy) == NO_ROOM
        })
}

/// Generation step 12 of a standard floor (`ov_0001::0214E3DC`,
/// `GenerateRoomImperfections` in `pmd-red`): cuts corners off rooms. Cells
/// are visited column by column. A cell qualifies when it is valid, holds a
/// room that a hallway reaches, and has `imperfect` set but none of `+0x09`,
/// `+0x11`, `+0x12` or the maze mark; it is then used with 40 % chance (one
/// `DungeonRandInt(100)`, 60 or more). The room gets `(width + height) / 4`
/// rounds (at least 1). A round takes two corners: each draws a corner
/// (`DungeonRandInt(4)`) and walks from it along a side of the room, up to 10
/// tiles and while inside the room, to the first floor tile. That tile
/// becomes wall if no hallway tile is within one tile of any of its 8
/// neighbours and its neighbours to the south, east, north and west are floor
/// exactly as the corner's shape says (`CORNER_SHAPES`). Written from the
/// disassembly; see `docs/re/dungeon-generation.md`.
pub fn generate_room_imperfections(floor: &mut Floor, grid: &Grid, rng: &mut DungeonRng) {
    let is_floor = |floor: &Floor, x: i32, y: i32| terrain_type(floor, x, y) == TERRAIN_FLOOR;
    for gx in 0..grid.grid_x {
        for gy in 0..grid.grid_y {
            let cell = grid.cell(gx, gy);
            if cell.invalid
                || cell.extra_mark
                || cell.extra_linked
                || !cell.hallway_mark
                || !cell.has_room
                || cell.flag_09
                || cell.maze
                || !cell.imperfect
                || rng.int(100) < 60
            {
                continue;
            }
            let (width, height) = span(cell);
            let [x0, y0, x1, y1] =
                [cell.start_x, cell.start_y, cell.end_x, cell.end_y].map(i32::from);
            for _ in 0..((width + height) / 4).max(1) {
                for walk in [0, 1] {
                    let corner = usize::try_from(rng.int(4)).unwrap_or(0);
                    let (mut x, mut y) = match corner {
                        1 => (x1 - 1, y0),
                        2 => (x1 - 1, y1 - 1),
                        3 => (x0, y1 - 1),
                        _ => (x0, y0),
                    };
                    let (dx, dy) = CORNER_WALKS[corner][walk];
                    for _ in 0..10 {
                        if x < x0 || x >= x1 || y < y0 || y >= y1 {
                            break;
                        }
                        if !is_floor(floor, x, y) {
                            (x, y) = (x + dx, y + dy);
                            continue;
                        }
                        let near_hallway = DIRECTIONS_8
                            .iter()
                            .any(|&(ox, oy)| hallway_in_block(floor, x + ox, y + oy));
                        let shape = (0..4).all(|k| {
                            let (ox, oy) = DIRECTIONS_8[k * 2];
                            is_floor(floor, x + ox, y + oy) == CORNER_SHAPES[corner][k]
                        });
                        if !near_hallway && shape {
                            make_wall(floor, x, y);
                        }
                        break;
                    }
                }
            }
        }
    }
}

/// Sets `flags` in the second flag word of the tile at `(x, y)`.
fn mark_tile(floor: &mut Floor, x: i32, y: i32, flags: u16) {
    if let Some(tile) = floor.tile_mut(x, y) {
        tile.spawn_or_visibility |= flags;
    }
}

/// Bit of the second tile flag word set on every tile of a room that got a
/// secondary structure (`ov_0001::0214CA3C`). Its role is not known.
const TILE_STRUCTURE_ROOM: u16 = 0x20;
/// Terrain flag on one half of a room that a line of secondary terrain splits.
/// Its role is not known.
const TERRAIN_STRUCTURE_SIDE: u16 = 0x80;

fn mark_side(floor: &mut Floor, x: i32, y: i32) {
    if let Some(tile) = floor.tile_mut(x, y) {
        tile.terrain |= TERRAIN_STRUCTURE_SIDE;
    }
}

/// Marks every tile of the room of `cell` (`ov_0001::0214CA3C`).
fn mark_room(floor: &mut Floor, cell: &GridCell) {
    for x in i32::from(cell.start_x)..i32::from(cell.end_x) {
        for y in i32::from(cell.start_y)..i32::from(cell.end_y) {
            mark_tile(floor, x, y, TILE_STRUCTURE_ROOM);
        }
    }
}

/// Generation step 13 of a standard floor (`ov_0001::0214F6F0`,
/// `GenerateSecondaryStructures` in `pmd-red`). Cells are visited row by row.
/// A valid room cell with `structure` set and none of `imperfect`, the monster
/// house mark and `+0x12` draws `DungeonRandInt(6)`: 0 does nothing, 1 to 5
/// build a structure of the secondary terrain (see [`build_structure`]). Each
/// structure costs one unit of `budget` (floor-properties byte `+0x0C` in
/// `gFloorPropsByteC`, `021B45C0`) and is skipped when none is left. Written
/// from the disassembly; see `docs/re/dungeon-generation.md`.
pub fn generate_secondary_structures(
    floor: &mut Floor,
    grid: &mut Grid,
    budget: &mut u8,
    features: &mut RoomFeatures,
    rng: &mut DungeonRng,
) {
    for gy in 0..grid.grid_y {
        for gx in 0..grid.grid_x {
            let cell = grid.cell_mut(gx, gy);
            if cell.invalid
                || cell.monster_house
                || cell.extra_linked
                || !cell.has_room
                || cell.imperfect
                || !cell.structure
            {
                continue;
            }
            build_structure(floor, cell, budget, features, rng);
        }
    }
}

/// `ov_0001::0214C244`: one draw of `DungeonRandInt(6)` picks what to build in
/// the room of `cell`; a structure sets `+0x09` on the cell.
///
/// 1. A maze of secondary terrain if both sides are odd; otherwise a plus of
///    five tiles in the middle (3 tiles wide) for a room at least 5 x 5, or a
///    single tile.
/// 2. Random single tiles: 64 draws of a position, kept when the sum of the
///    two offsets is odd. Needs both sides odd.
/// 3. A rectangle between two random points, for rooms at least 5 x 5. The
///    four draws come before the budget check.
/// 4. A ring 4 x 4 around the centre, for rooms at least 6 x 6.
/// 5. A line of secondary terrain across the middle, vertical or horizontal
///    (one draw), unless a hallway tile touches it.
fn build_structure(
    floor: &mut Floor,
    cell: &mut GridCell,
    budget: &mut u8,
    features: &mut RoomFeatures,
    rng: &mut DungeonRng,
) {
    let [x0, y0, x1, y1] = [cell.start_x, cell.start_y, cell.end_x, cell.end_y].map(i32::from);
    let (width, height) = (x1 - x0, y1 - y0);
    let (cx, cy) = ((x0 + x1) / 2, (y0 + y1) / 2);
    let kind = rng.int(6);
    let available = *budget != 0;
    match kind {
        1 if available => {
            *budget -= 1;
            if width & 1 != 0 && height & 1 != 0 {
                let room = room_at(floor, x0, y0);
                mark_room(floor, cell);
                make_maze(floor, cell, features, MazeWall::SecondaryIn(room), rng);
            } else if width >= 5 && height >= 5 {
                for (x, y) in [
                    (cx + 1, cy),
                    (cx, cy + 1),
                    (cx - 1, cy),
                    (cx, cy - 1),
                    (cx, cy),
                ] {
                    make_secondary(floor, x, y, 0);
                }
            } else {
                make_secondary(floor, cx, cy, 0);
            }
        }
        2 if available && width & 1 != 0 && height & 1 != 0 => {
            *budget -= 1;
            mark_room(floor, cell);
            for _ in 0..64 {
                let a = rng.int(width);
                let b = rng.int(height);
                if (a + b) & 1 != 0 {
                    make_secondary(floor, x0 + a, y0 + b, 0);
                }
            }
        }
        3 if width >= 5 && height >= 5 => {
            let ax = rng.range(x0 + 2, x1 - 3);
            let ay = rng.range(y0 + 2, y1 - 3);
            let bx = rng.range(x0 + 2, x1 - 3);
            let by = rng.range(y0 + 2, y1 - 3);
            if !available {
                return;
            }
            *budget -= 1;
            mark_room(floor, cell);
            for x in ax.min(bx)..=ax.max(bx) {
                for y in ay.min(by)..=ay.max(by) {
                    make_secondary(floor, x, y, 0);
                }
            }
        }
        4 if available && width >= 6 && height >= 6 => {
            *budget -= 1;
            mark_room(floor, cell);
            for x in cx - 2..=cx + 1 {
                for y in cy - 2..=cy + 1 {
                    if x == cx - 2 || x == cx + 1 || y == cy - 2 || y == cy + 1 {
                        make_secondary(floor, x, y, 0x4);
                    }
                }
            }
            for (x, y, flags) in [
                (cx - 1, cy - 1, 0x54),
                (cx, cy - 1, 0x12),
                (cx - 1, cy, 0x12),
                (cx, cy, 0x12),
            ] {
                mark_tile(floor, x, y, flags);
            }
        }
        5 if available => {
            *budget -= 1;
            mark_room(floor, cell);
            if rng.int(2) != 0 {
                if (y0..y1).any(|y| hallway_in_cross(floor, cx, y)) {
                    return;
                }
                for y in y0..y1 {
                    make_secondary(floor, cx, y, 0);
                }
                for (x, y) in (x0..cx).flat_map(|x| (y0..y1).map(move |y| (x, y))) {
                    mark_side(floor, x, y);
                }
            } else {
                if (x0..x1).any(|x| hallway_in_cross(floor, x, cy)) {
                    return;
                }
                for x in x0..x1 {
                    make_secondary(floor, x, cy, 0);
                }
                for (x, y) in (y0..cy).flat_map(|y| (x0..x1).map(move |x| (x, y))) {
                    mark_side(floor, x, y);
                }
            }
        }
        _ => return,
    }
    cell.flag_09 = true;
}

/// `ov_0001::02149A70` (`ResetInnerBoundaryTileRows`): clears the tiles of the
/// second and the second-to-last row, and makes the ones at the two ends of
/// those rows impassable walls.
pub fn reset_inner_boundary_tile_rows(floor: &mut Floor) {
    let last_x = WIDTH as i32 - 1;
    for x in 0..=last_x {
        for y in [1, HEIGHT as i32 - 2] {
            if let Some(tile) = floor.tile_mut(x, y) {
                *tile = Tile::EMPTY;
                if x == 0 || x == last_x {
                    tile.terrain |= TERRAIN_IMPASSABLE_WALL;
                }
            }
        }
    }
}

/// `ov_0001::0214C0E0` (`EnsureImpassableTilesAreWalls`): every tile with the
/// impassable wall flag becomes wall.
pub fn ensure_impassable_tiles_are_walls(floor: &mut Floor) {
    for x in 0..WIDTH as i32 {
        for y in 0..HEIGHT as i32 {
            if let Some(tile) = floor.tile_mut(x, y)
                && tile.terrain & TERRAIN_IMPASSABLE_WALL != 0
            {
                tile.terrain &= !TERRAIN_TYPE_MASK;
            }
        }
    }
}

/// Terrain flag set on a room tile that touches a hallway.
const TERRAIN_JUNCTION: u16 = 0x8;
/// `ov_0001::0214D900` (`FinalizeJunctions`), without its last call
/// (`0214D82C`, which lists the junction tiles of each room in the dungeon
/// struct, outside the tile grid). Walking the map column by column: a
/// hallway tile (floor with no room) sets the junction flag on each of its
/// four neighbours that is in a room and turns it from the secondary terrain
/// into floor; a floor tile numbered `0xFE` (an anchor) loses its number.
pub fn finalize_junctions(floor: &mut Floor) {
    for x in 0..WIDTH as i32 {
        for y in 0..HEIGHT as i32 {
            if terrain_type(floor, x, y) != TERRAIN_FLOOR {
                continue;
            }
            match room_at(floor, x, y) {
                NO_ROOM => {
                    for (nx, ny) in [(x - 1, y), (x, y - 1), (x, y + 1), (x + 1, y)] {
                        if let Some(tile) = floor.tile_mut(nx, ny).filter(|t| t.room != NO_ROOM) {
                            tile.terrain |= TERRAIN_JUNCTION;
                            if tile.terrain & TERRAIN_TYPE_MASK == TERRAIN_SECONDARY {
                                tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
                            }
                        }
                    }
                }
                ANCHOR_ROOM => {
                    if let Some(tile) = floor.tile_mut(x, y) {
                        tile.room = NO_ROOM;
                    }
                }
                _ => {}
            }
        }
    }
}

/// `ov_0001::0214B2F8`: a wall tile that is not an impassable wall becomes the
/// secondary terrain. Outside the map nothing happens.
fn wall_to_secondary(floor: &mut Floor, x: i32, y: i32) {
    if let Some(tile) = floor.tile_mut(x, y)
        && tile.terrain & (TERRAIN_TYPE_MASK | TERRAIN_IMPASSABLE_WALL) == 0
    {
        tile.terrain |= TERRAIN_SECONDARY;
    }
}

/// How many of the eight tiles around `(x, y)` are the secondary terrain.
fn secondary_neighbours(floor: &Floor, x: i32, y: i32) -> usize {
    (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| (dx, dy) != (0, 0))
        .filter(|&(dx, dy)| terrain_type(floor, x + dx, y + dy) == TERRAIN_SECONDARY)
        .count()
}

/// One winding stripe of the secondary terrain: it starts at a random column on
/// the top or bottom row and runs vertically for 2 to 7 tiles, then sideways
/// for 2 to 7 tiles, and so on, until it leaves the map at the top or bottom.
/// Stepping onto the secondary terrain (its own earlier path, or a stripe from
/// before) ends it. Once it has covered a random 10 to 59 tiles it grows a
/// lumpy edge: 64 random points within 3 tiles of the head that touch the
/// terrain are filled, then every tile within 3 tiles with 4 or more neighbours
/// of it is.
fn secondary_stripe(floor: &mut Floor, rng: &mut DungeonRng) {
    let from_bottom = rng.int(100) < 50;
    let mut y = if from_bottom { HEIGHT as i32 - 1 } else { 0 };
    let mut dy = if from_bottom { -1 } else { 1 };
    let mut dx = 0;
    let mut until_lumps = rng.int(50) + 10;
    let mut x = rng.range(2, 54);
    let mut steps = rng.int(6) + 2;
    loop {
        while steps != 0 {
            if (0..WIDTH as i32).contains(&x) {
                if terrain_type(floor, x, y) == TERRAIN_SECONDARY {
                    return;
                }
                wall_to_secondary(floor, x, y);
            }
            steps -= 1;
            x += dx;
            y += dy;
            if !(0..HEIGHT as i32).contains(&y) {
                break;
            }
            until_lumps -= 1;
            if until_lumps == 0 {
                for _ in 0..64 {
                    let px = x + rng.int(7) - 3;
                    let py = y + rng.int(7) - 3;
                    if (2..WIDTH as i32 - 2).contains(&px)
                        && (2..HEIGHT as i32 - 2).contains(&py)
                        && secondary_neighbours(floor, px, py) > 0
                    {
                        wall_to_secondary(floor, px, py);
                    }
                }
                for px in x - 3..=x + 3 {
                    for py in y - 3..=y + 3 {
                        if (2..WIDTH as i32 - 2).contains(&px)
                            && (2..HEIGHT as i32 - 2).contains(&py)
                            && secondary_neighbours(floor, px, py) >= 4
                        {
                            wall_to_secondary(floor, px, py);
                        }
                    }
                }
            }
        }
        // Turn: a vertical run is followed by a sideways one (left or right
        // by a draw), a sideways run by the original vertical direction.
        if dx == 0 {
            dy = 0;
            dx = if rng.int(100) < 50 { -1 } else { 1 };
        } else {
            dx = 0;
            dy = if from_bottom { -1 } else { 1 };
        }
        if !(0..HEIGHT as i32).contains(&y) {
            return;
        }
        steps = rng.int(6) + 2;
    }
}

/// One pond of the secondary terrain: a random point at least 1 tile from the
/// map edge (up to 200 tries, then none), and a 10 x 10 block around it whose
/// border is marked and whose inside is filled by 80 random marks that touch a
/// marked tile (4 neighbours). Every tile of the block left unmarked becomes
/// the secondary terrain, so the pond is the hole the marks leave.
fn secondary_pond(floor: &mut Floor, rng: &mut DungeonRng) {
    let Some((px, py)) = (0..200)
        .map(|_| (rng.range(0, WIDTH as i32), rng.range(0, HEIGHT as i32)))
        .find(|&(x, y)| (1..WIDTH as i32 - 1).contains(&x) && (1..HEIGHT as i32 - 1).contains(&y))
    else {
        return;
    };
    let mut marked = [[false; 10]; 10];
    for (a, row) in marked.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            *cell = a == 0 || a == 9 || b == 0 || b == 9;
        }
    }
    for _ in 0..80 {
        let a = usize::try_from(rng.int(8) + 1).unwrap_or(1);
        let b = usize::try_from(rng.int(8) + 1).unwrap_or(1);
        if marked[a - 1][b] || marked[a + 1][b] || marked[a][b - 1] || marked[a][b + 1] {
            marked[a][b] = true;
        }
    }
    for (a, row) in marked.iter().enumerate() {
        for (b, &cell) in row.iter().enumerate() {
            if !cell {
                wall_to_secondary(floor, px + a as i32 - 5, py + b as i32 - 5);
            }
        }
    }
}

/// `ov_0001::0214AAF8` (`GenerateSecondaryTerrainFormations(1, props)` in
/// `pmd-red`; a `[red-hypothesis]` name): water, lava or a chasm, by tileset, as
/// the terrain type 2. Runs only when bit 0 of floor-properties byte `+0x0D` is
/// set (not the case on the first floor, so it draws nothing there). It draws
/// the number of stripes (1, 1, 1, 2, 2, 2, 3 or 3, by `DungeonRandInt(8)`),
/// builds each with [`secondary_stripe`], then `ponds` ponds (floor-properties
/// byte `+0x15`) with [`secondary_pond`]. A last pass over the map keeps the
/// terrain only where it is at least 2 tiles from the edge and not a shop,
/// monster house or flagged tile (`0x160` in the terrain word, or bit 0 of the
/// second word); the flagged tiles become floor, the others wall.
pub fn generate_secondary_terrain(floor: &mut Floor, byte_0d: u8, ponds: u8, rng: &mut DungeonRng) {
    if byte_0d & 1 == 0 {
        return;
    }
    let stripes = [1, 1, 1, 2, 2, 2, 3, 3][usize::try_from(rng.int(8)).unwrap_or(0)];
    for _ in 0..stripes {
        secondary_stripe(floor, rng);
    }
    for _ in 0..ponds {
        secondary_pond(floor, rng);
    }
    for x in 0..WIDTH as i32 {
        for y in 0..HEIGHT as i32 {
            let Some(tile) = floor.tile_mut(x, y) else {
                continue;
            };
            if tile.terrain & TERRAIN_TYPE_MASK != TERRAIN_SECONDARY {
                continue;
            }
            if tile.terrain & 0x160 != 0 || tile.spawn_or_visibility & 1 != 0 {
                tile.terrain = (tile.terrain & !TERRAIN_TYPE_MASK) | TERRAIN_FLOOR;
            } else if !((2..WIDTH as i32 - 1).contains(&x) && (2..HEIGHT as i32 - 1).contains(&y)) {
                tile.terrain &= !TERRAIN_TYPE_MASK;
            }
        }
    }
}

/// What `SpawnNonEnemies` reads from the floor properties.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpawnCounts {
    /// `+0x0F`: average number of items in the rooms; 0 places none.
    pub items: u8,
    /// `+0x10`: the most traps; the count is drawn between half of it and all of it.
    pub traps: u8,
    /// `+0x14`: average number of items buried in the walls; 0 places none.
    pub buried: u8,
}

/// The values `SpawnNonEnemies` reads and writes besides the tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnState {
    /// Pair A (dungeon `+0xC0B8`, `+0xC0BA`): where the leader starts; set by the step when unset.
    pub leader: (i16, i16),
    /// Pair B (dungeon `+0xC0BC`, `+0xC0BE`): the stairs; set by the step when unset.
    pub stairs: (i16, i16),
    /// Byte `021B4598`: room number of the stairs.
    pub stairs_room: u8,
    /// Dungeon `+0x34AC`: room that floor type 2 turns into a monster house.
    pub house_room: u8,
}

/// Spawn markers in the second tile word, as `SpawnNonEnemies` sets and reads them.
const SPAWN_STAIRS: u16 = 0x1;
const SPAWN_ITEM: u16 = 0x2;
const SPAWN_TRAP: u16 = 0x4;
const SPAWN_BLOCKED: u16 = 0x8;
const SPAWN_NO_STAIRS: u16 = 0x10;
const TERRAIN_NO_SPAWN: u16 = 0x100;

/// The most items a monster house gets (a constant in overlay 1's data).
const HOUSE_ITEM_LIMIT: i32 = 7;

/// Swaps two random entries, `2 * len` times (`ov_0001::0214BD40`).
fn shuffle_positions(list: &mut [(u8, u8)], rng: &mut DungeonRng) {
    let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
    for _ in 0..2 * list.len() {
        let a = usize::try_from(rng.int(len)).unwrap_or(0);
        let b = usize::try_from(rng.int(len)).unwrap_or(0);
        list.swap(a, b);
    }
}

/// Every tile (x outer, y inner) for which `keep(terrain, spawn, room)` holds.
fn tile_positions(floor: &Floor, keep: impl Fn(u16, u16, u8) -> bool) -> Vec<(u8, u8)> {
    let mut found = Vec::new();
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            let tile = floor.tile(x as i32, y as i32).expect("in-map tile");
            if keep(tile.terrain, tile.spawn_or_visibility, tile.room) {
                found.push((x as u8, y as u8));
            }
        }
    }
    found
}

/// A random entry of `list`, which must not be empty.
fn pick_position(list: &[(u8, u8)], rng: &mut DungeonRng) -> (u8, u8) {
    let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
    list[usize::try_from(rng.int(len)).unwrap_or(0)]
}

/// Sets `marker` in the spawn word of `count` tiles of `list`, from a random
/// start and wrapping at the end.
fn mark_from_random(
    floor: &mut Floor,
    list: &[(u8, u8)],
    count: i32,
    marker: u16,
    rng: &mut DungeonRng,
) {
    let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
    let mut at = usize::try_from(rng.int(len)).unwrap_or(0);
    for _ in 0..count {
        let (x, y) = list[at];
        if let Some(tile) = floor.tile_mut(i32::from(x), i32::from(y)) {
            tile.spawn_or_visibility |= marker;
        }
        at = (at + 1) % list.len();
    }
}

/// `ov_0001::0214B650` (`SpawnNonEnemies(props, isEmptyMonsterHouse)` in
/// `pmd-red`; a `[red-hypothesis]` name). It marks tiles; the entities come
/// from later steps. In order:
///
/// 1. **Stairs**, when pair B is unset: one room floor tile that is not a
///    shop, junction or no-spawn tile and has neither spawn bit `0x8` nor
///    [`SPAWN_NO_STAIRS`], by `DungeonRandInt`. It gets the stairs marker and
///    loses the item marker. On floor type 2 every floor tile of the stairs'
///    room also gets the monster-house terrain bit.
/// 2. **Items**: `items` (floor-properties `+0x0F`) turns into
///    `DungeonRandRange(items - 2, items + 2)`, at least 1 (nothing when it
///    is 0), over a shuffled list of room tiles that are not a shop, junction,
///    no-spawn tile or monster house.
/// 3. **Buried items**: the same over wall tiles (terrain type 0), with
///    `buried` (`+0x14`) and no minimum.
/// 4. **Monster-house items**, unless `empty_house`: over the monster-house
///    tiles, `DungeonRandRange(5 n / 10, 8 n / 10)` of them, clamped to 6
///    and [`HOUSE_ITEM_LIMIT`]. Each draws `DungeonRandInt(2)`: an item, or a
///    trap when `trap_flag` (dungeon `+0x64D`) is set.
/// 5. **Traps**: `DungeonRandRange(traps / 2, traps)` (`+0x10`), at most 56,
///    over room tiles with no item.
/// 6. **Leader**, when pair A is unset: one room tile with no item, trap or
///    `0x8` marker.
///
/// Lists are scanned x outer, y inner, shuffled with `2 n` swaps of two
/// `DungeonRandInt(n)` draws, and read from a random start. Written from the
/// disassembly; see `docs/re/dungeon-generation.md`.
pub fn spawn_non_enemies(
    floor: &mut Floor,
    counts: SpawnCounts,
    empty_house: bool,
    floor_type: u8,
    trap_flag: bool,
    state: &mut SpawnState,
    rng: &mut DungeonRng,
) {
    let in_room =
        |terrain: u16, room: u8| terrain & TERRAIN_TYPE_MASK == TERRAIN_FLOOR && room != NO_ROOM;
    let plain = |terrain: u16| terrain & (TERRAIN_SHOP | TERRAIN_JUNCTION | TERRAIN_NO_SPAWN) == 0;

    if state.stairs.0 == -1 || state.stairs.1 == -1 {
        let found = tile_positions(floor, |t, s, room| {
            in_room(t, room) && plain(t) && s & (SPAWN_BLOCKED | SPAWN_NO_STAIRS) == 0
        });
        if !found.is_empty() {
            let (x, y) = pick_position(&found, rng);
            let tile = floor
                .tile_mut(i32::from(x), i32::from(y))
                .expect("in-map tile");
            tile.spawn_or_visibility = (tile.spawn_or_visibility | SPAWN_STAIRS) & !SPAWN_ITEM;
            let room = tile.room;
            state.stairs_room = room;
            state.stairs = (i16::from(x), i16::from(y));
            if floor_type == 2 {
                for tx in 0..WIDTH as i32 {
                    for ty in 0..HEIGHT as i32 {
                        let t = floor.tile_mut(tx, ty).expect("in-map tile");
                        if t.terrain & TERRAIN_TYPE_MASK == TERRAIN_FLOOR && t.room == room {
                            t.terrain |= TERRAIN_MONSTER_HOUSE;
                        }
                    }
                }
                state.house_room = room;
            }
        }
    }

    let mut found = tile_positions(floor, |t, _, room| {
        in_room(t, room) && plain(t) && t & TERRAIN_MONSTER_HOUSE == 0
    });
    let mut count = i32::from(counts.items);
    if !found.is_empty() && count != 0 {
        count = rng.range(count - 2, count + 2).max(1);
    }
    if !found.is_empty() && count > 0 {
        shuffle_positions(&mut found, rng);
        mark_from_random(floor, &found, count, SPAWN_ITEM, rng);
    }

    let mut found = tile_positions(floor, |t, _, _| t & TERRAIN_TYPE_MASK == 0);
    let mut count = i32::from(counts.buried);
    if !found.is_empty() && count != 0 {
        count = rng.range(count - 2, count + 2);
    }
    if !found.is_empty() && count > 0 {
        shuffle_positions(&mut found, rng);
        mark_from_random(floor, &found, count, SPAWN_ITEM, rng);
    }

    let mut found = if empty_house {
        Vec::new()
    } else {
        tile_positions(floor, |t, _, _| {
            t & (TERRAIN_SHOP | TERRAIN_JUNCTION) == 0 && t & TERRAIN_MONSTER_HOUSE != 0
        })
    };
    if !found.is_empty() {
        let len = i32::try_from(found.len()).unwrap_or(i32::MAX);
        let count = rng
            .range(len * 5 / 10, len * 8 / 10)
            .clamp(6, HOUSE_ITEM_LIMIT);
        shuffle_positions(&mut found, rng);
        let mut at = usize::try_from(rng.int(len)).unwrap_or(0);
        for _ in 0..count {
            let (x, y) = found[at];
            let item = rng.int(2) != 0;
            if let Some(tile) = floor.tile_mut(i32::from(x), i32::from(y)) {
                if item {
                    tile.spawn_or_visibility |= SPAWN_ITEM;
                } else if trap_flag {
                    tile.spawn_or_visibility |= SPAWN_TRAP;
                }
            }
            at = (at + 1) % found.len();
        }
    }

    let mut found = tile_positions(floor, |t, s, room| {
        in_room(t, room) && plain(t) && s & SPAWN_ITEM == 0
    });
    if !found.is_empty() {
        let most = i32::from(counts.traps);
        let count = rng.range(most >> 1, most);
        if count > 0 {
            shuffle_positions(&mut found, rng);
            mark_from_random(floor, &found, count.min(WIDTH as i32), SPAWN_TRAP, rng);
        }
    }

    if state.leader.0 == -1 || state.leader.1 == -1 {
        let found = tile_positions(floor, |t, s, room| {
            in_room(t, room) && plain(t) && s & (SPAWN_ITEM | SPAWN_BLOCKED | SPAWN_TRAP) == 0
        });
        if !found.is_empty() {
            let (x, y) = pick_position(&found, rng);
            state.leader = (i16::from(x), i16::from(y));
        }
    }
}

/// FNV-1a 64 over the spawn word (`u16`, little endian) of every tile, row by
/// row; `spawnFnv1a` of `floor_stages.py`.
#[must_use]
pub fn spawn_fnv1a(floor: &Floor) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let tile = floor.tile(x as i32, y as i32).expect("in-map tile");
            for byte in tile.spawn_or_visibility.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            }
        }
    }
    hash
}

/// FNV-1a 64 over the terrain flags (`u16`, little endian) and the room of
/// every tile, row by row. `verification/v7-trace/floor_stages.py` computes the
/// same value from the retail ROM (`tilesFnv1a` at the entry of step 7).
#[must_use]
pub fn tiles_fnv1a(floor: &Floor) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    let mut feed = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    };
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let tile = floor.tile(x as i32, y as i32).expect("in-map tile");
            for byte in tile.terrain.to_le_bytes() {
                feed(byte);
            }
            feed(tile.room);
        }
    }
    hash
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

    /// The dungeon RNG state at the entry of step 6 (the end of step 5), in the
    /// order of [`NATURAL_SEED`] and [`FORCED_SEEDS`], from the retail ROM.
    /// Step 5 takes 33 RNG steps for the natural, `0x12345678` and `0x0BADF00D`
    /// seeds and 34 for `0xCAFEBABF`.
    const STEP6_ENTRY: [u32; 4] = [3_888_478_419, 958_992_781, 96_248_885, 756_241_683];

    #[test]
    fn connections_match_the_retail_rng_budget() {
        for (i, (stages, std6)) in std::iter::once(NATURAL_SEED)
            .chain(FORCED_SEEDS)
            .zip(STEP6_ENTRY)
            .enumerate()
        {
            let (_, _, _, mut grid) = first_floor_steps(stages.grid_y, stages.std3);
            let mut rng = DungeonRng::from_state(stages.std5);
            assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
            assert_eq!(rng.state(), std6, "case {i}, step 6 entry");
        }
    }

    /// Step 6 checkpoints from the retail ROM, in the order of
    /// [`NATURAL_SEED`] and [`FORCED_SEEDS`]: the dungeon RNG state at the
    /// entry of step 7 and `tilesFnv1a` (the tile hash of
    /// `floor_stages.py`) at the same point.
    const HALLWAY_RESULTS: [(u32, u64); 4] = [
        (717_111_574, 0x08aa_8f91_334e_3d50),
        (3_678_770_459, 0x6e15_e485_50cd_b376),
        (2_147_750_935, 0xf251_6ef3_4075_0e51),
        (4_249_534_461, 0x3299_603e_f26a_4dfd),
    ];

    #[test]
    fn hallways_match_the_retail_rng_budget_and_tiles() {
        let cases = std::iter::once(NATURAL_SEED).chain(FORCED_SEEDS);
        for (i, (stages, (std7, tiles))) in cases.zip(HALLWAY_RESULTS).enumerate() {
            let (_, _, mut floor, mut grid) = first_floor_steps(stages.grid_y, stages.std3);
            let mut rng = DungeonRng::from_state(stages.std5);
            assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
            let (xs, ys) = grid_bounds(4, stages.grid_y);
            create_hallways(&mut floor, &mut grid, &xs, &ys, false, &mut rng);
            assert_eq!(rng.state(), std7, "case {i}, step 7");
            assert_eq!(tiles_fnv1a(&floor), tiles, "case {i}, tiles");
        }
    }

    /// Step 7 checkpoints from the retail ROM, in the order of
    /// [`NATURAL_SEED`] and [`FORCED_SEEDS`]: the dungeon RNG state and
    /// `tilesFnv1a` at the entry of step 8. Only the `0x12345678` floor has an
    /// unlinked cell, so only it draws (4 steps) and digs.
    const ENSURE_RESULTS: [(u32, u64); 4] = [
        (717_111_574, 0x08aa_8f91_334e_3d50),
        (4_001_168_455, 0x6185_0823_cb3c_1bb3),
        (2_147_750_935, 0xf251_6ef3_4075_0e51),
        (4_249_534_461, 0x3299_603e_f26a_4dfd),
    ];

    #[test]
    fn ensure_connected_grid_matches_the_retail_rng_budget_and_tiles() {
        let cases = std::iter::once(NATURAL_SEED).chain(FORCED_SEEDS);
        for (i, (stages, (std8, tiles))) in cases.zip(ENSURE_RESULTS).enumerate() {
            let (_, _, mut floor, mut grid) = first_floor_steps(stages.grid_y, stages.std3);
            let mut rng = DungeonRng::from_state(stages.std5);
            assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
            let (xs, ys) = grid_bounds(4, stages.grid_y);
            create_hallways(&mut floor, &mut grid, &xs, &ys, false, &mut rng);
            ensure_connected_grid(&mut floor, &mut grid, &xs, &ys, &mut rng);
            assert_eq!(rng.state(), std8, "case {i}, step 8");
            assert_eq!(tiles_fnv1a(&floor), tiles, "case {i}, tiles");
        }
    }

    /// More forced seeds where step 7 changes the floor, from the retail ROM:
    /// the grid height, the RNG state at the entry of steps 3, 5 and 8, and
    /// `tilesFnv1a` at the entry of step 8. They change the RNG by 0 to 10 draws
    /// and dig, wall off anchors and erase rooms (23 of 36 seeds tried).
    const ENSURE_SEEDS: &[(usize, u32, u32, u32, u64)] = &[
        // 0x08D12DFD
        (
            2,
            1_976_947_032,
            284_701_749,
            504_553_352,
            0x7aa6_256a_a9d6_cf3e,
        ),
        // 0x11A25BFA
        (
            3,
            2_937_234_321,
            1_860_216_811,
            1_083_456_640,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0x28B7BC6F
        (
            3,
            3_280_667_150,
            394_156_293,
            3_075_025_184,
            0xed95_bbd0_b9c1_d18b,
        ),
        // 0x2E2AC0EA
        (
            2,
            278_952_215,
            898_684_437,
            818_453_974,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0x36FBEEE7
        (
            3,
            3_033_275_370,
            3_731_767_195,
            785_028_812,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0x3C6EF362
        (
            3,
            2_733_681_987,
            2_735_416_536,
            2_388_364_781,
            0x97a2_8fad_e62b_87c6,
        ),
        // 0x3FCD1CE4
        (
            3,
            2_617_097_491,
            3_091_852_748,
            2_402_063_198,
            0xfce3_338f_d2b5_541d,
        ),
        // 0x4540215F
        (
            3,
            3_289_384_834,
            3_735_770_515,
            4_142_196_411,
            0xacc0_685b_f00f_e8c4,
        ),
        // 0x538453D7
        (
            2,
            475_179_094,
            2_983_244_959,
            3_689_695_638,
            0x5c97_6138_91eb_e191,
        ),
        // 0x5C5581D4
        (
            2,
            1_815_861_487,
            3_747_341_869,
            2_242_055_630,
            0xfccd_8013_5d37_f380,
        ),
        // 0x6A99B44C
        (
            3,
            3_385_425_883,
            2_404_653_519,
            2_775_422_417,
            0xf102_551e_60fb_401c,
        ),
        // 0x78DDE6C4
        (
            3,
            3_641_535_347,
            189_740_765,
            4_203_639_610,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0x8A8042BE
        (
            3,
            2_809_179_589,
            602_133_439,
            3_326_839_150,
            0xe373_ce19_eb33_11c3,
        ),
        // 0x8FF34739
        (
            3,
            2_304_828_844,
            527_474_027,
            1_615_930_158,
            0x4245_b5f2_2b37_19d8,
        ),
        // 0x98C47536
        (
            3,
            3_902_873_959,
            1_344_564_337,
            3_808_655_134,
            0x1c24_261f_1645_50ea,
        ),
        // 0xA195A333
        (
            3,
            4_281_705_724,
            2_454_085_806,
            244_803_382,
            0x0472_d239_7daa_bc69,
        ),
        // 0xA708A7AE
        (
            3,
            3_853_845_675,
            2_451_130_894,
            804_935_811,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0xB54CDA26
        (
            2,
            1_249_533_463,
            2_240_918_293,
            774_831_830,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0xBE1E0823
        (
            3,
            2_238_119_330,
            2_333_177_289,
            1_679_710_362,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0xCC623A9B
        (
            3,
            3_417_439_566,
            1_590_288_527,
            4_060_182_839,
            0xa92d_97a5_acc8_b7e2,
        ),
        // 0xEC48C90D
        (
            3,
            2_841_193_272,
            3_064_137_930,
            1_634_106_075,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0xF1BBCD88
        (
            3,
            3_513_480_615,
            3_226_624_689,
            2_074_527_326,
            0xbc4c_fafe_a003_4b05,
        ),
        // 0xFA8CFB85
        (
            3,
            4_002_331_848,
            490_432_218,
            3_283_895_198,
            0xee83_2232_f3d3_0df3,
        ),
    ];

    #[test]
    fn ensure_connected_grid_matches_more_retail_floors() {
        for (i, &(grid_y, std3, std5, std8, tiles)) in ENSURE_SEEDS.iter().enumerate() {
            let (_, _, mut floor, mut grid) = first_floor_steps(grid_y, std3);
            let mut rng = DungeonRng::from_state(std5);
            assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
            let (xs, ys) = grid_bounds(4, grid_y);
            create_hallways(&mut floor, &mut grid, &xs, &ys, false, &mut rng);
            ensure_connected_grid(&mut floor, &mut grid, &xs, &ys, &mut rng);
            assert_eq!(rng.state(), std8, "seed row {i}, step 8");
            assert_eq!(tiles_fnv1a(&floor), tiles, "seed row {i}, tiles");
        }
    }

    /// One run of steps 8 to 10 on the retail ROM, from
    /// `floor_stages.py --force-chance ... [--maze-gate]`: the three chances
    /// forced on the steps, whether dungeon `+0x34B6` was set to -1 at step
    /// 8's entry, the dungeon RNG state and `tilesFnv1a` at the entries of
    /// steps 9, 10 and 11, and the state the steps left in the floor-wide
    /// data at step 11's entry.
    struct FeatureRun {
        chances: [i32; 3],
        maze_gate: bool,
        checkpoints: [(u32, u64); 3],
        maze: bool,
        monster_house: bool,
        shop: bool,
        centre: (u16, u16),
        rect: [i32; 4],
        bounds: [i32; 4],
        house_room: u8,
    }

    /// A forced seed (or the natural one) of the first floor: grid height and
    /// the dungeon RNG state at the entries of steps 3 and 5, then its runs.
    struct FeatureRow {
        grid_y: usize,
        std3: u32,
        std5: u32,
        runs: &'static [FeatureRun],
    }

    /// Steps 1 to 7 of the first floor from the recorded states.
    fn floor_before_features(grid_y: usize, std3: u32, std5: u32) -> (Floor, Grid, DungeonRng) {
        let (_, _, mut floor, mut grid) = first_floor_steps(grid_y, std3);
        let mut rng = DungeonRng::from_state(std5);
        assign_connections(&mut grid, FIRST_FLOOR_WALK_STEPS, false, &mut rng);
        let (xs, ys) = grid_bounds(4, grid_y);
        create_hallways(&mut floor, &mut grid, &xs, &ys, false, &mut rng);
        ensure_connected_grid(&mut floor, &mut grid, &xs, &ys, &mut rng);
        (floor, grid, rng)
    }

    /// Rows: the natural seed and the three forced seeds with the four
    /// configurations (maze only, shop only, house only, all three), then 23
    /// more forced seeds with all three.
    const FEATURE_ROWS: &[FeatureRow] = &[
        // natural seed
        FeatureRow {
            grid_y: 2,
            std3: 1410305108,
            std5: 2134860938,
            runs: &[
                FeatureRun {
                    chances: [100, 0, 0],
                    maze_gate: true,
                    checkpoints: [
                        (1634322581, 0x99dd_76b2_c541_3162),
                        (1634322581, 0x99dd_76b2_c541_3162),
                        (1634322581, 0x99dd_76b2_c541_3162),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 100, 0],
                    maze_gate: false,
                    checkpoints: [
                        (717111574, 0x08aa_8f91_334e_3d50),
                        (1798613775, 0x312b_6d1e_5783_7670),
                        (1798613775, 0x312b_6d1e_5783_7670),
                    ],
                    maze: false,
                    monster_house: false,
                    shop: true,
                    centre: (6, 26),
                    rect: [5, 25, 8, 28],
                    bounds: [5, 25, 7, 27],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 0, 100],
                    maze_gate: false,
                    checkpoints: [
                        (717111574, 0x08aa_8f91_334e_3d50),
                        (717111574, 0x08aa_8f91_334e_3d50),
                        (2957822255, 0x021a_66d9_f9b5_bfd0),
                    ],
                    maze: false,
                    monster_house: true,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 2,
                },
                FeatureRun {
                    chances: [100, 100, 100],
                    maze_gate: true,
                    checkpoints: [
                        (1634322581, 0x99dd_76b2_c541_3162),
                        (2204092330, 0xf710_fd9b_97a0_cde2),
                        (983766035, 0xf710_fd9b_97a0_cde2),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: true,
                    centre: (20, 24),
                    rect: [17, 19, 24, 29],
                    bounds: [17, 19, 23, 28],
                    house_room: 255,
                },
            ],
        },
        // 0x12345678
        FeatureRow {
            grid_y: 2,
            std3: 985678915,
            std5: 3860814012,
            runs: &[
                FeatureRun {
                    chances: [100, 0, 0],
                    maze_gate: true,
                    checkpoints: [
                        (521387549, 0xbe2e_be97_cb90_b781),
                        (521387549, 0xbe2e_be97_cb90_b781),
                        (521387549, 0xbe2e_be97_cb90_b781),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 100, 0],
                    maze_gate: false,
                    checkpoints: [
                        (4001168455, 0x6185_0823_cb3c_1bb3),
                        (528987620, 0xba23_8e86_db30_a813),
                        (528987620, 0xba23_8e86_db30_a813),
                    ],
                    maze: false,
                    monster_house: false,
                    shop: true,
                    centre: (7, 11),
                    rect: [5, 10, 10, 13],
                    bounds: [5, 10, 9, 12],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 0, 100],
                    maze_gate: false,
                    checkpoints: [
                        (4001168455, 0x6185_0823_cb3c_1bb3),
                        (4001168455, 0x6185_0823_cb3c_1bb3),
                        (4245128324, 0x1654_d903_9acb_fa73),
                    ],
                    maze: false,
                    monster_house: true,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 3,
                },
                FeatureRun {
                    chances: [100, 100, 100],
                    maze_gate: true,
                    checkpoints: [
                        (521387549, 0xbe2e_be97_cb90_b781),
                        (548743762, 0x020e_2f56_7b73_dbe1),
                        (2611230811, 0x020e_2f56_7b73_dbe1),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: true,
                    centre: (7, 11),
                    rect: [5, 10, 10, 13],
                    bounds: [5, 10, 9, 12],
                    house_room: 255,
                },
            ],
        },
        // 0xCAFEBABF
        FeatureRow {
            grid_y: 2,
            std3: 1477956308,
            std5: 3196431495,
            runs: &[
                FeatureRun {
                    chances: [100, 0, 0],
                    maze_gate: true,
                    checkpoints: [
                        (35589997, 0x9202_d91f_d697_3383),
                        (35589997, 0x9202_d91f_d697_3383),
                        (35589997, 0x9202_d91f_d697_3383),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 100, 0],
                    maze_gate: false,
                    checkpoints: [
                        (2147750935, 0xf251_6ef3_4075_0e51),
                        (1760264948, 0xbcf8_9554_27aa_08b1),
                        (1760264948, 0xbcf8_9554_27aa_08b1),
                    ],
                    maze: false,
                    monster_house: false,
                    shop: true,
                    centre: (8, 21),
                    rect: [7, 19, 10, 24],
                    bounds: [7, 19, 9, 23],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 0, 100],
                    maze_gate: false,
                    checkpoints: [
                        (2147750935, 0xf251_6ef3_4075_0e51),
                        (2147750935, 0xf251_6ef3_4075_0e51),
                        (1244314004, 0xb572_543d_5347_e291),
                    ],
                    maze: false,
                    monster_house: true,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 1,
                },
                FeatureRun {
                    chances: [100, 100, 100],
                    maze_gate: true,
                    checkpoints: [
                        (35589997, 0x9202_d91f_d697_3383),
                        (1749313250, 0xa790_f224_de00_0663),
                        (3196669227, 0xa790_f224_de00_0663),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: true,
                    centre: (8, 21),
                    rect: [7, 19, 10, 24],
                    bounds: [7, 19, 9, 23],
                    house_room: 255,
                },
            ],
        },
        // 0x0BADF00D
        FeatureRow {
            grid_y: 3,
            std3: 3889763896,
            std5: 2379107274,
            runs: &[
                FeatureRun {
                    chances: [100, 0, 0],
                    maze_gate: true,
                    checkpoints: [
                        (2402210748, 0x48c3_24d9_d0d0_4e97),
                        (2402210748, 0x48c3_24d9_d0d0_4e97),
                        (2402210748, 0x48c3_24d9_d0d0_4e97),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 100, 0],
                    maze_gate: false,
                    checkpoints: [
                        (4249534461, 0x3299_603e_f26a_4dfd),
                        (318166962, 0x4302_16fe_d41d_9f7d),
                        (318166962, 0x4302_16fe_d41d_9f7d),
                    ],
                    maze: false,
                    monster_house: false,
                    shop: true,
                    centre: (20, 15),
                    rect: [18, 13, 23, 17],
                    bounds: [18, 13, 22, 16],
                    house_room: 255,
                },
                FeatureRun {
                    chances: [0, 0, 100],
                    maze_gate: false,
                    checkpoints: [
                        (4249534461, 0x3299_603e_f26a_4dfd),
                        (4249534461, 0x3299_603e_f26a_4dfd),
                        (146444626, 0xe9b1_2e60_556c_82bd),
                    ],
                    maze: false,
                    monster_house: true,
                    shop: false,
                    centre: (65535, 65535),
                    rect: [-1, -1, -1, -1],
                    bounds: [0, 0, 0, 0],
                    house_room: 0,
                },
                FeatureRun {
                    chances: [100, 100, 100],
                    maze_gate: true,
                    checkpoints: [
                        (2402210748, 0x48c3_24d9_d0d0_4e97),
                        (471689613, 0xdf09_4233_b872_bc57),
                        (2439132066, 0xdf09_4233_b872_bc57),
                    ],
                    maze: true,
                    monster_house: false,
                    shop: true,
                    centre: (20, 15),
                    rect: [18, 13, 23, 17],
                    bounds: [18, 13, 22, 16],
                    house_room: 255,
                },
            ],
        },
        // 0x08D12DFD
        FeatureRow {
            grid_y: 2,
            std3: 1976947032,
            std5: 284701749,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1482685133, 0xb243_3346_3e2b_e8ea),
                    (2261251010, 0x67a9_33bc_14f8_760a),
                    (49119627, 0x67a9_33bc_14f8_760a),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (20, 24),
                rect: [17, 22, 24, 27],
                bounds: [17, 22, 23, 26],
                house_room: 255,
            }],
        },
        // 0x11A25BFA
        FeatureRow {
            grid_y: 3,
            std3: 2937234321,
            std5: 1860216811,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1129668225, 0xbc4c_fafe_a003_4b05),
                    (3786867654, 0xbc4c_fafe_a003_4b05),
                    (2790326047, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0x28B7BC6F
        FeatureRow {
            grid_y: 3,
            std3: 3280667150,
            std5: 394156293,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1234432400, 0x99f8_655b_1034_795b),
                    (1485895729, 0x0fa2_f100_26ec_56bb),
                    (3318515286, 0x0fa2_f100_26ec_56bb),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (6, 14),
                rect: [5, 13, 8, 16],
                bounds: [5, 13, 7, 15],
                house_room: 255,
            }],
        },
        // 0x2E2AC0EA
        FeatureRow {
            grid_y: 2,
            std3: 278952215,
            std5: 898684437,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (745109359, 0xbc4c_fafe_a003_4b05),
                    (1032167596, 0xbc4c_fafe_a003_4b05),
                    (2333460445, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0x36FBEEE7
        FeatureRow {
            grid_y: 3,
            std3: 3033275370,
            std5: 3731767195,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1743667837, 0xbc4c_fafe_a003_4b05),
                    (4193686578, 0xbc4c_fafe_a003_4b05),
                    (2616171963, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0x3C6EF362
        FeatureRow {
            grid_y: 3,
            std3: 2733681987,
            std5: 2735416536,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (2138617104, 0x9a80_2fa2_5304_dbe2),
                    (3892828593, 0x240d_35a8_0294_cbe2),
                    (4028890582, 0x240d_35a8_0294_cbe2),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (19, 15),
                rect: [17, 13, 22, 17],
                bounds: [17, 13, 21, 16],
                house_room: 255,
            }],
        },
        // 0x3FCD1CE4
        FeatureRow {
            grid_y: 3,
            std3: 2617097491,
            std5: 3091852748,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (2129276439, 0xfce3_338f_d2b5_541d),
                    (1227932916, 0x19f5_437c_0c5b_429d),
                    (3004051525, 0x19f5_437c_0c5b_429d),
                ],
                maze: false,
                monster_house: false,
                shop: true,
                centre: (7, 15),
                rect: [5, 13, 10, 17],
                bounds: [5, 13, 9, 16],
                house_room: 255,
            }],
        },
        // 0x4540215F
        FeatureRow {
            grid_y: 3,
            std3: 3289384834,
            std5: 3735770515,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (755158840, 0xa75c_4def_bf7a_42d4),
                    (3421306745, 0x20ec_8a2e_2ff7_b034),
                    (3922689470, 0x20ec_8a2e_2ff7_b034),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (19, 15),
                rect: [17, 14, 22, 17],
                bounds: [17, 14, 21, 16],
                house_room: 255,
            }],
        },
        // 0x538453D7
        FeatureRow {
            grid_y: 2,
            std3: 475179094,
            std5: 2983244959,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (3453978614, 0x49be_58fb_03e9_7063),
                    (377360751, 0x8d0a_3456_2ae9_8363),
                    (2757502412, 0x8d0a_3456_2ae9_8363),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (6, 24),
                rect: [3, 19, 10, 29],
                bounds: [3, 19, 9, 28],
                house_room: 255,
            }],
        },
        // 0x5C5581D4
        FeatureRow {
            grid_y: 2,
            std3: 1815861487,
            std5: 3747341869,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1677308003, 0xdd63_be75_c4fd_fb92),
                    (775123696, 0x6cd2_3517_3bcc_3d92),
                    (3694438577, 0x6cd2_3517_3bcc_3d92),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (19, 25),
                rect: [17, 21, 22, 29],
                bounds: [17, 21, 21, 28],
                house_room: 255,
            }],
        },
        // 0x6A99B44C
        FeatureRow {
            grid_y: 3,
            std3: 3385425883,
            std5: 2404653519,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1659966976, 0xea56_ffab_9e1f_a316),
                    (2369589345, 0xbbac_53ba_bff8_dd96),
                    (3209961798, 0xbbac_53ba_bff8_dd96),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (5, 5),
                rect: [3, 3, 8, 7],
                bounds: [3, 3, 7, 6],
                house_room: 255,
            }],
        },
        // 0x78DDE6C4
        FeatureRow {
            grid_y: 3,
            std3: 3641535347,
            std5: 189740765,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (3769889763, 0xbc4c_fafe_a003_4b05),
                    (2661667952, 0xbc4c_fafe_a003_4b05),
                    (1480958001, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0x8A8042BE
        FeatureRow {
            grid_y: 3,
            std3: 2809179589,
            std5: 602133439,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (2446231134, 0xdc56_56ae_4b40_55ed),
                    (441017463, 0x47d0_1ca3_6273_54cd),
                    (4234756084, 0x47d0_1ca3_6273_54cd),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (20, 4),
                rect: [18, 3, 23, 6],
                bounds: [18, 3, 22, 5],
                house_room: 255,
            }],
        },
        // 0x8FF34739
        FeatureRow {
            grid_y: 3,
            std3: 2304828844,
            std5: 527474027,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (4233802329, 0xf85b_3582_03d6_515c),
                    (745489406, 0x4a6e_893e_13ac_633c),
                    (2535505207, 0x4a6e_893e_13ac_633c),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (6, 14),
                rect: [4, 13, 9, 16],
                bounds: [4, 13, 8, 15],
                house_room: 255,
            }],
        },
        // 0x98C47536
        FeatureRow {
            grid_y: 3,
            std3: 3902873959,
            std5: 1344564337,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (3950004238, 0xf533_f6a3_01c4_6d3e),
                    (1314137831, 0x87e2_d9c3_aa48_9a1e),
                    (3803891236, 0x87e2_d9c3_aa48_9a1e),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (19, 4),
                rect: [17, 3, 22, 6],
                bounds: [17, 3, 21, 5],
                house_room: 255,
            }],
        },
        // 0xA195A333
        FeatureRow {
            grid_y: 3,
            std3: 4281705724,
            std5: 2454085806,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (257296975, 0x0472_d239_7daa_bc69),
                    (4106879756, 0x8308_338c_1bf2_0fa9),
                    (2876946365, 0x8308_338c_1bf2_0fa9),
                ],
                maze: false,
                monster_house: false,
                shop: true,
                centre: (21, 5),
                rect: [19, 3, 24, 7],
                bounds: [19, 3, 23, 6],
                house_room: 255,
            }],
        },
        // 0xA708A7AE
        FeatureRow {
            grid_y: 3,
            std3: 3853845675,
            std5: 2451130894,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (3755740336, 0xbc4c_fafe_a003_4b05),
                    (1002191825, 0xbc4c_fafe_a003_4b05),
                    (1117131894, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0xB54CDA26
        FeatureRow {
            grid_y: 2,
            std3: 1249533463,
            std5: 2240918293,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (343456879, 0xbc4c_fafe_a003_4b05),
                    (3735443884, 0xbc4c_fafe_a003_4b05),
                    (2037499101, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0xBE1E0823
        FeatureRow {
            grid_y: 3,
            std3: 2238119330,
            std5: 2333177289,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1627689667, 0xbc4c_fafe_a003_4b05),
                    (763949776, 0xbc4c_fafe_a003_4b05),
                    (1516157969, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0xCC623A9B
        FeatureRow {
            grid_y: 3,
            std3: 3417439566,
            std5: 1590288527,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (605186286, 0xf1e2_6a50_f80b_1790),
                    (3562049351, 0x7b82_47d4_5ff1_5030),
                    (2365330436, 0x7b82_47d4_5ff1_5030),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (6, 15),
                rect: [4, 14, 9, 17],
                bounds: [4, 14, 8, 16],
                house_room: 255,
            }],
        },
        // 0xEC48C90D
        FeatureRow {
            grid_y: 3,
            std3: 2841193272,
            std5: 3064137930,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (3698024808, 0xbc4c_fafe_a003_4b05),
                    (1792820329, 0xbc4c_fafe_a003_4b05),
                    (1797521518, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0xF1BBCD88
        FeatureRow {
            grid_y: 3,
            std3: 3513480615,
            std5: 3226624689,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (2899397911, 0xbc4c_fafe_a003_4b05),
                    (2866249716, 0xbc4c_fafe_a003_4b05),
                    (157218629, 0xbc4c_fafe_a003_4b05),
                ],
                maze: false,
                monster_house: false,
                shop: false,
                centre: (65535, 65535),
                rect: [-1, -1, -1, -1],
                bounds: [0, 0, 0, 0],
                house_room: 255,
            }],
        },
        // 0xFA8CFB85
        FeatureRow {
            grid_y: 3,
            std3: 4002331848,
            std5: 490432218,
            runs: &[FeatureRun {
                chances: [100, 100, 100],
                maze_gate: true,
                checkpoints: [
                    (1811794642, 0xb7e1_9d1b_1850_4419),
                    (1540390971, 0xdc0e_f1e2_9b4e_af59),
                    (4062879816, 0xdc0e_f1e2_9b4e_af59),
                ],
                maze: true,
                monster_house: false,
                shop: true,
                centre: (20, 5),
                rect: [17, 3, 24, 7],
                bounds: [17, 3, 23, 6],
                house_room: 255,
            }],
        },
    ];

    #[test]
    fn maze_shop_and_monster_house_match_the_retail_floors() {
        for (i, row) in FEATURE_ROWS.iter().enumerate() {
            for (j, run) in row.runs.iter().enumerate() {
                let (mut floor, mut grid, mut rng) =
                    floor_before_features(row.grid_y, row.std3, row.std5);
                let mut features = RoomFeatures::default();
                let [maze, shop, house] = run.chances;
                let mut seen = Vec::new();
                generate_maze_room(
                    &mut floor,
                    &mut grid,
                    maze,
                    run.maze_gate,
                    &mut features,
                    &mut rng,
                );
                seen.push((rng.state(), tiles_fnv1a(&floor)));
                generate_shop(&mut floor, &mut grid, shop, 0, &mut features, &mut rng);
                seen.push((rng.state(), tiles_fnv1a(&floor)));
                generate_monster_house(
                    &mut floor,
                    &mut grid,
                    house,
                    0,
                    false,
                    &mut features,
                    &mut rng,
                );
                seen.push((rng.state(), tiles_fnv1a(&floor)));
                let what = format!("row {i}, run {j}");
                assert_eq!(seen, run.checkpoints, "{what}: RNG state and tiles");
                assert_eq!(features.maze, run.maze, "{what}: maze");
                assert_eq!(features.monster_house, run.monster_house, "{what}: house");
                assert_eq!(features.shop, run.shop, "{what}: shop");
                assert_eq!(features.shop_centre, run.centre, "{what}: centre");
                assert_eq!(features.shop_rect, run.rect, "{what}: rect");
                assert_eq!(features.shop_bounds, run.bounds, "{what}: bounds");
                assert_eq!(
                    features.monster_house_room, run.house_room,
                    "{what}: house room"
                );
            }
        }
    }

    /// Step 11 checkpoints from the retail ROM, in the order of
    /// [`NATURAL_SEED`] and [`FORCED_SEEDS`]: the dungeon RNG state and
    /// `tilesFnv1a` at the entry of step 12. The count (floor properties
    /// `+0x13`) is 5 on the first floor. Steps 8 to 10 draw nothing there.
    const EXTRA_HALLWAY_RESULTS: [(u32, u64); 4] = [
        (984_344_710, 0xb6dc_487d_352f_cc8e),
        (1_462_097_070, 0x8aae_f9a4_88cb_299e),
        (764_592_701, 0xf251_6ef3_4075_0e51),
        (1_671_193_971, 0x3299_603e_f26a_4dfd),
    ];

    /// The seed behind row `i` of [`FINISH_ROWS`]: the natural seed, the three
    /// forced seeds, then [`ENSURE_SEEDS`]. Returns the grid height and the
    /// RNG states at the entries of steps 3 and 5.
    fn seed_row(i: usize) -> (usize, u32, u32) {
        match i {
            0 => (NATURAL_SEED.grid_y, NATURAL_SEED.std3, NATURAL_SEED.std5),
            1..=3 => {
                let s = &FORCED_SEEDS[i - 1];
                (s.grid_y, s.std3, s.std5)
            }
            _ => {
                let &(grid_y, std3, std5, ..) = &ENSURE_SEEDS[i - 4];
                (grid_y, std3, std5)
            }
        }
    }

    /// Steps 12 and 13 and the three tile passes on the retail ROM, from
    /// `floor_stages.py --force-structures BUDGET`, which sets `imperfect` on
    /// the cells whose x + y is a multiple of 3, `structure` on all of them
    /// and `gFloorPropsByteC` to the budget (the first floor has none). One row
    /// per seed (see [`seed_row`]) and budget: the RNG state and `tilesFnv1a`
    /// at the entries of step 12 and step 13, the RNG state and hash at the
    /// entry of `ResetInnerBoundaryTileRows`, then the hash at the entries of
    /// `EnsureImpassableTilesAreWalls`, `FinalizeJunctions` and
    /// `GenerateSecondaryTerrainFormations`.
    type FinishRow = (
        usize,
        u8,
        (u32, u64),
        (u32, u64),
        (u32, u64),
        u64,
        Option<u64>,
        Option<u64>,
    );

    const FINISH_ROWS: &[FinishRow] = &[
        (
            0,
            100,
            (984344710, 0xb6dc_487d_352f_cc8e),
            (984344710, 0xb6dc_487d_352f_cc8e),
            (1910890242, 0x5d74_2f7e_8119_3995),
            0x5d74_2f7e_8119_3995,
            Some(0x5d74_2f7e_8119_3995),
            Some(0xdefc_5608_77c6_25a2),
        ),
        (
            0,
            1,
            (984344710, 0xb6dc_487d_352f_cc8e),
            (984344710, 0xb6dc_487d_352f_cc8e),
            (1910890242, 0x5d74_2f7e_8119_3995),
            0x5d74_2f7e_8119_3995,
            Some(0x5d74_2f7e_8119_3995),
            Some(0xdefc_5608_77c6_25a2),
        ),
        (
            1,
            100,
            (1462097070, 0x8aae_f9a4_88cb_299e),
            (1043817127, 0x8aae_f9a4_88cb_299e),
            (3303325147, 0x00a6_58dd_ca06_db54),
            0x9727_dba2_a9b9_c781,
            Some(0x9727_dba2_a9b9_c781),
            Some(0x0d23_9be7_7f07_4e79),
        ),
        (
            1,
            1,
            (1462097070, 0x8aae_f9a4_88cb_299e),
            (1043817127, 0x8aae_f9a4_88cb_299e),
            (3303325147, 0x502d_08e9_ec11_4a78),
            0xabc4_30d7_d983_31a5,
            Some(0xabc4_30d7_d983_31a5),
            Some(0x97c9_3ac3_6c54_c47d),
        ),
        (
            2,
            100,
            (764592701, 0xf251_6ef3_4075_0e51),
            (764592701, 0xf251_6ef3_4075_0e51),
            (1972965577, 0x2ba3_fe89_79ef_def0),
            0x2ba3_fe89_79ef_def0,
            Some(0x2ba3_fe89_79ef_def0),
            Some(0x1d41_3b13_340b_47f1),
        ),
        (
            2,
            1,
            (764592701, 0xf251_6ef3_4075_0e51),
            (764592701, 0xf251_6ef3_4075_0e51),
            (2073639753, 0xda0e_a6c1_41a5_004a),
            0xda0e_a6c1_41a5_004a,
            Some(0xda0e_a6c1_41a5_004a),
            Some(0xdcb2_a372_2f3b_d1dc),
        ),
        (
            3,
            100,
            (1671193971, 0x3299_603e_f26a_4dfd),
            (2282574908, 0x506f_ea15_31c2_7e0c),
            (1896501689, 0xb3d1_a9f4_2040_b961),
            0xb3d1_a9f4_2040_b961,
            Some(0xb3d1_a9f4_2040_b961),
            Some(0x924e_6faf_e169_b362),
        ),
        (
            3,
            1,
            (1671193971, 0x3299_603e_f26a_4dfd),
            (2282574908, 0x506f_ea15_31c2_7e0c),
            (4290763608, 0x113a_580a_0bff_7e26),
            0x113a_580a_0bff_7e26,
            Some(0x113a_580a_0bff_7e26),
            Some(0x8e66_da04_ca57_fdb5),
        ),
        (
            4,
            100,
            (3605883602, 0x0d7f_ccee_e07d_d81a),
            (3605883602, 0x0d7f_ccee_e07d_d81a),
            (49075254, 0x59f1_7f5b_45be_06fe),
            0x59f1_7f5b_45be_06fe,
            Some(0x59f1_7f5b_45be_06fe),
            Some(0x8fbc_c1dd_a5ba_237e),
        ),
        (
            4,
            1,
            (3605883602, 0x0d7f_ccee_e07d_d81a),
            (3605883602, 0x0d7f_ccee_e07d_d81a),
            (49075254, 0x59f1_7f5b_45be_06fe),
            0x59f1_7f5b_45be_06fe,
            Some(0x59f1_7f5b_45be_06fe),
            Some(0x8fbc_c1dd_a5ba_237e),
        ),
        (
            5,
            100,
            (3411281182, 0xbc4c_fafe_a003_4b05),
            (3411281182, 0xbc4c_fafe_a003_4b05),
            (2984695764, 0xbc4c_fafe_a003_4b05),
            0xbc4c_fafe_a003_4b05,
            None,
            None,
        ),
        (
            5,
            1,
            (3411281182, 0xbc4c_fafe_a003_4b05),
            (3411281182, 0xbc4c_fafe_a003_4b05),
            (2984695764, 0xbc4c_fafe_a003_4b05),
            0xbc4c_fafe_a003_4b05,
            None,
            None,
        ),
        (
            6,
            100,
            (1268549573, 0xed95_bbd0_b9c1_d18b),
            (254904250, 0xed95_bbd0_b9c1_d18b),
            (489458275, 0xed95_bbd0_b9c1_d18b),
            0xed95_bbd0_b9c1_d18b,
            Some(0xed95_bbd0_b9c1_d18b),
            Some(0x665a_0b12_353d_6918),
        ),
        (
            6,
            1,
            (1268549573, 0xed95_bbd0_b9c1_d18b),
            (254904250, 0xed95_bbd0_b9c1_d18b),
            (489458275, 0xed95_bbd0_b9c1_d18b),
            0xed95_bbd0_b9c1_d18b,
            Some(0xed95_bbd0_b9c1_d18b),
            Some(0x665a_0b12_353d_6918),
        ),
        (
            7,
            100,
            (2361972292, 0xbc4c_fafe_a003_4b05),
            (2361972292, 0xbc4c_fafe_a003_4b05),
            (477744371, 0x1de0_77ed_78bd_06c7),
            0x1de0_77ed_78bd_06c7,
            None,
            None,
        ),
        (
            7,
            1,
            (2361972292, 0xbc4c_fafe_a003_4b05),
            (2361972292, 0xbc4c_fafe_a003_4b05),
            (477744371, 0x1de0_77ed_78bd_06c7),
            0x1de0_77ed_78bd_06c7,
            None,
            None,
        ),
        (
            8,
            100,
            (3016989194, 0xbc4c_fafe_a003_4b05),
            (3016989194, 0xbc4c_fafe_a003_4b05),
            (70969569, 0xf753_22e8_5877_b023),
            0xf753_22e8_5877_b023,
            None,
            None,
        ),
        (
            8,
            1,
            (3016989194, 0xbc4c_fafe_a003_4b05),
            (3016989194, 0xbc4c_fafe_a003_4b05),
            (4003293024, 0xa580_482d_ba36_6ec9),
            0xa580_482d_ba36_6ec9,
            None,
            None,
        ),
        (
            9,
            100,
            (694667235, 0x97a2_8fad_e62b_87c6),
            (1082466802, 0xb5b4_d084_2d8d_b215),
            (1783792809, 0x4a03_00b0_bec2_7f02),
            0x4a03_00b0_bec2_7f02,
            Some(0x4a03_00b0_bec2_7f02),
            Some(0x7c30_aefb_9fed_d914),
        ),
        (
            9,
            1,
            (694667235, 0x97a2_8fad_e62b_87c6),
            (1082466802, 0xb5b4_d084_2d8d_b215),
            (1783792809, 0x4a03_00b0_bec2_7f02),
            0x4a03_00b0_bec2_7f02,
            Some(0x4a03_00b0_bec2_7f02),
            Some(0x7c30_aefb_9fed_d914),
        ),
        (
            10,
            100,
            (22117772, 0xfce3_338f_d2b5_541d),
            (22117772, 0xfce3_338f_d2b5_541d),
            (3362884669, 0xb8ec_8968_f04d_30cd),
            0xb8ec_8968_f04d_30cd,
            None,
            None,
        ),
        (
            10,
            1,
            (22117772, 0xfce3_338f_d2b5_541d),
            (22117772, 0xfce3_338f_d2b5_541d),
            (3362884669, 0xb8ec_8968_f04d_30cd),
            0xb8ec_8968_f04d_30cd,
            None,
            None,
        ),
        (
            11,
            100,
            (4170230507, 0x69c4_bab6_fda7_79ce),
            (3693350676, 0xb598_aa8b_3210_be36),
            (1897502771, 0xf9b6_5557_541f_e33a),
            0xf9b6_5557_541f_e33a,
            Some(0xf9b6_5557_541f_e33a),
            Some(0x3aea_1723_d328_847e),
        ),
        (
            11,
            1,
            (4170230507, 0x69c4_bab6_fda7_79ce),
            (3693350676, 0xb598_aa8b_3210_be36),
            (3382175894, 0x5bd3_0ac4_ea23_6153),
            0x5bd3_0ac4_ea23_6153,
            Some(0x5bd3_0ac4_ea23_6153),
            Some(0x9c8c_dc6e_355f_b646),
        ),
        (
            12,
            100,
            (3889543036, 0xad3d_351f_60c5_44ec),
            (2710219589, 0x5559_007e_6861_6e29),
            (2224169443, 0xdbce_7bb2_0eb1_6f99),
            0xdbce_7bb2_0eb1_6f99,
            Some(0xdbce_7bb2_0eb1_6f99),
            Some(0xd1c6_c871_b9a0_e218),
        ),
        (
            12,
            1,
            (3889543036, 0xad3d_351f_60c5_44ec),
            (2710219589, 0x5559_007e_6861_6e29),
            (2224169443, 0xdbce_7bb2_0eb1_6f99),
            0xdbce_7bb2_0eb1_6f99,
            Some(0xdbce_7bb2_0eb1_6f99),
            Some(0xd1c6_c871_b9a0_e218),
        ),
        (
            13,
            100,
            (1281755260, 0xfccd_8013_5d37_f380),
            (2172431597, 0xfccd_8013_5d37_f380),
            (3371545255, 0x2ca7_83bf_af25_8c94),
            0x2ca7_83bf_af25_8c94,
            Some(0x2ca7_83bf_af25_8c94),
            Some(0xb14a_19e7_3144_701d),
        ),
        (
            13,
            1,
            (1281755260, 0xfccd_8013_5d37_f380),
            (2172431597, 0xfccd_8013_5d37_f380),
            (3371545255, 0x59f3_9d57_ce95_06f0),
            0x59f3_9d57_ce95_06f0,
            Some(0x59f3_9d57_ce95_06f0),
            Some(0x6bf7_8b51_df9e_6e51),
        ),
        (
            14,
            100,
            (3499036911, 0x7ba6_a400_d850_b6dc),
            (747768289, 0x4976_6de2_3f47_0a65),
            (216034358, 0xc42a_69d7_cfd6_8bf3),
            0xc42a_69d7_cfd6_8bf3,
            Some(0xc42a_69d7_cfd6_8bf3),
            Some(0x9ab1_4a09_1edf_9f34),
        ),
        (
            14,
            1,
            (3499036911, 0x7ba6_a400_d850_b6dc),
            (747768289, 0x4976_6de2_3f47_0a65),
            (216034358, 0xc42a_69d7_cfd6_8bf3),
            0xc42a_69d7_cfd6_8bf3,
            Some(0xc42a_69d7_cfd6_8bf3),
            Some(0x9ab1_4a09_1edf_9f34),
        ),
        (
            15,
            100,
            (4241393544, 0xbc4c_fafe_a003_4b05),
            (4241393544, 0xbc4c_fafe_a003_4b05),
            (656177909, 0x7c08_71ef_3747_7d6b),
            0x7c08_71ef_3747_7d6b,
            None,
            None,
        ),
        (
            15,
            1,
            (4241393544, 0xbc4c_fafe_a003_4b05),
            (4241393544, 0xbc4c_fafe_a003_4b05),
            (656177909, 0x98c3_3c91_e2a0_ecc5),
            0x98c3_3c91_e2a0_ecc5,
            None,
            None,
        ),
        (
            16,
            100,
            (3518489234, 0x5937_2976_90dc_0d8c),
            (2176952731, 0x5937_2976_90dc_0d8c),
            (3792227278, 0xc08d_3b3f_c13f_4cbe),
            0xc08d_3b3f_c13f_4cbe,
            Some(0xc08d_3b3f_c13f_4cbe),
            Some(0xfdd6_d97a_7a10_dc46),
        ),
        (
            16,
            1,
            (3518489234, 0x5937_2976_90dc_0d8c),
            (2176952731, 0x5937_2976_90dc_0d8c),
            (1685590094, 0x4121_bf85_2c90_22b3),
            0x4121_bf85_2c90_22b3,
            Some(0x4121_bf85_2c90_22b3),
            Some(0x88b9_743e_3dcc_6cc3),
        ),
        (
            17,
            100,
            (1102739866, 0x7457_2bab_d8f0_0818),
            (1102739866, 0x7457_2bab_d8f0_0818),
            (552103601, 0x003a_2843_d41e_ac6b),
            0x7249_877b_4965_3190,
            Some(0x7249_877b_4965_3190),
            Some(0x6855_2a95_8ae2_7938),
        ),
        (
            17,
            1,
            (1102739866, 0x7457_2bab_d8f0_0818),
            (1102739866, 0x7457_2bab_d8f0_0818),
            (552103601, 0x003a_2843_d41e_ac6b),
            0x7249_877b_4965_3190,
            Some(0x7249_877b_4965_3190),
            Some(0x6855_2a95_8ae2_7938),
        ),
        (
            18,
            100,
            (145941314, 0x6ba1_89a3_54fd_4ec2),
            (145941314, 0x6ba1_89a3_54fd_4ec2),
            (3195584695, 0x1cfe_1c9e_43d2_e4b4),
            0x1cfe_1c9e_43d2_e4b4,
            Some(0x1cfe_1c9e_43d2_e4b4),
            Some(0x0c40_5957_0d19_4bb8),
        ),
        (
            18,
            1,
            (145941314, 0x6ba1_89a3_54fd_4ec2),
            (145941314, 0x6ba1_89a3_54fd_4ec2),
            (783820599, 0x59a9_6f8b_83b1_a1bb),
            0x59a9_6f8b_83b1_a1bb,
            Some(0x59a9_6f8b_83b1_a1bb),
            Some(0xc261_1a34_15c9_1747),
        ),
        (
            19,
            100,
            (4129670668, 0x8485_0304_62a2_63e5),
            (261072573, 0x8485_0304_62a2_63e5),
            (3461751451, 0x8485_0304_62a2_63e5),
            0x8485_0304_62a2_63e5,
            Some(0x8485_0304_62a2_63e5),
            Some(0x4acd_d94d_8d42_7fd2),
        ),
        (
            19,
            1,
            (4129670668, 0x8485_0304_62a2_63e5),
            (261072573, 0x8485_0304_62a2_63e5),
            (3461751451, 0x8485_0304_62a2_63e5),
            0x8485_0304_62a2_63e5,
            Some(0x8485_0304_62a2_63e5),
            Some(0x4acd_d94d_8d42_7fd2),
        ),
        (
            20,
            100,
            (624912457, 0xbc4c_fafe_a003_4b05),
            (624912457, 0xbc4c_fafe_a003_4b05),
            (2109458382, 0xbc4c_fafe_a003_4b05),
            0xbc4c_fafe_a003_4b05,
            None,
            None,
        ),
        (
            20,
            1,
            (624912457, 0xbc4c_fafe_a003_4b05),
            (624912457, 0xbc4c_fafe_a003_4b05),
            (2109458382, 0xbc4c_fafe_a003_4b05),
            0xbc4c_fafe_a003_4b05,
            None,
            None,
        ),
        (
            21,
            100,
            (1142681412, 0xbc4c_fafe_a003_4b05),
            (1142681412, 0xbc4c_fafe_a003_4b05),
            (3996174908, 0x7455_3816_294e_da9b),
            0x7455_3816_294e_da9b,
            None,
            None,
        ),
        (
            21,
            1,
            (1142681412, 0xbc4c_fafe_a003_4b05),
            (1142681412, 0xbc4c_fafe_a003_4b05),
            (2042907324, 0x05a7_159e_14b2_b28b),
            0x05a7_159e_14b2_b28b,
            None,
            None,
        ),
        (
            22,
            100,
            (321055208, 0xbc4c_fafe_a003_4b05),
            (321055208, 0xbc4c_fafe_a003_4b05),
            (3827654303, 0x9b8e_26a4_f7c5_91c5),
            0x9b8e_26a4_f7c5_91c5,
            None,
            None,
        ),
        (
            22,
            1,
            (321055208, 0xbc4c_fafe_a003_4b05),
            (321055208, 0xbc4c_fafe_a003_4b05),
            (3827654303, 0x9b8e_26a4_f7c5_91c5),
            0x9b8e_26a4_f7c5_91c5,
            None,
            None,
        ),
        (
            23,
            100,
            (3311669459, 0x1c63_f2d2_c771_9015),
            (2531686946, 0xbf05_73ed_f29d_8daa),
            (3078619033, 0xeffa_5916_c9d2_2000),
            0xeffa_5916_c9d2_2000,
            Some(0xeffa_5916_c9d2_2000),
            Some(0x91f5_2543_0e2e_20b9),
        ),
        (
            23,
            1,
            (3311669459, 0x1c63_f2d2_c771_9015),
            (2531686946, 0xbf05_73ed_f29d_8daa),
            (3078619033, 0xeffa_5916_c9d2_2000),
            0xeffa_5916_c9d2_2000,
            Some(0xeffa_5916_c9d2_2000),
            Some(0x91f5_2543_0e2e_20b9),
        ),
        (
            24,
            100,
            (2517180897, 0xbc4c_fafe_a003_4b05),
            (2517180897, 0xbc4c_fafe_a003_4b05),
            (664152237, 0x22c2_3cda_f5fc_0c03),
            0x22c2_3cda_f5fc_0c03,
            None,
            None,
        ),
        (
            24,
            1,
            (2517180897, 0xbc4c_fafe_a003_4b05),
            (2517180897, 0xbc4c_fafe_a003_4b05),
            (3789306172, 0x64e4_cebb_f09e_428b),
            0x64e4_cebb_f09e_428b,
            None,
            None,
        ),
        (
            25,
            100,
            (185948300, 0xbc4c_fafe_a003_4b05),
            (185948300, 0xbc4c_fafe_a003_4b05),
            (1930505907, 0x1ff6_b4ed_e93b_206b),
            0x1996_24e7_6959_3c8f,
            None,
            None,
        ),
        (
            25,
            1,
            (185948300, 0xbc4c_fafe_a003_4b05),
            (185948300, 0xbc4c_fafe_a003_4b05),
            (3860454858, 0x7280_6f2d_09e4_55c5),
            0x183e_1683_ce1b_a1c9,
            None,
            None,
        ),
        (
            26,
            100,
            (2691512074, 0xfae4_df19_8210_9f00),
            (499222972, 0xa0ef_896f_2f9a_b24b),
            (2009361197, 0xa0ef_896f_2f9a_b24b),
            0xa0ef_896f_2f9a_b24b,
            Some(0xa0ef_896f_2f9a_b24b),
            Some(0xe999_04fa_8662_7136),
        ),
        (
            26,
            1,
            (2691512074, 0xfae4_df19_8210_9f00),
            (499222972, 0xa0ef_896f_2f9a_b24b),
            (2009361197, 0xa0ef_896f_2f9a_b24b),
            0xa0ef_896f_2f9a_b24b,
            Some(0xa0ef_896f_2f9a_b24b),
            Some(0xe999_04fa_8662_7136),
        ),
    ];

    #[test]
    fn imperfections_structures_and_tile_passes_match_the_retail_floors() {
        let (mut cut, mut built, mut finished) = (0, 0, 0);
        for &(seed, budget, std12, std13, reset, ensure, finalize, secondary) in FINISH_ROWS {
            let what = format!("seed row {seed}, budget {budget}");
            let (grid_y, std3, std5) = seed_row(seed);
            let (mut floor, mut grid, mut rng) = floor_before_features(grid_y, std3, std5);
            generate_extra_hallways(&mut floor, &grid, GridMode::LeftHalf, 5, &mut rng);
            assert_eq!((rng.state(), tiles_fnv1a(&floor)), std12, "{what}: step 12");
            for x in 0..grid.grid_x {
                for y in 0..grid.grid_y {
                    let cell = grid.cell_mut(x, y);
                    cell.imperfect = (x + y) % 3 == 0;
                    cell.structure = true;
                }
            }
            generate_room_imperfections(&mut floor, &grid, &mut rng);
            assert_eq!((rng.state(), tiles_fnv1a(&floor)), std13, "{what}: step 13");
            cut += usize::from(tiles_fnv1a(&floor) != std12.1);
            let mut features = RoomFeatures::default();
            let mut left = budget;
            generate_secondary_structures(
                &mut floor,
                &mut grid,
                &mut left,
                &mut features,
                &mut rng,
            );
            assert_eq!(
                (rng.state(), tiles_fnv1a(&floor)),
                reset,
                "{what}: structures"
            );
            built += usize::from(left != budget);
            reset_inner_boundary_tile_rows(&mut floor);
            assert_eq!(tiles_fnv1a(&floor), ensure, "{what}: boundary rows");
            ensure_impassable_tiles_are_walls(&mut floor);
            // A layout the game rejects starts over, so later checkpoints are missing.
            let (Some(finalize), Some(secondary)) = (finalize, secondary) else {
                continue;
            };
            assert_eq!(tiles_fnv1a(&floor), finalize, "{what}: impassable walls");
            finalize_junctions(&mut floor);
            assert_eq!(tiles_fnv1a(&floor), secondary, "{what}: junctions");
            finished += 1;
        }
        // The seeds must exercise both steps.
        assert!(
            cut > 0 && built > 0 && finished > 0,
            "{cut} {built} {finished}"
        );
    }

    /// `GenerateSecondaryTerrainFormations` on the retail ROM, from
    /// `floor_stages.py --force-secondary PONDS` (the first floor turns it
    /// off): the grid height and the RNG states at the entries of steps 3 and
    /// 5, the number of ponds (`None`: not forced, so the step does nothing),
    /// then the RNG state and `tilesFnv1a` at the step's entry and at the entry
    /// of `SpawnNonEnemies`. The natural seed and the three forced seeds come
    /// first, then more forced seeds; the last RNG state includes the draw for
    /// the itemless house that comes between the two checkpoints.
    type SecondaryRow = (usize, u32, u32, Option<u8>, (u32, u64), (u32, u64));

    const SECONDARY_ROWS: &[SecondaryRow] = &[
        (
            2,
            1410305108,
            2134860938,
            None,
            (984344710, 0xbcf5b4263ff9d1e9),
            (421756639, 0xbcf5b4263ff9d1e9),
        ),
        (
            2,
            1410305108,
            2134860938,
            Some(3),
            (984344710, 0xbcf5b4263ff9d1e9),
            (810328609, 0x316b9e3419347ae9),
        ),
        (
            2,
            985678915,
            3860814012,
            None,
            (1462097070, 0x94d7066093b792c3),
            (1043817127, 0x94d7066093b792c3),
        ),
        (
            2,
            985678915,
            3860814012,
            Some(3),
            (1462097070, 0x94d7066093b792c3),
            (3452531741, 0x6d4964d2d16dad15),
        ),
        (
            2,
            1477956308,
            3196431495,
            None,
            (764592701, 0x520e24f7aa329aef),
            (2555756818, 0x520e24f7aa329aef),
        ),
        (
            2,
            1477956308,
            3196431495,
            Some(3),
            (764592701, 0x520e24f7aa329aef),
            (1244542666, 0xe091d9cf4c81bac3),
        ),
        (
            3,
            3889763896,
            2379107274,
            None,
            (1671193971, 0x4ef1904d755692ba),
            (3729880928, 0x4ef1904d755692ba),
        ),
        (
            3,
            3889763896,
            2379107274,
            Some(3),
            (1671193971, 0x4ef1904d755692ba),
            (3817175678, 0x8f4ceed0ecf63d06),
        ),
        (
            2,
            1031762507,
            704037417,
            Some(4),
            (3175693038, 0x3b95714954898ed5),
            (4225868913, 0x109549ddb52685d9),
        ),
        (
            3,
            4109621265,
            3458849308,
            Some(4),
            (670001766, 0x0e65868c85d25864),
            (3688638145, 0x4f3b47db1a438616),
        ),
        (
            2,
            1068349216,
            2834300022,
            Some(4),
            (3554079142, 0x0be7d7bb5cd325ea),
            (2558603250, 0xeac95c5c4155e18c),
        ),
        (
            2,
            2052513785,
            4193080618,
            Some(4),
            (2125820566, 0xd9e68b9f4f728716),
            (3121261179, 0x8abcb2c66ea9c98c),
        ),
        (
            2,
            752411725,
            2077545230,
            Some(4),
            (4103512569, 0x1740147978621222),
            (375417557, 0x480c8734d8daae6a),
        ),
        (
            2,
            1586214204,
            3871952690,
            Some(4),
            (2362988417, 0x44f5e3cf944e74f5),
            (3814387899, 0xb15dcdb61c6565b9),
        ),
        (
            3,
            2185703870,
            11885440,
            Some(4),
            (4063945599, 0x6b530485acda3696),
            (2916533633, 0x184b74d150a62c20),
        ),
        (
            3,
            3943766742,
            517664088,
            Some(0),
            (4055765272, 0x4d6f601f13eb38b3),
            (53342706, 0xb7752ceb966e5a9d),
        ),
        (
            3,
            3943766742,
            517664088,
            Some(12),
            (4055765272, 0x4d6f601f13eb38b3),
            (3522039756, 0x3caf02365cfbcf67),
        ),
        (
            3,
            3943766742,
            517664088,
            Some(4),
            (4055765272, 0x4d6f601f13eb38b3),
            (45155174, 0x1cd167097d6e538f),
        ),
        (
            2,
            1799538090,
            1915798426,
            Some(0),
            (3355430540, 0x6f921a898a7e8287),
            (3717959798, 0x4437c24d9b691abd),
        ),
        (
            2,
            1799538090,
            1915798426,
            Some(12),
            (3355430540, 0x6f921a898a7e8287),
            (219245982, 0x51de4670199e6123),
        ),
        (
            2,
            1799538090,
            1915798426,
            Some(4),
            (3355430540, 0x6f921a898a7e8287),
            (542402862, 0x5bc02da901302427),
        ),
        (
            2,
            582192186,
            4158871296,
            Some(4),
            (3268438712, 0xef0b583623b52e62),
            (1977936500, 0x513e3cc411cc3704),
        ),
        (
            3,
            3322329749,
            1905100559,
            Some(4),
            (1973797867, 0x931b6949a85d125f),
            (3173951125, 0xaf0251c36b69b6df),
        ),
        (
            2,
            184663476,
            79693546,
            Some(4),
            (1932714545, 0x238a1d0b045578de),
            (824320746, 0x75222e19bc15b526),
        ),
        (
            2,
            2094663780,
            149633562,
            Some(4),
            (1742621029, 0xf1ed5442ac1ecd37),
            (2900350877, 0x0da1da2193ff823b),
        ),
        (
            3,
            2715685190,
            2303111271,
            Some(4),
            (4262172640, 0x211efacf96ef864f),
            (1258307009, 0xed6fa9232e18ebfb),
        ),
        (
            2,
            702125919,
            2280820214,
            Some(4),
            (2282005893, 0x8fd674f108ae0142),
            (3119072417, 0x59e22f9b970e0bc8),
        ),
        (
            2,
            248168963,
            1668987260,
            Some(4),
            (1531826, 0x9059d0c1d932d48d),
            (2852704713, 0xc376f0c4e7c242ab),
        ),
    ];

    #[test]
    fn secondary_terrain_matches_the_retail_floors() {
        let mut changed = 0;
        for &(grid_y, std3, std5, ponds, entry, spawn) in SECONDARY_ROWS {
            let what = format!("{std3} {ponds:?}");
            let (mut floor, grid, mut rng) = floor_before_features(grid_y, std3, std5);
            generate_extra_hallways(&mut floor, &grid, GridMode::LeftHalf, 5, &mut rng);
            reset_inner_boundary_tile_rows(&mut floor);
            ensure_impassable_tiles_are_walls(&mut floor);
            finalize_junctions(&mut floor);
            assert_eq!((rng.state(), tiles_fnv1a(&floor)), entry, "{what}: entry");
            generate_secondary_terrain(
                &mut floor,
                u8::from(ponds.is_some()),
                ponds.unwrap_or(0),
                &mut rng,
            );
            changed += usize::from(tiles_fnv1a(&floor) != entry.1);
            rng.int(100);
            assert_eq!((rng.state(), tiles_fnv1a(&floor)), spawn, "{what}: spawn");
        }
        assert!(changed > 10, "{changed}");
    }

    /// `SpawnNonEnemies` on the retail ROM, from `floor_stages.py` (rows after
    /// the first four use `--force-spawn`; the `h` rows add `--force-chance
    /// 10=100`, a monster house): the grid height and the RNG states at the
    /// entries of steps 3 and 5, the floor-properties counts, the entry of
    /// `SpawnNonEnemies` (RNG state and `tilesFnv1a`) and the entry of
    /// `SpawnEnemies` (RNG state, `spawnFnv1a`, `tilesFnv1a`), the leader and
    /// stairs positions, and the RNG steps between the two.
    struct SpawnRow {
        grid_y: usize,
        std3: u32,
        std5: u32,
        house: bool,
        counts: SpawnCounts,
        itemless: bool,
        trap_flag: bool,
        entry: (u32, u64),
        after: (u32, u64, u64),
        leader: (i16, i16),
        stairs: (i16, i16),
        steps: usize,
    }

    const SPAWN_ROWS: &[SpawnRow] = &[
        // nat
        SpawnRow {
            grid_y: 2,
            std3: 1410305108,
            std5: 2134860938,
            house: false,
            counts: SpawnCounts {
                items: 2,
                traps: 0,
                buried: 0,
            },
            itemless: false,
            trap_flag: false,
            entry: (421756639, 0xbcf5b4263ff9d1e9),
            after: (2253708027, 0xad01dfd2d2a3157e, 0xbcf5b4263ff9d1e9),
            leader: (20, 25),
            stairs: (23, 8),
            steps: 788,
        },
        // f1
        SpawnRow {
            grid_y: 2,
            std3: 985678915,
            std5: 3860814012,
            house: false,
            counts: SpawnCounts {
                items: 2,
                traps: 0,
                buried: 0,
            },
            itemless: false,
            trap_flag: false,
            entry: (1043817127, 0x94d7066093b792c3),
            after: (1303458447, 0xf9b1bd37df0df12e, 0x94d7066093b792c3),
            leader: (9, 24),
            stairs: (7, 27),
            steps: 760,
        },
        // f2
        SpawnRow {
            grid_y: 2,
            std3: 1477956308,
            std5: 3196431495,
            house: false,
            counts: SpawnCounts {
                items: 2,
                traps: 0,
                buried: 0,
            },
            itemless: false,
            trap_flag: false,
            entry: (2555756818, 0x520e24f7aa329aef),
            after: (3861683090, 0xd7a7a00ea6a52fa6, 0x520e24f7aa329aef),
            leader: (23, 28),
            stairs: (20, 25),
            steps: 384,
        },
        // f3
        SpawnRow {
            grid_y: 3,
            std3: 3889763896,
            std5: 2379107274,
            house: false,
            counts: SpawnCounts {
                items: 2,
                traps: 0,
                buried: 0,
            },
            itemless: false,
            trap_flag: false,
            entry: (3729880928, 0x4ef1904d755692ba),
            after: (2696250568, 0xa39e79e99997830e, 0x4ef1904d755692ba),
            leader: (19, 13),
            stairs: (21, 14),
            steps: 408,
        },
        // a1
        SpawnRow {
            grid_y: 2,
            std3: 985678915,
            std5: 3860814012,
            house: false,
            counts: SpawnCounts {
                items: 6,
                traps: 8,
                buried: 5,
            },
            itemless: false,
            trap_flag: true,
            entry: (1043817127, 0x94d7066093b792c3),
            after: (3391043811, 0x4d9dcffa260e3e7e, 0x94d7066093b792c3),
            leader: (22, 3),
            stairs: (7, 27),
            steps: 7668,
        },
        // a2
        SpawnRow {
            grid_y: 2,
            std3: 1477956308,
            std5: 3196431495,
            house: false,
            counts: SpawnCounts {
                items: 3,
                traps: 4,
                buried: 0,
            },
            itemless: false,
            trap_flag: false,
            entry: (2555756818, 0x520e24f7aa329aef),
            after: (1393630876, 0x50aead524190c110, 0x520e24f7aa329aef),
            leader: (10, 21),
            stairs: (20, 25),
            steps: 750,
        },
        // a3
        SpawnRow {
            grid_y: 3,
            std3: 3889763896,
            std5: 2379107274,
            house: false,
            counts: SpawnCounts {
                items: 8,
                traps: 12,
                buried: 4,
            },
            itemless: false,
            trap_flag: true,
            entry: (3729880928, 0x4ef1904d755692ba),
            after: (575383652, 0x9150252763ef68be, 0x4ef1904d755692ba),
            leader: (4, 15),
            stairs: (21, 14),
            steps: 7260,
        },
        // h1
        SpawnRow {
            grid_y: 2,
            std3: 985678915,
            std5: 3860814012,
            house: true,
            counts: SpawnCounts {
                items: 4,
                traps: 5,
                buried: 2,
            },
            itemless: false,
            trap_flag: true,
            entry: (3817659253, 0xcc0288f32104c959),
            after: (757834862, 0xda0a92d5f9d6069a, 0xcc0288f32104c959),
            leader: (7, 11),
            stairs: (7, 27),
            steps: 7613,
        },
        // h2
        SpawnRow {
            grid_y: 2,
            std3: 1477956308,
            std5: 3196431495,
            house: true,
            counts: SpawnCounts {
                items: 4,
                traps: 5,
                buried: 2,
            },
            itemless: false,
            trap_flag: true,
            entry: (1268302414, 0x0c802c777a0f272f),
            after: (859905314, 0x2eb9f26dff8e2f3e, 0x0c802c777a0f272f),
            leader: (9, 19),
            stairs: (23, 29),
            steps: 748,
        },
        // h3
        SpawnRow {
            grid_y: 3,
            std3: 3889763896,
            std5: 2379107274,
            house: true,
            counts: SpawnCounts {
                items: 4,
                traps: 5,
                buried: 2,
            },
            itemless: false,
            trap_flag: false,
            entry: (961689483, 0x765db24b5429815f),
            after: (2357950828, 0x9cd71bac77df0e7e, 0x765db24b5429815f),
            leader: (18, 25),
            stairs: (21, 13),
            steps: 7245,
        },
        // h4
        SpawnRow {
            grid_y: 2,
            std3: 181269758,
            std5: 1729437732,
            house: true,
            counts: SpawnCounts {
                items: 3,
                traps: 9,
                buried: 1,
            },
            itemless: false,
            trap_flag: true,
            entry: (2253548544, 0x3ab4f818bf0688fa),
            after: (2740741813, 0xabddcf47d0fd6f62, 0x3ab4f818bf0688fa),
            leader: (16, 22),
            stairs: (20, 24),
            steps: 7357,
        },
        // r1
        SpawnRow {
            grid_y: 3,
            std3: 3145669994,
            std5: 904877196,
            house: false,
            counts: SpawnCounts {
                items: 5,
                traps: 7,
                buried: 3,
            },
            itemless: false,
            trap_flag: true,
            entry: (2881710534, 0x4b37c30632c17cd0),
            after: (243286298, 0xac27a59c94e72472, 0x4b37c30632c17cd0),
            leader: (3, 7),
            stairs: (4, 6),
            steps: 7148,
        },
        // r2
        SpawnRow {
            grid_y: 3,
            std3: 3200292023,
            std5: 175351402,
            house: false,
            counts: SpawnCounts {
                items: 1,
                traps: 3,
                buried: 2,
            },
            itemless: false,
            trap_flag: true,
            entry: (1093691434, 0xd5de6eede9abb21a),
            after: (4037258089, 0xa4820cd6f2172516, 0xd5de6eede9abb21a),
            leader: (22, 26),
            stairs: (2, 27),
            steps: 699,
        },
    ];

    fn unset_spawn_state() -> SpawnState {
        SpawnState {
            leader: (-1, -1),
            stairs: (-1, -1),
            stairs_room: 0,
            house_room: NO_ROOM,
        }
    }

    #[test]
    fn spawn_non_enemies_matches_the_retail_floors() {
        let (mut houses, mut buried, mut traps) = (0, 0, 0);
        for (i, row) in SPAWN_ROWS.iter().enumerate() {
            let what = format!("row {i}");
            let (mut floor, mut grid, mut rng) =
                floor_before_features(row.grid_y, row.std3, row.std5);
            if row.house {
                let mut features = RoomFeatures::default();
                assert!(generate_monster_house(
                    &mut floor,
                    &mut grid,
                    100,
                    0,
                    false,
                    &mut features,
                    &mut rng
                ));
            }
            generate_extra_hallways(&mut floor, &grid, GridMode::LeftHalf, 5, &mut rng);
            reset_inner_boundary_tile_rows(&mut floor);
            ensure_impassable_tiles_are_walls(&mut floor);
            finalize_junctions(&mut floor);
            generate_secondary_terrain(&mut floor, 0, 0, &mut rng);
            rng.int(100);
            assert_eq!(
                (rng.state(), tiles_fnv1a(&floor)),
                row.entry,
                "{what}: entry"
            );

            let mut state = unset_spawn_state();
            spawn_non_enemies(
                &mut floor,
                row.counts,
                row.itemless,
                0,
                row.trap_flag,
                &mut state,
                &mut rng,
            );
            let got = (rng.state(), spawn_fnv1a(&floor), tiles_fnv1a(&floor));
            assert_eq!(got, row.after, "{what}: RNG state and tiles");
            assert_eq!(state.leader, row.leader, "{what}: leader");
            assert_eq!(state.stairs, row.stairs, "{what}: stairs");

            let mut counted = DungeonRng::from_state(row.entry.0);
            let mut steps = 0;
            while counted.state() != row.after.0 {
                counted.next_u16();
                steps += 1;
            }
            assert_eq!(steps, row.steps, "{what}: RNG steps");
            houses += usize::from(row.house);
            buried += usize::from(row.counts.buried > 0);
            traps += usize::from(row.counts.traps > 0);
        }
        // The rows must exercise the monster-house, buried-item and trap branches.
        assert!(
            houses > 0 && buried > 0 && traps > 0,
            "{houses} {buried} {traps}"
        );
    }

    /// A floor of one 10 x 6 room (room 3) in an otherwise walled map.
    fn one_room_floor() -> Floor {
        let mut floor = Floor::new();
        for x in 5..15 {
            for y in 5..11 {
                open_tile(&mut floor, x, y, 3);
            }
        }
        floor
    }

    /// Branches the first floor cannot reach, read from the disassembly: with
    /// both pairs set and no counts, the step draws nothing; floor type 2 turns
    /// the stairs' room into a monster house; `isEmptyMonsterHouse` skips the
    /// house items and the trap flag picks what a house tile gets.
    #[test]
    fn spawn_non_enemies_gates() {
        let counts = SpawnCounts::default();
        let set = SpawnState {
            leader: (6, 6),
            stairs: (7, 7),
            stairs_room: 9,
            house_room: NO_ROOM,
        };
        let mut floor = one_room_floor();
        let mut rng = DungeonRng::from_state(77);
        let mut state = set;
        spawn_non_enemies(&mut floor, counts, false, 0, true, &mut state, &mut rng);
        assert_eq!((rng.state(), state), (77, set));
        assert_eq!(spawn_fnv1a(&floor), spawn_fnv1a(&one_room_floor()));

        // Unset pairs cost one draw each (60 candidates both times: the stairs
        // tile stays a leader candidate); the stairs lose the item bit and floor
        // type 2 flags their room.
        let mut floor = one_room_floor();
        let mut state = unset_spawn_state();
        let mut rng = DungeonRng::from_state(77);
        spawn_non_enemies(&mut floor, counts, true, 2, true, &mut state, &mut rng);
        let mut expected = DungeonRng::from_state(77);
        expected.int(60);
        expected.int(60);
        assert_eq!(rng.state(), expected.state());
        let (sx, sy) = (i32::from(state.stairs.0), i32::from(state.stairs.1));
        assert_eq!(floor.tile(sx, sy).unwrap().spawn_or_visibility, 1);
        assert_eq!((state.stairs_room, state.house_room), (3, 3));
        assert!(floor.tile(6, 6).unwrap().terrain & TERRAIN_MONSTER_HOUSE != 0);

        // A 60-tile monster house gets 6 to 7 marks. An empty house skips them
        // with no draw; the trap flag only decides what a zero draw becomes.
        let marked = |floor: &Floor, bit: u16| tile_positions(floor, |_, s, _| s & bit != 0).len();
        let mut house = one_room_floor();
        for x in 5..15 {
            for y in 5..11 {
                house.tile_mut(x, y).unwrap().terrain |= TERRAIN_MONSTER_HOUSE;
            }
        }
        let go = |empty: bool, flag: bool| {
            let mut floor = house.clone();
            let mut state = set;
            let mut rng = DungeonRng::from_state(5);
            spawn_non_enemies(&mut floor, counts, empty, 0, flag, &mut state, &mut rng);
            (floor, rng.state())
        };
        let (skipped, rng_skipped) = go(true, true);
        assert_eq!((rng_skipped, marked(&skipped, 7)), (5, 0));
        let (with_flag, _) = go(false, true);
        let (without, _) = go(false, false);
        assert_eq!(marked(&with_flag, 2), marked(&without, 2));
        assert_eq!(marked(&without, 4), 0);
        assert!((1..=7).contains(&(marked(&with_flag, 2) + marked(&with_flag, 4))));
    }

    #[test]
    fn extra_hallways_match_the_retail_rng_budget_and_tiles() {
        let cases = std::iter::once(NATURAL_SEED).chain(FORCED_SEEDS);
        for (i, (stages, (std12, tiles))) in cases.zip(EXTRA_HALLWAY_RESULTS).enumerate() {
            let (mut floor, grid, mut rng) =
                floor_before_features(stages.grid_y, stages.std3, stages.std5);
            generate_extra_hallways(&mut floor, &grid, GridMode::LeftHalf, 5, &mut rng);
            assert_eq!(rng.state(), std12, "case {i}, step 12");
            assert_eq!(tiles_fnv1a(&floor), tiles, "case {i}, tiles");
        }
    }

    #[test]
    fn features_with_zero_chance_draw_nothing() {
        for stages in std::iter::once(NATURAL_SEED).chain(FORCED_SEEDS) {
            let (mut floor, mut grid, mut rng) =
                floor_before_features(stages.grid_y, stages.std3, stages.std5);
            let (before, tiles) = (rng.state(), tiles_fnv1a(&floor));
            let mut features = RoomFeatures::default();
            assert!(!generate_maze_room(
                &mut floor,
                &mut grid,
                0,
                true,
                &mut features,
                &mut rng
            ));
            assert!(!generate_shop(
                &mut floor,
                &mut grid,
                0,
                0,
                &mut features,
                &mut rng
            ));
            assert!(!generate_monster_house(
                &mut floor,
                &mut grid,
                0,
                0,
                false,
                &mut features,
                &mut rng
            ));
            assert_eq!(rng.state(), before);
            assert_eq!(tiles_fnv1a(&floor), tiles);
            assert_eq!(features, RoomFeatures::default());
        }
    }

    /// Gates the retail floors cannot reach (read from the disassembly): a
    /// closed maze gate still costs the chance draw; a monster house already
    /// on the floor, or floor type 2, stops the shop before any draw; a shop,
    /// a blocked house or another floor type stops the house after its draw.
    #[test]
    fn feature_gates_cost_the_documented_draws() {
        let stages = NATURAL_SEED;
        let setup = || floor_before_features(stages.grid_y, stages.std3, stages.std5);
        let one_draw = {
            let mut rng = DungeonRng::from_state(setup().2.state());
            rng.int(100);
            rng.state()
        };
        let start = setup().2.state();

        let (mut floor, mut grid, mut rng) = setup();
        let mut f = RoomFeatures::default();
        assert!(!generate_maze_room(
            &mut floor, &mut grid, 100, false, &mut f, &mut rng
        ));
        assert_eq!(rng.state(), one_draw);

        let (mut floor, mut grid, mut rng) = setup();
        let mut f = RoomFeatures {
            monster_house: true,
            ..RoomFeatures::default()
        };
        assert!(!generate_shop(
            &mut floor, &mut grid, 100, 0, &mut f, &mut rng
        ));
        assert_eq!(rng.state(), start);
        let mut f = RoomFeatures::default();
        assert!(!generate_shop(
            &mut floor, &mut grid, 100, 2, &mut f, &mut rng
        ));
        assert_eq!(rng.state(), start);

        for (shop, blocked, floor_type) in [(true, false, 0), (false, true, 0), (false, false, 1)] {
            let (mut floor, mut grid, mut rng) = setup();
            let mut f = RoomFeatures {
                shop,
                ..RoomFeatures::default()
            };
            assert!(!generate_monster_house(
                &mut floor, &mut grid, 100, floor_type, blocked, &mut f, &mut rng
            ));
            assert_eq!(rng.state(), one_draw);
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
