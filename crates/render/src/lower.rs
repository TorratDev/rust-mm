//! The lower-screen dungeon chrome (engine A): the status line in BG0 row 0, its HP bar, the
//! touch-button panel in rows 22 and 23, and the message log box (BG0 frame and text over a
//! BG1 fill that shows through WIN0 and blends with what is below it).
//!
//! Layout, tile numbers and blend are from `docs/re/ui-screens.md`: the BG0 layout (section
//! 2), the frame tiles (section 3), the WIN0 and alpha log box (section 6) and the status line
//! and HP bar (the status-line section). Each map entry below was checked against the dng1
//! capture's BG0 map, cell by cell, in the ROM-gated test.
//!
//! Not drawn: the minimap (BG0 rows 1 to 16). Its cell rules depend on game predicates the
//! notes do not yet record (`FUN_0200c5dc`, `FUN_0206db90`), so it is a separate step.
//!
//! `[hypothesis]` in the code: the non-basement floor letter (column 1), the status
//! flag's meaning (it selects the white digits and the green bar), and the 999 clamp on HP.

use rescue_formats::gfx::{Rgba, TILE_PIXELS};
use rescue_formats::graphics::Palette;
use rescue_formats::read::{Error, Result};

use crate::bg::Frame;
use crate::screens::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::text::Canvas;
use crate::ui::{
    BOX_EDGE_BOTTOM, BOX_EDGE_TOP, BOX_FILL_X, BOX_FILL_Y, DIALOGUE_BOX, DIALOGUE_CORNER,
    DIALOGUE_SIDE, FILL_TILE, FONT_BG_TILE, FRAME_PALETTE, TEXT_ORIGIN, TileSet, frame_cells,
};

/// BG palette bank of every chrome tile (`docs/re/ui-screens.md`, section 2).
const PALETTE: u16 = (FRAME_PALETTE as u16) << 12;
/// Map entry of an empty cell (tile 0, transparent in the capture).
const EMPTY: u16 = 0;
/// Font tiles of the status line (BG tile numbers, `docs/re/ui-screens.md`).
const TILE_BLANK: u16 = 0x279;
const TILE_PAD: u16 = 0x278;
const TILE_B: u16 = 0x2BE;
const TILE_F: u16 = 0x2B8;
const TILE_L: u16 = 0x2B9;
const TILE_V: u16 = 0x2BA;
const TILE_H: u16 = 0x2BB;
const TILE_P: u16 = 0x2BC;
const TILE_SLASH: u16 = 0x2BD;
/// Digits: `0x288 + d` (white) or `0x2BF + d` (alternate colour).
const DIGIT_WHITE: u16 = 0x288;
const DIGIT_ALT: u16 = 0x2BF;
/// HP bar: 12 generated tiles at BG tile `0x2CC`, in cols 18 to 29.
const BAR_FIRST: u16 = 0x2CC;
pub const BAR_TILES: usize = 12;
/// Touch-button panel tiles (`panelnn`, BG tiles from `0x300`) and the file's header size.
const PANEL_FIRST: u16 = 0x300;
const PANEL_TILES: usize = 50;
const PANEL_HEADER: usize = 20;
/// Rows of the touch-button panel.
const PANEL_ROWS: [u32; 2] = [22, 23];

/// The status line's inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    /// Basement floor: column 1 shows `B`.
    pub basement: bool,
    pub floor: u32,
    pub level: u32,
    pub hp: u32,
    pub max_hp: u32,
    /// The state flag (`+0x2e`). Its meaning is `[hypothesis]`; here, set means the white
    /// digits and the green bar, clear the alternate colours (the capture has it set).
    pub flag: bool,
}

/// The decoded tiles the lower screen needs: the font sheet (BG palette 15), and the
/// touch-button panel tiles from `panelnn`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LowerTiles {
    pub font: TileSet,
    pub panel: Vec<[u8; TILE_PIXELS]>,
}

impl LowerTiles {
    /// `font` and `fontpal` are the `system.sbin` `font` sheet and `fontpal`; `panelnn` is
    /// the `panelnn` file: a 20-byte header, then 50 4bpp tiles.
    pub fn new(font: &[u8], fontpal: &[u8], panelnn: &[u8]) -> Result<Self> {
        let font = TileSet::new(font, fontpal)?;
        let body = panelnn
            .get(PANEL_HEADER..PANEL_HEADER + PANEL_TILES * 32)
            .ok_or_else(|| Error::invalid("panelnn", "shorter than its 50 tiles"))?;
        Ok(Self {
            font,
            panel: body.chunks_exact(32).map(decode_4bpp).collect(),
        })
    }

