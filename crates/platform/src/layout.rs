//! The two DS screens; their placement is `rescue_render::screens`.

use rescue_render::screens::{ArrangedLayout, Arrangement};

pub use rescue_render::screens::Screen;

/// DS pixel under a window point on the bottom screen: `x` in 0..=255, `y` in 0..=191.
///
/// `width` and `height` are the window's drawable size and `gap` the DS rows between the
/// screens, the same values the loop draws with. `None` for points on the top screen, in the
/// gap, or in the letterbox.
pub fn bottom_touch(
    arrangement: Arrangement,
    width: u32,
    height: u32,
    gap: u32,
    x: i32,
    y: i32,
) -> Option<(u8, u8)> {
    let hit = ArrangedLayout::new(arrangement, width, height, gap).touch(x, y)?;
    (hit.screen == Screen::Bottom).then_some((hit.x, hit.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_render::screens::Rect;

    /// Independent reference: the bottom screen's rectangle and scale, from the layout.
    fn expected(arr: Arrangement, w: u32, h: u32, gap: u32, x: i32, y: i32) -> Option<(u8, u8)> {
        let (r, s): (Rect, u32) = ArrangedLayout::new(arr, w, h, gap).bottom?;
        let (dx, dy) = (x - r.x, y - r.y);
        let inside = dx >= 0 && dy >= 0 && dx < r.w as i32 && dy < r.h as i32;
        inside.then(|| ((dx as u32 / s) as u8, (dy as u32 / s) as u8))
    }

    #[test]
    fn every_pixel_of_a_window_matches_the_reference() {
        for arr in Arrangement::ALL {
            for gap in [0, 4, 9] {
                for (w, h) in [(260, 400), (300, 420), (96, 120)] {
                    for y in -1..=h as i32 {
                        for x in -1..=w as i32 {
                            assert_eq!(
                                bottom_touch(arr, w, h, gap, x, y),
                                expected(arr, w, h, gap, x, y),
                                "{arr:?} gap {gap} {w}x{h} at ({x}, {y})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn bottom_corners_and_scale_steps_map_to_ds_pixels() {
        for arr in [
            Arrangement::Vertical,
            Arrangement::SideBySide,
            Arrangement::TopOverlay,
            Arrangement::LargeTop,
        ] {
            let (w, h, gap) = (1920, 1200, 8);
            let l = ArrangedLayout::new(arr, w, h, gap);
            let (r, s) = l.bottom.unwrap();
            let (x0, y0) = (r.x, r.y);
            let s = s as i32;
            assert_eq!(
                bottom_touch(arr, w, h, gap, x0, y0),
                Some((0, 0)),
                "{arr:?}"
            );
            assert_eq!(
                bottom_touch(arr, w, h, gap, x0 + r.w as i32 - 1, y0 + r.h as i32 - 1),
                Some((255, 191)),
                "{arr:?}"
            );
            assert_eq!(
                bottom_touch(arr, w, h, gap, x0 + 10 * s + s - 1, y0 + 20 * s),
                Some((10, 20)),
                "{arr:?}"
            );
        }
    }

    #[test]
    fn gap_and_top_screen_presses_are_ignored() {
        let (w, h, gap) = (512, 800, 8);
        let l = ArrangedLayout::new(Arrangement::Vertical, w, h, gap);
        let (top, _) = l.top.unwrap();
        let (bottom, _) = l.bottom.unwrap();
        let gap_row = top.y + top.h as i32 + 3;
        assert!(gap_row < bottom.y);
        assert_eq!(
            bottom_touch(Arrangement::Vertical, w, h, gap, 100, gap_row),
            None
        );
        assert_eq!(
            bottom_touch(Arrangement::Vertical, w, h, gap, 100, top.y + 1),
            None
        );
        assert_eq!(
            bottom_touch(Arrangement::Vertical, w, h, gap, 100, bottom.y + 1),
            // Scale 2, bottom screen at x 0: DS (50, 0).
            Some((50, 0))
        );
    }

    #[test]
    fn top_only_has_no_bottom_touch() {
        for (x, y) in [(0, 0), (100, 100), (400, 300)] {
            assert_eq!(bottom_touch(Arrangement::TopOnly, 800, 600, 0, x, y), None);
        }
    }
}
