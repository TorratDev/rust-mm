//! Nintendo DS "Nitro" compression: LZ10 (`0x10`), LZ11 (`0x11`), Huffman 4-bit (`0x20`),
//! Huffman 8-bit (`0x28`) and RLE (`0x30`).
//!
//! Each blob starts with a type byte and a 24-bit little-endian decompressed size. A zero
//! size means the real size is the 32-bit word that follows. Decoders stop at the declared
//! size and never write past it, and the output limit is checked before anything is
//! allocated, so a hostile header cannot force a large allocation.
//!
//! BLZ (backwards LZ, used for overlays) is a different format and lives in `rescue-nds`.
//! The bit order and length formulas follow GBATEK. They are checked against the dump in
//! the ROM-gated tests (`docs/formats/compression.md`), not only against the synthetic
//! vectors here.

use crate::error::FormatError;

/// Default output limit: 16 MiB.
pub const DEFAULT_MAX_OUTPUT: usize = 16 * 1024 * 1024;

/// A compression type, from the first byte of a blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Lz10,
    Lz11,
    Huffman4,
    Huffman8,
    Rle,
}

impl Codec {
    /// The codec for a type byte, or `None` if it is not one of the five.
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0x10 => Some(Codec::Lz10),
            0x11 => Some(Codec::Lz11),
            0x20 => Some(Codec::Huffman4),
            0x28 => Some(Codec::Huffman8),
            0x30 => Some(Codec::Rle),
            _ => None,
        }
    }
}

/// Decompresses a blob, using its first byte as the type, with [`DEFAULT_MAX_OUTPUT`].
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, FormatError> {
    decompress_limited(data, DEFAULT_MAX_OUTPUT)
}

/// Decompresses a blob, refusing a declared size above `max_out`.
pub fn decompress_limited(data: &[u8], max_out: usize) -> Result<Vec<u8>, FormatError> {
    let tag = byte(data, 0)?;
    let codec = Codec::from_tag(tag).ok_or(FormatError::UnknownCodec(tag))?;

    let small = u24_le(data)?;
    let (size, body_at) = if small == 0 {
        (u32_le(data, 4)? as usize, 8)
    } else {
        (small, 4)
    };
    if size > max_out {
        return Err(FormatError::TooLarge { size, max: max_out });
    }

    match codec {
        Codec::Lz10 => lz10(data, body_at, size),
        Codec::Lz11 => lz11(data, body_at, size),
        Codec::Huffman4 => huffman(data, body_at, 4, size),
        Codec::Huffman8 => huffman(data, body_at, 8, size),
        Codec::Rle => rle(data, body_at, size),
    }
}

/// LZ10: a flag byte covers 8 tokens (MSB first). `0` is a literal. `1` is two bytes:
/// length `(b0 >> 4) + 3`, distance `(((b0 & 0xF) << 8) | b1) + 1`.
fn lz10(src: &[u8], body_at: usize, size: usize) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::with_capacity(size);
    let mut pos = body_at;
    while out.len() < size {
        let flags = byte(src, pos)?;
        pos += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) != 0 {
                let b0 = byte(src, pos)?;
                let b1 = byte(src, pos + 1)?;
                pos += 2;
                let count = usize::from(b0 >> 4) + 3;
                let disp = ((usize::from(b0 & 0x0F) << 8) | usize::from(b1)) + 1;
                copy_back(&mut out, disp, count, size)?;
            } else {
                out.push(byte(src, pos)?);
                pos += 1;
            }
        }
    }
    Ok(out)
}

/// LZ11: like LZ10, but the first byte of a back-reference selects one of three lengths.
/// `>> 4 == 0`: 3 bytes, length `12 bits + 0x11`. `>> 4 == 1`: 4 bytes, length `16 bits +
/// 0x111`. Otherwise: 2 bytes, length `(b0 >> 4) + 1`. Distance is always `+ 1`.
fn lz11(src: &[u8], body_at: usize, size: usize) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::with_capacity(size);
    let mut pos = body_at;
    while out.len() < size {
        let flags = byte(src, pos)?;
        pos += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(byte(src, pos)?);
                pos += 1;
                continue;
            }
            let b0 = byte(src, pos)?;
            let (count, disp, used) = match b0 >> 4 {
                0 => {
                    let b1 = byte(src, pos + 1)?;
                    let b2 = byte(src, pos + 2)?;
                    let count = ((usize::from(b0 & 0x0F) << 4) | usize::from(b1 >> 4)) + 0x11;
                    let disp = ((usize::from(b1 & 0x0F) << 8) | usize::from(b2)) + 1;
                    (count, disp, 3)
                }
                1 => {
                    let b1 = byte(src, pos + 1)?;
                    let b2 = byte(src, pos + 2)?;
                    let b3 = byte(src, pos + 3)?;
                    let count = ((usize::from(b0 & 0x0F) << 12)
                        | (usize::from(b1) << 4)
                        | usize::from(b2 >> 4))
                        + 0x111;
                    let disp = ((usize::from(b2 & 0x0F) << 8) | usize::from(b3)) + 1;
                    (count, disp, 4)
                }
                ind => {
                    let b1 = byte(src, pos + 1)?;
                    let count = usize::from(ind) + 1;
                    let disp = ((usize::from(b0 & 0x0F) << 8) | usize::from(b1)) + 1;
                    (count, disp, 2)
                }
            };
            pos += used;
            copy_back(&mut out, disp, count, size)?;
        }
    }
    Ok(out)
}

