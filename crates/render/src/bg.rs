//! Background layer composition.
//!
//! A [`BgLayer`] is a wrapping tilemap of 8x8 tiles (palette indices), scrolled by a
//! pixel offset. [`compose`] draws the layers into a 256x192 RGBA [`Frame`]: the
//! backdrop first, then layers from the lowest priority (back) to the highest, like the
//! DS: a layer with a smaller `priority` number is in front, and between equal
//! priorities the lower layer index is in front. Colour index 0 and fully transparent
//! palette colours are not drawn. Tile and map decoding is in `rescue_formats`.

use rescue_formats::gfx::{self, MapEntry, Rgba, TILE_PIXELS};
use rescue_formats::graphics::Palette;
use rescue_formats::ground::{Bma, Bpa, Bpc};
use rescue_formats::read::{Error, Result};

use crate::screens::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// One background layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgLayer {
    /// Decoded tiles, indexed by [`MapEntry::tile`].
    pub tiles: Vec<[u8; TILE_PIXELS]>,
    /// Tilemap cells, row-major, `cols` x `rows`.
    pub map: Vec<MapEntry>,
    /// Map width in tiles.
    pub cols: usize,
    /// Map height in tiles.
    pub rows: usize,
    /// Horizontal scroll in pixels (the map wraps).
    pub scroll_x: i32,
    /// Vertical scroll in pixels (the map wraps).
    pub scroll_y: i32,
    /// 0 is the front-most priority, 3 the back-most.
    pub priority: u8,
    pub visible: bool,
}

impl BgLayer {
    /// A visible layer at priority 0 with no scroll. `None` if the map is empty or
    /// `map.len() != cols * rows`.
    pub fn new(
        tiles: Vec<[u8; TILE_PIXELS]>,
        map: Vec<MapEntry>,
        cols: usize,
        rows: usize,
    ) -> Option<Self> {
        (cols > 0 && rows > 0 && cols.checked_mul(rows) == Some(map.len())).then_some(BgLayer {
            tiles,
            map,
            cols,
            rows,
            scroll_x: 0,
            scroll_y: 0,
            priority: 0,
            visible: true,
        })
    }

    /// Colour of the layer at screen pixel `(x, y)`, or `None` if transparent.
    /// `palettes[n]` is the bank of tilemap cells with `palette == n`.
    pub fn pixel(&self, x: i32, y: i32, palettes: &[Palette]) -> Option<Rgba> {
        let w = self.cols as i64 * 8;
        let h = self.rows as i64 * 8;
        let mx = (i64::from(x) + i64::from(self.scroll_x)).rem_euclid(w) as usize;
        let my = (i64::from(y) + i64::from(self.scroll_y)).rem_euclid(h) as usize;
        let cell = self.map[(my / 8) * self.cols + mx / 8];
        let tile = self.tiles.get(usize::from(cell.tile))?;
        let tx = if cell.hflip { 7 - mx % 8 } else { mx % 8 };
        let ty = if cell.vflip { 7 - my % 8 } else { my % 8 };
        let index = tile[ty * 8 + tx];
        if index == 0 {
            return None;
        }
        let colour = *palettes
            .get(usize::from(cell.palette))?
            .get(usize::from(index))?;
        (colour[3] != 0).then_some(colour)
    }
}

/// A 256x192 RGBA frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// RGBA8, row-major, `SCREEN_WIDTH * SCREEN_HEIGHT * 4` bytes.
    pub pixels: Vec<u8>,
}

impl Frame {
    /// A frame filled with `colour`.
    pub fn filled(colour: Rgba) -> Self {
        let n = (SCREEN_WIDTH * SCREEN_HEIGHT) as usize;
        Frame {
            pixels: colour.iter().copied().cycle().take(n * 4).collect(),
        }
    }

