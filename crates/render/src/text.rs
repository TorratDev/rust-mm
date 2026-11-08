//! Message text: layout, glyph drawing and the typewriter, as Blue's dialogue window does it.
//!
//! [`Dialogue`] plays a decoded message (`rescue_formats::text::Token`) into a 4bpp
//! [`Canvas`], the window's text bitmap (BG tiles in palette 15, [`CANVAS_TILES`] tiles).
//! It is a small state machine ticked once per presentation frame, never by the sim:
//! one glyph per frame, `#W` waits for a button on the same page, `#P` waits and then
//! scrolls the page up over [`SCROLL_FRAMES`] frames, `#p` waits and then clears, `#~`
//! waits frames, and fast-forward draws the rest of the page at once. Rules and tags:
//! `docs/re/text-engine.md` and `docs/formats/text.md`. The prompt arrow and the window
//! frame are not drawn here.
//!
//! Not modelled: `$` variables (the caller substitutes them), the `#[ ... ]` callback, the
//! descender shift (font selector 1) and wrapping (the game does not wrap).

use rescue_formats::font::{GLYPH, Glyph};
use rescue_formats::text::Token;

/// The text canvas of a dialogue window in tiles (BG0 cols 3 to 28, rows 17 to 21).
pub const CANVAS_TILES: (usize, usize) = (26, 5);
/// Its size in pixels.
pub const CANVAS_SIZE: (usize, usize) = (CANVAS_TILES.0 * 8, CANVAS_TILES.1 * 8);
/// Line pitch in pixels.
pub const LINE_PITCH: i32 = 11;
/// Pen position after a window opens or a page turns: x of a new line, y of the first line.
pub const LINE_START: (i32, i32) = (4, 4);
/// A line starting below this y is "page full" (lines are at y 4, 15, 26; 37 is full).
pub const PAGE_LAST_Y: i32 = 0x22;
/// Frames a page scroll takes.
pub const SCROLL_FRAMES: u32 = 18;
/// Pixels a page scrolls up: three lines. `[hypothesis]`: the distance is not measured.
pub const SCROLL_DISTANCE: i32 = 3 * LINE_PITCH;
/// Colour index of the outline and shadow of glyphs with font flag bit 1.
pub const OUTLINE_COLOUR: u8 = 3;
/// Colour index a window starts with (white).
pub const DEFAULT_COLOUR: u8 = 7;

