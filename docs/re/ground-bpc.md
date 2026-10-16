# Ground map loading: BPC, BPA and BMA

Blue's code that loads a ground (town / story scene) map's background, and the BPC compression it reads. Red stores BPC tiles and chunks raw; Blue compresses both. Issue: [#35](https://github.com/TorratDev/rust-mm/issues/35). Format spec: [`formats/maps.md`](../formats/maps.md). Parser: `crates/formats/src/ground.rs`.

Dump: `BlueRescueTeamEur` ([`supported-dumps.md`](../supported-dumps.md)). All code is in overlay 0 (`ov_0000::`), read with ghidra-cli on the `rescue-arm9` project, 2026-10-16.

Tags: **[blue-verified]** means read in Blue's code; **[rom-checked]** means the reading was then checked against every ground map in the ROM (`ground_maps_parse` in `tests/tests/formats_rom.rs`); **[hypothesis]** means not confirmed. No emulator check was done.

## How it was found

The strings `cell size x size over %d`, `cell map x size over %d`, `pattern max over %d`, `file open error` and `pattern animation size error %d %d %d` (`ov_0000::0215b250` ... `0215b3b4`) are the asserts of Red's `ground_bg.c`. `x-ref to ov_0000::0215b250` gives `FUN_ov_0000__0214a8f4` (the map loader) and `FUN_ov_0000__0214999c` (the BMA reader).

## Functions [blue-verified]

| Address | Red counterpart `[hypothesis]` | What it does |
|---------|----------|--------------|
| `ov_0000::0214a8f4` | map loader in `ground_bg.c` | Looks up the map's row (0x1C bytes: 7 file names) in the table at `DAT_ov_0000__0214b1a0`, opens BPL, BPC and BMA, copies the BPC header (8 `u16`), checks chunk size `< 4`, BMA chunk size equals BPC chunk size, BMA width `<= 0x40`, then calls the three decoders below. Then opens up to 4 BPA files (names 4-7 of the row): slots 0 and 1 are kept for animation, slots 2 and 3 have frame 0 copied once. Asserts each BPA's tile count equals the BPC header's count for that slot. |
| `ov_0000::02149fc8` | `CopyBpcTilesToVram` | Decodes the BPC tiles (below), from BPC offset 16. Clears tile 0 first, decodes until `(header[2]) * 32` bytes are filled (so `header[2] - 1` stored tiles), returns the input pointer rounded up to an even offset from the start. |
| `ov_0000::02149d30` | chunk part of `_UncompressCell` | Decodes `w * h * (header[7] - 1)` chunk entries from that pointer (below), then spreads them into 9-entry slots from chunk 1 on, adding the VRAM tile base and palette base (`base | pal << 12`) to each. Chunk 0 is all zero. Called with a 5th stack argument, the BPC header copy (Ghidra shows 4 at the call site). |
| `ov_0000::0214999c` | BMA layer reader | Called with the BMA at offset 12. The current BMA parser already matches every map, so it was not re-read. |

## BPC tile coding (`02149fc8`) [blue-verified] [rom-checked]

A byte stream. The code writes `u16`s with a pending-odd-byte flag, which is the same as a plain byte stream. State: `last` and `before` fill bytes, both 0 at the start. Each command emits n + 1 bytes:

| Command | n | Bytes |
|---------|---|-------|
| `0x00-0x7D` | the command | literal, copied from the input |
| `0x7E` | next byte | literal |
| `0x7F` | next `u16` (little endian) | literal |
| `0x80-0xBE` | command - 0x80 | then one byte `v`: `before = last; last = v`; emits `v` |
| `0xBF` | next byte | same, `v` after the count |
| `0xC0-0xDE` | command - 0xC0 | `last` again |
| `0xDF` | next byte | `last` again |
| `0xE0-0xFE` | command - 0xE0 | `before`, and `before` and `last` swap |
| `0xFF` | next byte | same |

Decoding stops once the output is full; a command can run past the end (the extra bytes are dropped).

## BPC chunk coding (`02149d30`) [blue-verified] [rom-checked]

Two passes over the `count` entries: the first writes high bytes, the second ORs in low bytes. Per pass, commands until `count` entries are covered:

| Command | Effect |
|---------|--------|
| `0x00-0x7F` | skip command + 1 entries (0 in the high pass, unchanged in the low pass) |
| `0x80-0xBF` | one byte, repeated for (command & 0x3F) + 1 entries |
| `0xC0-0xFF` | (command & 0x3F) + 1 literal bytes |

Entries are DS tilemap entries: tile index (bits 0-9), flips (10, 11), palette (12-15).

## BPA [blue-verified] [rom-checked]

`u16 tiles`, `u16 frames`, 4 bytes per frame (timing; layout not read), then `frames * tiles` raw 4bpp tiles. Slot `i` of map `X` is the sub-file `X{i+1}` [rom-checked: every slot the BPC header declares has a file of that name with that tile count; the table itself was not dumped].

## ROM checks

On the supported dump, `ground_maps_parse`:

- All 194 BPC files decode, and the chunk data ends within 3 bytes (padding) of the file end (one-off probe, not kept as a test).
- Every chunk entry names a tile below `tiles + sum(bpa_tiles)`, and every non-empty entry a palette the map's BPL has.
- 201 chunk maps; the 187 with their own BPC and BPL render to non-empty images. 14 maps (`B01P02Am`, ...) lack their own `c` or palette part `[hypothesis: they share another map's]`.
- Renders of `A01P01`, `A05P01` (moon over a night sky) and `H01P01W` (underwater overlay, BPA tiles only) were checked by eye.

## Open

- Which loader table row each map uses, and so the maps without their own parts.
- BPA frame info layout (frame durations, `[hypothesis]` from Red).
- BMA layer priority (which layer is drawn on top).
