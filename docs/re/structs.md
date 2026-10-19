# Dungeon state structures

Layout of the dungeon struct, the entity struct and the entity info struct in Blue, compared field by field with `pmd-red`'s `include/structs/str_dungeon.h` and `include/structs/dungeon_entity.h`. Issue: [#46](https://github.com/TorratDev/rust-mm/issues/46). Function names and addresses are in [`symbols.md`](../symbols.md).

**Method:** a field is listed only when a Blue function already matched to its `pmd-red` counterpart (same calls, constants and arguments) reads or writes it at that offset. The Evidence column names that function. Fields not listed are unknown in Blue: do not assume Red's offset, because Blue's structs drop bytes at several points. The fields read by the V7 trace (`gDungeon`, `fractionalTurn`, the seeds, `teamPokemon`/`wildPokemon`, entity `type` and position, info `id`, `level`, `HP`, `maxHPStat`, `speedStage`) decode to consistent values on the retail ROM by emulator (V7 in [`verification.md`](../verification.md#v7--scriptable-ds-emulator-for-ram-and-rng-dumps)); the rest is checked in code only.

Tags: **[blue-verified]** unless marked *probable* (one access, consistent with its neighbours) or *red-hypothesis*. A Blue function name taken from `pmd-red` is *red-hypothesis* on its own; the offsets the function reads or writes can still be blue-verified.

## Entity

Same size as Red, 0x74 bytes, and the same offsets wherever checked; Red's `Entity` is the working hypothesis for the rest. The routine `021428F8` sets the team, wild, item and trap entity pointers 0x74 apart. It does not allocate the infos.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x00` | `type` (0 = no entity) | `0x00` | `EntityIsValid` |
| `+0x20` | `isVisible` | `0x20` | `SpawnItem` |
| `+0x70` | `axObj.info` (pointer to the entity info; for an item entity, its `Item`) | `0x70` | `GetLeader`, `AbilityIsActive`, `GetItemInfo`, ... |
| `+0x04`, `+0x06` | `pos.x`, `pos.y` (s16) | `0x04` | `0219082C` copies them into the info's `aiTarget.aiTargetPos`; `021563CC` passes `entity+4` as a position |

## Entity info

Pointed to by `entity+0x70`. Blue's struct is shorter than Red's (0x208): bytes are missing before the types, in the status block, around the speed counters and before the belly. The **Shift** column is Blue offset minus Red offset.

**Size: not found.** The array or allocation that holds the infos is not located. The shift table bounds Blue's size to 0x1C8 ≤ size ≤ 0x208 (blue-verified). A value of 0x1D4 is a *red-hypothesis* candidate and is not recorded here. Measure the infos' pointer stride in the emulator ([#17](https://github.com/TorratDev/rust-mm/issues/17)).

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
| `+0x3C`, `+0x3E` | `hiddenPower` base power, type | `0x3C` | 0 | `GetMovePower`, `GetMoveTypeForMonster` |
| `+0x42` | `action` block (*probable*: its `direction` is at `+0x44`) | `0x44` | −2 | `ov_0001::02188030` (candidate `RunMonsterAI`): byte compared with 0, then written |
| `+0x44` | `direction` (u8) | `0x46` | −2 | `DisplayActions_Async` (`ov_0001::0218766C`) compares it with the direction from `FUN_0201187C` (the helper's name `GetDirectionTowardsPosition` is *red-hypothesis*) |
| `+0x56` | `types[2]` | `0x5C` | −6 | `sub_806F370_Async`, `sub_806F4A4` |
| `+0x58` | `abilities[2]` | `0x5E` | −6 | `AbilityIsActive` |
| `+0x5A` | `heldItem` (3-byte `Item`: `flags` `+0`, `id` `+2`) | `0x60` | −6 | `HasHeldItem` (flag `1` exists, `8` sticky) |
| `+0x70` | `aiTarget` (`aiTargetPos` `+0x10` = info `+0x80`, written by `0219082C`; `aiObjective` `+0`, `aiTargetSpawnGenID` `+4`, `aiTarget` `+8` and the other inner offsets follow the −8 shift and are not checked) | `0x78` | −8 | `0219082C` |
| `+0x71` | `aiNotNextToTarget` (inside `aiTarget`; *probable*: the writer is only a candidate) | `0x79` | −8 | `ov_0001::02188030` (candidate `RunMonsterAI`) writes it |
| `+0x8A` | `tactic` | `0x94` | −0xA | `IsTacticSet` |
| `+0x99`, `+0x9A` | `sleepClassStatus` status, turns | `0xA8` | −0xF | `TickStatusAndHealthRegen` |
| `+0x9B`, `+0x9C` | `burnClassStatus` status, turns | `0xAC` | −0x11 | `CalcSpeedStage` (paralysis), `TickStatusAndHealthRegen` |
| `+0xA0` | `frozenClassStatus.status` | `0xB0` | −0x10 | `TickStatusAndHealthRegen` |
| `+0xA8` | `frozenClassStatus` counter | `0xB8` | −0x10 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`; decremented by `FUN_ov_0001__0219bf1c`) |
| `+0xAC`, `+0xAD` | cringe status, counter (*red-hypothesis*: class guessed from the shift; read only by Tick) | `0xBC`, `0xBD` | −0x10 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) only |
| `+0xAE` | `bideClassStatus.status` | `0xC0` | −0x12 | `CalcDamage` |
| `+0xB1` | `reflectClassStatus.status` | `0xC4` | −0x13 | `CalcDamage`, `TickStatusAndHealthRegen` |
| `+0xB2` | `reflectClassStatus` counter | `0xC5` | −0x13 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) |
| `+0xB3`, `+0xB6` | curse status, counter (*red-hypothesis*: class guessed from the shift; read only by Tick) | `0xC8`, `0xCB` | −0x15 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) only |
| `+0xB8`, `+0xC1` | leech seed status, counter (*red-hypothesis*: class guessed from the shift; read only by Tick) | `0xD0`, `0xD9` | −0x18 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) only |
| `+0xC4` | `sureShotClassStatus.status` | `0xDC` | −0x18 | `CalcDamage` |
| `+0xC5` | `sureShotClassStatus` counter | `0xDD` | −0x18 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) |
| `+0xD5` | `speedStageChanged` | `0xF8` | −0x23 | `RunLeaderTurn_Async` |
| `+0xDD` | `decoyAITracker` (*probable*: the writer is only a candidate) | `0x100` | −0x23 | `ov_0001::02188030` (candidate `RunMonsterAI`) writes it |
| `+0xE0` | `speedStage` | `0x104` | −0x24 | `CalcSpeedStage` |
| `+0xE4` | `speedUpCounters[5]` | `0x108` | −0x24 | `CalcSpeedStage` |
| `+0xE9` | `speedDownCounters[5]` | `0x10D` | −0x24 | `CalcSpeedStage` |
| `+0xF4` | `moves` (4 × 6-byte `Move`, see below) | `0x118` | −0x24 | `0216E56C` (passes `info+0xF4` as the move list, sets `moveFlags2` bit 2 of entry *i* at `+0xF5 + 6i`) |
| `+0x10E` | `belly` | `0x13C` | −0x2E | `CalcDamage` (*probable*) |
| `+0x116` | `aiAllySkip` | `0x144` | −0x2E | `sub_8044574_Async` (`ov_0001::02140AC0`) writes it |
| `+0x117` | `recalculateFollow` | `0x145` | −0x2E | `sub_8044574_Async` (`ov_0001::02140AC0`) writes it |
| `+0x118` | `bellyEmpty` | `0x146` | −0x2E | `TickStatusAndHealthRegen` |
| `+0x119` | `waiting` | `0x147` | −0x2E | `DisplayActions_Async` (`ov_0001::0218766C`) |
| `+0x11A` | `attacking` | `0x148` | −0x2E | `RunLeaderTurn_Async` |
| `+0x120` | `visualFlags` | `0x14E` | −0x2E | `CalcDamage` |
| `+0x122` | `previousVisualFlags` | `0x150` | −0x2E | `CalcDamage` |
| `+0x124` | `unk152` | `0x152` | −0x2E | `CalcDamage` |
| `+0x138`, `+0x13A` | `targetPos.x`, `.y` (u16; Red's block starts at `0x16C`) | `0x16C` | −0x34 | `DisplayActions_Async` (`ov_0001::0218766C`) |
| `+0x1C0` | `numMoveTiles` | `0x1F4` | −0x34 | `DisplayActions_Async` (`ov_0001::0218766C`) |
| `+0x1C4` | `unk1F8` (HP regeneration counter) | `0x1F8` | −0x34 | `TickStatusAndHealthRegen` |

Not listed: `IQ` (Red `0x0C`). Its Red readers are known, but no Blue function reading `+0x0C` is matched yet. See the open questions.

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
| `+0x5B8` | `unk5C0` | `0x5C0` | `RunDungeon_Async` |
| `+0x638` | start of `unk644` | `0x644` | see below |
| `+0x3360` | `deoxysForm` | `0x3800` | `RunDungeon_Async` |
| `+0x3424` | `numItems` | `0x3904` | `ResetFloor` |
| `+0x34AC` | stairs room index (floor type 2 only) | not matched | `SpawnNonEnemies` (`ov_0001::0214B650`) writes it |
| `+0x34AE`, `+0x34B0`, `+0x34B4` | `tileset`, `unk3A10`, `fixedRoomNumber` | `0x3A0E`, `0x3A10`, `0x3A14` | `RunDungeon_Async` |
| `+0x34B8` | `tiles[32][56]` (0x14 bytes each, see below) | `0x3A18` (0x18 bytes each) | `sub_80495E4` |
| `+0xC0BC`, `+0xC0BE` | `stairsSpawn.x`, `.y` | `0xE21C` | `ResetFloor` (pmd-red's comment says `0xEA1C`; the layout gives `0xE21C`); written by `SpawnNonEnemies` |
| `+0xC11C` | `unkE27C[8][8]` (fixed-room tiles) | `0xE27C` | `ResetFloor` |
| `+0xC660` | `tilePointers[32][56]` | `0xE8C0` | `GetTile`, `GetTileMut`, `sub_80495E4` |
| `+0x113B4` | `items[64]` (entity pointers) | `0x1361C` | `SpawnItemEntity` (searches the 64 entries for a free slot) and `0219B700` |
| `+0x114B4` | `traps[64]` (entity pointers) | `0x1371C` | `ResetFloor` clears all 64. No function that writes trap entities is found |
| `+0x11314` | `teamPokemon[4]` (entity pointers) | `0x1357C` | `GetLeader` |
| `+0x11324` | `wildPokemon[16]` (entity pointers) | `0x1358C` | five loops over 16 entries with `EntityIsValid` (`0219F09C`, ...) |
| `+0x15FB0`, `+0x15FB1` | `unk181e8.unk18218`, `unk18219` | `0x18218`, `0x18219` | `RunDungeon_Async` |
| `+0x19DF6` | `unk1BDD4.unk1C05E` | `0x1C05E` | `RunDungeon_Async` |
| `+0x1A2F8`–`+0x1A308` | `floorProperties` fields | `0x1C576`–`0x1C588` | `RunDungeon_Async` ([`dungeon-generation.md`](dungeon-generation.md)) |

The shift grows: −8 at `0x5C0`, −0xC at `unk644`, −0x4A0 at `0x3800`, −0x560 at `0x3A0E`, −0x2160 at `stairsSpawn`, −0x2268 from `traps` on. From the tiles to the tile pointers, the extra shift is exactly 4 bytes per tile (56 × 32 map tiles plus 64 fixed-room tiles).

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

## Tile

[blue-verified] by `ResetTile` (`ov_0001::0214C0A8`), which writes every field in Red's order, and by `sub_80495E4`/`GetTileMut` (stride 0x14). `pmd-red`'s `include/structs/map.h` already notes the DS size: the spawn/visibility flags take 2 bytes instead of 6, so every later field is 4 bytes earlier.

| Blue | Red field | Red | Notes |
|------|-----------|-----|-------|
| `+0x00` | `terrainFlags` (u16) | `0x00` | `0x10` = impassable wall (set on the map border by `ResetFloor`); `0x200` is set by `ResolveInvalidSpawns` (`ov_0001::0214C1A8`) |
| `+0x02` | `spawnOrVisibilityFlags` (u16) | `0x04` | bits used by `SpawnNonEnemies` (`0x2` item candidate, `0x4` trap or other candidate), `SpawnEnemies` and `SpawnNonEnemies` (gate on `0x8`, `0x20`, `0x40`, `0x100`), and `ResolveInvalidSpawns` (clears `0x4` when `0x1` or `0x2` is set). The bit names are guessed; the uses are blue-verified |
| `+0x04` | `unk8`: graphics variant, picked with the global RNG; cosmetic ([`dungeon-generation.md`](dungeon-generation.md#emulator-check)) | `0x08` | |
| `+0x05` | `room` (`0xFF` = none) | `0x09` | written by `CreateRoomsAndAnchors` (`ov_0001::0214F7A8`) as a counter from 0; anchor tiles get `0xFE`; `ResetTile` sets `0xFF` |
| `+0x06` | `walkableNeighborFlags[4]` | `0x0A` | one byte per crossable-terrain kind |
| `+0x0A` | `unkE` | `0x0E` | |
| `+0x0C` | `monster` (entity pointer) | `0x10` | |
| `+0x10` | `object` (item or trap entity pointer) | `0x14` | |

**Rooms.** The room data is per tile: the room index is in `+0x05`. A persistent room array in the dungeon struct was not found (medium confidence). The 30-byte cell grid (`x0`/`y0`/`x1`/`y1` at `+0`/`+2`/`+4`/`+6`, room flag at `+0x1C`) is generation scratch passed in by the caller, `GenerateStandardFloor` and its siblings. The meaning of `gNumRoomsAssigned` (`ov_0001::021B45BC`) is taken from older notes and was not rechecked.

Size `0x14` (Red `0x18`). The grid is `tiles[y][x]` at dungeon `+0x34B8`, row stride `0x460`; `GetTile(x, y)` returns `tilePointers[y][x]` (dungeon `+0xC660`, row stride `0xE0`) or, outside 56 × 32, the out-of-bounds tile `gCurTilesetOobTile`.

## Moves, items and inventory

**Move** [blue-verified]: `moveFlags` `+0` (`5` = exists and enabled for the AI), `moveFlags2` `+1`, `id` (u16) `+2`, `PP` `+4`, `ginseng` `+5`; 6 bytes (Red 8). Every field is written by `InitPokemonMove` (`0200B568`), and `GetMovePower` reads `id` and `ginseng`. The info struct holds 4 of them at `+0xF4`; `pmd-red` notes `struggleMoveFlags` at `+0x18` after them.

**Item** [blue-verified]: `flags` `+0` (bit `1` = exists, `8` = sticky, `0x10` = set for L+R throwing), `quantity` `+1`, `id` `+2`; 3 bytes, the same as Red's fields. Read by `HasHeldItem` and the inventory functions; `SpawnItem` clears bit `0x10`.

**Team inventory** [blue-verified]: `gTeamInventoryRef` (`0x020DECF8`) points to it. Every field below is written by `InitializeMoneyItems` (`02008588`) or read by a function matched to `pmd-red`, and the offsets agree with `pmd-red`'s Blue notes in `include/structs/str_items.h`.

| Blue | Field | Red | Evidence |
|------|-------|-----|----------|
| `+0x000` | `teamItems[20]` (3-byte `Item`) | `0x000` | `GetItemCountInInventory`, `InitializeMoneyItems` |
| `+0x03C` | `teamStorage[240]` (u16 counts per item id) | `0x050` | `InitializeMoneyItems` (240 entries; ends exactly at `+0x21C`) |
| `+0x21C` | `kecleonShopItems[8]` (2-byte `BulkItem`: `id`, `quantity`) | `0x230` | `CountKecleonShopItems`, `AddKecleonShopItem`, `InitKecleonShopItem` |
| `+0x22C` | `kecleonWareItems[4]` | `0x250` | `CountKecleonWareItems`, `AddKecleonWareItem` |
| `+0x234` | `teamMoney` (s32, clamped to 99,999) | `0x260` | `AddToTeamMoney` |
| `+0x238` | `teamSavings` (s32) | `0x264` | `InitializeMoneyItems` |

Blue's struct is 0x23C bytes (Red 0x268). The difference is packing: Blue stores `Item` in 3 bytes and `BulkItem` in 2, where Red's arrays use a 4-byte stride for both.

**Floor items** [blue-verified]: 64 entity pointers at dungeon `+0x113B4` (Red `0x1361C`). `SpawnItemEntity` (`ov_0001::02141F58`) takes a free slot. It sets the entity type to 3 (`ENTITY_ITEM`), copies the position to `+0x4`/`+0x6`, and sets `entity+0x70` to the 3-byte `Item` slot `gDungeon + 3·i + DAT_02142064`. The base `DAT_02142064` is not read. `SpawnItem` then copies the `Item` into the entity's info, sets `isVisible` and puts the entity in the tile's `object` field.

## Recruited Pokémon

The team roster outside dungeons (Red `Pokemon`). Offsets read from `WritePoke1Bits` (`02003E1C`), which writes each field in turn; they match `pmd-red`'s Blue notes in `include/structs/str_pokemon.h` field for field. Size `0x40` (Red `0x58`).

| Blue | Field | Red |
|------|-------|-----|
| `+0x03` | `level` | `0x03` |
| `+0x04` | `dungeonLocation` | `0x04` |
| `+0x06` | `speciesNum` | `0x08` |
| `+0x08`, `+0x09` | `unkC[2]` (levels) | `0x0C` |
| `+0x0A` | `IQ` | `0x14` |
| `+0x0C` | `pokeHP` | `0x16` |
| `+0x0E`–`+0x11` | `offense` (attack, sp. attack, defense, sp. defense) | `0x18` |
| `+0x14` | `currExp` | `0x1C` |
| `+0x18` | `IQSkills` (24 bits) | `0x20` |
| `+0x1B` | `tacticIndex` | `0x24` |
| `+0x1C` | `heldItem` (2-byte `BulkItem`) | `0x28` |
| `+0x1E` | `moves[4]` (6 bytes each) | `0x2C` |
| `+0x36` | `name[10]` | `0x4C` |

## RAM for golden traces

What the V7 dumper ([#55](https://github.com/TorratDev/rust-mm/issues/55)) should read on each turn trigger ([`turns.md`](turns.md)), all through the pointer in `gDungeon`:

| What | Where |
|------|-------|
| Dungeon struct | `*(u32 *)0x020E0788` |
| Turn counter | dungeon `+0x650` |
| Run and floor seeds | dungeon `+0x66C`, `+0x670` |
| Team | 4 entity pointers at dungeon `+0x11314`; for each: `+0x70` → info: `id` `+0x02`, `level` `+0x09`, `HP` `+0x0E`, `maxHPStat` `+0x10`, `types` `+0x56`, statuses `+0x99`–`+0xC4`, `speedStage` `+0xE0`, `belly` `+0x10E` |
| Wild Pokémon | 16 entity pointers at dungeon `+0x11324` |
| Positions | entity `+0x04` (x), `+0x06` (y) |
| Moves, held item | info `+0xF4` (4 × 6 bytes: flags, id `+2`, PP `+4`), info `+0x5A` (3 bytes) |
| Floor items | 64 entity pointers at dungeon `+0x113B4`; each entity's `+0x04` position and `+0x70` → 3-byte `Item` |
| Team inventory | `*(u32 *)0x020DECF8`: 20 × 3-byte items at `+0`, money `+0x234`, savings `+0x238` |
| Floor map | 56 × 32 tiles of 0x14 bytes at dungeon `+0x34B8` (`terrainFlags` `+0`, `room` `+5`, `monster` `+0xC`, `object` `+0x10`); stairs at dungeon `+0xC0BC` |
| RNG | `sPRNGState` `0x020DF704`, `gDungeonRngState` `0x021B3F58` ([`rng.md`](rng.md)) |

## Open questions

- **Entity info size.** Not found. The bound is 0x1C8 ≤ size ≤ 0x208 (blue-verified). Measure the infos' pointer stride in the emulator ([#17](https://github.com/TorratDev/rust-mm/issues/17)). Do not record 0x1D4 until then.
- **`IQ` at info `+0x0C`** (Red `0x0C`). Red's readers are known (`HandleGummiItemAction`, `sub_8072778`, `WriteMonster`), but no Blue function that reads `+0x0C` is matched. Find Blue's `GetMovesLearnedAtLevel` caller and check the offset of its IQ argument.
- **`aiTarget` inner layout** beyond `aiTargetPos`: `aiObjective` `+0x70`, `aiTargetingEnemy` `+0x72`, `aiTurningAround` `+0x73`, `aiTargetSpawnGenID` `+0x74`, `aiTarget` `+0x78` follow the −8 shift but are unchecked.
- **`RunMonsterAI` candidate** `ov_0001::02188030`: name it and check its remaining branches against Red's `dungeon_ai.c`.
- **Status bytes not assigned:** `+0xC9`/`+0xCA` and `+0xCB`/`+0xCC` are either invisible (`0xE4`/`0xE5`) or blinker (`0xE8`/`0xE9`) in Red's layout; both are read in `TickStatusAndHealthRegen`. Cringe (`+0xAC`), curse (`+0xB3`) and leech seed (`+0xB8`) are *red-hypothesis* and have no write sites yet. Status write sites were not examined.
- **Trap entities.** The writer of `traps[64]` (`+0x114B4`) is not found. Search the overlay 1 functions called from the floor-generation chain (`GenerateFloor`, `ov_0001::02151E7C`), or dump RAM after a floor load (#17).
- **`DAT_02142064`**, the base of the floor-item slots (dungeon `+0x3·i`). Read its value to name the floor-item storage.
- **Dungeon `+0xC0B8`/`+0xC0BA`** (checked for `-1` next to the stairs; meaning unknown), and the later use of `+0x34AC` (stairs room, floor type 2).
- **Tile spawn bits.** Names for `0x2`, `0x4`, `0x8`, `0x20`, `0x40`, `0x100` and `0x200`. Name them after the trap writer is found.
- **Rooms.** Confirm that no persistent room array exists (medium confidence now), and recheck `gNumRoomsAssigned`.
- Add the remaining trace fields (statuses, moves, held items, floor items, inventory) to `verification/v7-trace/retail_trace.py` and check them by emulator.
