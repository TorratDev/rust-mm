//! Cartridge header, bytes `0x000`–`0x1FF` (layout from GBATEK, little-endian).
//!
//! Only the fields the project uses are parsed. The header CRC covers `0x000..0x15E` and
//! is checked before anything else is read, so a corrupt header is rejected up front.

use crate::error::NdsError;
use crate::read;

/// Length of the header block; the CRC-16 sits at [`CRC_OFFSET`].
pub const HEADER_LEN: usize = 0x200;
/// Offset of the stored CRC-16. It covers all bytes before it.
pub const CRC_OFFSET: usize = 0x15E;

/// A `(offset, size)` range inside the ROM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub offset: u32,
    pub size: u32,
}

impl Range {
    /// One past the last byte, as `u64` so it cannot overflow.
    pub fn end(self) -> u64 {
        u64::from(self.offset) + u64::from(self.size)
    }
}

/// An ARM9 or ARM7 binary as laid out in the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binary {
    /// Where the bytes are in the ROM.
    pub range: Range,
    /// Entry point address in RAM.
    pub entry: u32,
    /// Load address in RAM.
    pub ram_addr: u32,
}

/// The parsed cartridge header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// Title, trailing NULs removed.
    pub title: String,
    /// Four-character game code.
    pub game_code: [u8; 4],
    /// Two-character maker code.
    pub maker_code: [u8; 2],
    pub arm9: Binary,
    pub arm7: Binary,
    /// File Name Table.
    pub fnt: Range,
    /// File Allocation Table.
    pub fat: Range,
    /// ARM9 overlay table (32-byte entries).
    pub arm9_overlays: Range,
    /// ARM7 overlay table (32-byte entries).
    pub arm7_overlays: Range,
    /// Offset of the icon/title banner.
    pub banner_offset: u32,
    /// Size of the ROM as used by the cartridge.
    pub used_rom_size: u32,
    /// The stored header CRC-16 (already verified).
    pub crc: u16,
}

impl Header {
    /// Parses and validates the header of `rom`.
    ///
    /// Fails on a bad CRC, a truncated ROM, or any table that points outside the ROM.
    pub fn parse(rom: &[u8]) -> Result<Self, NdsError> {
        let head = read::bytes(rom, 0, CRC_OFFSET + 2, "header")?;
        let stored = read::u16_le(head, CRC_OFFSET, "header CRC")?;
        let computed = crc16(read::bytes(head, 0, CRC_OFFSET, "header")?);
        if stored != computed {
            return Err(NdsError::BadCrc { stored, computed });
        }

        let title_raw = read::bytes(rom, 0x00, 12, "title")?;
        let title = String::from_utf8_lossy(title_raw)
            .trim_end_matches('\0')
            .to_owned();
        let game_code = array::<4>(rom, 0x0C, "game code")?;
        let maker_code = array::<2>(rom, 0x10, "maker code")?;

        let header = Header {
            title,
            game_code,
            maker_code,
            arm9: binary(rom, 0x20, "ARM9")?,
            arm7: binary(rom, 0x30, "ARM7")?,
            fnt: range(rom, 0x40, "FNT")?,
            fat: range(rom, 0x48, "FAT")?,
            arm9_overlays: range(rom, 0x50, "ARM9 overlay table")?,
            arm7_overlays: range(rom, 0x58, "ARM7 overlay table")?,
            banner_offset: read::u32_le(rom, 0x68, "banner offset")?,
            used_rom_size: read::u32_le(rom, 0x80, "used ROM size")?,
            crc: stored,
        };

        check(rom.len(), header.arm9.range, "ARM9")?;
        check(rom.len(), header.arm7.range, "ARM7")?;
        check(rom.len(), header.fnt, "FNT")?;
        check(rom.len(), header.fat, "FAT")?;
        check(rom.len(), header.arm9_overlays, "ARM9 overlay table")?;
        check(rom.len(), header.arm7_overlays, "ARM7 overlay table")?;
        Ok(header)
    }
}

/// CRC-16 as used by the NDS header (reflected polynomial `0xA001`, initial value `0xFFFF`).
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Confirms that `r` lies inside a ROM of `rom_len` bytes. Zero-size ranges are allowed.
pub(crate) fn check(rom_len: usize, r: Range, what: &'static str) -> Result<(), NdsError> {
    if r.end() > rom_len as u64 {
        return Err(NdsError::OutOfBounds {
            what,
            offset: u64::from(r.offset),
            size: u64::from(r.size),
            rom_len: rom_len as u64,
        });
    }
    Ok(())
}

fn array<const N: usize>(rom: &[u8], at: usize, what: &'static str) -> Result<[u8; N], NdsError> {
    let s = read::bytes(rom, at, N, what)?;
    let mut out = [0u8; N];
    out.copy_from_slice(s);
    Ok(out)
}

fn range(rom: &[u8], at: usize, what: &'static str) -> Result<Range, NdsError> {
    Ok(Range {
        offset: read::u32_le(rom, at, what)?,
        size: read::u32_le(rom, at + 4, what)?,
    })
}

/// Reads an ARM9/ARM7 descriptor: ROM offset, entry, RAM address, size (in that order).
fn binary(rom: &[u8], at: usize, what: &'static str) -> Result<Binary, NdsError> {
    Ok(Binary {
        range: Range {
            offset: read::u32_le(rom, at, what)?,
            size: read::u32_le(rom, at + 12, what)?,
        },
        entry: read::u32_le(rom, at + 4, what)?,
        ram_addr: read::u32_le(rom, at + 8, what)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::build_rom;

    #[test]
    fn parses_synthetic_header_fields() {
        let rom = build_rom(&[("a.bin", b"aa")], &[]);
        let h = Header::parse(&rom).unwrap();
        assert_eq!(h.title, "TESTROM");
        assert_eq!(&h.game_code, b"TEST");
        assert_eq!(h.arm9.entry, 0x0200_0800);
        assert_eq!(h.arm9.range.size, 0x40);
        assert_eq!(h.arm7.ram_addr, 0x0380_0000);
        assert_eq!(u64::from(h.used_rom_size), rom.len() as u64);
    }

    #[test]
    fn rejects_bad_crc() {
        let mut rom = build_rom(&[("a.bin", b"aa")], &[]);
        rom[0x20] ^= 1; // change a covered byte without fixing the CRC
        assert!(matches!(Header::parse(&rom), Err(NdsError::BadCrc { .. })));
    }

    #[test]
    fn rejects_truncated_and_out_of_range() {
        let rom = build_rom(&[("a.bin", b"aa")], &[]);
        assert!(matches!(
            Header::parse(&rom[..0x100]),
            Err(NdsError::Truncated { .. })
        ));

        // Point the FNT past the end of the ROM, then fix the CRC so only the range is wrong.
        let mut bad = rom.clone();
        bad[0x40..0x44].copy_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
        let crc = crc16(&bad[..CRC_OFFSET]);
        bad[CRC_OFFSET..CRC_OFFSET + 2].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            Header::parse(&bad),
            Err(NdsError::OutOfBounds { what: "FNT", .. })
        ));
    }
}
