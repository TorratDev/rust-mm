//! The title screens (#380): the logo screen and the menu screen, drawn from the ROM.
//!
//! Logo screen (waits for START): ground map `S02` of `ground.sbin`, layer 0 over layer 1, on
//! the lower screen; the upper screen is black. Menu screen: a random `titlenK` backdrop on the
//! lower screen (engine A, BG3 map 1 over BG2 map 0), the `subdef` logo on the upper screen
//! (engine B), and on the lower screen the two windows (font frame tiles over the fill), the
//! text and the blinking cursor (`fontsp` tile 4). Both backdrops fade with the 32-step
//! counter of `rescue_formats::titlemenu::fade_color`. Evidence: the findings of #380
//! (investigator a): `FUN_0201F008`, the fade loop in `GameLoop_Async`, the message hook.
//!
//! Not drawn: the `titleop1`/`titleop2`/`titleopc` sprites (envelope and birds) and the demo
//! before the logo screen; the logo screen's own fade and the 33-frame transition at START.

use rescue_formats::archive::Archive;
use rescue_formats::font::{Glyph, parse_font};
use rescue_formats::gfx::{self, Rgba};
use rescue_formats::graphics::Palette;
use rescue_formats::ground;
use rescue_formats::text::{self, Token};
use rescue_formats::titlemenu::{self, TitleScreen};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_render::bg::{BgLayer, Frame, compose, ground_layer};
use rescue_render::text::{Canvas, DEFAULT_COLOUR, LINE_PITCH, draw_glyph};
use rescue_render::ui::{TileRect, TileSet, draw_cursor, draw_fill, draw_menu_frame};
use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};

use crate::scene::MenuItem;

/// Steps of the fade counter, in frames (`docs/re/io.md`, the fade loop of the title).
pub const FADE_FRAMES: u32 = 32;
/// Backdrops `titlen0` to `titlen2`; the game draws `FUN_02011968(3)`.
pub const BACKDROPS: usize = 3;
/// Frames the cursor is shown, then hidden. `[hypothesis]`: from six samples of OAM slot 127
/// (visible f2541, hidden f2544 to 2550, visible f2553 to 2559, hidden f2562 to 2565).
pub const BLINK_FRAMES: u32 = 8;
/// The cursor's top-left on the first row, and the pitch between rows. `[hypothesis]`: low
/// confidence, from two OAM samples (y = 27, 39).
const CURSOR_AT: (i32, i32) = (24, 27);
const ROW_PITCH: i32 = 12;
/// Item text origin. `[hypothesis]`: x about 31 from the notes; y is the cursor's minus 2.
const ITEM_TEXT: (i32, i32) = (31, 25);
/// Description text origin: the window's top-left plus the dialogue's `LINE_START`.
const DESC_TEXT: (i32, i32) = (28, 132);
const ITEM_WINDOW: TileRect = TileRect {
    col: 2,
    row: 2,
    cols: 15,
    rows: 5,
};
const DESC_WINDOW: TileRect = TileRect {
    col: 2,
    row: 16,
    cols: 28,
    rows: 5,
};
const ITEM_CANVAS: (usize, usize) = (96, 12);
const DESC_CANVAS: (usize, usize) = (216, 24);

/// Message ids (1-based, `docs/formats/text.md`) of a menu entry and its description lines.
/// Names: 2238 to 2243 and 2247 are `[blue-verified]` (#380). Descriptions: 2261 to 2264,
/// 2267 and 2268 are `[blue-verified]`; Delete Save Data's 2269 and 2270 are `[hypothesis]`
/// (the findings say "2269..", not checked). The three DS entries' descriptions are not
/// known, so none is shown.
fn text_ids(item: MenuItem) -> (u32, &'static [u32]) {
    match item {
        MenuItem::Continue => (2240, &[2267, 2268]),
        MenuItem::NewGame => (2238, &[2261, 2262]),
        MenuItem::DeleteSave => (2241, &[2269, 2270]),
        MenuItem::AdventureLog => (2239, &[2263, 2264]),
        MenuItem::FriendRescue => (2242, &[]),
        MenuItem::TradeItems => (2243, &[]),
        MenuItem::WonderMail => (2247, &[]),
    }
}

const ITEMS: [MenuItem; 7] = [
    MenuItem::Continue,
    MenuItem::NewGame,
    MenuItem::DeleteSave,
    MenuItem::AdventureLog,
    MenuItem::FriendRescue,
    MenuItem::TradeItems,
    MenuItem::WonderMail,
];

