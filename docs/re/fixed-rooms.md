# Fixed rooms: the fixed-room file and ProcessFixedRoom

**Date:** 2026-11-12
**Status:** partial. File format, decoder, tile setter and both placement paths ported; spawn effects and staged rooms still open
**Subsystem:** dungeon floor generation (fixed rooms)

Issue: [#342](https://github.com/TorratDev/rust-mm/issues/342), parent [#111](https://github.com/TorratDev/rust-mm/issues/111). Parent notes: [`dungeon-generation.md`](dungeon-generation.md).

## Addresses
| Address | Name | Source |
|---------|------|--------|
| `ov_0001::02150958` | `ProcessFixedRoom(number, props)`; w or h 0 -> one-room monster house, false; number < 50 -> `0214A0D4`, true; else `02150788`, false | [blue-verified] static |
| `ov_0001::0214A0D4` | places a fixed room below 50 (a w x h block of decoded tiles, wall elsewhere) | [blue-verified] static |
| `ov_0001::02150788`, `0214A3D0`, `02149E80` | rooms 50 and up: grid placement, std4 variant, stamp | [blue-verified] static |
| `ov_0001::0214A348` | tile-stream decoder | [blue-verified] static |
| `ov_0001::02152B20` | per-tile setter for a tile code | [blue-verified] static, codes `0x10`+ not decoded |
| `0201A54C` | `OpenFileAndGetFileDataPtr("fixedmap")` | [blue-verified] |

## File and decoder (ported)
- `fixedmap`: SIR0 container in `dungeon.sbin`. Entry `N` is the file offset at index `N` of a `u32` array; entry = width, height, flags, tile stream. [blue-verified] for the entry; [hypothesis] that the array starts at the SIR0 content pointer (header word `+4`); the game reaches it through the file handle's `+0xC`. Not run.
- Decoder: byte `0x0E` then a literal code; otherwise code `b >> 4` repeated `(b & 0xF) + 1` times, row by row. [blue-verified] static; [hypothesis] that the escape ## Placement (ported)
Sources: `~/rescue-re/scratch/findings-342.md` and `findings-342b.md` (ghidra-cli decompile and disassembly). All [blue-verified] static unless marked; no emulator run in this port.

| Address | What | Rust |
|---------|------|------|
| `02152B20` via wrapper `0214A318` | tile setter: wrapper sets terrain `0x100`, room 0; codes 0-0xF, 0x43-0x45, table path for `0x10`-`0xDB`; codes above `0xDB` only get type 1 | `floor::set_fixed_tile` |
| `021AC208` | 220 x 16-byte table; offsets +0 item, +2 qty, +4 slot or item flags, +8 event/monster id, +0xA tag, +0xB trap (0x14 none), +0xC room, +0xD trap param, +0xE type-2 flag | passed in as `PortedSteps::fixed_table` (game data, not read by the sim) |
| `0214A0D4` | number < 50: w x h block at (5, 5), every other tile a wall (type 3 when `021B45A8`), then FinalizeJunctions; no RNG | `floor::place_fixed_room` |
| `02150788` | number >= 50: AssignRooms, up to 64 picks `(int(gx), int(gy))` for a room cell, `0214A3D0`, then (grid not 1 x 1) `0214F224` from the pick, `0214E824(.., 1)`, `0214DB54`, stamp | `PortedSteps::place_fixed_room_large` |
| `0214A3D0` | step-4 variant: picked cell gets the entry size at (x0, y0) with no draw; others draw (min 5, max cell size; width 10-14 if gx < 3; height 16-24 if gy == 1); anchors 0xFF | `floor::place_fixed_grid_rooms` |
| `02149E80` | stamp of the stream into the picked cell, keeping each tile's room byte (flags bit 0 clear) | `floor::stamp_fixed_room` |

Not ported, with the reason:
- Item, trap and monster spawns of the setter (`02153190`, `02152FAC`, `02153018`) need the object system. They are recorded in `FixedRoomOut::spawns`, in setter order, and nothing runs them. Their own RNG draws are not traced, so the RNG count of a floor with such codes is unchecked (the run saw 1 unattributed draw for number 5).
- Code 7's call to `0214961C` (out-of-bounds tile global): the flag `021B45A8` is kept, the call is not.
- Staged rooms (entry flags bit 0, revealed by `0215277C`): `stamp_fixed_room` panics. [hypothesis] that no retail room sets the bit; not checked.
- Number 4 together with the tileset flag: the rectangle x 2..4, y 5..16 has unknown bounds, so `place_fixed_room` panics.
- Cell byte `+0x1B` (set to 1 by `02150788`, meaning unknown) is not modelled.
- [hypothesis] `0214A3D0` differs from `place_rooms` (height margin 4 against 3, anchors 0xFF against 0xFE, no 80 % flag draw). Taken as the findings give it; no run compares the RNG count (the earlier run for number 60: std3 to std6 = 192 steps).
- [hypothesis] After 64 missed picks the last pick's cell is stamped.
- The numbers 1-66 are not checked against a retail tile hash.

Rust: `floor::fixed_map_entry`, `floor::TileStream`, `PortedSteps::{fixed_map, fixed_table, tileset, fixed_out}`.
