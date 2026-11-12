//! Scene graph for the boot flow: splash, title, main menu, and the placeholder
//! scenes behind the menu entries. Pure logic: one [`Scene::step`] per logic tick.
//!
//! Order (from the title loop in `docs/re/execution-model.md`, "Start-up and the title
//! menu"): splash, then title, then the menu. The menu entries and the placeholder
//! scenes are ours; the original choice numbers are not mapped yet.

use rescue_platform::{ButtonState, DsButton};

/// Ticks the splash stays up without a press (about 1.5 s at the DS rate).
pub const SPLASH_TICKS: u32 = 90;

/// Main menu entries, in on-screen order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    NewGame,
    Continue,
    Options,
}

impl MenuItem {
    pub const ALL: [MenuItem; 3] = [MenuItem::NewGame, MenuItem::Continue, MenuItem::Options];

    /// Continue needs a save, and saving is not implemented yet (#77), so it is shown
    /// greyed out and the cursor skips it.
    // shortcut: Continue is always disabled, upgrade when the save crate reads a save (#77)
    pub fn enabled(self) -> bool {
        self != MenuItem::Continue
    }
}

/// The screen the game is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The ROM check failed. Nothing else can run; a press quits (see `app`).
    Refused,
    Splash {
        ticks: u32,
    },
    Title,
    /// `cursor` indexes [`MenuItem::ALL`].
    Menu {
        cursor: usize,
    },
    /// Placeholder until the options screen exists.
    Options,
    /// Placeholder until the first dungeon (slice 2, #70).
    NewGame,
}

impl Scene {
    /// Advances one logic tick. `pressed` holds the buttons that went down this tick,
    /// not the ones held.
    pub fn step(self, pressed: ButtonState) -> Scene {
        let confirm = pressed.is_held(DsButton::A) || pressed.is_held(DsButton::Start);
        let back = pressed.is_held(DsButton::B);
        match self {
            Scene::Refused => Scene::Refused,
            Scene::Splash { ticks } => {
                if confirm || ticks + 1 >= SPLASH_TICKS {
                    Scene::Title
                } else {
                    Scene::Splash { ticks: ticks + 1 }
                }
            }
            Scene::Title => {
                if confirm {
                    Scene::Menu { cursor: 0 }
                } else {
                    self
                }
            }
            Scene::Menu { cursor } => {
                if pressed.is_held(DsButton::Up) {
                    Scene::Menu {
                        cursor: move_cursor(cursor, true),
                    }
                } else if pressed.is_held(DsButton::Down) {
                    Scene::Menu {
                        cursor: move_cursor(cursor, false),
                    }
                } else if confirm {
                    match MenuItem::ALL[cursor] {
                        MenuItem::NewGame => Scene::NewGame,
                        MenuItem::Options => Scene::Options,
                        MenuItem::Continue => self,
                    }
                } else {
                    self
                }
            }
            Scene::Options if back => Scene::Menu { cursor: 2 },
            Scene::NewGame if back => Scene::Menu { cursor: 0 },
            _ => self,
        }
    }
}

/// The next enabled menu entry above or below `cursor`, wrapping around.
fn move_cursor(cursor: usize, up: bool) -> usize {
    let n = MenuItem::ALL.len();
    let mut c = cursor;
    loop {
        c = if up { (c + n - 1) % n } else { (c + 1) % n };
        if c == cursor || MenuItem::ALL[c].enabled() {
            return c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(b: DsButton) -> ButtonState {
        let mut s = ButtonState::default();
        s.set(b, true);
        s
    }

    #[test]
    fn splash_ends_after_its_ticks_or_on_a_press() {
        let mut s = Scene::Splash { ticks: 0 };
        for _ in 0..SPLASH_TICKS - 1 {
            s = s.step(ButtonState::default());
            assert!(matches!(s, Scene::Splash { .. }));
        }
        assert_eq!(s.step(ButtonState::default()), Scene::Title);
        assert_eq!(
            Scene::Splash { ticks: 3 }.step(press(DsButton::Start)),
            Scene::Title
        );
    }

    #[test]
    fn title_waits_for_a_press() {
        assert_eq!(Scene::Title.step(ButtonState::default()), Scene::Title);
        assert_eq!(
            Scene::Title.step(press(DsButton::A)),
            Scene::Menu { cursor: 0 }
        );
    }

    #[test]
    fn cursor_skips_disabled_entries_and_wraps() {
        assert_eq!(move_cursor(0, false), 2);
        assert_eq!(move_cursor(2, false), 0);
        assert_eq!(move_cursor(0, true), 2);
        assert_eq!(move_cursor(2, true), 0);
        let down = Scene::Menu { cursor: 0 }.step(press(DsButton::Down));
        assert_eq!(down, Scene::Menu { cursor: 2 });
    }

    #[test]
    fn continue_is_never_selected() {
        assert!(!MenuItem::Continue.enabled());
        // Only reachable if the cursor sits on it, which move_cursor never does.
        let s = Scene::Menu { cursor: 1 };
        assert_eq!(s.step(press(DsButton::A)), s);
    }

    #[test]
    fn menu_entries_open_placeholders_and_b_returns() {
        assert_eq!(
            Scene::Menu { cursor: 0 }.step(press(DsButton::A)),
            Scene::NewGame
        );
        assert_eq!(
            Scene::Menu { cursor: 2 }.step(press(DsButton::Start)),
            Scene::Options
        );
        assert_eq!(
            Scene::NewGame.step(press(DsButton::B)),
            Scene::Menu { cursor: 0 }
        );
        assert_eq!(
            Scene::Options.step(press(DsButton::B)),
            Scene::Menu { cursor: 2 }
        );
        assert_eq!(Scene::Options.step(press(DsButton::A)), Scene::Options);
    }

    #[test]
    fn refused_stays_refused() {
        assert_eq!(Scene::Refused.step(press(DsButton::A)), Scene::Refused);
    }
}