    /// Pixel at `(x, y)`, which must be on screen.
    pub fn get(&self, x: u32, y: u32) -> Rgba {
        let i = ((y * SCREEN_WIDTH + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    fn set(&mut self, x: u32, y: u32, c: Rgba) {
        let i = ((y * SCREEN_WIDTH + x) * 4) as usize;
        self.pixels[i..i + 4].copy_from_slice(&c);
    }
}

/// Composes `layers` over `backdrop` using the shared `palettes`.
pub fn compose(layers: &[BgLayer], palettes: &[Palette], backdrop: Rgba) -> Frame {
    let mut order: Vec<&BgLayer> = layers.iter().filter(|l| l.visible).collect();
    // Back to front. `sort_by_key` is stable, so equal priorities keep layer order; the
    // reversal puts the lowest index in front.
    order.reverse();
    order.sort_by_key(|l| std::cmp::Reverse(l.priority));
    let mut frame = Frame::filled(backdrop);
    for layer in order {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                if let Some(c) = layer.pixel(x as i32, y as i32, palettes) {
                    frame.set(x, y, c);
                }
            }
        }
    }
    frame
}

/// A layer that paints each cell of `cells` (row-major, `cols` x `rows`) as one solid 8x8
/// tile in palette index `cells[i]` of bank 0. A stand-in for a tileset until the ROM's
/// tiles are loaded (#60). `None` if the map is empty or `cells` is not `cols * rows`.
pub fn solid_cells_layer(cells: &[u8], cols: usize, rows: usize) -> Option<BgLayer> {
    let tiles = (0..=u8::MAX).map(|v| [v; TILE_PIXELS]).collect();
    let map = cells
        .iter()
        .map(|&v| MapEntry {
            tile: u16::from(v),
            hflip: false,
            vflip: false,
            palette: 0,
        })
        .collect();
    BgLayer::new(tiles, map, cols, rows)
}

/// Builds a layer from ground-map data: layer `layer` of `bma`, with the BPC tiles
/// followed by frame 0 of each BPA slot (a missing slot is blank), as in VRAM. Palette
/// numbers are BPL bank indices. `Err` if the layer or a chunk is out of range.
pub fn ground_layer(bpc: &Bpc, bpa: &[Option<Bpa>], bma: &Bma, layer: usize) -> Result<BgLayer> {
    ground_layer_at(bpc, bpa, bma, layer, [0; 4])
}

/// Like [`ground_layer`], with BPA slot `i` showing animation frame `frames[i]` (the frame
/// the game has copied into VRAM at that moment). A frame the slot lacks is blank.
pub fn ground_layer_at(
    bpc: &Bpc,
    bpa: &[Option<Bpa>],
    bma: &Bma,
    layer: usize,
    frames: [usize; 4],
) -> Result<BgLayer> {
    let rows = bma
        .layers
        .get(layer)
        .ok_or(Error::Malformed("BMA has no such layer"))?;
    let mut chr = bpc.chr.clone();
    for (i, &n) in bpc.bpa_tiles.iter().enumerate() {
        let want = usize::from(n) * 32;
        match bpa
            .get(i)
            .and_then(Option::as_ref)
            .and_then(|b| b.frame(frames[i]))
        {
            Some(f) if f.len() == want => chr.extend_from_slice(f),
            _ => chr.resize(chr.len() + want, 0),
        }
    }
    let tiles = gfx::decode_4bpp_tiles(&chr)?;
    let (cw, ch) = (usize::from(bpc.chunk_width), usize::from(bpc.chunk_height));
    let cols = usize::from(bma.width_chunks) * cw;
    let rows_t = usize::from(bma.height_chunks) * ch;
    let mut map = vec![MapEntry::from_raw(0); cols * rows_t];
    for (cy, row) in rows.iter().enumerate() {
        for (cx, &c) in row.iter().enumerate() {
            let entries = bpc
                .chunk(c)
                .ok_or(Error::Malformed("BMA names a chunk the BPC lacks"))?;
            for (k, &raw) in entries.iter().enumerate() {
                let at = (cy * ch + k / cw) * cols + cx * cw + k % cw;
                if let Some(cell) = map.get_mut(at) {
                    *cell = MapEntry::from_raw(raw);
                }
            }
        }
    }
    BgLayer::new(tiles, map, cols, rows_t).ok_or(Error::Malformed("BMA map is empty"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = [255, 0, 0, 255];
    const GREEN: Rgba = [0, 255, 0, 255];
    const BLUE: Rgba = [0, 0, 255, 255];
    const BACK: Rgba = [1, 2, 3, 255];

    fn banks() -> Vec<Palette> {
        vec![vec![[0, 0, 0, 0], RED, GREEN, BLUE]]
    }

    fn cell(tile: u16, hflip: bool, vflip: bool) -> MapEntry {
        MapEntry {
            tile,
            hflip,
            vflip,
            palette: 0,
        }
    }

    /// Tile whose left-top pixel is index 1, top-right 2, bottom-left 3, rest 0.
    fn corner_tile() -> [u8; TILE_PIXELS] {
        let mut t = [0u8; TILE_PIXELS];
        t[0] = 1;
        t[7] = 2;
        t[56] = 3;
        t
    }

    fn one_cell(entry: MapEntry) -> BgLayer {
        BgLayer::new(vec![[0; 64], corner_tile()], vec![entry], 1, 1).unwrap()
    }

    #[test]
    fn no_flip() {
        let l = one_cell(cell(1, false, false));
        let p = banks();
        assert_eq!(l.pixel(0, 0, &p), Some(RED));
        assert_eq!(l.pixel(7, 0, &p), Some(GREEN));
        assert_eq!(l.pixel(0, 7, &p), Some(BLUE));
        assert_eq!(l.pixel(7, 7, &p), None);
    }

    #[test]
    fn horizontal_flip() {
        let l = one_cell(cell(1, true, false));
        let p = banks();
        assert_eq!(l.pixel(7, 0, &p), Some(RED));
        assert_eq!(l.pixel(0, 0, &p), Some(GREEN));
        assert_eq!(l.pixel(7, 7, &p), Some(BLUE));
    }

    #[test]
    fn vertical_flip() {
        let l = one_cell(cell(1, false, true));
        let p = banks();
        assert_eq!(l.pixel(0, 7, &p), Some(RED));
        assert_eq!(l.pixel(7, 7, &p), Some(GREEN));
        assert_eq!(l.pixel(0, 0, &p), Some(BLUE));
    }

    #[test]
    fn both_flips() {
        let l = one_cell(cell(1, true, true));
        let p = banks();
        assert_eq!(l.pixel(7, 7, &p), Some(RED));
        assert_eq!(l.pixel(0, 7, &p), Some(GREEN));
        assert_eq!(l.pixel(7, 0, &p), Some(BLUE));
    }

    #[test]
    fn missing_tile_bank_or_colour_is_transparent() {
        let p = banks();
        assert_eq!(one_cell(cell(9, false, false)).pixel(0, 0, &p), None);
        let mut bad_bank = cell(1, false, false);
        bad_bank.palette = 5;
        assert_eq!(one_cell(bad_bank).pixel(0, 0, &p), None);
        let clear = vec![vec![[0, 0, 0, 0], [9, 9, 9, 0]]];
        assert_eq!(one_cell(cell(1, false, false)).pixel(0, 0, &clear), None);
    }

    #[test]
    fn scroll_wraps_both_ways() {
        // 2x1 map: tile 1 then tile 0 (blank), 16x8 pixels.
        let mut l = BgLayer::new(
            vec![[0; 64], corner_tile()],
            vec![cell(1, false, false), cell(0, false, false)],
            2,
            1,
        )
        .unwrap();
        let p = banks();
        l.scroll_x = 1;
        assert_eq!(l.pixel(15, 0, &p), Some(RED)); // 15 + 1 wraps to map x 0
        assert_eq!(l.pixel(14, 0, &p), None); // map x 15, blank tile
        l.scroll_x = 0;
        assert_eq!(l.pixel(16, 0, &p), Some(RED)); // wraps at 16
        assert_eq!(l.pixel(32, 0, &p), Some(RED));
        l.scroll_x = -1;
        assert_eq!(l.pixel(0, 0, &p), None); // map x 15 -> blank tile
        assert_eq!(l.pixel(1, 0, &p), Some(RED)); // map x 0
        l.scroll_x = 0;
        l.scroll_y = -1;
        assert_eq!(l.pixel(0, 0, &p), Some(BLUE)); // map y 7 (rows wrap at 8)
        assert_eq!(l.pixel(0, 1, &p), Some(RED)); // map y 0
    }

    fn solid(colour_index: u8, priority: u8) -> BgLayer {
        let mut l = BgLayer::new(
            vec![[0; 64], [colour_index; 64]],
            vec![cell(1, false, false)],
            1,
            1,
        )
        .unwrap();
        l.priority = priority;
        l
    }

    #[test]
    fn priority_orders_layers() {
        let p = banks();
        // Layer 0 red at priority 2, layer 1 green at priority 1: green is in front.
        let f = compose(&[solid(1, 2), solid(2, 1)], &p, BACK);
        assert_eq!(f.get(10, 10), GREEN);
        // Equal priority: the lower index is in front.
        let f = compose(&[solid(1, 1), solid(2, 1)], &p, BACK);
        assert_eq!(f.get(10, 10), RED);
        // Hidden layers are skipped; transparent areas show what is behind.
        let mut front = solid(1, 0);
        front.visible = false;
        let f = compose(&[front, solid(3, 3)], &p, BACK);
        assert_eq!(f.get(0, 0), BLUE);
        let f = compose(&[], &p, BACK);
        assert_eq!(f.get(255, 191), BACK);
        assert_eq!(f.pixels.len(), 256 * 192 * 4);
    }

    #[test]
    fn transparent_pixels_show_the_layer_behind() {
        let p = banks();
        let f = compose(&[one_cell(cell(1, false, false)), solid(3, 3)], &p, BACK);
        assert_eq!(f.get(0, 0), RED);
        assert_eq!(f.get(3, 3), BLUE); // blank pixel of the front layer
    }

    #[test]
    fn layer_constructor_validates_size() {
        assert!(BgLayer::new(vec![], vec![], 0, 0).is_none());
        assert!(BgLayer::new(vec![], vec![cell(0, false, false)], 2, 1).is_none());
    }
}
