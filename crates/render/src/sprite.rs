//! Multi-piece sprites and their z-order against background layers.
//!
//! [`render_sprite`] turns one decoded frame (`rescue_formats::sprite`) into a
//! [`SpriteImage`]: pieces placed at their own offsets with H/V flips and sizes from the
//! OAM shape/size bits, in the species' overworld palette (docs/formats/sprites.md), with
//! each pixel's OAM priority kept for [`compose_with_sprites`]. Tile decoding is
//! `rescue_formats::gfx`; only the VRAM loading order is repeated here, because the
//! decoder's own frame renderer drops the per-piece palette and priority.

use rescue_formats::gfx::{self, Rgba};
use rescue_formats::graphics::{Palette, TILE_4BPP};
use rescue_formats::read::{Error, Result};
use rescue_formats::sprite::{AnimStep, Piece, Sprite};

use crate::bg::{BgLayer, Frame};
use crate::screens::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// OAM attr1 bit 10 (`unk6_1`): the piece keeps its own palette bits instead of the
/// species palette.
const KEEP_PIECE_PALETTE: u16 = 0x0400;

/// A rendered sprite frame: opaque pixels with their OAM priority (0 front to 3 back).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteImage {
    /// Top-left corner relative to the sprite origin.
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    /// Row-major; `None` is transparent.
    pub pixels: Vec<Option<(Rgba, u8)>>,
}

/// The palette a piece is drawn with: the species' overworld palette, unless the piece
/// sets attr1 bit 10 (then its own palette bits).
pub fn piece_palette(piece: &Piece, species_palette: u8) -> u8 {
    if piece.attr1 & KEEP_PIECE_PALETTE != 0 {
        piece.palette()
    } else {
        species_palette
    }
}

/// OAM priority of a piece (attr2 bits 10 and 11).
pub fn piece_priority(piece: &Piece) -> u8 {
    ((piece.attr2 >> 10) & 3) as u8
}

/// Renders frame `frame` of `sprite`. `palettes` are the shared sprite palettes
/// (`monster.sbin` `palet`); a missing palette or colour renders transparent.
pub fn render_sprite(
    sprite: &Sprite,
    frame: usize,
    species_palette: u8,
    palettes: &[Palette],
) -> Result<SpriteImage> {
    let pieces = sprite
        .frames
        .get(frame)
        .filter(|p| !p.is_empty())
        .ok_or(Error::Malformed("sprite frame is missing or has no pieces"))?;
    // Load every piece's image into the frame's VRAM first (pieces without an image
    // reuse what is there), as the decoder does.
    let mut vram = sprite.tiles.clone();
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in pieces {
        let (w, h) = p.size();
        x0 = x0.min(p.x());
        y0 = y0.min(p.y());
        x1 = x1.max(p.x() + w as i32);
        y1 = y1.max(p.y() + h as i32);
        if let Some(img) = p.image {
            let px = sprite
                .images
                .get(usize::from(img))
                .ok_or(Error::Malformed("sprite piece names a missing image"))?;
            let at = p.tile() * TILE_4BPP;
            if vram.len() < at + px.len() {
                vram.resize(at + px.len(), 0);
            }
            vram[at..at + px.len()].copy_from_slice(px);
        }
    }
    let (width, height) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut pixels = vec![None; width * height];
    // Earlier OAM entries are on top, so paint in reverse.
    for p in pieces.iter().rev() {
        let (w, h) = p.size();
        let palette = palettes.get(usize::from(piece_palette(p, species_palette)));
        let priority = piece_priority(p);
        for py in 0..h {
            for px in 0..w {
                let sx = if p.hflip() { w - 1 - px } else { px };
                let sy = if p.vflip() { h - 1 - py } else { py };
                let tile = p.tile() + (sy / 8) * (w / 8) + sx / 8;
                let Some(bytes) = vram.get(tile * TILE_4BPP..(tile + 1) * TILE_4BPP) else {
                    continue;
                };
                let Ok(decoded) = gfx::decode_4bpp_tile(bytes) else {
                    continue;
                };
                let index = decoded[(sy % 8) * 8 + sx % 8];
                if index == 0 {
                    continue;
                }
                let Some(colour) = palette.and_then(|pl| pl.get(usize::from(index))) else {
                    continue;
                };
                if colour[3] == 0 {
                    continue;
                }
                let (ox, oy) = ((p.x() - x0) as usize + px, (p.y() - y0) as usize + py);
                pixels[oy * width + ox] = Some((*colour, priority));
            }
        }
    }
    Ok(SpriteImage {
        left: x0,
        top: y0,
        width,
        height,
        pixels,
    })
}

