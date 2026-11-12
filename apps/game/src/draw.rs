//! Placeholder drawing: coloured blocks stand in for the original screens until the ROM's
//! graphics and text are loaded. Layout only; no text is drawn yet.

use rescue_platform::{Painter, PlatformError, Rgb, Screen};

use crate::scene::{MenuItem, SPLASH_TICKS, Scene};

const BACKGROUND: Rgb = Rgb(24, 32, 72);
const REFUSED: Rgb = Rgb(120, 24, 24);
const BAR: Rgb = Rgb(240, 200, 64);
const ROW: Rgb = Rgb(40, 90, 56);
const ROW_CURSOR: Rgb = Rgb(250, 250, 250);
const BOTTOM: Rgb = Rgb(16, 56, 32);

/// Draws one screen of `scene`.
pub fn screen(scene: Scene, screen: Screen, p: &mut Painter<'_>) -> Result<(), PlatformError> {
    match screen {
        Screen::Top => top(scene, p),
        Screen::Bottom => {
            p.clear(BOTTOM);
            Ok(())
        }
    }
}

fn top(scene: Scene, p: &mut Painter<'_>) -> Result<(), PlatformError> {
    if scene == Scene::Refused {
        p.clear(REFUSED);
        return Ok(());
    }
    p.clear(BACKGROUND);
    match scene {
        Scene::Refused => {}
        Scene::Splash { ticks } => {
            // A bar that fills up over the splash.
            let w = 160 * ticks.min(SPLASH_TICKS) / SPLASH_TICKS;
            p.fill_rect(48, 88, w, 16, BAR)?;
        }
        Scene::Title => p.fill_rect(48, 80, 160, 32, BAR)?,
        Scene::Menu { cursor } => {
            p.fill_rect(48, 24, 160, 24, BAR)?;
            for i in 0..MenuItem::ALL.len() {
                let colour = if i == cursor { ROW_CURSOR } else { ROW };
                p.fill_rect(64, 96 + 24 * i as i32, 128, 16, colour)?;
            }
        }
        Scene::AdventureLog | Scene::NewGame => p.fill_rect(48, 80, 160, 32, ROW)?,
    }
    Ok(())
}
