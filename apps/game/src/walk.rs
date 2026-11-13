//! The leader's walk on screen (#347 P2). The sim moves the leader at once, to the new tile;
//! this draws the sprite walking from the old tile's centre to the new one, 24 frames a tile
//! at 1 px a frame on each axis (a diagonal moves both). Inputs are not read until the walk
//! ends. The timing is measured in Blue (#347 P2, investigator a's runtime probe).

use rescue_sim::command::Direction;

/// Frames a tile takes at normal speed: 24 px at 1 px a frame.
// shortcut: every tile takes 24 frames, and a run walks each tile of its straight line at the
// same pace. The speed-stage effect (the queued moves of the game's walk, `02187C90`) and the
// one-tile-a-frame run are not settled (#347 P2), so a fast leader is not shown faster. Upgrade
// when they are measured.
pub const TILE_FRAMES: u8 = 24;

/// A walk of one or more tiles in one direction, drawn from the tile it starts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Walk {
    /// The tile the current leg starts on.
    cell: (i32, i32),
    dir: Direction,
    /// Legs still to walk, the current one included.
    legs: u32,
    /// Frames into the current leg.
    frame: u8,
}

impl Walk {
    /// A walk of `legs` tiles in `dir` from the tile `from`. `None` when there is no leg.
    pub fn new(from: (i32, i32), dir: Direction, legs: u32) -> Option<Walk> {
        (legs > 0).then_some(Walk {
            cell: from,
            dir,
            legs,
            frame: 0,
        })
    }

    /// Where the sprite is drawn: the current leg's start tile, and the offset in pixels from
    /// that tile's centre.
    pub fn pose(&self) -> ((i32, i32), (i32, i32)) {
        let (dx, dy) = self.dir.delta();
        let k = i32::from(self.frame);
        (self.cell, (dx * k, dy * k))
    }

    /// Advances one frame. Returns true when the last leg has ended.
    pub fn tick(&mut self) -> bool {
        self.frame += 1;
        if self.frame < TILE_FRAMES {
            return false;
        }
        self.frame = 0;
        let (dx, dy) = self.dir.delta();
        self.cell = (self.cell.0 + dx, self.cell.1 + dy);
        self.legs -= 1;
        self.legs == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tile_takes_24_frames_at_one_pixel_a_frame() {
        let mut w = Walk::new((20, 25), Direction::Down, 1).expect("one leg");
        assert_eq!(w.pose(), ((20, 25), (0, 0)));
        for k in 1..TILE_FRAMES as i32 {
            assert!(!w.tick(), "still walking at frame {k}");
            assert_eq!(w.pose(), ((20, 25), (0, k)));
        }
        assert!(w.tick(), "the leg ends on frame 24");
        assert_eq!(
            w.pose(),
            ((20, 26), (0, 0)),
            "the sprite is on the new tile"
        );
    }

    #[test]
    fn a_diagonal_moves_both_axes_one_pixel_a_frame() {
        let mut w = Walk::new((3, 3), Direction::UpRight, 1).expect("one leg");
        for _ in 0..10 {
            w.tick();
        }
        assert_eq!(w.pose(), ((3, 3), (10, -10)));
    }

    #[test]
    fn a_run_walks_each_tile_in_turn() {
        let mut w = Walk::new((2, 2), Direction::Left, 3).expect("three legs");
        for _ in 0..TILE_FRAMES {
            assert!(!w.tick());
        }
        assert_eq!(
            w.pose(),
            ((1, 2), (0, 0)),
            "the second leg starts on the first tile"
        );
        for _ in 0..TILE_FRAMES {
            assert!(!w.tick());
        }
        assert_eq!(w.pose(), ((0, 2), (0, 0)));
        for _ in 0..TILE_FRAMES - 1 {
            assert!(!w.tick());
        }
        assert!(w.tick(), "the last leg ends on frame 24");
        assert_eq!(w.pose(), ((-1, 2), (0, 0)));
    }

    #[test]
    fn no_legs_is_no_walk() {
        assert_eq!(Walk::new((0, 0), Direction::Down, 0), None);
    }
}
