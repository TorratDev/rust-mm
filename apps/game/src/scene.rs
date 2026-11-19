//! Scene graph for the boot flow: splash, title, main menu, and the placeholder
//! scenes behind the menu entries. Pure logic: one [`Scene::step`] per logic tick.
//!
//! Order (from the title loop in `docs/re/execution-model.md`, "Start-up and the title
//! menu"): splash, then title, then the menu. A fresh boot shows New Game and Adventure
//! Log only (`docs/verification.md`, overlay 7 entry). Continue is added as the first entry when
//! a suspend save exists; `app` loads it.

use rescue_platform::{ButtonState, DsButton};

/// Ticks the splash stays up without a press (about 1.5 s at the DS rate).
pub const SPLASH_TICKS: u32 = 90;

/// Main menu entries, in on-screen order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Continue,
    NewGame,
    AdventureLog,
}

impl MenuItem {
    pub const ALL: [MenuItem; 2] = [MenuItem::NewGame, MenuItem::AdventureLog];

    /// The entries on screen: Continue first when a save exists.
    pub fn list(has_save: bool) -> &'static [MenuItem] {
        if has_save {
            &[
                MenuItem::Continue,
                MenuItem::NewGame,
                MenuItem::AdventureLog,
            ]
        } else {
            &Self::ALL
        }
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
    /// Placeholder until the adventure log exists.
    AdventureLog,
    /// The dungeon: the run, driven by `app` (#70, #347). B is run here, so it does not leave;
    /// the dungeon's clear goes back to the menu.
    Dungeon,
}

impl Scene {
    /// Advances one logic tick. `pressed` holds the buttons that went down this tick,
    /// not the ones held.
    #[cfg(test)]
    pub fn step(self, pressed: ButtonState) -> Scene {
        self.step_with(pressed, false)
    }

    /// [`Scene::step`] with the menu's Continue entry when `has_save`. Confirming Continue
    /// leaves the scene on the menu: `app` loads the save and opens the dungeon, or reports it.
    pub fn step_with(self, pressed: ButtonState, has_save: bool) -> Scene {
        let items = MenuItem::list(has_save);
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
                if pressed.is_held(DsButton::Up) || pressed.is_held(DsButton::Down) {
                    Scene::Menu {
                        cursor: (cursor + 1) % items.len(),
                    }
                } else if confirm {
                    match items.get(cursor).copied().unwrap_or(MenuItem::NewGame) {
                        MenuItem::Continue => self,
                        MenuItem::NewGame => Scene::Dungeon,
                        MenuItem::AdventureLog => Scene::AdventureLog,
                    }
                } else {
                    self
                }
            }
            Scene::AdventureLog if back => Scene::Menu {
                cursor: items
                    .iter()
                    .position(|i| *i == MenuItem::AdventureLog)
                    .unwrap_or(0),
            },
            _ => self,
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
    fn fresh_boot_menu_has_only_new_game_and_adventure_log() {
        assert_eq!(MenuItem::ALL, [MenuItem::NewGame, MenuItem::AdventureLog]);
    }

    #[test]
    fn cursor_moves_between_the_two_entries() {
        let down = Scene::Menu { cursor: 0 }.step(press(DsButton::Down));
        assert_eq!(down, Scene::Menu { cursor: 1 });
        let wrapped = down.step(press(DsButton::Down));
        assert_eq!(wrapped, Scene::Menu { cursor: 0 });
        assert_eq!(
            Scene::Menu { cursor: 0 }.step(press(DsButton::Up)),
            Scene::Menu { cursor: 1 }
        );
    }

    #[test]
    fn menu_entries_open_placeholders_and_b_returns() {
        assert_eq!(
            Scene::Menu { cursor: 0 }.step(press(DsButton::A)),
            Scene::Dungeon
        );
        assert_eq!(
            Scene::Menu { cursor: 1 }.step(press(DsButton::Start)),
            Scene::AdventureLog
        );
        // B is run in the dungeon, so it does not leave.
        assert_eq!(Scene::Dungeon.step(press(DsButton::B)), Scene::Dungeon);
        assert_eq!(
            Scene::AdventureLog.step(press(DsButton::B)),
            Scene::Menu { cursor: 1 }
        );
        assert_eq!(
            Scene::AdventureLog.step(press(DsButton::A)),
            Scene::AdventureLog
        );
    }

    #[test]
    fn continue_is_the_first_entry_with_a_save_and_stays_on_the_menu() {
        assert_eq!(MenuItem::list(true)[0], MenuItem::Continue);
        assert_eq!(MenuItem::list(false), MenuItem::ALL);
        let at = Scene::Menu { cursor: 0 };
        assert_eq!(at.step_with(press(DsButton::A), true), at);
        let down = at.step_with(press(DsButton::Down), true);
        assert_eq!(down, Scene::Menu { cursor: 1 });
        assert_eq!(down.step_with(press(DsButton::A), true), Scene::Dungeon);
        let log = Scene::Menu { cursor: 2 }.step_with(press(DsButton::A), true);
        assert_eq!(log, Scene::AdventureLog);
        assert_eq!(
            log.step_with(press(DsButton::B), true),
            Scene::Menu { cursor: 2 }
        );
    }

    #[test]
    fn refused_stays_refused() {
        assert_eq!(Scene::Refused.step(press(DsButton::A)), Scene::Refused);
    }
}
