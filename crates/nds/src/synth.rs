//! Builds small synthetic ROMs for tests. Nothing here comes from a real dump.

use crate::header::{CRC_OFFSET, HEADER_LEN, crc16};

/// Overlay table entry size.
const OVERLAY_LEN: usize = 32;

fn put_u32(rom: &mut [u8], at: usize, v: u32) {
    rom[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// A ROM with a flat root directory holding `named` files and, after them in the FAT,
/// `overlays` (no names, referenced from the ARM9 overlay table).
pub fn build_rom(named: &[(&str, &[u8])], overlays: &[Vec<u8>]) -> Vec<u8> {
    let mut rom = vec![0u8; HEADER_LEN];
    rom[..12].copy_from_slice(b"TESTROM\0\0\0\0\0");
    rom[0x0C..0x10].copy_from_slice(b"TEST");
    rom[0x10..0x12].copy_from_slice(b"01");

    let arm9_off = rom.len();
    rom.extend_from_slice(&[0xA9; 0x40]);
    let arm7_off = rom.len();
    rom.extend_from_slice(&[0xA7; 0x20]);

    let ov9_off = rom.len();
    rom.resize(rom.len() + overlays.len() * OVERLAY_LEN, 0);

    // FNT: one root directory (id 0xF000). Main entry = subtable offset, first file id,
    // parent field (= number of directories, 1 here).
    let fnt_off = rom.len();
    rom.extend_from_slice(&8u32.to_le_bytes());
    rom.extend_from_slice(&0u16.to_le_bytes());
    rom.extend_from_slice(&1u16.to_le_bytes());
    for (name, _) in named {
        rom.push(name.len() as u8);
        rom.extend_from_slice(name.as_bytes());
    }
    rom.push(0);
    let fnt_len = rom.len() - fnt_off;

    let fat_off = rom.len();
    let count = named.len() + overlays.len();
    rom.resize(rom.len() + count * 8, 0);
    let datas = named
        .iter()
        .map(|(_, d)| d.to_vec())
        .chain(overlays.iter().cloned());
    for (id, data) in datas.enumerate() {
        let start = rom.len();
        rom.extend_from_slice(&data);
        let at = fat_off + id * 8;
        put_u32(&mut rom, at, start as u32);
        let end = rom.len() as u32;
        put_u32(&mut rom, at + 4, end);
    }

    for (i, data) in overlays.iter().enumerate() {
        let at = ov9_off + i * OVERLAY_LEN;
        put_u32(&mut rom, at, i as u32);
        put_u32(&mut rom, at + 4, 0x0220_0000 + (i as u32) * 0x1000);
        put_u32(&mut rom, at + 8, data.len() as u32);
        put_u32(&mut rom, at + 0x18, (named.len() + i) as u32);
    }

    // Header fields. The offsets use the layout in `header.rs`.
    put_u32(&mut rom, 0x20, arm9_off as u32);
    put_u32(&mut rom, 0x24, 0x0200_0800);
    put_u32(&mut rom, 0x28, 0x0200_0000);
    put_u32(&mut rom, 0x2C, 0x40);
    put_u32(&mut rom, 0x30, arm7_off as u32);
    put_u32(&mut rom, 0x34, 0x0380_0000);
    put_u32(&mut rom, 0x38, 0x0380_0000);
    put_u32(&mut rom, 0x3C, 0x20);
    put_u32(&mut rom, 0x40, fnt_off as u32);
    put_u32(&mut rom, 0x44, fnt_len as u32);
    put_u32(&mut rom, 0x48, fat_off as u32);
    put_u32(&mut rom, 0x4C, (count * 8) as u32);
    put_u32(&mut rom, 0x50, ov9_off as u32);
    put_u32(&mut rom, 0x54, (overlays.len() * OVERLAY_LEN) as u32);
    let used = rom.len() as u32;
    put_u32(&mut rom, 0x80, used);

    let crc = crc16(&rom[..CRC_OFFSET]);
    rom[CRC_OFFSET..CRC_OFFSET + 2].copy_from_slice(&crc.to_le_bytes());
    rom
}
