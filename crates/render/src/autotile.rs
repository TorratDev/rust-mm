//! Dungeon floor autotiling: the metatile for every cell of the 56x32 grid.
//!
//! Plain data in, plain data out (no SDL, no sim types). The terrain grid and the
//! tileset's tables come from the caller, and so do the variant draws, so the sim can
//! feed its own RNG later.
//!
//! Rules, as the original does them (found by static and runtime checks, see #60):
//! - `kind = terrain & 3`. Neighbour mask bits: S (x, y+1) 0x01, SE 0x02, E (x+1, y)
//!   0x04, NE 0x08, N (x, y-1) 0x10, NW 0x20, W (x-1, y) 0x40, SW 0x80. Out of bounds
//!   counts as wall.
//! - Wall (0): bit set when the neighbour is a wall, block 0. Floor (1): bit set when
//!   the neighbour is not a wall, block 0x200. Secondary (2) and both (3): bit set when
//!   the neighbour has the same kind, block 0x100.
//! - Metatile = `cex[(block | mask) * 3 + variant]`. The variant is one `RandInt(4)` draw
//!   per cell (walls too, y outer, x inner), with 3 read as 0, and always 0 when the
//!   floor's fixed-room flag is set.
//! - Tilesets from 0x40 on use no table: the metatile is `emap[y * 24 + x]` for
//!   `x, y < 24` and 0 elsewhere, and no draws are made.

/// Grid width in cells.
pub const GRID_W: usize = 56;
/// Grid height in cells.
pub const GRID_H: usize = 32;
/// Bytes the autotile table (`cex`) needs: 3 blocks x 256 masks x 3 variants.
pub const CEX_LEN: usize = 3 * 256 * 3;
/// First tileset number that uses the fixed map (`emap`) instead of the table.
pub const FIRST_EMAP_TILESET: u16 = 0x40;
/// Side of the fixed map, in cells.
const EMAP_SIDE: usize = 24;

/// Why autotiling could not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutotileError {
    /// The terrain grid is not 56x32 cells.
    BadGrid,
    /// The autotile table is shorter than [`CEX_LEN`].
    ShortTable,
}

impl std::fmt::Display for AutotileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AutotileError::BadGrid => "terrain grid is not 56x32",
            AutotileError::ShortTable => "autotile table is shorter than 2304 bytes",
        })
    }
}

impl std::error::Error for AutotileError {}

/// Terrain kind of `(x, y)` (`terrain & 3`); out of bounds is wall (0).
fn kind(terrain: &[u16], x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 || x >= GRID_W as i32 || y >= GRID_H as i32 {
        return 0;
    }
    (terrain[y as usize * GRID_W + x as usize] & 3) as u8
}

/// `block | mask` for cell `(x, y)`: the table row of that cell (before the variant).
/// `terrain` must hold 56x32 words, row-major.
pub fn cell_key(terrain: &[u16], x: usize, y: usize) -> u16 {
    let (xi, yi) = (x as i32, y as i32);
    let own = kind(terrain, xi, yi);
    let (block, test): (u16, fn(u8, u8) -> bool) = match own {
        0 => (0x000, |_, n| n == 0),
        1 => (0x200, |_, n| n != 0),
        _ => (0x100, |own, n| n == own),
    };
    // (dx, dy, bit): S, SE, E, NE, N, NW, W, SW.
    const NEIGHBOURS: [(i32, i32, u16); 8] = [
        (0, 1, 0x01),
        (1, 1, 0x02),
        (1, 0, 0x04),
        (1, -1, 0x08),
        (0, -1, 0x10),
        (-1, -1, 0x20),
        (-1, 0, 0x40),
        (-1, 1, 0x80),
    ];
    let mask = NEIGHBOURS
        .iter()
        .filter(|(dx, dy, _)| test(own, kind(terrain, xi + dx, yi + dy)))
        .fold(0, |m, (_, _, bit)| m | bit);
    block | mask
}