    /// The BG palette-15 colours (`fontpal` bank 0).
    pub fn palette(&self) -> &Palette {
        self.font.palette()
    }
}

/// Decodes one 4bpp tile, low nibble left, to palette indices.
fn decode_4bpp(bytes: &[u8]) -> [u8; TILE_PIXELS] {
    let mut out = [0u8; TILE_PIXELS];
    for (i, &b) in bytes.iter().take(32).enumerate() {
        out[i * 2] = b & 15;
        out[i * 2 + 1] = b >> 4;
    }
    out
}

/// Decimal digits of `n`, most significant first (`0` is one digit).
fn digits(n: u32) -> Vec<u8> {
    n.to_string().bytes().map(|b| b - b'0').collect()
}

fn digit(d: u8, white: bool) -> u16 {
    PALETTE
        | if white {
            DIGIT_WHITE + u16::from(d)
        } else {
            DIGIT_ALT + u16::from(d)
        }
}

/// The 32 BG0 row-0 map entries of the status line (`docs/re/ui-screens.md`, status line).
/// Column 0 blank, column 1 `B` on a basement, floor digits from column 2 and `F` after them,
/// `Lv` at columns 5 and 6, level digits from column 7, `HP` at 9 and 10, current HP
/// left-aligned from column 11 (padded to column 13), `/` at 14, maximum HP from 15 (padded to
/// column 17), and the 12 bar tiles in columns 18 to 29. Matches the dng1 capture.
pub fn status_row(s: &Status) -> [u16; 32] {
    let mut row = [EMPTY; 32];
    let mut put = |c: usize, v: u16| {
        if let Some(cell) = row.get_mut(c) {
            *cell = v;
        }
    };
    put(0, PALETTE | TILE_BLANK);
    // Non-basement floors: the capture shows only a basement, so the blank is
    // `[hypothesis]` (the notes name a language-dependent F variant there, not traced).
    put(1, PALETTE | if s.basement { TILE_B } else { TILE_PAD });
    let mut c = 2;
    for d in digits(s.floor.min(99)) {
        put(c, digit(d, s.flag));
        c += 1;
    }
    put(c, PALETTE | TILE_F);
    put(5, PALETTE | TILE_L);
    put(6, PALETTE | TILE_V);
    c = 7;
    for d in digits(s.level.min(99)) {
        put(c, digit(d, s.flag));
        c += 1;
    }
    put(9, PALETTE | TILE_H);
    put(10, PALETTE | TILE_P);
    c = 11;
    for d in digits(s.hp.min(999)) {
        put(c, digit(d, s.flag));
        c += 1;
    }
    while c <= 13 {
        put(c, PALETTE | TILE_PAD);
        c += 1;
    }
    put(14, PALETTE | TILE_SLASH);
    c = 15;
    for d in digits(s.max_hp.min(999)) {
        put(c, digit(d, s.flag));
        c += 1;
    }
    while c <= 17 {
        put(c, PALETTE | TILE_PAD);
        c += 1;
    }
    for i in 0..BAR_TILES {
        put(18 + i, PALETTE | (BAR_FIRST + i as u16));
    }
    row
}

/// The 12 HP bar tiles (palette indices), `[hypothesis]` for the scaling above 95 maximum HP
/// and for the colour meaning of `flag`; the rest is from `docs/re/ui-screens.md`.
///
/// Bar length is the maximum HP in pixels, capped at 96. Above a maximum of 95 the filled part
/// is scaled as `hp * 96 / max_hp`. Tiles shown are `ceil(max_hp / 8)`, at most 12; the rest
/// are empty. Rows 0 and 7 are empty, rows 1 and 6 are white (7) over the bar length, and rows
/// 2 to 5 are green (4, or cyan 5 when `flag` is clear) for the HP part and red (2) for the
/// missing part.
pub fn hp_bar(hp: u32, max_hp: u32, flag: bool) -> [[u8; TILE_PIXELS]; BAR_TILES] {
    let mut tiles = [[0u8; TILE_PIXELS]; BAR_TILES];
    if max_hp == 0 {
        return tiles;
    }
    let (bar_len, hp_px) = if max_hp <= 95 {
        (max_hp, hp.min(max_hp))
    } else {
        (
            96,
            (u64::from(hp.min(max_hp)) * 96 / u64::from(max_hp)) as u32,
        )
    };
    let shown = max_hp.div_ceil(8).min(BAR_TILES as u32) as usize;
    let fill = if flag { 4 } else { 5 };
    for (i, tile) in tiles.iter_mut().enumerate().take(shown) {
        for y in 0..8 {
            for x in 0..8 {
                let gx = (i * 8 + x) as u32;
                tile[y * 8 + x] = match y {
                    0 | 7 => 0,
                    1 | 6 => {
                        if gx < bar_len {
                            7
                        } else {
                            0
                        }
                    }
                    _ => {
                        if gx < hp_px {
                            fill
                        } else if gx < bar_len {
                            2
                        } else {
                            0
                        }
                    }
                };
            }
        }
    }
    tiles
}

