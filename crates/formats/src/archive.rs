//! `.sbin` pack archives: a flat table of named sub-files. Spec: `docs/formats/archive.md`.
//!
//! Every non-SDAT `.sbin` file in Blue's filesystem is one of these. The table is a run
//! of 16-byte entries `{name: [u8; 8] NUL-padded, offset: u32, size: u32}` at the start
//! of the file; it ends at an all-zero name or where the first sub-file begins.

use crate::read::{self as bytes, Error, Result};

const ENTRY_SIZE: usize = 16;

/// One sub-file of an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Name (up to 8 bytes, Latin-1).
    pub name: String,
    /// Offset in the archive.
    pub offset: u32,
    /// Size in bytes.
    pub size: u32,
}

/// A parsed archive table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archive {
    entries: Vec<Entry>,
}

impl Archive {
    /// Parses the entry table of `data`, checking every sub-file lies inside it.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut entries = Vec::new();
        let mut table_end = data.len();
        let mut pos = 0;
        while pos + ENTRY_SIZE <= table_end {
            let raw_name = bytes::slice(data, pos, 8, "archive entry name")?;
            if raw_name[0] == 0 {
                break;
            }
            let offset = bytes::u32_at(data, pos + 8, "archive entry")?;
            let size = bytes::u32_at(data, pos + 12, "archive entry")?;
            let name_len = raw_name.iter().position(|&b| b == 0).unwrap_or(8);
            let name: String = raw_name[..name_len]
                .iter()
                .map(|&b| char::from(b))
                .collect();
            bytes::slice(data, offset as usize, size as usize, "archive sub-file")?;
            if (offset as usize) < pos + ENTRY_SIZE {
                return Err(Error::invalid(
                    "archive",
                    format!("entry {name:?} starts inside the table"),
                ));
            }
            table_end = table_end.min(offset as usize);
            entries.push(Entry { name, offset, size });
            pos += ENTRY_SIZE;
        }
        if entries.is_empty() {
            return Err(Error::invalid("archive", "no entries"));
        }
        Ok(Self { entries })
    }

    /// Entries in table order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Entry named `name`.
    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Bytes of `entry` within the archive `data` it was parsed from.
    pub fn data<'a>(&self, data: &'a [u8], entry: &Entry) -> Result<&'a [u8]> {
        bytes::slice(
            data,
            entry.offset as usize,
            entry.size as usize,
            "archive sub-file",
        )
    }

    /// Bytes of the sub-file named `name`.
    pub fn get<'a>(&self, data: &'a [u8], name: &str) -> Result<&'a [u8]> {
        let e = self
            .find(name)
            .ok_or_else(|| Error::invalid("archive", format!("no sub-file named {name:?}")))?;
        self.data(data, e)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds an archive from `(name, bytes)` pairs.
    pub(crate) fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
        let table = (files.len() + 1) * ENTRY_SIZE;
        let mut out = vec![0u8; table];
        for (i, (name, body)) in files.iter().enumerate() {
            let off = out.len();
            out.extend_from_slice(body);
            let e = &mut out[i * ENTRY_SIZE..(i + 1) * ENTRY_SIZE];
            e[..name.len()].copy_from_slice(name.as_bytes());
            e[8..12].copy_from_slice(&(off as u32).to_le_bytes());
            e[12..16].copy_from_slice(&(body.len() as u32).to_le_bytes());
        }
        out
    }

    #[test]
    fn lists_and_reads_entries() {
        let a = build(&[("one", b"111"), ("two", b"22")]);
        let arc = Archive::parse(&a).unwrap();
        assert_eq!(arc.entries().len(), 2);
        assert_eq!(arc.get(&a, "two").unwrap(), b"22");
        assert!(arc.get(&a, "three").is_err());
    }

    #[test]
    fn rejects_out_of_range_entries() {
        let mut a = build(&[("one", b"111")]);
        a[12] = 0xFF; // size past the end
        assert!(Archive::parse(&a).is_err());
        assert!(Archive::parse(&[0; 32]).is_err());
        assert!(Archive::parse(&[]).is_err());
    }
}
