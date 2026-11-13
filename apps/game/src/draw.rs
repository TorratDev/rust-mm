//! Placeholder drawing: coloured blocks stand in for the original screens until the ROM's
//! graphics and text are loaded. Layout only; no text is drawn yet.

use rescue_platform::{Painter, PlatformError, Rgb, Screen};
use rescue_render::bg::Frame;
use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};

use crate::dungeon::Dungeon;
use crate::scene::{MenuItem, SPLASH_TICKS, Scene};

const BACKGROUND: Rgb = Rgb(24, 32, 72);
const REFUSED: Rgb = Rgb(120, 24, 24);
const BAR: Rgb = Rgb(240, 200, 64);
const ROW: Rgb = Rgb(40, 90, 56);
const ROW_CURSOR: Rgb = Rgb(250, 250, 250);
const BOTTOM: Rgb = Rgb(16, 56, 32);

/// Draws one screen of `scene`. `dungeon` is the floor while the dungeon scene is up.
pub fn screen(
    scene: Scene,
    dungeon: Option<&Dungeon>,
    screen: Screen,
    p: &mut Painter<'_>,
) -> Result<(), PlatformError> {
    match screen {
        Screen::Top => top(scene, dungeon, p),
        Screen::Bottom => match (scene, dungeon) {
            (
                Scene::Dungeon,
                Some(Dungeon {
                    minimap: Some(m), ..
                }),
            ) => blit(m, p),
            _ => {
                p.clear(BOTTOM);
                Ok(())
            }
        },
    }
}

fn top(scene: Scene, dungeon: Option<&Dungeon>, p: &mut Painter<'_>) -> Result<(), PlatformError> {
    if scene == Scene::Refused {
        p.clear(REFUSED);
        return Ok(());
    }
    if let (Scene::Dungeon, Some(d)) = (scene, dungeon) {
        return blit(&d.frame, p);
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
        Scene::AdventureLog | Scene::Dungeon => p.fill_rect(48, 80, 160, 32, ROW)?,
    }
    Ok(())
}

/// Copies `frame` to the top screen, one fill per run of equal pixels in a row.
// shortcut: one fill_rect per run, not a texture upload; upgrade when the platform has one (#60).
fn blit(frame: &Frame, p: &mut Painter<'_>) -> Result<(), PlatformError> {
    for y in 0..SCREEN_HEIGHT {
        let mut x = 0;
        while x < SCREEN_WIDTH {
            let c = frame.get(x, y);
            let start = x;
            while x < SCREEN_WIDTH && frame.get(x, y) == c {
                x += 1;
            }
            p.fill_rect(start as i32, y as i32, x - start, 1, Rgb(c[0], c[1], c[2]))?;
        }
    }
    Ok(())
}
