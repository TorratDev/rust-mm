//! PX compression in its AT4PX and PKDPX containers. Spec:
//! `docs/formats/compression.md#px`.
//!
//! Header: 5-byte magic (`AT4PX` / `PKDPX`), `u16` container length (header included),
//! 9 control nibbles, then the decoded size (`u16` for AT4PX, `u32` for PKDPX). Data
//! follows: a command byte per 8 items, MSB first. A set bit copies one literal byte.
//! A clear bit reads a byte `b` with high nibble `h` and low nibble `l`:
//!
//! * `h` equals control nibble `i`: emit a 4-nibble pattern as 2 bytes. For `i == 0` all
//!   four nibbles are `l`; for `i` in 1..=4 all are `l` except nibble `i-1`, which is
//!   `l-1`; for `i` in 5..=8 all are `l` except nibble `i-5`, which is `l+1` (mod 16).
//!   Nibbles pack big-end first: bytes `n0<<4|n1`, `n2<<4|n3`.
//! * otherwise: a back-reference of `h + 3` bytes from `((l << 8) | next) - 0x1000`
//!   bytes before the current end of output.
//!
//! The pattern rule was settled against Blue's portraits (the alternatives leave stray
//! pixels); see the spec.

use crate::read::{self as bytes, Error, Result};

/// AT4PX magic.
pub const AT4PX: &[u8; 5] = b"AT4PX";
/// PKDPX magic.
pub const PKDPX: &[u8; 5] = b"PKDPX";

/// Default cap on decoded size for PKDPX (AT4PX is capped at 64 KiB by its format).
pub const DEFAULT_LIMIT: usize = 4 << 20;

/// Container kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `AT4PX`, 16-bit decoded size, header 0x12 bytes.
    At4px,
    /// `PKDPX`, 32-bit decoded size, header 0x14 bytes.
    Pkdpx,
}

/// Parsed container header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Container kind.
    pub kind: Kind,
    /// Container length including the header.
    pub length: usize,
    /// The 9 control nibbles.
    pub flags: [u8; 9],
    /// Decoded size.
    pub decoded_size: usize,
    /// Offset of the compressed stream.
    pub data_offset: usize,
}

/// Container kind of `data`, if it starts with a PX magic.
pub fn kind(data: &[u8]) -> Option<Kind> {
    if data.starts_with(AT4PX) {
        Some(Kind::At4px)
    } else if data.starts_with(PKDPX) {
        Some(Kind::Pkdpx)
    } else {
        None
    }
}

/// Parses the container header.
pub fn header(data: &[u8]) -> Result<Header> {
    let kind = kind(data).ok_or_else(|| Error::invalid("PX", "missing AT4PX/PKDPX magic"))?;
    let length = usize::from(bytes::u16_at(data, 5, "PX header")?);
    let flags = bytes::array::<9>(data, 7, "PX header")?;
    let (decoded_size, data_offset) = match kind {
        Kind::At4px => (usize::from(bytes::u16_at(data, 16, "PX header")?), 0x12),
        Kind::Pkdpx => (bytes::offset_at(data, 16, "PX header")?, 0x14),
    };
    if length < data_offset || length > data.len() {
        return Err(Error::invalid(
            "PX header",
            format!(
                "length {length} does not fit the data ({} bytes)",
                data.len()
            ),
        ));
    }
    if flags.iter().any(|&f| f > 0xF) {
        return Err(Error::invalid("PX header", "control flag above 0xF"));
    }
    Ok(Header {
        kind,
        length,
        flags,
        decoded_size,
        data_offset,
    })
}

