//! BLZ ("backwards LZ") decompression, used for ARM9 and overlay code.
//!
//! The compressed stream is read from the end of the buffer toward the start, and the
//! output is written from the end too, so the expansion can happen in place. An 8-byte
//! footer at the end holds:
//!
//! - `0x00` (`u32`): low 24 bits = compressed length including the footer; high 8 bits =
//!   footer length (normally 8);
//! - `0x04` (`u32`): bytes the data grows by when decompressed.
//!
//! Each flag byte covers 8 tokens, most significant bit first. A `0` bit is one literal
//! byte. A `1` bit is a back-reference of two bytes: length `(b0 >> 4) + 3` and distance
//! `(((b0 & 0xF) << 8) | b1) + 3`.

use crate::error::NdsError;
use crate::read;

/// Default output limit. Overlays are far smaller; this stops hostile input from
/// allocating without bound.
pub const MAX_OUTPUT: usize = 16 * 1024 * 1024;

/// Decompresses a BLZ-packed buffer with the default output limit.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, NdsError> {
    decompress_limited(data, MAX_OUTPUT)
}

/// Decompresses a BLZ-packed buffer, refusing outputs larger than `max_out` bytes.
pub fn decompress_limited(data: &[u8], max_out: usize) -> Result<Vec<u8>, NdsError> {
    let len = data.len();
    let word0 = read::u32_le(data, len.saturating_sub(8), "BLZ footer")?;
    let grow = read::u32_le(data, len.saturating_sub(4), "BLZ footer")? as usize;
    let header_len = (word0 >> 24) as usize;
    let enc_len = (word0 & 0x00FF_FFFF) as usize;

    if header_len != 8 || enc_len < header_len || enc_len > len {
        return Err(NdsError::Malformed("BLZ footer"));
    }
    let total = len
        .checked_add(grow)
        .ok_or(NdsError::Malformed("BLZ size overflow"))?;
    if total > max_out {
        return Err(NdsError::TooLarge {
            size: total,
            max: max_out,
        });
    }

    // Bytes before `stop` are stored as-is. Everything after it is compressed.
    let stop = len - enc_len;
    let mut out = vec![0u8; total];
    out[..stop].copy_from_slice(read::bytes(data, 0, stop, "BLZ prefix")?);

    let mut src = len - header_len; // next byte to read, moving down
    let mut dst = total; // output is complete when dst reaches stop
    let mut flags = 0u8;
    let mut mask = 0u8; // 0 means the next token needs a new flag byte

    while dst > stop {
        if mask == 0 {
            if src <= stop {
                return Err(NdsError::Malformed("BLZ stream ended early"));
            }
            src -= 1;
            flags = read::u8_at(data, src, "BLZ flags")?;
            mask = 0x80;
        }

        if flags & mask != 0 {
            if src < stop + 2 {
                return Err(NdsError::Malformed("BLZ stream ended early"));
            }
            let b0 = read::u8_at(data, src - 1, "BLZ token")?;
            let b1 = read::u8_at(data, src - 2, "BLZ token")?;
            src -= 2;
            let count = usize::from(b0 >> 4) + 3;
            let disp = ((usize::from(b0 & 0x0F) << 8) | usize::from(b1)) + 3;
            if count > dst - stop {
                return Err(NdsError::Malformed("BLZ copy past start of data"));
            }
            if dst + disp > total {
                return Err(NdsError::Malformed("BLZ distance out of range"));
            }
            for _ in 0..count {
                dst -= 1;
                out[dst] = out[dst + disp];
            }
        } else {
            if src <= stop {
                return Err(NdsError::Malformed("BLZ stream ended early"));
            }
            src -= 1;
            dst -= 1;
            out[dst] = read::u8_at(data, src, "BLZ literal")?;
        }
        mask >>= 1;
    }

    if dst != stop {
        return Err(NdsError::Malformed("BLZ output size mismatch"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Footer-terminated blob: `body` followed by the footer for `grow` extra bytes.
    fn with_footer(body: &[u8], grow: u32) -> Vec<u8> {
        let enc = (body.len() + 8) as u32;
        let mut v = body.to_vec();
        v.extend_from_slice(&((8u32 << 24) | enc).to_le_bytes());
        v.extend_from_slice(&grow.to_le_bytes());
        v
    }

    #[test]
    fn back_reference_expands_in_place() {
        // Plaintext "abc" * 8 (24 bytes). Tokens from the end: 3 literals, one back-reference
        // of 18 bytes at distance 3, then 3 literals. Flag bits: 0 0 0 1 0 0 0.
        let body = [b'a', b'b', b'c', 0x00, 0xF0, b'a', b'b', b'c', 0x10];
        let blob = with_footer(&body, 24 - 17);
        assert_eq!(decompress(&blob).unwrap(), b"abc".repeat(8));
    }

    #[test]
    fn rejects_bad_footer_and_oversized_output() {
        let mut blob = with_footer(&[0x00], 0);
        let n = blob.len();
        blob[n - 4..].copy_from_slice(&(MAX_OUTPUT as u32).to_le_bytes());
        assert!(matches!(decompress(&blob), Err(NdsError::TooLarge { .. })));

        assert!(matches!(
            decompress(&[1, 2, 3]),
            Err(NdsError::Truncated { .. })
        ));
        let bad = [0u8; 8]; // header length 0 is not a BLZ footer
        assert_eq!(decompress(&bad), Err(NdsError::Malformed("BLZ footer")));
    }
}