/// The glyphs of the dialogue font, sorted by character code.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Glyphs(Vec<Glyph>);

impl Glyphs {
    fn new(mut glyphs: Vec<Glyph>) -> Self {
        glyphs.sort_by_key(|g| g.code);
        Glyphs(glyphs)
    }

    fn get(&self, code: u16) -> Option<&Glyph> {
        self.0
            .binary_search_by_key(&code, |g| g.code)
            .ok()
            .and_then(|i| self.0.get(i))
    }
}

/// A message screen on the menu's backdrop: a window of `lines`, then `choices` (the row at
/// `cursor` has the cursor), as the confirmation and the not-available message use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelView<'a> {
    pub backdrop: usize,
    pub lines: &'a [String],
    pub choices: &'a [&'a str],
    pub cursor: usize,
}

const PANEL_WINDOW: TileRect = TileRect {
    col: 2,
    row: 2,
    cols: 28,
    rows: 19,
};
const PANEL_TEXT: (i32, i32) = (28, 24);
const PANEL_CANVAS: (usize, usize) = (200, 12);

/// What the menu screen shows besides the backdrop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuView<'a> {
    /// Backdrop number, `0..BACKDROPS`.
    pub backdrop: usize,
    /// Fade counter, `0..=FADE_FRAMES`.
    pub fade: u32,
    /// Whether the windows are up (not during the fade in).
    pub windows: bool,
    pub items: &'a [MenuItem],
    pub cursor: usize,
    /// Frames the menu has been up, for the cursor blink.
    pub blink: u32,
}

/// A decoded title screen with its 224-colour palette (RGBX as stored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub art: TitleScreen,
    pub palette: Palette,
}

impl Picture {
    /// The frame at fade `level`: BG2 (map 0) over BG3 (map 1), palette banks 0 to 13 faded,
    /// the backdrop colour being faded colour 0.
    fn render(&self, level: u32) -> Frame {
        let colours: Vec<Rgba> = titlemenu::fade_palette(&self.palette, level)
            .into_iter()
            .map(gfx::bgr555)
            .collect();
        let banks: Vec<Palette> = colours.chunks(16).map(<[Rgba]>::to_vec).collect();
        let layer =
            |map: &[gfx::MapEntry]| BgLayer::new(self.art.tiles.clone(), map.to_vec(), 32, 32);
        let layers: Vec<BgLayer> = [layer(&self.art.map0), layer(&self.art.map1)]
            .into_iter()
            .flatten()
            .collect();
        let backdrop = colours.first().copied().unwrap_or([0, 0, 0, 255]);
        compose(&layers, &banks, backdrop)
    }
}

/// Everything the title screens draw, read from the dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleArt {
    logo: Vec<BgLayer>,
    logo_palettes: Vec<Palette>,
    backdrops: Vec<Picture>,
    subdef: Picture,
    frames: TileSet,
    sprites: TileSet,
    glyphs: Glyphs,
    /// Per `ITEMS`: the entry's text, and its description lines.
    texts: Vec<(Canvas, Canvas)>,
}

/// Draws `lines` (message strings) into a canvas, one line per string, with `font`. Only plain
/// text and line breaks are drawn; no menu string uses a control code.
fn render_lines(font: &Glyphs, lines: &[&[u8]], (w, h): (usize, usize)) -> Canvas {
    let mut canvas = Canvas::new(w, h);
    let mut y = 0;
    for line in lines {
        let mut x = 0;
        for token in text::decode(line) {
            match token {
                Token::Text(s) => {
                    for c in s.chars() {
                        let Some(g) = u16::try_from(u32::from(c)).ok().and_then(|c| font.get(c))
                        else {
                            continue;
                        };
                        x += draw_glyph(&mut canvas, g, x, y, DEFAULT_COLOUR);
                    }
                }
                Token::Newline => {
                    x = 0;
                    y += LINE_PITCH;
                }
                _ => {}
            }
        }
        y += LINE_PITCH;
    }
    canvas
}

