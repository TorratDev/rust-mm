//! Effects (`effect.sbin`). Spec: `docs/formats/effects.md`.
//!
//! `efobNNN` are effect objects. They use the creature sprite layout ([`crate::sprite`]):
//! pose pieces, 8-direction animation groups, no images and no reference points. Their
//! 4bpp tiles and one 16-color palette sit in the file, so an object is a [`Sprite`] with
//! its tiles in [`Sprite::tiles`]. SIR0 content header of eight `u32`: pose table,
//! animation table, animation count, two pointers that are always 0, tile data, palette,
//! tile count.
//!
//! `efbgNNN` are backgrounds: a 256-color palette, 4bpp tiles, and frames that hold a
//! 10-value header and a tilemap. SIR0 content header of five `u32`: frame count, frame
//! table, tile count minus one, tile data, palette.

use crate::graphics::{Palette, TILE_4BPP, rgbx_palette};
use crate::read::{self as bytes, Error, Result};
use crate::sir0::Sir0;
use crate::sprite::{AnimStep, DIRECTIONS, Sprite, read_pieces, read_steps};

/// Sanity bounds on counts read from the file (not format limits).
const MAX_POSES: usize = 4096;
const MAX_ANIMS: usize = 256;
const MAX_TILES: usize = 2048;
const MAX_FRAMES: usize = 1024;
/// Bytes of an object palette (16 RGBX colors) and of a background palette (256).
const OBJECT_PALETTE: usize = 64;
const BACKGROUND_PALETTE: usize = 1024;
/// Bytes of a background frame's header (10 `u16`).
const FRAME_HEADER: usize = 20;

/// An effect object: a sprite (poses, animations, tiles) and its palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    /// Poses as frames, animation groups, and the tiles (`sprite.tiles`). Pieces never
    /// name an image, and `offsets` is empty: effect objects have no reference points.
    pub sprite: Sprite,
    /// The object's palette, used for every piece.
    pub palette: Palette,
}

/// A background frame: a 10-value header (meanings unknown) and its tilemap entries
/// ([`crate::gfx::MapEntry`] layout).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundFrame {
    /// Header values, meanings unknown.
    pub header: [u16; 10],
    /// Tilemap entries, row by row.
    pub map: Vec<u16>,
}

/// An effect background.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    /// Frames in table order; `None` where the frame pointer is null.
    pub frames: Vec<Option<BackgroundFrame>>,
    /// 4bpp tiles, `tile count + 1` of them.
    pub tiles: Vec<u8>,
    /// 256 colors: 16 banks of 16, selected by a tilemap entry's palette bits.
    pub palette: Palette,
}

/// Parses an `efobNNN` effect object.
pub fn parse_object(data: &[u8]) -> Result<Object> {
    let s = Sir0::parse(data)?;
    let h = s.data_offset;
    let poses_at = s.ptr(h, "effect object header")?;
    let anims_at = s.ptr(h + 4, "effect object header")?;
    let anim_count = s.u32(h + 8, "effect object header")? as usize;
    if s.u32(h + 12, "effect object header")? != 0 || s.u32(h + 16, "effect object header")? != 0 {
        return Err(Error::invalid(
            "effect object",
            "sprite data or positions present",
        ));
    }
    let tiles_at = s.ptr(h + 20, "effect object header")?;
    let palette_at = s.ptr(h + 24, "effect object header")?;
    let tile_count = s.u32(h + 28, "effect object header")? as usize;
    if anim_count > MAX_ANIMS || tile_count > MAX_TILES {
        return Err(Error::invalid(
            "effect object",
            format!("{anim_count} animations, {tile_count} tiles"),
        ));
    }
    let blocks = (0..anim_count)
        .map(|g| s.ptr(anims_at + g * 4, "effect animation table"))
        .collect::<Result<Vec<_>>>()?;
    // Neither the pose pointer table nor its length is stored. The table runs up to the
    // first table that follows it: the animation blocks, the animation table or the data.
    let pose_end = blocks
        .iter()
        .copied()
        .chain([anims_at, tiles_at, palette_at, h])
        .filter(|&t| t > poses_at)
        .min()
        .unwrap_or(data.len());
    let pose_count = (pose_end - poses_at) / 4;
    if pose_count > MAX_POSES {
        return Err(Error::invalid(
            "effect object",
            format!("{pose_count} poses"),
        ));
    }
    let mut frames = Vec::with_capacity(pose_count);
    for i in 0..pose_count {
        let at = s.ptr(poses_at + i * 4, "effect pose table")?;
        frames.push(if at == 0 {
            Vec::new()
        } else {
            read_pieces(data, at)?
        });
    }
    if frames.iter().flatten().any(|p| p.image.is_some()) {
        return Err(Error::invalid("effect object", "a piece names an image"));
    }
    let mut groups = Vec::with_capacity(anim_count);
    for block in blocks {
        let mut dirs: [Vec<AnimStep>; DIRECTIONS] = Default::default();
        if block != 0 {
            for (d, seq) in dirs.iter_mut().enumerate() {
                let at = s.ptr(block + d * 4, "effect animation block")?;
                if at != 0 {
                    *seq = read_steps(data, at, frames.len())?;
                }
            }
        }
        groups.push(dirs);
    }
    let tiles = bytes::slice(data, tiles_at, tile_count * TILE_4BPP, "effect tiles")?.to_vec();
    let palette = rgbx_palette(
        bytes::slice(data, palette_at, OBJECT_PALETTE, "effect palette")?,
        16,
    )?;
    Ok(Object {
        sprite: Sprite {
            frames,
            offsets: Vec::new(),
            images: Vec::new(),
            groups,
            tiles,
        },
        palette,
    })
}

