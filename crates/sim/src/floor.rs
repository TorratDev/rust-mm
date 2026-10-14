//! Dungeon floor map and the first steps of floor generation.
//!
//! The layout follows the game's (see `docs/re/structs.md`, "Tile", and
//! `docs/re/dungeon-generation.md`), but the types are our own: a floor is a
//! 56 x 32 grid of [`Tile`]s. Generation is being reimplemented step by step and
//! checked against `verification/v7-trace/floor_stages.py`, which records the
//! dungeon RNG state at every step of the game's generator.

/// Map width in tiles.
pub const WIDTH: usize = 56;
/// Map height in tiles.
pub const HEIGHT: usize = 32;

/// Terrain flag: impassable wall (the map border is always one).
pub const TERRAIN_IMPASSABLE_WALL: u16 = 0x10;
/// Room number of a tile outside every room.
pub const NO_ROOM: u8 = 0xFF;

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

    #[test]
    fn grid_bounds_round_down() {
        let (xs, ys) = grid_bounds(5, 3);
        assert_eq!(xs, [0, 11, 22, 33, 44, 55]);
        assert_eq!(ys, [0, 10, 20, 30]);
    }
}
