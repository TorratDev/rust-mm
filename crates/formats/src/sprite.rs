//! Creature sprites (`monster.sbin` `axNNN`): OAM-style frames, images and animation
//! tables. Spec: `docs/formats/sprites.md`.
//!
//! SIR0 content header, five `u32`: frame table pointer, animation group table pointer,
//! group count, image table pointer, per-frame offsets pointer. Tables sit back to back
//! (frame table, then offsets; image table, then the content header), which gives their
//! lengths. Palettes are not in the file: pieces name a palette from `monster.sbin`
//! `palet` (14 x 16 RGBX colors).

use crate::gfx::MapEntry;
use crate::graphics::{Depth, IndexedImage, TILE_4BPP};
use crate::read::{self as bytes, Error, Result};
use crate::sir0::Sir0;

/// Facing directions per animation group.
pub const DIRECTIONS: usize = 8;
/// x/y origin of piece coordinates: pieces are placed around (256, 512).
pub const ORIGIN: (i32, i32) = (256, 512);
const MAX_PIECES: usize = 64;
const MAX_SEQUENCE: usize = 512;

/// One OAM piece of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Image index, or `None` (-1) to reuse tiles already loaded at [`Piece::tile`].
    pub image: Option<u16>,
    /// Unknown second field (0 in almost all pieces).
    pub unknown: u16,
    /// OAM attribute 0 (y, shape).
    pub attr0: u16,
    /// OAM attribute 1 (x, flips, size, last-piece flag in bit 11).
    pub attr1: u16,
    /// OAM attribute 2 (tile number, priority, palette).
    pub attr2: u16,
}

impl Piece {
    /// Width and height in pixels from the OAM shape/size bits.
    pub fn size(&self) -> (usize, usize) {
        const SIZES: [[(usize, usize); 4]; 3] = [
            [(8, 8), (16, 16), (32, 32), (64, 64)],
            [(16, 8), (32, 8), (32, 16), (64, 32)],
            [(8, 16), (8, 32), (16, 32), (32, 64)],
        ];
        let shape = usize::from(self.attr0 >> 14).min(2);
        SIZES[shape][usize::from(self.attr1 >> 14)]
    }
    /// x relative to the sprite origin.
    pub fn x(&self) -> i32 {
        i32::from(self.attr1 & 0x1FF) - ORIGIN.0
    }
    /// y relative to the sprite origin.
    pub fn y(&self) -> i32 {
        i32::from(self.attr0 & 0x3FF) - ORIGIN.1
    }
    /// Horizontal flip.
    pub fn hflip(&self) -> bool {
        self.attr1 & 0x1000 != 0
    }
    /// Vertical flip.
    pub fn vflip(&self) -> bool {
        self.attr1 & 0x2000 != 0
    }
    /// Last piece of its frame.
    pub fn is_last(&self) -> bool {
        self.attr1 & 0x0800 != 0
    }
    /// First VRAM tile (32-byte units within the frame).
    pub fn tile(&self) -> usize {
        usize::from(self.attr2 & 0x3FF)
    }
    /// Palette number (into `monster.sbin` `palet`).
    pub fn palette(&self) -> u8 {
        (self.attr2 >> 12) as u8
    }
}

/// One step of an animation sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimStep {
    /// Duration in game frames.
    pub duration: u8,
    /// Flag byte (meaning unknown; 0 in most steps).
    pub flags: u8,
    /// Frame index.
    pub frame: u16,
    /// Sprite offset.
    pub offset: (i16, i16),
    /// Second point, probably the shadow offset (`[hypothesis]`).
    pub shadow: (i16, i16),
}

/// A decoded sprite file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    /// Frames, each a list of pieces.
    pub frames: Vec<Vec<Piece>>,
    /// Per-frame reference points (4 `(x, y)` pairs; Explorers calls them head, hands,
    /// centre: `[hypothesis]`). Empty for effect objects, which have none.
    pub offsets: Vec<[(i16, i16); 4]>,
    /// Images: 4bpp tile data in 1D order.
    pub images: Vec<Vec<u8>>,
    /// Animation groups: per group, one step sequence per direction.
    pub groups: Vec<[Vec<AnimStep>; DIRECTIONS]>,
    /// 4bpp tiles already in VRAM before any frame loads. Effect objects keep their
    /// tiles here ([`crate::effect`]); creature sprites leave it empty.
    pub tiles: Vec<u8>,
}

/// A rendered frame: image plus where the sprite origin sits inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedFrame {
    /// Pixels; palette numbers are the pieces' OAM palettes.
    pub image: IndexedImage,
    /// Position of the origin in `image`.
    pub origin: (i32, i32),
}

/// Species number from an `axNNN` sub-file name.
pub fn species_of(name: &str) -> Option<u16> {
    name.strip_prefix("ax")?.parse().ok()
}

