//! Placement of the two 256x192 DS screens in a window.
//!
//! The screens are stacked (top above bottom), separated by an optional gap given in
//! DS pixels. Both screens and the gap share one integer scale, the largest that fits
//! the window, never below 1. The block is centred; the unused window area is the
//! letterbox (the caller clears it). A window smaller than the block at scale 1 crops
//! instead of blurring.

/// Width of one DS screen in pixels.
pub const SCREEN_WIDTH: u32 = 256;
/// Height of one DS screen in pixels.
pub const SCREEN_HEIGHT: u32 = 192;

/// A rectangle in window pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Where each screen is drawn, for one window size and gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenLayout {
    /// Integer scale applied to both screens and the gap (at least 1).
    pub scale: u32,
    pub top: Rect,
    pub bottom: Rect,
}

impl ScreenLayout {
    /// Layout for a `width` x `height` window with `gap` DS pixels between the screens.
    pub fn stacked(width: u32, height: u32, gap: u32) -> Self {
        let total = 2 * SCREEN_HEIGHT + gap;
        let scale = (width / SCREEN_WIDTH).min(height / total).max(1);
        let w = SCREEN_WIDTH * scale;
        let h = SCREEN_HEIGHT * scale;
        let x = centre(width, w);
        let y = centre(height, total * scale);
        ScreenLayout {
            scale,
            top: Rect { x, y, w, h },
            bottom: Rect {
                x,
                y: y + (h + gap * scale) as i32,
                w,
                h,
            },
        }
    }
}

/// One of the two DS screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Top,
    Bottom,
}

impl Screen {
    /// Both screens, top first.
    pub const ALL: [Screen; 2] = [Screen::Top, Screen::Bottom];
}

/// How the two screens are arranged in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Arrangement {
    /// Top above bottom, like the DS (the default).
    #[default]
    Vertical,
    /// Top left, bottom right.
    SideBySide,
    /// Only the top screen.
    TopOnly,
    /// Only the top screen, with the bottom screen at half the scale over its bottom-right
    /// corner. The bottom screen is hidden when the top scale is 1 (no integer half).
    TopOverlay,
    /// Top screen large, bottom screen below it at half the scale (at least 1).
    LargeTop,
}

impl Arrangement {
    /// Every arrangement, in cycling order.
    pub const ALL: [Arrangement; 5] = [
        Arrangement::Vertical,
        Arrangement::SideBySide,
        Arrangement::TopOnly,
        Arrangement::TopOverlay,
        Arrangement::LargeTop,
    ];

    /// The next arrangement in [`Arrangement::ALL`], wrapping around.
    pub fn next(self) -> Arrangement {
        let i = Self::ALL.iter().position(|a| *a == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Stable lowercase name, used on the command line and in the settings file.
    pub fn name(self) -> &'static str {
        match self {
            Arrangement::Vertical => "vertical",
            Arrangement::SideBySide => "side",
            Arrangement::TopOnly => "top",
            Arrangement::TopOverlay => "overlay",
            Arrangement::LargeTop => "large",
        }
    }

    /// Inverse of [`Arrangement::name`].
    pub fn from_name(name: &str) -> Option<Arrangement> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// Screen placement for one [`Arrangement`], window size and gap.
///
/// Each screen has its own integer scale (at least 1), so its rectangle is
/// `256 * scale` x `192 * scale`. A screen that is not shown is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrangedLayout {
    pub arrangement: Arrangement,
    pub top: Option<(Rect, u32)>,
    pub bottom: Option<(Rect, u32)>,
}

/// A window point mapped onto a screen, in DS pixels (`x` 0..=255, `y` 0..=191).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPoint {
    pub screen: Screen,
    pub x: u8,
    pub y: u8,
}

fn rect(x: i32, y: i32, scale: u32) -> Rect {
    Rect {
        x,
        y,
        w: SCREEN_WIDTH * scale,
        h: SCREEN_HEIGHT * scale,
    }
}

