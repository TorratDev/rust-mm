# Random number generators

Blue has four random number generators: the global LCG, the dungeon LCG (with its preseed LCG), the Mersenne Twister and a 16-bit generator. All four match generators in the `pmd-red` decomp ([`references.md`](../references.md)) by constant and by behaviour, so they carry the `pmd-red` names. Issue: [#47](https://github.com/TorratDev/rust-mm/issues/47).

Dump: `BlueRescueTeamEur` ([`supported-dumps.md`](../supported-dumps.md)). Addresses are RAM addresses. `ov_0001::` marks code in overlay 1, which is loaded while a dungeon runs; plain addresses are in the ARM9 binary. The names are applied to a Ghidra project by [`tools/ghidra/RescueRngLabels.java`](../../tools/ghidra/RescueRngLabels.java), which checks each constant below before it changes anything.

Tags: **[blue-verified]** means read in Blue's code; **[red-hypothesis]** means taken from `pmd-red` and not yet confirmed in Blue (see [#43](https://github.com/TorratDev/rust-mm/issues/43)). The LCG step of both LCGs and the dungeon seed chain are also confirmed on the retail ROM by emulator (V7 in [`verification.md`](../verification.md#v7--scriptable-ds-emulator-for-ram-and-rng-dumps)). The 16-bit generator has no emulator check yet.

## Summary

| Generator | State | Step | Output | Seeded by | `pmd-red` file |
|-----------|-------|------|--------|-----------|----------------|
| Global LCG | `sPRNGState` `020DF704` (s32) | `s = s * 0x5D588B65 + 1` | `s >> 16` (16 bits); two steps make 32 bits | `SeedRng` at boot (console MAC address), `SetRNGSeed` from the save file | `src/random.c` |
| Dungeon LCG | `gDungeonRngState` `ov_0001::021B3F58` (u32) | `s = s * 0x5D588B65 + 1` | `s >> 16` (16 bits) | `InitDungeonRNG` once per floor | `src/dungeon_random.c` |
| Dungeon preseed LCG | `gDungeonRngPreseedState` `ov_0001::021B3F5C` (u32) | same step, two per call | odd 24-bit floor seed | `SetDungeonRNGPreseed23Bit` once per dungeon run | `src/dungeon_random.c` |
| Mersenne Twister | `sStateArray` `020DED24` (624 × u32), `sStateIndex` `020DED18`, `sTwistMatrix` `020DED1C` | MT19937 (`0x6C078965`, `0x9908B0DF`) | 32 bits | `MersenneTwister_InitializeState` | `src/random_mersenne_twister.c` |
| 16-bit generator | `gOtherRngState` `020E03F4` (u16 in use) | `s = s * 109 + 1021` (16 bits) | `s` zero-extended (u16) | static initial value `0x348C` | `src/other_random.c` |

## Global LCG [blue-verified]

| Address | Name | Behaviour |
|---------|------|-----------|
| `020119D8` | `SeedRng(u8 seed[6])` | `s = 0xD305 + seed[0]*seed[1] + seed[2]*seed[3] + seed[4]*seed[5]` (`0xD305` = 54021, same constant as Red) |
| `020119A4` | `Rand16Bit` | one LCG step, returns `s >> 16` as `s16` |
| `0201198C` | `Rand32Bit` | `(Rand16Bit() << 16) \| Rand16Bit()` |
| `02011968` | `RandInt(max)` | `((Rand32Bit() & 0xFFFF) * max) >> 16` |
| `02011924` | `RandRange(a, b)` | `a` if equal, else the lower bound plus `RandInt(|b - a|)` |
| `02011914` | `GetRNGState` | returns `sPRNGState` |
| `020118EC` | `SetRNGSeed(state)` | `s = state`, then `s = Rand32Bit()` |

Seeding:
- **At boot**, code at `02000CC4` (in the start-up path, not yet inside a named function) copies 6 bytes from `027FFCF4` and passes them to `SeedRng`. `027FFCF4` is in the DS system area where the console's wireless MAC address is kept. So the boot seed is fixed per console. In an emulator it comes from the firmware's MAC address. **Red differs:** it seeds from 6 constant bytes in `main.c`.
- **From the save file:** see [Save round trip](#save-round-trip) below.

Use: 25 call sites of `Rand32Bit` (16 functions), 44 of `RandInt` (29 functions), 7 of `RandRange` (5 functions). Callers are in the ARM9 and in overlays 0, 1 and 7, so **the global LCG also advances during dungeons**. A golden trace must capture both `sPRNGState` and `gDungeonRngState`. The callers, by subsystem, are in [Call sites by subsystem](#call-sites-by-subsystem).

## Dungeon LCG [blue-verified]

| Address | Name | Behaviour |
|---------|------|-----------|
| `ov_0001::0219D07C` | `DungeonRand16Bit` | one LCG step, returns `s >> 16` |
| `ov_0001::0219D058` | `DungeonRandInt(cap)` | `((DungeonRand16Bit() & 0xFFFF) * cap) >> 16` |
| `ov_0001::0219D014` | `DungeonRandRange(low, high)` | `low` if equal, else the lower bound plus `DungeonRandInt(|high - low|)` |
| `ov_0001::0219CFCC` | `DungeonRandOutcome(pct)` | `DungeonRandInt(100) < pct` |
| `ov_0001::0219CFF0` | `DungeonRandOutcome_2(pct)` | same body as `DungeonRandOutcome` (also two copies in Red) |
| `ov_0001::0219D0B0` | `InitDungeonRNG(value)` | `gDungeonRngState = value \| 1` |
| `ov_0001::0219D0C4` | `GenerateDungeonRNGSeed` | two steps of the preseed LCG; returns `((hi16 of step 2) << 16 \| (hi16 of step 1)) & 0xFFFFFF \| 1` |
| `ov_0001::0219D124` | `SetDungeonRNGPreseed23Bit(p)` | `gDungeonRngPreseedState = (p \| 1) & 0xFFFFFF` |

Use: `DungeonRandInt` has 175 call sites in 85 functions, almost all in overlay 1. (An earlier version of this page said 176 and 86; the x-ref count is 175 and 85.) `DungeonRandRange` has 62 call sites (19 functions), and the two `DungeonRandOutcome` functions have 9. The subsystem labels are in [Call sites by subsystem](#call-sites-by-subsystem). They feed #48, #50 and #52.

## Seeding a dungeon run [blue-verified]

`RunDungeon_Async` (`0206DC20`, ARM9) matches Red's `RunDungeon_Async` (`src/run_dungeon.c`) by its seeding code. `LoadAndRunDungeon_Async` (`0201DB70`) loads the dungeon overlays, calls it, then unloads them (see [`execution-model.md`](execution-model.md)). `RunDungeon_Async` reaches the dungeon struct through the pointer at `DAT_0206EBF0` (`gDungeon` in Red).

1. **Once per run** (not when resuming): if `dungeon+0x668` (Red: `unk644.missionKind`) is `DUNGEON_MISSION_OUTONRESCUE`, the preseed comes from the dungeon setup (`setup+0x14`, Red: `info.dungeonSeed.seed`). Otherwise it is `Rand32Bit() & 0xFFFFFF`. It is stored at `dungeon+0x66C` (Red: `unk644.prngPreseed23Bit`) and passed to `SetDungeonRNGPreseed23Bit`.
2. **Once per floor:** `GenerateDungeonRNGSeed()` is stored at `dungeon+0x670` (Red: `unk644.unk3C`) and passed to `InitDungeonRNG`.

In Red these are `unkDungeon644` fields `+0x34`, `+0x38` and `+0x3C`. In Blue that sub-struct starts at `0x638` (Red `0x644`), and its fields from `+0x14` on are 4 bytes earlier, so these three are at `+0x30`, `+0x34`, `+0x38` inside it. Layout details: [`structs.md`](structs.md).

**For golden traces (V7):** the floor seed follows from the run preseed, and the run preseed follows from the global LCG. So it is enough to capture `sPRNGState` at dungeon entry, or `dungeon+0x66C` and `dungeon+0x670`. The rescue path offers a way to set the preseed directly.

## Mersenne Twister

Tag: **[blue-verified]**. `MersenneTwister_InitializeState` (`0200CCB0`) fills `sStateArray` with the MT19937 recurrence (`0x6C078965`) and sets `sStateIndex` to 624. `Random32MersenneTwister` (`0200CB40`) is the generator; `sStateIndex` starts at 625 (`0x271`), which forces initialisation on first use.

Two functions use it. Both seed it from the global LCG, and neither feeds the Mersenne Twister state anywhere else.

| Address | Function | Use | Caller | Global LCG draws |
|---------|----------|-----|--------|------------------|
| `0200D4DC` | `sub_809542C` | Mail-record builder. One MT output is stored as the mail's `unk10`. | `LoadAndRunQuickSaveDungeon_Async` (`0201E304`) | `Rand32Bit` once before seeding |
| `0206C6E4` | `HandleTestTrackerState` | Personality test, state machine. Case 15 (`PERSONALITY_TEST_END`) uses MT outputs, then passes the result to `sub_8011C40`. | Reached through a dispatch table at `ov_0000::02159D00` | `Rand32Bit` for the product factor, then `Rand32Bit` for the seed |

Both match `pmd-red`: `sub_809542C` in `src/code_8094F88.c:270–283`, and case 15 in `src/personality_test1.c:96`. The constant 13 in the personality product loop is `NUM_PERSONALITIES` in Red; it has not been checked against a Red header. **For golden traces:** these calls draw from the global LCG, so a dump of `sPRNGState` must account for the extra draws when either routine runs.

## 16-bit generator

Tag: **[blue-verified]**. Blue's copy of `pmd-red`'s `OtherRandom16` (`src/other_random.c`). The NDS addresses that `pmd-red` gives for it (`arm9.bin::020127F4`) do not match this dump; the code at `020127F4` here is something else. The constants and the behaviour do match.

| Address | Name | Behaviour |
|---------|------|-----------|
| `02023B9C` | `OtherRandom16` | `state = state * 109 + 1021` in 16 bits (the store is `short`); returns the new state, zero-extended (`ldrh` at `02023BB4`) |
| `02023B84` | `OtherRandInt(n)` | `(n * OtherRandom16()) >> 16`: `mul` at `02023B90`, `asr #16` at `02023B94`; the state is unsigned, so for `0 < n < 0x8000` the result is in `[0, n)` |
| `02023B6C` | `OtherRandRange(low, high)` | `low + OtherRandInt(high - low)`; no equal-bounds case, so the generator still advances |

- **State:** `gOtherRngState` at `0x020E03F4` (`short`). The literal pool at `02023BBC` holds the address and at `02023BC0` the increment `1021`. The static initial value is `0x348C`, read from the image; `pmd-red`'s `gOtherRngState` is `0x348C` too.
- **Accesses:** x-ref to `020E03F4` lists three accesses, all in `OtherRandom16` (a read at `02023BA4`, a write at `02023BB0`, a read at `02023BB4`). Searching all literal pools for the address was not done, so aliasing through another pointer is not ruled out.
- **Seeding:** no runtime seed write was found. The state starts from the static value.
- **Callers:** x-ref to `OtherRandInt` (`02023B84`) lists 20 call sites: `OtherRandRange` (`02023B78`, 1), `FUN_02021d9c` (5, at `02021E88`, `02021EB0`, `02021ED8`, `02021EFC`, `02021F14`), `FUN_0201ff48` (1, at `0201FFFC`), and overlay 0 (13: `FUN_ov_0000__02140860` at `ov_0000::02141998`, `FUN_ov_0000__02154220` at `ov_0000::02154618`, and 11 call sites in `FUN_ov_0000__021421cc`). Investigator C's text says 18 direct sites; the list has 19 outside `OtherRandRange`. The list is taken as given. What these callers use the generator for (gameplay or presentation) is not checked.
- **Emulator check:** none yet. The unit tests in `crates/sim/src/rng.rs` check the formula only, from values computed separately.

## Save round trip

The save routines are `ReadSaveFromPak` (`0202A1AC`, callers `GameLoop_Async` at `0201F3A8`, `FUN_02065CFC` and `FUN_0202AA14`) and `WriteSavetoPak` (`0202A53C`, five callers: `FUN_0202AC00`, `FUN_020665F4`, `FUN_0202B140`, `FUN_02069848` and `FUN_0201ED4C`). See also [`symbols.md`](../symbols.md).

- **Write:** `WriteSavetoPak` stores `GetRNGState()` (the current `sPRNGState`) at `+0x420` of the save struct.
- **Read:** `ReadSaveFromPak` calls `SetRNGSeed(dest+0x420)`. `SetRNGSeed` stores the state, then replaces it with `Rand32Bit()`. So a save/load round trip is **not the identity** on `sPRNGState`. The game runs from the post-load value. `pmd-red` does the same (`src/save.c:198`, `:203`, `:323`, `:328`).
- **Quick save:** when the quick-save struct is in use (`DAT_0202A41C` on read, `DAT_0202A740` on write), neither routine calls `SetRNGSeed` or `GetRNGState`. The RNG state is not stored in that branch.

For golden traces, capture the state after the load (the state the game actually runs from), not the value stored in the save.

## Call sites by subsystem

The x-ref counts are from the Ghidra copy. The **subsystem labels are a reading of the names of the named callers, not of their bodies (medium confidence)**. Unnamed callers stay unclassified until their bodies are read.

**Dungeon LCG** (`DungeonRandInt` and `DungeonRandRange`, overlay 1):

| Routine | Sites | Functions | Named callers (sites) | Label (medium) |
|---------|-------|-----------|-----------------------|----------------|
| `DungeonRandInt` `ov_0001::0219D058` | 175 | 85 | `HandleDealingDamage_Async` (10), `CalcDamage` (3) | combat |
| | | | `GenerateFloor` (3), `GenerateSecondaryTerrainFormations` (9), `SpawnNonEnemies` (7), `SpawnEnemies` (2), `CreateRoomsAndAnchors` (4), `AssignRooms` (4), `GenerateOuterRingFloor` (2), `GenerateCrossroadsFloor` (2), `GenerateTwoRoomsWithMonsterHouseFloor` (2) | floor generation |
| | | | `DungeonRandRange` (2), `DungeonRandOutcome` (1), `DungeonRandOutcome_2` (1) | wrappers |
| | | | about 60 unnamed functions | unclassified |
| `DungeonRandRange` `ov_0001::0219D014` | 62 | 19 | `GenerateFloor` (4), `GenerateSecondaryTerrainFormations` (3), `SpawnNonEnemies` (4), `SpawnEnemies` (2), `CreateRoomsAndAnchors` (4), `GenerateOuterRingFloor` (4), `GenerateCrossroadsFloor` (4), `GenerateTwoRoomsWithMonsterHouseFloor` (2) | floor generation |
| | | | 8 unnamed functions | unclassified |
| `DungeonRandOutcome` `ov_0001::0219CFCC` | 4 | 2 | `FUN_ov_0001__02159f10` (2), `FUN_ov_0001__02159f6c` (2) | unclassified |
| `DungeonRandOutcome_2` `ov_0001::0219CFF0` | 5 | 3 | `FUN_ov_0001__0218579c` (3), `FUN_ov_0001__02185568` (1), `FUN_ov_0001__02192d24` (1) | unclassified |

`DungeonRand16Bit` has one caller, `DungeonRandInt`. "About 60 unnamed functions" is the investigator's count for the unnamed callers of `DungeonRandInt`; their site count is not given.

**Global LCG** (ARM9 and overlays 0, 1, 7):

| Routine | Sites | Functions | Named callers (sites) | Label (medium) |
|---------|-------|-----------|-----------------------|----------------|
| `Rand32Bit` `0201198C` | 25 | 16 | `RandInt`, `SetRNGSeed` (`020118EC`), `RunDungeon_Async` (`0206DC20`), `GeneratePelipperJobs` (3) | RNG wrappers, seeding, items |
| | | | `FUN_0200d4dc` (1), `FUN_0206c6e4` (2) | Mersenne Twister users (see above) |
| | | | `FUN_ov_0001__0217af58` (2), `FUN_ov_0001__0217b120` (2, `UpdateMonsterSprite`), `FUN_02012648` (1, sprite animation) | animation |
| | | | `FUN_0200eda4`, `FUN_0200f768` (4), `FUN_02018acc` (2), `FUN_ov_0001__02155760` (1, called from the per-turn update `FUN_02142504`), `FUN_0206a228`, `FUN_0206a170`, `FUN_ov_0000__02140860` | unclassified |
| `RandInt` `02011968` | 44 | 29 | `RandRange` (2) | wrappers |
| | | | 26 unnamed functions | unclassified |
| `RandRange` `02011924` | 7 | 5 | `InitBulkItem` (1), `GeneratePelipperJobs` (2) | items |
| | | | `FUN_02008458`, `FUN_0200eed4` (2), `FUN_0200f5c4` | unclassified |
| `Rand16Bit` `020119A4` | 2 | | both in `Rand32Bit` | wrappers |

The labels for the named callers are medium. `FUN_ov_0001__0217af58` as an animation routine comes from its neighbours' names, not its body.

**Dungeon message display reaches the global LCG** (static, low confidence; see [`ai.md`](ai.md), P6-f). The static walk from the AI code reaches `Rand32Bit` along `02154BF0` (message display) → `02154910` → `0213AFA8` → `0213AD54` → `02142504` → `0217B120`. So a dungeon message can advance `sPRNGState` as well as `gDungeonRngState`. The chain runs through `0213AFA8` and `0217B120`, which [`symbols.md`](../symbols.md) names `DungeonRunFrameActions` and `UpdateMonsterSprite`; those names were not checked against this path. A runtime check is open.

## Open questions

- Classify the unnamed callers by reading their bodies: the dungeon callers (about 60 functions) and the global callers (about 25). This feeds #48, #50 and #52.
- Correct the subsystem labels above once the bodies are read.
- Which of the 20 `OtherRandInt` callers use the 16-bit generator for gameplay, and which for presentation. Needs the bodies and an emulator check.
- Whether any other code writes `gOtherRngState` through a pointer alias (literal-pool search).
- An emulator vector for the 16-bit generator: a state dump before and after a call that uses it.
- Whether the quick-save branch in the save routines leaves the global RNG state from the dungeon (not checked).