/// Screen position of the sprite origin for an animation step: the origin plus the
/// step's sprite offset.
pub fn sprite_origin(origin: (i32, i32), step: &AnimStep) -> (i32, i32) {
    (
        origin.0 + i32::from(step.offset.0),
        origin.1 + i32::from(step.offset.1),
    )
}

/// Offset of a shadow's top-left corner from its entity's screen position. Measured, not
/// decoded: all five shadows in two captured dungeon frames (leader, partner, wild) sit at
/// exactly this offset, whatever the animation frame (docs/formats/sprites.md).
pub const SHADOW_OFFSET: (i32, i32) = (-8, -4);

/// Screen position of an entity's shadow image (see [`SHADOW_OFFSET`]).
pub fn shadow_origin(entity: (i32, i32)) -> (i32, i32) {
    (entity.0 + SHADOW_OFFSET.0, entity.1 + SHADOW_OFFSET.1)
}

/// Draws a `w` x `h` OAM object (1D tile mapping, 4bpp) whose first tile is `tile` in
/// `tiles` (32 bytes per tile) with `palette`, at priority `priority`. Used for the
/// non-monster objects (shadows, items). The top-left corner is `left`, `top` relative to
/// the placement point. A tile or colour that is missing renders transparent.
pub fn render_obj(
    tiles: &[u8],
    tile: usize,
    (w, h): (usize, usize),
    palette: &Palette,
    priority: u8,
    (left, top): (i32, i32),
) -> SpriteImage {
    let mut pixels = vec![None; w * h];
    for y in 0..h {
        for x in 0..w {
            let t = tile + (y / 8) * (w / 8) + x / 8;
            let Some(bytes) = tiles.get(t * TILE_4BPP..(t + 1) * TILE_4BPP) else {
                continue;
            };
            let Ok(decoded) = gfx::decode_4bpp_tile(bytes) else {
                continue;
            };
            let index = decoded[(y % 8) * 8 + x % 8];
            if let Some(c) = palette
                .get(usize::from(index))
                .filter(|c| index != 0 && c[3] != 0)
            {
                pixels[y * w + x] = Some((*c, priority));
            }
        }
    }
    SpriteImage {
        left,
        top,
        width: w,
        height: h,
        pixels,
    }
}

/// A sprite image placed on the screen.
#[derive(Debug, Clone, Copy)]
pub struct PlacedSprite<'a> {
    pub image: &'a SpriteImage,
    /// Screen position of the sprite origin.
    pub at: (i32, i32),
}

impl PlacedSprite<'_> {
    fn pixel(&self, x: i32, y: i32) -> Option<(Rgba, u8)> {
        let lx = x - self.at.0 - self.image.left;
        let ly = y - self.at.1 - self.image.top;
        if lx < 0 || ly < 0 || lx as usize >= self.image.width || ly as usize >= self.image.height {
            return None;
        }
        self.image.pixels[ly as usize * self.image.width + lx as usize]
    }
}

fn put(frame: &mut Frame, x: u32, y: u32, c: Rgba) {
    let i = ((y * SCREEN_WIDTH + x) * 4) as usize;
    frame.pixels[i..i + 4].copy_from_slice(&c);
}

