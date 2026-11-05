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

Both match `pmd-red`: `sub_809542C` in `src/code_8094F88.c:270–283`, and case 15 in `src/personality_test1.c:96`. The constant 13 in the personality product loop matches `NUM_PERSONALITIES` in Red (`MISC` = 13 in `include/constants/personality_test.h`); see the tracker notes below (#170 P2). **For golden traces:** these calls draw from the global LCG, so a dump of `sPRNGState` must account for the extra draws when either routine runs.

### Personality tracker (#170 P2, P3)

Source: the #170 P2 and P3 comment by investigator A. Static, Ghidra copy `rescue-arm9-a`; read-only, nothing renamed or saved. `tracker` is `*DAT_0206C874` (a pointer to the tracker pointer) in `FUN_0206C6E4` (`0x0206C6E4`).

**Tags:** **[blue-verified]** for the offsets, the loop bound, the switch and the arithmetic; **[hypothesis]** for the reading of the 13 words as one score per personality; red-hypothesis for the meaning of the words.

| Offset | Size | Use (Blue) | Tag |
|--------|------|------------|-----|
| `+0x00` | u32 | call counter, `*T = *T + 1` on every call | blue-verified |
| `+0x3C` | u32 | phase selector: `switch` with cases 0 to 15. Cases 0 to 14 each call one routine. Case 15 is the final step. The routine returns 0, or 3 when done | blue-verified (switch and returns) |
| `+0x4C` to `+0x7C` | 13 words (`i` = 0 to 12) | read in the final loop, `value *= (i + word[i] + 3)`; the loop runs `while (iVar3 < 0xd)` | blue-verified (loop and arithmetic); [hypothesis] for one score per personality |

- **P2, the constant 13.** The loop bound is 13. In Red, `enum NaturePersonalities` runs `HARDY` (0) to `QUIRKY` (12), and `MISC` = 13 is the `#define NUM_PERSONALITIES` (`include/constants/personality_test.h`). Blue's bound matches `NUM_PERSONALITIES`. The count is the evidence; the field meaning is not established (medium).
- The result is seeded from `Rand32Bit()` and `MersenneTwister_InitializeState`. The output of `Random32MersenneTwister()` is added, retried if it is `-1`, then passed to `sub_8011C40`.
- **Not verified:** the routines behind cases 0 to 14 (`FUN_0206C590`, `FUN_0206C568` and others), apart from case 15's arithmetic; `sub_8011C40` and `sub_8011C34`; the 13 words as personality scores; runtime (no Blue dump configured, no emulator check).

## 16-bit generator

Tag: **[blue-verified]**. Blue's copy of `pmd-red`'s `OtherRandom16` (`src/other_random.c`). The NDS addresses that `pmd-red` gives for it (`arm9.bin::020127F4`) do not match this dump; the code at `020127F4` here is something else. The constants and the behaviour do match.

| Address | Name | Behaviour |
|---------|------|-----------|
| `02023B9C` | `OtherRandom16` | `state = state * 109 + 1021` in 16 bits (the store is `short`); returns the new state, zero-extended (`ldrh` at `02023BB4`) |
| `02023B84` | `OtherRandInt(n)` | `(n * OtherRandom16()) >> 16`: `mul` at `02023B90`, `asr #16` at `02023B94`; the state is unsigned, so for `0 < n < 0x8000` the result is in `[0, n)` |
| `02023B6C` | `OtherRandRange(low, high)` | `low + OtherRandInt(high - low)`; no equal-bounds case, so the generator still advances |

- **State:** `gOtherRngState` at `0x020E03F4` (`short`). The literal pool at `02023BBC` holds the address and at `02023BC0` the increment `1021`. The static initial value is `0x348C`, read from the image; `pmd-red`'s `gOtherRngState` is `0x348C` too.
- **Accesses:** x-ref to `020E03F4` lists three accesses, all in `OtherRandom16` (a read at `02023BA4`, a write at `02023BB0`, a read at `02023BB4`). A literal-pool search (#169 P3, below) finds no other static word that holds the address. Aliasing through a run-time pointer is not ruled out.
- **Seeding:** no runtime seed write was found. The state starts from the static value.
- **Callers:** x-ref to `OtherRandInt` (`02023B84`) lists 20 call sites: `OtherRandRange` (`02023B78`, 1), `FUN_02021d9c` (5, at `02021E88`, `02021EB0`, `02021ED8`, `02021EFC`, `02021F14`), `FUN_0201ff48` (1, at `0201FFFC`), and overlay 0 (13: `FUN_ov_0000__02140860` at `ov_0000::02141998`, `FUN_ov_0000__02154220` at `ov_0000::02154618`, and 11 call sites in `FUN_ov_0000__021421cc`). Investigator C's text says 18 direct sites; the list has 19 outside `OtherRandRange`. The list is taken as given. What these callers use the generator for is classified in [Caller classes (#169 P2)](#caller-classes-169-p2) below.
- **Emulator check:** attempted, not reached (runtime, #169 P1): `verification/v7-trace/other_rng.py` hooks `02023B9C` and `02023BB8` and saw no call in 27000 frames of the `boot_to_dungeon.txt` boot path (see [`verification.md`](../verification.md)). The vectors are still open. The unit tests in `crates/sim/src/rng.rs` check the formula only, from values computed separately.

### Caller classes (#169 P2)

Source: the #169 comment by investigator B. The 20 sites are in the ARM9 and in overlay 0 (ground mode). **None is in overlay 1 (dungeon).** A dungeon trace does not need this generator; a ground-mode trace does. Caller lists and call-site code: **[blue-verified]**. Subsystem labels: **[hypothesis]**, from the context lines and the functions named in the decompile.

| Caller | Sites | Use (from decompile) | Class | Confidence |
|--------|-------|----------------------|-------|------------|
| `OtherRandRange` (`02023B78`) | 1 | wrapper; its own callers are the uses | wrapper | high |
| `FUN_02021D9C` (`02021E88` and four more) | 5 | a facing transform on one of 8 directions (checked `0 <= dir < 8`), mode 0 to 9: the 5 random sites are in modes 6 (2), 7, 8 (`rand(8) & ~1`) and 9 (`rand(8)`), each a random turn or a random direction, then `x mod 8` through `FUN_0208625C` [blue-verified, high] | presentation (likely) | medium |
| `FUN_0201FF48` (`0201FFFC`) | 1 | opcode `0xB` of a script arithmetic operator evaluator (0 set, 1 sub, 2 add, 3 mul, 4 div, 5 mod, 6 and, 7 or, 8 xor, 9 set bit, 10 clear bit, `0xB` = `rand(b)`, `a` ignored), used by script value cases `0xC1` and `0xC2` of `FUN_ov_0000__021421CC` [blue-verified, high] | script-defined (random operand) | medium |
| `FUN_ov_0000__02140860` (`ov_0000::02141998`) | 1 | "S10 event check": if the event state is 0 (`FUN_02020D90`), a 1-in-256 roll sets event flag `0x44` | gameplay (ground event trigger) | medium |
| `FUN_ov_0000__02154220` (`ov_0000::02154618`) | 1 | for states `0xE` to `0x1D`, a random value `0` to `17` goes to an actor field at `+0xA0` | presentation (likely) | low |
| `FUN_ov_0000__021421CC` (`ov_0000::021440B0` and ten more) | 11 | 8 sites place or jitter an actor position (8.8 fixed point: `<< 8`, `+0x18`/`+0x19`); 2 sites write a random value to script field `+0x4E` (`case 0x6D`: `rand(param)`; `case 0xDC`: `min + rand(max - min)`), which looks like a command duration or step counter, so a random duration [hypothesis, medium: no read of `+0x4E` found]; 1 site (`case 0xC3`) is a random script `switch` value: the result goes through `FUN_ov_0000__021405B4` to field `+0x28` [blue-verified, high] | 8 presentation, 2 random duration (hypothesis), 1 random script switch value | medium |

Totals: presentation (likely) 14 sites (5, 1 and 8 in the three rows above); gameplay 1 (`FUN_02140860`); script-defined 4 (`FUN_0201FF48`, the two `+0x4E` sites, and the `case 0xC3` switch value, which is not an advance-only call); wrapper 1. That is 20.

Only the `FUN_02140860` event trigger is a clear gameplay use. **Not verified:** no emulator run for any caller; which scripts use opcode `0xB`, commands `0x6D` and `0xDC` and switch `0xC3`; who reads and decrements `+0x4E`. `FUN_0208625C` is the signed divide with remainder (`__aeabi_idivmod`: r0 quotient, r1 remainder, divisor 0 gives quotient 0 and remainder `a`), so "wrap to 8" is `x mod 8` through r1 [blue-verified, high]. The emulator check of the state and the zero-extended return is still open (see [Open questions](#open-questions)).

### Literal-pool search for the state address (#169 P3)

Tags: **[blue-verified]** (static search). High confidence for the exact search; medium for "no alias", since run-time pointers are not covered.

- `0x020E03F4` (bytes `f4 03 0e 02`, all of memory): one hit, at `02023BBC`. That is the literal pool entry that `OtherRandom16` already uses.
- `0x020E03F8` (bytes `f8 03 0e 02`): 19 hits, all in ARM9 code between `02023DD8` and `020255FC`. Each is the address of a global pointer variable that holds the base of a heap block of 0x20 entries of 0xD0 bytes, allocated by `FUN_02025508` (entry `+4 = -1` marks a free slot; effect or task manager is a hypothesis). All 19 functions (36 pool loads) were read: 34 loads read the word at offset 0, 2 (in `FUN_02025508`) write it, and none uses a negative offset or `sub` on the loaded register, so none reaches `0x020E03F4` (`base - 4`) [blue-verified, static, high].
- `0x020E03F0`: no hits.

**Result:** no static literal or data word holds `0x020E03F4` except the one already known, and none of the 19 `0x020E03F8` hits reaches it, so the state has no static alias. Aliasing through a run-time computed pointer is not ruled out, and the `0x020E03F8` search covered the ARM9 block only, not the overlays.

**Dungeon callers:** none. The generator has no callers in overlay 1.

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
| | | | 65 unnamed functions after six names in other docs (#168 P1; 71 before that, and this table said "about 60") | classified in [Dungeon RNG callers](#dungeon-rng-callers-168-p1-p2) |
| `DungeonRandRange` `ov_0001::0219D014` | 62 | 19 | `GenerateFloor` (4), `GenerateSecondaryTerrainFormations` (3), `SpawnNonEnemies` (4), `SpawnEnemies` (2), `CreateRoomsAndAnchors` (4), `GenerateOuterRingFloor` (4), `GenerateCrossroadsFloor` (4), `GenerateTwoRoomsWithMonsterHouseFloor` (2) | floor generation |
| | | | 8 unnamed functions | unclassified |
| `DungeonRandOutcome` `ov_0001::0219CFCC` | 4 | 2 | `FUN_ov_0001__02159f10` (2), `FUN_ov_0001__02159f6c` (2) | secondary-effect roll: Serene Grace and Shield Dust (#168 P2b) |
| `DungeonRandOutcome_2` `ov_0001::0219CFF0` | 5 | 3 | `FUN_ov_0001__0218579c` (3), `FUN_ov_0001__02185568` (1), `FUN_ov_0001__02192d24` (1) | items, AI item use and AI move choice (#168 P2b) |

`DungeonRand16Bit` has one caller, `DungeonRandInt`. The unnamed callers of `DungeonRandInt` are 65 functions (71 before six were named in other docs; the old "about 60" was wrong; #168 P1); their site count is not given. The classification is in [Dungeon RNG callers](#dungeon-rng-callers-168-p1-p2).

**Global LCG** (ARM9 and overlays 0, 1, 7):

| Routine | Sites | Functions | Named callers (sites) | Label (medium) |
|---------|-------|-----------|-----------------------|----------------|
| `Rand32Bit` `0201198C` | 25 | 16 | `RandInt`, `SetRNGSeed` (`020118EC`), `RunDungeon_Async` (`0206DC20`), `GeneratePelipperJobs` (3) | RNG wrappers, seeding, items |
| | | | `FUN_0200d4dc` (1), `FUN_0206c6e4` (2) | Mersenne Twister users (see above) |
| | | | `FUN_ov_0001__0217af58` (2), `FUN_ov_0001__0217b120` (2, `UpdateMonsterSprite`), `FUN_02012648` (1, sprite animation) | animation |
| | | | `FUN_0200eda4`, `FUN_0200f768` (4), `FUN_02018acc` (2), `FUN_ov_0001__02155760` (1, called from the per-turn update `FUN_02142504`), `FUN_0206a228`, `FUN_0206a170`, `FUN_ov_0000__02140860` | mission generation (`FUN_0200f768` and its cluster, #168 P3b), frame callback (`FUN_02018acc`), sprite HUD (`FUN_ov_0001__02155760`), script task (`FUN_ov_0000__02140860`) |
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
| `FUN_0200EDA4` (`0200EDA4`) | `Rand32Bit` | `FUN_0200F184`, `FUN_0200F250`, `FUN_0200F2F0`, `FUN_0200F394`, `FUN_0200F3AC`, `FUN_0200F3E4` | fixed first (tutorial) job: frees finished slots, then writes one job (state 5, dungeon 2, floor 3) | [blue-verified] structure; [hypothesis] name | medium |
| `FUN_0200EED4` (`0200EED4`) | `RandRange` ×2 | `FUN_0200F768`, `FUN_0201F994`, `FUN_0206A66C` | GenerateMissions: fills the free slots of the 4-slot job array (one `FUN_0200F768` per slot, then the bonus-reward pick) | [blue-verified] structure; [hypothesis] name | high |
| `FUN_0200F3E4` (`0200F3E4`) | `RandInt` | `FUN_02006794`, `FUN_02007450` | pick the reward or target item (ids 1..0xEF that pass the item filters); called from `FUN_0200F768` and `FUN_0200EDA4` | [blue-verified] | high |
| `FUN_0200F46C` (`0200F46C`) | `RandInt` | `FUN_02009120` | pick a bonus-reward entry (ids 1..0x39 with table value 2 that no job record uses) | [blue-verified] | medium |
| `FUN_0200F5C4` (`0200F5C4`) | `RandInt`, `RandRange` | `FUN_020061D8`, `FUN_02006364`, `FUN_020703B8`, `FUN_0208625C` | pick a free (dungeon, floor) pair for a new mission; called from `FUN_0200F768` | [blue-verified] | high |
| `FUN_0200F768` (`0200F768`) | `Rand32Bit` ×5, `RandInt` ×3 | `FUN_0206A0B0`, `FUN_0206A170`, `FUN_0206A228`, `FUN_0206A2E0`, `FUN_0206A384`, `FUN_0206A6DC`, `02069FF0` | GenerateMission: one job record (dungeon and floor, job type, seed, client and target species, reward item, subtype) | [blue-verified] structure; [hypothesis] name | high (structure), medium (names) |
| `FUN_02069FF0`, `FUN_0206A0B0`, `FUN_0206A2E0` (`02069FF0`, `0206A0B0`, `0206A2E0`) | `RandInt` | `FUN_0206A6DC` (common helper) | mission subtype pickers: `02069FF0` (subtype 6) and `0206A0B0` pick a (species, item) pair; `0206A2E0` picks a species pair (subtype 7) | [blue-verified] structure; [hypothesis] names | medium |
| `FUN_0206A170`, `FUN_0206A228` (`0206A170`, `0206A228`) | `Rand32Bit` + `RandInt` | `FUN_0206A6DC` | mission subtype pickers: species pairs; `0206A170` sets subtype 9 and swaps client and target on `Rand32Bit() & 0x10`; `0206A228` subtype not read | [blue-verified] structure; [hypothesis] names | medium |
| `FUN_02012648` (`02012648`) | `Rand32Bit` | none in the RNG family | sprite animation; already noted above | [blue-verified] for the call; [hypothesis] for the label | medium |
| `FUN_02018ACC` (`02018ACC`) | `Rand32Bit` ×2 | `FUN_02017B20` (input poll), `FUN_0201D150`, `FUN_0201966C` | frame callback: per-frame RNG (see [Per-frame steps](#per-frame-steps-159)) | [blue-verified] | high |
| `FUN_0201DE24` (`0201DE24`) | `RandInt(100)` | `FUN_02007B68` | item and money loss, not a transition (see [`execution-model.md`](execution-model.md)); one draw of 0 to 99 | [hypothesis] | medium |
| `FUN_0201F008` (`0201F008`) | `RandInt(3)` | `OpenFileAndGetFileDataPtr`, `MemoryAlloc`, `MemoryFree`, `GetLanguage` | file-loading routine that picks one of three variants | [hypothesis] | low |
| `FUN_0204730C` (`0204730C`) | `RandInt(0xFF)` | `FUN_0200475C` | unknown; one byte drawn | [hypothesis] | low |
| `FUN_0206C394` (`0206C394`) | `RandInt(13)` | `FUN_0206C034` | cluster with `FUN_0206C6E4` (Mersenne Twister) | [hypothesis] | low |
| `FUN_0206C590` (`0206C590`) | `RandInt(55)` | `FUN_0202EDF0` | cluster with `FUN_0206C6E4` | [hypothesis] | low |
| `FUN_0206C6E4` (`0206C6E4`) | `Rand32Bit` ×2 | `MersenneTwister_InitializeState`, `Random32MersenneTwister`, `FUN_0206C0F0` to `FUN_0206C590` | the personality test (see [Mersenne Twister](#mersenne-twister)); the comment's label was a Mersenne Twister user | [hypothesis] | medium |
| `FUN_ov_0000__02140860` (`02140860`, overlay 0) | `Rand32Bit` ×1, `RandInt` ×3 | `FUN_0200EED4` (case 4), `FUN_0200EDA4` (case 6), debug strings | script-task executor (`script_task_type_error`): case 2 seeds the game, cases 4 and 6 generate missions, case 0x29 `CREATE_NPC_NEW_FRIEND` picks a random species | [blue-verified] (strings and calls) | high |
| `FUN_ov_0000__021548EC` (`021548EC`, overlay 0) | `RandInt(15)` | roster and table lookups; debug string `monster index error` | script monster-index resolver; the `RandInt(15)` is index 0x24, a random pick from a 15-entry species table | [blue-verified] (strings); medium for the table | high (label) |
| `FUN_ov_0001__0213E6D8` (`0213E6D8`, overlay 1) | `RandInt(2)`, `RandInt(0xFF)`, `RandInt(n*2+8)` | `DungeonRunFrameActions`, `GetLeader`, `MemoryAlloc` | wind or blast particle spawn (only caller `0213EE9C`, from `UpdateWindTurns`); cosmetic | [blue-verified] body; [hypothesis] name | medium |
| `FUN_ov_0001__021492EC` (`021492EC`, overlay 1) | `RandInt(4)` | `GetTile`, `GetTileMut` | dungeon tile operation; one of four directions | [hypothesis] | medium |
| `FUN_ov_0001__02155760` (`02155760`, overlay 1) | `Rand32Bit` | `EntityIsValid`, `FUN_02012648`, `FUN_020198AC`, `FUN_0217AD64` | team-member HUD sprite update, called per team slot from `FUN_02142504` | [blue-verified] body; [hypothesis] name | medium |
| `FUN_ov_0001__0217AF58` (`0217AF58`, overlay 1) | `Rand32Bit` ×2 | `FUN_020128DC`, `FUN_02156134`, `FUN_02178C04` | SetEntityAnimation: sets the animation and direction, starts the sprite at a random frame (`Rand32Bit() & 3`); cosmetic | [blue-verified] body; [hypothesis] name | high |
| `FUN_ov_0001__0217B120` (`0217B120`, overlay 1) | `Rand32Bit` ×2 | `FUN_02012648`, `FUN_02025DA0`, `FUN_0213E314`, `FUN_0213EDF4`, `FUN_021401C0` | per-frame sprite update of one entity (consistent with `UpdateMonsterSprite` in [symbols.md](../symbols.md)); called from `FUN_02142504` | [blue-verified] body; [hypothesis] name | high |
| `FUN_ov_0001__0219DCF0` (`0219DCF0`, overlay 1) | `RandInt(6)` ×2 | `FUN_ov_0001__0219CD64` | one of six draws; subsystem unclear | [hypothesis] | low |
| `FUN_ov_0001__0219F09C` (`0219F09C`, overlay 1) | `RandInt(8)` | `EntityIsValid`, `FUN_ov_0001__0217ADF8` | face the 16 enemies (`gDungeon+0x11324`) toward a direction; a random facing if the direction is 8 or more | [blue-verified] | high |
| `FUN_ov_0001__0219F128` (`0219F128`, overlay 1) | `RandInt(8)` ×2 | `EntityIsValid`, `FUN_ov_0001__0217ADF8` | face the team and the enemies with `info+0x98 == 2` toward a direction; random facing if 8 or more (same family as `0219F09C`) | [blue-verified] | high |
| `FUN_ov_0007__02138C34` (`02138C34`, overlay 7) | `RandInt(15)` | `FUN_02008040`, `FUN_020146C8`, `FUN_02014A64`, `FUN_02015F40` | overlay-7 routine | [hypothesis] | low |
| `FUN_ov_0007__02138EFC` (`02138EFC`, overlay 7) | `RandInt` ×2 | `FUN_0200661C`, `FUN_02007450` | overlay-7 routine | [hypothesis] | low |
| `FUN_02001330` (`02001330`) | `RandInt(10)`, `RandInt(0x12)` | `FUN_02001330` (recursive) | unknown small routine | [hypothesis] | low |
| `FUN_02005828` (`02005828`) | `RandInt` ×2 | `FUN_0200661C` | unknown | [hypothesis] | low |
| `FUN_02007184` (`02007184`) | `RandInt(16)`, `RandInt(4)` | `FUN_0200220C`, `FUN_02007164` | unknown | [hypothesis] | low |

**Counts.** These are counted from the table above. It has 35 rows and 38 functions (a grouped row counts one per function). Of the 38, 12 call `Rand32Bit` and 26 call `RandInt` or `RandRange` only. The call sites in the table sum to 66 (my count from the RNG-call column).

The comment gives 41 functions, 76 call sites, and 12 and 29 for the two groups. The table does not reproduce these: it is 3 functions and about 10 call sites short. The 3 missing functions would be `RandInt`-only (29 minus 26), but the comment does not name them, so they are not added here. The comment's figures are cited as the comment's, and the table figures are the ones used in this section.

Labels from the comment: three Kecleon shop or ware functions (`FUN_02006B4C`, `FUN_02006E0C`, and `FUN_02008458` as item or bulk generation); two per-frame or sprite (`FUN_02018ACC` high, `FUN_02012648` medium); the `FUN_0200F768` or `FUN_0206A*` cluster, which is now labelled mission generation (#168 P3b). The Mersenne Twister group (`FUN_0200D4DC`, `FUN_0206C6E4`, `FUN_0206C394`, `FUN_0206C590`) is medium.

**Where this file and the comment differ.**
- `FUN_0201DE24` is labelled "scene transition" in the comment. [`execution-model.md`](execution-model.md) (#161 P2) identifies it as item and money loss, not a transition, and this table follows that.
- The comment says `rng.md` lists passwords or Wonder Mail as candidates for the Mersenne Twister group. This file does not, and its own identification of `FUN_0200D4DC` and `FUN_0206C6E4` (see [Mersenne Twister](#mersenne-twister)) is used above. The password and Wonder Mail candidates are not added until checked.

**Not verified:** no runtime check; the mission cluster names are hypotheses (its structure is read); `FUN_0200F368`, `FUN_0200F568`, `FUN_02005828`, `FUN_0206A694`, `FUN_02006794` and `FUN_02007450` were not decompiled; the meaning of record fields `+0`, `+2` and `+0x11`, and which array is the accepted list, are not established; the Red Mission comparison is from memory, not read; the overlay-7 labels rest on the overlay-7 screen, which was not checked (see [#161 P3](https://github.com/TorratDev/rust-mm/issues/161)); `FUN_0200661C` was not decompiled. The overlay-0 and overlay-1 routines in the table are read from their bodies (#168 P3b).

## Dungeon RNG callers (#168 P1, P2)

Source: the #168 comments by investigator A (P1) and investigator B (P2). Tags: **[blue-verified]** for the x-ref, call lines and decompile; **[hypothesis]** for subsystem labels, which come from named callees (the P1b and P2b labels come from bodies, with names as noted in each row). Copy A (`rescue-arm9-a`) shows the names as `FUN_`, so the names in [`symbols.md`](../symbols.md) are not applied here. Nothing was run in the emulator.

### `DungeonRandInt` callers (#168 P1)

`DungeonRandInt` (`ov_0001::0219D058`) has 175 call sites, all unconditional, from 85 distinct functions. In copy A, 14 of those functions are named and 71 are unnamed. The "about 60" elsewhere in this file is corrected to 71. Six of the 71 have names in other docs (see the 19-caller table below), so 65 stay unnamed.

The 71 unnamed functions, by bucket (**[hypothesis]**; bucket names medium, bucket sizes low):

| Group | Functions | Basis |
|-------|-----------|-------|
| Documented name exists in `symbols.md` (13; see below) | 13 | name |
| Floor generation or terrain (`GetTileMut`, `GetFloorType`, `InitDungeonGrid`, `AssignRooms`) | 10 | named callees |
| Tile or entity interaction, not floor generation | 9 | named callees |
| Combat and moves (`GetMoveBasePower`, `InitPokemonMove`, `GetMoveTypeForMonster`, move executor `ov_0001::0215B384`) | 7 | named callees |
| Held item, ability or weather | 5 | named callees |
| Message selection (`GetLanguage` string tables) | 6 | named callees |
| AI weight filter `02192728` | 1 | named callee |
| No named callee: bodies read in #168 P1b (6 have names in other docs, 13 are labelled by us; see below) | 19 | none |

The bucket counts sum to 38, against 39 in the comment, so one of the 71 is not accounted for. The comment says the bucket totals may be off by one.

**Documented names (13), as in `symbols.md`:** `PlayMoveAttackerAnim` (`0213DD38`), `CarveShopAndPlaceItems` (`02149AF8`), `AccuracyCalc` (`0215A928`), `HasStatusThatPreventsActing` (`02180048`), `EnemyEvolution` (`02183BB4`), `AIDecideUseItem` (`0218579C`), `DoEndOfTurnEffects_Async` (`021861E8`), `RunMonsterAI` (`02188030`), `AIWander` (`02190854`), `AIMovement` (`021918D4`), `IsAITargetEligible` (`02192274`), `ChooseAIMove` (`02192D24`), and the recruit check `0217F9B8` (named in the P1 comment as `docs/re/data.md`; that file is not in this checkout, so the link is not added). Two of these disagree with the P2 labels below: `02149AF8` is `CarveShopAndPlaceItems` here, while P2 calls `FUN_02149AF8` floor generation (medium); `0218579C` is `AIDecideUseItem` here, while P2 calls `FUN_0218579C` items (medium). Both labels are kept as the investigators gave them.

**The 19 callers with no named callee (#168 P1b).** Source: investigator A. Each body (decompile text, all under 200 lines) and its callers were read; all 19 are in `ov_0001`. Tags: **blue-verified** for the body and callers; the names are ours (**hypothesis**) unless marked *docs*, which means the name is already in another doc (a cross-check, not new). Six of the 19 are *docs*, so 13 stay labelled by us only.

| Address | Label | What the body does (roll) | Callers | Tag, confidence |
|---------|-------|---------------------------|---------|-----------------|
| `021388D4` | monster species pick (weighted) | `DungeonRandInt(total)`, walks the species table cumulatively per column until it passes the roll; fallback `0x17C` | `021826A0`, `0219037C`, `021624D4`, `0219752C` (summon trap), `0219706C`, `02179E2C` | blue-verified; hypothesis (name). High for the pick, medium for the exact table |
| `02138B74` | floor item pick (weighted) | roll over 12 category weights, then a second roll over the 0xF0 item weights of that category; fallback `0x69` | `02179990`, `02163724`, `021A75C0`, `02144714` | blue-verified; hypothesis (name). High |
| `02138C68` | trap id pick (weighted) | roll over 20 trap weights; fallback `0x10` | `02198824` (random trap placement), `021983F8` | blue-verified; hypothesis (name). High |
| `02143290` | item quantity roll | `DungeonRandInt(100)` halves the index until `table[i] <= n`; stores the quantity; fallback 1 | `021448A0` | blue-verified; hypothesis (name). Medium |
| `021448A0` | item sticky flag and quantity | mode 0: `DungeonRandInt(100) < gDungeon+0x1A300` sets item flag 8; mode 1 forces it; category 6 goes to `02143290` | `02144714`, `021A75C0`, `0217D5FC`, `02163724`, `021613F0`, `02197828` | blue-verified; hypothesis (name). High |
| `02145660` | item-use effect: one of five status setters | `DungeonRandInt(5)` after a status call picks one of `0218A410`, `02189CBC`, `0218A710`, `021885A0`, `02189534` | `02146880` (item effect dispatcher, case 0x54) | blue-verified (caller); hypothesis (name). Medium; the item is not identified |
| `0214BD40` | array shuffle (spawn markers) | 2n swaps of two `DungeonRandInt(n)` entries | already named in [dungeon-generation.md](dungeon-generation.md) | docs, high |
| `0214CF2C` | GenerateMazeRoom (standard step 8) | `DungeonRandInt(100) < chance`, gate `+0x34B6 < 0`, shuffle, then picks a cell and calls `0214CC7C` | `GenerateStandardFloor` (`02151CCC`), `GenerateOuterRoomsFloor` (`02150C24`) | docs, high |
| `0214F224` | grid-cell connections (random walk) | body of AssignGridCellConnections | `021508E0`, `0214F6A0` | docs, high |
| `0214F6A0` | AssignGridCellConnections (wrapper) | `DungeonRandInt(w)`, `DungeonRandInt(h)`, then `0214F224` | `GenerateStandardFloor`, `GenerateOuterRingFloor`, `GenerateLineFloor` | docs, high |
| `021602FC` | Present (move 115) | roll 100: under 10 a 0x78 damage branch; under 0x1E heal maxHP/4 (`0218DF38`); under 0x3C a 0x50 damage; else 0x28 | `02156824` | docs (see [move-effects.md](move-effects.md)), high |
| `02161A00` | Tri Attack (move 0xCF) | `DungeonRandInt(3)` picks the paralysis-like status (`0218A710`), a second status (`02189F68`) or the default (`021885A0`) | gates `0215675C`, `02159F6C` | docs (see [move-effects.md](move-effects.md)), high |
| `021659D0` | confused or cowering direction, with walkability test | confusion: random direction 0..7, rotated until `0218141C` accepts; cowering: turn around | `0218453C` (action execution) | blue-verified; hypothesis (name). Medium-high |
| `02187FC4` | confused or cowering direction, no walkability test | as `021659D0` without the test; updates facing through `02176D10` | `0218453C` | blue-verified; hypothesis (name). Medium-high |
| `02182A94` | level-up: drop a random move | counts moves with flag bit 0, clears bit 1, `DungeonRandInt(count)`, deletes that entry from the 8-entry list | `02182E5C` (move list update that inserts learned moves) | blue-verified; hypothesis (name). Medium |
| `02182B3C` | initial move set from the learnset | fills 4 slots from the learnset up to the level; when full, `DungeonRandInt(4)` picks the slot to overwrite | `0217A7EC`, `0217AA14` | blue-verified; hypothesis (name). High |
| `021972B4` | gust trap: random push direction | `DungeonRandInt(8)`, then `021934D4` (push) | trap dispatcher `02197D88`, case 7 | blue-verified (case); hypothesis (name). Medium |
| `02197CB8` | mud trap: random stat drop | roll 100: under 25 or under 50 drops stat A or B (`02189534`); else `021893A4` | `02197D88`, case 1 | blue-verified (case); hypothesis (name). Medium |
| `021A7124` | scripted event: random not-yet-got entry | collects ids 1..0x39 whose table value (`02009120`) is not 2 or 3 and whose got byte is 0; `DungeonRandInt(n)`; returns 0x3A if none | `021A75C0` (a long event routine that marks the pick with `02009014`) | blue-verified (caller); hypothesis (name). Low on what the entries are |

**Trap dispatcher `02197D88` (note, #168 P1b).** A switch on a trap-id byte. Case 4 is summon (it calls the species pick `021388D4` and `0219752C`), and case 6 is warp (the id that `02198824` uses in #165). Blue-verified for the summon case and the warp id. Cases 7 (gust), 1 (mud) and 3 (grimy item) are not checked beyond the caller table above.

### `DungeonRandRange` callers (#168 P2)

`DungeonRandRange` (`ov_0001::0219D014`) has 62 call sites in 19 functions. Counts match [Dungeon LCG](#dungeon-lcg-blue-verified).

| Caller | Sites | Subsystem | Evidence | Confidence |
|--------|-------|-----------|----------|------------|
| `GenerateFloor` (`021520F4`) | 4 | floor generation | room and corridor sizes; calls the floor-shape generators | high |
| `GenerateCrossroadsFloor` (`02151468`) | 4 | floor generation | calls `InitDungeonGrid`, `GetTileMut` | high |
| `GenerateOuterRingFloor` (`02151900`) | 4 | floor generation | calls `InitDungeonGrid`, `GetTileMut` | high |
| `GenerateTwoRoomsWithMonsterHouseFloor` (`02150580`) | 2 | floor generation | calls `InitDungeonGrid`, `GetTileMut` | high |
| `GenerateSecondaryTerrainFormations` (`0214ABB8`) | 3 | floor generation | range over the grid; calls itself | high |
| `CreateRoomsAndAnchors` (`0214F8BC`) | 4 | floor generation | room placement; calls itself | high |
| `FUN_02149AF8` (`02149B34`) | 3 | floor generation | `GetTileMut`, `DungeonRandInt`; grid range | medium |
| `FUN_0214A3D0` (`0214A524`) | 4 | floor generation | `GetTileMut`; grid range | medium |
| `FUN_0214C244` (`0214C6E4`) | 4 | floor generation | `GetTileMut`; room range | medium |
| `FUN_0214DB54` (`0214DC1C`) | 10 | floor generation | `GetTileMut`; room range | medium |
| `FUN_0214E824` (`0214E9A8`) | 6 | floor generation | `GetTile`, `GetTileMut`; room range | medium |
| `FUN_0214FF30` (`0215000C`) | 2 | floor generation | `GetTile`, `GetTileMut`; `x`/`y` from a room | medium |
| `SpawnEnemies` (`0214B36C`) | 2 | spawn (enemies) | named; spawn bounds | high |
| `SpawnNonEnemies` (`0214B898`) | 4 | spawn (non-enemies) | named; spawn bounds and count | high |
| `FUN_0219CF28` (`0219CF3C`) | 1 | status duration | table range, adjusted by level type and ability, minimum 1 (`CalculateStatusTurns` by behaviour); P2 calls it the source of the leech-seed counter in #166 (**[hypothesis]**) | high |
| `FUN_021967E0` (`02196AB4`) | 2 | combat (move effects) | calls `sub_806F370_Async` (damage), `InitPokemonMove`, `DungeonRunFrameActions` | medium |
| `FUN_0219037C` (`0219055C`) | 1 | dungeon event: monster house trigger (`TriggerMonsterHouse_Async`; see the P2b subsection) | `GetTileMut`, `DungeonRunFrameActions`; this `DungeonRandRange(min, max)` site rolls the monster count (capped to the candidates); a separate `DungeonRandInt(cap)` picks each tile among up to 100 candidates | blue-verified; red-hypothesis (name); high |
| `FUN_0215FB74` (`0215FB94`) | 1 | move effect: fixed-damage move, Psywave (`case 0x6C`; see the P2b subsection) | range 0x80 to 0x180 times the user's level (`info+9`), divided by 0x100; result to `FUN_02156488` | blue-verified; red-hypothesis (name); high |
| `FUN_0215A380` (`0215A39C`) | 1 | none (RNG advance only) | result discarded, under a `FUN_0200B408() == 0` guard | high |

Totals: 50 sites in floor generation, 6 in spawning, 6 in the rest (62). **`FUN_0215A380` is RNG-advance-only**: its result is discarded, so a trace that counts RNG draws counts it.

### Outcome-wrapper callers (#168 P2)

`DungeonRandOutcome` (`ov_0001::0219CFCC`) has 4 sites in 2 functions. `DungeonRandOutcome_2` (`ov_0001::0219CFF0`) has 5 sites in 3 functions.

| Caller | Sites | Subsystem | Evidence | Confidence |
|--------|-------|-----------|----------|------------|
| `FUN_02159F10` (`02159F54`) | 2 | secondary-effect roll (`RollSecondaryEffect`); Serene Grace (`0x26`) doubles the chance | `AbilityIsActive(entity, 0x26)`, then `DungeonRandOutcome(pct << 1)` or `DungeonRandOutcome(pct)` | blue-verified; red-hypothesis (names); high |
| `FUN_02159F6C` (`02159FF8`) | 2 | secondary-effect roll (`sub_805727C`), with `IsFloorOver`; Shield Dust (`0x4C`) blocks it on the target | `AbilityIsActive` for ids 0x26 and 0x4C, then the same roll as `FUN_02159F10` | blue-verified; red-hypothesis (names); high |
| `FUN_02185568` (`0218563C`) | 1 (`OUT2`) | AI item use: arc-item target collection (`GetPossibleAIArcItemTargets`) | per-target roll `DungeonRandOutcome_2(FUN_021473A8(e, item, 1))`; see the P2b subsection | blue-verified; red-hypothesis (name); high |
| `FUN_0218579C` (`02185820`) | 3 (`OUT2`) | items | `GetItemInfo`, item entity type 3, tile item lookup | medium |
| `FUN_02192D24` (`02192D80`) | 1 (`OUT2`) | AI (move choice) | `InitPokemonMove`, `IsTacticSet`, `GetMoveTypeForMonster` | medium |

Totals: 9 sites (4 secondary-effect rolls, 3 items, 2 AI: `FUN_02185568` and `FUN_02192D24`).

### Secondary-effect rolls, AI item targets and two events (#168 P2b)

Source: investigator B. Static Ghidra only; no runtime run. Red names are **red-hypothesis**, read from `pmd-red` (`dungeon_ai_items.c`, `dungeon_monster_house.c`, `dungeon_move_util.c`, `constants/ability.h`) and not copied.

- **`0x26` = Serene Grace** (`ABILITY_SERENE_GRACE` = `0x26` in Red's `constants/ability.h`). **Blue-verified:** when the user has it, the roll is `DungeonRandOutcome(pct << 1)` instead of `DungeonRandOutcome(pct)`.
- **`0x4C` = Shield Dust** (`ABILITY_SHIELD_DUST` = `0x4C`). **Blue-verified, in `FUN_02159F6C` only:** when the roll succeeds, the user is not the target and the target has Shield Dust, it calls `FUN_0213EB00` (Red `sub_80429C8`, the "unaffected" effect) and returns false, so the secondary effect is blocked.
- **`FUN_02159F10(entity, pct)`** = Red `RollSecondaryEffect`: `pct == 0` is always true, else the Serene Grace doubling. **`FUN_02159F6C(user, target, pct)`** = Red `sub_805727C`: it returns false if `IsFloorOver()`, either entity is invalid, or the target's `info+0x12A` or HP is 0; then the same roll and the Shield Dust check. The caller applies the move's additional effect when the roll is true. Tags: blue-verified for the logic; red-hypothesis for the names.
- **Once per attack per target, not per turn** (blue-verified callers). `02159F10` has 14 call sites and `02159F6C` about 45, all in move-effect handlers (for example `FUN_0215C654`, `FUN_0215CC64`, `FUN_0215F410`) reached only from the per-target dispatcher `FUN_02156824` (switch on move effect). That dispatcher is called by the move executors `FUN_0215B384` (at `0215B9D4`) and `FUN_0215AB34` (at `0215B004`). No per-turn caller exists.
- **`FUN_02185568` (AI arc-item targets)** = Red `GetPossibleAIArcItemTargets` (red-hypothesis; `dungeon_ai_items.c` lines 297–330). **Blue-verified body:** it keeps active entities that are valid, not the user, pass `FUN_02141C08` and `FUN_02180994(user, e, 0, 1) == 1` (can see), and are within Chebyshev distance under 0xB. If `param_4 == 0`, it makes one roll per target, `DungeonRandOutcome_2(FUN_021473A8(e, item, 1))` (Red `GetAIUseItemProbability`), and a success appends the target's position. Callers: `FUN_0218579C` (`AIDecideUseItem`) at `021858AC` and `02185B4C`. So this is AI item use, not combat or status.
- **`FUN_0215FB74` (Psywave)** = Red `PsywaveMoveAction` (red-hypothesis). **Blue-verified:** `DungeonRandRange(0x80, 0x180)` times the user's level (`info+9`), divided by 0x100; below 0 becomes 1 and above 199 becomes 199; then `FUN_02156488`. Its only caller is `FUN_02156824`, switch case `0x6C` (`02158BD0`).
- **`FUN_0219037C` (monster house trigger)** = Red `TriggerMonsterHouse_Async` (red-hypothesis). **Blue-verified, matching Red line by line:** it returns early unless the entity passes `info+6 == 0`, the tile has flag `0x40` (Red `TERRAIN_TYPE_IN_MONSTER_HOUSE`), and the one-shot flag at dungeon `+0x661` is 0 (it sets the flag to 1). The room is dungeon `+0x34AC` (Red `monsterHouseRoom`; see #165 P4), with 0xFF meaning none. The candidates are the room's tiles, or a 9x9 area around the entity. The count comes from `DungeonRandRange(min, max)`, capped to the candidate count. Each monster is placed by `DungeonRandInt(cap)` over the candidates (with wrap-around), then `FUN_021388D4(1)` (Red `GetRandomFloorMonsterId(1)`), a spawn struct with level 0, `FUN_02175BA0`, `FUN_02179CFC` (`SpawnWildMon`), the drop-in animation, and `FUN_021401CC(1)`. Its nine callers are movement and step handlers (`02187200`, `021732C4`, `02173F18`, `0217371C`, `021740EC`, `02173CCC`, `02173A48`, `02174280`, `02173800`), not AI.

**Not verified (#168 P2b):** the exact status each of the ~59 secondary-effect handlers applies (only sampled); the identity of `DungeonRandOutcome` and `DungeonRandOutcome_2` in Blue (the table above lists both; P2b did not check it); the behaviour for multi-hit moves (the roll runs per hit that reaches the handler, not verified); any runtime check.

## Open questions

- The unnamed global-LCG callers are classified above with [hypothesis] labels. Still open: the names of the `FUN_0200F768` mission cluster (its structure is read; the meaning of record fields `+0`, `+2` and `+0x11`, and which array is the player's accepted list, are not established). Of the dungeon callers (#168 P1), the 13 labels that have no name in other docs have these open points: `021A7124` (which event), `02145660` (which item), and the trap ids behind `021972B4` and `02197CB8` (the dispatcher `02197D88` is checked only for its summon and warp cases). The floor weight tables behind `021388D4`, `02138B74` and `02138C68` are not decoded. This feeds #48, #50 and #52.
- Correct the subsystem labels above once the bodies are read.
- The 20 `OtherRandInt` callers are classed in [Caller classes (#169 P2)](#caller-classes-169-p2) with [hypothesis] labels. Open: who reads `+0x4E`, and which scripts use opcode `0xB`, commands `0x6D` and `0xDC`, and switch `0xC3`.
- Whether any run-time pointer writes `gOtherRngState` through an alias. The static literal-pool search found none (#169 P3, all 19 `0x020E03F8` hits read); overlays were not searched for `0x020E03F8`.
- An emulator vector for the 16-bit generator: a state dump before and after a call that uses it.
- Whether the quick-save branch in the save routines leaves the global RNG state from the dungeon (not checked).
- The caller slot of `FUN_02018ACC` and the writer of its flag word (see [Per-frame steps](#per-frame-steps-159)). The A and Select check on the frame routine needs a boot-phase run (frames 2418 to 2561).
