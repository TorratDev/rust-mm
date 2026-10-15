//! ARM9/ARM7 binaries and the overlay tables, plus the memory-map export that the Ghidra
//! loader (V8) reads.

use std::fmt::Write as _;

use crate::blz;
use crate::error::NdsError;
use crate::fs::Filesystem;
use crate::header::{Binary, Header, Range, check};
use crate::read;

/// Size of one overlay table entry.
pub const OVERLAY_ENTRY_LEN: usize = 32;
/// Bit 24 of the flags word marks a BLZ-compressed overlay.
pub const FLAG_COMPRESSED: u32 = 1 << 24;

/// One overlay table entry, as laid out in the ARM9 or ARM7 table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub id: u32,
    pub ram_addr: u32,
    pub ram_size: u32,
    pub bss_size: u32,
    pub static_init_start: u32,
    pub static_init_end: u32,
    /// FAT index of the overlay's bytes.
    pub file_id: u32,
    /// Flags word: bits 0–23 are the compressed size, bit 24 is the compressed flag.
    pub flags: u32,
}

impl Overlay {
    /// Whether the overlay is stored BLZ-compressed.
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }
}

/// Parses an overlay table.
pub fn parse_overlay_table(table: &[u8]) -> Result<Vec<Overlay>, NdsError> {
    if !table.len().is_multiple_of(OVERLAY_ENTRY_LEN) {
        return Err(NdsError::Malformed(
            "overlay table size is not a multiple of 32",
        ));
    }
    table
        .chunks_exact(OVERLAY_ENTRY_LEN)
        .map(|e| {
            Ok(Overlay {
                id: read::u32_le(e, 0x00, "overlay")?,
                ram_addr: read::u32_le(e, 0x04, "overlay")?,
                ram_size: read::u32_le(e, 0x08, "overlay")?,
                bss_size: read::u32_le(e, 0x0C, "overlay")?,
                static_init_start: read::u32_le(e, 0x10, "overlay")?,
                static_init_end: read::u32_le(e, 0x14, "overlay")?,
                file_id: read::u32_le(e, 0x18, "overlay")?,
                flags: read::u32_le(e, 0x1C, "overlay")?,
            })
        })
        .collect()
}

/// The raw bytes of an ARM9 or ARM7 binary, as stored in the ROM (possibly compressed).
pub fn binary_bytes<'a>(rom: &'a [u8], bin: &Binary) -> Result<&'a [u8], NdsError> {
    check(rom.len(), bin.range, "binary")?;
    read::bytes(
        rom,
        bin.range.offset as usize,
        bin.range.size as usize,
        "binary",
    )
}

/// Reads the ARM9 and ARM7 overlay tables from the ROM.
pub fn read_overlay_tables(
    rom: &[u8],
    header: &Header,
) -> Result<(Vec<Overlay>, Vec<Overlay>), NdsError> {
    Ok((
        table(rom, header.arm9_overlays, "ARM9 overlay table")?,
        table(rom, header.arm7_overlays, "ARM7 overlay table")?,
    ))
}

fn table(rom: &[u8], r: Range, what: &'static str) -> Result<Vec<Overlay>, NdsError> {
    check(rom.len(), r, what)?;
    parse_overlay_table(read::bytes(rom, r.offset as usize, r.size as usize, what)?)
}

/// The decompressed bytes of an overlay, read through the FAT.
pub fn overlay_data(rom: &[u8], fs: &Filesystem, ov: &Overlay) -> Result<Vec<u8>, NdsError> {
    let id = u16::try_from(ov.file_id).map_err(|_| NdsError::Malformed("overlay file id"))?;
    let raw = fs.data(rom, id)?;
    if ov.is_compressed() {
        blz::decompress(raw)
    } else {
        Ok(raw.to_vec())
    }
}

/// Memory map as JSON for the Ghidra loader (`docs/symbols.md`). It holds addresses and
/// sizes only, no ROM bytes.
pub fn memory_map_json(
    arm9: &Binary,
    arm7: &Binary,
    arm9_overlays: &[Overlay],
    arm7_overlays: &[Overlay],
) -> String {
    let mut s = String::new();
    s.push_str("{\n  \"segments\": [\n");
    let segs = [("arm9", arm9), ("arm7", arm7)];
    for (i, (name, b)) in segs.iter().enumerate() {
        let sep = if i + 1 < segs.len() { "," } else { "" };
        let _ = writeln!(
            s,
            "    {{\"name\": \"{name}\", \"ram_addr\": {}, \"size\": {}, \"entry\": {}, \"rom_offset\": {}}}{sep}",
            b.ram_addr, b.range.size, b.entry, b.range.offset
        );
    }
    s.push_str("  ],\n  \"overlays\": [\n");
    let all: Vec<(&str, &Overlay)> = arm9_overlays
        .iter()
        .map(|o| ("arm9", o))
        .chain(arm7_overlays.iter().map(|o| ("arm7", o)))
        .collect();
    for (i, (proc_name, o)) in all.iter().enumerate() {
        let sep = if i + 1 < all.len() { "," } else { "" };
        let _ = writeln!(
            s,
            "    {{\"table\": \"{proc_name}\", \"id\": {}, \"ram_addr\": {}, \"ram_size\": {}, \"bss_size\": {}, \"file_id\": {}, \"compressed\": {}}}{sep}",
            o.id,
            o.ram_addr,
            o.ram_size,
            o.bss_size,
            o.file_id,
            o.is_compressed()
        );
    }
    s.push_str("  ]\n}\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::Filesystem;
    use crate::header::Header;
    use crate::synth::build_rom;

    fn entry(file_id: u32, flags: u32) -> Vec<u8> {
        let mut e = vec![0u8; OVERLAY_ENTRY_LEN];
        e[0x18..0x1C].copy_from_slice(&file_id.to_le_bytes());
        e[0x1C..0x20].copy_from_slice(&flags.to_le_bytes());
        e
    }

    #[test]
    fn overlay_table_fields_and_flag() {
        let mut table = entry(3, FLAG_COMPRESSED | 0x100);
        table.extend(entry(4, 0));
        let ov = parse_overlay_table(&table).unwrap();
        assert_eq!(ov.len(), 2);
        assert_eq!(ov[0].file_id, 3);
        assert!(ov[0].is_compressed());
        assert!(!ov[1].is_compressed());
        assert!(parse_overlay_table(&table[..40]).is_err());
    }

    #[test]
    fn uncompressed_overlay_is_returned_as_is() {
        let payload = vec![7u8, 8, 9];
        let rom = build_rom(&[], std::slice::from_ref(&payload));
        let header = Header::parse(&rom).unwrap();
        let fs = Filesystem::parse(&rom, &header).unwrap();
        let (arm9, arm7) = read_overlay_tables(&rom, &header).unwrap();
        assert_eq!(arm9.len(), 1);
        assert!(arm7.is_empty());
        assert_eq!(overlay_data(&rom, &fs, &arm9[0]).unwrap(), payload);
    }

    #[test]
    fn memory_map_lists_segments_and_overlays() {
        let rom = build_rom(&[], &[vec![0u8; 4]]);
        let header = Header::parse(&rom).unwrap();
        let (arm9, arm7) = read_overlay_tables(&rom, &header).unwrap();
        let json = memory_map_json(&header.arm9, &header.arm7, &arm9, &arm7);
        assert!(json.contains("\"name\": \"arm9\""));
        assert!(json.contains("\"ram_addr\": 33554432")); // 0x0200_0000
        assert!(json.contains("\"table\": \"arm9\""));
    }
}
