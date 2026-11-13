//! The fixed-room file `fixedmap`: a SIR0 container inside `dungeon.sbin`, one entry per
//! fixed room (width, height, flags and a run-length tile stream). Spec:
//! `docs/formats/fixedmap.md`. Consumer: the fixed-room placement in `rescue-sim` (#397).

use crate::error::FormatError;
use crate::read::Result;
use crate::sir0::Sir0;

/// Entries in the table. The game does not store the count; retail has 140.
pub const ENTRY_COUNT: usize = 140;

/// Smallest offset an entry can start at: the SIR0 header ends there.
const FIRST_ENTRY: usize = 0x10;

/// Bytes before the tile stream in an entry: width, height, flags.
const HEADER_LEN: usize = 3;

/// Escape byte of the tile stream: the next byte is a literal tile code.
const ESCAPE: u8 = 0x0E;

/// One fixed-room entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedMapEntry<'a> {
    pub width: u8,
    pub height: u8,
    /// Bit 0 is read by the stamp of rooms numbered 50 and up (`02149E80`).
    pub flags: u8,
    /// From byte 3 to the next entry's start (the table's content pointer for entry 139).
    pub stream: &'a [u8],
}

/// A checked fixed-room file. [`FixedMap::parse`] checks every entry, so [`FixedMap::entry`]
/// only returns well-formed ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedMap<'a> {
    entries: Vec<FixedMapEntry<'a>>,
}

impl<'a> FixedMap<'a> {
    /// Parses `file` and checks its entry table. The table is the SIR0 content pointer
    /// (`u32` offsets from the file start). Rejected (a design choice; the game checks none
    /// of this): a table that runs past the file; an entry that starts before the header
    /// or whose header and stream do not fit before the next entry (entry 139: before the
    /// table); a stream that ends before `width * height` codes; a run that passes the last
    /// code. Width or height 0 is accepted. Bytes after the last code are accepted.
    pub fn parse(file: &'a [u8]) -> Result<Self> {
        let table = Sir0::parse(file)?.data_offset;
        let starts = (0..ENTRY_COUNT)
            .map(|n| {
                let at = table.saturating_add(4 * n);
                crate::read::offset_at(file, at, "fixedmap table")
            })
            .collect::<Result<Vec<usize>>>()?;
        let ends = starts.iter().skip(1).copied().chain([table]);
        let mut entries = Vec::with_capacity(ENTRY_COUNT);
        for (n, (&start, end)) in starts.iter().zip(ends).enumerate() {
            let bad = |why: String| FormatError::invalid("fixedmap", format!("entry {n}: {why}"));
            if start < FIRST_ENTRY {
                return Err(bad(format!("starts at {start:#x}, inside the header")));
            }
            let entry = split(file, start, end)
                .ok_or_else(|| bad(format!("{start:#x}..{end:#x} has no room for its header")))?;
            let codes = usize::from(entry.width) * usize::from(entry.height);
            let mut tiles = TileStream::new(entry.stream);
            for _ in 0..codes {
                if tiles.try_next().is_none() {
                    return Err(bad(format!("stream ends before its {codes} codes")));
                }
            }
            if tiles.repeat > 0 {
                return Err(bad(format!("a run passes its {codes} codes")));
            }
            entries.push(entry);
        }
        Ok(Self { entries })
    }

    /// Entry `number`. `None` for a negative or out-of-range number.
    #[must_use]
    pub fn entry(&self, number: i16) -> Option<FixedMapEntry<'a>> {
        let n = usize::try_from(number).ok()?;
        self.entries.get(n).copied()
    }
}

/// The entry in `file[start..end]`: its header, then the stream. `None` if the header does
/// not fit.
fn split(file: &[u8], start: usize, end: usize) -> Option<FixedMapEntry<'_>> {
    let (header, stream) = file.get(start..end)?.split_first_chunk::<HEADER_LEN>()?;
    Some(FixedMapEntry {
        width: header[0],
        height: header[1],
        flags: header[2],
        stream,
    })
}

/// The tile-stream decoder (`0214A348`): one tile code per call, row by row. A byte `0x0E`
/// is an escape: the next byte is a literal code. Any other byte `b` is the code `b >> 4`,
/// repeated `(b & 0xF) + 1` times. [blue-verified static]
#[derive(Clone, Debug)]
pub struct TileStream<'a> {
    data: &'a [u8],
    pos: usize,
    repeat: u8,
    code: u8,
}

