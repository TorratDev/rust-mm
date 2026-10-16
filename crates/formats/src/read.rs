//! Bounds-checked little-endian reads for the container formats (`.sbin`, SIR0, PX) and
//! everything built on them. A bad offset is a [`FormatError`](Error), never a panic.

pub use crate::error::FormatError as Error;

/// Result alias for the readers.
pub type Result<T> = std::result::Result<T, Error>;

fn truncated(what: &'static str, offset: usize, len: usize, have: usize) -> Error {
    Error::Truncated {
        what,
        need: offset.saturating_add(len),
        have,
    }
}

/// `data[offset..offset + len]`.
pub fn slice<'a>(
    data: &'a [u8],
    offset: usize,
    len: usize,
    what: &'static str,
) -> Result<&'a [u8]> {
    offset
        .checked_add(len)
        .and_then(|end| data.get(offset..end))
        .ok_or_else(|| truncated(what, offset, len, data.len()))
}

/// Fixed-size array at `offset`.
pub fn array<const N: usize>(data: &[u8], offset: usize, what: &'static str) -> Result<[u8; N]> {
    let mut out = [0u8; N];
    out.copy_from_slice(slice(data, offset, N, what)?);
    Ok(out)
}

/// Byte at `offset`.
pub fn u8_at(data: &[u8], offset: usize, what: &'static str) -> Result<u8> {
    Ok(array::<1>(data, offset, what)?[0])
}

/// Little-endian `u16` at `offset`.
pub fn u16_at(data: &[u8], offset: usize, what: &'static str) -> Result<u16> {
    array(data, offset, what).map(u16::from_le_bytes)
}

/// Little-endian `i16` at `offset`.
pub fn i16_at(data: &[u8], offset: usize, what: &'static str) -> Result<i16> {
    array(data, offset, what).map(i16::from_le_bytes)
}

/// Little-endian `u32` at `offset`.
pub fn u32_at(data: &[u8], offset: usize, what: &'static str) -> Result<u32> {
    array(data, offset, what).map(u32::from_le_bytes)
}

/// Little-endian `i32` at `offset`.
pub fn i32_at(data: &[u8], offset: usize, what: &'static str) -> Result<i32> {
    array(data, offset, what).map(i32::from_le_bytes)
}

/// Little-endian `u32` at `offset`, widened to an offset.
pub fn offset_at(data: &[u8], offset: usize, what: &'static str) -> Result<usize> {
    u32_at(data, offset, what).map(|v| v as usize)
}

/// NUL-terminated byte string at `offset` (without the NUL).
pub fn cstr_at<'a>(data: &'a [u8], offset: usize, what: &'static str) -> Result<&'a [u8]> {
    let tail = data
        .get(offset..)
        .ok_or_else(|| truncated(what, offset, 1, data.len()))?;
    let end = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| Error::invalid(what, format!("string at {offset:#x} has no NUL")))?;
    Ok(&tail[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_bounds_checked() {
        let d = [1, 2, 3, 4, 5];
        assert_eq!(u16_at(&d, 0, "x"), Ok(0x0201));
        assert_eq!(u32_at(&d, 1, "x"), Ok(0x0504_0302));
        assert!(u32_at(&d, 2, "x").is_err());
        assert!(slice(&d, usize::MAX, 2, "x").is_err());
        assert_eq!(cstr_at(b"ab\0c", 0, "s"), Ok(&b"ab"[..]));
        assert!(cstr_at(b"abc", 0, "s").is_err());
    }
}
