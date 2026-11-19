//! Placeholder drawing: coloured blocks stand in for the original screens until the ROM's
//! graphics and text are loaded. Layout only; no text is drawn yet.

use rescue_platform::{Painter, PlatformError, Rgb, Screen};
use rescue_render::bg::Frame;
use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};

use crate::dungeon::Dungeon;
use crate::scene::{SPLASH_TICKS, Scene};

const BACKGROUND: Rgb = Rgb(24, 32, 72);
const REFUSED: Rgb = Rgb(120, 24, 24);
const BAR: Rgb = Rgb(240, 200, 64);
const ROW: Rgb = Rgb(40, 90, 56);
const ROW_CURSOR: Rgb = Rgb(250, 250, 250);
/// The notice bar: x, y, width, height.
const NOTICE: (u32, u32, u32, u32) = (32, 176, 192, 8);
const BOTTOM: Rgb = Rgb(16, 56, 32);

/// What the screens show: the `scene`; `dungeon`, the floor while the dungeon scene is up;
/// `menu`, the move menu's cursor while it is open (drawn over the lower screen); `menu_items`,
/// the main menu's entries; and whether a `notice` (a refused save) is up.
pub struct View<'a> {
    pub scene: Scene,
    pub dungeon: Option<&'a Dungeon>,
    pub menu: Option<usize>,
    pub menu_items: usize,
    pub notice: bool,
    /// The title screens' frame for the screen being drawn, when the art is loaded.
    pub title: Option<&'a Frame>,
}

/// Draws one screen of the view.
pub fn screen(view: &View<'_>, screen: Screen, p: &mut Painter<'_>) -> Result<(), PlatformError> {
    let View {
        scene,
        dungeon,
        menu,
        menu_items,
        notice,
        title,
    } = *view;
    if let Some(frame) = title {
        if screen == Screen::Top && notice && matches!(scene, Scene::Menu { .. }) {
            return blit(&with_notice(frame), p);
        }
        return blit(frame, p);
    }
    match screen {
        Screen::Top => top(scene, dungeon, (menu_items, notice), p),
        Screen::Bottom if scene == Scene::Dungeon && menu.is_some() => {
            p.clear(BOTTOM);
            // shortcut: no text is drawn yet, so a move's name is not shown: one row per slot, and the last one is Suspend.
            for i in 0..crate::input::MENU_ROWS {
                let colour = if menu == Some(i) { ROW_CURSOR } else { ROW };
                p.fill_rect(64, 40 + 24 * i as i32, 128, 16, colour)?;
            }
            Ok(())
        }
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

fn top(
    scene: Scene,
    dungeon: Option<&Dungeon>,
    (menu_items, notice): (usize, bool),
    p: &mut Painter<'_>,
) -> Result<(), PlatformError> {
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
            for i in 0..menu_items {
                let colour = if i == cursor { ROW_CURSOR } else { ROW };
                p.fill_rect(64, 96 + 24 * i as i32, 128, 16, colour)?;
            }
            // shortcut: no text is drawn yet, so a refused save is a red bar (the reason is printed).
            if notice {
                p.fill_rect(
                    NOTICE.0 as i32,
                    NOTICE.1 as i32,
                    NOTICE.2,
                    NOTICE.3,
                    REFUSED,
                )?;
            }
        }
        Scene::AdventureLog
        | Scene::ConfirmDelete { .. }
        | Scene::Unavailable { .. }
        | Scene::Dungeon => p.fill_rect(48, 80, 160, 32, ROW)?,
    }
    Ok(())
}

/// `frame` with the refused-save bar over its bottom edge, as the placeholder menu draws it.
// shortcut: no text is drawn over the ROM art either, so a failed Continue is a red bar.
fn with_notice(frame: &Frame) -> Frame {
    let mut out = frame.clone();
    for y in NOTICE.1..NOTICE.1 + NOTICE.3 {
        for x in NOTICE.0..NOTICE.0 + NOTICE.2 {
            let i = ((y * SCREEN_WIDTH + x) * 4) as usize;
            out.pixels[i..i + 4].copy_from_slice(&[REFUSED.0, REFUSED.1, REFUSED.2, 255]);
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_bar_is_drawn_over_the_title_frame() {
        let frame = Frame::filled([1, 2, 3, 255]);
        let out = with_notice(&frame);
        assert_eq!(out.get(32, 176), [REFUSED.0, REFUSED.1, REFUSED.2, 255]);
        assert_eq!(out.get(223, 183), [REFUSED.0, REFUSED.1, REFUSED.2, 255]);
        assert_eq!(out.get(31, 176), [1, 2, 3, 255]);
        assert_eq!(out.get(32, 184), [1, 2, 3, 255]);
        assert_eq!(
            frame.get(32, 176),
            [1, 2, 3, 255],
            "the source frame is untouched"
        );
    }
}
