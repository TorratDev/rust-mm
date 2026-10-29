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
- **At boot** (#158 P2, blue-verified): the seed is copied at `0x02000CB8`, which calls `0x02077F20`, a wrapper for a block copy (`0x0207853C`, source `027FFCF4`, 6 bytes). `SeedRng` (`0x020119D8`) is called at `0x02000CC4`. `SeedRng` computes `s = seed[4]*seed[5] + seed[2]*seed[3] + seed[0]*seed[1] + 0xD305` (`0xD305` at `0x02011A18`, the store to `sPRNGState` at `0x020DF704`). `027FFCF4` is in the DS system area, where the console's wireless MAC address is kept. In the emulator it comes from the firmware's MAC address. **Runtime:** after `SeedRng` the six bytes are `00 09 BF 12 34 56`, the MAC the emulator reports (`00:09:BF:12:34:56`), and `sPRNGState` = `0x0000F1EB`, which equals the formula on those bytes (match). **Not verified:** that `027FFCF4` holds the wireless MAC on a real DS (only the emulator's firmware was run); whether `0x0207853C` is a NitroSDK function by that name (its shape is `copy(src, dst, len)`); a second boot with another firmware MAC (not run). **Red differs:** it seeds from 6 constant bytes in `main.c`.
- **From the save file:** see [Save round trip](#save-round-trip) below.

Use: 25 call sites of `Rand32Bit` (16 functions), 44 of `RandInt` (29 functions), 7 of `RandRange` (5 functions). Callers are in the ARM9 and in overlays 0, 1 and 7, so **the global LCG also advances during dungeons**. A golden trace must capture both `sPRNGState` and `gDungeonRngState`. The callers, by subsystem, are in [Call sites by subsystem](#call-sites-by-subsystem).

## Per-frame steps (#159)

The global LCG also advances once or twice per emulated frame, from the VBlank handler `FUN_02018ACC` (`0x02018ACC`). Tags: **blue-verified** (decompile and runtime); the caller of the handler is not named.

- **Calls in `FUN_02018ACC`.** `Rand32Bit` (`0201198C`) is two LCG steps. The handler calls `Rand32Bit` at `02018B00` (return address `0x02018B04`) only when `*puVar1 & 0xC2` is nonzero, where `puVar1 = DAT_02018B24` (a literal holding the address of the flag word, `0x020DF89C`; see Flag word below). It then calls `Rand32Bit` at `02018B08` (return address `0x02018B0C`) unconditionally. So a frame advances the state by 4 steps when the flag test passes and by 2 when it fails (#159 P2, blue-verified).
- **Runtime, plain run (boot only, 3000 frames).** Return address `0x02018B0C` occurs in every frame from the first logged frame (frame 20), so the handler runs once per emulated frame. In 1737 consecutive frame pairs the state advanced 4 steps, and in 1174 it advanced 2. Every 4-step frame with the flag set had both calls (`0x02018B04` and `0x02018B0C`); the 2-step frames had only `0x02018B0C`. With the flag test passing, the frame advanced 4 steps in 1695 cases and 2 in 11. With the flag clear it advanced 2 steps in 1163 cases and 4 in 42. Those 42 have other callers: `0x020126F8` (sprite routine `FUN_02012648`) or the overlay-1 callers below.
- **The two-step intervals (#159 P3).** The double frame-routine hit at frame 2419 is not the cause: the state advanced 4 steps across that frame, like its neighbours, and a hook double hit does not change the state. The 2-step intervals are real frames where the flag test fails: frames 2532 and 2533 advanced 2 steps, and their `Rand32Bit` calls were only at `0x02018B0C`. Later, 63 frames in the window advanced 2 steps with the frame routine not running. The earlier note's gaps are multi-frame intervals between frame-routine hits, not a different per-frame rate. In both `plain_a.jsonl` and `rng_run1.jsonl` the gaps are 2448 to 2458 (10 frames, 40 steps), 2459 to 2472 (13 frames, 52 steps) and 2472 to 2474 (2 frames, 8 steps). The earlier 36 and 56 values are not reproduced in either file.
- **Flag word (#159 P2).** The literal at `0x02018B24` holds the pointer `0x020DF89C`, and the handler tests the `u16` at `0x020DF89C`. It is not `0x027E3FF8`: the handler sets that word with `|= 1` (bit 0, VBlank; see [`execution-model.md`](execution-model.md)). The handler adds 4 steps when `(flag & 0xC2) != 0` and 2 steps otherwise. `FUN_02019130` (`ReadKeyInput`, see [`io.md`](io.md#frame-path)) adds the pressed-key mask into the word at `0x020DF89C`: A adds 1 and Select adds 4. The mask comes from `REG_KEYINPUT` (`0x04000130`) and the shadow word `0x027FFFA8`, masked with `0x2FFF`. So the value is constant while idle, counts up by one per frame while A is held, and by about four per frame while Select is held (see the next bullet). **Bit 15 of the flag word: not established.** Tag: static (Ghidra addresses from the investigator findings; not re-run here), consistent with the runtime counts in the next bullet.
- **A and Select windows (#159 P4, partial).** The frame routine `MainLoops_RunFrameActions` (`0201F268`) has no hits in the dungeon (1500 frames from frame 24960). In the boot phase it ran from frame 2418 to 2561 only. So the A and Select test could not be run on the frame routine itself. Measured instead, in the dungeon (`win_a` run, repeated identically): idle, 4 steps in about 260 of 300 frames and 6 steps in about 30 (from overlay-1 callers `0x0217B2C4`, `0x02155820`, `0x0217AFC0`); A held, 4 steps in 240, 6 in 27, 2 in 27, 8 in 5, 14 in 1, and the flag word counts up from 11683 to 11982; Select held, 4 steps in 263, 6 in 34, 8 in 1, 10 in 2, no 2-step frames, and the flag word counts up from 11986 to 13182. Holding A changes the per-frame step count through the flag test. The 6-step frames also occur in idle, so they are not an A or Select effect. Tag: blue-verified (runtime); confidence high for the flag behaviour and for "no 2-step frames in Select", low for the Select effect on steps (one run, one window, frame routine not running).
- **Repeatability (#159 P1, partial).** The probe `rng_dungeon.py` (scratch, not committed) was run twice for each case with identical output. Plain run, 3000 frames from power-on: sha256 `4758e33e74ab1c5e423a3b786291d2ba253a336b9f8dd0999332f7ea82720c65`. Windows run, 27000 frames (boot to the dungeon, then 1500 idle frames with A held at +300 to +600 and Select held at +900 to +1200, with idle frames between): sha256 `5893f821c3da9d439c09dc6dbcdfa37120b6956dd8cab999a7ad69412886a3ff`. The script is not in the repository yet: committing it as `verification/v7-trace/rng_per_frame.py` is a separate change. Tag: blue-verified (runtime). Confidence: high for repeatability of this one ROM on this machine. **Not verified:** repeats on a second OS, a second ROM, or in CI.

**Not verified (#159)**

- The name of the flag word at `0x020DF89C` (the target of `DAT_02018B24`).
- The callback slot that calls `FUN_02018ACC` (the PARAM reference at `0x02000C2C`, a data word that points to the function, probably a callback slot). The run shows it is called every emulated frame, but not which slot.
- The other callers in the 42 four-step frames with the flag clear (`0x020126F8`, `0x02011974`, `0x0217AFC0` and others).
- Whether the A-window 6-step and 8-step frames come from the player's action or from the turn logic.
- Whether `FUN_02019130` is the only writer of the flag word at `0x020DF89C`, and what bit 15 of that word does.
- The A and Select check on the frame routine: the frame routine runs only in boot frames 2418 to 2561, and the windows were run in the dungeon. A new run needs inputs before frame 2561.

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

## Unnamed global-LCG callers (#168 P3)

Investigator C decompiled every unnamed caller of `Rand32Bit` (`0201198C`), `RandInt` (`02011968`) and `RandRange` (`02011924`). The x-ref lists give 76 call sites in 41 unnamed functions; named callers are excluded. Tags: **[blue-verified]** for the call relations and constants from the decompile; **[hypothesis]** for the subsystem label, which is inferred from the callees and not from a string, a message or a runtime check. Confidence is per row. Source: the #168 comment. Nothing was run in the emulator.

| Function (address) | RNG calls | Key callees | Label | Tag | Confidence |
|--------------------|-----------|-------------|-------|-----|------------|
| `FUN_02006B4C` (`02006B4C`) | `RandInt` ×2 | `AddKecleonWareItem`, `FUN_02006D4C` | Kecleon shop: ware items | [hypothesis] | high |
| `FUN_02006E0C` (`02006E0C`) | `RandInt` ×2 | `AddKecleonShopItem`, `InitKecleonShopItem`, `FUN_02006EA0` | Kecleon shop: shop items | [hypothesis] | high |
| `FUN_02008458` (`02008458`) | `RandRange` | `FUN_020080D8`, `FUN_020082FC`, `FUN_02008504` | item or bulk generation, same family as `InitBulkItem` | [hypothesis] | medium |
| `FUN_0200D4DC` (`0200D4DC`) | `Rand32Bit` | `MersenneTwister_InitializeState`, `Random32MersenneTwister` | the mail-record builder (see [Mersenne Twister](#mersenne-twister)); the comment's label was a Mersenne Twister seed | [hypothesis] | medium |
| `FUN_0200EDA4` (`0200EDA4`) | `Rand32Bit` | `FUN_0200F184`, `FUN_0200F250`, `FUN_0200F2F0`, `FUN_0200F394`, `FUN_0200F3AC`, `FUN_0200F3E4` | cluster with `FUN_0200F768` (team or floor generation) | [hypothesis] | low |
| `FUN_0200EED4` (`0200EED4`) | `RandRange` ×2 | `FUN_0200F768`, `FUN_0201F994`, `FUN_0206A66C` | cluster with `FUN_0200F768` | [hypothesis] | low |
| `FUN_0200F3E4` (`0200F3E4`) | `RandInt` | `FUN_02006794`, `FUN_02007450` | unknown; called from `FUN_0200F768` and `FUN_0200EDA4` | [hypothesis] | low |
| `FUN_0200F46C` (`0200F46C`) | `RandInt` | `FUN_02009120` | unknown | [hypothesis] | low |
| `FUN_0200F5C4` (`0200F5C4`) | `RandInt`, `RandRange` | `FUN_020061D8`, `FUN_02006364`, `FUN_020703B8`, `FUN_0208625C` | unknown; called from `FUN_0200F768` | [hypothesis] | low |
| `FUN_0200F768` (`0200F768`) | `Rand32Bit` ×5, `RandInt` ×3 | `FUN_0206A0B0`, `FUN_0206A170`, `FUN_0206A228`, `FUN_0206A2E0`, `FUN_0206A384`, `FUN_0206A6DC`, `02069FF0` | the central cluster: picks a value and fills by helper | [hypothesis] | low |
| `FUN_02069FF0`, `FUN_0206A0B0`, `FUN_0206A2E0` (`02069FF0`, `0206A0B0`, `0206A2E0`) | `RandInt` | `FUN_0206A6DC` (common helper) | cluster with `FUN_0200F768` | [hypothesis] | low |
| `FUN_0206A170`, `FUN_0206A228` (`0206A170`, `0206A228`) | `Rand32Bit` + `RandInt` | `FUN_0206A6DC` | cluster with `FUN_0200F768` | [hypothesis] | low |
| `FUN_02012648` (`02012648`) | `Rand32Bit` | none in the RNG family | sprite animation; already noted above | [blue-verified] for the call; [hypothesis] for the label | medium |
| `FUN_02018ACC` (`02018ACC`) | `Rand32Bit` ×2 | `FUN_02017B20` (input poll), `FUN_0201D150`, `FUN_0201966C` | frame callback: per-frame RNG (see [Per-frame steps](#per-frame-steps-159)) | [blue-verified] | high |
| `FUN_0201DE24` (`0201DE24`) | `RandInt(100)` | `FUN_02007B68` | item and money loss, not a transition (see [`execution-model.md`](execution-model.md)); one draw of 0 to 99 | [hypothesis] | medium |
| `FUN_0201F008` (`0201F008`) | `RandInt(3)` | `OpenFileAndGetFileDataPtr`, `MemoryAlloc`, `MemoryFree`, `GetLanguage` | file-loading routine that picks one of three variants | [hypothesis] | low |
| `FUN_0204730C` (`0204730C`) | `RandInt(0xFF)` | `FUN_0200475C` | unknown; one byte drawn | [hypothesis] | low |
| `FUN_0206C394` (`0206C394`) | `RandInt(13)` | `FUN_0206C034` | cluster with `FUN_0206C6E4` (Mersenne Twister) | [hypothesis] | low |
| `FUN_0206C590` (`0206C590`) | `RandInt(55)` | `FUN_0202EDF0` | cluster with `FUN_0206C6E4` | [hypothesis] | low |
| `FUN_0206C6E4` (`0206C6E4`) | `Rand32Bit` ×2 | `MersenneTwister_InitializeState`, `Random32MersenneTwister`, `FUN_0206C0F0` to `FUN_0206C590` | the personality test (see [Mersenne Twister](#mersenne-twister)); the comment's label was a Mersenne Twister user | [hypothesis] | medium |
| `FUN_ov_0000__02140860` (`02140860`, overlay 0) | `Rand32Bit` ×1, `RandInt` ×3 | about 100 callees, including `GetNumberOfFilledInventorySlots` and `AddToTeamMoney` | large overlay-0 routine; subsystem unclear | [hypothesis] | low |
| `FUN_ov_0000__021548EC` (`021548EC`, overlay 0) | `RandInt(15)` | about 16 callees | overlay-0 routine; subsystem unclear | [hypothesis] | low |
| `FUN_ov_0001__0213E6D8` (`0213E6D8`, overlay 1) | `RandInt(2)`, `RandInt(0xFF)`, `RandInt(n*2+8)` | `DungeonRunFrameActions`, `GetLeader`, `MemoryAlloc` | dungeon run-level helper | [hypothesis] | low |
| `FUN_ov_0001__021492EC` (`021492EC`, overlay 1) | `RandInt(4)` | `GetTile`, `GetTileMut` | dungeon tile operation; one of four directions | [hypothesis] | medium |
| `FUN_ov_0001__02155760` (`02155760`, overlay 1) | `Rand32Bit` | `EntityIsValid`, `FUN_02012648`, `FUN_020198AC`, `FUN_0217AD64` | entity update using the sprite RNG | [hypothesis] | low |
| `FUN_ov_0001__0217AF58` (`0217AF58`, overlay 1) | `Rand32Bit` ×2 | `FUN_020128DC`, `FUN_02156134`, `FUN_02178C04` | entity routine | [hypothesis] | low |
| `FUN_ov_0001__0217B120` (`0217B120`, overlay 1) | `Rand32Bit` ×2 | `FUN_02012648`, `FUN_02025DA0`, `FUN_0213E314`, `FUN_0213EDF4`, `FUN_021401C0` | entity routine (about 20 callees) | [hypothesis] | low |
| `FUN_ov_0001__0219DCF0` (`0219DCF0`, overlay 1) | `RandInt(6)` ×2 | `FUN_ov_0001__0219CD64` | one of six draws; subsystem unclear | [hypothesis] | low |
| `FUN_ov_0001__0219F09C` (`0219F09C`, overlay 1) | `RandInt(8)` | `EntityIsValid`, `FUN_ov_0001__0217ADF8` | entity routine | [hypothesis] | low |
| `FUN_ov_0001__0219F128` (`0219F128`, overlay 1) | `RandInt(8)` ×2 | `EntityIsValid`, `FUN_ov_0001__0217ADF8` | entity routine (same family as `0219F09C`) | [hypothesis] | low |
| `FUN_ov_0007__02138C34` (`02138C34`, overlay 7) | `RandInt(15)` | `FUN_02008040`, `FUN_020146C8`, `FUN_02014A64`, `FUN_02015F40` | overlay-7 routine | [hypothesis] | low |
| `FUN_ov_0007__02138EFC` (`02138EFC`, overlay 7) | `RandInt` ×2 | `FUN_0200661C`, `FUN_02007450` | overlay-7 routine | [hypothesis] | low |
| `FUN_02001330` (`02001330`) | `RandInt(10)`, `RandInt(0x12)` | `FUN_02001330` (recursive) | unknown small routine | [hypothesis] | low |
| `FUN_02005828` (`02005828`) | `RandInt` ×2 | `FUN_0200661C` | unknown | [hypothesis] | low |
| `FUN_02007184` (`02007184`) | `RandInt(16)`, `RandInt(4)` | `FUN_0200220C`, `FUN_02007164` | unknown | [hypothesis] | low |

**Counts (from the comment).** Of the 41 functions, 12 call `Rand32Bit` and 29 call `RandInt` or `RandRange` only. Of those: three are Kecleon shop or ware (two high, and `FUN_02008458` medium); two are per-frame or sprite (`FUN_02018ACC` high, `FUN_02012648` medium). Eight are in the `FUN_0200F768` or `FUN_0206A*` cluster, which is not named. The Mersenne Twister group (`FUN_0200D4DC`, `FUN_0206C6E4`, `FUN_0206C394`, `FUN_0206C590`) is medium.

**Where this file and the comment differ.**
- `FUN_0201DE24` is labelled "scene transition" in the comment. [`execution-model.md`](execution-model.md) (#161 P2) identifies it as item and money loss, not a transition, and this table follows that.
- The comment says `rng.md` lists passwords or Wonder Mail as candidates for the Mersenne Twister group. This file does not, and its own identification of `FUN_0200D4DC` and `FUN_0206C6E4` (see [Mersenne Twister](#mersenne-twister)) is used above. The password and Wonder Mail candidates are not added until checked.

**Not verified:** no runtime check; the names of the `FUN_0200F768` cluster and of the overlay-0 and overlay-1 entity routines rest on callee names only; the overlay-7 labels rest on the overlay-7 screen, which was not checked (see [#161 P3](https://github.com/TorratDev/rust-mm/issues/161)); the helpers `FUN_0206A6DC` and `FUN_0200661C` were not decompiled.

## Open questions

- The unnamed global-LCG callers are classified above with [hypothesis] labels. Still open: the `FUN_0200F768` cluster and the overlay-0 and overlay-1 entity routines (callee names only), and the dungeon callers (about 60 functions, #48, #50, #52). This feeds #48, #50 and #52.
- Correct the subsystem labels above once the bodies are read.
- Which of the 20 `OtherRandInt` callers use the 16-bit generator for gameplay, and which for presentation. Needs the bodies and an emulator check.
- Whether any other code writes `gOtherRngState` through a pointer alias (literal-pool search).
- An emulator vector for the 16-bit generator: a state dump before and after a call that uses it.
- Whether the quick-save branch in the save routines leaves the global RNG state from the dungeon (not checked).
- The caller slot of `FUN_02018ACC` and the writer of its flag word (see [Per-frame steps](#per-frame-steps-159)). The A and Select check on the frame routine needs a boot-phase run (frames 2418 to 2561).
