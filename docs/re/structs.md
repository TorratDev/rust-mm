# Dungeon state structures

Layout of the dungeon struct, the entity struct and the entity info struct in Blue, compared field by field with `pmd-red`'s `include/structs/str_dungeon.h` and `include/structs/dungeon_entity.h`. Issue: [#46](https://github.com/TorratDev/rust-mm/issues/46). Function names and addresses are in [`symbols.md`](../symbols.md).

**Method:** a field is listed only when a Blue function already matched to its `pmd-red` counterpart (same calls, constants and arguments) reads or writes it at that offset. The Evidence column names that function. Fields not listed are unknown in Blue: do not assume Red's offset, because Blue's structs drop bytes at several points. Not confirmed by emulator yet.

Tags: **[blue-verified]** unless marked *probable* (one access, consistent with its neighbours) or *red-hypothesis*.

## Entity

Same layout as Red at the two offsets checked, so Red's `Entity` (size 0x74) is the working hypothesis for the rest.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x00` | `type` (0 = no entity) | `0x00` | `EntityIsValid` |
| `+0x70` | `axObj.info` (pointer to the entity info) | `0x70` | `GetLeader`, `AbilityIsActive`, `CalcDamage`, ... |
| `+0x04` | `pos` | `0x04` | *red-hypothesis* |

## Entity info

Pointed to by `entity+0x70`. Blue's struct is shorter than Red's (0x208): bytes are missing before the types, in the status block, around the speed counters and before the belly. The **Shift** column is Blue offset minus Red offset.

| Blue | Red field | Red | Shift | Evidence |
|------|-----------|-----|-------|----------|
| `+0x02` | `id` (species) | `0x02` | 0 | `CalcSpeedStage`, `TickStatusAndHealthRegen` |
| `+0x04` | `apparentID` | `0x04` | 0 | `CalcSpeedStage`, `CalcDamage` |
| `+0x06` | `isNotTeamMember` | `0x06` | 0 | `CalcSpeedStage`, `CalcDamage` |
| `+0x07` | `isTeamLeader` | `0x07` | 0 | `GetLeader`, `CalcDamage` |
| `+0x09` | `level` | `0x09` | 0 | `CalcDamage` |
| `+0x0E` | `HP` | `0x0E` | 0 | `TickStatusAndHealthRegen` |
| `+0x10` | `maxHPStat` | `0x10` | 0 | `TickStatusAndHealthRegen` |
| `+0x16` | `def[2]` | `0x16` | 0 | `CalcDamage` |
| `+0x28` | `flashFireBoost` | `0x28` | 0 | `CalcDamage` (*probable*) |
| `+0x56` | `types[2]` | `0x5C` | −6 | `sub_806F370_Async`, `sub_806F4A4` |
| `+0x58` | `abilities[2]` | `0x5E` | −6 | `AbilityIsActive` |
| `+0x99`, `+0x9A` | `sleepClassStatus` status, turns | `0xA8` | −0xF | `TickStatusAndHealthRegen` |
| `+0x9B`, `+0x9C` | `burnClassStatus` status, turns | `0xAC` | −0x11 | `CalcSpeedStage` (paralysis), `TickStatusAndHealthRegen` |
| `+0xA0` | `frozenClassStatus.status` | `0xB0` | −0x10 | `TickStatusAndHealthRegen` |
| `+0xAE` | `bideClassStatus.status` | `0xC0` | −0x12 | `CalcDamage` |
| `+0xB1` | `reflectClassStatus.status` | `0xC4` | −0x13 | `CalcDamage`, `TickStatusAndHealthRegen` |
| `+0xC4` | `sureShotClassStatus.status` | `0xDC` | −0x18 | `CalcDamage` |
| `+0xD5` | `speedStageChanged` | `0xF8` | −0x23 | `RunLeaderTurn_Async` |
| `+0xE0` | `speedStage` | `0x104` | −0x24 | `CalcSpeedStage` |
| `+0xE4` | `speedUpCounters[5]` | `0x108` | −0x24 | `CalcSpeedStage` |
| `+0xE9` | `speedDownCounters[5]` | `0x10D` | −0x24 | `CalcSpeedStage` |
| `+0x10E` | `belly` | `0x13C` | −0x2E | `CalcDamage` (*probable*) |
| `+0x118` | `bellyEmpty` | `0x146` | −0x2E | `TickStatusAndHealthRegen` |
| `+0x11A` | `attacking` | `0x148` | −0x2E | `RunLeaderTurn_Async` |
| `+0x120` | `visualFlags` | `0x14E` | −0x2E | `CalcDamage` |
| `+0x122` | `previousVisualFlags` | `0x150` | −0x2E | `CalcDamage` |
| `+0x124` | `unk152` | `0x152` | −0x2E | `CalcDamage` |
| `+0x1C4` | `unk1F8` (HP regeneration counter) | `0x1F8` | −0x34 | `TickStatusAndHealthRegen` |

