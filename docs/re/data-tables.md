# Data tables: readers, globals and evidence

**Date:** 2026-10-18
**Status:** blue-verified for the layouts, offsets and readers listed; hypothesis for the meanings of the flag bytes and gate fields
**Subsystem:** data tables (issue [#37](https://github.com/TorratDev/rust-mm/issues/37))

Dump: `BlueRescueTeamEur` ([`supported-dumps.md`](../supported-dumps.md)). Addresses are ARM9 or overlay RAM as marked. Spec: [`formats/data.md`](../formats/data.md). Comparison with Red: [`blue-vs-red.md`](../blue-vs-red.md#data-tables-formatsdatamd-37).

Tags: **[blue-verified]** read in Blue's code, or confirmed by a ROM scan; **[hypothesis]** a reading that is not yet checked; **[unverified]** not checked.

## Tables

| Table | Reader | Globals | File | Layout | Tag |
|-------|--------|---------|------|--------|-----|
| species | `LoadMonsterParameters` `02003794`, field getters (29 checked, e.g. `FUN_02002288`, `FUN_020022a4`, `FUN_02002368`) | table pointer `020DDF80` | `system.sbin` `monspara` | 0x48 bytes per entry; fields in `species.rs` | blue-verified (offsets); attack/defence order medium |
| moves | `FUN_0200b888` (language-indexed SIR0 load into a 0x18000 buffer) | move table `020DED10`, learnsets `020DED08` | `wazapara`, `wazapar{f,g,i,s}` | 0x24 bytes per move; accessors `GetMoveBasePower` `0200b3e8`, `GetMoveType` `0200b4cc`, `GetMoveBasePP` `0200b3a4`, `FUN_0200b4ec` and others | blue-verified (offsets); `+0x07`, `+0x1A`–`+0x1B` unread |
| learnsets | `FUN_0200b48c` (level-up at `+0`), `FUN_0200b448` (TM/HM at `+4`) | as moves | same file | 8 bytes per species (424 entries) | blue-verified |
| move stream decoder | `FUN_02001c20` (ID), `FUN_02001844` (level-up walk with level bytes) | — | — | 1 or 2 bytes per ID; `(b0 & 0x7F) << 7 \| (b1 & 0x7F)` | blue-verified |
| items | `FUN_02008614` (0xC000 buffer, language-indexed) | item table `020DECF4` | `itempara`, `itempar{f,g,i,s}` | 0x20 bytes per item; getters `FUN_02007e20` (name), `FUN_02008154` / `FUN_0200813c` (prices), `FUN_020082fc` (category), `FUN_020080c0` (description), `FUN_020080a4` (AI flags), `FUN_02007670` (move, u16), `FUN_02008124` (order), `FUN_020080d8` (spawn), `FUN_0200810c` (palette), `FUN_020080f4` (action) | blue-verified (offsets); price direction unverified |
| item validity and special IDs | `FUN_0200747c` | — | — | both prices non-zero; IDs `0`, `0x69`, `0x77`–`0x7A` excluded | blue-verified |
| special move IDs | `FUN_020092bc` | — | — | 8, 0x3C, 0x4B, 0x64, 0x97, 0x99, 0x9C, 0xCD, 0x15C | blue-verified |
| level growth | `lvmpNNN` loader `FUN_02001c48` | — | `lvmpNNN` (421 files) | SIR0 + AT4PX, 100 x 12 bytes | file layout blue-verified; loader [unverified] |
| dungeon floors | `FUN_ov_0001__02138ccc` (monster loader, `iVar10 * 8`), item and trap loads by the same loader | — | `dungeon.sbin` `mapparam` | see below | blue-verified |
| monster spawn end | `FUN_02000eec` (species bits `& 0x1FF`, stops at 0) | — | — | 8-byte records | blue-verified |
| type chart | `FUN_ov_0001__0217ba94` and callers | — | decoded overlay 1, `0x72688` | 18 x 18 `s16` | blue-verified (V6) |
| recruit rates | `FUN_ov_0001__0217f9b8` | `gRecruitRateByLevel` | decoded overlay 1, `0x7210C` | 101 x `s16` | blue-verified |

## `mapparam` (ROM scans)

Scan of `pmdb.nds` through the repository's FNT walker (`dungeon.sbin`, `mapparam`), done 2026-10-18 by the parser's own checks in `tests/tests/data_rom.rs`:

- 64 dungeons, 1831 floors, 839 monster lists, 148 trap lists, 178 item lists.
- Monster lists: 5632 entries in total (the parser's count, checked 2026-10-18). Each list ends with a species-0 record; that record ends exactly where the next list starts for 838 of the 838 lists that have a successor (checked 2026-10-18). The last list is followed by other data. Under a 6-byte stride no list fits this layout (0 of 839, investigator A's scan).
- Monster record fields: species bits 0–8, level bits 9–15 (max 90), `s16` weights at `+2` and `+4`, `u16` at `+6` zero in every entry (investigator C's scan; the parser ignores it).
- `FloorProperties` count 1764 is not stored; it is `(first monster list − floor properties start) / 28` (`0x7380 + 1764 x 28 = 0x13470`) `[hypothesis: the count is the layout, not a field]`.
- Item lists: `u16` values; a value at or above 30000 is `value - 30000` zeros; 12 categories + 240 items = 252 values per list (loader constants `0x7530`, `0xfc`).
- Trap lists: 20 `u16` per list (loader reads `content[4]`).
- Blue caps each monster list at 32 entries in the loop; the largest list has 11 entries (investigator C's scan).

## Learnsets (ROM scan)

The table holds TM/HM streams for species 420 (2 move IDs) and 421 (59 move IDs), but Blue's readers return the fixed empty stream `0x02091C84` for species 0, 420 and 421 (`FUN_0200b448`, `FUN_0200b48c`, `FUN_02001844`). The parser now returns empty learnsets for those species (`blue-verified`). Species 420 and 421 are also excluded from the learnable-move checks in `FUN_02001a34` and `FUN_02001844`.

Level gate: `FUN_02001844` skips move IDs 0xEE (238), 0xEF (239), 0x110 (272) and 0x162 (354) unless a monster field at `+0xA` (`u16`) is at least 333. The threshold is read from `02091E18`, `02091E34`, `02091E28` and `02091E2C` (`0x014D`). The field is not a level (levels run 1 to 100), so its meaning is [unverified]. The learn cap is 413 (`DAT_02001a2c`).

## Species fields

- Regen byte `+0x1B`: speed `(b & 0x7F) << 1` (`FUN_02002368`); bit 7 is a flag [hypothesis: meaning unknown].
- Byte `+0x12`, read by `FUN_02002288` from `ov_0001::0217F7EC`; byte `+0x33`, read by `FUN_020022a4` from `ov_0001::021803B8`. Both are flags [hypothesis: meaning unknown].
- Bytes `+0x0A`–`+0x0B`, `+0x30`–`+0x32`, `+0x46`–`+0x47` are not read by any of the 29 getters checked [unverified].
- `recruit_rate` (`i16` at `+0x40`) is per mille (`FUN_ov_0001__0217f9b8`).

## Recruit (overlay 1)

- `gRecruitRateByLevel`: 101 `s16` at `0x7210C` (RAM `0x021aa96c`). Values match Red's table: 240 at levels 90 to 100.
- Check `FUN_ov_0001__0217f9b8`: roll `DungeonRandInt(1000)`, read the species rate, fail on `-999`, add 100 for Friend Bow, add the level rate, fail when the roll is at or above the rate. The same as Red's `DungeonMonRecruit` logic.
- The level index is the recruiter's info `+0x09`; the argument role is inferred from the call order [hypothesis].

## Items

- Price fields `+4` and `+8`: which is the buy price and which the sell price is not confirmed [unverified].
- Byte `+0x1F`: no reader found [unverified]. Icon `+0xD` is read by overlay 1 (`FUN_ov_0001__02143f7c`).
- Move ID `+0x18`: read as `u16` (`FUN_02007670`). The parser was `i16` before #37.

## Red comparison

Red's `str_spawn_pokemon_data.h` comment "R=0x8 | B=0x6" is wrong for Blue: Blue's monster records are 8 bytes. Red's recruit table, formula and constants match Blue (see [`blue-vs-red.md`](../blue-vs-red.md#data-tables-formatsdatamd-37)). Special item and move IDs, and the level gate, are not checked against Red. `pmd-red` is only read, not copied ([`references.md`](../references.md)).

## Open questions

- The gate field at `+0xA` of the monster struct.
- Meanings of species `+0x12` and `+0x33`, and of regen bit 7.
- Buy and sell direction of the item prices.
- Move bytes `+0x07` and `+0x1A`–`+0x1B`; item byte `+0x1F`.
- The ground-side `mapparam` reader in overlay 0 (`FUN_ov_0000__0215869c`).
- The `lvmpNNN` loader (`FUN_02001c48`).

## Links
- Spec: [formats/data.md](../formats/data.md)
- Comparison: [blue-vs-red.md](../blue-vs-red.md#data-tables-formatsdatamd-37)
- Verification: [verification.md](../verification.md)