/// Metatile of every cell, row-major (`y * 56 + x`).
///
/// `draw` returns one `RandInt(4)` result (0 to 3) per call; it is called once per cell
/// in y-outer, x-inner order, except for tilesets that use `emap`. `emap` is the fixed
/// 24x24 map, used only for tilesets from [`FIRST_EMAP_TILESET`].
pub fn autotile(
    terrain: &[u16],
    tileset: u16,
    cex: &[u8],
    emap: &[u8],
    fixed_room: bool,
    mut draw: impl FnMut() -> u32,
) -> Result<Vec<u8>, AutotileError> {
    if terrain.len() != GRID_W * GRID_H {
        return Err(AutotileError::BadGrid);
    }
    let mut out = vec![0u8; GRID_W * GRID_H];
    if tileset >= FIRST_EMAP_TILESET {
        for y in 0..EMAP_SIDE.min(GRID_H) {
            for x in 0..EMAP_SIDE.min(GRID_W) {
                out[y * GRID_W + x] = emap.get(y * EMAP_SIDE + x).copied().unwrap_or(0);
            }
        }
        return Ok(out);
    }
    if cex.len() < CEX_LEN {
        return Err(AutotileError::ShortTable);
    }
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let v = draw();
            let variant = if fixed_room || v >= 3 { 0 } else { v as usize };
            let key = usize::from(cell_key(terrain, x, y));
            out[y * GRID_W + x] = cex[key * 3 + variant];
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(f: impl Fn(usize, usize) -> u16) -> Vec<u16> {
        (0..GRID_W * GRID_H)
            .map(|i| f(i % GRID_W, i / GRID_W))
            .collect()
    }

    /// A table where entry `i` is `i % 251` (distinct enough per key/variant).
    fn table() -> Vec<u8> {
        (0..CEX_LEN).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn open_wall_has_all_bits_and_block_0() {
        let t = grid(|_, _| 0);
        assert_eq!(cell_key(&t, 10, 10), 0x0FF);
        // Corner: out of bounds counts as wall, so still all bits.
        assert_eq!(cell_key(&t, 0, 0), 0x0FF);
    }

    #[test]
    fn lone_floor_has_no_bits_and_block_0x200() {
        let t = grid(|x, y| u16::from(x == 10 && y == 10));
        assert_eq!(cell_key(&t, 10, 10), 0x200);
    }

    #[test]
    fn each_neighbour_sets_its_own_bit_for_floor() {
        // Floor at the centre; one floor neighbour at a time.
        for (dx, dy, bit) in [
            (0i32, 1i32, 0x01u16),
            (1, 1, 0x02),
            (1, 0, 0x04),
            (1, -1, 0x08),
            (0, -1, 0x10),
            (-1, -1, 0x20),
            (-1, 0, 0x40),
            (-1, 1, 0x80),
        ] {
            let t = grid(|x, y| {
                let centre = x == 10 && y == 10;
                let n = x as i32 == 10 + dx && y as i32 == 10 + dy;
                u16::from(centre || n)
            });
            assert_eq!(cell_key(&t, 10, 10), 0x200 | bit, "({dx},{dy})");
        }
    }

    #[test]
    fn east_and_west_are_different_bits() {
        let east = grid(|x, y| u16::from(y == 5 && (x == 5 || x == 6)));
        let west = grid(|x, y| u16::from(y == 5 && (x == 5 || x == 4)));
        assert_eq!(cell_key(&east, 5, 5), 0x204);
        assert_eq!(cell_key(&west, 5, 5), 0x240);
    }

    #[test]
    fn wall_bits_are_set_for_wall_neighbours_only() {
        // A wall with floor to the east and south-west.
        let t = grid(|x, y| u16::from((x, y) == (6, 5) || (x, y) == (4, 6)));
        assert_eq!(cell_key(&t, 5, 5), 0x0FF & !0x04 & !0x80);
    }

    #[test]
    fn secondary_matches_same_kind_only() {
        // Kind 2 at the centre with kind 2 to the north and kind 1 to the south.
        let t = grid(|x, y| match (x, y) {
            (10, 10) | (10, 9) => 2,
            (10, 11) => 1,
            _ => 0,
        });
        assert_eq!(cell_key(&t, 10, 10), 0x100 | 0x10);
        // Kind 3 (both) uses the same block and compares the full kind.
        let both = grid(|x, y| {
            if (x, y) == (3, 3) || (x, y) == (4, 3) {
                3
            } else {
                0
            }
        });
        assert_eq!(cell_key(&both, 3, 3), 0x100 | 0x04);
    }

    #[test]
    fn only_the_low_two_bits_of_terrain_count() {
        // 0x0010 (border flag) is still a wall; 0x0201 and 0x0009 are floor.
        let t = grid(|x, _| match x {
            1 => 0x0010,
            2 => 0x0201,
            3 => 0x0009,
            _ => 0,
        });
        assert_eq!(cell_key(&t, 1, 4) & 0x300, 0x000);
        assert_eq!(cell_key(&t, 2, 4) & 0x300, 0x200);
        assert_eq!(cell_key(&t, 3, 4) & 0x300, 0x200);
        // Columns are uniform: floor to the N, S and the whole east side, and the
        // flagged wall (x = 1) on the west counts as a wall, so W, NW, SW stay clear.
        assert_eq!(cell_key(&t, 2, 4), 0x200 | 0x1F);
    }

    #[test]
    fn floor_at_the_edge_sees_wall_outside() {
        let t = grid(|_, _| 1);
        // Top-left corner: S, SE, E exist and are floor; N, NW, W, NE, SW are outside.
        assert_eq!(cell_key(&t, 0, 0), 0x200 | 0x01 | 0x02 | 0x04);
        assert_eq!(cell_key(&t, 20, 10), 0x2FF);
    }

    #[test]
    fn metatile_comes_from_table_with_variant() {
        let cex = table();
        let t = grid(|_, _| 0);
        let key = 0x0FF_usize;
        for (draw, variant) in [(0u32, 0usize), (1, 1), (2, 2), (3, 0)] {
            let out = autotile(&t, 14, &cex, &[], false, || draw).unwrap();
            assert_eq!(out[100], cex[key * 3 + variant], "draw {draw}");
        }
    }

    #[test]
    fn fixed_room_forces_variant_0_but_still_draws() {
        let cex = table();
        let t = grid(|_, _| 0);
        let mut calls = 0;
        let out = autotile(&t, 14, &cex, &[], true, || {
            calls += 1;
            2
        })
        .unwrap();
        assert_eq!(calls, GRID_W * GRID_H);
        assert_eq!(out[0], cex[0x0FF * 3]);
    }

    #[test]
    fn draws_run_y_outer_x_inner() {
        let cex = table();
        let t = grid(|_, _| 0);
        let mut n = 0u32;
        let out = autotile(&t, 14, &cex, &[], false, || {
            n += 1;
            0
        })
        .unwrap();
        assert_eq!(n as usize, GRID_W * GRID_H);
        assert_eq!(out.len(), GRID_W * GRID_H);
        // Draw i belongs to cell i: make the draw encode its index, check one cell.
        let mut i = 0usize;
        let seen = std::cell::RefCell::new(vec![]);
        let u = grid(|x, y| u16::from(x > 0 && y > 0)); // floor everywhere but row/col 0
        autotile(&u, 14, &cex, &[], false, || {
            seen.borrow_mut().push(i);
            i += 1;
            0
        })
        .unwrap();
        assert_eq!(seen.borrow()[GRID_W + 1], GRID_W + 1);
    }

    #[test]
    fn emap_tilesets_copy_the_fixed_map_and_draw_nothing() {
        let emap: Vec<u8> = (0..24 * 24).map(|i| (i % 200) as u8 + 1).collect();
        let t = grid(|_, _| 0);
        let mut calls = 0;
        let out = autotile(&t, 0x40, &[], &emap, false, || {
            calls += 1;
            0
        })
        .unwrap();
        assert_eq!(calls, 0);
        assert_eq!(out[0], emap[0]);
        assert_eq!(out[3 * GRID_W + 5], emap[3 * 24 + 5]);
        assert_eq!(out[23 * GRID_W + 23], emap[23 * 24 + 23]);
        assert_eq!(out[24], 0); // x = 24 is outside the fixed map
        assert_eq!(out[24 * GRID_W], 0); // y = 24 too
        // A short emap reads as 0 past its end; a short cex is fine for emap tilesets.
        let out = autotile(&t, 0x41, &[], &emap[..10], false, || 0).unwrap();
        assert_eq!(out[10], 0);
    }

    #[test]
    fn rejects_bad_input() {
        let cex = table();
        assert_eq!(
            autotile(&[0; 10], 14, &cex, &[], false, || 0),
            Err(AutotileError::BadGrid)
        );
        assert_eq!(
            autotile(&grid(|_, _| 0), 14, &cex[..100], &[], false, || 0),
            Err(AutotileError::ShortTable)
        );
    }
}
