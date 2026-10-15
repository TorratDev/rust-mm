//! Cartridge filesystem: File Name Table (FNT, directory tree and names) and File
//! Allocation Table (FAT, byte range of each file ID).
//!
//! Files get IDs in FNT walk order. Overlays have no name in the FNT but do have a FAT
//! entry, so [`Filesystem::data`] can read any ID, named or not.

use std::collections::BTreeMap;

use crate::error::NdsError;
use crate::header::{Header, Range, check};
use crate::read;

/// Directory ID of the root directory.
pub const ROOT_DIR_ID: u16 = 0xF000;

/// A named file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// File ID (index into the FAT).
    pub id: u16,
    /// Path from the root, `/`-separated (for example `dir/name.bin`).
    pub path: String,
    /// Start offset in the ROM.
    pub start: u32,
    /// End offset in the ROM (exclusive).
    pub end: u32,
}

impl File {
    /// Length in bytes.
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Whether the file is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The parsed filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filesystem {
    fat: Vec<(u32, u32)>,
    files: Vec<File>,
    by_path: BTreeMap<String, usize>,
}

impl Filesystem {
    /// Reads the FNT and FAT that `header` points to. Every named file's range is checked
    /// against `rom` up front.
    pub fn parse(rom: &[u8], header: &Header) -> Result<Self, NdsError> {
        let fnt = slice(rom, header.fnt, "FNT")?;
        let fat_bytes = slice(rom, header.fat, "FAT")?;
        if !fat_bytes.len().is_multiple_of(8) {
            return Err(NdsError::Malformed("FAT size is not a multiple of 8"));
        }
        let fat = fat_bytes
            .chunks_exact(8)
            .map(|c| {
                let start = read::u32_le(c, 0, "FAT entry")?;
                let end = read::u32_le(c, 4, "FAT entry")?;
                Ok((start, end))
            })
            .collect::<Result<Vec<_>, NdsError>>()?;

        let mut files = Vec::new();
        for (id, path) in walk_fnt(fnt, fat.len())? {
            let (start, end) = fat
                .get(usize::from(id))
                .copied()
                .ok_or(NdsError::Malformed("file id beyond FAT"))?;
            if start > end || u64::from(end) > rom.len() as u64 {
                return Err(NdsError::OutOfBounds {
                    what: "file in FAT",
                    offset: u64::from(start),
                    size: u64::from(end.saturating_sub(start)),
                    rom_len: rom.len() as u64,
                });
            }
            files.push(File {
                id,
                path,
                start,
                end,
            });
        }

        let by_path = files
            .iter()
            .enumerate()
            .map(|(i, f)| (f.path.clone(), i))
            .collect();
        Ok(Filesystem {
            fat,
            files,
            by_path,
        })
    }

    /// All named files, ordered by ID.
    pub fn files(&self) -> &[File] {
        &self.files
    }

    /// Number of FAT entries, including unnamed overlays.
    pub fn fat_len(&self) -> usize {
        self.fat.len()
    }

    /// Looks up a named file by path.
    pub fn get(&self, path: &str) -> Option<&File> {
        self.by_path.get(path).and_then(|&i| self.files.get(i))
    }

    /// Bytes of the file with `id` (named or overlay), taken from `rom`.
    pub fn data<'a>(&self, rom: &'a [u8], id: u16) -> Result<&'a [u8], NdsError> {
        let (start, end) = self
            .fat
            .get(usize::from(id))
            .copied()
            .ok_or(NdsError::Malformed("file id beyond FAT"))?;
        if start > end {
            return Err(NdsError::Malformed("FAT entry ends before it starts"));
        }
        read::bytes(rom, start as usize, (end - start) as usize, "file data")
    }
}

fn slice<'a>(rom: &'a [u8], r: Range, what: &'static str) -> Result<&'a [u8], NdsError> {
    check(rom.len(), r, what)?;
    read::bytes(rom, r.offset as usize, r.size as usize, what)
}

