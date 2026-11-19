//! Scene graph for the boot flow: splash, title, main menu, and the placeholder
//! scenes behind the menu entries. Pure logic: one [`Scene::step`] per logic tick.
//!
//! Order (from the title loop in `docs/re/execution-model.md`, "Start-up and the title
//! menu"): splash, then title, then the menu. A fresh boot shows New Game and Adventure
//! Log only (`docs/verification.md`, overlay 7 entry). With a save the menu is Blue's (#537):
//! Continue, Delete Save Data, Adventure Log, Friend Rescue, Trade Items, Wonder Mail, with no
//! New Game; `app` loads the save and deletes it. The three DS-link and password entries open
//! a not-available message (`docs/supported-dumps.md`, Limitations).

use rescue_platform::{ButtonState, DsButton};

/// Ticks the splash stays up without a press (about 1.5 s at the DS rate).
pub const SPLASH_TICKS: u32 = 90;

/// Blue's title-menu choice numbers (`docs/re/execution-model.md`, "Start-up and the title
/// menu"): 2 is `RunGameMode_Async(1)`, i.e. Continue; 3 is a new game; 4 is
/// `RunGameMode_Async(3)`, the Adventure Log.
pub const CHOICE_CONTINUE: u8 = 2;
pub const CHOICE_NEW_GAME: u8 = 3;
pub const CHOICE_ADVENTURE_LOG: u8 = 4;

/// Main menu entries, in on-screen order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Continue,
    NewGame,
    DeleteSave,
    AdventureLog,
    FriendRescue,
    TradeItems,
    WonderMail,
}

impl MenuItem {
    pub const ALL: [MenuItem; 2] = [MenuItem::NewGame, MenuItem::AdventureLog];
    const WITH_SAVE: [MenuItem; 6] = [
        MenuItem::Continue,
        MenuItem::DeleteSave,
        MenuItem::AdventureLog,
        MenuItem::FriendRescue,
        MenuItem::TradeItems,
        MenuItem::WonderMail,
    ];

    /// The entries on screen: Blue's six with a save, else New Game and Adventure Log.
    pub fn list(has_save: bool) -> &'static [MenuItem] {
        if has_save {
            &Self::WITH_SAVE
        } else {
            &Self::ALL
        }
    }

    /// Blue's choice number for the entry (2 to 4). `None` for the entries whose number is not
    /// established: Blue's choice 5 is `RunOverlay7Mode`, and which entry reaches it was not
    /// seen, so none is mapped to it.
    pub fn choice(self) -> Option<u8> {
        match self {
            MenuItem::Continue => Some(CHOICE_CONTINUE),
            MenuItem::NewGame => Some(CHOICE_NEW_GAME),
            MenuItem::AdventureLog => Some(CHOICE_ADVENTURE_LOG),
            _ => None,
        }
    }

    /// The English name, for the not-available message.
    pub fn name(self) -> &'static str {
        match self {
            MenuItem::Continue => "Continue",
            MenuItem::NewGame => "New Game",
            MenuItem::DeleteSave => "Delete Save Data",
            MenuItem::AdventureLog => "Adventure Log",
            MenuItem::FriendRescue => "Friend Rescue",
            MenuItem::TradeItems => "Trade Items",
            MenuItem::WonderMail => "Wonder Mail",
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
    /// `cursor` indexes [`MenuItem::list`].
    Menu {
        cursor: usize,
    },
    /// The adventure log screen; B goes back.
    AdventureLog,
    /// Delete Save Data's confirmation; `yes` is the cursor, which starts on No.
    ConfirmDelete {
        yes: bool,
    },
    /// The not-available message of a DS-link or password entry; `cursor` is its menu row.
    Unavailable {
        cursor: usize,
    },
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
                let n = items.len();
                if pressed.is_held(DsButton::Down) {
                    Scene::Menu {
                        cursor: (cursor + 1) % n,
                    }
                } else if pressed.is_held(DsButton::Up) {
                    Scene::Menu {
                        cursor: (cursor + n - 1) % n,
                    }
                } else if confirm {
                    let item = items.get(cursor).copied().unwrap_or(MenuItem::NewGame);
                    match item.choice() {
                        Some(CHOICE_CONTINUE) => self,
                        Some(CHOICE_NEW_GAME) => Scene::Dungeon,
                        Some(CHOICE_ADVENTURE_LOG) => Scene::AdventureLog,
                        _ if item == MenuItem::DeleteSave => Scene::ConfirmDelete { yes: false },
                        _ => Scene::Unavailable { cursor },
                    }
                } else {
                    self
                }
            }
            Scene::AdventureLog if back => Scene::Menu {
                cursor: row_of(items, MenuItem::AdventureLog),
            },
            Scene::ConfirmDelete { yes } => {
                let toggle = [
                    DsButton::Up,
                    DsButton::Down,
                    DsButton::Left,
                    DsButton::Right,
                ]
                .into_iter()
                .any(|b| pressed.is_held(b));
                if back {
                    Scene::Menu {
                        cursor: row_of(items, MenuItem::DeleteSave),
                    }
                } else if toggle {
                    Scene::ConfirmDelete { yes: !yes }
                } else if confirm && yes {
                    // The save is gone: `app` deletes it, and the fresh-boot menu follows.
                    Scene::Menu { cursor: 0 }
                } else if confirm {
                    Scene::Menu {
                        cursor: row_of(items, MenuItem::DeleteSave),
                    }
                } else {
                    self
                }
            }
            Scene::Unavailable { cursor } if confirm || back => Scene::Menu { cursor },
            _ => self,
        }
    }
}