/// Pieces of one frame, read from `at` through the piece with the last-piece flag.
pub(crate) fn read_pieces(data: &[u8], mut at: usize) -> Result<Vec<Piece>> {
    let mut pieces = Vec::new();
    loop {
        let img = bytes::i16_at(data, at, "sprite piece")?;
        let p = Piece {
            image: u16::try_from(img).ok(),
            unknown: bytes::u16_at(data, at + 2, "sprite piece")?,
            attr0: bytes::u16_at(data, at + 4, "sprite piece")?,
            attr1: bytes::u16_at(data, at + 6, "sprite piece")?,
            attr2: bytes::u16_at(data, at + 8, "sprite piece")?,
        };
        at += 10;
        pieces.push(p);
        if p.is_last() {
            return Ok(pieces);
        }
        if pieces.len() >= MAX_PIECES {
            return Err(Error::invalid("sprite frame", "no last-piece flag"));
        }
    }
}

/// Animation steps of one direction, read from `at` up to the zero-duration terminator.
/// Every step must name one of the `frame_count` frames.
pub(crate) fn read_steps(data: &[u8], mut at: usize, frame_count: usize) -> Result<Vec<AnimStep>> {
    let mut seq = Vec::new();
    loop {
        let duration = bytes::u8_at(data, at, "sprite animation")?;
        if duration == 0 {
            return Ok(seq);
        }
        let i16_at = |o| bytes::i16_at(data, at + o, "sprite animation");
        let step = AnimStep {
            duration,
            flags: bytes::u8_at(data, at + 1, "sprite animation")?,
            frame: bytes::u16_at(data, at + 2, "sprite animation")?,
            offset: (i16_at(4)?, i16_at(6)?),
            shadow: (i16_at(8)?, i16_at(10)?),
        };
        if usize::from(step.frame) >= frame_count {
            return Err(Error::invalid(
                "sprite animation",
                format!("frame {} of {frame_count}", step.frame),
            ));
        }
        seq.push(step);
        at += 12;
        if seq.len() > MAX_SEQUENCE {
            return Err(Error::invalid(
                "sprite animation",
                "sequence has no terminator",
            ));
        }
    }
}

