//! Bounds-checked little-endian reads. Every read goes through `get`, so a bad offset is
//! an error, not a panic.

use crate::error::NdsError;

pub(crate) fn bytes<'a>(
    b: &'a [u8],
    at: usize,
    len: usize,
    what: &'static str,
) -> Result<&'a [u8], NdsError> {
    let need = at.saturating_add(len);
    b.get(at..need).ok_or(NdsError::Truncated {
        what,
        need,
        have: b.len(),
    })
}

pub(crate) fn u8_at(b: &[u8], at: usize, what: &'static str) -> Result<u8, NdsError> {
    bytes(b, at, 1, what).map(|s| s.first().copied().unwrap_or_default())
}

pub(crate) fn u16_le(b: &[u8], at: usize, what: &'static str) -> Result<u16, NdsError> {
    let s = bytes(b, at, 2, what)?;
    let a: [u8; 2] = s.try_into().map_err(|_| truncated(what, at, 2, b))?;
    Ok(u16::from_le_bytes(a))
}

pub(crate) fn u32_le(b: &[u8], at: usize, what: &'static str) -> Result<u32, NdsError> {
    let s = bytes(b, at, 4, what)?;
    let a: [u8; 4] = s.try_into().map_err(|_| truncated(what, at, 4, b))?;
    Ok(u32::from_le_bytes(a))
}

fn truncated(what: &'static str, at: usize, len: usize, b: &[u8]) -> NdsError {
    NdsError::Truncated {
        what,
        need: at.saturating_add(len),
        have: b.len(),
    }
}