impl TitleArt {
    /// Reads the title art from the dump `rom`: ground map `S02`, `titlemenu.sbin`'s `titlen0`
    /// to `titlen2` and `subdef` (the English logo), `system.sbin`'s font, frame tiles and
    /// cursor, and `message_e.str`. `Err` names what is missing or does not parse.
    // shortcut: English only: the game picks `subdef<lang>`, `S02_<lang>` and `message_<lang>`
    // by `GetLanguage()`, which the port has no setting for.
    pub fn from_rom(rom: &[u8]) -> Result<Self, String> {
        let header = Header::parse(rom).map_err(|e| e.to_string())?;
        let fs = Filesystem::parse(rom, &header).map_err(|e| e.to_string())?;
        let file = |name: &str| -> Result<&[u8], String> {
            let id = fs
                .get(name)
                .ok_or_else(|| format!("{name} is not in the ROM"))?
                .id;
            fs.data(rom, id).map_err(|e| e.to_string())
        };
        let err = |e: rescue_formats::read::Error| e.to_string();

        let ground_file = file("ground.sbin")?;
        let arc = Archive::parse(ground_file).map_err(err)?;
        let part = |name: &str| arc.get(ground_file, name).map_err(err);
        let bpl = ground::parse_bpl(part("S02")?).map_err(err)?;
        let bpc = ground::parse_bpc(part("S02c")?).map_err(err)?;
        let bma = ground::parse_bma(part("S02m")?).map_err(err)?;
        let no_bpa = [None, None, None, None];
        // Layer 0 over layer 1: the first layer is in front at equal priority.
        let logo = (0..bma.layers.len().min(2))
            .map(|l| ground_layer(&bpc, &no_bpa, &bma, l).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;

        let menu = file("titlemenu.sbin")?;
        let marc = Archive::parse(menu).map_err(err)?;
        let screen = |name: &str| -> Result<Picture, String> {
            let art = titlemenu::parse(marc.get(menu, name).map_err(err)?).map_err(err)?;
            titlemenu::check_tiles(&art).map_err(err)?;
            let palette =
                titlemenu::parse_palette(marc.get(menu, &format!("{name}p")).map_err(err)?)
                    .map_err(err)?;
            Ok(Picture { art, palette })
        };
        let backdrops = (0..BACKDROPS)
            .map(|k| screen(&format!("titlen{k}")))
            .collect::<Result<Vec<_>, _>>()?;
        let subdef = screen("subdef")?;

        let sys = file("system.sbin")?;
        let sarc = Archive::parse(sys).map_err(err)?;
        let sget = |name: &str| sarc.get(sys, name).map_err(err);
        let frames = TileSet::new(sget("font")?, sget("fontpal")?).map_err(err)?;
        let sprites = TileSet::new(sget("fontsp")?, sget("fontsppa")?).map_err(err)?;
        let font = Glyphs::new(parse_font(sget("kanji_a")?).map_err(err)?);

        let strings = text::parse_str(file("message_e.str")?).map_err(err)?;
        let string = |id: u32| -> Result<&[u8], String> {
            strings
                .get(id as usize - 1)
                .copied()
                .ok_or_else(|| format!("message {id} is not in message_e.str"))
        };
        let texts = ITEMS
            .iter()
            .map(|item| {
                let (name, desc) = text_ids(*item);
                // A description that is not in the table is left out; the name must be there.
                let desc: Vec<&[u8]> = desc.iter().filter_map(|id| string(*id).ok()).collect();
                Ok((
                    render_lines(&font, &[string(name)?], ITEM_CANVAS),
                    render_lines(&font, &desc, DESC_CANVAS),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(TitleArt {
            logo,
            logo_palettes: bpl.palettes,
            backdrops,
            subdef,
            frames,
            sprites,
            glyphs: font,
            texts,
        })
    }

    /// The logo screen's lower screen: `S02`, scrolled to the middle of its 312 pixels.
    // shortcut: the camera is a centred, top-aligned window; the game's scroll is not read.
    pub fn logo_frame(&self) -> Frame {
        let layers: Vec<BgLayer> = self
            .logo
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, mut l)| {
                l.scroll_x = ((l.cols as i32 * 8) - SCREEN_WIDTH as i32).max(0) / 2;
                l.priority = i as u8;
                l
            })
            .collect();
        compose(&layers, &self.logo_palettes, [0, 0, 0, 255])
    }

    /// The black upper screen of the logo screen.
    pub fn black() -> Frame {
        Frame::filled([0, 0, 0, 255])
    }

    /// The menu screen's upper screen: the `subdef` logo at fade `level`.
    pub fn menu_top(&self, level: u32) -> Frame {
        self.subdef.render(level)
    }

    /// The menu screen's lower screen.
    pub fn menu_bottom(&self, v: &MenuView<'_>) -> Frame {
        let Some(backdrop) = self.backdrops.get(v.backdrop) else {
            return Self::black();
        };
        let mut frame = backdrop.render(v.fade);
        if !v.windows {
            return frame;
        }
        let entries = v.items.len().max(1) as u32;
        let items = TileRect {
            rows: (entries * ROW_PITCH as u32 + 16).div_ceil(8),
            ..ITEM_WINDOW
        };
        for rect in [items, DESC_WINDOW] {
            draw_fill(&mut frame, &self.frames, rect);
            draw_menu_frame(&mut frame, &self.frames, rect);
        }
        for (i, item) in v.items.iter().enumerate() {
            if let Some((name, _)) = self.text_of(*item) {
                let y = ITEM_TEXT.1 + ROW_PITCH * i as i32;
                blit_canvas(&mut frame, name, (ITEM_TEXT.0, y), self.frames.palette());
            }
        }
        if let Some((_, desc)) = v.items.get(v.cursor).and_then(|i| self.text_of(*i)) {
            blit_canvas(&mut frame, desc, DESC_TEXT, self.frames.palette());
        }
        if (v.blink / BLINK_FRAMES).is_multiple_of(2) {
            let y = CURSOR_AT.1 + ROW_PITCH * v.cursor as i32;
            draw_cursor(&mut frame, &self.sprites, CURSOR_AT.0, y);
        }
        frame
    }

    /// The lower screen of a message screen: the backdrop, a window, the lines and the choices.
    pub fn panel_bottom(&self, v: &PanelView<'_>) -> Frame {
        let Some(backdrop) = self.backdrops.get(v.backdrop) else {
            return Self::black();
        };
        let mut frame = backdrop.render(FADE_FRAMES);
        draw_fill(&mut frame, &self.frames, PANEL_WINDOW);
        draw_menu_frame(&mut frame, &self.frames, PANEL_WINDOW);
        let palette = self.frames.palette();
        let mut y = PANEL_TEXT.1;
        for line in v.lines {
            let c = render_lines(&self.glyphs, &[line.as_bytes()], PANEL_CANVAS);
            blit_canvas(&mut frame, &c, (PANEL_TEXT.0, y), palette);
            y += ROW_PITCH;
        }
        y += ROW_PITCH;
        for (i, choice) in v.choices.iter().enumerate() {
            let c = render_lines(&self.glyphs, &[choice.as_bytes()], PANEL_CANVAS);
            blit_canvas(&mut frame, &c, (PANEL_TEXT.0 + 12, y), palette);
            if i == v.cursor {
                draw_cursor(&mut frame, &self.sprites, PANEL_TEXT.0, y + 2);
            }
            y += ROW_PITCH;
        }
        frame
    }

    fn text_of(&self, item: MenuItem) -> Option<&(Canvas, Canvas)> {
        ITEMS
            .iter()
            .position(|i| *i == item)
            .and_then(|i| self.texts.get(i))
    }
}

/// Draws a text canvas with its top-left at `at`; index 0 is transparent.
fn blit_canvas(frame: &mut Frame, canvas: &Canvas, at: (i32, i32), palette: &Palette) {
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let index = usize::from(canvas.get(x, y));
            let (px, py) = (at.0 + x as i32, at.1 + y as i32);
            if index == 0
                || !(0..SCREEN_WIDTH as i32).contains(&px)
                || !(0..SCREEN_HEIGHT as i32).contains(&py)
            {
                continue;
            }
            if let Some(c) = palette.get(index) {
                let i = (py as usize * SCREEN_WIDTH as usize + px as usize) * 4;
                frame.pixels[i..i + 4].copy_from_slice(c);
            }
        }
    }
}

/// Where the fade of the menu screen is. One step per logic tick, as in the game's loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Counter 0 to 32.
    In,
    /// Fully up: input is read.
    Up,
    /// Counter 32 down to 0.
    Out,
}

