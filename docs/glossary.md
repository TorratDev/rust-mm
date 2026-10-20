# Glossary

Names, addresses and layouts for Blue's structs, fields, functions, globals and enums. Every Phase 3 RE issue uses these names, so a name means the same thing everywhere. Full evidence for each area stays in its topic page; this page holds the summary and the tag.

Dump: `BlueRescueTeamEur` (EUR, NDS), SHA-1 in [`verification.md`](verification.md). Machine-readable rows for the Ghidra import are in [`symbols.md`](symbols.md) (functions and globals only).

## Tagging rule

Every entry carries exactly one tag:

- **`[blue-verified]`**: the name, role or layout was read in Blue. The entry has an ARM9 address or an overlay address, or it was confirmed by an emulator trace (V7, [`verification/v7-trace/`](../verification/v7-trace/), see [`verification.md`](verification.md#v7--scriptable-ds-emulator-for-ram-and-rng-dumps)). Record the command or file used.
- **`[red-hypothesis]`**: the name, value or layout comes from `pmd-red` (the decomp of Red, GBA), and Blue has not confirmed it. Red is a map for hypotheses only. Read it and cite it; never copy its code or data.

Rules for promoting an entry from `[red-hypothesis]` to `[blue-verified]`:

1. The entry needs an ARM9 address (no overlay prefix) or an overlay address (`ov_NNNN::RAM`), taken from Ghidra or from the decompiled code.
2. Or it needs an emulator trace that shows the value at that address.
3. A matching name or a matching value in Red alone is not enough. Changing the tag without one of the two items above is wrong.

Overlays share RAM ranges, so every address names its overlay: `ov_0001::021AAEE8` is overlay 1, and an address with no prefix is the ARM9 binary. Entries for data inside the heap show `heap` in the Overlay column.

Tags used in the topic pages and in [`symbols.md`](symbols.md) also exist: `tentative` (our own name, role partly known) and `sdk-hypothesis` (NitroSDK function recognised by its behaviour). They are not glossary tags. Promote them into this page only after the rule above is met.

## Columns

| Column | Meaning |
|--------|---------|
| Name | The name used in the repo. Names from Red keep Red's spelling. |
| Address | RAM address in hex, or a file offset when noted. |
| Overlay | `ARM9`, `ov_NNNN` (overlay number), or `heap`. |
| Offset | Fields only: byte offset inside the parent struct. A field has no RAM address of its own; its overlay is the parent struct's (`heap`). |
| Tag | One of the two tags above. |
| Evidence | The topic page, symbol row or trace that supports the entry. |

## Structs

| Name | Size | Overlay | Tag | Description | Evidence |
|------|------|---------|-----|-------------|----------|
| Entity | 0x74 | heap | `[blue-verified]` for size and allocation; the field offsets below are checked one at a time | A monster, item or trap in a dungeon. Team entities are allocated 0x74 apart. | [`re/structs.md`](re/structs.md#entity) |
| Entity info | Blue: shorter than Red's 0x208 | heap | `[blue-verified]` for the fields listed; `[red-hypothesis]` for the rest | Pointed to by `entity+0x70`. | [`re/structs.md`](re/structs.md#entity-info) |
| Dungeon | see topic page | heap | `[blue-verified]` up to `0x17A`, then `[red-hypothesis]` where fields move | Floor and run state. Reached through the global `gDungeon`. | [`re/structs.md`](re/structs.md#dungeon-struct) |
| Tile | see topic page | heap | `[blue-verified]` (`ResetTile`, `GetTileMut`, stride `0x14`) | One cell of the dungeon grid. | [`re/structs.md`](re/structs.md#tile) |
| Key state | 0x10 each (current at `0x020F2790`, previous at `0x020F2780`) | ARM9 | `[blue-verified]` | Per-frame button state: held keys, pressed edge, D-pad repeat output, B and R tap flags, D-pad group and hold counter. | [`re/io.md`](re/io.md#key-state) |
| Touch state | see topic page | ARM9 | `[blue-verified]` (layout read in `UpdateInputSnapshot`) | Touch position, tap, drag and hold output, and the gesture state at `+0x20`. At `0x020F27B0`. | [`re/io.md`](re/io.md#touch-state) |
| Status class | — | heap | `[blue-verified]` for the byte offsets; the class names are Red's and only partly confirmed | A group of status bytes in the entity info: sleep `+0x99`, burn `+0x9B`, frozen `+0xA0`, cringe `+0xAC`, reflect `+0xB1`. Each class has a status byte and a turns or counter byte. Partial catalogue. | [`re/status.md`](re/status.md#status-classes) |

### Fields

| Struct | Field | Offset | Tag | Description | Evidence |
|--------|-------|--------|-----|-------------|----------|
| Entity | type | `+0x00` | `[blue-verified]` | 0 means no entity. | `EntityIsValid` ([`re/structs.md`](re/structs.md#entity)) |
| Entity | pos.x, pos.y | `+0x04`, `+0x06` | `[blue-verified]` | s16 position. | `021563CC` passes `entity+4` as a position ([`re/structs.md`](re/structs.md#entity)) |
| Entity | isVisible | `+0x20` | `[blue-verified]` | Set by `SpawnItem`. | [`re/structs.md`](re/structs.md#entity) |
| Entity | info | `+0x70` | `[blue-verified]` | Pointer to the entity info. For an item entity, its `Item`. | `GetLeader`, `AbilityIsActive` ([`re/structs.md`](re/structs.md#entity)) |
| Entity info | id (species) | `+0x02` | `[blue-verified]` | Species ID. | `CalcSpeedStage`, `TickStatusAndHealthRegen` ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | level | `+0x09` | `[blue-verified]` | Level. | `CalcDamage` ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | HP | `+0x0E` | `[blue-verified]` | HP. | `TickStatusAndHealthRegen` ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | maxHPStat | `+0x10` | `[blue-verified]` | Max HP stat. | `TickStatusAndHealthRegen` ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | types[2] | `+0x56` | `[blue-verified]` | Defender types, read by the type chart lookup. Red's offset is `0x5C`. | `sub_806F370_Async`, `sub_806F4A4` ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | flashFireBoost | `+0x28` | `[red-hypothesis]` | *probable*: one access, consistent with its neighbours. | `CalcDamage` ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | direction | `+0x44` | `[blue-verified]` | Direction, compared with the direction toward the target. | `DisplayActions_Async` (`ov_0001::0218766C`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | waiting | `+0x119` | `[blue-verified]` | Tested and cleared by `DisplayActions_Async` when set. | `DisplayActions_Async` (`ov_0001::0218766C`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | targetPos | `+0x138` | `[blue-verified]` | Target position: x at `+0x138`, y at `+0x13A`. | `DisplayActions_Async` (`ov_0001::0218766C`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | numMoveTiles | `+0x1C0` | `[blue-verified]` | Number of move tiles; tested for 0 in `DisplayActions_Async`. | `DisplayActions_Async` (`ov_0001::0218766C`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | aiAllySkip | `+0x116` | `[blue-verified]` | Set to `FALSE` or `TRUE` by the monster-turn loop. | `sub_8044574_Async` (`ov_0001::02140AC0`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | recalculateFollow | `+0x117` | `[blue-verified]` | Set to `FALSE` or `TRUE` by the monster-turn loop. | `sub_8044574_Async` (`ov_0001::02140AC0`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | aiNotNextToTarget | `+0x71` | `[red-hypothesis]` | *probable*: the write is in Blue, the field identity rests on the candidate `RunMonsterAI` (`ov_0001::02188030`, name *red-hypothesis*). Cleared there. | [`re/structs.md`](re/structs.md#entity-info) |
| Entity info | decoyAITracker | `+0xDD` | `[red-hypothesis]` | *probable*: the write is in Blue, the field identity rests on the candidate `RunMonsterAI` (`ov_0001::02188030`, name *red-hypothesis*). Set to `DECOY_AI_NONE` there. | [`re/structs.md`](re/structs.md#entity-info) |
| Entity info | frozen counter | `+0xA8` | `[blue-verified]` | Decremented by `TickStatusAndHealthRegen`. | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | reflect counter | `+0xB2` | `[blue-verified]` | Decremented by `TickStatusAndHealthRegen`. | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Entity info | sure shot counter | `+0xC5` | `[blue-verified]` | Decremented by `TickStatusAndHealthRegen`. | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) ([`re/structs.md`](re/structs.md#entity-info)) |
| Dungeon | stairs room index | `+0x34AC` | `[blue-verified]` | Stairs room index, written on floor type 2. | `SpawnNonEnemies` (`ov_0001::0214B650`) ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | fractionalTurn | `+0x650` | `[blue-verified]` | Turn fraction, the name from `re/structs.md`. | `RunFractionalTurn_Async` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | prngPreseed23Bit | `+0x66C` | `[blue-verified]` | Run preseed, `Rand32Bit() & 0xFFFFFF` (24 bits); the name is Red's. | `RunDungeon_Async` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | floor seed (`unk3C`) | `+0x670` | `[blue-verified]` | Floor seed. The name `unk3C` is Red's. | `RunDungeon_Async` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | tiles[32][56] | `+0x34B8` | `[blue-verified]` | Tile grid, 0x14 bytes per tile. | `sub_80495E4` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | stairsSpawn | `+0xC0BC`, `+0xC0BE` | `[blue-verified]` | Stairs position, x and y. | `ResetFloor`, `SpawnNonEnemies` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | teamPokemon[4] | `+0x11314` | `[blue-verified]` | Team slots: four entity pointers. | `GetLeader` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | wildPokemon[16] | `+0x11324` | `[blue-verified]` | Wild slots: 16 entity pointers. | `EntityIsValid` loops, from `0219F09C` ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | items[64] | `+0x113B4` | `[blue-verified]` | Floor item slots: 64 entity pointers. | `SpawnItemEntity` (`ov_0001::02141F58`) ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Dungeon | traps[64] | `+0x114B4` | `[blue-verified]` | Trap slots: 64 entity pointers, cleared by `ResetFloor`. No writer found. | `ResetFloor` (`ov_0001::0214BDBC`) ([`re/structs.md`](re/structs.md#dungeon-struct)) |
| Tile | terrainFlags | `+0x00` | `[blue-verified]` | Terrain flags; `0x10` = impassable wall. | `ResetTile` (`ov_0001::0214C0A8`) ([`re/structs.md`](re/structs.md#tile)) |
| Tile | room | `+0x05` | `[blue-verified]` | Room index: `0xFF` = none, `0xFE` = anchor. | `CreateRoomsAndAnchors` (`ov_0001::0214F7A8`) ([`re/structs.md`](re/structs.md#tile)) |
| Tile | monster, object | `+0x0C`, `+0x10` | `[blue-verified]` | Entity pointers on the tile: the monster, and the item or trap. | `ResetTile` (`ov_0001::0214C0A8`) ([`re/structs.md`](re/structs.md#tile)) |

## Globals

| Name | Address | Overlay | Type | Tag | Description | Evidence |
|------|---------|---------|------|-----|-------------|----------|
| gDungeon | `020E0788` | ARM9 | `void *` | `[blue-verified]` | Pointer to the dungeon struct. | [`symbols.md`](symbols.md) |
| sPRNGState | `020DF704` | ARM9 | `s32` | `[blue-verified]` | State of the global LCG. | [`re/rng.md`](re/rng.md) |
| gDungeonRngState | `021B3F58` | `ov_0001` | `u32` | `[blue-verified]` | State of the dungeon LCG. | [`re/rng.md`](re/rng.md) |
| gDungeonRngPreseedState | `021B3F5C` | `ov_0001` | `u32` | `[blue-verified]` | State of the preseed LCG, seeded once per dungeon run. | [`re/rng.md`](re/rng.md) |
| gTypeEffectivenessChart | `021AAEE8` | `ov_0001` | `short[18][18]` | `[blue-verified]` | Attack type (row) by defender type (column). Byte-identical to Red's table, 648 bytes. In the BLZ-decoded overlay 1 it sits at file offset `0x72688`. | [`symbols.md`](symbols.md), [`verification.md`](verification.md) (V6) |
| gAtkStatStageMultipliers | `021AA660` | `ov_0001` | `int[21]` | `[blue-verified]` | Attack stat stage multipliers (s24.8). | [`symbols.md`](symbols.md) |
| gDefStatStageMultipliers | `021AA6B4` | `ov_0001` | `int[21]` | `[blue-verified]` | Defense stat stage multipliers (s24.8). | [`symbols.md`](symbols.md) |
| gOtherRngState | `020E03F4` | ARM9 | `short` | `[blue-verified]` | State of the 16-bit generator `OtherRandom16`. Static initial value `0x348C`. | [`re/rng.md`](re/rng.md#16-bit-generator), [`symbols.md`](symbols.md) |

## Functions

| Name | Address | Overlay | Tag | Description | Evidence |
|------|---------|---------|-----|-------------|----------|
| GameLoop_Async | `0201F2E4` | ARM9 | `[blue-verified]` | Main loop after boot: init calls, then the title menu. | [`symbols.md`](symbols.md) |
| LoadAndRunDungeon_Async | `0201DB70` | ARM9 | `[blue-verified]` | Loads the dungeon overlays, runs `RunDungeon_Async`, then unloads them. | [`symbols.md`](symbols.md) |
| RunDungeon_Async | `0206DC20` | ARM9 | `[blue-verified]` | Seeds the dungeon RNG (matches Red; see RNG notes). | [`re/rng.md`](re/rng.md) |
| MemoryAlloc | `02022188` | ARM9 | `[blue-verified]` | `(size, group)`; freed with `MemoryFree`. | [`symbols.md`](symbols.md) |
| MemoryFree | `02022170` | ARM9 | `[blue-verified]` | Frees a `MemoryAlloc` result. | [`symbols.md`](symbols.md) |
| SeedRng | `020119D8` | ARM9 | `[blue-verified]` | Seeds the global LCG from six bytes. | [`re/rng.md`](re/rng.md) |
| RandInt | `02011968` | ARM9 | `[blue-verified]` | Global LCG: a value in `[0, max)`. | [`re/rng.md`](re/rng.md) |
| InitDungeonRNG | `ov_0001::0219D0B0` | `ov_0001` | `[blue-verified]` | `gDungeonRngState = value \| 1`. | [`re/rng.md`](re/rng.md) |
| DungeonRandInt | `ov_0001::0219D058` | `ov_0001` | `[blue-verified]` | Dungeon LCG: a value in `[0, cap)`. | [`re/rng.md`](re/rng.md) |
| GetDefaultMonsterAnimId | `ov_0001::0217AD64` | `ov_0001` | `[blue-verified]` | Animation a monster shows when idle. | [`symbols.md`](symbols.md) |
| sub_806F4A4 | `ov_0001::0217BA94` | `ov_0001` | `[blue-verified]` | Checks the target's two types for immunity in `gTypeEffectivenessChart`. | [`symbols.md`](symbols.md) |
| WeightWeakTypePicker | `ov_0001::0217CB3C` | `ov_0001` | `[blue-verified]` | Reads `gTypeEffectivenessChart`; 3 arguments. | [`symbols.md`](symbols.md) |
| sub_806E100 | `ov_0001::0217CE88` | `ov_0001` | `[blue-verified]` | Reads `gTypeEffectivenessChart`; the type is its 4th argument. | [`symbols.md`](symbols.md) |
| CalcDamage | `ov_0001::0217BD94` | `ov_0001` | `[blue-verified]` | Damage formula. Same constants as Red (`999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`). | [`symbols.md`](symbols.md) |
| OtherRandom16 | `02023B9C` | ARM9 | `[blue-verified]` | 16-bit generator: `state * 109 + 1021` in 16 bits, returns the new state, zero-extended. | [`re/rng.md`](re/rng.md#16-bit-generator), [`symbols.md`](symbols.md) |
| OtherRandInt | `02023B84` | ARM9 | `[blue-verified]` | `(n * OtherRandom16()) >> 16`. | [`re/rng.md`](re/rng.md#16-bit-generator) |
| OtherRandRange | `02023B6C` | ARM9 | `[blue-verified]` | `low + OtherRandInt(high - low)`; equal bounds still advance the generator. | [`re/rng.md`](re/rng.md#16-bit-generator) |
| sub_809542C | `0200D4DC` | ARM9 | `[blue-verified]` | Mail-record builder: seeds the Mersenne Twister from the global LCG, stores one output as the mail's `unk10`. Called from `LoadAndRunQuickSaveDungeon_Async`. | [`re/rng.md`](re/rng.md#mersenne-twister), [`symbols.md`](symbols.md) |
| HandleTestTrackerState | `0206C6E4` | ARM9 | `[blue-verified]` | Personality-test state machine. Its case 15 uses the Mersenne Twister, seeded from the global LCG. | [`re/rng.md`](re/rng.md#mersenne-twister), [`symbols.md`](symbols.md) |
| RunGameMode_Async | `0201E3C8` | ARM9 | `[blue-verified]` | Runs a game mode; the title menu calls it with modes 0–3. Only `GameLoop_Async` calls it. | [`re/execution-model.md`](re/execution-model.md#scene-graph) |
| RunOverlay7Mode | `0201DAC4` | ARM9 | `[blue-verified]` | Title-menu choice 5: loads overlay 7, calls its entry, then reloads overlay 0. | [`re/execution-model.md`](re/execution-model.md#overlays), [`symbols.md`](symbols.md) |
| GroundStation_SetBank | `ov_0000::0214FC50` | `ov_0000` | `[blue-verified]` | Loads or unloads the ground bank `gGroundBankTable[bank]`; its sole caller is `FUN_ov_0000__0214D510`. | [`symbols.md`](symbols.md) |
| GroundStation_LoadBank | `ov_0000::0214FD00` | `ov_0000` | `[blue-verified]` | Loads the overlay of `gGroundBankTable[bank]` with `FS_LoadOverlayInfo`. | [`symbols.md`](symbols.md) |
| PxDecompress | `02013388` | ARM9 | `[blue-verified]` | PX decoder for `AT4P` and `AT3P` headers. Reached through the dispatcher slot `0x02013864`. | [`re/io.md`](re/io.md#px-decoder), [`formats/compression.md`](formats/compression.md) |

## Enums and constants

| Name | Value | Overlay | Tag | Description | Evidence |
|------|-------|---------|-----|-------------|----------|
| EFFECTIVENESS_IMMUNE | `0` | `ov_0001` | `[blue-verified]` | Result value for an immune matchup. `sub_806F4A4` (`ov_0001::0217BA94`) compares the chart entries to it. The name is from `pmd-red`'s `include/dungeon_config.h`. The raw chart bytes were not read; the value rests on the decompiled code. | the zero comparison and Ghost branch in `sub_806F4A4` (`ov_0001::0217BA94`) and `sub_806E100` (`ov_0001::0217CE88`), [`symbols.md`](symbols.md) |
| Archive index | 1–9 (0 unused) | ARM9 | `[blue-verified]` | Pack archive a TOC record belongs to: 1 `sample`, 2 `system`, 3 `titlemenu`, 4 `dungeon`, 5 `monster`, 6 `effect`, 7 `ground`, 8 `ornament`, 9 `sound`. Name table at `0x02096EE4`. | [`re/io.md`](re/io.md#archive-index-and-toc), [`formats/archive.md`](formats/archive.md#archive-index) |

## Open questions

- Fill the Entity info, Dungeon and Tile rows from [`re/structs.md`](re/structs.md) as their fields are checked; Red's offsets are not a substitute.
- The Ghidra import (#42) should read this page's addresses, not invent its own.
- Enums other than `EFFECTIVENESS_IMMUNE` are not listed yet.

## Links

- Symbols (machine-readable): [symbols.md](symbols.md)
- Structs: [re/structs.md](re/structs.md)
- RNG: [re/rng.md](re/rng.md)
- Blue vs Red differences: [blue-vs-red.md](blue-vs-red.md)
- Verification: [verification.md](verification.md)
- Legal and dump rules: [legal.md](legal.md)
