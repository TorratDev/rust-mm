//! The game state the platform loop drives. No SDL here, so the boot flow runs headless.

use rescue_platform::{ButtonState, Flow};

use crate::boot::RomCheck;
use crate::dungeon::{self, Dungeon, FloorData};
use crate::scene::Scene;

/// The running game: the current scene, last tick's held buttons, the ROM's floor (None for
/// the placeholder), the seed the dungeon scene generates from, and the floor while the
/// dungeon scene is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub scene: Scene,
    held: ButtonState,
    floor: Option<FloorData>,
    seed: u32,
    dungeon: Option<Dungeon>,
}

impl App {
    /// `open_dungeon` starts on the dungeon scene instead of the splash (with a ROM).
    pub fn new(rom: &RomCheck, floor: Option<FloorData>, seed: u32, open_dungeon: bool) -> App {
        let scene = match rom {
            RomCheck::Ready(_) if open_dungeon => Scene::Dungeon,
            RomCheck::Ready(_) => Scene::Splash { ticks: 0 },
            RomCheck::NotConfigured | RomCheck::Refused(_) => Scene::Refused,
        };
        let mut app = App {
            scene,
            held: ButtonState::default(),
            floor,
            seed,
            dungeon: None,
        };
        if app.scene == Scene::Dungeon {
            app.enter_dungeon();
        }
        app
    }

    /// Builds the floor on entry. If it cannot be built, the menu stays up.
    fn enter_dungeon(&mut self) {
        match dungeon::build(self.seed, self.floor.as_ref()) {
            Some(d) => self.dungeon = Some(d),
            None => self.scene = Scene::Menu { cursor: 0 },
        }
    }

    /// The floor the dungeon scene shows, while it is on screen.
    pub fn dungeon(&self) -> Option<&Dungeon> {
        self.dungeon.as_ref()
    }

    /// One logic tick with the buttons held now.
    pub fn tick(&mut self, buttons: ButtonState) -> Flow {
        let mut pressed = ButtonState::default();
        for b in buttons.held().filter(|b| !self.held.is_held(*b)) {
            pressed.set(b, true);
        }
        self.held = buttons;
        if self.scene == Scene::Refused {
            return if pressed.held().next().is_some() {
                Flow::Quit
            } else {
                Flow::Continue
            };
        }
        self.scene = self.scene.step(pressed);
        if self.scene == Scene::Dungeon && self.dungeon.is_none() {
            // Dropped on leaving, so each visit starts from the same seed.
            self.enter_dungeon();
        } else if self.scene != Scene::Dungeon {
            self.dungeon = None;
        }
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_nds::rom::RomId;
    use rescue_platform::DsButton;

    fn held(b: DsButton) -> ButtonState {
        let mut s = ButtonState::default();
        s.set(b, true);
        s
    }

    fn tap(app: &mut App, b: DsButton) {
        app.tick(held(b));
        app.tick(ButtonState::default());
    }

    #[test]
    fn opens_on_the_dungeon_when_asked() {
        let ready = RomCheck::Ready(RomId::BlueRescueTeamEur);
        let app = App::new(&ready, None, dungeon::SEED, true);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.dungeon().is_some(), "the floor is built at start");
        let refused = App::new(&RomCheck::NotConfigured, None, dungeon::SEED, true);
        assert_eq!(refused.scene, Scene::Refused);
    }

    #[test]
    fn boot_to_menu_and_back_headless() {
        let mut app = App::new(
            &RomCheck::Ready(RomId::BlueRescueTeamEur),
            None,
            dungeon::SEED,
            false,
        );
        assert_eq!(app.scene, Scene::Splash { ticks: 0 });
        for _ in 0..crate::scene::SPLASH_TICKS {
            app.tick(ButtonState::default());
        }
        assert_eq!(app.scene, Scene::Title);
        tap(&mut app, DsButton::Start);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        tap(&mut app, DsButton::Down);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::AdventureLog);
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
        tap(&mut app, DsButton::Up);
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.dungeon().is_some(), "entering builds the floor");
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        assert!(app.dungeon().is_none(), "leaving drops the floor");
    }

    #[test]
    fn a_held_button_counts_once() {
        let mut app = App::new(
            &RomCheck::Ready(RomId::BlueRescueTeamEur),
            None,
            dungeon::SEED,
            false,
        );
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Title);
        // Still held: must not also fire the title's press on the next tick.
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Title);
    }

    #[test]
    fn refused_rom_quits_on_press() {
        let mut app = App::new(&RomCheck::NotConfigured, None, dungeon::SEED, false);
        assert_eq!(app.scene, Scene::Refused);
        assert_eq!(app.tick(ButtonState::default()), Flow::Continue);
        assert_eq!(app.tick(held(DsButton::A)), Flow::Quit);
    }
}
