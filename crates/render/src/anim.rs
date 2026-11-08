//! Sprite animation player.
//!
//! Plays one direction of one animation group of a decoded sprite
//! (`rescue_formats::sprite::AnimStep` tables). Time is in presentation ticks, one per
//! DS frame at the clock of `rescue_platform::timing` (#64): a step's `duration` is in
//! those frames. The player never touches the simulation, which stays turn-driven.

use rescue_formats::sprite::{AnimStep, DIRECTIONS, Sprite};

/// A running animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimPlayer {
    steps: Vec<AnimStep>,
    looping: bool,
    /// Index of the current step.
    index: usize,
    /// Ticks spent in the current step.
    elapsed: u32,
    finished: bool,
}

impl AnimPlayer {
    /// Starts `group`, `direction` of `sprite`. `None` if either does not exist or the
    /// sequence is empty.
    pub fn new(sprite: &Sprite, group: usize, direction: usize, looping: bool) -> Option<Self> {
        if direction >= DIRECTIONS {
            return None;
        }
        let steps = sprite.groups.get(group)?[direction].clone();
        Self::from_steps(steps, looping)
    }

    /// A player over explicit steps. `None` if there are none.
    pub fn from_steps(steps: Vec<AnimStep>, looping: bool) -> Option<Self> {
        if steps.is_empty() {
            return None;
        }
        Some(Self {
            steps,
            looping,
            index: 0,
            elapsed: 0,
            finished: false,
        })
    }

    /// The current step.
    pub fn step(&self) -> &AnimStep {
        &self.steps[self.index]
    }

    /// Index of the current step.
    pub fn step_index(&self) -> usize {
        self.index
    }

    /// Frame (picture) index of the current step.
    pub fn frame(&self) -> usize {
        usize::from(self.step().frame)
    }

    /// Whether a non-looping animation has played its last step to the end. It then
    /// keeps showing that step.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Total length of one pass, in ticks (a zero duration counts as one).
    pub fn length(&self) -> u32 {
        self.steps
            .iter()
            .map(|s| u32::from(s.duration.max(1)))
            .sum()
    }

    /// Restarts from the first step.
    pub fn restart(&mut self) {
        self.index = 0;
        self.elapsed = 0;
        self.finished = false;
    }

    /// Advances by `ticks` presentation ticks.
    pub fn advance(&mut self, ticks: u32) {
        if self.finished {
            return;
        }
        let mut ticks = u64::from(ticks);
        if self.looping {
            // Whole passes change nothing; skip them.
            ticks %= u64::from(self.length().max(1));
        }
        while ticks > 0 {
            // Steps have a non-zero duration (a zero ends the table when decoded);
            // treat a zero as one tick so a hand-built table cannot stall.
            let left = u64::from(self.steps[self.index].duration.max(1)) - u64::from(self.elapsed);
            if ticks < left {
                self.elapsed += ticks as u32;
                return;
            }
            ticks -= left;
            self.elapsed = 0;
            if self.index + 1 < self.steps.len() {
                self.index += 1;
            } else if self.looping {
                self.index = 0;
            } else {
                self.finished = true;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(duration: u8, frame: u16) -> AnimStep {
        AnimStep {
            duration,
            flags: 0,
            frame,
            offset: (0, 0),
            shadow: (0, 0),
        }
    }

    fn player(looping: bool) -> AnimPlayer {
        AnimPlayer::from_steps(vec![step(3, 10), step(1, 11), step(2, 12)], looping).unwrap()
    }

    fn frames(p: &mut AnimPlayer, n: usize) -> Vec<usize> {
        (0..n)
            .map(|_| {
                let f = p.frame();
                p.advance(1);
                f
            })
            .collect()
    }

    #[test]
    fn steps_last_their_duration_and_loop() {
        let mut p = player(true);
        assert_eq!(
            frames(&mut p, 13),
            [10, 10, 10, 11, 12, 12, 10, 10, 10, 11, 12, 12, 10]
        );
        assert_eq!(p.length(), 6);
        assert!(!p.finished());
    }

    #[test]
    fn one_shot_stops_on_the_last_step() {
        let mut p = player(false);
        p.advance(5);
        assert_eq!((p.frame(), p.finished()), (12, false));
        p.advance(1);
        assert_eq!((p.frame(), p.finished()), (12, true));
        p.advance(100);
        assert_eq!((p.frame(), p.finished()), (12, true));
        p.restart();
        assert_eq!((p.frame(), p.finished()), (10, false));
    }

    #[test]
    fn big_advances_equal_many_small_ones() {
        for looping in [true, false] {
            for n in [0u32, 1, 2, 5, 6, 7, 13, 1000, u32::MAX] {
                let (mut a, mut b) = (player(looping), player(looping));
                a.advance(n);
                for _ in 0..n.min(2000) {
                    b.advance(1);
                }
                if n <= 2000 {
                    assert_eq!(a, b, "{looping} {n}");
                }
            }
        }
        // Looping: u32::MAX ticks is (u32::MAX mod 6) ticks into the pass.
        let mut a = player(true);
        a.advance(u32::MAX);
        let mut b = player(true);
        b.advance(u32::MAX % 6);
        assert_eq!(a, b);
    }

    #[test]
    fn advance_splits_across_calls() {
        let mut p = player(true);
        p.advance(2);
        p.advance(1);
        assert_eq!((p.step_index(), p.frame()), (1, 11));
        p.advance(1);
        assert_eq!((p.step_index(), p.frame()), (2, 12));
    }

    #[test]
    fn zero_duration_does_not_stall() {
        let mut p = AnimPlayer::from_steps(vec![step(0, 1), step(0, 2)], true).unwrap();
        p.advance(1);
        assert_eq!(p.frame(), 2);
    }

    #[test]
    fn selects_group_and_direction() {
        let dirs = |base: u16| -> [Vec<AnimStep>; DIRECTIONS] {
            std::array::from_fn(|d| vec![step(1, base + d as u16)])
        };
        let sprite = Sprite {
            frames: vec![],
            offsets: vec![],
            images: vec![],
            groups: vec![dirs(0), dirs(100)],
            tiles: vec![],
        };
        assert_eq!(AnimPlayer::new(&sprite, 1, 3, true).unwrap().frame(), 103);
        assert!(AnimPlayer::new(&sprite, 2, 0, true).is_none());
        assert!(AnimPlayer::new(&sprite, 0, 8, true).is_none());
        assert!(AnimPlayer::from_steps(vec![], true).is_none());
    }
}