impl Sprite {
    /// Parses a sprite file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let s = Sir0::parse(data)?;
        let h = s.data_offset;
        let frames_at = s.ptr(h, "sprite header")?;
        let groups_at = s.ptr(h + 4, "sprite header")?;
        let group_count = s.u32(h + 8, "sprite header")? as usize;
        let images_at = s.ptr(h + 12, "sprite header")?;
        let offsets_at = s.ptr(h + 16, "sprite header")?;
        if offsets_at < frames_at || h < images_at || group_count > 256 {
            return Err(Error::invalid("sprite header", "tables out of order"));
        }
        let frame_count = (offsets_at - frames_at) / 4;
        let mut frames = Vec::with_capacity(frame_count);
        for i in 0..frame_count {
            let at = s.ptr(frames_at + i * 4, "sprite frame table")?;
            frames.push(read_pieces(data, at)?);
        }
        let offsets = (0..frame_count)
            .map(|i| {
                let mut pts = [(0, 0); 4];
                for (k, p) in pts.iter_mut().enumerate() {
                    let at = offsets_at + i * 16 + k * 4;
                    *p = (
                        bytes::i16_at(data, at, "sprite offsets")?,
                        bytes::i16_at(data, at + 2, "sprite offsets")?,
                    );
                }
                Ok(pts)
            })
            .collect::<Result<Vec<_>>>()?;
        let image_count = (h - images_at) / 4;
        let mut images = Vec::with_capacity(image_count);
        for i in 0..image_count {
            let mut at = s.ptr(images_at + i * 4, "sprite image table")?;
            let mut px = Vec::new();
            loop {
                let ptr = s.ptr(at, "sprite image strip")?;
                let len = bytes::offset_at(data, at + 4, "sprite image strip")?;
                at += 8;
                if len == 0 {
                    break;
                }
                if px.len() + len > 64 * 64 {
                    return Err(Error::invalid("sprite image", "larger than 64x64"));
                }
                if ptr == 0 {
                    px.resize(px.len() + len, 0);
                } else {
                    px.extend_from_slice(bytes::slice(data, ptr, len, "sprite pixels")?);
                }
            }
            images.push(px);
        }
        let mut groups = Vec::with_capacity(group_count);
        for g in 0..group_count {
            let dirs_at = s.ptr(groups_at + g * 4, "sprite group table")?;
            let mut dirs: [Vec<AnimStep>; DIRECTIONS] = Default::default();
            for (d, seq) in dirs.iter_mut().enumerate() {
                let at = s.ptr(dirs_at + d * 4, "sprite direction table")?;
                *seq = read_steps(data, at, frame_count)?;
            }
            groups.push(dirs);
        }
        Ok(Self {
            frames,
            offsets,
            images,
            groups,
            tiles: Vec::new(),
        })
    }

    /// Renders frame `index`: loads each piece's image into a per-frame VRAM at the
    /// piece's tile number (pieces without an image reuse what is there), then draws
    /// pieces back to front.
    pub fn render_frame(&self, index: usize) -> Result<RenderedFrame> {
        let pieces = self
            .frames
            .get(index)
            .ok_or_else(|| Error::invalid("sprite", format!("no frame {index}")))?;
        if pieces.is_empty() {
            return Err(Error::invalid(
                "sprite",
                format!("frame {index} has no pieces"),
            ));
        }
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        let mut vram = self.tiles.clone();
        for p in pieces {
            let (w, h) = p.size();
            x0 = x0.min(p.x());
            y0 = y0.min(p.y());
            x1 = x1.max(p.x() + w as i32);
            y1 = y1.max(p.y() + h as i32);
            if let Some(img) = p.image {
                let px = self
                    .images
                    .get(usize::from(img))
                    .ok_or_else(|| Error::invalid("sprite piece", format!("no image {img}")))?;
                let at = p.tile() * TILE_4BPP;
                if vram.len() < at + px.len() {
                    vram.resize(at + px.len(), 0);
                }
                vram[at..at + px.len()].copy_from_slice(px);
            }
        }
        let mut image = IndexedImage::new((x1 - x0) as usize, (y1 - y0) as usize);
        // Earlier OAM entries draw on top, so paint in reverse.
        for p in pieces.iter().rev() {
            let (w, h) = p.size();
            let (tw, th) = (w / 8, h / 8);
            for ty in 0..th {
                for tx in 0..tw {
                    let tile = p.tile() + ty * tw + tx;
                    let dx = if p.hflip() { tw - 1 - tx } else { tx };
                    let dy = if p.vflip() { th - 1 - ty } else { ty };
                    let e = MapEntry {
                        tile: 0,
                        hflip: p.hflip(),
                        vflip: p.vflip(),
                        palette: p.palette(),
                    };
                    let src = vram.get(tile * TILE_4BPP..).unwrap_or(&[]);
                    image.draw_tile(
                        src,
                        Depth::Bpp4,
                        e,
                        (p.x() - x0) as isize + dx as isize * 8,
                        (p.y() - y0) as isize + dy as isize * 8,
                        true,
                    )?;
                }
            }
        }
        Ok(RenderedFrame {
            image,
            origin: (-x0, -y0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sir0::tests::wrap;

    /// One frame, one 8x8 piece, one image, one group whose 8 directions share a
    /// one-step sequence.
    fn synthetic() -> Vec<u8> {
        let mut b = Vec::new();
        let px_at = b.len();
        b.extend([0x21u8; 32]); // image pixels
        let strips_at = b.len();
        b.extend((0x10 + px_at as u32).to_le_bytes());
        b.extend(32u32.to_le_bytes());
        b.extend([0u8; 8]);
        let piece_at = b.len();
        b.extend(0i16.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend((512u16 - 4).to_le_bytes()); // y = -4, square
        b.extend(((256u16 - 4) | 0x0800).to_le_bytes()); // x = -4, 8x8, last
        b.extend(0x3000u16.to_le_bytes()); // palette 3
        b.extend([0u8; 2]);
        let seq_at = b.len();
        b.extend([5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        b.extend([0u8; 12]);
        let frames_at = b.len();
        b.extend((0x10 + piece_at as u32).to_le_bytes());
        let offsets_at = b.len();
        b.extend([0u8; 16]);
        let dirs_at = b.len();
        for _ in 0..8 {
            b.extend((0x10 + seq_at as u32).to_le_bytes());
        }
        let groups_at = b.len();
        b.extend((0x10 + dirs_at as u32).to_le_bytes());
        let images_at = b.len();
        b.extend((0x10 + strips_at as u32).to_le_bytes());
        let hdr = b.len();
        for v in [frames_at, groups_at, 1, images_at, offsets_at] {
            let v = if v == 1 { 1 } else { 0x10 + v as u32 };
            b.extend(v.to_le_bytes());
        }
        let mut ptrs = vec![
            strips_at,
            frames_at,
            groups_at,
            images_at,
            hdr,
            hdr + 4,
            hdr + 12,
            hdr + 16,
        ];
        ptrs.extend((0..8).map(|i| dirs_at + i * 4));
        ptrs.sort();
        wrap(&b, hdr, &ptrs)
    }

    #[test]
    fn parses_and_renders() {
        let s = Sprite::parse(&synthetic()).unwrap();
        assert_eq!((s.frames.len(), s.images.len(), s.groups.len()), (1, 1, 1));
        assert_eq!(s.groups[0][7][0].duration, 5);
        let r = s.render_frame(0).unwrap();
        assert_eq!((r.image.width, r.image.height, r.origin), (8, 8, (4, 4)));
        assert_eq!(r.image.pixels[0], Some((3, 1)));
        assert!(s.render_frame(1).is_err());
        assert_eq!(species_of("ax025"), Some(25));
    }

    #[test]
    fn truncated_files_fail_cleanly() {
        let f = synthetic();
        for cut in [0x20, f.len() / 2, f.len() - 4] {
            assert!(Sprite::parse(&f[..cut]).is_err());
        }
    }
}
