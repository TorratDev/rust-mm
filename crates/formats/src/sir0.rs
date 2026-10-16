//! SIR0, the relocatable wrapper around most of Blue's sub-files. Spec:
//! `docs/formats/sir0.md`.
//!
//! Header: `"SIR0"`, `u32` offset of the content header ("data pointer"), `u32` offset
//! of the pointer list, `u32` 0. Pointers inside the file are offsets from the file
//! start; the game relocates them at load time using the pointer list, a run of
//! big-endian base-128 varints (high bit = more bytes) giving the delta from the previous
//! pointer's position, ended by a 0 byte. Parsers here read the offsets as they are.

use crate::read::{self as bytes, Error, Result};

/// Magic at offset 0.
pub const MAGIC: &[u8; 4] = b"SIR0";

/// A parsed SIR0 wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sir0<'a> {
    /// The whole file.
    pub data: &'a [u8],
    /// Offset of the content header.
    pub data_offset: usize,
    /// Offset of the pointer list.
    pub pointer_list_offset: usize,
    /// Positions of every pointer in the file, ascending.
    pub pointers: Vec<usize>,
}

impl<'a> Sir0<'a> {
    /// True if `data` starts with the SIR0 magic.
    pub fn is_sir0(data: &[u8]) -> bool {
        data.starts_with(MAGIC)
    }

    /// Parses the header and the pointer list. Every pointer position and the value
    /// stored there must lie inside the file.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if !Self::is_sir0(data) {
            return Err(Error::invalid("SIR0", "missing SIR0 magic"));
        }
        let data_offset = bytes::offset_at(data, 4, "SIR0 header")?;
        let pointer_list_offset = bytes::offset_at(data, 8, "SIR0 header")?;
        if data_offset >= data.len() || pointer_list_offset >= data.len() {
            return Err(Error::invalid(
                "SIR0",
                "header offsets point outside the file",
            ));
        }
        let mut pointers = Vec::new();
        let mut pos = pointer_list_offset;
        let mut cur = 0usize;
        loop {
            let mut v = 0usize;
            loop {
                let b = bytes::u8_at(data, pos, "SIR0 pointer list")?;
                pos += 1;
                v = v
                    .checked_mul(128)
                    .and_then(|v| v.checked_add(usize::from(b & 0x7F)))
                    .filter(|&v| v < data.len())
                    .ok_or_else(|| Error::invalid("SIR0 pointer list", "delta too large"))?;
                if b & 0x80 == 0 {
                    break;
                }
            }
            if v == 0 {
                break;
            }
            cur += v;
            let target = bytes::offset_at(data, cur, "SIR0 pointer")?;
            if target > data.len() {
                return Err(Error::invalid(
                    "SIR0",
                    format!("pointer at {cur:#x} targets {target:#x}, outside the file"),
                ));
            }
            pointers.push(cur);
        }
        Ok(Self {
            data,
            data_offset,
            pointer_list_offset,
            pointers,
        })
    }

    /// `u32` at `offset`.
    pub fn u32(&self, offset: usize, what: &'static str) -> Result<u32> {
        bytes::u32_at(self.data, offset, what)
    }

    /// Pointer (file offset) stored at `offset`.
    pub fn ptr(&self, offset: usize, what: &'static str) -> Result<usize> {
        let v = bytes::offset_at(self.data, offset, what)?;
        if v > self.data.len() {
            return Err(Error::invalid(
                what,
                format!("pointer {v:#x} at {offset:#x} is outside the file"),
            ));
        }
        Ok(v)
    }

    /// Bytes from `offset` to the end of the file.
    pub fn tail(&self, offset: usize, what: &'static str) -> Result<&'a [u8]> {
        self.data.get(offset..).ok_or(Error::Truncated {
            what,
            need: offset,
            have: self.data.len(),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Wraps `body` (content at 0x10) in SIR0 with the content header at
    /// `0x10 + header_at` and pointers at the given body-relative positions.
    pub(crate) fn wrap(body: &[u8], header_at: usize, pointer_positions: &[usize]) -> Vec<u8> {
        let mut out = b"SIR0".to_vec();
        out.extend([0u8; 12]);
        out.extend_from_slice(body);
        while !out.len().is_multiple_of(4) {
            out.push(0xAA);
        }
        let list = out.len();
        out[4..8].copy_from_slice(&((0x10 + header_at) as u32).to_le_bytes());
        out[8..12].copy_from_slice(&(list as u32).to_le_bytes());
        let mut prev = 0;
        for p in [4, 8]
            .into_iter()
            .chain(pointer_positions.iter().map(|p| p + 0x10))
        {
            let mut d = p - prev;
            prev = p;
            let mut enc = vec![(d & 0x7F) as u8];
            d >>= 7;
            while d > 0 {
                enc.insert(0, 0x80 | (d & 0x7F) as u8);
                d >>= 7;
            }
            out.extend(enc);
        }
        out.push(0);
        out
    }

    #[test]
    fn reads_pointer_list() {
        let mut body = vec![0u8; 0x100];
        body[0x90..0x94].copy_from_slice(&0x10u32.to_le_bytes());
        let f = wrap(&body, 0x90, &[0x90]);
        let s = Sir0::parse(&f).unwrap();
        assert_eq!(s.data_offset, 0xA0);
        assert_eq!(s.pointers, [4, 8, 0xA0]);
        assert_eq!(s.ptr(0xA0, "p").unwrap(), 0x10);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Sir0::parse(b"SIR1aaaaaaaa").is_err());
        assert!(Sir0::parse(b"SIR0\xff\xff\xff\xff\0\0\0\0").is_err());
        let mut f = wrap(&[0; 16], 0, &[]);
        let n = f.len();
        f[n - 1] = 0x81; // unterminated varint
        assert!(Sir0::parse(&f).is_err());
    }
}
