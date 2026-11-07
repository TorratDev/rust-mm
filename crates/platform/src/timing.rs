//! Fixed-timestep accumulator.
//!
//! The game logic advances in whole ticks of a fixed length, independent of the
//! render rate. [`FixedTimestep::ds`] paces ticks at the DS frame rate, with debug
//! fast-forward and frame-step. The clock knows nothing about the simulation.

use std::time::Duration;

/// Logic tick rate for the hello-world loop.
pub const TICKS_PER_SECOND: u32 = 60;

/// DS frame rate as an exact ratio: 33 513 982 Hz / (355 x 263 x 6) = about 59.8261 Hz.
pub const DS_FRAME_RATE: (u64, u64) = (33_513_982, 355 * 263 * 6);

/// Length of one DS frame, rounded to the nearest nanosecond (error under 0.5 ns per frame).
pub const DS_FRAME_STEP: Duration = Duration::from_nanos(16_715_113);

/// Converts elapsed wall time into a whole number of fixed-length ticks.
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    step: Duration,
    max_steps: u32,
    accumulator: Duration,
    speed: u32,
    paused: bool,
    pending_steps: u32,
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
            speed: 1,
            paused: false,
            pending_steps: 0,
        }
    }

    /// A timestep at the DS frame rate ([`DS_FRAME_STEP`]).
    pub fn ds(max_steps: u32) -> Self {
        Self::new(DS_FRAME_STEP, max_steps)
    }

    /// Fast-forward: runs `speed` times faster than real time (1 is normal; 0 is read as 1).
    pub fn set_speed(&mut self, speed: u32) {
        self.speed = speed.max(1);
    }

    /// Current fast-forward multiplier.
    pub fn speed(&self) -> u32 {
        self.speed
    }

    /// Pauses or resumes. While paused, time is discarded and [`advance`](Self::advance)
    /// returns only ticks requested with [`request_step`](Self::request_step).
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            self.accumulator = Duration::ZERO;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Frame-step: asks for one tick on the next [`advance`](Self::advance) while paused.
    pub fn request_step(&mut self) {
        self.pending_steps = self.pending_steps.saturating_add(1);
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
        if self.paused {
            return std::mem::take(&mut self.pending_steps).min(self.max_steps);
        }
        self.pending_steps = 0;
        self.accumulator += elapsed.saturating_mul(self.speed);
        let cap = self.max_steps.saturating_mul(self.speed);
        let mut steps = 0;
        while self.accumulator >= self.step && steps < cap {
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
        if self.paused {
            return self.step;
        }
        (self.step.saturating_sub(self.accumulator) / self.speed).max(Duration::from_micros(100))
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

    /// Ticks run over `seconds` of simulated display refreshes at `hz`.
    fn run_display(hz: u64, seconds: u64, mut t: FixedTimestep) -> u64 {
        let (mut last, mut ticks) = (0u64, 0u64);
        for i in 1..=hz * seconds {
            let now = i * 1_000_000_000 / hz;
            ticks += u64::from(t.advance(Duration::from_nanos(now - last)));
            last = now;
        }
        ticks
    }

    #[test]
    fn ds_step_matches_the_rate() {
        let exact = DS_FRAME_RATE.1 as f64 / DS_FRAME_RATE.0 as f64;
        assert!((DS_FRAME_STEP.as_secs_f64() - exact).abs() < 1e-9);
        assert_eq!(DS_FRAME_RATE.1, 560_190);
    }

    #[test]
    fn no_drift_at_60_120_144_hz() {
        // One hour of display refreshes; ticks must stay within one of the DS count.
        let seconds = 3600;
        let expected = seconds * 1_000_000_000 / DS_FRAME_STEP.as_nanos() as u64;
        for hz in [60, 120, 144] {
            let ticks = run_display(hz, seconds, FixedTimestep::ds(5));
            assert!(
                ticks.abs_diff(expected) <= 1,
                "{hz} Hz: {ticks} vs {expected}"
            );
        }
    }

    #[test]
    fn fast_forward_scales_ticks() {
        let mut t = FixedTimestep::ds(5);
        t.set_speed(4);
        assert_eq!(t.speed(), 4);
        let base = run_display(60, 60, FixedTimestep::ds(5));
        let fast = run_display(60, 60, t);
        assert!(fast.abs_diff(4 * base) <= 4, "{fast} vs 4 x {base}");
        t = FixedTimestep::ds(5);
        t.set_speed(0);
        assert_eq!(t.speed(), 1);
    }

    #[test]
    fn frame_step_while_paused() {
        let mut t = FixedTimestep::new(10 * MS, 5);
        t.set_paused(true);
        assert_eq!(t.advance(100 * MS), 0);
        t.request_step();
        assert_eq!(t.advance(100 * MS), 1);
        assert_eq!(t.advance(100 * MS), 0);
        t.request_step();
        t.request_step();
        assert_eq!(t.advance(MS), 2);
        // Resuming starts from a clean accumulator; unused step requests are dropped.
        t.request_step();
        t.set_paused(false);
        assert!(!t.is_paused());
        assert_eq!(t.advance(4 * MS), 0);
        assert_eq!(t.advance(6 * MS), 1);
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