impl<'a> TileStream<'a> {
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            repeat: 0,
            code: 0,
        }
    }

    /// The next tile code. `None` once the data has run out, including an escape with no
    /// literal byte after it.
    pub fn try_next(&mut self) -> Option<u8> {
        if self.repeat > 0 {
            self.repeat -= 1;
            return Some(self.code);
        }
        let b = *self.data.get(self.pos)?;
        if b == ESCAPE {
            self.code = *self.data.get(self.pos + 1)?;
            self.pos += 2;
        } else {
            self.code = b >> 4;
            self.repeat = b & 0xF;
            self.pos += 1;
        }
        Some(self.code)
    }

    /// The next tile code; past the end of the data, code 0.
    pub fn next_code(&mut self) -> u8 {
        self.try_next().unwrap_or(0)
    }

    /// The bytes not read yet.
    #[must_use]
    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or(&[])
    }

    /// True when every byte has been read and no run is pending.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.repeat == 0 && self.rest().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file whose table holds `entries` (exactly `ENTRY_COUNT`), built with the SIR0
    /// test helper: header words and table words are pointers, as the game relocates them.
    fn build(entries: &[Vec<u8>]) -> Vec<u8> {
        assert_eq!(entries.len(), ENTRY_COUNT);
        let mut body = Vec::new();
        let mut starts = Vec::new();
        for e in entries {
            starts.push(0x10 + body.len());
            body.extend(e);
        }
        let table = body.len();
        for s in starts {
            body.extend((s as u32).to_le_bytes());
        }
        let positions: Vec<usize> = (0..ENTRY_COUNT).map(|n| table + 4 * n).collect();
        crate::sir0::tests::wrap(&body, table, &positions)
    }

    fn plain() -> Vec<Vec<u8>> {
        vec![vec![1, 1, 0, 0x00]; ENTRY_COUNT]
    }

    /// Overwrites table word `n` of a file built by [`build`].
    fn set_offset(file: &mut [u8], n: usize, value: usize) {
        let table = u32::from_le_bytes([file[4], file[5], file[6], file[7]]) as usize;
        file[table + 4 * n..table + 4 * n + 4].copy_from_slice(&(value as u32).to_le_bytes());
    }

    fn offset(file: &[u8], n: usize) -> usize {
        let table = u32::from_le_bytes([file[4], file[5], file[6], file[7]]) as usize;
        let at = table + 4 * n;
        u32::from_le_bytes([file[at], file[at + 1], file[at + 2], file[at + 3]]) as usize
    }

    #[test]
    fn looks_up_entries_and_bounds_each_stream() {
        let mut e = plain();
        // 2 x 1: the run 0x21 gives code 2 twice; the escape after it is not part of the room.
        e[0] = vec![2, 1, 3, 0x21, 0x0E, 0x43];
        e[1] = vec![0, 0, 0];
        let file = build(&e);
        let m = FixedMap::parse(&file).unwrap();
        let a = m.entry(0).unwrap();
        assert_eq!((a.width, a.height, a.flags), (2, 1, 3));
        assert_eq!(a.stream, [0x21, 0x0E, 0x43]);
        // Width 0 is returned, with an empty stream.
        let z = m.entry(1).unwrap();
        assert_eq!((z.width, z.height, z.stream.len()), (0, 0, 0));
        // The last entry's stream ends at the table.
        assert_eq!(m.entry(139).unwrap().stream, [0x00]);
        assert!(m.entry(-1).is_none());
        assert!(m.entry(ENTRY_COUNT as i16).is_none());
        assert!(m.entry(i16::MAX).is_none());
    }

    #[test]
    fn tile_stream_runs_escapes_and_ends() {
        // 0x21: code 2, repeated twice; 0x0E 0x43: literal 0x43; 0x90: code 9 once.
        let mut s = TileStream::new(&[0x21, 0x0E, 0x43, 0x90]);
        let got: Vec<u8> = (0..7).map(|_| s.next_code()).collect();
        assert_eq!(got, [2, 2, 0x43, 9, 0, 0, 0]);
        assert!(s.is_exhausted());
    }

    #[test]
    fn tile_stream_reports_an_escape_without_its_literal() {
        assert_eq!(TileStream::new(&[0x0E]).try_next(), None);
        assert_eq!(TileStream::new(&[0x0E]).next_code(), 0);
    }

    #[test]
    fn rejects_bad_tables_and_short_or_long_streams() {
        let good = build(&plain());
        assert!(FixedMap::parse(&good).is_ok());
        assert!(FixedMap::parse(b"SIR1").is_err());

        // The table runs past the end of the file.
        let mut f = good.clone();
        let n = f.len() as u32 - 2;
        f[4..8].copy_from_slice(&n.to_le_bytes());
        assert!(FixedMap::parse(&f).is_err());

        // Entry 0 starts inside the header.
        let mut f = good.clone();
        set_offset(&mut f, 0, 4);
        assert!(FixedMap::parse(&f).is_err());

        // Entry 139 starts at the table: no room for its header.
        let mut f = good.clone();
        let table = u32::from_le_bytes([f[4], f[5], f[6], f[7]]) as usize;
        set_offset(&mut f, 139, table);
        assert!(FixedMap::parse(&f).is_err());

        // Entry 4 ends before its header: entry 5 starts one byte before entry 4.
        let mut f = good.clone();
        set_offset(&mut f, 5, offset(&good, 4) - 1);
        assert!(FixedMap::parse(&f).is_err());

        // Streams: short, a run past the last code, an escape with no literal.
        for body in [
            vec![2, 2, 0, 0x21],
            vec![1, 1, 0, 0x02],
            vec![1, 1, 0, 0x0E],
        ] {
            let mut e = plain();
            e[7] = body;
            assert!(FixedMap::parse(&build(&e)).is_err());
        }

        // Width 0 is not rejected.
        let mut e = plain();
        e[7] = vec![0, 9, 0];
        assert_eq!(
            FixedMap::parse(&build(&e)).unwrap().entry(7).unwrap().width,
            0
        );
    }
}
