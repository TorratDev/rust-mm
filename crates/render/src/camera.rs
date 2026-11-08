//! Camera: the BG scroll and the sprite origin that follow the leader.
//!
//! A pure function of the leader's position. Dungeon cells are 24x24-pixel metatiles. The
//! rule is measured from two captured dungeon frames (docs/formats/sprites.md): an
//! entity's pixel position is its cell times 24 plus (12, 16) plus any sub-cell offset;
//! the camera is the leader's pixel position minus (128, 108), with no clamping at the map
//! edges (the floor layer wraps); an entity's screen position is its pixel position minus
//! the camera. So the leader is always drawn at (128, 108).

/// Side of a dungeon cell in pixels.
pub const CELL: i32 = 24;

/// Screen position of the leader's entity origin. Measured: it is (128, 108) in both
/// captures.
pub const LEADER_SCREEN: (i32, i32) = (128, 108);

/// Offset of an entity's pixel position inside its cell: x is the middle, y is 4 pixels
/// below it.
pub const CELL_ANCHOR: (i32, i32) = (12, 16);

/// Where to scroll the background and where to draw the leader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    /// The camera in map pixels: the scroll of the floor layer (`BgLayer::scroll_x`,
    /// `scroll_y`; the 512x256 floor map wraps).
    pub scroll: (i32, i32),
    /// Screen position of the leader's entity origin: always [`LEADER_SCREEN`].
    pub sprite_origin: (i32, i32),
}

impl Camera {
    /// Screen position of an entity at map pixel position `pixel`.
    pub fn to_screen(&self, pixel: (i32, i32)) -> (i32, i32) {
        (pixel.0 - self.scroll.0, pixel.1 - self.scroll.1)
    }
}

/// Map pixel position of an entity in cell `cell`, `sub` pixels off the cell anchor (for
/// walking animations).
pub fn entity_pixel(cell: (i32, i32), sub: (i32, i32)) -> (i32, i32) {
    (
        cell.0 * CELL + CELL_ANCHOR.0 + sub.0,
        cell.1 * CELL + CELL_ANCHOR.1 + sub.1,
    )
}

/// Camera for a leader in cell `cell` with a sub-cell offset of `sub` pixels.
pub fn follow(cell: (i32, i32), sub: (i32, i32)) -> Camera {
    let p = entity_pixel(cell, sub);
    Camera {
        scroll: (p.0 - LEADER_SCREEN.0, p.1 - LEADER_SCREEN.1),
        sprite_origin: LEADER_SCREEN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_captured_cameras() {
        // dng1: leader in cell (4, 23), camera (-20, 460); b2: cell (5, 21), camera (4, 412).
        let c = follow((4, 23), (0, 0));
        assert_eq!(c.scroll, (-20, 460));
        assert_eq!(c.sprite_origin, (128, 108));
        assert_eq!(follow((5, 21), (0, 0)).scroll, (4, 412));
    }

    #[test]
    fn other_entities_are_placed_by_their_pixel_position() {
        let c = follow((4, 23), (0, 0));
        // Captured: partner in cell (3, 23) at (104, 108), wild in (5, 23) at (152, 108).
        assert_eq!(c.to_screen(entity_pixel((3, 23), (0, 0))), (104, 108));
        assert_eq!(c.to_screen(entity_pixel((5, 23), (0, 0))), (152, 108));
        // Off screen: left of the view.
        assert!(c.to_screen(entity_pixel((12, 21), (0, 0))).0 > 256);
    }

    #[test]
    fn sub_cell_offsets_move_the_view_with_the_leader() {
        let a = follow((28, 16), (0, 0));
        let b = follow((28, 16), (5, -3));
        assert_eq!(b.scroll, (a.scroll.0 + 5, a.scroll.1 - 3));
        assert_eq!(b.sprite_origin, a.sprite_origin);
    }

    #[test]
    fn the_leader_stays_put_and_there_is_no_clamp_at_the_map_edges() {
        for cell in [(0, 0), (10, 5), (55, 31)] {
            let c = follow(cell, (0, 0));
            assert_eq!(c.to_screen(entity_pixel(cell, (0, 0))), LEADER_SCREEN);
        }
        assert!(follow((0, 0), (0, 0)).scroll.0 < 0);
    }
}
