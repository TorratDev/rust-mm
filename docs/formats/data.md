# Data tables

Issue: [#37](https://github.com/TorratDev/rust-mm/issues/37). Code: `crates/data/src/` (`species.rs`, `moves.rs`, `item.rs`, `dungeon.rs`, `types.rs`). Tests: `tests/tests/data_rom.rs` (counts, cross-table index checks, a SHA-1 snapshot per table), mutation test in `tests/tests/formats_rom.rs`. Viewer: `system.sbin` → `monspara`, `wazapara`, `itempara`, `lvmpNNN`. Red/Blue comparison: [`blue-vs-red.md`](../blue-vs-red.md#data-tables-formatsdatamd-37).

**Verification status:** verified against Blue (2026-10-16): every table parses, indices between tables are consistent, and species, moves and items equal Red's values field for field. Field names are Red's (`pmd-red` structs).

| Table | Where | Layout |
|-------|-------|--------|
| species | `system.sbin` `monspara`, SIR0 | 424 x 0x48-byte `MonsterDataEntry` (name and category pointers first) |
| moves | `system.sbin` `wazapara` (+ `wazapar{f,g,i,s}`), SIR0 | content header `{moves pointer, learnsets pointer}`; 413 x 0x24-byte `MoveDataEntry` |
| learnsets | same file | 424 x `{level-up stream pointer, TM/HM stream pointer}`; species 0 has null pointers |
| items | `system.sbin` `itempara` (+ `itempar{f,g,i,s}`), SIR0 | 240 x 0x20-byte `ItemDataEntry` |
| level growth | `system.sbin` `lvmpNNN` (421 files), SIR0 + AT4PX | 100 x 12-byte `LevelData` `{s32 exp, u16 HP, u8 atk[2], u8 def[2], pad}` |
| dungeon floors | `dungeon.sbin` `mapparam`, SIR0 | `DungeonMapParam2`, below |
| type chart | decoded ARM9 overlay 1, offset `0x72688` | 18 x 18 `s16`, values 0–3 |

Move streams: a move ID is 1 byte, or 2 when bit 7 of the first is set (`(b0 & 0x7F) << 7 | (b1 & 0x7F)`); level-up streams add a level byte after each ID; 0 ends a stream.

## `mapparam`

Content header, five pointers:

1. per-dungeon floor tables (64 pointers). A floor is 16 bytes: `s16` floor-properties index, `s16` monster list, `s16` trap list, `s16` item lists for normal / shop / monster house / buried, `s16` unused. A dungeon's table runs to the next one.
2. `FloorProperties` array: 1764 x 28 bytes (Red's struct: layout, room density, tileset, music, weather, ...).
3. item spawn lists (183): `u16` weights; a value `>= 30000` means `value - 30000` zeros. Expanded length = 12 categories + 240 item IDs.
4. monster spawn lists (839): 6 bytes `{u16 species (bits 0–8) | level (bits 9–15), s16 weight[2]}` until species 0.
5. trap chance lists (148): 20 `u16` each.

## Resource IDs

Species index = `kaoNNN`, `axNNN`, `lvmpNNN` number. Dungeon index = position in table 1. Names come from the tables themselves (species, moves, items); dungeon names are in the message tables (index range not mapped yet).

## Unknown fields

- Species bytes `0x12`, `0x1A`, `0x30`–`0x33`, `0x46`; move bytes `0x07`, `0x1A`–`0x1B`; item byte `0x1F` (padding in Red).
- The unused sixth `s16` of each floor; floor 0 of each dungeon (a placeholder in Red).
- Dungeon names and floor counts outside `mapparam` (Red keeps them in `gDungeons`).