/// The menu screen's timing: the fade counter and the cursor blink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuClock {
    pub phase: Phase,
    pub counter: u32,
    /// Frames since the menu came up, for the blink.
    pub blink: u32,
}

impl MenuClock {
    /// A menu that starts to fade in.
    pub fn new() -> Self {
        MenuClock {
            phase: Phase::In,
            counter: 0,
            blink: 0,
        }
    }

    /// Starts the fade out.
    pub fn leave(&mut self) {
        self.phase = Phase::Out;
    }

    /// Advances one frame. `true` on the frame the fade out ends.
    pub fn tick(&mut self) -> bool {
        match self.phase {
            Phase::In => {
                self.counter += 1;
                if self.counter >= FADE_FRAMES {
                    self.phase = Phase::Up;
                }
            }
            Phase::Up => self.blink += 1,
            Phase::Out => {
                self.counter = self.counter.saturating_sub(1);
                return self.counter == 0;
            }
        }
        false
    }

    /// Whether input is read this frame.
    pub fn ready(&self) -> bool {
        self.phase == Phase::Up
    }

    /// The windows are up once the fade in is done.
    pub fn windows(&self) -> bool {
        self.phase != Phase::In
    }
}

impl Default for MenuClock {
    fn default() -> Self {
        Self::new()
    }
}

