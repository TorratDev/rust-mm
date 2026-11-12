//! The game state the platform loop drives. No SDL here, so the boot flow runs headless.

use rescue_platform::{ButtonState, Flow};

use crate::boot::RomCheck;
use crate::scene::Scene;

/// The running game: the current scene and last tick's held buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub scene: Scene,
    held: ButtonState,
}

impl App {
    pub fn new(rom: &RomCheck) -> App {
        let scene = match rom {
            RomCheck::Ready(_) => Scene::Splash { ticks: 0 },
            RomCheck::NotConfigured | RomCheck::Refused(_) => Scene::Refused,
        };
        App {
            scene,
            held: ButtonState::default(),
        }
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
    fn boot_to_menu_and_back_headless() {
        let mut app = App::new(&RomCheck::Ready(RomId::BlueRescueTeamEur));
        assert_eq!(app.scene, Scene::Splash { ticks: 0 });
        for _ in 0..crate::scene::SPLASH_TICKS {
            app.tick(ButtonState::default());
        }
        assert_eq!(app.scene, Scene::Title);
        tap(&mut app, DsButton::Start);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        tap(&mut app, DsButton::Down);
        assert_eq!(app.scene, Scene::Menu { cursor: 2 });
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Options);
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Menu { cursor: 2 });
        tap(&mut app, DsButton::Up);
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::NewGame);
    }

    #[test]
    fn a_held_button_counts_once() {
        let mut app = App::new(&RomCheck::Ready(RomId::BlueRescueTeamEur));
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Title);
        // Still held: must not also fire the title's press on the next tick.
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Title);
    }

    #[test]
    fn refused_rom_quits_on_press() {
        let mut app = App::new(&RomCheck::NotConfigured);
        assert_eq!(app.scene, Scene::Refused);
        assert_eq!(app.tick(ButtonState::default()), Flow::Continue);
        assert_eq!(app.tick(held(DsButton::A)), Flow::Quit);
    }
}
