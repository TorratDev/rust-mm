# NDS container: header, filesystem, binaries, overlays

Issues: [#28](https://github.com/TorratDev/rust-mm/issues/28) header, [#29](https://github.com/TorratDev/rust-mm/issues/29) FNT/FAT, [#30](https://github.com/TorratDev/rust-mm/issues/30) binaries, overlays and BLZ. Code: `crates/nds` (`header.rs`, `fs.rs`, `binary.rs`, `blz.rs`). Layout is from GBATEK, little-endian.

**Verification status:** implemented and tested on synthetic ROMs. The ROM-gated checks in `tests/tests/nds_rom.rs` have not run against the reference dump in this environment, so the counts below (24 named files, 169 FAT entries, 145 overlays) are from [`file-survey.md`](../file-survey.md) and still need that run.

## Header (`0x000`–`0x1FF`)

| Offset | Size | Field | Notes |
|--------|------|-------|-------|
| `0x000` | 12 | title | NUL-padded |
| `0x00C` | 4 | game code | |
| `0x010` | 2 | maker code | |
| `0x020` | 4 | ARM9 ROM offset | `0x024` entry, `0x028` RAM address, `0x02C` size |
| `0x030` | 4 | ARM7 ROM offset | `0x034` entry, `0x038` RAM address, `0x03C` size |
| `0x040` | 8 | FNT offset, size | |
| `0x048` | 8 | FAT offset, size | |
| `0x050` | 8 | ARM9 overlay table offset, size | |
| `0x058` | 8 | ARM7 overlay table offset, size | |
| `0x068` | 4 | banner offset | |
| `0x080` | 4 | used ROM size | |
| `0x15E` | 2 | header CRC-16 | covers `0x000`–`0x15D`; reflected poly `0xA001`, init `0xFFFF` |

`Header::parse` checks the CRC first, then that every range lies inside the ROM. Zero-size ranges are allowed.

## Filesystem

- **FNT.** The root main-table entry at `FNT+6` holds the directory count. Each 8-byte main entry gives a subtable offset and a first file ID. A subtable entry is a length byte (bit 7 = directory, then a 2-byte directory ID) followed by the name. A zero byte ends the subtable. The walk is iterative and rejects cycles and out-of-range IDs.
- **FAT.** 8 bytes per file ID: start and end offsets. Named files get IDs in walk order; overlays have FAT entries but no name.

## Overlay table (32-byte entries)

| Offset | Field |
|--------|-------|
| `0x00` | overlay ID |
| `0x04` | RAM address |
| `0x08` | RAM size |
| `0x0C` | BSS size |
| `0x10`, `0x14` | static initializer start, end |
| `0x18` | FAT file ID |
| `0x1C` | flags: bits 0–23 compressed size, **bit 24 = BLZ-compressed** (issue #30) |

Bit 24 is taken from the issue. Whether the stored size is in bits 0–23 is per GBATEK and is still to be checked on the dump.

## BLZ ("backwards LZ")

Footer: `0x00` (u32) = footer length in the top 8 bits (must be 8) and compressed length in the low 24 bits. `0x04` (u32) = growth in bytes. Decoding reads the stream from the end and writes the output from the end. Output is capped at 16 MiB (`blz::MAX_OUTPUT`). Full algorithm: the module comment in `blz.rs`.

**Open:** whether `arm9.bin` is BLZ-compressed. The survey tagged it *unknown* (453,292 bytes, entropy 7.21). The decoder is available, but the ARM9 compression flag is not decided. Answer it from the dump.

## Memory map

`binary::memory_map_json` writes segment and overlay addresses and sizes as JSON for the Ghidra loader (V8). It contains no ROM bytes.

## Open items

- Confirm the header fields not yet used (the banner contents, the secure-area layout).
- Confirm the BLZ footer and flags against the dump; run `nds_rom.rs`.
- Decide the ARM9 compression question above.