/// The touch-button panel map entries: rows 22 and 23, five buttons of five tiles at columns
/// 1, 7, 13, 19 and 25 (`docs/re/ui-screens.md`, section 2). Button `k` uses tiles
/// `0x300 + 10k + dx` in row 22 and `+5` in row 23. Column 0 holds the blank font tile.
pub fn panel_cells() -> Vec<(u32, u32, u16)> {
    let mut cells = Vec::new();
    for row in PANEL_ROWS {
        cells.push((0, row, PALETTE | TILE_BLANK));
    }
    for k in 0..5u32 {
        for dx in 0..5u32 {
            let base = PANEL_FIRST + 10 * k as u16 + dx as u16;
            let col = 1 + 6 * k + dx;
            cells.push((col, PANEL_ROWS[0], PALETTE | base));
            cells.push((col, PANEL_ROWS[1], PALETTE | (base + 5)));
        }
    }
    cells
}

/// Blends the fill colour over the pixel below it: `(fill * 8 + below * 8) / 16` per channel,
/// the alpha of `BLDALPHA` 8/8 (`docs/re/ui-screens.md`, section 6).
pub fn blend_alpha_8(fill: Rgba, below: Rgba) -> Rgba {
    let mix = |a: u8, b: u8| ((u16::from(a) * 8 + u16::from(b) * 8) / 16) as u8;
    [
        mix(fill[0], below[0]),
        mix(fill[1], below[1]),
        mix(fill[2], below[2]),
        255,
    ]
}

fn put(frame: &mut Frame, x: i32, y: i32, c: Rgba) {
    if (0..SCREEN_WIDTH as i32).contains(&x) && (0..SCREEN_HEIGHT as i32).contains(&y) {
        let i = (y as usize * SCREEN_WIDTH as usize + x as usize) * 4;
        frame.pixels[i..i + 4].copy_from_slice(&c);
    }
}

fn get(frame: &Frame, x: i32, y: i32) -> Rgba {
    frame.get(x as u32, y as u32)
}

/// Draws an 8x8 tile with its palette, index 0 transparent.
fn blit(frame: &mut Frame, x: i32, y: i32, tile: &[u8; TILE_PIXELS], palette: &Palette) {
    for ty in 0..8 {
        for tx in 0..8 {
            let index = usize::from(tile[ty * 8 + tx]);
            if index == 0 {
                continue;
            }
            if let Some(c) = palette.get(index) {
                put(frame, x + tx as i32, y + ty as i32, *c);
            }
        }
    }
}

/// The tile a BG0 entry shows: the bar, the font sheet, or the panel. `None` for empty
/// entries and unknown numbers.
fn tile_for<'a>(
    tiles: &'a LowerTiles,
    bar: &'a [[u8; TILE_PIXELS]; BAR_TILES],
    entry: u16,
) -> Option<&'a [u8; TILE_PIXELS]> {
    let n = entry & 0x3FF;
    if n == 0 {
        None
    } else if (BAR_FIRST..BAR_FIRST + BAR_TILES as u16).contains(&n) {
        bar.get(usize::from(n - BAR_FIRST))
    } else if (PANEL_FIRST..PANEL_FIRST + PANEL_TILES as u16).contains(&n) {
        tiles.panel.get(usize::from(n - PANEL_FIRST))
    } else if n >= FONT_BG_TILE as u16 {
        tiles.font.tile(n - FONT_BG_TILE as u16)
    } else {
        None
    }
}

/// Draws the status line and the HP bar in BG0 row 0.
pub fn draw_status(frame: &mut Frame, tiles: &LowerTiles, s: &Status) {
    let bar = hp_bar(s.hp, s.max_hp, s.flag);
    for (col, &entry) in status_row(s).iter().enumerate() {
        if let Some(tile) = tile_for(tiles, &bar, entry) {
            blit(frame, col as i32 * 8, 0, tile, tiles.palette());
        }
    }
}

