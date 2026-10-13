//! Fixed-timestep accumulator.
//!
//! The game logic advances in whole ticks of a fixed length, independent of the
//! render rate. Precise DS frame pacing comes later (frame timing, #64).

use std::time::Duration;

/// Logic tick rate for the hello-world loop.
pub const TICKS_PER_SECOND: u32 = 60;

/// Converts elapsed wall time into a whole number of fixed-length ticks.
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    step: Duration,
    max_steps: u32,
    accumulator: Duration,
}

impl FixedTimestep {
    /// A timestep of `step` that runs at most `max_steps` ticks per call to
    /// [`advance`](Self::advance); time beyond that is dropped, so a long stall
    /// (debugger, window drag) does not cause a burst of catch-up ticks.
    ///
    /// # Panics
    /// If `step` is zero or `max_steps` is zero.
    pub fn new(step: Duration, max_steps: u32) -> Self {
        assert!(!step.is_zero(), "timestep must be non-zero");
        assert!(max_steps > 0, "max_steps must be non-zero");
        Self {
            step,
            max_steps,
            accumulator: Duration::ZERO,
        }
    }

    /// A timestep of `1 / ticks_per_second` seconds.
    pub fn from_rate(ticks_per_second: u32, max_steps: u32) -> Self {
        Self::new(Duration::from_secs(1) / ticks_per_second.max(1), max_steps)
    }

    /// Length of one tick.
    pub fn step(&self) -> Duration {
        self.step
    }

    /// Adds `elapsed` wall time and returns how many ticks to run now.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accumulator += elapsed;
        let mut steps = 0;
        while self.accumulator >= self.step && steps < self.max_steps {
            self.accumulator -= self.step;
            steps += 1;
        }
        if self.accumulator >= self.step {
            // Over the cap: drop the backlog, keeping only the sub-tick remainder.
            let rem = self.accumulator.as_nanos() % self.step.as_nanos();
            self.accumulator = Duration::from_nanos(rem as u64);
        }
        steps
    }

    /// Wall time left until the next tick is due.
    pub fn time_until_next(&self) -> Duration {
        self.step.saturating_sub(self.accumulator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn accumulates_partial_ticks() {
        let mut t = FixedTimestep::new(10 * MS, 5);
        assert_eq!(t.advance(4 * MS), 0);
        assert_eq!(t.time_until_next(), 6 * MS);
        assert_eq!(t.advance(4 * MS), 0);
        assert_eq!(t.advance(4 * MS), 1);
        assert_eq!(t.time_until_next(), 8 * MS);
    }

    #[test]
    fn runs_several_ticks_for_long_frames() {
        let mut t = FixedTimestep::new(10 * MS, 5);
        assert_eq!(t.advance(35 * MS), 3);
        assert_eq!(t.time_until_next(), 5 * MS);
    }

    #[test]
    fn caps_catch_up_and_drops_backlog() {
        let mut t = FixedTimestep::new(10 * MS, 5);
        assert_eq!(t.advance(1003 * MS), 5);
        // Only the 3 ms remainder survives.
        assert_eq!(t.time_until_next(), 7 * MS);
        assert_eq!(t.advance(Duration::ZERO), 0);
    }

    #[test]
    fn exact_multiple_at_cap_keeps_no_backlog() {
        let mut t = FixedTimestep::new(10 * MS, 2);
        assert_eq!(t.advance(20 * MS), 2);
        assert_eq!(t.advance(Duration::ZERO), 0);
        assert_eq!(t.time_until_next(), 10 * MS);
    }

    #[test]
    fn rate_constructor() {
        let t = FixedTimestep::from_rate(TICKS_PER_SECOND, 4);
        assert_eq!(t.step(), Duration::from_nanos(16_666_666));
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn zero_step_panics() {
        let _ = FixedTimestep::new(Duration::ZERO, 1);
    }
}
