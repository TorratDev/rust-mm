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
