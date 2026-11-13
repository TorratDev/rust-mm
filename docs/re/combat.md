# Combat: damage, accuracy, targeting and defeat

First entry points into Blue's combat code, found from the code that reads the type chart. Issue: [#50](https://github.com/TorratDev/rust-mm/issues/50). Names, addresses and tags are in [`symbols.md`](../symbols.md). Everything here is in overlay 1, the dungeon overlay ([`execution-model.md`](execution-model.md)).

Every function below matches its `pmd-red` counterpart in `src/dungeon_damage.c` by argument count, constants and calls, so it keeps that name, including `pmd-red`'s placeholder names (`sub_806E100`, ...). Confirmed by reading Blue's code, not by emulator trace.

**Status.** Most of this page was read statically, from Blue's decompiled code, its overlay and ARM9 data, and `pmd-red` at the pinned commit. The runtime evidence is the damage formula check for 14 observed `CalcDamage` hits (#185, [verification](../verification.md#v6--blue-reuses-reds-logic)), the move-dispatch and EXP checks in the sections on [move effects](#move-effects) and [defeat and experience](#defeat-and-experience). The formula matches the hits, except the parts listed in [the damage formula](#damage-formula). The issue's condition, "matches observed emulator hits", holds for the cases those 14 hits cover. The hit check, critical-hit roll, Huge Power and Hustle, and the RNG order of a basic attack were checked in a later runtime pass, see [basic attack at runtime](#basic-attack-at-runtime-356).

## Functions

| Function | Role |
|----------|------|
| `CalcDamage` `ov_0001::0217BD94` | Damage formula. Same constants as Red: damage cap `999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`. Reads `gAtkStatStageMultipliers` and `gDefStatStageMultipliers`. |
| `sub_806E100` `ov_0001::0217CE88` | Type-effectiveness part of the damage calculation; called by `CalcDamage`. |
| `CalcStatBoosts` `ov_0001::0217C7F8` | Ability modifiers to attack and defense, called from `CalcDamage`. Its sixth argument is the roll from the first `DungeonRandInt(100)` ([RNG calls](#rng-calls-in-calcdamage)). Own name; the Red name is not confirmed. |
| `sub_806F370_Async` `ov_0001::0217BB94` | Applies a fixed amount of damage (34 callers). Wonder Guard (`AbilityIsActive(target, 0x35)`) zeroes it unless the combined effectiveness is "super". Then `HandleDealingDamage_Async`. |
| `HandleDealingDamage_Async` `ov_0001::0217E890` | Deals the damage. |
| `sub_806F4A4` `ov_0001::0217BA94` | Returns 1 when the move type is not immune to either of the target's two types, and 0 when it is immune to one of them. It also returns 0 for the Ghost exception (target type 14, move type 1 or 7, target not exposed). |
| `WeightWeakTypePicker` `ov_0001::0217CB3C` | Uses the chart to weigh move types against a target (AI). |
| `AbilityIsActive` `ov_0001::0218067C` | Ability check. |
| `AccuracyCalc` `ov_0001::0215A928` | Hit check. See [accuracy and evasion](#accuracy-and-evasion). |
| `IqSkillIsEnabled` `ov_0001::021805B0` | Bit test on the IQ bitfield. See [accuracy and evasion](#accuracy-and-evasion). |
| `HandleDamagingMove` `ov_0001::0215675C` | Damage handler for 60 move IDs in the dispatch. See [move effects](#move-effects). |

Red has a fifth reader of the type chart, `TargetNotImmuneTo`, marked unused; Blue has no matching code reference.

## Tables

All four have the same values as in Red (compared entry by entry; the values are not reproduced here, see [`legal.md`](../legal.md)):

| Table | Address | Shape |
|-------|---------|-------|
| `gTypeEffectivenessChart` | `ov_0001::021AAEE8` | 18 × 18 `s16`, row = attacking type, column = defending type |
| `gEffectivenessChart` | `ov_0001::021AA588` | 4 × 4 `s32`, combines the effectiveness against the two defending types |
| `gAtkStatStageMultipliers` | `ov_0001::021AA660` | 21 × s24.8, index = attack stat stage, 1.0 at index 10 |
| `gDefStatStageMultipliers` | `ov_0001::021AA6B4` | 21 × s24.8, index = defense stat stage |

The accuracy and evasion stage tables (`ov_0001::021AA8C4`, `ov_0001::021AA918`) are in [accuracy and evasion](#accuracy-and-evasion).

## Damage formula

`CalcDamage` runs these steps in this order. The constants and step order match Red's `CalcDamage` (static comparison, P1.1). The formula reproduces 14 captured hits, with the fixed-point corrections of steps 10 and 11 (#185, [verification](../verification.md#v6--blue-reuses-reds-logic)). Not checked by those hits: critical hits, items, abilities, status effects, weather and type effects (step 13).

1. A belly check that sets the damage to 1 (Red's `SetDamageOne`).
2. Wonder Guard, as in Red.
3. Clamp the attacker's and the defender's stat stages to 0 to 20.
4. Deoxys: a ±2 adjustment, as in Red. The Deoxys species (form) IDs are `0x1A1` to `0x1A3`.
5. Base power: the move's power plus the attacker's attack stat. Multiply by the attack stage multiplier, then by the attacker's offensive multiplier, in 24.8 fixed point. Convert to an integer by truncation.
6. Defense side (Skull Bash: +1 defense stage). Multiply the defense by its stage multiplier and then by the defender's defensive multiplier, and truncate.
7. Held-item boosts, in `CalcDamage` (see the held-item table below).
8. Clamp the attack to 0 to 999. Only the attack is clamped; the defense is not.
9. Ability modifiers, through `CalcStatBoosts` (see the ability table below).
10. Add the level-or-attack term, `r6`. The branch is on the attacker's `isNotTeamMember` (info `+0x06`) (blue-verified, P6):
    - `isNotTeamMember == 0`: `level * 2 / 3`, with `level` at info `+0x09`. Test `ldrb r0,[r5,#6]` at `0x0217c270`; body `0x0217c27c`–`0x0217c2bc`; divide at `0x0217c2b4`.
    - `isNotTeamMember != 0`: `attack / 3`. The attack is the value after `CalcStatBoosts` (call at `0x0217c2f4`) and is read from info `+0x14 + splitIndex` ([structs](structs.md)). Body `0x0217c2c0`–`0x0217c32c`; divide at `0x0217c328`.
    - The value used in the formula is the 48.16 fixed-point one: `level × 0xAAAA` for the first case, and `attack / 3` as a 48.16 divide for the second. Runtime-checked on 14 hits (#185). The integer `r6` (truncated `level * 2 / 3` or `attack / 3`, with `FUN_0208625c(x, 3)`) is stored at dungeon `+0x14A` (`rawDamageFormulaValues[5]`) and matches the hits too, but the truncated value does not feed step 11. Using it instead gives 14 of 14 mismatches. Red has the same branch at `dungeon_damage.c:1243-1249`.
11. Base damage = (attack − defense) / 8 + that term, in 48.16 fixed point (the division is not truncated).
12. Base damage `x` becomes `x² × 0xCCC + (2x − defense + 10)`, where `0xCCC` is the 0.05 constant. Clamp the result to [1.0, 999]. Runtime-checked on 14 hits (#185).
13. Multiply by the type multiplier from `sub_806E100` (see [type effectiveness](#type-effectiveness)). Not modelled in the #185 check (fixed at 1.0; the 14 hits allow [0.9868, 1.0025)).
14. Critical hit: ×1.5 when the crit roll passes (see [critical hits](#critical-hits)). The odds are 0 for a non-team attacker, as in Red. The crit path is not exercised by the #185 hits.
15. Scale by `arg8`, an s24.8 damage scale (1.0 = `0x100`). See [argument 8](#argument-8-arg8). Runtime-checked for `0x80` and `0x100` (#185).
16. Variance: a factor of `(0xE000 + roll) / 0x10000`, from `DungeonRandInt(0x4000)`. That is 0.875 to about 1.125.
17. Convert the 48.16 result to an integer with `FP48_16_ToS32` (see [rounding](#rounding)).

### Type effectiveness

`sub_806E100` looks up the move type's row in `gTypeEffectivenessChart`, uses the defender's two types (at info `+0x56` and `+0x57`) as columns, and combines the two results through `gEffectivenessChart`. Then it applies the following rules, in this order. Everything here matches Red line by line (static comparison, P1.2).

- **Ghost exception:** a Ghost defender (type 14) is treated as immune (the immune multiplier, 0.5) to Normal (type 1) or Fighting (type 7) moves, unless the defender is exposed (flag at info `+0xD2`, not confirmed in Blue).
- **Immunity abilities:** Flash Fire (against Fire moves with a status) and Levitate (against Ground moves) return "immune". Thick Fat halves Fire and Ice damage.
- **Low-HP abilities:** Torrent, Overgrow, Blaze and Swarm double the move's power when the attacker is at or below 1/4 HP.
- **STAB:** ×1.5 (the pool value shared with the critical multiplier).
- **Weather:** sunny, rain, cloudy and fog change the multiplier. The weather IDs (1, 3, 4, 6) come from Red's enum and were not checked in Blue.
- **Charging:** ×2.0.
- **Return value:** whether the combined result is "super-effective". Wonder Guard uses this.

The four effectiveness multipliers in the pool are 0.5, 0.9, 1.0 and 1.5. Which index maps to which multiplier and label is in [P1 of the damaging-moves section](#p1-type-factor-and-label) (#419).

### Abilities in `CalcStatBoosts`

These modifiers are applied in the stat-boost function (`ov_0001::0217C7F8`). Ability IDs are as in Red's `ability.h`. Integer division truncates toward zero.

| Ability or item | ID | Applies to | Effect |
|-----------------|----|-----------|--------|
| Guts | `0x11` | attacker | attack ×2 while the attacker has a negative status |
| Huge Power, Pure Power | `0x22`, `0x4B` | attacker | attack ×3/2 for physical moves when the roll is 32 or less (the roll is `CalcDamage`'s first `DungeonRandInt(100)`). Gate measured at runtime, see [P3](#p3-huge-power-and-hustle) |
| Hustle | `0x30` | attacker | attack ×3/2 for physical moves. Measured on one move only ([P3](#p3-huge-power-and-hustle)). Its target evasion effect is in [P5](#p5-static-leftovers) |
| Plus, Minus | `0x38`, `0x3F` | attacker and target (flag chosen by the not-enemy test) | ×3/2 on attack and defense for special moves. The flags are at `gDungeon` `+0x3358` and `+0x335A` (pairing inferred, see [open questions](#open-questions)) |
| Intimidate | `0x06` | target | attack ×4/5 for physical moves |
| Marvel Scale | `0x34` | target | defense ×3/2 while the target has a negative status |

Held items, applied in `CalcDamage` (static comparison, P2.2):

| Item | ID | Holder | Effect |
|------|----|--------|--------|
| Power Band | 19 | attacker | attack +12 |
| Munch Belt | 43 | attacker | attack +8 |
| Special Band | 33 | attacker | attack +12 |
| Scope Lens | 12 | attacker | critical odds +40 |
| Patsy Band | 13 | target | critical odds +40 |
| Def Scarf | 30 | target | defense +8 for physical moves |
| Zinc Band | 34 | target | defense +8 for special moves |

The item boosts are gated by the argument `arg_10`, as in Red.

### Critical hits

- The crit roll (`DungeonRandInt(100)`, `0217C698`) is drawn only when the defender has neither Battle Armor (`0x0C`) nor Shell Armor (`0x13`). This gate is static. The runtime probes drew the roll on every attack they ran ([P2](#p2-critical-hit-roll)), and the findings do not say whether those targets had either ability, so the gate stays open.
- Focus Energy (status 4) sets the odds to 999.
- IQ Type Advantage Master (IQ skill ID 1, not confirmed) sets the odds to 40 when the type result is super-effective.
- A crit multiplies by 1.5. The crit flag is stored in the damage struct at `+0xD` and is cleared when the damage is 0.

### Rounding

The fixed-point helpers round as follows (static, P2.5). "Blue address" is the ARM9 address, which has no overlay prefix.

| Helper | Blue address | Red name | Behaviour |
|--------|--------------|----------|-----------|
| 24.8 multiply | `02017804` | `s24_8_mul` | Rounds half up, using bit 7 of the result |
| 24.8 to integer | `0208625C` | `F248ToInt` | Truncates toward zero. In Blue it is called as `(x, 0x100)`, so it may be a general signed divide |
| 48.16 multiply | `02016FF4` | `F48_16_UMul` | Rounds half up, using bit 15 |
| 48.16 to integer | `02017620` | `FP48_16_ToS32` | Integer part, plus 1 when bit 15 of the low word is set |
| Integer to 48.16 | `02017654` | `FP48_16_FromS32` | Checks bit 15 of the source, not its sign, the same quirk as Red. It has no effect here because the values stay within −999 to 999 |

Red's `u24_8` round-up is compared by structure only (medium).

### Clamps

The base damage is clamped to [1.0, 999]. The attack is clamped to 0 to 999 after the item boosts, before the ability modifiers. The defense is not clamped. These match Red (static, P2.6).

### Argument 8 (`arg8`)

`arg8` is the seventh parameter of `CalcDamage`, at stack `S+8`. It is loaded at `0x0217c6e0` (`ldr r1,[r11,#0xb8]`) and stored at dungeon `+0x15C` (`0x0217c6ec`). Red names the field `damageCalcWorkingValues.unk15C` and types it s24.8 (**red-hypothesis**, from `pmd-red`, read-only).

The flow is **blue-verified** (P7):

- The accumulator `V` (48.16, at `r11+0x24`) is multiplied by the type factor `local_94` (`0x0217c6d8`; written by `sub_806E100` at `0x0217c47c`).
- `V` is then multiplied by `arg8`, converted to 48.16 by `FUN_020175f0` (`0x0217c6f0`, `0x0217c6f8`). The conversion moves the s24.8 value's high byte into the high word and shifts the rest into the low word.
- Snapshots: dungeon `+0x154` is `round(V)` before the `arg8` scale (`0x0217c6bc`–`0x0217c6cc`); dungeon `+0x150` is `round(V)` after it (`0x0217c708`–`0x0217c714`).
- The variance from step 16 multiplies `V` next (`0x0217c718`). The final value is written to `dmgStruct`.

Callers, **blue-verified**. All four are unconditional calls:

| Caller | `arg8` passed | `arg_10` | Notes |
|--------|---------------|----------|-------|
| `FUN_0215675c` (`ov_0001::0215675c`) | its own `param_4` | `true` | Hub with about 90 callers. The dispatcher `FUN_02156824` passes `0` (case 0), `0x80` (case `0x163`) and `0x100` (cases `0x150` and `0x16e`) |
| `FUN_021566cc` (`ov_0001::021566cc`) | its own `param_5` | `true` | One caller, `FUN_0215d7a8` |
| `FUN_02161ab4` (`ov_0001::02161ab4`) | `0x200` | `false` | Reached from dispatcher case `0xce` |
| `FUN_02161cc0` (`ov_0001::02161cc0`) | `0x100` | `false` | Reached from dispatcher case `0xcc` |

**Meaning**: a damage scale with 1.0 = `0x100`. Runtime-checked on 14 hits (#185): `0x100` leaves the damage unchanged and `0x80` halves it (moves 154 and 260 pass `0x100`; move 355 passes `0x80`). Other values, and the per-move choice, are not checked against move data.

**Not verified:** the source of the dispatcher's `param_4`; whether anything reads `+0x150`, `+0x154` or `+0x158` (no search done).

## RNG calls in `CalcDamage`

For bit-exact replay the order of RNG calls matters ([`rng.md`](rng.md)). Blue's `CalcDamage` makes three dungeon RNG calls, in this order (static, P2.4). The type function and `CalcStatBoosts` make no RNG call.

1. `DungeonRandInt(100)` at the top of `CalcDamage` (`0217C02C`), before the items. Its result is the **roll** that `CalcStatBoosts` receives as its sixth argument. Huge Power and Pure Power read it (boost only when the roll is 32 or less). The earlier note that this result is unused was wrong. The Ghidra call site shows four arguments because the prototype is not set. The sixth is inferred from the callee, and the stack store was not seen in the disassembly.
2. `DungeonRandInt(100)` for the critical hit (`0217C698`), compared with the odds. It runs after the type function and the screen multipliers, and only when the defender has neither Battle Armor nor Shell Armor.
3. `DungeonRandInt(0x4000)` for the variance (`0217C720`). It always runs.

## Accuracy and evasion

The hit check is `AccuracyCalc`, at `ov_0001::0215A928`. It is the Blue counterpart of Red's `AccuracyCalc` (static, P3a).

- Its first step is `DungeonRandInt(100)`, drawn before every early return. The result is used only at the end, as `roll < effective accuracy`. So one RNG call is consumed on every call, including forced hits and misses ([P1](#p1-hit-check-accuracycalc), [P6](#p6-rng-order-for-a-basic-attack)). Keep this draw order in any golden trace, even when the result is forced.
- The accuracy is the move-data table entry for the move ID at `+0xE + slot` (slot 0 to 2), read by `FUN_0200B3C4` (`0200B3C4`; the table index is move ID × `0x24`, [P1](#p1-hit-check-accuracycalc)). It is not the move instance's byte. A damaging attack calls the check twice, at slot 0 and slot 1.
- Early results, as in Red:
  - Attacker status 1 (sure shot) hits; status 2 (whiffer) misses.
  - Thunder (`0x40`) always hits in rain (weather 4). In sun (weather 1) it lowers the attacker's accuracy stage by 2.
  - Compound Eyes (`0x33`) on the attacker: +2 accuracy stage.
  - Target exposed (flag at info `+0xD2`): evasion stage 10.
  - Sand Veil (`0x1D`) in a sandstorm (weather 2): +2 evasion stage.
  - Hustle (`0x30`) with a physical move: **+2 to the target's evasion stage** (`info+0x26`, static). That lowers the hit chance. See [P5](#p5-static-leftovers).
- Penalties: a target holding the Detect Band (item 35) takes −30. A target with the Quick Dodger IQ skill (IQ bit 5) takes −30. For move `0x163` (Regular Attack), an attacker with IQ bit 4 (Sure Hit) is a sure-hit case.
- Stage tables: accuracy at `ov_0001::021AA8C4` (pointer slot `ov_0001::0215AB2C`) and evasion at `ov_0001::021AA918` (pointer slot `ov_0001::0215AB30`). Both have 21 entries and match Red's `gAccEvsStatStageMultipliers` entry by entry. Blue stores integers where Red's table holds s24.8 values. The stage is clamped to 0 to 20, and the multiplier to a cap of `0x6400`.
- `IqSkillIsEnabled` at `ov_0001::021805B0` tests bit `n` of the info's 24-bit IQ bitfield, at `+0x87` to `+0x89`. It calls the helper `02004250` (`IsIQSkillSet`) through the pointer `PTR_FUN_021805C8` (blue-verified). The bit numbers 4, 5 and 8 match Red's IQ skill indices.
- Callers of the random outcome functions and their chance sources are in [P4](#p4-outcome-wrappers-and-ai-chance-sources). Of the 63 move-side wrapper calls in overlay 1, 19 pass a literal 0 and draw nothing, and 44 draw. The AI chance sources are open.

## Basic attack at runtime (#356)

Runtime evidence in this section comes from investigator a's scratch probes. They are not in the repo. The probe is `rngprobe.py` (sha256 prefix `e4b0c544dc60d5b6`). It hooks exec on `DungeonRandInt` at `0219D058` (the result is read at the return site, and the caller from LR), on `AccuracyCalc` at `0215A928` and on `CalcDamage` at `0217BD94`. It ran on the retail dump with SHA-1 `6f01887356268de39085df01490347bbcc3aa330`, driven by `boot_to_dungeon` and `combat_inputs.txt` (the #185 scenario, in [`verification/v7-trace/`](../../verification/v7-trace/)). Frames count from the first dungeon frame. Run IDs are the `ev.json` sha256 prefixes: base `cf15b0ff`, forced slot-0 miss `25f4c5ac`, forced crit odds `6b21d775`, forced Huge Power `7aa9ed88`, forced Hustle `23907351`. Some runs write memory, and each point says which. Committing the probe to `verification/v7-trace/` is a follow-up.

### P1. Hit check (AccuracyCalc)

- **Draw order.** `AccuracyCalc` (`ov_0001::0215A928`) draws `DungeonRandInt(100)` first, at `0215A950`, before every early return. It then returns `roll < effective accuracy`. **blue-verified** (static).
- **Two checks per damaging attack.** Slot 0 is called from the dispatcher `02156824` (return address `021571B8`). Slot 1 is called from `DamageAccuracyCheck` (`ov_0001::0215652C`, return address `02156550`), which the damage path reaches after `CalcDamage`. A slot-0 miss ends the attack: forced misses showed no `CalcDamage` and no second check. A slot-1 miss comes after `CalcDamage` has drawn its rolls. **blue-verified** (static call sites, and the runtime points below).
- **Slot-1 result.** Result 0 prints the missed message, sets the damage struct `+0xF` to 1 and skips `HandleDealingDamage_Async`. Result 1 calls `HandleDealingDamage_Async`. **blue-verified** (static, medium: an `AbilityIsActive(target, 0x2E)` branch before the call writes `gDungeon+0x652`, not traced).
- **Accuracy source.** `FUN_0200B3C4` (`0200B3C4`) reads the move-data table at `*(u16 *)(move + 2) × 0x24 + table base + 0xE + slot`. Its first argument is a move pointer, and the entry is the move ID's, not the move instance's. **blue-verified** (static).
- **Runtime, hit.** All 29 calls in the 9000-frame base run (`cf15b0ff`) returned a hit, including accuracy values above 100, which return a hit after the draw. Example: frame 1433, move 154, roll 39, hit. **blue-verified** (runtime, scratch probe).
- **Runtime, natural slot-1 miss.** Frame 3411, move 260, slot 1: roll 91 against the move's accuracy value, miss. No patch. Run `6b21d775` changes only the crit argument, not this path. **blue-verified** (runtime).
- **Runtime, forced slot-0 miss.** The move's accuracy byte in the move-data table was set to 1 at `AccuracyCalc` entry (a 32-bit memory write; a byte write did not stick). 101 calls in run `25f4c5ac`. Frame 1433, roll 39, miss, and no `CalcDamage` after it. **blue-verified** (runtime, patched memory).
- **Not seen.** A miss through a lowered accuracy stage or a raised evasion stage, and the sure-shot and whiffer early returns. **open**.

### P2. Critical hit roll

- **Position.** `DungeonRandInt(100)` at `0217C698` is drawn after the first roll (`0217C02C`) and before the variance roll (`0217C720`). It was drawn in every `CalcDamage` the probes ran, including attacks with natural crit odds 0 (move 355, roll 41, no crit). **blue-verified** (runtime). The Battle and Shell Armor gate above is still static.
- **Flag.** A crit sets byte `+0xD` of the damage struct to `01`. Run `6b21d775` forced the odds argument (the stack argument at `sp+0`) to 999 at `CalcDamage` entry: 7 crits in 12 hits. **blue-verified** (runtime, patched argument).
- **Multiplier.** Frame 1528, move 355, same rolls in both runs (38, 41, 4326): damage 7 without a crit and 10 with one, consistent with ×1.5. The exact multiplier was not recomputed from the 48.16 value; only integer damage was compared. **blue-verified** (runtime; medium on the exact value).
- **Natural odds.** The natural odds argument was 12 for moves 154 and 260, and 0 for move 355. The two natural-odds attacks in the base run rolled 48 and 56 and did not crit. A natural crit has not been seen firing. **open**.

### P3. Huge Power and Hustle

Each ability was forced by writing its ID to the attacker's ability byte `info+0x58` (the first of the two bytes that `AbilityIsActive` tests) at `CalcDamage` entry (`0217BD94`). The RNG is the same as in the base run.

| Frame | Move | Roll at `0217C02C` | Damage, base | Damage, Huge Power |
|-------|------|--------------------|--------------|--------------------|
| 1437 | 154 | 14 | 4 | 6 |
| 1528 | 355 | 38 | 7 | 7 |
| about 1557 | 355 | 4 | 1 | 2 |
| about 1650 | 355 | 37 | 8 | 8 |
| 3051 | 260 | 64 | 18 | 18 |

- **Gate.** Huge Power boosts only when the roll is 32 or less. This matches the comparison in `CalcStatBoosts` (`0217C7F8`, where `0x20 < param_6` is the cut). Rolls 14 and 4 boosted; 38, 37 and 64 did not. **blue-verified** (static comparison; the runtime rows agree).
- **Hustle.** Forced the same way (run `23907351`). Move 154 went from 4 to 6, the ×3/2 on a physical move. On move 355 the integer damage stayed 7 at frame 1528, too small to tell. **blue-verified** (static `CalcStatBoosts` row above; the runtime value is for one move only, medium).

### P4. Outcome wrappers and AI chance sources

- **Move callers.** `DungeonRandOutcome` (`ov_0001::0219CFCC`) is reached only through the wrappers `ov_0001::02159F10` and `ov_0001::02159F6C`. Each returns true at once when its chance argument is 0, with no draw: `02159F10` tests its second argument and `02159F6C` its third. The overlay 1 export has 63 call sites: 14 to `02159F10` and 49 to `02159F6C`. Of these, 19 pass the literal 0 (13 and 6), so they draw nothing. Examples: dispatcher `02156824` (`02159F10(param_2, 0)`, a validity test), `0215C44C`, `0215E298` and `021629D0`. The other 44 pass a constant read from the overlay: 1 to `02159F10` and 43 to `02159F6C`. For example, `0215DD2C` (Thundershock) passes `(int)*(short *)PTR_DAT_ov_0001__021aa0a4`. For these, the wrapper's entity checks run first, then one `DungeonRandInt(100)` through `DungeonRandOutcome` when they pass (the secondary gate in [move-effects.md](move-effects.md#miss-paths-357-p2)). So the chance doubling for ability `0x26` and the ability `0x4C` hook are reachable through these 44 calls. A basic attack (`0x163`) calls `HandleDamagingMove` directly, with no wrapper, so [P6](#p6-rng-order-for-a-basic-attack) does not change. **blue-verified** (static: a multi-line count of the `ov_0001` export).
- **AI chance sources.** `DungeonRandOutcome_2` (`ov_0001::0219CFF0`) is called with these chances. Call sites are **blue-verified** (static):
  - `02185568`: chance from `FUN_021473A8(entity, param, 1)`.
  - `0218579C`, call at `02185820`: chance from `FUN_021473A8(param_1, item, 2)`.
  - `02185AB8` and `02185BEC`: chance from the table at `02185C60`, index `i` below a count read at `021B3F44`. The count was 0 at dungeon start, so the table was not read.
  - `02192D24`, call at `02192D80`: chance is the value at `021AA184`, which is **70** at runtime (run `cf15b0ff`). The call is gated by `info+0xAC == 2`. The value is **blue-verified** (runtime, scratch probe). That the roll is the confusion roll of `ChooseAIMove` is a **hypothesis**, which the findings do not show. Test: an exec hook on `02192D80` in a turn where the monster has `info+0xAC == 2`, comparing the roll with the move chosen. The meaning of `info+0xAC` is open.
- **Open.** `FUN_021473A8` beyond its first cases, the values at `02185C60`, and the meaning of `info+0xAC`. These are leads for a later trace and are not traced here. **open**.

### P5. Static leftovers

- **Weather accessor** `FUN_02195C48`, named `GetEffectiveWeather` in [`symbols.md`](../symbols.md). It returns the weather byte at `gDungeon+0xC104`, or 0 when the entity is non-null and `HasHeldItem(entity, 45)` holds. A null entity gives the raw weather. The name of item 45 is open. **blue-verified** (export body, not run).
- **IQ bit helper** `02004250` (`IsIQSkillSet`, in [`symbols.md`](../symbols.md)), reached through `PTR_FUN_021805C8` (read at runtime). It tests `(b0 & m) | (b1 & (m >> 8)) | (b2 & (m >> 16)) != 0` over the three IQ bytes. **blue-verified**.
- **Physical-move table** `FUN_02008C64` (`IsTypePhysical`, the name from `pmd-red`). It returns the byte at `DAT_02008C70 + type`. The table is at `0x02091E78`, 18 bytes, read at runtime. Its per-type values are not copied here. The runtime values make Regular Attack (type 0) and Normal (type 1) physical. **blue-verified** for the role; the name is Red's.
- **Hustle, target evasion.** With `AbilityIsActive(attacker, 0x30)` and a physical move, the target's evasion stage at `info+0x26` rises by 2, clamped to 0 to 20 (static; the offset is investigator a's read). The evasion table at `021AA918` (runtime read) then lowers the hit chance. In one forced run (`23907351`), the attack the base run hit, with the same roll, missed. That is one case: medium. This +2 is not the attacker's stage. Hustle's attack ×3/2 in `CalcStatBoosts` is a separate multiplier ([P3](#p3-huge-power-and-hustle)). **blue-verified** (static); runtime, one case.
- **Other penalties.** The Detect Band penalty (`021AA39C`) and the Quick Dodger penalty (`021AA290`) were read at runtime and match the values above. **blue-verified**.

### P6. RNG order for a basic attack

These are the dungeon-RNG draws of one basic attack, in order. Each address is the **return site** of its `DungeonRandInt` call; the call instruction is 4 bytes earlier (for the `AccuracyCalc` draw, the call is at `0215A94C`). Observed on move 355 (Regular Attack, ID `0x163`), 154 and 260 (base run `cf15b0ff`, and the crit path in `6b21d775`). **blue-verified** (static call sites, runtime sequence).

- **Hit, or a slot-1 miss: five draws.**
  1. `rand(100)` at `0215A950`: `AccuracyCalc` slot 0, return address `021571B8`.
  2. `rand(100)` at `0217C02C`: `CalcDamage`, the Huge Power roll.
  3. `rand(100)` at `0217C698`: the critical roll.
  4. `rand(0x4000)` at `0217C720`: the variance.
  5. `rand(100)` at `0215A950`: `AccuracyCalc` slot 1, return address `02156550`.

  Draw 5 decides a slot-1 miss. A hit and a slot-1 miss draw the same five values; only the last comparison differs.
- **Slot-0 miss: one draw.** `rand(100)` at `0215A950`, then the attack ends. No `CalcDamage`.
- **Critical hit.** The same five draws as a hit. Only the critical comparison differs.
- **Non-damaging moves** (only 217 and 135 were seen): only the slot-0 draw. Other non-damaging handlers draw more: Hypnosis calls `CalculateStatusTurns`, and Leech Seed and Water Sport call `DungeonRandRange(10, 12)` ([move-effects.md](move-effects.md)).
- **Not covered.** Targets with Battle or Shell Armor (the static gate), Thunder and weather, sure-shot, and kills. After the hits, extra draws were seen in the crit run only and are not attributed: `rand(10000)` at `021388E8`, `rand(56)` at `0219C038`, `rand(32)` at `0219C044`, `rand(112)` at `0219C1E4`, `rand(100)` at `021828A4`, `rand(2)` and `rand(3)` at `02190A10`, and `rand(100)` at `02179DF0`. **open**.

Anything that simulates a basic attack needs this five-draw order.

## Range and targeting

A move's targets are picked by two resolvers. They read the same move fields but are different code, so each has its own tables below. Static evidence only, from the Blue ARM9 and overlay 1 text export and the xrefs in investigator B's findings on [#355](https://github.com/TorratDev/rust-mm/issues/355). No emulator run. Tag **blue-verified** unless a line says otherwise.

- **Executor path** (applies a move the user uses): `ov_0001::0215A520` (`GetTargetsForMove`, Red's name) builds the candidate list. `ov_0001::0215A3A8` (`FilterMoveTarget`, our name) keeps or drops each candidate. The move dispatch path calls `0215A520` (see [move effects](move-effects.md)). The line area is walked by `ov_0001::0215AB34`.
- **AI path** (picks a target for a monster): `ov_0001::02192728` and `ov_0001::02192508` build the list. `ov_0001::02192274` (`IsAITargetEligible`) filters it.
- **Targeting value.** `ov_0001::02191A30(user, move, i)` returns `targeting[i]`, except that move `0xED` with `i == 0` returns `0x73` unless the attacker is type `0x0E`. The executor uses `i = 0`, which is the `u16` at move `+0x8`. The AI uses `i = 1`, which is the `u16` at move `+0xA`. The low nibble is the target class and the high nibble is the area.

### Helpers

Tags are for the role read in the export. The names are ours unless marked.

| Address | Name | Role | Tag |
|---------|------|------|-----|
| `ov_0001::02180FFC` | `CanMoveOrAttackInDirection` | Direction test from a tile. The neighbour in `dir` must not have flag `0x10` (wall). An entity there must be type 1 (monster), else false. A per-tile "direction open" bit is then tested, to block corner cutting. The byte it reads is chosen by a terrain-class index: default 2, minimum 2 from `0218267C`, 3 for some info, held-item and IQ-bit cases. Callers: `02156404`, `02177688`, `02191F84`, `02192728` (2 sites). | blue-verified |
| `ov_0001::02192618` | `IsTargetInLine` | AI line test `(user, target, limit)`. True when the Chebyshev distance is under 11 and no more than `limit`, and the offset lies on one of the 8 directions (axis or exact diagonal). Returns a bool, not a direction. Caller: `02192728` only. | blue-verified |
| `ov_0001::02191E70` | `IsProjectilePathClear` | Projectile path test `(user, target, dir, maxdist)`. The distance is clamped to `maxdist` first. If IQ bit 3 is clear, the function returns true, because the clamp makes the test always pass. If set, it walks from the user in `dir` for up to dist + 1 steps: false when it leaves the map (x 1 to 0x36, y 1 to 0x1E) or meets a tile with `flags & 3 == 0`, true on reaching the target, false on any other entity. Callers: `021853CC`, `02192728`. | blue-verified; IQ bit 3 name open |
| `ov_0001::02141C08` | `IsInSameRoomAI` | AI relation `(a, b)`. False if b's `+0x20` byte is 0. For two monsters, false when the `+0xC7` and `+0xC9` flags exclude b. Otherwise `0219C6BC(a+4, b+4)`: the same room, or within 2 tiles in a corridor (the limit is `gDungeon+0x15FA1`, or 2 if that is 0). 15 callers. The executor's room test `02141AF0` is the same without the two flag exclusions. | blue-verified |
| `ov_0001::021818A4` | `IsHpBelowQuarter` | True when HP (`info+0xE`) is at or below `0208625C(max HP, 4)`. Callers: `021644B8`, `02192274` (2 sites). | blue-verified; the "quarter" depends on `0208625C`, not decompiled |
| `ov_0001::02181904` | `HasAnyStatus` | True when any of a fixed set of info fields holds its listed value: `+0x99`, `+0x9B`, `+0xA0`, `+0xAC`, `+0xB3`, `+0xB8`, `+0xC4`, `+0xC9`, `+0xCB`, `+0xD2`, `+0xDA`, a six-byte entry block at `+0xF4`, and five bytes at `+0xE9` to `+0xED`. 8 callers. | blue-verified; the meanings of `+0x99` and `+0xA0` are medium confidence |
| `ov_0001::021818D0` | `IsSleepLike` | True when `info+0x99` is 1, 3 or 5. 11 callers. | blue-verified |
| `ov_0001::02180994` | `GetTreatmentBetweenMonsters` | Returns 0 for an ally or the same entity, 1 for a foe, 2 to ignore. The name is Red's (red-hypothesis); the return values are read in Blue. Used by `0215A3A8` and `02192274`. | blue-verified |

### Executor path (`0215A520`, `0215A3A8`)

**Target class**, in `0215A3A8`. For every class, a candidate is rejected when its entity has `+0x8 == 1`, or `+0x98` of 1 or 4. A move `0xFC` aimed at the user is also rejected. After that:

- `0` and `4`: a foe (relation 1).
- `1`: an ally or the user (relation 0). **The user is in class 1.** `02180994(user, user)` returns 0 for the same entity, so the self case passes.
- `2` and `3`: any candidate that passes the common rejects.
- `5`: any candidate except the user.
- `6`: an ally, not the user.
- `7` and up: false.
- Argument 7 (`skipRelation`): when set, every class passes the relation test. The caller sets it when the user's `+0xAC` is 2 or 4, or `+0xC9 == 1`. The meaning of `+0xAC` is open.
- A veto from `0215A054` can drop a target by its `+0xDC` value. The veto set depends on the move and on `+0xAE`. The field names are open.

**Class 4 is decided before filtering**, in `0215A520`. If `0215A2C0(user, move)` returns 0, the targeting becomes `0x73` (class 3, self). Otherwise it becomes `0` (area `0x00`, class 0: a foe on the facing tile). So executor class 4 is a conditional self. `0215A2C0` is not decompiled.

### AI path (`02192274`, `02192728`)

**Target class**, in `02192274`:

- `0`: a foe (relation 1).
- `1`: an ally or the user (relation 0). Whether the user is a candidate depends on the area (below).
- `2`: any candidate, excluding `+0x8 == 1` and `+0x98` of 1 or 4.
- `3` and `4`: always pass.
- `5`: any candidate except the user, with the same excludes as `2`.
- `6`: an ally, not the user.
- `7` to `15`: false.

### Area (high nibble)

| Nibble | Executor (`0215A520`) | AI (`02192728`, `02192508`) | Red name (hypothesis) |
|--------|-----------------------|-----------------------------|-----------------------|
| `0x00` | facing tile, only if `02180FFC(user, facing)` passes; the type-1 entity there | 8 neighbours, each through `02180FFC` | `TARGET_OTHER` |
| `0x10` | a frontal cone of 3 tiles: facing −1, facing, facing +1 (wrapping); type-1 entities | 8 neighbours, no gate | `FRONTAL_CONE` |
| `0x20` | 8 neighbours; type-1 entities | 8 neighbours, no gate | `AROUND` |
| `0x30` | the 20 slots at `gDungeon+0x11364` in the same room or area (`02141AF0`); any valid entity, the user included | the 20 slots, relation from `02141C08` | `ROOM` |
| `0x40` | facing tile (`021775F0`, rejected if `021820B0` says blocked); if no entity there, the tile two ahead (`021774FC`) | 8 offsets at twice the distance, each through `02180FFC` | `2_TILES_AHEAD` |
| `0x50` | the line walked by `0215AB34`: up to 10 tiles in the facing direction, stopping at a wall; the first entity hit | the line test `02192618` (distance 10), then the path test `02191E70` | `LINE` |
| `0x60` | all 20 slots; every valid entity, the user included | all 20 valid slots | `FLOOR` |
| `0x70` | the user only | the user only | `SELF` |
| `0x80` | facing tile, with the corner-cut check (`021775F0`); the type-1 entity there | distance 1 through `02192618` (limit 1) | `CUT_CORNERS` |

The user can be a candidate only in areas `0x30`, `0x60` and `0x70`, the areas that reach the user's own entity. The other areas touch neighbour tiles or a line, which never hold the user. This was checked on the executor path. On the AI path the class-1 relation test is the same, but the self case through `02141C08` was not checked separately.

**Where the two paths differ**

- `0x10`: a three-tile cone on the executor path, eight neighbours on the AI path, so the AI is wider. `0x20` is eight neighbours on both paths. Red's `FRONTAL_CONE` and `AROUND` match the executor.
- `0x00`, `0x40`, `0x50` and `0x80`: the tile rules above differ.
- `0x30`: the executor uses `02141AF0`, and the AI uses `02141C08`, which also excludes monsters through `+0xC7` and `+0xC9`.
- Class 4: a conditional self or foe on the executor path, always passing on the AI path.

### AI condition (`0xF00` bits, read in `02192274`)

- `0x100`: random. The chance is the move entry byte at `+0x10`, which is `accuracy[2]`. `FUN_0200B3C4(move, i)` reads the byte at the move entry plus `0xE` plus `i`, with entries `0x24` bytes apart, so `i = 2` gives `+0x10`. The chance is compared with `DungeonRandInt(100)`. `+0x13` is crit chance, a different byte. The port reads this as `accuracy[2]` in `crates/data/src/moves.rs`. Red's name `ACCURACY_AI_CONDITION_RANDOM_CHANCE` is a red-hypothesis.
- `0x200`: HP at or below 25% (`021818A4`).
- `0x300`: has a status (`02181904`).
- `0x400`: asleep (`021818D0`).
- `0x500`: HP at or below 25% or has a status.
- `0x600`: ghost target (type `0x0E`) with `+0xD2 == 0`.
- The condition is checked only when the attacker has the Status Checker IQ skill (IQ bit 8, through `IqSkillIsEnabled`). Without it, only `0x100` applies.

### Candidate selection (P1, #186)

Investigator A's findings for the area classes. Tag: **blue-verified** (decompile of `ov_0001::02192728`, `ov_0001::02192508` and the helpers below, Ghidra copy `rescue-arm9-a`).

- **Class.** `uVar10 = FUN_02191A30(user, move, 1)` (that is `FUN_0200B4EC(move, 1)`, or `0x73` for move `0xED` when the user is not type `0x0E`). The class is `uVar10 & 0xF0`. `FUN_0200B4EC` was not decompiled, so where the class bits come from is not verified.
- **Eight adjacent tiles.** Classes `0x00`, `0x10` and `0x20` use the 8 adjacent tiles, each needing a type-1 entity. The condition is `uVar18 == 0x10 || uVar18 == 0x20 || gate`, so `0x10` and `0x20` need no direction gate. `0x00` uses the gate `FUN_02180FFC(user, dir)`.
- **Other classes.** `0x30`: 20 slots at `gDungeon + 0x11364`, kept when `FUN_02141C08(user, e) != 0`. `0x40`: 8 offsets at twice the distance, with the direction gate. `0x50` and `0x80`: all 20 slots; `FUN_02192618(user, target, limit)` keeps targets within Chebyshev distance `limit` (10 for `0x50`, 1 for `0x80`) and a direction; then `FUN_02191E70` checks the path. `0x60`: all 20 valid slots. `0x70`: the user only.
- **Single candidate.** If `user.info + 0xC9 == 1`, there is one candidate: the tile in the user's facing direction (`info + 0x44`), with no entity (pointer 0) and weight 99. This applies only for classes `0x00`, `0x10` and `0x20`, because the `info+0xC9 == 1` check sits inside that branch of `ov_0001::02192728`.
- **Choice.** Candidates are picked by a weighted `DungeonRandInt(sum of weights)`. The weights come from `FUN_021921B0`.
- **Target filter.** `FUN_02192274(flags, user, target, ...)`: the low nibble of `flags` is a relation filter (`0`: `FUN_02180994 == 1`; `1`: `== 0`; `2` and `5`: refuse when the target's `info+8` or `info+0x98` is set; `6`: not self and `== 0`; `3` and `4`: always pass). The `0xF00` nibble adds the AI checks listed above (`0x100` through `0x600`, using `FUN_0200B3C4`, `FUN_021818A4`, `FUN_02181904`, `FUN_021818D0` and a type check on `info +0x56`/`+0x57` against `0x0E`).
- **Direction.** `FUN_02192508(count, flags, user, target, ...)`: the direction is `info+0x44` when user and target share a tile, or when the class is `0x30`, `0x60` or `0x70`. Otherwise it is `FUN_0201187C(user, target)`. It adds a candidate when `FUN_02192274` passes and the slot is empty.

**Confidence:** high for the branch table. Medium for the names given to `info+0xC9` (player control, from its use as a gate in `FUN_02141C08`), to the `0x30` slot list, and to the `0x0E` type check.

**Not verified**

- `FUN_0200B4EC` (the class bits source).
- Whether the classes `0x10` and `0x20` differ outside these two helpers: they do on the executor path (see [Range and targeting](#range-and-targeting)). The AI path was not re-checked for callers.
- The meaning of `info+0xC9`.
- Runtime behaviour: no trace.

## Move effects

The move-effect dispatch is `ov_0001::02156824`. It switches on the move ID at `move+2` and calls one handler per case. The full catalogue of 413 IDs and 262 handler addresses is in [`move-effects.md`](move-effects.md). Handler names there are Red names, mapped by move ID, and are hypotheses until each handler is read.

### Dispatch shape and hit/miss branch (P2, #186)

Investigator A's findings. Tag: **blue-verified** (decompile of `ov_0001::02156824`, Ghidra copy `rescue-arm9-a`). Confidence: **medium**. The structure is high confidence; the meanings of the fields named below are not.

- `FUN_02156824(param_1 = entity list, param_2 = user, param_3 = move, param_4, param_5)` loops over up to `0x40` target slots. Its only `switch` is `switch(uVar17)`, where `uVar17` is the move ID (the `u16` at `move + 2`). It has 413 case labels and the default label `switchD_ov_0001__02157544_default`. Several cases are `goto switchD_..._default`. Those gotos are the only other occurrences of the switch label, so the "8 other switches" may be those.
- Each case sets `local_114`, the per-target result. The case callees take `(user, target, move, param_4)`. Most return a bool. The case for `CalcDamage` (`FUN_0215675C`) calls `CalcDamage` directly.
- The dispatch makes one accuracy call, earlier in `02156824` (export line 246, before the switch): slot 0 through `AccuracyCalc`, return address `021571B8` ([P1](#p1-hit-check-accuracycalc)). Rolls inside the case callees go through the wrappers in [`move-effects.md`](move-effects.md#roll-wrapper-per-move-id-p3-186).
- Earlier in the loop, `info + 0x12A` is set to 1 for the target, and `info + 0xD8` is set to 1 or 2 depending on `param_5`.
- After the switch:
  - **Miss** (`local_114 == 0`): if the user and target are both valid and `target.info + 6 != 0` (not a team member), the saved value `cVar1` is written back to `target.info + 0xD8`.
  - **Hit**: if `FUN_0215A344(move) != 0` and `FUN_02159F10(user, 0) != 0`, it sets `*DAT_02159AE4 = 1`. `FUN_02159F10(user, 0)` returns true whenever the user is valid, so it is a validity check.

**Not verified**

- The meaning of `DAT_02159AE4`, `info+0xD8`, `info+0x12A`.
- What `FUN_0215A344` checks.
- Whether the "8 other switches" in the issue are the `goto switchD_..._default` jumps or something else.
- Which case callees return a miss (only the flag shape was read, not each callee).
- Runtime behaviour: no trace.

## Defeat and experience

Static, unless marked "runtime". The damage path and faint handler match Red by structure and parameter order. The field offsets are in [`structs.md`](structs.md).

- **Damage path.** `HandleDealingDamage_Async` (`ov_0001::0217E890`) calls the per-hit function `ov_0001::0217D5FC`, which is `HandleDealingDamageInternal_Async` in Red by structure. The per-hit function is entered from `ov_0001::0217E8C0` and `ov_0001::0217EB54`.
- **Faint.** `HandleFaint_Async` is `ov_0001::021778F4`, by structure (the same parameter order, exit-reason guard, team-leader and cutscene branches). Callers: `ov_0001::0217E278`, `ov_0001::0217E850`, `ov_0001::0217E864`, `ov_0001::0217E878` (damage path) and `ov_0001::0217F4E0` (join path).
- **KO reason.** When no item is involved, the exit reason is the ID of the move that dealt the KO, as in Red (`GetDungeonExitReasonFromMoveOrItemID`). Runtime: the reasons `0x9A`, `0x163` and `0x104` were seen. Item KOs and Wonder Orb KOs use the 500-range reasons (`DUNGEON_EXIT_REASON_START = 500`). These were not observed.
- **EXP formula.** `CalculateEXPGain` is ARM9 `020020F8`. The formula is the base yield plus the yield × (level − 1) / 10. The yield is read from the species table, which has stride `0x48` and the yield at offset `0x20`. The formula matches Red (static).
- **EXP multiplier.** The caller applies the multiplier, not `AddExpPoints`. The target's multiplier at info `+0xD8` gives: `0` halves the amount, `2` multiplies it by 3/2, and `1` leaves it unchanged. A result of 0 becomes 1. Tag: **blue-verified**, from the decompile of `ov_0001::0217D5FC` (about lines 490–500). `CalculateEXPGain` (`020020F8`) is called from `ov_0001::0217E624` and `ov_0001::0217AB88`. `AddExpPoints` (`ov_0001::02183F78`) is called from `ov_0001::0217DD20`, `ov_0001::0217E748` and `ov_0001::0217E788`. Confidence: high.
  - **Runtime** (blue-verified, runtime): 21 awards in 3,000 scripted frames on the dump with SHA-1 `6f01887356268de39085df01490347bbcc3aa330`. Multiplier `0` gave amount 7 and multiplier `1` gave amount 15, both for species 290 at level 1. Multiplier `2` was not observed. Probe: an exec hook on `0217D5FC` entry (target from `r1`), `020020F8` and `02183F78`; scratch files, not in the repo.
  - **Writer of `2`** (red-hypothesis for Red; Blue not verified). Confidence: low. In Red, `UseMoveAgainstTargets` sets boosted when `isLinkedMove` is true, and `TryUseChosenMove` computes `isLinkedMove = (j > 1)` (`src/dungeon_move.c` 305–310, `src/dungeon_move_util.c`). Blue's equivalent is not identified. `HandleDamagingMove` (`ov_0001::0215675C`) has many callers, so it has not been checked as the writer.
  - **Not verified:** Blue's writer of `2`, and whether a linked-move scenario reaches it. The `+0xD8` offset against a Blue struct (depends on [#46](https://github.com/TorratDev/rust-mm/issues/46)). Runtime value `2` (never observed).
  - **Suggested next step:** find Blue's move-use loop and the write of `2` to `info+0xD8`, then script a move-chain scenario to observe `2` at runtime.
- **Award.** `AddExpPoints` is `ov_0001::02183F78`, with arguments `(_, recipient, amount)`. It returns at once when the recipient's level (info `+0x09`) is 100 (`0x64`, disassembly `0x02183F7C`–`0x02183F84`), and when the join-location byte (info `+0x40`) is `'J'` (`0x4A`) or `'G'` (`0x47`, `0x02183F88`–`0x02183FA8`). That lock check replaces Red's `IsExperienceLocked`. Otherwise the delta is `min(exp + amount, CAP) - exp`, where `CAP` is the literal at `0x02183FE8`, which is **9,999,999** (read at runtime). The delta is added to info `+0x1C8` (EXP gained this turn), not to `+0x18`, and it sets `gDungeon` `+0xD` to 1 when the delta is nonzero. The EXP total at `+0x18` changes outside this function (runtime: the attacker's EXP changed across two `add_exp` events without a store here). It is called once per recipient: the attacker and each team member. Tag: blue-verified (the cap value is confirmed at runtime; the logic is static). Confidence: high for the cap value and the code path; medium that the lock behaves as read.
- **Recruit and join.** `ov_0001::0217F9B8` (two arguments; the name matches Red's `TryRecruitMonster`) and `ov_0001::0217F388` (`MonsterJoinSequence_Async`). If `TryRecruitMonster` returns 0, the target takes the normal faint path (`0x0217E858`), with the KO reason read from `[sp,#0x74]`. It does not use reason 501. If it returns nonzero, `MonsterJoinSequence_Async` is called (`0x0217E818`–`0x0217E858`). If the join returns 0, the target faints with reason 501 (`0x0217E844`). That reason is the literal at `0x0217E88C`, which is 501 (`0x1F5`), and its name in pret/pmd-red `include/constants/dungeon_exit.h` is `DUNGEON_EXIT_LEFT_WITHOUT_BEING_BEFRIENDED` (red-hypothesis for the name). If the join returns nonzero, the byte at the pointer in the literal `0x0217E888` (which points to `0x021B473C` at runtime) is set to 1, and the target does not faint. Tag: blue-verified for control flow and value. Confidence: high for the flow and the value; the join path was not observed. Runtime: the `recruit` event on frame 687 returned 0, and the faint on that frame had reason 355 (`0x163`).
- **Revive.** `ov_0001::0217D5FC` has two blocks that restore a target. Each sets HP to the maximum HP. The first block (orb-like) returns 0, which means no faint. The second (seed-like) falls through, as in Red's Reviver Seed. Not observed. Static, medium confidence; the field names come from their position in Red's struct.
- **Item drop on KO.** The drop is in the damage handler `ov_0001::0217D5FC`, not in `HandleFaint_Async`: at `0x0217E5A4`–`0x0217E5E4`, after the Reviver Seed block and before the EXP award. Condition: `isTeamLeader` (info `+0x07`) is 0 and the held item (info `+0x5A`, flags bit 1) exists. Action: call `0x02143BF8(target, &target+0x4, &heldItem)`. It builds an `ENTITY_ITEM` (type 3) at the target's position and calls `0x02143918`, which leads to `SpawnItem` (`0x021440B0`) and `SpawnItemEntity` (`0x02141F58`). Then the 3-byte held item at `+0x5A`..`+0x5C` is zeroed. This matches the block `if (!isTeamLeader && ItemExists(heldItem)) SpawnDroppedItemWrapper(...); ZeroOutItem(...)` in pret/pmd-red `src/dungeon_damage.c` (red-hypothesis for the equivalence). Tag: blue-verified for the location and condition (static). Not observed at runtime: no KO'd target held an item in the 9,000-frame run.
- **Open discrepancy.** One faint's return address (`0x0217E868`) lies inside the range that Ghidra gives `ov_0001::0217D5FC`, which is 4,612 bytes long. A damage entry at frame 621 and a faint at frame 687 are 66 frames apart. The timing suggests that the faint belongs to the same invocation. This is an inference from timing, not a checked fact.

## Damaging moves in the sim (#419)

The sim runs a leader's damaging move (`crates/sim/src/combat.rs`). The findings P1 to P8 (numbered as #419 numbers them, not the P1 to P6 of the sections above) come from two investigators' comments on [#419](https://github.com/TorratDev/rust-mm/issues/419): investigator a for P1, P2, P7 and P8, investigator b for P3 to P6. Their static reads were checked against the Ghidra text export (STAMP 2026-11-12), and their runtime probes ran on the dump. The probes are scratch files, not in the repo. The tables are not copied here ([`legal.md`](../legal.md)): `rescue_data::combat` reads them from the user's dump at run time, and this page gives addresses, shapes and the rules. Tags are the same as above.

### P1. Type factor and label (`sub_806E100`, `0217CE88`)

- **Start and cells.** The factor starts at 1.0 (48.16). For each of the defender's two types (info `+0x56`, `+0x57`), the cell is `gTypeEffectivenessChart[move type][defender type]` (`ov_0001::021AAEE8`, [above](#type-effectiveness)). A Ghost defender (type 14) takes cell 0 against a Normal (1) or Fighting (7) move, unless it is exposed (flag `+0xD2`). Cell 2 is neutral and is not multiplied; cells 0, 1 and 3 multiply the factor by their pool entry (immune class x0.5, not very effective x0.9, super effective x1.5). **blue-verified** (decompile of `0217CE88`; investigator a's 26 runtime calls on 13 type pairs, including dual types).
- **Pool.** The four pool entries are pointers in the array at `ov_0001::021AE248`. Entry `c` points to an 8-byte object whose second word is the 16.16 multiplier. The object for STAB, and for a critical hit, is `ov_0001::021AE238` (1.5). **blue-verified** (decompile, `0217CE88` and `0217BD94`).
- **Label.** `gEffectivenessChart` (`ov_0001::021AA588`, 4 by 4 `s32`) is indexed `[cell of the first type][cell of the second]`. The sim uses only the label that means super effective (3). **blue-verified**; the other labels are not read by this code.
- **Order.** The chart loop runs first. STAB comes after it, if the attacker has the move's type and the factor is not zero. Weather, charging and Wonder Guard are after it; the sim has none of them (see the shortcuts below). **blue-verified** (decompile).
- **Sim.** `combat::type_factor`. A hand-made table gives a super pair, a dual pair (cells multiply), the Ghost exception and STAB.

### P2. Physical or special (`FUN_02008C64`)

- **Table.** `FUN_02008C64(type)` returns the byte at `DAT_02008C70 + type`, and `DAT_02008C70` holds `0x02091E78`. It is 18 bytes, one per type 0 to 17, in the ARM9 RAM image. That image is BLZ-compressed in the ROM, and `0x02091E78` lies past the bytes stored for the binary, so the table is read from the decompressed image. **blue-verified** (live read by investigator a). The ROM test reads them from the decompressed image and checks that each is 0 or 1.
- **Meaning.** A nonzero byte is a physical type (split index 0: attack and defence). A zero byte is a special type (split index 1). The sim's split is `byte == 0`. **blue-verified** (static callers `CalcDamage` at `0217BDC8`, `AccuracyCalc` at `0215AAC4`, `0217C824`, and the hit handler at `0217E984`, `0217EA44`, `0217EBBC`).
- **Sim.** `rescue_data::combat::CombatTables::physical`. Normal (type 1) is physical; the ROM test checks it.

### P3. PP: mark on use, take it at the end of the action

- **Check.** The use loop `FUN_ov_0001__0215BE14` reads the move at `info + 0xF4 + 6 * slot`. A slot whose PP is 0 is refused ("no PP") unless the move-ID table at `021AD0B8` matches. Otherwise it calls the per-use function `0215B384`, which runs the dispatcher and so the hit checks. **blue-verified** (static, and the monster path at runtime).
- **Mark.** When `0215B384` returns non-zero (any completed use, a hit or a miss), the loop sets `move[1] |= 0x08`. **blue-verified** (static).
- **Apply.** `FUN_ov_0001__02159B18` runs at the end of the action. For a marked move it clears the mark and takes 1 PP (2 with the `0x10` mark, below). It draws nothing. **blue-verified** (static, and runtime: PP goes down by one per use, including 11 forced slot-0 misses in run `o_miss`).
- **Not seen, or not modelled.** The `0x10` mark (Pressure, ability `0x39` on the target, [hypothesis] name) takes 2 PP; the bit `0x04` clears the mark without a take (its writer is not traced). **open**.
- **Regular Attack** (move `0x163`) takes no PP and never enters `0215BE14`: see [P8](#p8-regular-attack-action-0x32). **blue-verified** (static and runtime).
- **Sim.** `MoveSlot` (6 bytes: `flags`, `flags2`, `id`, `pp`, `ginseng`) in `EntityInfo::moves`; `strike` sets and clears the mark, and takes 1 PP after the hit checks.

### P4. Hit check (`AccuracyCalc`, `0215A928`)

- **Draw.** `DungeonRandInt(100)` is the first draw, before every early return. **blue-verified** (static; [P1](#p1-hit-check-accuracycalc)).
- **Order.** Early hits: an accuracy above 100 (checked before any stage is read); the sure-shot and whiffer statuses; the same-entity case. Then the accuracy is reduced by the Detect Band (item 35) and the Quick Dodger IQ (bit 5), which the sim does not model.
- **Attacker stage.** The attacker's accuracy stage is `info + 0x24` (s16), `+2` when the attacker has Compound Eyes (`0x33`) (static, the export of `0215A928`; not seen at runtime), then clamped to 0 to 20. The accuracy becomes `acc * ACC[stage] / 256` (truncated), where `ACC` is the 21-entry integer table at `ov_0001::021AA8C4` (read through the pointer slot `ov_0001::0215AB2C`), each entry clamped to 0 to `0x6400`.
- **Target stage.** The target's evasion stage is `info + 0x26`, clamped to 0 to 20, `+2` when the attacker has Hustle (`0x30`) and the move is physical (its attack ×3/2 is in the sim too). The accuracy becomes `acc * EVA[stage] / 256` (truncated), `EVA` the 21-entry table at `ov_0001::021AA918` (pointer slot `ov_0001::0215AB30`).
- **Hit.** `roll < accuracy`. Neutral stage is index 10 (256 in both tables), so a neutral pair keeps the accuracy. **blue-verified** (formula from the decompile; 36 runtime calls matched it, with a lowered and a raised stage among them, run `o_acc`).
- **Sim.** `combat::hits` (with `hits`'s threshold). The two truncations are separate.

### P5. Critical hit odds

- **Source.** The odds are the move data's byte `+0x13`, `Move::crit_chance`, passed to `CalcDamage` as `critChance` by its four callers (`021566CC`, `0215675C`, `02161AB4`, `02161CC0`). **blue-verified** (static callers; runtime, the argument equals the table byte in 14 calls).
- **Wild gate.** `info + 6` non-zero (a wild monster) sets the odds to 0 before the other terms. The sim's attacker is always the leader, so the odds are the move's. Runtime: with forced odds 50, the 12 team-member calls crit exactly when `roll < 50`, and the 8 wild calls never crit. **blue-verified**.
- **Other terms, not modelled.** Focus Energy (status 4, odds `DAT_0217C7C8`), Scope Lens (item 12, `+40`), Patsy Band (item 13, `+40`), the IQ type advantage (its odds replace the others). **medium** (values not read).
- **Roll and effect.** A critical hit multiplies the type factor by the STAB multiplier (1.5). Its flag goes to the damage struct at `+0xD`, cleared when the damage is 0. **blue-verified**.
- **Battle and Shell Armor.** The game does not draw the critical roll for a target with Battle Armor (`0x0C`) or Shell Armor (`0x13`): the draw sits inside a test of those two abilities (`0217BD94` lines 274 and 275 of the export). The sim skips the draw for such a target, so a hit draws four values, not five. **static**: no runtime run on an armoured target was made.

### P6. Abilities

- **Writer.** `info + 0x58` and `+0x59` are the species' two abilities, copied at monster spawn by `FUN_ov_0001__0217930C` (its callers: the spawn at `021794CC`, the evolution at `02182900`, and others). `FUN_020021EC` reads the species record at `+0x17` and `+0x18`, the same bytes as `rescue_data::Species::abilities`. Level-up does not write them. **blue-verified** (static; runtime: 10 of 10 calls matched the species record).
- **Sim.** `EntityInfo::apply_species` copies `Species::abilities` into `abilities`, and `has_ability` tests the pair. The sim changes no ability (no ability-changing effect is modelled).

### P7. Stage tables (`CalcDamage`)

- **Tables.** Attack `ov_0001::021AA660` and defence `ov_0001::021AA6B4`: 21 `s24.8` words each. Only `CalcDamage` reads them. **blue-verified** (export xrefs).
- **Index.** The stage is `info + 0x1C` (attack) or `+0x20` (defence), plus `2 * split`, clamped to 0 to 20. Neutral is 10. The shape is non-decreasing in both tables, and equal except at indices 7 to 9, where the defence table is higher. **blue-verified** (live read; runtime probe `r.py` of the clamp at (25, -5) gave (20, 0)).
- **Not checked.** The Deoxys ±2 (its three form IDs are in [Damage formula](#damage-formula), not the signs), and the Skull Bash defence stage (`info + 0xAE == 6`). **open**.
- **Sim.** `rescue_data::combat::CombatTables::{attack_stage, defence_stage}`; the sim keeps `EntityInfo::{offensive_stages, defensive_stages}`.

### P8. Regular Attack (action `0x32`)

- **Action.** Regular Attack is action `0x32` (50) at info `+0x42`. The leader's input loop `02166B10` writes it with `021418AC(info+0x42, 0x32)` when the attack input has no `0x200` bit and `02165A80` returns 0. The move-menu actions `0x14` (leader, `02172CDC`) and `0x15` (`02191DBC`) use the move slot path and reach `0215BE14`; action `0x32` does not. Runtime: the leader ran action 50 (163 executions) in base run `5803e729`. **blue-verified**.
- **Execute switch.** `0218453C` case `0x32` calls `FUN_02172ADC(entity, 0x163)`. The literal is at `02184E44`. **blue-verified** (static; the literal read at runtime).
- **Route.** `02172ADC` builds a stack copy of move `0x163` (flags `0x0D`, PP 10, a temporary: no move slot is read or written). It runs `0215B384(entity, 0, 0, 0, 0, move)` once, or twice when a weather or ability count says so (`02181D98`, not run). `0215B384` skips the animation call and calls the dispatcher `02156824` for the target list. The dispatcher runs the slot-0 hit check `0215A928` (`0215A950` draws), then switch case `0x163` calls `0215675C(entity, target, move, 0x80)`, which calls `CalcDamage` (`arg8` `0x80`), then the slot-1 check at `0215652C`. `0215BE14` is not on this path. **blue-verified** (static; the hit and miss runs below reached the accuracy and damage hooks along this route). The probe's entry counts for `0215B384` and `0215BE14` were not split by action, so they are not quoted here. **open** for the two-strike branch (`02181D98` returning 2).
- **Move row.** Accuracy: slot 0 is 125, slot 1 is 93 (`FUN_0200B3C4`). Power 1 (base power, with the ginseng byte 0 added). Type 0 (`GetMoveTypeForMonster`; the row's own type byte is also 0, so the sim reads the row). Critical odds 0 (row `+0x13`). The sim reads all four from the move table in the dump, not from this note. **blue-verified** (runtime hooks of `acc` and `calc`; the dump's row read by the ROM check).
- **PP.** None. The PP byte of the stack copy is a literal and is never written back. **blue-verified** (runtime, 14 uses in the base run).
- **Draws, hit.** Return sites, in order: `rand(100)` at `0215A950` (slot-0 check), `rand(100)` at `0217C02C` (boost), `rand(100)` at `0217C698` (critical), `rand(0x4000)` at `0217C720` (variance), `rand(100)` at `0215A950` (slot-1 check, return `02156550`). Six leader hits in run `5803e729` (the first at frame 1511). **blue-verified** (runtime).
- **Draws, slot-0 miss.** One draw, `rand(100)` at `0215A950`; no `CalcDamage`, no slot-1 check. Forced: the accuracy byte of `0x163` set to 1 at `AccuracyCalc` entry (run `75abd573`, ten leader attacks). **blue-verified** (runtime, patched memory).
- **Critical.** The odds argument is 0, so a leader hit does not crit without Focus Energy or the IQ skill. In the six leader hits the roll was 41, 97, 0, 72, 83 and 29 with the odds at 0, and no hit was reported critical; the crit flag itself was not hooked. **medium** for "no natural crit"; the Focus Energy and IQ terms are not run ([P5](#p5-critical-hit-odds)).
- **Natural miss.** Not seen. All 24 accuracy calls on `0x163` in the base run hit. The slot-1 miss path is the shared one. **open**.
- **Empty swings.** Most action-50 executions in the script hit an empty tile: no accuracy call and no `CalcDamage` followed. The sim does not model them (see below). **open**.
- **Sim.** `combat::REGULAR_ATTACK` (`0x163`, the row is read from `CombatData::moves`), `regular_attack` (the facing foe, the leader's turn) and `resolve` (the draws and damage, shared with `strike`). `strike` adds the PP mark and the PP take, which `resolve` does not do.

### What the sim runs, and what it leaves out

The sim runs a leader's use of Tackle (`0x9A`) or Scratch (`0x104`), the two moves of the dispatch's direct `HandleDamagingMove` path with a verified `arg8` of 1.0 ([#185](#damage-formula)), and its Regular Attack ([P8](#p8-regular-attack-action-0x32)), which takes the same hit checks, damage and draws with `arg8` `0x80` and no PP. It takes the leader's turn, then:

1. Marks the PP ([P3](#p3-pp-mark-on-use-take-it-at-the-end-of-the-action)). Draws the slot-0 hit check.
2. On a hit: draws the boost roll, the critical roll (not for a target with Battle or Shell Armor, [P5](#p5-critical-hit-odds)) and the variance roll, in that order, then computes the damage: the attack (power plus attack, the stage, truncated, clamped to 0 to 999), then `CalcStatBoosts` as one fraction with one truncating divide: the numerator and denominator take Huge Power and Pure Power (×3/2, at a boost roll of 32 or less), Hustle (×3/2) and the target's Intimidate (×4/5), physical moves only (the export of `0217C7F8`: the ×3 and ×4 go to the numerator, the 2 and 5 to the denominator, then one divide). The defence (the stage, truncated), the level term (`level × 2/3` in 48.16), `x = (attack − defence) / 8 + term`, `base = x² × 0.05 + (2x − defence + 10)` clamped to [1, 999], the type factor and the critical multiplier, `arg8` (1.0), the variance `(0xE000 + roll) / 0x10000`, and the rounding of each 48.16 product (half up on bit 15) and the final integer (`FP48_16_ToS32`).
3. Draws the slot-1 hit check. On a hit the target's HP falls by the damage.
4. Takes 1 PP at the end of the action, after a hit or a miss.

Order of draws, for a hit or a slot-1 miss: accuracy (slot 0), boost, critical, variance, accuracy (slot 1). A target with Battle or Shell Armor draws no critical value, so its hit takes four. A slot-0 miss draws one value. Tests cover the counts, the armoured targets, the critical path, the type cases, the stage formula at lowered and raised stages, the single divide of the stat boosts (Hustle with Intimidate, where sequential truncation differs), Compound Eyes, and PP after a hit and after a miss. For Regular Attack: the five draws of a hit and the one draw of a slot-0 miss, no PP, `arg8` `0x80` (half the damage of the same hit at 1.0), and `Attack` applied on a foe and unmodelled without one.

Left out, each as a `shortcut:` in the code:

- Regular Attack (`0x163`): the refusals of `02172ADC` (`info + 0xAC == 1`, `info + 0x9B == 4`), the second strike of `02181D98`, and the empty-tile swing ([P8](#p8-regular-attack-action-0x32)).
- Targets beyond the facing tile (#420), defeat (#422), secondary effects (#421).
- Held items, the Deoxys adjustment, Wonder Guard, the exposed flag, weather, charging, statuses, the offensive and defensive multipliers (`+0x2C`, `+0x34`, read as 1.0), the wild attacker's level term (`attack / 3`).
- Guts, Marvel Scale and Plus/Minus (the [abilities table](#abilities-in-calcstatboosts)): they need a negative status or a dungeon flag.
- The IQ type-advantage critical term (`FUN_021805b0(attacker, 1)`, [P5](#p5-critical-hit-odds)).
- A facing tile with no wild monster: the game's result for an empty target is not read, so the use is not modelled.

Checks: the damage steps were translated from `verification/v7-trace/combat_recompute.py`, which replays the 14 captured hits. Those hits are scratch data and were not re-run in this change.

## Entity layout seen here

- `entity+0x70` points to the entity's info struct, as in Red (`Entity.axObj.info`, `0x28 + 0x48`).
- The info struct's two types are at `+0x56` and `+0x57`. In Red, `EntityInfo.types` is at `+0x5C`, so Blue's `EntityInfo` has 6 fewer bytes before the types. Input for [#46](https://github.com/TorratDev/rust-mm/issues/46).

## Open questions

Unresolved items from the three investigation files for this issue. Each item is either a gap in the evidence or a claim that was not checked.

**Not yet done**

- Held items, abilities other than Huge Power, status effects, weather and type effects against observed hits. The #185 check does not cover them (see [the damage formula](#damage-formula)). Critical hits are partly covered by [P2](#p2-critical-hit-roll): a natural crit has not been seen.
- Huge Power is checked at runtime ([P3](#p3-huge-power-and-hustle)). A natural critical hit is not ([P2](#p2-critical-hit-roll)).
- Hits, a slot-0 miss and a slot-1 miss are seen at runtime ([P1](#p1-hit-check-accuracycalc)). Still open: a miss from a lowered accuracy stage, a raised evasion stage, sure-shot or whiffer.

**Damage formula**

- The disassembly-level store of the sixth argument (the roll) to `CalcStatBoosts` has not been seen.
- The offsets of `isTeamLeader` (`+7`), `isNotTeamMember` (`+6`) and `level` (`+9`) are checked statically against Red (P3, see [`structs.md`](structs.md)). `belly` (`+0x10E`) stays *probable*, because `FUN_0200b9a4` is inferred ([#46](https://github.com/TorratDev/rust-mm/issues/46)).
- The weather enum values (1, 3, 4, 6) and the IQ skill ID 1 (Type Advantage Master) are taken from Red, not checked in Blue.
- Red's `u24_8` round-up is compared by structure only.
- The `gDungeon` `+0x3358` and `+0x335A` Plus and Minus flags need a struct check (the pairing is inferred).
- `arg8`: the flow and callers are static (see [argument 8](#argument-8-arg8)). Its meaning as a scale is medium confidence.
- The step-10 branch is confirmed on 14 hits (#185): the fixed-point value matches, the truncated one does not (see step 10).

**Accuracy and evasion**

- The weather accessor `FUN_02195C48` is named `GetEffectiveWeather` ([P5](#p5-static-leftovers)). The item check it makes (item 45) is not identified.
- Closed: the helper behind `PTR_FUN_021805C8` is `02004250` ([P5](#p5-static-leftovers)).
- Closed: `FUN_02008C64` is the physical-move table lookup, read at runtime ([P5](#p5-static-leftovers)).
- The offsets `+0x24`, `+0xC4` and `+0xD2` in the info struct are inferred from Red's names. `+0xC4` is listed in [`structs.md`](structs.md) as the sure-shot status. `+0x26` is the target's evasion stage by investigator a's static read ([P5](#p5-static-leftovers)).
- `FUN_0208625C(a, 0x100)` is treated as `a / 256`, which is inferred from its use.
- Hustle's evasion effect is static plus one forced run ([P5](#p5-static-leftovers)). Hustle's attack ×3/2 was measured on one move ([P3](#p3-huge-power-and-hustle)). The two effects are not yet tied to one code path; a second move would test it.
- The AI chance sources of `DungeonRandOutcome_2` are open: `FUN_021473A8` beyond its first cases, the values at `02185C60`, and the meaning of `info+0xAC` at `02192D80` ([P4](#p4-outcome-wrappers-and-ai-chance-sources)).

**Range and targeting**

- `FUN_0208625C` is not decompiled. The 25% in `IsHpBelowQuarter` (`021818A4`) rests on reading it as a divide by 4.
- `FUN_0215A2C0`, which decides executor class 4, is not decompiled.
- The meanings of `info+0xAC`, `+0xC9` (the executor's `skipRelation` and the AI single-candidate case), and `+0x99` and `+0xA0` (the status helpers) are open.
- The names of IQ bits 3, `0xC` and `0xD` are open. Bit 3 gates the projectile walk, and `0xC` and `0xD` gate the terrain-class index.
- The veto fields of `0215A054` (`+0xDC`, `+0xAE`) are named by position only.
- The Red name for the random-chance byte is unconfirmed. The byte's offset (`+0x10`) is read in Blue.
- The executor room test `02141AF0` has no row in [`symbols.md`](../symbols.md).

**Move effects**

- The miss path of the dispatch (Red's `dungeon_move.c` around line 290, Hi Jump Kick and Jump Kick).
- The 262 handler bodies. The Red names for them are hypotheses.
- Seven IDs have no handler (`none`, no effect). `MOVE_IS_WATCHING` has no Red name; its Blue handler is `ov_0001::0215CDB4`.

**Defeat and experience**

- The cap is never reached at runtime; the clamp branch is static only. The `'J'`/`'G'` lock never fires at runtime. The level-100 early return never fires at runtime. `gDungeon+0xD` is not checked at runtime.
- The EXP multiplier value 2 has not been observed at runtime, and its writer is not located (see [EXP multiplier](#defeat-and-experience)).
- The join path at runtime: no recruit succeeded, so `MonsterJoinSequence_Async` was never called. The join flag's meaning (the pointed-to byte is not identified). Whether `MonsterJoinSequence_Async` returns 0 on failure in practice. That the reason from the stack is the same value Red uses for every KO (only the 355 case was seen).
- The `IQ_ITEM_MASTER` check for Reviver Seed was not found in Blue.
- The revive field offsets (`+0x112`, `+0x114`, `+0x11E`) are named by position only.
- Item drop: at runtime, no KO'd target held an item, so the drop call never ran. Whether the held-item check is the only gate (the Reviver Seed branch exits earlier, and the drop may depend on other flags). The meaning of the `0x02143918` arguments beyond `(entity, item entity, item, 1, 0)`. Whether team members (non-leader) drop items at the same rate as wild monsters.
- The team size behind the four `AddExpPoints` calls seen for two KOs was not logged.
- The discrepancy around `0x0217E868` (see the defeat section).