/// Like [`crate::bg::compose`], with sprites mixed in by priority. `sprites` are in OAM
/// order, index 0 first. As on the DS, the sprite pixel is chosen first, by OAM index
/// alone: the first sprite with an opaque pixel there wins, whatever its priority. Only
/// that winner is then compared with the backgrounds: a lower priority number is in front,
/// and at equal priority the sprite is in front of the backgrounds and an earlier layer is
/// in front of a later one. With no sprites the result equals `compose`.
pub fn compose_with_sprites(
    layers: &[BgLayer],
    palettes: &[Palette],
    backdrop: Rgba,
    sprites: &[PlacedSprite<'_>],
) -> Frame {
    let mut frame = Frame::filled(backdrop);
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let sprite = sprites.iter().find_map(|s| s.pixel(x as i32, y as i32));
            // Back to front: priority 3 down to 0; within a level the backgrounds
            // (last layer first), then the sprite.
            for level in (0..=3u8).rev() {
                for layer in layers.iter().rev() {
                    if layer.visible
                        && layer.priority.min(3) == level
                        && let Some(c) = layer.pixel(x as i32, y as i32, palettes)
                    {
                        put(&mut frame, x, y, c);
                    }
                }
                if let Some((c, prio)) = sprite
                    && prio == level
                {
                    put(&mut frame, x, y, c);
                }
            }
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_formats::gfx::Rgba;

    const RED: Rgba = [255, 0, 0, 255];
    const GREEN: Rgba = [0, 255, 0, 255];
    const BLUE: Rgba = [0, 0, 255, 255];
    const BACK: Rgba = [1, 2, 3, 255];

    fn palettes() -> Vec<Palette> {
        // Bank 0: red/green/blue at 1..3. Bank 1: shifted (green, blue, red).
        vec![
            vec![[0, 0, 0, 0], RED, GREEN, BLUE],
            vec![[0, 0, 0, 0], GREEN, BLUE, RED],
        ]
    }

    /// A 4bpp 8x8 tile: top-left index 1, top-right 2, bottom-left 3, rest 0.
    fn corner_tile() -> Vec<u8> {
        let mut t = vec![0u8; 32];
        t[0] = 0x01; // pixel (0,0) = 1
        t[3] = 0x20; // pixel (7,0) = 2 (high nibble of byte 3)
        t[28] = 0x03; // pixel (0,7) = 3
        t
    }

    /// attr0 for y, shape; attr1 for x, size, flags; attr2.
    fn piece(x: i32, y: i32, attr1_extra: u16, attr2: u16) -> Piece {
        Piece {
            image: Some(0),
            unknown: 0,
            attr0: (y + 512) as u16,
            attr1: ((x + 256) as u16 & 0x1FF) | attr1_extra,
            attr2,
        }
    }

    fn sprite_of(pieces: Vec<Piece>, images: Vec<Vec<u8>>) -> Sprite {
        Sprite {
            frames: vec![pieces],
            offsets: vec![],
            images,
            groups: vec![],
            tiles: vec![],
        }
    }

    #[test]
    fn flip_positions() {
        let pal = palettes();
        let render = |extra| {
            let s = sprite_of(vec![piece(0, 0, extra, 0)], vec![corner_tile()]);
            render_sprite(&s, 0, 0, &pal).unwrap()
        };
        let at = |i: &SpriteImage, x: usize, y: usize| i.pixels[y * i.width + x].map(|p| p.0);
        let n = render(0);
        assert_eq!(
            (at(&n, 0, 0), at(&n, 7, 0), at(&n, 0, 7)),
            (Some(RED), Some(GREEN), Some(BLUE))
        );
        let h = render(0x1000);
        assert_eq!(
            (at(&h, 7, 0), at(&h, 0, 0), at(&h, 7, 7)),
            (Some(RED), Some(GREEN), Some(BLUE))
        );
        let v = render(0x2000);
        assert_eq!(
            (at(&v, 0, 7), at(&v, 7, 7), at(&v, 0, 0)),
            (Some(RED), Some(GREEN), Some(BLUE))
        );
        let hv = render(0x3000);
        assert_eq!(
            (at(&hv, 7, 7), at(&hv, 0, 7), at(&hv, 7, 0)),
            (Some(RED), Some(GREEN), Some(BLUE))
        );
    }

    #[test]
    fn pieces_are_placed_at_their_offsets_and_bounds_cover_them() {
        let pal = palettes();
        let s = sprite_of(
            vec![piece(-4, -8, 0, 0), piece(10, 2, 0, 0)],
            vec![corner_tile()],
        );
        let img = render_sprite(&s, 0, 0, &pal).unwrap();
        assert_eq!((img.left, img.top), (-4, -8));
        assert_eq!((img.width, img.height), (22, 18));
        // The first piece's top-left pixel is the box's top-left; the second's is at (14, 10).
        assert_eq!(img.pixels[0].map(|p| p.0), Some(RED));
        assert_eq!(img.pixels[10 * 22 + 14].map(|p| p.0), Some(RED));
    }

    #[test]
    fn larger_shapes_use_tiles_in_1d_order() {
        // 16x8 (shape 1, size 0): two tiles side by side; the second tile is a solid index 2.
        let mut data = corner_tile();
        data.extend([0x22u8; 32]);
        let mut p = piece(0, 0, 0, 0);
        p.attr0 |= 1 << 14;
        let s = sprite_of(vec![p], vec![data]);
        let img = render_sprite(&s, 0, 0, &palettes()).unwrap();
        assert_eq!((img.width, img.height), (16, 8));
        assert_eq!(img.pixels[8].map(|p| p.0), Some(GREEN));
        assert_eq!(img.pixels[15 + 7 * 16].map(|p| p.0), Some(GREEN));
    }

    #[test]
    fn species_palette_replaces_piece_bits_unless_bit_10() {
        let pal = palettes();
        // The piece asks for bank 0 in attr2; the species palette is bank 1.
        let s = sprite_of(vec![piece(0, 0, 0, 0)], vec![corner_tile()]);
        let img = render_sprite(&s, 0, 1, &pal).unwrap();
        assert_eq!(img.pixels[0].map(|p| p.0), Some(GREEN)); // bank 1 colour 1
        // With attr1 bit 10 the piece's own bits (bank 0) win.
        let s = sprite_of(vec![piece(0, 0, 0x0400, 0)], vec![corner_tile()]);
        let img = render_sprite(&s, 0, 1, &pal).unwrap();
        assert_eq!(img.pixels[0].map(|p| p.0), Some(RED));
        assert_eq!(piece_palette(&piece(0, 0, 0x0400, 0x1000), 5), 1);
        assert_eq!(piece_palette(&piece(0, 0, 0, 0x1000), 5), 5);
    }

    #[test]
    fn earlier_pieces_are_on_top_and_priority_is_kept() {
        let pal = palettes();
        let solid = |idx: u8| vec![idx | idx << 4; 32];
        let a = Piece {
            image: Some(0),
            ..piece(0, 0, 0, 0x0800)
        }; // priority 2, tile 0
        let b = Piece {
            image: Some(1),
            ..piece(0, 0, 0, 0x0400 | 1)
        }; // priority 1, tile 1
        let s = sprite_of(vec![a, b], vec![solid(1), solid(2)]);
        let img = render_sprite(&s, 0, 0, &pal).unwrap();
        assert_eq!(img.pixels[0], Some((RED, 2))); // the first piece wins
        assert_eq!(piece_priority(&b), 1);
    }

    #[test]
    fn bad_frames_and_images_are_errors_not_panics() {
        let pal = palettes();
        let s = sprite_of(vec![piece(0, 0, 0, 0)], vec![corner_tile()]);
        assert!(render_sprite(&s, 5, 0, &pal).is_err());
        let mut bad = piece(0, 0, 0, 0);
        bad.image = Some(9);
        assert!(render_sprite(&sprite_of(vec![bad], vec![]), 0, 0, &pal).is_err());
        // A missing palette draws nothing.
        let img = render_sprite(&s, 0, 9, &pal).unwrap();
        assert!(img.pixels.iter().all(Option::is_none));
    }

    #[test]
    fn step_offset_and_shadow_origin() {
        let step = AnimStep {
            duration: 1,
            flags: 0,
            frame: 0,
            offset: (3, -2),
            shadow: (-1, 5),
        };
        assert_eq!(sprite_origin((100, 50), &step), (103, 48));
        // The shadow ignores the step: measured at the entity position + (-8, -4).
        assert_eq!(shadow_origin((128, 108)), (120, 104));
        assert_eq!(shadow_origin((104, 132)), (96, 128));
    }

    #[test]
    fn render_obj_uses_1d_tiles_and_palette() {
        let mut tiles = vec![0u8; 32];
        tiles.extend([0x21u8; 32]); // tile 1: indices 1 and 2 alternate
        tiles.extend([0x33u8; 32]); // tile 2: index 3
        let img = render_obj(&tiles, 1, (16, 8), &palettes()[0], 3, (-8, -4));
        assert_eq!((img.left, img.top, img.width, img.height), (-8, -4, 16, 8));
        assert_eq!(img.pixels[0], Some((RED, 3)));
        assert_eq!(img.pixels[1], Some((GREEN, 3)));
        assert_eq!(img.pixels[8], Some((BLUE, 3))); // second tile
        // A tile past the data is transparent.
        assert!(
            render_obj(&tiles, 5, (8, 8), &palettes()[0], 3, (0, 0))
                .pixels
                .iter()
                .all(Option::is_none)
        );
    }

    // z-order ---------------------------------------------------------------

    fn solid_layer(index: u8, priority: u8) -> BgLayer {
        let mut l = BgLayer::new(
            vec![[0; 64], [index; 64]],
            vec![rescue_formats::gfx::MapEntry::from_raw(1)],
            1,
            1,
        )
        .unwrap();
        l.priority = priority;
        l
    }

    fn square(colour: Rgba, priority: u8) -> SpriteImage {
        SpriteImage {
            left: 0,
            top: 0,
            width: 2,
            height: 2,
            pixels: vec![Some((colour, priority)); 4],
        }
    }

    #[test]
    fn sprite_priority_against_bg_priority() {
        let pal = palettes();
        let sprite = square(BLUE, 1);
        let placed = [PlacedSprite {
            image: &sprite,
            at: (10, 10),
        }];
        let at = |f: &Frame| f.get(10, 10);
        // BG priority 0 is in front of a priority 1 sprite.
        let f = compose_with_sprites(&[solid_layer(1, 0)], &pal, BACK, &placed);
        assert_eq!(at(&f), RED);
        // Equal priority: the sprite is in front.
        let f = compose_with_sprites(&[solid_layer(1, 1)], &pal, BACK, &placed);
        assert_eq!(at(&f), BLUE);
        // BG behind the sprite.
        let f = compose_with_sprites(&[solid_layer(1, 3)], &pal, BACK, &placed);
        assert_eq!(at(&f), BLUE);
        // Outside the sprite the background shows.
        assert_eq!(f.get(12, 10), RED);
    }

    #[test]
    fn sprites_order_among_themselves_and_with_layers() {
        let pal = palettes();
        let (front, back) = (square(GREEN, 0), square(BLUE, 0));
        let placed = [
            PlacedSprite {
                image: &front,
                at: (0, 0),
            },
            PlacedSprite {
                image: &back,
                at: (0, 0),
            },
        ];
        // Same priority: the earlier sprite is in front.
        assert_eq!(
            compose_with_sprites(&[], &pal, BACK, &placed).get(0, 0),
            GREEN
        );
        // A priority 2 layer and a priority 0 layer: the sprite (priority 1) sits between.
        let mid = square(BLUE, 1);
        let placed = [PlacedSprite {
            image: &mid,
            at: (0, 0),
        }];
        let layers = [solid_layer(1, 0), solid_layer(2, 2)];
        let f = compose_with_sprites(&layers, &pal, BACK, &placed);
        assert_eq!(f.get(0, 0), RED); // priority 0 layer on top
        let layers = [solid_layer(2, 2)];
        let f = compose_with_sprites(&layers, &pal, BACK, &placed);
        assert_eq!(f.get(0, 0), BLUE);
    }

    #[test]
    fn sprite_order_follows_oam_index_not_priority() {
        let pal = palettes();
        // The earlier sprite has the worse priority (3) but is first in OAM: it still wins
        // over the later priority 0 sprite, and then loses to a priority 1 layer.
        let (first, second) = (square(GREEN, 3), square(BLUE, 0));
        let placed = [
            PlacedSprite {
                image: &first,
                at: (0, 0),
            },
            PlacedSprite {
                image: &second,
                at: (0, 0),
            },
        ];
        assert_eq!(
            compose_with_sprites(&[], &pal, BACK, &placed).get(0, 0),
            GREEN
        );
        let layers = [solid_layer(1, 1)];
        assert_eq!(
            compose_with_sprites(&layers, &pal, BACK, &placed).get(0, 0),
            RED
        );
        // Where the first is transparent the second shows.
        let placed = [
            PlacedSprite {
                image: &first,
                at: (1, 0),
            },
            PlacedSprite {
                image: &second,
                at: (0, 0),
            },
        ];
        assert_eq!(
            compose_with_sprites(&[], &pal, BACK, &placed).get(0, 0),
            BLUE
        );
    }

    #[test]
    fn no_sprites_equals_compose_and_hidden_layers_are_skipped() {
        let pal = palettes();
        let layers = [solid_layer(1, 1), solid_layer(2, 0)];
        assert_eq!(
            compose_with_sprites(&layers, &pal, BACK, &[]),
            crate::bg::compose(&layers, &pal, BACK)
        );
        let mut hidden = solid_layer(1, 0);
        hidden.visible = false;
        assert_eq!(
            compose_with_sprites(&[hidden], &pal, BACK, &[]).get(0, 0),
            BACK
        );
    }

    #[test]
    fn sprites_clip_at_the_screen_edges_and_may_be_negative() {
        let pal = palettes();
        let sprite = square(BLUE, 0);
        let placed = [
            PlacedSprite {
                image: &sprite,
                at: (-1, -1),
            },
            PlacedSprite {
                image: &sprite,
                at: (255, 191),
            },
        ];
        let f = compose_with_sprites(&[], &pal, BACK, &placed);
        assert_eq!(f.get(0, 0), BLUE);
        assert_eq!(f.get(1, 1), BACK);
        assert_eq!(f.get(255, 191), BLUE);
    }
}