/// A 4bpp bitmap as colour indices 0 to 15 (0 is transparent), row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    /// The canvas of a dialogue window.
    pub fn window() -> Self {
        Self::new(CANVAS_SIZE.0, CANVAS_SIZE.1)
    }

    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    /// ORs `v` into the pixel at (`x`, `y`); pixels outside the canvas are ignored.
    fn or(&mut self, x: i32, y: i32, v: u8) {
        if (0..self.width as i32).contains(&x) && (0..self.height as i32).contains(&y) {
            self.pixels[y as usize * self.width + x as usize] |= v;
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    /// The 8x8 tile at tile column `tx`, row `ty` as 32 bytes of 4bpp data (low nibble
    /// left), as written to BG VRAM.
    pub fn tile_bytes(&self, tx: usize, ty: usize) -> [u8; 32] {
        let mut out = [0u8; 32];
        for y in 0..8 {
            for x in 0..8 {
                let v = self.pixels[(ty * 8 + y) * self.width + tx * 8 + x];
                out[y * 4 + x / 2] |= v << (4 * (x % 2));
            }
        }
        out
    }
}

/// Palette index for a colour letter or digit after `#C` / `#c` (`FUN_02013cdc`).
pub fn text_colour(ch: char) -> u8 {
    match ch {
        '\0'..='\u{f}' => ch as u8,
        '0'..='?' => ch as u8 - b'0',
        'C' | 'D' | 'N' => 6,
        'E' | 'S' | 'T' => 15,
        'G' | 'M' => 5,
        'H' | 'I' => 4,
        'W' => 2,
        _ => DEFAULT_COLOUR,
    }
}

/// The dialogue font: glyphs sorted by character code.
#[derive(Debug, Clone)]
pub struct Font {
    glyphs: Vec<Glyph>,
}

impl Font {
    pub fn new(mut glyphs: Vec<Glyph>) -> Self {
        glyphs.sort_by_key(|g| g.code);
        Self { glyphs }
    }

    pub fn get(&self, code: u16) -> Option<&Glyph> {
        self.glyphs
            .binary_search_by_key(&code, |g| g.code)
            .ok()
            .map(|i| &self.glyphs[i])
    }
}

/// Pixel `p` of a glyph recoloured to `colour` (`FUN_01ff994c`): per nibble
/// `(p & 1) + (mask & p)` with `mask = colour - 1` (colour 0 uses 6).
fn recolour(p: u8, colour: u8) -> u8 {
    let mask = if colour == 0 { 6 } else { (colour - 1) & 15 };
    ((p & 1) + (mask & p)) & 15
}

/// Shadow mask of `FUN_01ff994c` (`DAT_01ff9f2c` = `0x33333333`, one nibble per pixel): a
/// shadow takes bits 0 and 1 of a neighbouring pixel, so a solid pixel (15) casts colour 3.
const SHADOW_MASK: u8 = 3;

/// Draws `glyph` with its top-left at (`x`, `y`) and returns the advance, as `FUN_01ff994c`
/// does (`01ff994c` in the rescue-arm9-r decompile).
///
/// Flag bit 0 set: the pixels are copied as they are, with no shadow. Otherwise each pixel
/// is recoloured (`(p & 1) + (mask & p)`). With flag bit 1 set, two shadow terms are ORed
/// into the same pixels: the neighbour to the left in the same row gives the right shadow,
/// direction (1, 0); the neighbour above-left, in the previous glyph row, gives the
/// below-right shadow, direction (1, 1). The first row has no previous row, and the last
/// row's below-right shadow falls outside the glyph. The right shadow of the last column
/// reaches column 12, one past the glyph. Both directions `[blue-verified]`.
///
/// Every value is ORed into the canvas, as the game does: nothing is overwritten, and a
/// blank canvas pixel is not special. Not modelled: the descender shift, and the advance's
/// spacing word (0 at run time).
pub fn draw_glyph(canvas: &mut Canvas, glyph: &Glyph, x: i32, y: i32, colour: u8) -> i32 {
    let raw = glyph.flags & 1 != 0;
    let shadow = glyph.flags & 2 != 0 && !raw;
    let px = |gx: i32, gy: i32| -> u8 {
        if (0..GLYPH as i32).contains(&gx) && (0..GLYPH as i32).contains(&gy) {
            glyph
                .pixels
                .get(gy as usize * GLYPH + gx as usize)
                .copied()
                .unwrap_or(0)
        } else {
            0
        }
    };
    for gy in 0..GLYPH as i32 {
        // Column GLYPH is blank: it only takes the right shadow of the last column.
        for gx in 0..=GLYPH as i32 {
            let v = px(gx, gy);
            let mut out = match (gx < GLYPH as i32, raw) {
                (false, _) => 0,
                (true, true) => v,
                (true, false) => recolour(v, colour),
            };
            if shadow {
                out |= px(gx - 1, gy) & !v & SHADOW_MASK;
                if gy > 0 {
                    out |= px(gx - 1, gy - 1) & !v & SHADOW_MASK;
                }
            }
            if out != 0 {
                canvas.or(x + gx, y + gy, out);
            }
        }
    }
    i32::from(glyph.width)
}

/// One step of a message after expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    Glyph(u16),
    Newline,
    Token(Token),
}

fn expand(tokens: &[Token]) -> Vec<Op> {
    let mut ops = Vec::new();
    for t in tokens {
        match t {
            Token::Text(s) => ops.extend(s.chars().map(|c| Op::Glyph(c as u32 as u16))),
            Token::Glyph(g) => ops.push(Op::Glyph(*g)),
            Token::Newline => ops.push(Op::Newline),
            // `#1` to `#9` are not codes: the game draws `#` and the digit.
            Token::Code(c) if ('1'..='9').contains(c) => {
                ops.extend([Op::Glyph(u16::from(b'#')), Op::Glyph(*c as u16)]);
            }
            other => ops.push(Op::Token(other.clone())),
        }
    }
    ops
}

/// What a dialogue is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Drawing glyphs.
    Typing,
    /// `#W`: the prompt arrow shows; a button continues on the same page.
    WaitButton,
    /// Page full or `#P` / `#p`: the arrow shows; a button scrolls (`scroll`) or clears.
    PageWait { scroll: bool },
    /// Scrolling up; `frame` of [`SCROLL_FRAMES`] done.
    Scrolling { frame: u32 },
    /// `#~`: frames left to wait.
    WaitFrames(u32),
    /// The last glyph is drawn.
    Done,
}

