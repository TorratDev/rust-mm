//! Window frames, the dialogue box with its portrait, and the cursor sprites on the lower
//! screen. Every layout and tile role is copied from the RE notes: `docs/re/ui-screens.md`
//! (sections 2, 3, 5 and 8) and `docs/formats/portraits.md` (placement and the mirror
//! flag). Tile numbers here are font indices: the BG tile is [`FONT_BG_TILE`] plus the
//! index.
//!
//! Drawn: the window frame cells, the box fill, the box's top and bottom edge rows, the text
//! canvas, the portrait and its frame, the 16x8 wait marker and the 8x8 cursor. Not drawn:
//! the marker's blink (not recorded). Unverified details are marked `[hypothesis]` where
//! they are used.

use rescue_formats::font;
use rescue_formats::gfx::{Rgba, TILE_PIXELS};
use rescue_formats::graphics::{self, Palette};
use rescue_formats::portrait::{Portrait, SIZE as PORTRAIT_SIZE};
use rescue_formats::read::{Error, Result};

use crate::bg::Frame;
use crate::screens::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::text::Canvas;

/// BG tile of font index 0 (`docs/re/ui-screens.md`, section 3).
pub const FONT_BG_TILE: usize = 0x278;
/// Dialogue box corner, font index 31 (BG tile `0x297`).
pub const DIALOGUE_CORNER: u16 = 31;
/// Dialogue box side, font index 98 (BG tile `0x2DA`); the same tile as the menu side.
pub const DIALOGUE_SIDE: u16 = 98;
/// Box fill, font index 99 (BG tile `0x2DB`): solid colour index 1.
pub const FILL_TILE: u16 = 99;
/// Menu window corner, edge and side (font indices `0x60`, `0x61`, `0x62`).
pub const MENU_CORNER: u16 = 0x60;
pub const MENU_EDGE: u16 = 0x61;
pub const MENU_SIDE: u16 = 0x62;
/// Portrait frame corner, top or bottom edge, and side (font indices 100 to 102).
pub const PORTRAIT_CORNER: u16 = 100;
pub const PORTRAIT_EDGE: u16 = 101;
pub const PORTRAIT_SIDE: u16 = 102;
/// Wait marker: the 16x8 down arrow is font tiles 0 and 1 of `fontsp`.
pub const MARKER_TILE: u16 = 0;
/// Cursor: the 8x8 right-pointing cursor is `fontsp` tile 4.
pub const CURSOR_TILE: u16 = 4;
/// Text and frame tiles use BG palette bank 15; `fontpal` bank 0 is that bank.
pub const FRAME_PALETTE: usize = 15;
/// The portrait uses BG palette bank 14 (`docs/re/ui-screens.md`, section 4).
pub const PORTRAIT_PALETTE: usize = 14;
/// Pixel origin of the text canvas: BG0 col 3, row 17 (`docs/re/ui-screens.md`, section 2).
pub const TEXT_ORIGIN: (i32, i32) = (24, 136);
/// Colour indices of the box's top edge (canvas rows 0 to 2) and bottom edge (rows 37 to
/// 39): f, e, d and d, e, f (`docs/re/ui-screens.md`, section 2; the top edge is also in
/// `findings-b` item 2). The post_accept VRAM has these rows as solid colours across the canvas.
pub const BOX_EDGE_TOP: [u8; 3] = [15, 14, 13];
pub const BOX_EDGE_BOTTOM: [u8; 3] = [13, 14, 15];

/// A rectangle of 8x8 tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    pub col: u32,
    pub row: u32,
    pub cols: u32,
    pub rows: u32,
}

/// The dialogue box: BG0 cols 2 to 29, rows 17 to 21 (28 x 5 tiles).
pub const DIALOGUE_BOX: TileRect = TileRect {
    col: 2,
    row: 17,
    cols: 28,
    rows: 5,
};