/// Draws the touch-button panel in rows 22 and 23.
pub fn draw_panel(frame: &mut Frame, tiles: &LowerTiles) {
    let bar = [[0u8; TILE_PIXELS]; BAR_TILES];
    for (col, row, entry) in panel_cells() {
        if let Some(tile) = tile_for(tiles, &bar, entry) {
            blit(frame, col as i32 * 8, row as i32 * 8, tile, tiles.palette());
        }
    }
}

/// Draws the message log box: the fill through WIN0 (blended over what is below), the top and
/// bottom edge rows, the text canvas, then the corners and sides. The box is the dialogue
/// box's frame and canvas, but its fill is translucent here.
pub fn draw_log_box(frame: &mut Frame, tiles: &LowerTiles, canvas: &Canvas) {
    if let Some(fill) = tiles.font.tile(FILL_TILE) {
        let (x0, y0) = (DIALOGUE_BOX.col as i32 * 8, DIALOGUE_BOX.row as i32 * 8);
        for y in y0..y0 + DIALOGUE_BOX.rows as i32 * 8 {
            for x in x0..x0 + DIALOGUE_BOX.cols as i32 * 8 {
                if !BOX_FILL_X.contains(&x) || !BOX_FILL_Y.contains(&y) {
                    continue;
                }
                let index = usize::from(fill[((y - y0) % 8 * 8 + (x - x0) % 8) as usize]);
                let Some(&colour) = tiles.palette().get(index) else {
                    continue;
                };
                let below = get(frame, x, y);
                put(frame, x, y, blend_alpha_8(colour, below));
            }
        }
    }
    let edge_rows = BOX_EDGE_TOP
        .iter()
        .enumerate()
        .map(|(k, &index)| (k, index))
        .chain(
            BOX_EDGE_BOTTOM
                .iter()
                .enumerate()
                .map(|(k, &index)| (canvas.height - 3 + k, index)),
        );
    for (row, index) in edge_rows {
        if let Some(&c) = tiles.palette().get(usize::from(index)) {
            for x in 0..canvas.width {
                put(
                    frame,
                    TEXT_ORIGIN.0 + x as i32,
                    TEXT_ORIGIN.1 + row as i32,
                    c,
                );
            }
        }
    }
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let index = canvas.get(x, y);
            if index == 0 {
                continue;
            }
            if let Some(&c) = tiles.palette().get(usize::from(index)) {
                put(frame, TEXT_ORIGIN.0 + x as i32, TEXT_ORIGIN.1 + y as i32, c);
            }
        }
    }
    for cell in frame_cells(DIALOGUE_BOX, DIALOGUE_CORNER, None, DIALOGUE_SIDE) {
        if let Some(t) = tiles.font.tile(cell.tile) {
            let (x, y) = (cell.col as i32 * 8, cell.row as i32 * 8);
            blit_flipped(frame, x, y, t, tiles.palette(), cell.hflip, cell.vflip);
        }
    }
}

fn blit_flipped(
    frame: &mut Frame,
    x: i32,
    y: i32,
    tile: &[u8; TILE_PIXELS],
    palette: &Palette,
    hflip: bool,
    vflip: bool,
) {
    for ty in 0..8 {
        for tx in 0..8 {
            let sx = if hflip { 7 - tx } else { tx };
            let sy = if vflip { 7 - ty } else { ty };
            let index = usize::from(tile[sy * 8 + sx]);
            if index == 0 {
                continue;
            }
            if let Some(&c) = palette.get(index) {
                put(frame, x + tx as i32, y + ty as i32, c);
            }
        }
    }
}