The status records are smaller in Blue: the distance between consecutive ones shrinks (for example, reflect to sure shot is 0x13 bytes in Blue and 0x18 in Red), so each status record needs its own check.

## Dungeon struct

Pointed to by `gDungeon` (`0x020E0788`). Identical to Red at least up to `0x17A`; after that, fields move earlier.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x002` | `unk2` (floor-end state) | `0x002` | `IsFloorOver` |
| `+0x004` | `unk4` | `0x004` | `IsFloorOver` |
| `+0x005` | `unk5` (skips the turn loop) | `0x005` | `RunDungeon_Async` |
| `+0x00F` | `noActionInProgress` | `0x00F` | `RunLeaderTurn_Async` |
| `+0x0B8`, `+0x0BC` | `unkB8`, `unkBC` (acting entity) | `0x0B8`, `0x0BC` | `RunLeaderTurn_Async` |
| `+0x134`–`+0x17A` | `damageCalcWorkingValues` | same | `CalcDamage` (16 fields at Red's offsets) |
| `+0x638` | start of `unk644` | `0x644` | see below |
| `+0x11314` | `teamPokemon[4]` (entity pointers) | `0x1357C` | `GetLeader` |
| `+0x15FB0`, `+0x15FB1` | `unk181e8.unk18218`, `unk18219` | `0x18218`, `0x18219` | `RunDungeon_Async` |

The shift is 0xC at `unk644` and 0x2268 at `teamPokemon` and `unk181e8`, so most of the bytes Blue drops lie between `0x680` and `0x11314`.

### `unk644` sub-struct

In Blue it starts at `0x638`. Its first field checked, `unk10`, is at Red's relative offset; every later field checked is 4 bytes earlier, so Blue drops 4 bytes between `+0x11` and `+0x1B` of the sub-struct.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x648` | `unk10` (floor-end reason) | `0x654` | `IsFloorOver` |
| `+0x650` | `fractionalTurn` | `0x660` | `RunFractionalTurn_Async` |
| `+0x65C` | `unk28` | `0x66C` | `RunLeaderTurn_Async` |
| `+0x65E` | `stoleFromKecleon` | `0x66E` | `CalcSpeedStage` |
| `+0x663` | `unk2F` | `0x673` | `RunLeaderTurn_Async` |
| `+0x668` | `missionKind` | `0x678` | `RunDungeon_Async` ([`rng.md`](rng.md)) |
| `+0x66C` | `prngPreseed23Bit` | `0x67C` | `RunDungeon_Async` |
| `+0x670` | `unk3C` (floor seed) | `0x680` | `RunDungeon_Async` |

## RAM for golden traces

What the V7 dumper ([#55](https://github.com/TorratDev/rust-mm/issues/55)) should read on each turn trigger ([`turns.md`](turns.md)), all through the pointer in `gDungeon`:

| What | Where |
|------|-------|
| Dungeon struct | `*(u32 *)0x020E0788` |
| Turn counter | dungeon `+0x650` |
| Run and floor seeds | dungeon `+0x66C`, `+0x670` |
| Team | 4 entity pointers at dungeon `+0x11314`; for each: `+0x70` → info: `id` `+0x02`, `level` `+0x09`, `HP` `+0x0E`, `maxHPStat` `+0x10`, `types` `+0x56`, statuses `+0x99`–`+0xC4`, `speedStage` `+0xE0`, `belly` `+0x10E` |
| Wild Pokémon | 16 entity pointers at dungeon `+0x11324` (*red-hypothesis*: right after `teamPokemon`, as in Red) |
| Positions | entity `+0x04` (*red-hypothesis*) |
| RNG | `sPRNGState` `0x020DF704`, `gDungeonRngState` `0x021B3F58` ([`rng.md`](rng.md)) |

## Open questions

- Positions, moves (`moves` at Red `0x118`), held item, and the floor tile grid: find matched functions that use them.
- The size of Blue's entity info struct (Red 0x208): look for the array or allocation that holds the infos.
- Item and inventory structs.
- Check the trace RAM list against an emulator dump on the retail ROM.
