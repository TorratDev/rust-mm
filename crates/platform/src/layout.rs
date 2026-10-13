//! Placement of the two DS screens in the window, scaled by an integer factor.
//!
//! Other window layouts (side by side, single screen) come later (#67).

/// Width of one DS screen in pixels.
pub const SCREEN_WIDTH: u32 = 256;
/// Height of one DS screen in pixels.
pub const SCREEN_HEIGHT: u32 = 192;

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

/// A rectangle in window pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Where each screen is drawn, for one window size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Integer scale factor applied to both screens (at least 1).
    pub scale: u32,
    pub top: PixelRect,
    pub bottom: PixelRect,
}

impl Layout {
    /// Screens stacked vertically (top above bottom), at the largest integer scale
    /// that fits `width` x `height`, centred. Never scales below 1: a window smaller
    /// than 256x384 crops instead of blurring.
    pub fn stacked(width: u32, height: u32) -> Self {
        let scale = (width / SCREEN_WIDTH)
            .min(height / (2 * SCREEN_HEIGHT))
            .max(1);
        let w = SCREEN_WIDTH * scale;
        let h = SCREEN_HEIGHT * scale;
        let x = centre(width, w);
        let y = centre(height, 2 * h);
        Layout {
            scale,
            top: PixelRect { x, y, w, h },
            bottom: PixelRect {
                x,
                y: y + h as i32,
                w,
                h,
            },
        }
    }

    /// Rectangle for `screen`.
    pub fn rect(&self, screen: Screen) -> PixelRect {
        match screen {
            Screen::Top => self.top,
            Screen::Bottom => self.bottom,
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
    fn exact_fit() {
        let l = Layout::stacked(512, 768);
        assert_eq!(l.scale, 2);
        assert_eq!(
            l.top,
            PixelRect {
                x: 0,
                y: 0,
                w: 512,
                h: 384
            }
        );
        assert_eq!(
            l.bottom,
            PixelRect {
                x: 0,
                y: 384,
                w: 512,
                h: 384
            }
        );
    }

    #[test]
    fn picks_largest_integer_scale_and_centres() {
        // 1920x1080: height limits the scale to 2 (2 * 384 = 768 <= 1080 < 1152).
        let l = Layout::stacked(1920, 1080);
        assert_eq!(l.scale, 2);
        assert_eq!(l.top.x, (1920 - 512) / 2);
        assert_eq!(l.top.y, (1080 - 768) / 2);
        assert_eq!(l.bottom.y, l.top.y + 384);
        assert_eq!(l.rect(Screen::Bottom), l.bottom);
    }

    #[test]
    fn width_can_limit_scale() {
        assert_eq!(Layout::stacked(700, 5000).scale, 2);
    }

    #[test]
    fn never_below_one() {
        let l = Layout::stacked(100, 100);
        assert_eq!(l.scale, 1);
        assert_eq!(l.top.w, SCREEN_WIDTH);
        assert!(l.top.x < 0 && l.top.y < 0);
    }
}