fn row_of(items: &[MenuItem], item: MenuItem) -> usize {
    items.iter().position(|i| *i == item).unwrap_or(0)
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
    fn cursor_wraps_both_ways() {
        let down = Scene::Menu { cursor: 0 }.step(press(DsButton::Down));
        assert_eq!(down, Scene::Menu { cursor: 1 });
        assert_eq!(down.step(press(DsButton::Down)), Scene::Menu { cursor: 0 });
        assert_eq!(
            Scene::Menu { cursor: 0 }.step_with(press(DsButton::Up), true),
            Scene::Menu { cursor: 5 }
        );
        assert_eq!(
            Scene::Menu { cursor: 5 }.step_with(press(DsButton::Down), true),
            Scene::Menu { cursor: 0 }
        );
    }

    #[test]
    fn menu_entries_open_their_screens_and_b_returns() {
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
    fn the_save_menu_is_blues_order_without_new_game() {
        assert_eq!(
            MenuItem::list(true),
            [
                MenuItem::Continue,
                MenuItem::DeleteSave,
                MenuItem::AdventureLog,
                MenuItem::FriendRescue,
                MenuItem::TradeItems,
                MenuItem::WonderMail
            ]
        );
        assert_eq!(MenuItem::list(false), MenuItem::ALL);
    }

    #[test]
    fn the_title_choices_are_blues_numbers() {
        assert_eq!(MenuItem::Continue.choice(), Some(2));
        assert_eq!(MenuItem::NewGame.choice(), Some(3));
        assert_eq!(MenuItem::AdventureLog.choice(), Some(4));
        assert_eq!(MenuItem::DeleteSave.choice(), None);
    }

    #[test]
    fn continue_stays_on_the_menu_for_app_to_load() {
        let at = Scene::Menu { cursor: 0 };
        assert_eq!(at.step_with(press(DsButton::A), true), at);
    }

    #[test]
    fn the_adventure_log_returns_to_its_row() {
        let log = Scene::Menu { cursor: 2 }.step_with(press(DsButton::A), true);
        assert_eq!(log, Scene::AdventureLog);
        assert_eq!(
            log.step_with(press(DsButton::B), true),
            Scene::Menu { cursor: 2 }
        );
    }

    #[test]
    fn delete_asks_first_and_starts_on_no() {
        let ask = Scene::Menu { cursor: 1 }.step_with(press(DsButton::A), true);
        assert_eq!(ask, Scene::ConfirmDelete { yes: false });
        // A on No, and B, go back to the Delete row.
        let back = Scene::Menu { cursor: 1 };
        assert_eq!(ask.step_with(press(DsButton::A), true), back);
        assert_eq!(ask.step_with(press(DsButton::B), true), back);
        let yes = ask.step_with(press(DsButton::Down), true);
        assert_eq!(yes, Scene::ConfirmDelete { yes: true });
        assert_eq!(yes.step_with(press(DsButton::Up), true), ask);
        assert_eq!(
            yes.step_with(press(DsButton::A), true),
            Scene::Menu { cursor: 0 }
        );
    }

    #[test]
    fn the_ds_entries_show_a_message_that_returns_to_their_row() {
        for cursor in 3..6 {
            let m = Scene::Menu { cursor }.step_with(press(DsButton::A), true);
            assert_eq!(m, Scene::Unavailable { cursor });
            for b in [DsButton::A, DsButton::B] {
                assert_eq!(m.step_with(press(b), true), Scene::Menu { cursor });
            }
            assert_eq!(m.step_with(press(DsButton::Down), true), m);
        }
    }

    #[test]
    fn refused_stays_refused() {
        assert_eq!(Scene::Refused.step(press(DsButton::A)), Scene::Refused);
    }
}