impl ArrangedLayout {
    /// Places the screens in a `width` x `height` window. `gap` is in DS pixels and is
    /// scaled with the top screen. Never scales below 1: a small window crops.
    pub fn new(arrangement: Arrangement, width: u32, height: u32, gap: u32) -> Self {
        let fit = |w: u32, h: u32| (width / w).min(height / h).max(1);
        match arrangement {
            Arrangement::Vertical => {
                let l = ScreenLayout::stacked(width, height, gap);
                ArrangedLayout {
                    arrangement,
                    top: Some((l.top, l.scale)),
                    bottom: Some((l.bottom, l.scale)),
                }
            }
            Arrangement::SideBySide => {
                let s = fit(2 * SCREEN_WIDTH + gap, SCREEN_HEIGHT);
                let x = centre(width, (2 * SCREEN_WIDTH + gap) * s);
                let y = centre(height, SCREEN_HEIGHT * s);
                ArrangedLayout {
                    arrangement,
                    top: Some((rect(x, y, s), s)),
                    bottom: Some((rect(x + ((SCREEN_WIDTH + gap) * s) as i32, y, s), s)),
                }
            }
            Arrangement::TopOnly | Arrangement::TopOverlay => {
                let s = fit(SCREEN_WIDTH, SCREEN_HEIGHT);
                let x = centre(width, SCREEN_WIDTH * s);
                let y = centre(height, SCREEN_HEIGHT * s);
                let top = rect(x, y, s);
                let bottom = (arrangement == Arrangement::TopOverlay && s >= 2).then(|| {
                    let t = s / 2;
                    let r = rect(0, 0, t);
                    let r = Rect {
                        x: top.x + (top.w - r.w) as i32,
                        y: top.y + (top.h - r.h) as i32,
                        ..r
                    };
                    (r, t)
                });
                ArrangedLayout {
                    arrangement,
                    top: Some((top, s)),
                    bottom,
                }
            }
            Arrangement::LargeTop => {
                // Largest top scale s whose block (top s, bottom max(1, s/2), gap) fits.
                let mut s = 1;
                while {
                    let n = s + 1;
                    SCREEN_WIDTH * n <= width
                        && SCREEN_HEIGHT * (n + (n / 2).max(1)) + gap * n <= height
                } {
                    s += 1;
                }
                let t = (s / 2).max(1);
                let block_h = SCREEN_HEIGHT * (s + t) + gap * s;
                let y = centre(height, block_h);
                let top = rect(centre(width, SCREEN_WIDTH * s), y, s);
                let bottom = rect(
                    centre(width, SCREEN_WIDTH * t),
                    y + (SCREEN_HEIGHT * s + gap * s) as i32,
                    t,
                );
                ArrangedLayout {
                    arrangement,
                    top: Some((top, s)),
                    bottom: Some((bottom, t)),
                }
            }
        }
    }

    /// Rectangle of `screen`, if it is shown.
    pub fn rect(&self, screen: Screen) -> Option<Rect> {
        match screen {
            Screen::Top => self.top,
            Screen::Bottom => self.bottom,
        }
        .map(|(r, _)| r)
    }

    /// Maps a window point to the screen it falls on and the DS pixel under it. The bottom
    /// screen is tested first, so an overlay wins over the top screen below it. `None` when
    /// the point is outside both screens (letterbox or gap).
    pub fn touch(&self, x: i32, y: i32) -> Option<ScreenPoint> {
        [(Screen::Bottom, self.bottom), (Screen::Top, self.top)]
            .into_iter()
            .find_map(|(screen, placed)| {
                let (r, scale) = placed?;
                let dx = i64::from(x) - i64::from(r.x);
                let dy = i64::from(y) - i64::from(r.y);
                if dx < 0 || dy < 0 || dx >= i64::from(r.w) || dy >= i64::from(r.h) {
                    return None;
                }
                Some(ScreenPoint {
                    screen,
                    x: (dx / i64::from(scale)) as u8,
                    y: (dy / i64::from(scale)) as u8,
                })
            })
    }
}

