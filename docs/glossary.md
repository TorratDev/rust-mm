# Glossary

Names, addresses and layouts for Blue's structs, fields, functions, globals and enums. Every Phase 3 RE issue uses these names, so a name means the same thing everywhere. Full evidence for each area stays in its topic page; this page holds the summary and the tag.

Dump: `BlueRescueTeamEur` (EUR, NDS), SHA-1 in [`verification.md`](verification.md). Machine-readable rows for the Ghidra import are in [`symbols.md`](symbols.md) (functions and globals only). `symbols.md` is the source: the Globals and Functions tables below repeat its rows, and a change to one goes into both. The four RNG functions (`SeedRng`, `RandInt`, `InitDungeonRNG`, `DungeonRandInt`) have no `symbols.md` row yet; their evidence is in [`re/rng.md`](re/rng.md).

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
| Tag | One of the two tags above. |
| Evidence | The topic page, symbol row or trace that supports the entry. |

## Structs

| Name | Size | Overlay | Tag | Description | Evidence |
|------|------|---------|-----|-------------|----------|
| Entity | 0x74 | heap | `[blue-verified]` for size and allocation; the field offsets below are checked one at a time | A monster, item or trap in a dungeon. Team entities are allocated 0x74 apart. | [`re/structs.md`](re/structs.md#entity) |
| Entity info | Blue: shorter than Red's 0x208 | heap | `[blue-verified]` for the fields listed; `[red-hypothesis]` for the rest | Pointed to by `entity+0x70`. | [`re/structs.md`](re/structs.md#entity-info) |
| Dungeon | see topic page | heap | `[blue-verified]` up to `0x17A`, then `[red-hypothesis]` where fields move | Floor and run state. Reached through the global `gDungeon`. | [`re/structs.md`](re/structs.md#dungeon-struct) |
| Tile | see topic page | heap | `[blue-verified]` (`ResetTile`, `GetTileMut`, stride `0x14`) | One cell of the dungeon grid. | [`re/structs.md`](re/structs.md#tile) |

### Fields

| Struct | Field | Offset | Tag | Description | Evidence |
|--------|-------|--------|-----|-------------|----------|
| Entity | type | `+0x00` | `[blue-verified]` | 0 means no entity. | `EntityIsValid` |
| Entity | pos.x, pos.y | `+0x04`, `+0x06` | `[blue-verified]` | s16 position. | `021563CC` passes `entity+4` as a position |
| Entity | isVisible | `+0x20` | `[blue-verified]` | Set by `SpawnItem`. | [`re/structs.md`](re/structs.md#entity) |
| Entity | info | `+0x70` | `[blue-verified]` | Pointer to the entity info. For an item entity, its `Item`. | `GetLeader`, `AbilityIsActive` |
| Entity info | id (species) | `+0x02` | `[blue-verified]` | | `CalcSpeedStage`, `TickStatusAndHealthRegen` |
| Entity info | level | `+0x09` | `[blue-verified]` | | `CalcDamage` |
| Entity info | HP | `+0x0E` | `[blue-verified]` | | `TickStatusAndHealthRegen` |
| Entity info | maxHPStat | `+0x10` | `[blue-verified]` | | `TickStatusAndHealthRegen` |
| Entity info | types[2] | `+0x56` | `[blue-verified]` | Defender types, read by the type chart lookup. Red's offset is `0x5C`. | `sub_806F370_Async`, `sub_806F4A4` |
| Entity info | flashFireBoost | `+0x28` | `[red-hypothesis]` | *probable*: one access, consistent with its neighbours. | `CalcDamage` |

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

## Enums and constants

| Name | Value | Overlay | Tag | Description | Evidence |
|------|-------|---------|-----|-------------|----------|
| EFFECTIVENESS_IMMUNE | not yet recorded in this glossary | `ov_0001` | `[red-hypothesis]` | Result value for an immune matchup. Its use in `sub_806F4A4` is in Blue, but the value is not checked. Promote it once the value is read from `sub_806F4A4` or traced. | [`symbols.md`](symbols.md) |

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