/// One cell of a window frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCell {
    pub col: u32,
    pub row: u32,
    /// Font index.
    pub tile: u16,
    pub hflip: bool,
    pub vflip: bool,
}

/// The frame cells of `rect`: the four corners (top-right h-flipped, bottom-left v-flipped,
/// bottom-right both), the top and bottom rows (`edge`, bottom v-flipped) and the left and
/// right columns (`side`, right h-flipped). With `edge` `None`, the top and bottom rows
/// other than their corners are left out, because the caller draws them (the dialogue
/// box's edges are in its canvas).
pub fn frame_cells(rect: TileRect, corner: u16, edge: Option<u16>, side: u16) -> Vec<FrameCell> {
    let mut cells = Vec::new();
    if rect.cols < 2 || rect.rows < 2 {
        return cells;
    }
    for r in 0..rect.rows {
        for c in 0..rect.cols {
            let top = r == 0;
            let bottom = r == rect.rows - 1;
            let left = c == 0;
            let right = c == rect.cols - 1;
            let cell = |tile, hflip, vflip| FrameCell {
                col: rect.col + c,
                row: rect.row + r,
                tile,
                hflip,
                vflip,
            };
            let entry = if (top || bottom) && (left || right) {
                Some(cell(corner, right, bottom))
            } else if top || bottom {
                edge.map(|e| cell(e, false, bottom))
            } else if left || right {
                Some(cell(side, right, false))
            } else {
                None
            };
            cells.extend(entry);
        }
    }
    cells
}

/// Portrait slots by script position id: `(x, y, mirror)` in tiles, 21 entries
/// (`docs/formats/portraits.md`, mirror rule). Position 3 is the Charmander capture.
pub const PORTRAIT_SLOTS: [(u8, u8, bool); 21] = [
    (2, 8, false),
    (12, 5, false),
    (2, 8, false),
    (23, 8, true),
    (7, 8, false),
    (18, 8, true),
    (13, 5, true),
    (2, 8, true),
    (23, 8, false),
    (7, 8, true),
    (18, 8, false),
    (12, 1, false),
    (2, 2, false),
    (23, 2, true),
    (7, 1, false),
    (18, 1, true),
    (13, 1, true),
    (2, 2, true),
    (23, 2, false),
    (7, 1, true),
    (18, 1, false),
];

/// Where a portrait goes for one script position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortraitSlot {
    pub x: u8,
    pub y: u8,
    /// The flag byte that mirrors the image horizontally.
    pub mirror: bool,
}

impl PortraitSlot {
    /// Pixel of the image's top-left corner: tile (`x + 1`, `y + 2`). The frame's top-left
    /// is tile (`x`, `y + 1`) (`docs/formats/portraits.md`, checked on two captures).
    pub fn image_origin(self) -> (i32, i32) {
        ((i32::from(self.x) + 1) * 8, (i32::from(self.y) + 2) * 8)
    }
}

/// The slot for script position `position`, or `None` past the 21 entries.
pub fn portrait_slot(position: usize) -> Option<PortraitSlot> {
    PORTRAIT_SLOTS
        .get(position)
        .map(|&(x, y, mirror)| PortraitSlot { x, y, mirror })
}

/// 8x8 tiles of a `system.sbin` sheet (`font`, `fontsp`) and their palette: palette
/// indices per pixel, index 0 included, and the 16 colours of the palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileSet {
    tiles: Vec<[u8; TILE_PIXELS]>,
    palette: Palette,
}

impl TileSet {
    /// `sheet` is a count word then 32-byte 4bpp tiles; `palette` is RGBX data, of which
    /// the first 16-colour palette is used.
    pub fn new(sheet: &[u8], palette: &[u8]) -> Result<Self> {
        let tiles = font::parse_fixed_font(sheet)?
            .iter()
            .map(|img| {
                let mut t = [0u8; TILE_PIXELS];
                for (d, p) in t.iter_mut().zip(&img.pixels) {
                    *d = p.map_or(0, |(_, c)| c);
                }
                t
            })
            .collect();
        let palette = graphics::rgbx_palettes16(palette)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::invalid("UI palette", "no 16-colour palette"))?;
        Ok(Self { tiles, palette })
    }

    /// Tile `index` (font index for the frame sheet, OBJ tile for `fontsp`).
    pub fn tile(&self, index: u16) -> Option<&[u8; TILE_PIXELS]> {
        self.tiles.get(usize::from(index))
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }
}