/// Draws the whole lower dungeon screen: the status line and bar, the touch-button panel and
/// the message log box. The log box blends with the pixels already in `frame`, which hold the
/// floor below it.
pub fn draw_lower(frame: &mut Frame, tiles: &LowerTiles, status: &Status, canvas: &Canvas) {
    draw_status(frame, tiles, status);
    draw_panel(frame, tiles);
    draw_log_box(frame, tiles, canvas);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dng1 capture's BG0 row 0 map entries, columns 0 to 31 (`docs/re/ui-screens.md`).
    const DNG1_ROW0: [u16; 32] = [
        0xf279, 0xf2be, 0xf289, 0xf2b8, 0x0000, 0xf2b9, 0xf2ba, 0xf290, 0x0000, 0xf2bb, 0xf2bc,
        0xf28b, 0xf28c, 0xf278, 0xf2bd, 0xf28b, 0xf28c, 0xf278, 0xf2cc, 0xf2cd, 0xf2ce, 0xf2cf,
        0xf2d0, 0xf2d1, 0xf2d2, 0xf2d3, 0xf2d4, 0xf2d5, 0xf2d6, 0xf2d7, 0x0000, 0x0000,
    ];

    fn dng1() -> Status {
        Status {
            basement: true,
            floor: 1,
            level: 8,
            hp: 34,
            max_hp: 34,
            flag: true,
        }
    }

    #[test]
    fn status_row_matches_the_dng1_map() {
        assert_eq!(status_row(&dng1()), DNG1_ROW0);
    }

    #[test]
    fn two_digit_floor_and_hp_shift_the_columns() {
        let s = Status {
            floor: 12,
            hp: 100,
            max_hp: 120,
            ..dng1()
        };
        let row = status_row(&s);
        // Floor 12: digits at 2 and 3, F at 4, Lv still at 5.
        assert_eq!(row[2], PALETTE | (DIGIT_WHITE + 1));
        assert_eq!(row[3], PALETTE | (DIGIT_WHITE + 2));
        assert_eq!(row[4], PALETTE | TILE_F);
        assert_eq!(row[5], PALETTE | TILE_L);
        // HP 100 fills 11 to 13 with no pad; '/' at 14; 120 at 15 to 17.
        assert_eq!(row[13], PALETTE | DIGIT_WHITE);
        assert_eq!(row[14], PALETTE | TILE_SLASH);
        assert_eq!(row[17], PALETTE | DIGIT_WHITE);
    }

    #[test]
    fn alternate_colour_when_the_flag_is_clear() {
        let row = status_row(&Status {
            flag: false,
            ..dng1()
        });
        assert_eq!(row[2], PALETTE | (DIGIT_ALT + 1));
    }

    #[test]
    fn hp_bar_of_34_of_34_matches_the_capture() {
        // Tile 0 rows 1 and 6 white, rows 2 to 5 green, rows 0 and 7 empty; tile 4 has two
        // green-and-white pixels (34 - 32), the rest empty.
        let bar = hp_bar(34, 34, true);
        assert_eq!(&bar[0][0..8], &[0; 8]);
        assert_eq!(&bar[0][8..16], &[7; 8]);
        assert_eq!(&bar[0][16..24], &[4; 8]);
        assert_eq!(&bar[4][8..10], &[7, 7]);
        assert_eq!(&bar[4][16..18], &[4, 4]);
        assert_eq!(&bar[4][18..24], &[0; 6]);
        // Tiles past the shown count are empty.
        assert!(bar[5].iter().all(|&p| p == 0));
    }

    #[test]
    fn hp_bar_shows_missing_hp_in_red() {
        let bar = hp_bar(16, 34, true);
        // Bar length 34 px, HP part 16 px: tile 1 (px 8 to 15) is green in its rows 2 to 5, tile
        // 2 (px 16 to 23) is red there, and its border row 1 is white over the bar length.
        assert_eq!(&bar[1][16..24], &[4; 8]);
        assert_eq!(&bar[2][16..24], &[2; 8]);
        assert_eq!(&bar[2][8..16], &[7; 8]);
        // Tile 4 (px 32 to 39): px 32 and 33 are red, the rest is past the bar.
        assert_eq!(&bar[4][16..24], &[2, 2, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn panel_matches_the_capture_layout() {
        let cells = panel_cells();
        let at = |c: u32, r: u32| cells.iter().find(|x| x.0 == c && x.1 == r).map(|x| x.2);
        assert_eq!(at(0, 22), Some(PALETTE | TILE_BLANK));
        assert_eq!(at(1, 22), Some(PALETTE | 0x300));
        assert_eq!(at(1, 23), Some(PALETTE | 0x305));
        assert_eq!(at(7, 22), Some(PALETTE | 0x30a));
        assert_eq!(at(25, 23), Some(PALETTE | 0x32d));
        // The gap column between buttons has no entry.
        assert_eq!(at(6, 22), None);
    }

    #[test]
    fn blend_is_the_average_at_8_over_16() {
        // The doc's check: BG1 blue over the floor pixel.
        assert_eq!(
            blend_alpha_8([32, 74, 106, 255], [164, 222, 0, 255]),
            [98, 148, 53, 255]
        );
    }
}