/// Parses an `efbgNNN` effect background.
pub fn parse_background(data: &[u8]) -> Result<Background> {
    let s = Sir0::parse(data)?;
    let h = s.data_offset;
    let frame_count = s.u32(h, "effect background header")? as usize;
    let frames_at = s.ptr(h + 4, "effect background header")?;
    let tile_count = s.u32(h + 8, "effect background header")? as usize;
    let tiles_at = s.ptr(h + 12, "effect background header")?;
    let palette_at = s.ptr(h + 16, "effect background header")?;
    if frame_count > MAX_FRAMES || tile_count >= MAX_TILES {
        return Err(Error::invalid(
            "effect background",
            format!("{frame_count} frames, {} tiles", tile_count + 1),
        ));
    }
    let ptrs = (0..frame_count)
        .map(|i| s.ptr(frames_at + i * 4, "effect frame table"))
        .collect::<Result<Vec<_>>>()?;
    // A frame's tilemap runs to the next table or frame start, which is not stored.
    let mut starts: Vec<usize> = ptrs
        .iter()
        .copied()
        .filter(|&p| p != 0)
        .chain([frames_at, tiles_at, palette_at, h])
        .collect();
    starts.sort_unstable();
    let frames = ptrs
        .iter()
        .map(|&at| {
            if at == 0 {
                return Ok(None);
            }
            let end = starts
                .iter()
                .copied()
                .find(|&t| t > at)
                .unwrap_or(data.len());
            let body = end
                .checked_sub(at + FRAME_HEADER)
                .ok_or_else(|| Error::invalid("effect frame", "frame ends inside its header"))?;
            let mut header = [0u16; 10];
            for (k, v) in header.iter_mut().enumerate() {
                *v = bytes::u16_at(data, at + 2 * k, "effect frame header")?;
            }
            let map = bytes::slice(data, at + FRAME_HEADER, body, "effect frame map")?
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(Some(BackgroundFrame { header, map }))
        })
        .collect::<Result<Vec<_>>>()?;
    let tiles =
        bytes::slice(data, tiles_at, (tile_count + 1) * TILE_4BPP, "effect tiles")?.to_vec();
    let palette = rgbx_palette(
        bytes::slice(data, palette_at, BACKGROUND_PALETTE, "effect palette")?,
        256,
    )?;
    Ok(Background {
        frames,
        tiles,
        palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sir0::tests::wrap;

    /// One object: one 8x8 pose piece (no image, tile 0), one animation group whose eight
    /// directions each hold one step, one tile of `0x21` bytes, a 16-color palette.
    fn synthetic_object(piece_image: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0xD8];
        let put =
            |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let base = 0x10;
        put(&mut b, 0x00, base + 0x20); // pose table
        put(&mut b, 0x04, base + 0x24); // animation table
        put(&mut b, 0x08, 1); // one animation group
        put(&mut b, 0x14, base + 0x78); // tiles
        put(&mut b, 0x18, base + 0x98); // palette
        put(&mut b, 0x1C, 1); // one tile
        put(&mut b, 0x20, base + 0x30); // pose 0
        put(&mut b, 0x24, base + 0x40); // group 0
        for d in 0..DIRECTIONS {
            put(&mut b, 0x40 + 4 * d, base + 0x60);
        }
        b[0x30..0x32].copy_from_slice(&piece_image.to_le_bytes());
        b[0x36..0x38].copy_from_slice(&(252u16 | 0x0800).to_le_bytes()); // x = -4, last
        b[0x34..0x36].copy_from_slice(&508u16.to_le_bytes()); // y = -4
        b[0x60] = 5; // one step of five frames, then the zero terminator
        b[0x78..0x98].fill(0x21);
        for k in 0..16 {
            b[0x98 + 4 * k..0x9C + 4 * k].copy_from_slice(&[k as u8, 0, 0, 0x80]);
        }
        let ptrs: Vec<usize> = [0x00, 0x04, 0x14, 0x18, 0x20, 0x24]
            .into_iter()
            .chain((0..8).map(|d| 0x40 + 4 * d))
            .collect();
        wrap(&b, 0, &ptrs)
    }

    #[test]
    fn parses_object_and_renders_pose() {
        let o = parse_object(&synthetic_object(0xFFFF)).unwrap();
        assert_eq!((o.sprite.frames.len(), o.sprite.groups.len()), (1, 1));
        assert_eq!(o.sprite.groups[0][7][0].duration, 5);
        assert_eq!((o.sprite.tiles.len(), o.palette.len()), (32, 16));
        let r = o.sprite.render_frame(0).unwrap();
        assert_eq!((r.image.width, r.image.height), (8, 8));
        assert_eq!(r.image.pixels[0], Some((0, 1)));
    }

    #[test]
    fn rejects_pieces_that_name_images() {
        assert!(parse_object(&synthetic_object(0)).is_err());
    }

    #[test]
    fn truncated_files_fail_cleanly() {
        let f = synthetic_object(0xFFFF);
        for cut in [0x20, f.len() / 2, f.len() - 4] {
            assert!(parse_object(&f[..cut]).is_err());
            assert!(parse_background(&f[..cut]).is_err());
        }
    }
}