/// Walks the FNT from the root, returning `(file id, path)` for every named file.
fn walk_fnt(fnt: &[u8], fat_count: usize) -> Result<Vec<(u16, String)>, NdsError> {
    let dir_count = usize::from(read::u16_le(fnt, 6, "FNT root")?);
    if dir_count == 0 || dir_count > 0x1000 {
        return Err(NdsError::Malformed("FNT directory count"));
    }

    let mut out = Vec::new();
    let mut seen = vec![false; dir_count];
    let mut stack = vec![(ROOT_DIR_ID, String::new())];

    while let Some((dir_id, prefix)) = stack.pop() {
        let idx = usize::from(dir_id & 0x0FFF);
        if dir_id & 0xF000 != ROOT_DIR_ID || idx >= dir_count {
            return Err(NdsError::Malformed("FNT directory id out of range"));
        }
        if std::mem::replace(&mut seen[idx], true) {
            return Err(NdsError::Malformed("FNT directory visited twice"));
        }

        let base = idx * 8;
        let sub = read::u32_le(fnt, base, "FNT directory")? as usize;
        let mut file_id = usize::from(read::u16_le(fnt, base + 4, "FNT directory")?);
        let mut pos = sub;

        loop {
            let kind = read::u8_at(fnt, pos, "FNT entry")?;
            pos += 1;
            if kind == 0 {
                break;
            }
            let len = usize::from(kind & 0x7F);
            if len == 0 {
                return Err(NdsError::Malformed("FNT entry with empty name"));
            }
            let name = std::str::from_utf8(read::bytes(fnt, pos, len, "FNT name")?)
                .map_err(|_| NdsError::Malformed("FNT name is not UTF-8"))?;
            pos += len;

            if kind & 0x80 != 0 {
                let child = read::u16_le(fnt, pos, "FNT subdirectory")?;
                pos += 2;
                stack.push((child, format!("{prefix}{name}/")));
            } else {
                if file_id >= fat_count {
                    return Err(NdsError::Malformed("file id beyond FAT"));
                }
                out.push((file_id as u16, format!("{prefix}{name}")));
                file_id += 1;
            }
        }
    }

    out.sort_by_key(|(id, _)| *id);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::build_rom;

    #[test]
    fn lists_named_files_and_reads_bytes() {
        let rom = build_rom(
            &[("monster.sbin", b"MONSTER!"), ("message_e.str", b"text")],
            &[],
        );
        let header = Header::parse(&rom).unwrap();
        let fs = Filesystem::parse(&rom, &header).unwrap();

        let names: Vec<_> = fs.files().iter().map(|f| (f.id, f.path.as_str())).collect();
        assert_eq!(names, [(0, "monster.sbin"), (1, "message_e.str")]);
        let f = fs.get("message_e.str").unwrap();
        assert_eq!(f.len(), 4);
        assert_eq!(fs.data(&rom, f.id).unwrap(), b"text");
    }

    #[test]
    fn overlays_have_fat_entries_without_names() {
        let rom = build_rom(&[("a.bin", b"aa")], &[vec![1, 2, 3]]);
        let header = Header::parse(&rom).unwrap();
        let fs = Filesystem::parse(&rom, &header).unwrap();
        assert_eq!(fs.files().len(), 1);
        assert_eq!(fs.fat_len(), 2);
        assert_eq!(fs.data(&rom, 1).unwrap(), &[1, 2, 3]);
    }

    #[test]
    fn rejects_bad_fat_range_and_bad_id() {
        let mut rom = build_rom(&[("a.bin", b"aa")], &[]);
        let header = Header::parse(&rom).unwrap();
        // Point the first FAT entry past the end of the ROM.
        let fat = header.fat.offset as usize;
        rom[fat + 4..fat + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Filesystem::parse(&rom, &header),
            Err(NdsError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn unknown_id_is_an_error() {
        let rom = build_rom(&[("a.bin", b"aa")], &[]);
        let header = Header::parse(&rom).unwrap();
        let fs = Filesystem::parse(&rom, &header).unwrap();
        assert!(fs.data(&rom, 9).is_err());
    }

    #[test]
    fn truncated_fnt_is_an_error_not_a_panic() {
        let rom = build_rom(&[("a.bin", b"aa")], &[]);
        let header = Header::parse(&rom).unwrap();
        let cut = &rom[..header.fnt.offset as usize + 2];
        assert!(Filesystem::parse(cut, &header).is_err());
    }
}