/// A message being typed into a canvas.
#[derive(Debug, Clone)]
pub struct Dialogue<'a> {
    ops: Vec<Op>,
    font: &'a Font,
    pos: usize,
    canvas: Canvas,
    /// The page before a scroll started.
    scroll_from: Option<Canvas>,
    pen: (i32, i32),
    saved_x: i32,
    colour: u8,
    saved: u8,
    alt_saved: u8,
    page_clear: bool,
    table: [u8; 256],
    state: State,
    /// Glyphs drawn per frame (1 when a window opens).
    pub glyphs_per_frame: usize,
}

impl<'a> Dialogue<'a> {
    pub fn new(tokens: &[Token], font: &'a Font) -> Self {
        let mut table = [0u8; 256];
        table[0x30 + 0x10] = 3; // the only non-zero entry seen at run time
        Self {
            ops: expand(tokens),
            font,
            pos: 0,
            canvas: Canvas::window(),
            scroll_from: None,
            pen: LINE_START,
            saved_x: 0,
            colour: DEFAULT_COLOUR,
            saved: DEFAULT_COLOUR,
            alt_saved: DEFAULT_COLOUR,
            page_clear: false,
            table,
            state: State::Typing,
            glyphs_per_frame: 1,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// Pen position in canvas pixels.
    pub fn pen(&self) -> (i32, i32) {
        self.pen
    }

    /// Width of the line starting at the op index `from`: glyph advances up to a line or
    /// page break.
    fn line_width(&self, from: usize) -> i32 {
        let mut w = 0;
        for op in &self.ops[from..] {
            match op {
                Op::Glyph(c) => w += self.font.get(*c).map_or(0, |g| i32::from(g.width)),
                Op::Newline => break,
                Op::Token(
                    Token::LineFeed
                    | Token::PageBreak
                    | Token::PageBreakFlagged
                    | Token::WaitButton,
                ) => break,
                Op::Token(_) => {}
            }
        }
        w
    }

    fn page_full(&mut self) {
        self.state = State::PageWait {
            scroll: !self.page_clear,
        };
    }

    /// Advances one presentation frame. `button` is a confirm press this frame; `fast` is
    /// the fast-forward input (draws the rest of the page in this frame).
    pub fn tick(&mut self, button: bool, fast: bool) {
        match self.state {
            State::Typing => self.type_glyphs(if fast {
                usize::MAX
            } else {
                self.glyphs_per_frame
            }),
            State::WaitButton if button => self.state = State::Typing,
            State::PageWait { scroll } if button => {
                if scroll {
                    self.scroll_from = Some(self.canvas.clone());
                    self.state = State::Scrolling { frame: 0 };
                } else {
                    self.turn_page();
                }
            }
            State::Scrolling { frame } => {
                let frame = frame + 1;
                if frame >= SCROLL_FRAMES {
                    self.scroll_from = None;
                    self.turn_page();
                } else {
                    let shift = (SCROLL_DISTANCE * frame as i32 / SCROLL_FRAMES as i32) as usize;
                    if let Some(src) = &self.scroll_from {
                        self.canvas.clear();
                        let w = src.width;
                        let rows = src.height - shift;
                        self.canvas.pixels[..rows * w].copy_from_slice(&src.pixels[shift * w..]);
                    }
                    self.state = State::Scrolling { frame };
                }
            }
            State::WaitFrames(n) => {
                self.state = if n <= 1 {
                    State::Typing
                } else {
                    State::WaitFrames(n - 1)
                };
            }
            _ => {}
        }
    }

    fn turn_page(&mut self) {
        self.canvas.clear();
        self.pen = LINE_START;
        self.page_clear = false;
        self.state = State::Typing;
    }

    fn type_glyphs(&mut self, budget: usize) {
        let mut budget = budget;
        while budget > 0 && self.state == State::Typing {
            let Some(op) = self.ops.get(self.pos).cloned() else {
                self.state = State::Done;
                return;
            };
            // A line that would start below the page is full: wait before drawing.
            if self.pen.1 > PAGE_LAST_Y && !matches!(op, Op::Token(_)) {
                self.page_full();
                return;
            }
            self.pos += 1;
            match op {
                Op::Glyph(code) => {
                    if let Some(g) = self.font.get(code) {
                        let adv =
                            draw_glyph(&mut self.canvas, g, self.pen.0, self.pen.1, self.colour);
                        self.pen.0 += adv;
                    }
                    budget -= 1;
                }
                Op::Newline => self.pen = (LINE_START.0, self.pen.1 + LINE_PITCH),
                Op::Token(t) => self.token(&t),
            }
        }
        if self.state == State::Typing && self.pos >= self.ops.len() {
            self.state = State::Done;
        }
    }

    fn token(&mut self, t: &Token) {
        match t {
            Token::Color(c) => {
                self.saved = self.colour;
                self.colour = text_colour(*c);
            }
            Token::ColorEnd => self.colour = self.saved,
            Token::AltColor(c) => {
                self.alt_saved = self.colour;
                self.colour = text_colour(*c);
            }
            Token::AltColorEnd => self.colour = self.alt_saved,
            Token::Shade(b) => {
                self.saved = self.colour;
                self.colour = self.table[(0x30 + usize::from(*b)) & 0xFF];
            }
            Token::SetTable { index, value } => {
                self.table[usize::from(index & 0x7F)] = value & 0x7F;
            }
            Token::SetX(b) => self.pen.0 = i32::from(*b),
            Token::SetY(b) => self.pen.1 = i32::from(*b),
            Token::AddX(b) => self.pen.0 += i32::from(*b),
            Token::SetXAbs(n) => self.pen.0 = i32::from(*n),
            Token::SaveX => self.saved_x = self.pen.0,
            Token::SavedX(b) => self.pen.0 = self.saved_x + i32::from(*b),
            Token::LineFeed => self.pen = (0, self.pen.1 + LINE_PITCH),
            Token::Centre => self.pen.0 = (CANVAS_SIZE.0 as i32 - self.line_width(self.pos)) / 2,
            Token::WaitButton => self.state = State::WaitButton,
            Token::PageBreak => {
                self.pen.1 = 0x270F;
                self.page_clear = false;
                self.page_full();
            }
            Token::PageBreakFlagged => {
                self.pen.1 = 0x270F;
                self.page_clear = true;
                self.page_full();
            }
            Token::WaitFrames(n) if *n > 0 => self.state = State::WaitFrames(u32::from(*n)),
            // Variables are the caller's, `#[ ]` has no callback, other codes are unknown.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font with a solid 4x12 bar for 'A' (width 5), a 2-pixel dot glyph 'B' (width 3,
    /// flags 0xFF02: outlined), and a pre-coloured 'C' (flags 0xFF01, value 9).
    fn font() -> Font {
        let solid = |f: &dyn Fn(usize, usize) -> u8| {
            (0..GLYPH * GLYPH)
                .map(|i| f(i % GLYPH, i / GLYPH))
                .collect::<Vec<_>>()
        };
        let g = |code, width, flags, pixels| Glyph {
            code,
            width,
            flags,
            pixels,
        };
        Font::new(vec![
            g(
                u16::from(b'A'),
                5,
                0xFF00,
                solid(&|x, _| if x < 4 { 15 } else { 0 }),
            ),
            g(
                u16::from(b'B'),
                3,
                0xFF02,
                solid(&|x, y| if x == 0 && y == 0 { 15 } else { 0 }),
            ),
            g(
                u16::from(b'C'),
                4,
                0xFF01,
                solid(&|x, y| if x == 0 && y == 0 { 9 } else { 0 }),
            ),
            g(u16::from(b' '), 3, 0xFF00, solid(&|_, _| 0)),
            g(
                u16::from(b'#'),
                6,
                0xFF00,
                solid(&|x, y| if x == 0 && y == 0 { 15 } else { 0 }),
            ),
            g(
                u16::from(b'1'),
                6,
                0xFF00,
                solid(&|x, y| if x == 1 && y == 0 { 15 } else { 0 }),
            ),
        ])
    }

    fn play(src: &str, f: &Font, frames: usize) -> Dialogue<'static> {
        // Leak the font for a 'static dialogue in tests.
        let f: &'static Font = Box::leak(Box::new(f.clone()));
        let mut d = Dialogue::new(&rescue_formats::text::decode(src.as_bytes()), f);
        for _ in 0..frames {
            d.tick(false, false);
        }
        d
    }

    #[test]
    fn colour_letters_and_digits() {
        assert_eq!(
            [
                'C', 'D', 'N', 'E', 'S', 'T', 'G', 'M', 'H', 'I', 'W', 'x', '4', '?', '\u{3}'
            ]
            .map(text_colour),
            [6, 6, 6, 15, 15, 15, 5, 5, 4, 4, 2, 7, 4, 15, 3]
        );
    }

    #[test]
    fn recolour_turns_15_into_the_colour() {
        assert_eq!([1u8, 2, 6, 15].map(|c| recolour(15, c)), [1, 2, 6, 15]);
        assert_eq!(recolour(0, 5), 0);
        assert_eq!(recolour(15, 0), 7);
    }

    #[test]
    fn one_glyph_per_frame_from_the_line_start() {
        let f = font();
        let d = play("AA", &f, 1);
        assert_eq!(d.pen(), (4 + 5, 4));
        assert_eq!(d.state(), State::Typing);
        let d = play("AA", &f, 2);
        assert_eq!(d.pen(), (4 + 10, 4));
        assert_eq!(d.canvas().get(4, 4), 7); // default white
        let d = play("AA", &f, 3);
        assert_eq!(d.state(), State::Done);
    }

    #[test]
    fn colour_codes_use_one_saved_slot_each() {
        let f = font();
        // #C2 A #R A: red then back to white.
        let d = play("#C2A#RA", &f, 10);
        assert_eq!((d.canvas().get(4, 4), d.canvas().get(9, 4)), (2, 7));
        // No stack: #C2 #C4 #R gives 2, not white.
        let d = play("#C2#C4#RA", &f, 10);
        assert_eq!(d.canvas().get(4, 4), 2);
        // #c has its own slot: #C2 #c4 #r gives 2 (what #c saved), #R then gives the default.
        let d = play("#C2#c4#rA#RA", &f, 10);
        assert_eq!((d.canvas().get(4, 4), d.canvas().get(9, 4)), (2, 7));
    }

    #[test]
    fn shade_table_and_set_table() {
        let f = font();
        // #_ with 0x10 reads the one preset entry (3); #S writes entry 0x30 + 1.
        let d = play("#_\u{10}A", &f, 4);
        assert_eq!(d.canvas().get(4, 4), 3);
        let d = play("#S1\u{5}#_\u{1}A", &f, 4);
        assert_eq!(d.canvas().get(4, 4), 5);
    }

    #[test]
    fn outline_and_raw_glyphs() {
        let f = font();
        let d = play("B", &f, 1);
        let c = d.canvas();
        assert_eq!(c.get(4, 4), 7);
        // Right and below-right, not directly below.
        assert_eq!((c.get(5, 4), c.get(4, 5), c.get(5, 5)), (3, 0, 3));
        assert_eq!(c.get(3, 4), 0);
        // The raw glyph keeps its own value whatever the colour.
        let d = play("#C2C", &f, 1);
        assert_eq!(d.canvas().get(4, 4), 9);
    }

    #[test]
    fn pen_moves() {
        let f = font();
        let pen = |s: &str| play(s, &f, 0).tap(|d| d.tick(false, true)).pen();
        assert_eq!(pen("#=\u{28}"), (0x28, 4));
        assert_eq!(pen("#y\u{20}"), (4, 0x20));
        assert_eq!(pen("#.\u{07}"), (11, 4));
        assert_eq!(pen("#>100."), (100, 4));
        assert_eq!(pen("A#:AA#;\u{02}"), (4 + 5 + 2, 4));
        assert_eq!(pen("A\nA"), (4 + 5, 4 + 11));
        assert_eq!(pen("A#nA"), (5, 4 + 11)); // #n goes to x = 0, not the line start
        // #+ centres the line: (208 - 5) / 2
        assert_eq!(pen("#+A"), ((208 - 5) / 2 + 5, 4));
    }

    trait Tap: Sized {
        fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
            f(&mut self);
            self
        }
    }
    impl Tap for Dialogue<'_> {}

    #[test]
    fn digits_after_hash_are_drawn_literally() {
        let f = font();
        let d = play("#1", &f, 2);
        // '#' dot at (4,4) and '1' dot at (4 + 6 + 1, 4)
        assert_eq!((d.canvas().get(4, 4), d.canvas().get(11, 4)), (7, 7));
        assert_eq!(d.state(), State::Done);
    }

    #[test]
    fn w_waits_for_a_button_on_the_same_page() {
        let f = font();
        let mut d = play("A#WA", &f, 3);
        assert_eq!(d.state(), State::WaitButton);
        assert_eq!(d.pen(), (9, 4));
        d.tick(false, false);
        assert_eq!(d.state(), State::WaitButton);
        d.tick(true, false);
        assert_eq!(d.state(), State::Typing);
        d.tick(false, false);
        assert_eq!(d.pen(), (14, 4));
        assert_eq!(d.canvas().get(4, 4), 7); // the first A is still there
    }

    #[test]
    fn page_p_scrolls_over_18_frames_and_lowercase_p_clears() {
        let f = font();
        let mut d = play("A#PA", &f, 3);
        assert_eq!(d.state(), State::PageWait { scroll: true });
        d.tick(true, false);
        assert_eq!(d.state(), State::Scrolling { frame: 0 });
        for i in 1..SCROLL_FRAMES {
            d.tick(false, false);
            assert_eq!(d.state(), State::Scrolling { frame: i });
        }
        // Halfway the glyph has moved up by 3 * 11 * 9 / 18 = 16 pixels: gone off the top.
        d.tick(false, false);
        assert_eq!(d.state(), State::Typing);
        assert!(d.canvas().pixels.iter().all(|&p| p == 0)); // page cleared, y = 4
        assert_eq!(d.pen(), LINE_START);
        d.tick(false, false);
        assert_eq!(d.canvas().get(4, 4), 7);
        // #p clears at once.
        let mut d = play("A#pA", &f, 3);
        assert_eq!(d.state(), State::PageWait { scroll: false });
        d.tick(true, false);
        assert_eq!(d.state(), State::Typing);
        assert!(d.canvas().pixels.iter().all(|&p| p == 0));
    }

    #[test]
    fn scrolling_moves_the_page_up_in_steps() {
        let f = font();
        let mut d = play("A#PA", &f, 3);
        d.tick(true, false);
        d.tick(false, false); // frame 1: shift 33 / 18 = 1
        assert_eq!(d.canvas().get(4, 3), 7);
        assert_eq!(d.canvas().get(4, 4), 7); // column of the bar is 12 tall
        d.tick(false, false); // frame 2: shift 3
        assert_eq!(d.canvas().get(4, 1), 7);
    }

    #[test]
    fn a_fourth_line_makes_the_page_full() {
        let f = font();
        let mut d = play("A\nA\nA\nA", &f, 0);
        d.tick(false, true);
        // Lines at y 4, 15, 26 are drawn; the one at y 37 waits.
        assert_eq!(d.pen(), (4, 37));
        assert_eq!(d.state(), State::PageWait { scroll: true });
        assert_eq!(d.canvas().get(4, 26), 7);
        assert_eq!(d.canvas().get(4, 37 - 11 + 11 - 1), 7);
    }

    #[test]
    fn tilde_waits_the_given_frames() {
        let f = font();
        let mut d = play("A#~\u{3}A", &f, 1);
        assert_eq!(d.state(), State::Typing);
        d.tick(false, false); // runs #~
        assert_eq!(d.state(), State::WaitFrames(3));
        d.tick(false, false);
        d.tick(false, false);
        assert_eq!(d.state(), State::WaitFrames(1));
        d.tick(false, false);
        assert_eq!(d.state(), State::Typing);
    }

    #[test]
    fn fast_forward_draws_the_page_in_one_frame_but_stops_at_waits() {
        let f = font();
        let mut d = play("AAAA#WAA", &f, 0);
        d.tick(false, true);
        assert_eq!(d.state(), State::WaitButton);
        assert_eq!(d.pen(), (4 + 20, 4));
        d.tick(true, false);
        d.tick(false, true);
        assert_eq!(d.state(), State::Done);
    }

    #[test]
    fn tiles_pack_low_nibble_left() {
        let f = font();
        let d = play("A", &f, 1);
        let t = d.canvas().tile_bytes(0, 0);
        // Canvas x 4..8 of row 4 are the bar: nibbles 4..8 of the row.
        assert_eq!(&t[4 * 4..4 * 4 + 4], &[0x00, 0x00, 0x77, 0x77]);
    }
}
