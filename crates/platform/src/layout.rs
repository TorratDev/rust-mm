//! The two DS screens. Their placement in the window is `rescue_render::ScreenLayout`.

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