/// The backdrop number from the global generator's 32-bit output, as `FUN_02011968(3)` does:
/// `((r & 0xFFFF) * 3) >> 16`.
pub fn backdrop_from(r: u32) -> usize {
    (((r & 0xFFFF) * BACKDROPS as u32) >> 16) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-tile-per-colour synthetic art: no ROM needed.
    fn synthetic() -> TitleArt {
        let tile = |i: u8| [i; 64];
        let screen = |tile_index: u16| {
            let mut map0 = vec![gfx::MapEntry::from_raw(0); 1024];
            let map1 = vec![gfx::MapEntry::from_raw(tile_index); 1024];
            map0[0] = gfx::MapEntry::from_raw(0);
            Picture {
                art: TitleScreen {
                    map0,
                    map1,
                    tiles: (0..4).map(tile).collect(),
                },
                palette: (0..224u32).map(|i| [(i + 32) as u8, 200, 100, 0]).collect(),
            }
        };
        let ui_tiles = |n: u32, px: u8| {
            let mut sheet = n.to_le_bytes().to_vec();
            sheet.extend((0..n * 32).map(|_| px));
            sheet
        };
        let pal: Vec<u8> = (0..16u8)
            .flat_map(|i| [i * 16, i * 16, i * 16, 0x80])
            .collect();
        let frames = TileSet::new(&ui_tiles(0x100, 0x11), &pal).expect("tiles");
        let sprites = TileSet::new(&ui_tiles(8, 0x22), &pal).expect("tiles");
        let text = |w: usize, h: usize| {
            let mut c = Canvas::new(w, h);
            c.pixels[0] = 7;
            c
        };
        TitleArt {
            logo: vec![],
            logo_palettes: vec![],
            backdrops: (0..BACKDROPS).map(|_| screen(1)).collect(),
            subdef: screen(2),
            frames,
            sprites,
            glyphs: Glyphs::new(vec![]),
            texts: ITEMS
                .iter()
                .map(|_| (text(96, 12), text(216, 24)))
                .collect(),
        }
    }

    fn view(fade: u32, windows: bool, blink: u32) -> MenuView<'static> {
        MenuView {
            backdrop: 1,
            fade,
            windows,
            items: &MenuItem::ALL,
            cursor: 0,
            blink,
        }
    }

    #[test]
    fn backdrop_is_the_scaled_low_half() {
        assert_eq!(backdrop_from(0), 0);
        assert_eq!(backdrop_from(0x5555), 0);
        assert_eq!(backdrop_from(0x5556), 1);
        assert_eq!(backdrop_from(0xAAAB), 2);
        assert_eq!(backdrop_from(0xFFFF_FFFF), 2);
        assert_eq!(backdrop_from(0x1_0000), 0, "only the low half counts");
    }

    #[test]
    fn the_clock_fades_in_for_32_frames_then_blinks_then_fades_out() {
        let mut c = MenuClock::new();
        assert!(!c.ready() && !c.windows());
        for i in 1..=FADE_FRAMES {
            assert!(!c.tick());
            assert_eq!(c.counter, i);
        }
        assert!(c.ready() && c.windows());
        c.tick();
        assert_eq!(c.blink, 1);
        c.leave();
        assert!(!c.ready() && c.windows());
        for _ in 0..FADE_FRAMES - 1 {
            assert!(!c.tick());
        }
        assert!(c.tick(), "the 32nd step reaches 0");
        assert_eq!(c.counter, 0);
    }

    #[test]
    fn the_fade_follows_the_formula_and_holds_at_31() {
        let art = synthetic();
        let at = |n| art.menu_top(n).get(0, 0);
        assert_eq!(at(0), [0, 0, 0, 255], "counter 0 is black");
        let want = |n| gfx::bgr555(titlemenu::fade_color([34, 200, 100], n));
        assert_eq!(at(10), want(10));
        assert_eq!(at(31), want(31));
        assert_eq!(at(32), want(31), "the level is held at 31");
    }

    #[test]
    fn windows_and_cursor_show_after_the_fade_in_and_the_cursor_blinks() {
        let art = synthetic();
        let bare = art.menu_bottom(&view(32, false, 0));
        let on = art.menu_bottom(&view(32, true, 0));
        let off = art.menu_bottom(&view(32, true, BLINK_FRAMES));
        assert_ne!(bare, on, "the windows are drawn");
        assert_ne!(on, off, "the cursor blinks");
        assert_eq!(
            art.menu_bottom(&view(32, true, 2 * BLINK_FRAMES)),
            on,
            "and comes back"
        );
        // Inside the item window, the fill replaces the backdrop; outside it nothing changes.
        assert_ne!(on.get(40, 24), bare.get(40, 24));
        assert_eq!(on.get(250, 100), bare.get(250, 100));
    }

    #[test]
    fn a_longer_menu_has_a_taller_window() {
        let art = synthetic();
        let mut v = view(32, true, 0);
        let short = art.menu_bottom(&v);
        v.items = MenuItem::list(true);
        let tall = art.menu_bottom(&v);
        // Row 8 (y 64 to 71) is inside the three-item window only.
        assert_eq!(
            short.get(60, 66),
            art.menu_bottom(&view(32, false, 0)).get(60, 66)
        );
        assert_ne!(tall.get(60, 66), short.get(60, 66));
    }

    #[test]
    fn a_panel_draws_a_window_and_the_choice_cursor() {
        let art = synthetic();
        let lines = [String::from("Delete?")];
        let panel = |cursor| {
            art.panel_bottom(&PanelView {
                backdrop: 1,
                lines: &lines,
                choices: &["No", "Yes"],
                cursor,
            })
        };
        let bare = art.menu_bottom(&view(32, false, 0));
        assert_ne!(panel(0), bare, "the window is drawn");
        assert_ne!(panel(0), panel(1), "the cursor moves between the choices");
        assert_eq!(panel(0).get(250, 100), bare.get(250, 100));
    }

    rescue_tests::rom_test! {
        /// The title assets load from the dump and the screens draw: the logo is not blank,
        /// each backdrop differs, the faded-out menu is black and the up menu is not.
        fn title_assets_load_and_draw(rom) {
            let art = TitleArt::from_rom(&rom.read()).expect("the title art loads");
            let logo = art.logo_frame();
            let blank = Frame::filled([0, 0, 0, 255]);
            assert_ne!(logo, blank, "S02 draws");
            let mut seen = Vec::new();
            for k in 0..BACKDROPS {
                let up = art.menu_bottom(&MenuView { backdrop: k, ..view(32, true, 0) });
                assert_ne!(up, blank, "backdrop {k}");
                assert_eq!(
                    art.menu_bottom(&MenuView { backdrop: k, ..view(0, false, 0) }),
                    blank,
                    "faded out"
                );
                assert!(!seen.contains(&up), "backdrop {k} is its own picture");
                seen.push(up);
            }
            assert_ne!(art.menu_top(32), blank, "the subdef logo draws");
            let with_text = art.menu_bottom(&view(32, true, 0));
            let without = art.menu_bottom(&MenuView { items: &[], ..view(32, true, 0) });
            assert_ne!(with_text, without);
            if let Some(dir) = std::env::var_os("RESCUE_TITLE_DUMP") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).expect("dump dir");
                let ppm = |name: &str, f: &Frame| {
                    let mut out = format!("P6\n{SCREEN_WIDTH} {SCREEN_HEIGHT}\n255\n").into_bytes();
                    out.extend(f.pixels.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
                    std::fs::write(dir.join(name), out).expect("write");
                };
                ppm("logo.ppm", &logo);
                ppm("menu_bottom.ppm", &with_text);
                ppm("menu_top.ppm", &art.menu_top(32));
            }
        }
    }
}
