//! Portraits (`monster.sbin` `kaoNNN`): the dialogue face pictures, one file per
//! species, up to 13 emotions each. Spec: `docs/formats/portraits.md`.
//!
//! SIR0 content header: 13 pairs `(palette pointer, image pointer)`, one per emotion; a
//! missing emotion has both pointers 0. The palette is 16 RGBX colors; the image is an
//! AT4PX holding 40x40 pixels as 5x5 4bpp tiles, row-major.

use crate::gfx::MapEntry;
use crate::graphics::{self, Depth, IndexedImage, Palette};
use crate::px;
use crate::read::{Error, Result};
use crate::sir0::Sir0;

/// Emotion slots per portrait file.
pub const EMOTIONS: usize = 13;
/// Portrait width and height in pixels.
pub const SIZE: usize = 40;

/// Emotion names by slot. They follow Red's `EMOTION_*` constants and match what the
/// pictures show; treat them as `[red-hypothesis]` until Blue's code is read.
pub const EMOTION_NAMES: [&str; EMOTIONS] = [
    "normal",
    "happy",
    "pained",
    "angry",
    "worried",
    "sad",
    "crying",
    "shouting",
    "teary-eyed",
    "determined",
    "joyous",
    "inspired",
    "surprised",
];

/// One decoded portrait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Portrait {
    /// Emotion slot (index into [`EMOTION_NAMES`]).
    pub emotion: usize,
    /// 16-color palette; index 0 is opaque (portraits fill their box).
    pub palette: Palette,
    /// 40x40 image, palette number 0.
    pub image: IndexedImage,
}

impl Portrait {
    /// RGBA8 pixels.
    pub fn to_rgba(&self) -> Vec<u8> {
        self.image.to_rgba(std::slice::from_ref(&self.palette))
    }
}

/// Species number from a `kaoNNN` sub-file name.
pub fn species_of(name: &str) -> Option<u16> {
    name.strip_prefix("kao")?.parse().ok()
}

/// Decodes every present emotion of a portrait file.
pub fn parse(data: &[u8]) -> Result<Vec<Portrait>> {
    let s = Sir0::parse(data)?;
    let mut out = Vec::new();
    for emotion in 0..EMOTIONS {
        let at = s.data_offset + emotion * 8;
        let pal = s.ptr(at, "portrait palette pointer")?;
        let img = s.ptr(at + 4, "portrait image pointer")?;
        if pal == 0 && img == 0 {
            continue;
        }
        let palette = graphics::rgbx_palette(s.tail(pal, "portrait palette")?, 16)?;
        let pixels = px::decompress(s.tail(img, "portrait image")?, px::DEFAULT_LIMIT)?;
        if pixels.len() < SIZE * SIZE / 2 {
            return Err(Error::invalid(
                "portrait",
                format!("image is {} bytes, need 800", pixels.len()),
            ));
        }
        let mut image = IndexedImage::new(SIZE, SIZE);
        for t in 0..25u16 {
            let e = MapEntry {
                tile: t,
                hflip: false,
                vflip: false,
                palette: 0,
            };
            image.draw_tile(
                &pixels,
                Depth::Bpp4,
                e,
                (t % 5) as isize * 8,
                (t / 5) as isize * 8,
                false,
            )?;
        }
        out.push(Portrait {
            emotion,
            palette,
            image,
        });
    }
    Ok(out)
}

/// The portrait palette with color 0 made transparent (for overlaying on UI).
pub fn transparent0(palette: &Palette) -> Palette {
    let mut p = palette.clone();
    if let Some(c) = p.first_mut() {
        *c = [0, 0, 0, 0];
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px::tests::literal_at4px;
    use crate::sir0::tests::wrap;

    #[test]
    fn decodes_a_synthetic_portrait() {
        // Body: palette at 0, image at 64, header of 13 pairs after it.
        let mut body = vec![0u8; 64];
        body[4..8].copy_from_slice(&[10, 20, 30, 0x80]); // color 1
        let img = literal_at4px(&[0x11; 800]);
        body.extend(&img);
        while !body.len().is_multiple_of(4) {
            body.push(0);
        }
        let hdr = body.len();
        body.extend(0x10u32.to_le_bytes());
        body.extend((0x10 + 64u32).to_le_bytes());
        body.extend([0u8; 8 * 12]);
        let f = wrap(&body, hdr, &[hdr, hdr + 4]);
        let p = parse(&f).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].image.pixels[0], Some((0, 1)));
        assert_eq!(&p[0].to_rgba()[..4], &[10, 20, 30, 255]);
        assert_eq!(species_of("kao025"), Some(25));
    }
}
