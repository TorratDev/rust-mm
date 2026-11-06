# NDS container: header, filesystem, binaries, overlays

Issues: [#28](https://github.com/TorratDev/rust-mm/issues/28) header, [#29](https://github.com/TorratDev/rust-mm/issues/29) FNT/FAT, [#30](https://github.com/TorratDev/rust-mm/issues/30) binaries, overlays and BLZ. Code: `crates/nds` (`header.rs`, `fs.rs`, `binary.rs`, `blz.rs`). Layout is from GBATEK, little-endian.

**Verification status:** implemented and tested on synthetic ROMs, and verified against the Blue EUR dump (2026-10-16): `tests/tests/nds_rom.rs` passes with `RESCUE_REQUIRE_ROM=1` (24 named files, 169 FAT entries, 145 overlays, type chart in overlay 1).

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
| `0x1C` | flags: bits 0–23 compressed size, **bit 24 = BLZ-compressed** (issue #30; the overlay loader `FUN_0207BC9C` tests it as bit 0 of byte `+0x1F`, #283 P4a) |

Bit 24 is taken from the issue. Whether the stored size is in bits 0–23 is per GBATEK and is still to be checked on the dump.

## BLZ ("backwards LZ")

Footer: `0x00` (u32) = footer length in the top 8 bits and compressed length in the low 24 bits. The footer length is 8 in most overlays, but the Blue dump has 9 (overlay 6), so the decoder accepts 8–24. Bytes before the compressed region are stored raw. `0x04` (u32) = growth in bytes. Decoding reads the stream from the end and writes the output from the end. Output is capped at 16 MiB (`blz::MAX_OUTPUT`). Full algorithm: the module comment in `blz.rs`. The Blue ARM9 decoder (`FUN_02000934`, [`re/io.md`](../re/io.md#not-found-and-a-negative), #283 P4a) reads the same two words at the end of the data (at end − 8 and end − 4), with the footer length in the top 8 bits of the first word, which agrees with this layout.

**Open:** whether `arm9.bin` is BLZ-compressed. The Blue ARM9 contains the BLZ decoder (`FUN_02000934`, #283 P4a). `_start` calls it only when the module-parameter compressed-static-end field is nonzero: the argument is the word at `+0x14` of the structure whose pointer is the literal at `0x0200090C`. That field's value was not read, so the flag is not decided. The survey tagged it *unknown* (453,292 bytes, entropy 7.21). Answer it from the dump.

## Memory map

`binary::memory_map_json` writes segment and overlay addresses and sizes as JSON for the Ghidra loader (V8). It contains no ROM bytes.

## Open items

- Confirm the header fields not yet used (the banner contents, the secure-area layout).
- Confirm the BLZ footer and flags against the dump; run `nds_rom.rs`.
- Decide the ARM9 compression question above.