fn put(frame: &mut Frame, x: i32, y: i32, c: Rgba) {
    if (0..SCREEN_WIDTH as i32).contains(&x) && (0..SCREEN_HEIGHT as i32).contains(&y) {
        let i = (y as usize * SCREEN_WIDTH as usize + x as usize) * 4;
        frame.pixels[i..i + 4].copy_from_slice(&c);
    }
}

/// Draws one 8x8 tile at pixel (`x`, `y`). Index 0 is transparent, as it is for BG tiles
/// and sprites.
fn blit_tile(
    frame: &mut Frame,
    x: i32,
    y: i32,
    tile: &[u8; TILE_PIXELS],
    palette: &Palette,
    (hflip, vflip): (bool, bool),
) {
    for ty in 0..8 {
        for tx in 0..8 {
            let sx = if hflip { 7 - tx } else { tx };
            let sy = if vflip { 7 - ty } else { ty };
            let index = usize::from(tile[sy * 8 + sx]);
            if index == 0 {
                continue;
            }
            if let Some(c) = palette.get(index) {
                put(frame, x + tx as i32, y + ty as i32, *c);
            }
        }
    }
}

fn blit_cells(frame: &mut Frame, tiles: &TileSet, cells: &[FrameCell]) {
    for cell in cells {
        if let Some(tile) = tiles.tile(cell.tile) {
            blit_tile(
                frame,
                cell.col as i32 * 8,
                cell.row as i32 * 8,
                tile,
                tiles.palette(),
                (cell.hflip, cell.vflip),
            );
        }
    }
}

/// Fills `rect` with the box fill tile (solid colour index 1, `docs/re/ui-screens.md`
/// section 3).
pub fn draw_fill(frame: &mut Frame, tiles: &TileSet, rect: TileRect) {
    if let Some(tile) = tiles.tile(FILL_TILE) {
        for row in rect.row..rect.row + rect.rows {
            for col in rect.col..rect.col + rect.cols {
                blit_tile(
                    frame,
                    col as i32 * 8,
                    row as i32 * 8,
                    tile,
                    tiles.palette(),
                    (false, false),
                );
            }
        }
    }
}

/// Draws the menu window frame around `rect`: corners, edges and sides with the menu tiles.
/// The interior (text over the fill) is the caller's.
pub fn draw_menu_frame(frame: &mut Frame, tiles: &TileSet, rect: TileRect) {
    let cells = frame_cells(rect, MENU_CORNER, Some(MENU_EDGE), MENU_SIDE);
    blit_cells(frame, tiles, &cells);
}