/// Decodes an AT4PX or PKDPX container. Output larger than `limit` is refused.
pub fn decompress(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    const WHAT: &str = "PX data";
    let h = header(data)?;
    if h.decoded_size > limit {
        return Err(Error::TooLarge {
            size: h.decoded_size,
            max: limit,
        });
    }
    let src = &data[..h.length];
    let mut pos = h.data_offset;
    let mut out = Vec::with_capacity(h.decoded_size);
    let next = |pos: &mut usize| -> Result<u8> {
        let b = bytes::u8_at(src, *pos, WHAT)?;
        *pos += 1;
        Ok(b)
    };
    while out.len() < h.decoded_size {
        let cmd = next(&mut pos)?;
        for bit in (0..8).rev() {
            if out.len() >= h.decoded_size {
                break;
            }
            if cmd & (1 << bit) != 0 {
                out.push(next(&mut pos)?);
                continue;
            }
            let b = next(&mut pos)?;
            let (hi, lo) = (b >> 4, b & 0xF);
            if let Some(i) = h.flags.iter().position(|&f| f == hi) {
                let mut n = [lo; 4];
                match i {
                    0 => {}
                    1..=4 => n[i - 1] = lo.wrapping_sub(1) & 0xF,
                    _ => n[i - 5] = (lo + 1) & 0xF,
                }
                out.push(n[0] << 4 | n[1]);
                out.push(n[2] << 4 | n[3]);
            } else {
                let dist = 0x1000 - (usize::from(lo) << 8 | usize::from(next(&mut pos)?));
                let start = out.len().checked_sub(dist).ok_or_else(|| {
                    Error::invalid(WHAT, "back-reference before the start of output")
                })?;
                for k in 0..usize::from(hi) + 3 {
                    let v = out[start + k];
                    out.push(v);
                }
            }
        }
    }
    // A pattern can write one byte past an odd decoded size; the game sizes buffers by
    // the header, so trim to it.
    out.truncate(h.decoded_size);
    Ok(out)
}

/// Decodes `data` if it is a PX container, otherwise returns it unchanged.
pub fn maybe_decompress(data: &[u8], limit: usize) -> Result<std::borrow::Cow<'_, [u8]>> {
    Ok(match kind(data) {
        Some(_) => std::borrow::Cow::Owned(decompress(data, limit)?),
        None => std::borrow::Cow::Borrowed(data),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Encodes `raw` as an AT4PX of literals only (a valid, if useless, PX stream).
    pub(crate) fn literal_at4px(raw: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        for chunk in raw.chunks(8) {
            body.push(0xFFu8 << (8 - chunk.len()));
            body.extend_from_slice(chunk);
        }
        let mut out = AT4PX.to_vec();
        out.extend(((0x12 + body.len()) as u16).to_le_bytes());
        out.extend([1, 2, 3, 4, 5, 6, 7, 8, 9]);
        out.extend((raw.len() as u16).to_le_bytes());
        out.extend(body);
        out
    }

    #[test]
    fn literal_round_trip() {
        let raw: Vec<u8> = (0..100u8).collect();
        assert_eq!(
            decompress(&literal_at4px(&raw), DEFAULT_LIMIT).unwrap(),
            raw
        );
    }

    #[test]
    fn patterns_and_back_references() {
        // flags: index 0 -> nibble 1, index 1 -> 2, index 5 -> 6, others unused (0xF).
        let mut c = AT4PX.to_vec();
        let body = [
            0b1000_0000u8, // literal, then 7 non-literals
            0xAB,          // literal
            0x17,          // flag 0 (nibble 1): all 7 -> 77 77
            0x27,          // flag 1 (nibble 2): n0 = 6 -> 67 77
            0x67,          // flag 5 (nibble 6): n0 = 8 -> 87 77
            0x0F,          // back-reference: len 3, dist 0x1000 - 0xFFE = 2
            0xFE,
        ];
        c.extend(((0x12 + body.len()) as u16).to_le_bytes());
        c.extend([1, 2, 0xF, 0xF, 0xF, 6, 0xF, 0xF, 0xF]);
        c.extend(10u16.to_le_bytes());
        c.extend(body);
        assert_eq!(
            decompress(&c, DEFAULT_LIMIT).unwrap(),
            [0xAB, 0x77, 0x77, 0x67, 0x77, 0x87, 0x77, 0x87, 0x77, 0x87]
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(decompress(b"AT4PX", DEFAULT_LIMIT).is_err());
        let mut c = literal_at4px(&[1, 2, 3]);
        c[5] = 0xFF; // length past the end
        assert!(decompress(&c, DEFAULT_LIMIT).is_err());
        let c = literal_at4px(&[0; 64]);
        assert!(matches!(decompress(&c, 10), Err(Error::TooLarge { .. })));
        // Back-reference before the start.
        let mut c = AT4PX.to_vec();
        c.extend(0x15u16.to_le_bytes());
        c.extend([0xF; 9]);
        c.extend(4u16.to_le_bytes());
        c.extend([0x00, 0x0F, 0xFF]);
        assert!(decompress(&c, DEFAULT_LIMIT).is_err());
    }
}
