//! Camera: the BG scroll and the sprite origin that follow the leader.
//!
//! A pure function of the leader's position. Dungeon cells are 24x24-pixel metatiles.
//! `[hypothesis]` The original's camera is not traced yet: this centres the leader on the
//! screen and keeps the view inside the map, centring the map on an axis where it is
//! smaller than the screen. P5 (comparison with a capture) will confirm or correct it.

use crate::screens::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Side of a dungeon cell in pixels.
pub const CELL: i32 = 24;

/// Where to scroll the background and where to draw the leader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    /// Scroll for the BG layers (`BgLayer::scroll_x`, `scroll_y`).
    pub scroll: (i32, i32),
    /// Screen position of the leader's sprite origin: the middle of its cell, plus any
    /// sub-cell offset.
    pub sprite_origin: (i32, i32),
}

/// One axis: the scroll that centres `leader` (a map pixel) in a view of `view` pixels,
/// clamped to a map of `map` pixels.
fn axis(leader: i32, view: i32, map: i32) -> i32 {
    if map <= view {
        (map - view) / 2
    } else {
        (leader - view / 2).clamp(0, map - view)
    }
}

/// Camera for a leader in cell `cell` with a sub-cell offset of `sub` pixels (for
/// walking animations), on a map of `map_cells` cells.
pub fn follow(cell: (i32, i32), sub: (i32, i32), map_cells: (i32, i32)) -> Camera {
    let centre = (
        cell.0 * CELL + CELL / 2 + sub.0,
        cell.1 * CELL + CELL / 2 + sub.1,
    );
    let scroll = (
        axis(centre.0, SCREEN_WIDTH as i32, map_cells.0 * CELL),
        axis(centre.1, SCREEN_HEIGHT as i32, map_cells.1 * CELL),
    );
    Camera {
        scroll,
        sprite_origin: (centre.0 - scroll.0, centre.1 - scroll.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centres_the_leader_in_the_middle_of_a_large_map() {
        let c = follow((28, 16), (0, 0), (56, 32));
        assert_eq!(c.sprite_origin, (128, 96));
        // Leader centre (684, 396) minus the screen centre.
        assert_eq!(c.scroll, (684 - 128, 396 - 96));
    }

    #[test]
    fn sub_cell_offsets_move_the_view_with_the_leader() {
        let a = follow((28, 16), (0, 0), (56, 32));
        let b = follow((28, 16), (5, -3), (56, 32));
        assert_eq!(b.scroll, (a.scroll.0 + 5, a.scroll.1 - 3));
        assert_eq!(b.sprite_origin, a.sprite_origin);
    }

    #[test]
    fn clamps_at_the_map_edges_and_the_sprite_moves_instead() {
        let c = follow((0, 0), (0, 0), (56, 32));
        assert_eq!(c.scroll, (0, 0));
        assert_eq!(c.sprite_origin, (12, 12));
        let c = follow((55, 31), (0, 0), (56, 32));
        assert_eq!(c.scroll, (56 * 24 - 256, 32 * 24 - 192));
        assert_eq!(
            c.sprite_origin,
            (55 * 24 + 12 - c.scroll.0, 31 * 24 + 12 - c.scroll.1)
        );
        assert!(c.sprite_origin.0 > 128 && c.sprite_origin.1 > 96);
    }

    #[test]
    fn small_maps_are_centred() {
        // 8 x 4 cells = 192 x 96 pixels: smaller than the screen on both axes.
        let c = follow((3, 1), (0, 0), (8, 4));
        assert_eq!(c.scroll, ((192 - 256) / 2, (96 - 192) / 2));
        assert_eq!(c.sprite_origin, (3 * 24 + 12 + 32, 24 + 12 + 48));
    }

    #[test]
    fn the_sprite_origin_matches_the_map_position_under_the_scroll() {
        for cell in [(0, 0), (10, 5), (30, 20), (55, 31)] {
            let c = follow(cell, (0, 0), (56, 32));
            assert_eq!(c.sprite_origin.0 + c.scroll.0, cell.0 * 24 + 12);
            assert_eq!(c.sprite_origin.1 + c.scroll.1, cell.1 * 24 + 12);
        }
    }
}