/// Offset that centres `inner` within `outer`; negative if `inner` is larger.
fn centre(outer: u32, inner: u32) -> i32 {
    ((i64::from(outer) - i64::from(inner)) / 2) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_fit_no_gap() {
        let l = ScreenLayout::stacked(512, 768, 0);
        assert_eq!(l.scale, 2);
        assert_eq!(
            l.top,
            Rect {
                x: 0,
                y: 0,
                w: 512,
                h: 384
            }
        );
        assert_eq!(
            l.bottom,
            Rect {
                x: 0,
                y: 384,
                w: 512,
                h: 384
            }
        );
    }

    #[test]
    fn largest_integer_scale_is_centred() {
        // 1920x1080: 2 * 384 = 768 <= 1080 < 1152, so scale 2.
        let l = ScreenLayout::stacked(1920, 1080, 0);
        assert_eq!(l.scale, 2);
        assert_eq!(l.top.x, (1920 - 512) / 2);
        assert_eq!(l.top.y, (1080 - 768) / 2);
        assert_eq!(l.bottom.y, l.top.y + 384);
    }

    #[test]
    fn width_can_limit_scale() {
        assert_eq!(ScreenLayout::stacked(700, 5000, 0).scale, 2);
        assert_eq!(ScreenLayout::stacked(767, 5000, 0).scale, 2);
        assert_eq!(ScreenLayout::stacked(768, 5000, 0).scale, 3);
    }

    #[test]
    fn never_below_one_and_crops() {
        let l = ScreenLayout::stacked(100, 100, 0);
        assert_eq!(l.scale, 1);
        assert_eq!((l.top.w, l.top.h), (SCREEN_WIDTH, SCREEN_HEIGHT));
        assert!(l.top.x < 0 && l.top.y < 0);
    }

    #[test]
    fn gap_is_scaled_and_separates_screens() {
        // 512x800, gap 8: total 392 DS rows -> scale 2, gap 16 px.
        let l = ScreenLayout::stacked(512, 800, 8);
        assert_eq!(l.scale, 2);
        assert_eq!(l.top.y, (800 - 784) / 2);
        assert_eq!(l.bottom.y - (l.top.y + l.top.h as i32), 16);
    }

    #[test]
    fn gap_can_lower_the_scale() {
        // 512x768 fits scale 2 with no gap, but a gap makes the block 784 px tall.
        assert_eq!(ScreenLayout::stacked(512, 768, 0).scale, 2);
        let l = ScreenLayout::stacked(512, 768, 8);
        assert_eq!(l.scale, 1);
        assert_eq!(l.bottom.y - (l.top.y + l.top.h as i32), 8);
    }

    #[test]
    fn letterbox_is_symmetric() {
        let l = ScreenLayout::stacked(1000, 900, 4);
        let s = l.scale as i32;
        assert_eq!(l.top.x, (1000 - 256 * s) / 2);
        let block_h = (2 * 192 + 4) * s;
        assert_eq!(l.top.y, (900 - block_h) / 2);
        assert_eq!(l.bottom.y + l.bottom.h as i32, l.top.y + block_h);
    }

    fn within(r: Rect, w: u32, h: u32) -> bool {
        r.x >= 0 && r.y >= 0 && r.x as u32 + r.w <= w && r.y as u32 + r.h <= h
    }

    fn overlap(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.w as i32
            && b.x < a.x + a.w as i32
            && a.y < b.y + b.h as i32
            && b.y < a.y + a.h as i32
    }

    #[test]
    fn vertical_matches_stacked() {
        let a = ArrangedLayout::new(Arrangement::Vertical, 1000, 900, 4);
        let l = ScreenLayout::stacked(1000, 900, 4);
        assert_eq!(a.top, Some((l.top, l.scale)));
        assert_eq!(a.bottom, Some((l.bottom, l.scale)));
    }

    #[test]
    fn side_by_side_exact_fit_and_gap() {
        let a = ArrangedLayout::new(Arrangement::SideBySide, 1024, 384, 0);
        assert_eq!(
            a.top,
            Some((
                Rect {
                    x: 0,
                    y: 0,
                    w: 512,
                    h: 384
                },
                2
            ))
        );
        assert_eq!(
            a.bottom,
            Some((
                Rect {
                    x: 512,
                    y: 0,
                    w: 512,
                    h: 384
                },
                2
            ))
        );
        // Gap 8 makes the block 520 DS wide: scale 1 in a 1024 window, 8 px gap.
        let g = ArrangedLayout::new(Arrangement::SideBySide, 1024, 384, 8);
        let (t, s) = g.top.unwrap();
        let (b, _) = g.bottom.unwrap();
        assert_eq!(s, 1);
        assert_eq!(b.x - (t.x + t.w as i32), 8);
        assert_eq!(t.x, (1024 - 520) / 2);
    }

    #[test]
    fn top_only_has_no_bottom_and_scales_to_fit() {
        let a = ArrangedLayout::new(Arrangement::TopOnly, 1000, 800, 0);
        assert_eq!(a.bottom, None);
        // 1000/256 = 3, 800/192 = 4 -> 3.
        let (r, s) = a.top.unwrap();
        assert_eq!(s, 3);
        assert_eq!((r.w, r.h), (768, 576));
        assert_eq!((r.x, r.y), ((1000 - 768) / 2, (800 - 576) / 2));
    }

    #[test]
    fn overlay_sits_in_the_top_screens_corner_at_half_scale() {
        let a = ArrangedLayout::new(Arrangement::TopOverlay, 1024, 768, 0);
        let (t, s) = a.top.unwrap();
        let (b, bs) = a.bottom.unwrap();
        assert_eq!((s, bs), (4, 2));
        assert_eq!(b.x + b.w as i32, t.x + t.w as i32);
        assert_eq!(b.y + b.h as i32, t.y + t.h as i32);
        // No integer half of scale 1: the overlay is hidden.
        assert_eq!(
            ArrangedLayout::new(Arrangement::TopOverlay, 300, 300, 0).bottom,
            None
        );
    }

    #[test]
    fn large_top_stacks_a_smaller_bottom() {
        // Scale 2 top (384 high) + scale 1 bottom (192) = 576; scale 3 needs 576+288 > 600.
        let a = ArrangedLayout::new(Arrangement::LargeTop, 800, 600, 0);
        let (t, s) = a.top.unwrap();
        let (b, bs) = a.bottom.unwrap();
        assert_eq!((s, bs), (2, 1));
        assert_eq!(b.y, t.y + t.h as i32);
        assert_eq!(b.x, (800 - 256) / 2);
        assert!(within(t, 800, 600) && within(b, 800, 600));
        // Scale 4 top + scale 2 bottom = 4*192 + 2*192 = 1152.
        let big = ArrangedLayout::new(Arrangement::LargeTop, 1024, 1152, 0);
        assert_eq!((big.top.unwrap().1, big.bottom.unwrap().1), (4, 2));
    }

    #[test]
    fn every_arrangement_fits_integer_scaled_and_does_not_overlap() {
        for arr in Arrangement::ALL {
            for (w, h) in [(520, 768), (1280, 720), (1920, 1080), (900, 1500)] {
                let a = ArrangedLayout::new(arr, w, h, 4);
                for (r, s) in [a.top, a.bottom].into_iter().flatten() {
                    assert_eq!((r.w, r.h), (SCREEN_WIDTH * s, SCREEN_HEIGHT * s), "{arr:?}");
                    assert!(within(r, w, h), "{arr:?} {w}x{h}: {r:?}");
                }
                if let (Some((t, _)), Some((b, _))) = (a.top, a.bottom)
                    && arr != Arrangement::TopOverlay
                {
                    assert!(!overlap(t, b), "{arr:?} {w}x{h}");
                }
            }
        }
    }

    #[test]
    fn small_window_crops_at_scale_one() {
        for arr in Arrangement::ALL {
            let a = ArrangedLayout::new(arr, 100, 100, 0);
            assert_eq!(a.top.unwrap().1, 1, "{arr:?}");
        }
    }

    #[test]
    fn touch_maps_each_arrangement() {
        for arr in Arrangement::ALL {
            let a = ArrangedLayout::new(arr, 1600, 1000, 8);
            for screen in Screen::ALL {
                let Some(r) = a.rect(screen) else { continue };
                let s = (r.w / SCREEN_WIDTH) as i32;
                // Corners and a middle pixel map to the matching DS pixel.
                let hit = |dx: i32, dy: i32| a.touch(r.x + dx, r.y + dy);
                let expect = |x: u8, y: u8| Some(ScreenPoint { screen, x, y });
                // Overlay corners of the top screen are covered by the bottom screen.
                if !(arr == Arrangement::TopOverlay && screen == Screen::Top) {
                    assert_eq!(hit(0, 0), expect(0, 0), "{arr:?} {screen:?}");
                    assert_eq!(hit(r.w as i32 - 1, r.h as i32 - 1), expect(255, 191));
                    assert_eq!(hit(s * 10 + s - 1, s * 20), expect(10, 20));
                }
                assert_ne!(hit(-1, 0).map(|p| p.screen), Some(screen));
            }
        }
    }

    #[test]
    fn touch_overlay_wins_and_gap_is_a_miss() {
        let a = ArrangedLayout::new(Arrangement::TopOverlay, 1024, 768, 0);
        let (b, _) = a.bottom.unwrap();
        let p = a.touch(b.x, b.y).unwrap();
        assert_eq!((p.screen, p.x, p.y), (Screen::Bottom, 0, 0));
        let (t, _) = a.top.unwrap();
        assert_eq!(a.touch(t.x, t.y).unwrap().screen, Screen::Top);

        let v = ArrangedLayout::new(Arrangement::Vertical, 256, 400, 8);
        let (t, _) = v.top.unwrap();
        let (b, _) = v.bottom.unwrap();
        let mid_gap = t.y + t.h as i32 + 4;
        assert_eq!(v.touch(10, mid_gap), None);
        assert_eq!(v.touch(10, b.y).unwrap().screen, Screen::Bottom);
        assert_eq!(v.touch(-5, 10), None);
    }

    #[test]
    fn arrangement_names_round_trip_and_cycle() {
        for arr in Arrangement::ALL {
            assert_eq!(Arrangement::from_name(arr.name()), Some(arr));
        }
        assert_eq!(Arrangement::from_name("bogus"), None);
        let mut a = Arrangement::Vertical;
        for _ in 0..Arrangement::ALL.len() {
            a = a.next();
        }
        assert_eq!(a, Arrangement::Vertical);
    }
}