/// Huffman (4- or 8-bit symbols). The tree size byte is at `body_at`; the tree table is
/// `(size + 1) * 2` bytes, with the root node right after the size byte. A node byte has
/// the child offset in bits 0–5, the left-child leaf flag in bit 6 and the right-child leaf
/// flag in bit 7. The bitstream is read as 32-bit little-endian words, MSB first. In 4-bit
/// mode each output byte packs two symbols, low nibble first.
fn huffman(src: &[u8], body_at: usize, sym_bits: u8, size: usize) -> Result<Vec<u8>, FormatError> {
    let tree_len = (usize::from(byte(src, body_at)?) + 1) * 2;
    let root = body_at + 1;
    let stream_at = body_at + tree_len;

    let mut out = Vec::with_capacity(size);
    let mut stream = BitReader::new(src, stream_at);
    let mut pending: Option<u8> = None;
    while out.len() < size {
        let sym = decode_symbol(src, root, &mut stream)?;
        if sym_bits == 8 {
            out.push(sym);
        } else {
            let nib = sym & 0x0F;
            match pending.take() {
                Some(lo) => out.push(lo | (nib << 4)),
                None => pending = Some(nib),
            }
        }
    }
    Ok(out)
}

fn decode_symbol(src: &[u8], root: usize, bits: &mut BitReader<'_>) -> Result<u8, FormatError> {
    let mut node = root;
    loop {
        let node_byte = byte(src, node)?;
        let right = bits.next_bit()?;
        let leaf_flag = if right { 0x80 } else { 0x40 };
        let child = (node & !1) + (usize::from(node_byte & 0x3F) << 1) + 2 + usize::from(right);
        if node_byte & leaf_flag != 0 {
            return byte(src, child);
        }
        if child <= node {
            return Err(FormatError::Malformed("Huffman tree loops"));
        }
        node = child;
    }
}

struct BitReader<'a> {
    src: &'a [u8],
    pos: usize,
    word: u32,
    left: u32,
}

impl<'a> BitReader<'a> {
    fn new(src: &'a [u8], pos: usize) -> Self {
        BitReader {
            src,
            pos,
            word: 0,
            left: 0,
        }
    }

    fn next_bit(&mut self) -> Result<bool, FormatError> {
        if self.left == 0 {
            let w = u32_le(self.src, self.pos)?;
            self.pos += 4;
            self.word = w;
            self.left = 32;
        }
        let bit = self.word >> 31 != 0;
        self.word <<= 1;
        self.left -= 1;
        Ok(bit)
    }
}

/// RLE: a flag byte. Bit 7 set means a run of `(flag & 0x7F) + 3` copies of the next byte.
/// Bit 7 clear means `(flag & 0x7F) + 1` literal bytes.
fn rle(src: &[u8], body_at: usize, size: usize) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::with_capacity(size);
    let mut pos = body_at;
    while out.len() < size {
        let flag = byte(src, pos)?;
        pos += 1;
        let room = size - out.len();
        if flag & 0x80 != 0 {
            let count = usize::from(flag & 0x7F) + 3;
            let v = byte(src, pos)?;
            pos += 1;
            out.extend(std::iter::repeat_n(v, count.min(room)));
        } else {
            let count = usize::from(flag & 0x7F) + 1;
            let lit = slice(src, pos, count)?;
            pos += count;
            out.extend_from_slice(lit.get(..count.min(room)).unwrap_or_default());
        }
    }
    Ok(out)
}

/// Appends `count` bytes copied from `disp` bytes back, stopping at `size`.
fn copy_back(out: &mut Vec<u8>, disp: usize, count: usize, size: usize) -> Result<(), FormatError> {
    if disp > out.len() {
        return Err(FormatError::Malformed(
            "back-reference before start of output",
        ));
    }
    for _ in 0..count.min(size - out.len()) {
        let b = out[out.len() - disp];
        out.push(b);
    }
    Ok(())
}

fn byte(src: &[u8], at: usize) -> Result<u8, FormatError> {
    src.get(at).copied().ok_or(FormatError::Truncated {
        what: "compressed stream",
        need: at + 1,
        have: src.len(),
    })
}

fn slice(src: &[u8], at: usize, len: usize) -> Result<&[u8], FormatError> {
    src.get(at..at + len).ok_or(FormatError::Truncated {
        what: "compressed stream",
        need: at + len,
        have: src.len(),
    })
}

fn u24_le(src: &[u8]) -> Result<usize, FormatError> {
    let s = slice(src, 1, 3)?;
    Ok(usize::from(s[0]) | usize::from(s[1]) << 8 | usize::from(s[2]) << 16)
}

