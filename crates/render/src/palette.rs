//! Palette animation and fades.
//!
//! [`PaletteAnimator`] steps a [`PaletteAnim`] (water and lava cycles) by game ticks and
//! writes its current frame into a BPL bank. [`Fade`] blends colours towards black or
//! white in whole steps, like the DS master brightness, with integer arithmetic only.

use rescue_formats::gfx::Rgba;
use rescue_formats::graphics::Palette;
use rescue_formats::ground::PaletteAnim;

/// Plays a palette animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteAnimator {
    anim: PaletteAnim,
    ticks: u64,
}

impl PaletteAnimator {
    pub fn new(anim: PaletteAnim) -> Self {
        Self { anim, ticks: 0 }
    }

    /// Advances by `ticks` game frames.
    pub fn advance(&mut self, ticks: u32) {
        self.ticks += u64::from(ticks);
    }

    /// Index of the current frame: one frame per `duration` ticks (at least 1), looping.
    pub fn frame_index(&self) -> usize {
        if self.anim.frames.is_empty() {
            return 0;
        }
        let per = self.anim.duration.max(1) as u64;
        ((self.ticks / per) % self.anim.frames.len() as u64) as usize
    }

    /// The current frame's colours (the bank's colours 1 to 15), if there are any frames.
    pub fn current(&self) -> Option<&Palette> {
        self.anim.frames.get(self.frame_index())
    }

    /// Writes the current frame into `bank`, leaving colour 0 (transparent) alone.
    pub fn apply(&self, bank: &mut Palette) {
        if let Some(frame) = self.current() {
            for (dst, src) in bank.iter_mut().skip(1).zip(frame) {
                *dst = *src;
            }
        }
    }
}

/// What a fade moves towards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeTarget {
    Black,
    White,
}

/// A fade of `step` out of `steps` (0 is untouched, `steps` is fully the target).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    pub target: FadeTarget,
    pub step: u32,
    pub steps: u32,
}

impl Fade {
    /// `step` is clamped to `steps`; `steps` of 0 is read as 1.
    pub fn new(target: FadeTarget, step: u32, steps: u32) -> Self {
        let steps = steps.max(1);
        Fade {
            target,
            step: step.min(steps),
            steps,
        }
    }

    /// Fades one colour; alpha is kept.
    pub fn apply(&self, c: Rgba) -> Rgba {
        let goal: i64 = match self.target {
            FadeTarget::Black => 0,
            FadeTarget::White => 255,
        };
        let mix = |v: u8| {
            let v = i64::from(v);
            // Round to nearest; the ends are exact.
            let d = (goal - v) * i64::from(self.step);
            (v + (2 * d + i64::from(self.steps)).div_euclid(2 * i64::from(self.steps))) as u8
        };
        [mix(c[0]), mix(c[1]), mix(c[2]), c[3]]
    }

    /// Fades every colour of `palette`.
    pub fn apply_palette(&self, palette: &mut Palette) {
        for c in palette {
            *c = self.apply(*c);
        }
    }

    /// Fades a whole frame (screen-wide, like master brightness).
    pub fn apply_frame(&self, frame: &mut crate::bg::Frame) {
        for px in frame.pixels.chunks_exact_mut(4) {
            let c = self.apply([px[0], px[1], px[2], px[3]]);
            px.copy_from_slice(&c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(duration: i16, n: u8) -> PaletteAnim {
        PaletteAnim {
            duration,
            frames: (0..n).map(|i| vec![[i, 0, 0, 255]; 15]).collect(),
        }
    }

    #[test]
    fn cycle_advances_every_duration_and_loops() {
        let mut a = PaletteAnimator::new(anim(3, 3));
        let seen: Vec<usize> = (0..12)
            .map(|_| {
                let i = a.frame_index();
                a.advance(1);
                i
            })
            .collect();
        assert_eq!(seen, [0, 0, 0, 1, 1, 1, 2, 2, 2, 0, 0, 0]);
    }

    #[test]
    fn advance_by_many_ticks_and_odd_durations() {
        let mut a = PaletteAnimator::new(anim(2, 4));
        a.advance(7);
        assert_eq!(a.frame_index(), 3);
        a.advance(1);
        assert_eq!(a.frame_index(), 0);
        let mut z = PaletteAnimator::new(anim(0, 2)); // duration 0 reads as 1
        z.advance(1);
        assert_eq!(z.frame_index(), 1);
        let none = PaletteAnimator::new(anim(1, 0));
        assert_eq!(none.frame_index(), 0);
        assert!(none.current().is_none());
    }

    #[test]
    fn apply_replaces_colours_1_to_15_only() {
        let mut a = PaletteAnimator::new(anim(1, 2));
        a.advance(1);
        let mut bank = vec![[9, 9, 9, 0]; 16];
        a.apply(&mut bank);
        assert_eq!(bank[0], [9, 9, 9, 0]);
        assert!(bank[1..].iter().all(|c| *c == [1, 0, 0, 255]));
    }

    #[test]
    fn fade_to_black_steps() {
        let c = [200, 100, 50, 255];
        let f = |s| Fade::new(FadeTarget::Black, s, 4).apply(c);
        assert_eq!(f(0), c);
        assert_eq!(f(1), [150, 75, 38, 255]);
        assert_eq!(f(2), [100, 50, 25, 255]);
        assert_eq!(f(4), [0, 0, 0, 255]);
    }

    #[test]
    fn fade_to_white_steps_and_alpha() {
        let c = [0, 100, 255, 7];
        let f = |s| Fade::new(FadeTarget::White, s, 4).apply(c);
        assert_eq!(f(0), c);
        assert_eq!(f(2), [128, 178, 255, 7]);
        assert_eq!(f(4), [255, 255, 255, 7]);
    }

    #[test]
    fn fade_is_monotonic_and_clamped() {
        let mut last = 255u8;
        for s in 0..=16 {
            let v = Fade::new(FadeTarget::Black, s, 16).apply([255, 255, 255, 255])[0];
            assert!(v <= last);
            last = v;
        }
        assert_eq!(Fade::new(FadeTarget::Black, 99, 4).step, 4);
        assert_eq!(Fade::new(FadeTarget::White, 1, 0).steps, 1);
    }

    #[test]
    fn fade_palette_and_frame() {
        let mut p = vec![[100, 100, 100, 255]; 2];
        Fade::new(FadeTarget::Black, 1, 1).apply_palette(&mut p);
        assert_eq!(p[1], [0, 0, 0, 255]);
        let mut f = crate::bg::Frame::filled([10, 20, 30, 255]);
        Fade::new(FadeTarget::White, 1, 1).apply_frame(&mut f);
        assert_eq!(f.get(5, 5), [255, 255, 255, 255]);
    }
}
