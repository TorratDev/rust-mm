# Data tables

Issue: [#37](https://github.com/TorratDev/rust-mm/issues/37). Code: `crates/data/src/` (`species.rs`, `moves.rs`, `item.rs`, `dungeon.rs`, `types.rs`, `recruit.rs`). Tests: `tests/tests/data_rom.rs` (counts, cross-table index checks, a SHA-1 snapshot per table), mutation test in `tests/tests/formats_rom.rs`. Viewer: `system.sbin` → `monspara`, `wazapara`, `itempara`, `lvmpNNN`. Red/Blue comparison: [`blue-vs-red.md`](../blue-vs-red.md#data-tables-formatsdatamd-37). Evidence: [`re/data-tables.md`](../re/data-tables.md).

**Verification status:** the tables parse from Blue's EUR dump and their indices agree. Species, moves and items match `pmd-red`'s data field for field, with the exceptions below. Level growth, floor properties, floors, monster, trap and item lists match Red's values. The monster spawn lists were read with the wrong record size until 2026-10-18 (see `mapparam`). Byte offsets were checked against the getters of Blue's code ([`re/data-tables.md`](../re/data-tables.md)).

| Table | Where | Layout |
|-------|-------|--------|
| species | `system.sbin` `monspara`, SIR0 | 424 x 0x48-byte `MonsterDataEntry` (name and category pointers first) |
| moves | `system.sbin` `wazapara` (+ `wazapar{f,g,i,s}`), SIR0 | content header `{moves pointer, learnsets pointer}`; 413 x 0x24-byte `MoveDataEntry` |
| learnsets | same file | 424 x `{level-up stream pointer, TM/HM stream pointer}`; species 0, 420 and 421 read an empty stream (see below) |
| items | `system.sbin` `itempara` (+ `itempar{f,g,i,s}`), SIR0 | 240 x 0x20-byte `ItemDataEntry` |
| level growth | `system.sbin` `lvmpNNN` (421 files), SIR0 + AT4PX | 100 x 12-byte `LevelData` `{s32 exp, u16 HP, u8 atk[2], u8 def[2], pad}` |
| dungeon floors | `dungeon.sbin` `mapparam`, SIR0 | `DungeonMapParam2`, below |
| type chart | decoded ARM9 overlay 1, offset `0x72688` | 18 x 18 `s16`, values 0–3 |
| recruit rate by level | decoded ARM9 overlay 1, offset `0x7210C` | 101 x `s16`, below |

Move streams: a move ID is 1 byte, or 2 when bit 7 of the first is set (`(b0 & 0x7F) << 7 | (b1 & 0x7F)`, `FUN_02001c20`); level-up streams add a level byte after each ID; 0 ends a stream.

## Species (`monspara`)

- Regeneration byte `+0x1B`: the speed is `(b & 0x7F) << 1` (`FUN_02002368`, `blue-verified`). Bit 7 is a separate flag; the parser keeps both (`regen_speed`, `regen_flag`).
- `recruit_rate` (`i16` at `+0x40`) is per mille (`blue-verified`, `FUN_ov_0001__0217f9b8`).
- `byte_12` (`+0x12`) and `byte_33` (`+0x33`) are read by overlay 1 (`FUN_02002288`, `FUN_020022a4`). Their meaning is unknown.

## Moves (`wazapara`)

Field offsets confirmed by Blue's accessors (`GetMoveBasePower` `0200b3e8`, `GetMoveType` `0200b4cc`, `GetMoveBasePP` `0200b3a4` and others): power `+4`, type `+6`, targeting `+8`/`+0xA`, base PP `+0xC`, AI weight `+0xD`, accuracy `+0xE`–`+0x10`, chained hits `+0x11`, max upgrade `+0x12`, crit `+0x13`, flags `+0x14`–`+0x18`, range `+0x19`, description `+0x1C`, use text `+0x20`. The module comment's byte order is fixed.

## Learnsets

- Species 0, 420 (`0x1A4`) and 421 (`0x1A5`) read a fixed empty stream in Blue (`FUN_0200b448`, `FUN_0200b48c`, `FUN_02001844`; pointer `0x02091C84`). The table holds TM/HM entries for 420 (2) and 421 (59), but the game never reads them, so the parser returns empty learnsets for those species.
- Level-up moves 0xEE (238), 0xEF (239), 0x110 (272) and 0x162 (354) are skipped unless a monster field at `+0xA` is at least 333 (`02091E18`–`02091E34`, `blue-verified` values; the field is unidentified). Learnsets can hold at most 413 moves (`DAT_02001a2c`). The parser returns the raw streams; the game applies these gates.

## Items (`itempara`)

- `move_id` is an unsigned `u16` at `+0x18` (read as `u16` by `FUN_02007670`).
- The price fields `+4` and `+8` are read by the getters `FUN_02008154` / `FUN_0200813c`. Which one is buy and which is sell is not confirmed.
- Icon `+0xD` is read by overlay 1 (`FUN_ov_0001__02143f7c`). Byte `+0x1F` has no reader found.
- Validity (`FUN_0200747c`): an item is valid only if both prices are non-zero, and IDs `0`, `0x69`, `0x77`–`0x7A` are excluded from normal items. ID `0x69` has its own display path, probably money.

## `mapparam`

Content header, five pointers:

1. per-dungeon floor tables (64 pointers). A floor is 16 bytes: `s16` floor-properties index, `s16` monster list, `s16` trap list, `s16` item lists for normal / shop / monster house / buried, `s16` unused. A dungeon's table runs to the next one. The sixth `s16` is never read.
2. `FloorProperties` array: 1764 x 28 bytes (Red's struct: layout, room density, tileset, music, weather, ...). The count is not stored; it is the distance from this array to the first monster list (`0x7380 + 1764 x 28 = 0x13470`).
3. item spawn lists (178 `[inferred]`: counted from the pointer table, the file has no count): `u16` weights; a value `>= 30000` means `value - 30000` zeros. Expanded length = 12 categories + 240 item IDs.
4. monster spawn lists (839): 8-byte records `{u16 species (bits 0–8) | level (bits 9–15), s16 weight[2], u16 pad}` until species 0. Every list is followed by its terminator record, which ends exactly where the next list starts (838 of 838; the last list is followed by other data). The parser fails if this does not hold. Blue's loop caps each list at 32 entries; the largest list in the file has 11 (investigator scan).
5. trap chance lists (148 `[inferred]`, from the pointer table): 20 `u16` each.

Total monster spawn entries: 5632.

## Recruit rate by level

`gRecruitRateByLevel` is 101 `s16` values (levels 0 to 100) at offset `0x7210C` in decoded overlay 1 (`blue-verified`). The values are identical to Red's, with 240 at levels 90 to 100. The recruit check (`FUN_ov_0001__0217f9b8`) rolls `DungeonRandInt(1000)`, reads the species rate, fails on `-999`, adds 100 for Friend Bow, adds the level rate, and fails when the roll is at or above the rate. The level comes from the recruiter's info `+0x09`; which argument is the recruiter is inferred from the call order.

## Resource IDs

Species index = `kaoNNN`, `axNNN`, `lvmpNNN` number. Dungeon index = position in table 1. Names come from the tables themselves (species, moves, items); dungeon names are in the message tables (index range not mapped yet).

## Unknown fields

- Species bytes `0x0A`–`0x0B` and `0x46`–`0x47` differ from Red in every species (Red has no field there); `0x12` and `0x33` are read by overlay 1 (`byte_12`, `byte_33`); `0x1A`, `0x30`–`0x32`, `0x46`.
- Move bytes `0x07` and `0x1A`–`0x1B` have no reader in Blue.
- Item bytes `0x1F` (padding in Red); item icon `0xD` is read by overlay 1.
- The unused sixth `s16` of each floor; floor 0 of each dungeon (a placeholder in Red).
- Dungeon names and floor counts outside `mapparam` (Red keeps them in `gDungeons`).

## Open

Not verified; do not treat these as known:

- The gate field at `+0xA` of the monster struct (the level-up gate at 333).
- The meanings of species `+0x12` and `+0x33`, and of regen bit 7.
- Which item price is the buy price and which the sell price.
- Move bytes `+0x07` and `+0x1A`–`+0x1B`; item byte `+0x1F`.
- The ground-side `mapparam` reader in overlay 0 (`FUN_ov_0000__0215869c`).
- The `lvmpNNN` loader (`FUN_02001c48`): only the file layout is confirmed.