fn u32_le(src: &[u8], at: usize) -> Result<u32, FormatError> {
    let s = slice(src, at, 4)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(tag: u8, size: usize) -> Vec<u8> {
        let s = size as u32;
        vec![tag, s as u8, (s >> 8) as u8, (s >> 16) as u8]
    }

    #[test]
    fn lz10_literals_and_backref() {
        // "abcabcabc" = literal a, b, c, then a back-reference of 6 bytes at distance 3.
        // Flags: 0 0 0 1 in the first byte, i.e. 0x10.
        let mut blob = header(0x10, 9);
        blob.extend_from_slice(&[0x10, b'a', b'b', b'c', 0x30, 0x02]);
        assert_eq!(decompress(&blob).unwrap(), b"abcabcabc");
    }

    #[test]
    fn lz11_two_byte_form() {
        // Flags 0x20: tokens 1 and 2 are literals, token 3 is a back-reference.
        // Back-reference b0 = 0x20 => count = 2 + 1 = 3; b1 = 0x01 => distance 2.
        let mut blob = header(0x11, 5);
        blob.extend_from_slice(&[0x20, b'x', b'y', 0x20, 0x01]);
        assert_eq!(decompress(&blob).unwrap(), b"xyxyx");
    }

    #[test]
    fn lz11_three_byte_form() {
        // Flags 0x40: token 1 literal, token 2 back-reference. b0 = 0x00 selects the 3-byte
        // form, count = 0x11 + 0 = 17, distance = 0 + 1 = 1.
        let mut blob = header(0x11, 18);
        blob.extend_from_slice(&[0x40, b'a', 0x00, 0x00, 0x00]);
        assert_eq!(decompress(&blob).unwrap(), vec![b'a'; 18]);
    }

    #[test]
    fn rle_runs_and_literals() {
        // Run of 5 'Q' (0x80 | 2 + 3), literal run of 2 bytes ("hi": flag 0x01).
        let mut blob = header(0x30, 7);
        blob.extend_from_slice(&[0x82, b'Q', 0x01, b'h', b'i']);
        assert_eq!(decompress(&blob).unwrap(), b"QQQQQhi");
    }

    /// Tree with one root whose children are both leaves: 0 -> 0x11, 1 -> 0x22.
    /// Table: size byte 1 (4 bytes total), root node 0xC0 (both leaf flags) at 5, leaves at
    /// 6 and 7. The bitstream starts at 8.
    const TWO_LEAF_TREE: [u8; 4] = [0x01, 0xC0, 0x11, 0x22];

    #[test]
    fn huffman8_two_leaf_tree() {
        // Bits 0 1 1 0 0 1 0 1 (MSB first in the word) => 0x11 0x22 0x22 0x11 0x11 0x22 0x11 0x22.
        let mut blob = header(0x28, 8);
        blob.extend_from_slice(&TWO_LEAF_TREE);
        blob.extend_from_slice(&[0x00, 0x00, 0x00, 0x65]);
        assert_eq!(
            decompress(&blob).unwrap(),
            [0x11, 0x22, 0x22, 0x11, 0x11, 0x22, 0x11, 0x22]
        );
    }

    #[test]
    fn huffman4_packs_two_symbols_per_byte() {
        // Bits 0 1 1 0 => symbols 1 2 2 1, packed low nibble first: 0x21, 0x12.
        let mut blob = header(0x20, 2);
        blob.extend_from_slice(&TWO_LEAF_TREE);
        blob.extend_from_slice(&[0x00, 0x00, 0x00, 0x60]);
        assert_eq!(decompress(&blob).unwrap(), [0x21, 0x12]);
    }

    #[test]
    fn rejects_unknown_codec_and_truncation() {
        assert_eq!(
            decompress(&[0x99, 1, 0, 0]),
            Err(FormatError::UnknownCodec(0x99))
        );
        let blob = header(0x10, 100);
        assert!(matches!(
            decompress(&blob),
            Err(FormatError::Truncated { .. })
        ));
    }

    #[test]
    fn rejects_oversized_declared_output_before_allocating() {
        // The 24-bit size field tops out below the 16 MiB default, so test a small limit.
        let blob = header(0x30, 100);
        assert!(matches!(
            decompress_limited(&blob, 50),
            Err(FormatError::TooLarge { size: 100, max: 50 })
        ));
    }

    #[test]
    fn rejects_backref_before_start() {
        let mut blob = header(0x10, 4);
        blob.extend_from_slice(&[0x80, 0x00, 0x05]);
        assert!(matches!(decompress(&blob), Err(FormatError::Malformed(_))));
    }

    #[test]
    fn extended_size_field() {
        // Size 0 in the 24-bit field means a 32-bit size follows (here 3 literals).
        let mut blob = vec![0x30, 0, 0, 0, 3, 0, 0, 0];
        blob.extend_from_slice(&[0x02, b'a', b'b', b'c']);
        assert_eq!(decompress(&blob).unwrap(), b"abc");
    }
}
