# Random number generators

Blue has three random number generators. All three match generators in the `pmd-red` decomp ([`references.md`](../references.md)) by constant and by behaviour, so they carry the `pmd-red` names. Issue: [#47](https://github.com/TorratDev/rust-mm/issues/47).

Dump: `BlueRescueTeamEur` ([`supported-dumps.md`](../supported-dumps.md)). Addresses are RAM addresses. `ov_0001::` marks code in overlay 1, which is loaded while a dungeon runs; plain addresses are in the ARM9 binary. The names are applied to a Ghidra project by [`tools/ghidra/RescueRngLabels.java`](../../tools/ghidra/RescueRngLabels.java), which checks each constant below before it changes anything.

Tags: **[blue-verified]** means read in Blue's code; **[red-hypothesis]** means taken from `pmd-red` and not yet confirmed in Blue (see [#43](https://github.com/TorratDev/rust-mm/issues/43)). Nothing here is confirmed by an emulator capture yet.

## Summary

| Generator | State | Step | Output | Seeded by | `pmd-red` file |
|-----------|-------|------|--------|-----------|----------------|
| Global LCG | `sPRNGState` `020DF704` (s32) | `s = s * 0x5D588B65 + 1` | `s >> 16` (16 bits); two steps make 32 bits | `SeedRng` at boot (console MAC address), `SetRNGSeed` from the save file | `src/random.c` |
| Dungeon LCG | `gDungeonRngState` `ov_0001::021B3F58` (u32) | `s = s * 0x5D588B65 + 1` | `s >> 16` (16 bits) | `InitDungeonRNG` once per floor | `src/dungeon_random.c` |
| Dungeon preseed LCG | `gDungeonRngPreseedState` `ov_0001::021B3F5C` (u32) | same step, two per call | odd 24-bit floor seed | `SetDungeonRNGPreseed23Bit` once per dungeon run | `src/dungeon_random.c` |
| Mersenne Twister | `sStateArray` `020DED24` (624 × u32), `sStateIndex` `020DED18`, `sTwistMatrix` `020DED1C` | MT19937 (`0x6C078965`, `0x9908B0DF`) | 32 bits | `MersenneTwister_InitializeState` | `src/random_mersenne_twister.c` |

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
- **From the save file:** `0202A1AC` calls `SetRNGSeed`, and `0202A53C` calls `GetRNGState`. In Red, `save.c` restores and stores `RngState` in the same way. [red-hypothesis for the Blue functions' role; the calls are blue-verified]

Use: 25 call sites of `Rand32Bit` (16 functions), 42 of `RandInt` (28 functions), 7 of `RandRange`. Callers are in the ARM9 and in overlays 0, 1 and 7, so **the global LCG also advances during dungeons**. A golden trace must capture both `sPRNGState` and `gDungeonRngState`.

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

Use: `DungeonRandInt` has 176 call sites in 86 functions, almost all in overlay 1. `DungeonRandRange` has 62 call sites (19 functions), and the two `DungeonRandOutcome` functions have 9. Sorting these call sites by subsystem (floor generation, combat, AI, items) is still to do. It feeds #48, #50 and #52.

## Seeding a dungeon run [blue-verified]

`RunDungeon_Async` (`0206DC20`, ARM9) matches Red's `RunDungeon_Async` (`src/run_dungeon.c`) by its seeding code. `LoadAndRunDungeon_Async` (`0201DB70`) loads the dungeon overlays, calls it, then unloads them (see [`execution-model.md`](execution-model.md)). `RunDungeon_Async` reaches the dungeon struct through the pointer at `DAT_0206EBF0` (`gDungeon` in Red).

1. **Once per run** (not when resuming): if `dungeon+0x668` (Red: `unk644.missionKind`) is `DUNGEON_MISSION_OUTONRESCUE`, the preseed comes from the dungeon setup (`setup+0x14`, Red: `info.dungeonSeed.seed`). Otherwise it is `Rand32Bit() & 0xFFFFFF`. It is stored at `dungeon+0x66C` (Red: `unk644.prngPreseed23Bit`) and passed to `SetDungeonRNGPreseed23Bit`.
2. **Once per floor:** `GenerateDungeonRNGSeed()` is stored at `dungeon+0x670` (Red: `unk644.unk3C`) and passed to `InitDungeonRNG`.

The three fields sit at the same offsets inside Red's `unkDungeon644` sub-struct (`+0x34`, `+0x38`, `+0x3C`). That sub-struct starts at `0x634` in Blue and at `0x644` in Red, so the fields before it in the dungeon struct are 0x10 bytes shorter in Blue. This is input for #46.

**For golden traces (V7):** the floor seed follows from the run preseed, and the run preseed follows from the global LCG. So it is enough to capture `sPRNGState` at dungeon entry, or `dungeon+0x66C` and `dungeon+0x670`. The rescue path offers a way to set the preseed directly.

## Mersenne Twister [blue-verified code, use unknown]

`MersenneTwister_InitializeState` (`0200CCB0`) fills `sStateArray` with the MT19937 recurrence (`0x6C078965`) and sets `sStateIndex` to 624. `Random32MersenneTwister` (`0200CB40`) is the generator; `sStateIndex` starts at 625 (`0x271`), which forces initialisation on first use. Only two functions call it, `0200D4DC` and `0206C6E4`, and both also call `Rand32Bit`. What they do is not known yet (candidates: rescue passwords or Wonder Mail).

## Open questions

- Sort the dungeon RNG call sites by subsystem (#48, #50, #52).
- Identify the two Mersenne Twister users.
- Confirm the role of `0202A1AC` and `0202A53C` (save load and save write?) when the save format is studied (#53).
- Confirm the sequences by emulator capture (#17): step the game, read `sPRNGState` and `gDungeonRngState`, and compare with this algorithm.
- `pmd-red`'s `src/other_random.c` documents a fourth, 16-bit generator with NDS addresses (`arm9.bin::020127F4`). Those addresses do not match this dump (the code at `020127F4` here is something else), so they appear to come from another Blue build. Look for it by its constants (multiplier 109, increment 1021).