/// Draws the dialogue box: the box fill, the top and bottom edge rows (3 canvas rows each,
/// `BOX_EDGE_TOP` and `BOX_EDGE_BOTTOM`, across the canvas width), then the text canvas over
/// them (colour index 0 is transparent, so what is under it shows through), then the corners
/// and sides.
pub fn draw_dialogue_box(frame: &mut Frame, tiles: &TileSet, canvas: &Canvas) {
    draw_fill(frame, tiles, DIALOGUE_BOX);
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
    // The edge rows are canvas rows, 208 columns wide from the canvas origin.
    for (row, index) in edge_rows {
        if let Some(c) = tiles.palette().get(usize::from(index)) {
            for x in 0..canvas.width {
                put(
                    frame,
                    TEXT_ORIGIN.0 + x as i32,
                    TEXT_ORIGIN.1 + row as i32,
                    *c,
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
            if let Some(c) = tiles.palette().get(usize::from(index)) {
                put(
                    frame,
                    TEXT_ORIGIN.0 + x as i32,
                    TEXT_ORIGIN.1 + y as i32,
                    *c,
                );
            }
        }
    }
    let cells = frame_cells(DIALOGUE_BOX, DIALOGUE_CORNER, None, DIALOGUE_SIDE);
    blit_cells(frame, tiles, &cells);
}

/// Draws a portrait in its frame at `slot`. The frame is 7x7 tiles at (`x`, `y + 1`). The
/// image is 40x40 in the portrait's own palette, mirrored by `slot.mirror` as an exact
/// pixel flip of the 40 columns. Image index 0 is drawn: `[hypothesis]`, the notes record
/// it as unconfirmed in the capture.
pub fn draw_portrait(frame: &mut Frame, tiles: &TileSet, portrait: &Portrait, slot: PortraitSlot) {
    let frame_rect = TileRect {
        col: u32::from(slot.x),
        row: u32::from(slot.y) + 1,
        cols: 7,
        rows: 7,
    };
    let cells = frame_cells(
        frame_rect,
        PORTRAIT_CORNER,
        Some(PORTRAIT_EDGE),
        PORTRAIT_SIDE,
    );
    blit_cells(frame, tiles, &cells);

    let image = &portrait.image;
    let (ox, oy) = slot.image_origin();
    for y in 0..image.height.min(PORTRAIT_SIZE) {
        for x in 0..image.width.min(PORTRAIT_SIZE) {
            let sx = if slot.mirror { image.width - 1 - x } else { x };
            let Some(Some((_, index))) = image.pixels.get(y * image.width + sx).copied() else {
                continue;
            };
            if let Some(c) = portrait.palette.get(usize::from(index)) {
                put(frame, ox + x as i32, oy + y as i32, *c);
            }
        }
    }
}

/// Draws the 16x8 wait marker (`fontsp` tiles 0 and 1) with its top-left at (`x`, `y`).
/// Its position is the OAM entry's (`docs/re/ui-screens.md`, section 5). Whether the entry
/// is top-left is `[hypothesis]`, and so is its blink, which is not drawn here.
pub fn draw_marker(frame: &mut Frame, sprites: &TileSet, x: i32, y: i32) {
    for k in 0..2u16 {
        if let Some(tile) = sprites.tile(MARKER_TILE + k) {
            blit_tile(
                frame,
                x + 8 * i32::from(k),
                y,
                tile,
                sprites.palette(),
                (false, false),
            );
        }
    }
}

/// Draws the 8x8 cursor (`fontsp` tile 4) with its top-left at (`x`, `y`).
pub fn draw_cursor(frame: &mut Frame, sprites: &TileSet, x: i32, y: i32) {
    if let Some(tile) = sprites.tile(CURSOR_TILE) {
        blit_tile(frame, x, y, tile, sprites.palette(), (false, false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(n: u8) -> Rgba {
        [n, n, n, 255]
    }

    /// A 16-colour palette whose entry `i` is grey `i * 10`.
    fn palette() -> Palette {
        (0..16).map(|i| grey(i * 10)).collect()
    }

    fn tileset(tiles: Vec<[u8; TILE_PIXELS]>) -> TileSet {
        TileSet {
            tiles,
            palette: palette(),
        }
    }

    fn pixel(frame: &Frame, x: u32, y: u32) -> Rgba {
        frame.get(x, y)
    }

    fn cell_at(cells: &[FrameCell], col: u32, row: u32) -> Option<FrameCell> {
        cells.iter().copied().find(|c| c.col == col && c.row == row)
    }

    #[test]
    fn menu_frame_flips_follow_the_notes() {
        let rect = TileRect {
            col: 0,
            row: 0,
            cols: 5,
            rows: 5,
        };
        let cells = frame_cells(rect, MENU_CORNER, Some(MENU_EDGE), MENU_SIDE);
        // Perimeter of a 5x5 rectangle.
        assert_eq!(cells.len(), 16);
        let at = |c, r| cell_at(&cells, c, r).unwrap();
        // TL plain, TR h-flip, BL v-flip, BR both.
        assert_eq!(at(0, 0).tile, MENU_CORNER);
        assert_eq!((at(0, 0).hflip, at(0, 0).vflip), (false, false));
        assert_eq!((at(4, 0).hflip, at(4, 0).vflip), (true, false));
        assert_eq!((at(0, 4).hflip, at(0, 4).vflip), (false, true));
        assert_eq!((at(4, 4).hflip, at(4, 4).vflip), (true, true));
        // Top edge as-is, bottom edge v-flipped; left side as-is, right side h-flipped.
        assert_eq!((at(2, 0).tile, at(2, 0).vflip), (MENU_EDGE, false));
        assert_eq!((at(2, 4).tile, at(2, 4).vflip), (MENU_EDGE, true));
        assert_eq!((at(0, 2).tile, at(0, 2).hflip), (MENU_SIDE, false));
        assert_eq!((at(4, 2).tile, at(4, 2).hflip), (MENU_SIDE, true));
        // Interior is not a frame cell.
        assert!(cell_at(&cells, 2, 2).is_none());
    }

    #[test]
    fn dialogue_box_has_corners_and_sides_only() {
        let cells = frame_cells(DIALOGUE_BOX, DIALOGUE_CORNER, None, DIALOGUE_SIDE);
        // Corners at (2,17), (29,17), (2,21), (29,21); sides on rows 18 to 20 at cols 2 and 29.
        assert_eq!(cells.len(), 4 + 3 * 2);
        assert!(
            cells
                .iter()
                .all(|c| !(c.row == 17 && (3..=28).contains(&c.col)))
        );
        let tl = cell_at(&cells, 2, 17).unwrap();
        assert_eq!(
            (tl.tile, tl.hflip, tl.vflip),
            (DIALOGUE_CORNER, false, false)
        );
        let br = cell_at(&cells, 29, 21).unwrap();
        assert_eq!((br.hflip, br.vflip), (true, true));
        let side = cell_at(&cells, 29, 19).unwrap();
        assert_eq!((side.tile, side.hflip), (DIALOGUE_SIDE, true));
    }

    #[test]
    fn portrait_slots_match_the_table_in_the_notes() {
        // Position 3 is the Charmander capture: x 23, y 8, mirrored.
        assert_eq!(
            portrait_slot(3),
            Some(PortraitSlot {
                x: 23,
                y: 8,
                mirror: true
            })
        );
        // Position 20 is the last entry; 21 is past the table.
        assert_eq!(
            portrait_slot(20),
            Some(PortraitSlot {
                x: 18,
                y: 1,
                mirror: false
            })
        );
        assert_eq!(portrait_slot(21), None);
        // Image at tile (x + 1, y + 2): cols 24 to 28, rows 10 to 14 for position 3.
        assert_eq!(portrait_slot(3).unwrap().image_origin(), (192, 80));
    }

    #[test]
    fn mirror_flag_flips_the_image_columns() {
        // Image pixel (x, y) has index (x % 15) + 1, so the first and last columns differ.
        let mut image = rescue_formats::graphics::IndexedImage::new(PORTRAIT_SIZE, PORTRAIT_SIZE);
        for y in 0..PORTRAIT_SIZE {
            for x in 0..PORTRAIT_SIZE {
                image.set(x as isize, y as isize, Some((0, (x % 15 + 1) as u8)));
            }
        }
        let portrait = Portrait {
            emotion: 0,
            palette: palette(),
            image,
        };
        let tiles = tileset(vec![[0; TILE_PIXELS]]);
        let mut plain = Frame::filled([0, 0, 0, 255]);
        let mut mirrored = Frame::filled([0, 0, 0, 255]);
        let plain_slot = PortraitSlot {
            x: 23,
            y: 8,
            mirror: false,
        };
        let mirror_slot = PortraitSlot {
            mirror: true,
            ..plain_slot
        };
        draw_portrait(&mut plain, &tiles, &portrait, plain_slot);
        draw_portrait(&mut mirrored, &tiles, &portrait, mirror_slot);
        let (ox, oy) = plain_slot.image_origin();
        // Column 0 of the plain image has index 1 (grey 10). Mirrored, column 0 shows source
        // column 39, whose index is 39 % 15 + 1 = 10 (grey 100).
        assert_eq!(pixel(&plain, ox as u32, oy as u32), grey(10));
        assert_eq!(pixel(&mirrored, ox as u32, oy as u32), grey(100));
    }

    #[test]
    fn transparent_index_zero_keeps_the_background() {
        let mut tile = [0u8; TILE_PIXELS];
        tile[0] = 5;
        let mut frame = Frame::filled([9, 9, 9, 255]);
        blit_tile(&mut frame, 0, 0, &tile, &palette(), (false, false));
        assert_eq!(pixel(&frame, 0, 0), grey(50));
        assert_eq!(pixel(&frame, 1, 0), grey(9));
    }

    #[test]
    fn hflip_and_vflip_move_the_pixel() {
        let mut tile = [0u8; TILE_PIXELS];
        tile[0] = 5; // top-left
        let mut frame = Frame::filled([9, 9, 9, 255]);
        blit_tile(&mut frame, 0, 0, &tile, &palette(), (true, true));
        assert_eq!(pixel(&frame, 7, 7), grey(50));
        assert_eq!(pixel(&frame, 0, 0), grey(9));
    }

    #[test]
    fn marker_is_two_tiles_side_by_side() {
        let mut left = [0u8; TILE_PIXELS];
        left[0] = 3;
        let mut right = [0u8; TILE_PIXELS];
        right[0] = 4;
        let sprites = tileset(vec![left, right]);
        let mut frame = Frame::filled([0, 0, 0, 255]);
        draw_marker(&mut frame, &sprites, 120, 171);
        assert_eq!(pixel(&frame, 120, 171), grey(30));
        assert_eq!(pixel(&frame, 128, 171), grey(40));
    }

    #[test]
    fn dialogue_box_draws_fill_then_canvas_then_frame() {
        // Tile 31 (corner) is solid index 2; tile 98 (side) index 3; tile 99 (fill) index 1.
        let mut tiles = vec![[0u8; TILE_PIXELS]; 128];
        tiles[31] = [2; TILE_PIXELS];
        tiles[98] = [3; TILE_PIXELS];
        tiles[99] = [1; TILE_PIXELS];
        let tiles = tileset(tiles);
        let mut canvas = Canvas::window();
        canvas.pixels[0] = 7; // canvas (0, 0) is pixel (24, 136)
        let mut frame = Frame::filled([0, 0, 0, 255]);
        draw_dialogue_box(&mut frame, &tiles, &canvas);
        assert_eq!(pixel(&frame, 24, 136), grey(70)); // canvas over the fill
        assert_eq!(pixel(&frame, 25, 144), grey(10)); // fill, below the edge rows
        assert_eq!(pixel(&frame, 16, 136), grey(20)); // top-left corner tile
        assert_eq!(pixel(&frame, 16, 144), grey(30)); // left side tile
        assert_eq!(pixel(&frame, 232, 144), grey(30)); // right side tile
        // Edge rows: top canvas rows 0 to 2 are colours 15, 14, 13; bottom rows 37 to 39 are
        // 13, 14, 15 (`BOX_EDGE_TOP`, `BOX_EDGE_BOTTOM`).
        assert_eq!(pixel(&frame, 100, 136), grey(150));
        assert_eq!(pixel(&frame, 100, 137), grey(140));
        assert_eq!(pixel(&frame, 100, 138), grey(130));
        assert_eq!(pixel(&frame, 100, 173), grey(130));
        assert_eq!(pixel(&frame, 100, 175), grey(150));
    }
}
