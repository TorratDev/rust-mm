# Fixed rooms: the fixed-room file and ProcessFixedRoom

**Date:** 2026-11-12
**Status:** partial. File format and stream decoder [blue-verified] (static); tile placement not ported
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
- Decoder: byte `0x0E` then a literal code; otherwise code `b >> 4` repeated `(b & 0xF) + 1` times, row by row. [blue-verified] static; [hypothesis] that the escape clears the repeat count.

## Not ported (needs more findings)
- The tile setter `02152B20` and the `0x10`-`0xDB` table at `021AC208` (fields not decoded), so `place_fixed_room` (`0214A0D4`) still panics.
- Open points of `0214A0D4`: the findings give the block as `x in 5..w+4` and as "w x h at (5,5)", which differ by one; the tileset flag (`0214A314` pool) and dungeon `+0x34AE` are not modelled.
- Rooms 50 and up: `0214A3D0`, `0214F224` and the stamp `02149E80`.

Rust: `floor::fixed_map_entry`, `floor::TileStream`, `PortedSteps::fixed_map`.
