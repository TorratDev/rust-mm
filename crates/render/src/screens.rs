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
}
