# Combat: damage, accuracy, targeting and defeat

First entry points into Blue's combat code, found from the code that reads the type chart. Issue: [#50](https://github.com/TorratDev/rust-mm/issues/50). Names, addresses and tags are in [`symbols.md`](../symbols.md). Everything here is in overlay 1, the dungeon overlay ([`execution-model.md`](execution-model.md)).

Every function below matches its `pmd-red` counterpart in `src/dungeon_damage.c` by argument count, constants and calls, so it keeps that name, including `pmd-red`'s placeholder names (`sub_806E100`, ...). Confirmed by reading Blue's code, not by emulator trace.

**Status.** Everything in this page was read statically, from Blue's decompiled code, its overlay and ARM9 data, and `pmd-red` at the pinned commit. The only runtime evidence is the move-dispatch and EXP checks in the sections on [move effects](#move-effects) and [defeat and experience](#defeat-and-experience). **No observed damage number has been compared with the damage formula yet**, so the issue's condition "matches observed emulator hits" is **not met**.

## Functions

| Function | Role |
|----------|------|
| `CalcDamage` `ov_0001::0217BD94` | Damage formula. Same constants as Red: damage cap `999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`. Reads `gAtkStatStageMultipliers` and `gDefStatStageMultipliers`. |
| `sub_806E100` `ov_0001::0217CE88` | Type-effectiveness part of the damage calculation; called by `CalcDamage`. |
| `CalcStatBoosts` `ov_0001::0217C7F8` | Ability and held-item modifiers to attack and defense, called from `CalcDamage`. Its sixth argument is the roll from the first `DungeonRandInt(100)` ([RNG calls](#rng-calls-in-calcdamage)). Own name; the Red name is not confirmed. |
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

`CalcDamage` runs these steps in this order. The constants and step order match Red's `CalcDamage` (static comparison, P1.1). The formula has **not** been checked against observed hits.

1. A belly check that sets the damage to 1 (Red's `SetDamageOne`).
2. Wonder Guard, as in Red.
3. Clamp the attacker's and the defender's stat stages to 0 to 20.
4. Deoxys: a ±2 adjustment, as in Red. The Deoxys move IDs are `0x1A1` to `0x1A3`.
5. Base power: the move's power plus the attacker's attack stat. Multiply by the attack stage multiplier, then by the attacker's offensive multiplier, in 24.8 fixed point. Convert to an integer by truncation.
6. Defense side: Skull Bash (`0x15C`) adds one stage. Multiply the defense by its stage multiplier and then by the defender's defensive multiplier, and truncate.
7. Held-item boosts and ability modifiers, through `CalcStatBoosts` (see the next two sections).
8. Clamp the attack to 999. Only the attack is clamped; the defense is not.
9. Add a level or attack term, which is either level × 2/3 or attack / 3. The findings do not say which branch applies in each case.
10. Base damage = (attack − defense) / 8 + that term.
11. Base damage `x` becomes `x² × 0xCCC + (2x − defense + 10)`, where `0xCCC` is the 0.05 constant. Clamp the result to [1.0, 999].
12. Multiply by the type multiplier from `sub_806E100` (see [type effectiveness](#type-effectiveness)).
13. Critical hit: ×1.5 when the crit roll passes (see [critical hits](#critical-hits)).
14. The extra argument `arg8` (not described further in the findings).
15. Variance: a factor of `(0xE000 + roll) / 0x10000`, from `DungeonRandInt(0x4000)`. That is 0.875 to about 1.125.
16. Convert the 48.16 result to an integer with `FP48_16_ToS32` (see [rounding](#rounding)).

### Type effectiveness

`sub_806E100` looks up the move type's row in `gTypeEffectivenessChart`, uses the defender's two types (at info `+0x56` and `+0x57`) as columns, and combines the two results through `gEffectivenessChart`. Then it applies the following rules, in this order. Everything here matches Red line by line (static comparison, P1.2).

- **Ghost exception:** a Ghost defender (type 14) takes no damage from Normal (type 1) or Fighting (type 7) moves, unless the defender is exposed (flag at info `+0xD2`, not confirmed in Blue).
- **Immunity abilities:** Flash Fire (against Fire moves with a status) and Levitate (against Ground moves) return "immune". Thick Fat halves Fire and Ice damage.
- **Low-HP abilities:** Torrent, Overgrow, Blaze and Swarm double the move's power when the attacker is at or below 1/4 HP.
- **STAB:** ×1.5 (the pool value shared with the critical multiplier).
- **Weather:** sunny, rain, cloudy and fog change the multiplier. The weather IDs (1, 3, 4, 6) come from Red's enum and were not checked in Blue.
- **Charging:** ×2.0.
- **Return value:** whether the combined result is "super-effective". Wonder Guard uses this.

The four effectiveness multipliers in the pool are 0.5, 0.9, 1.0 and 1.5. Which index maps to which label has not been checked in Blue.

### Abilities and held items in `CalcStatBoosts`

These modifiers are applied in the stat-boost function (`ov_0001::0217C7F8`). Ability IDs are as in Red's `ability.h`. Integer division truncates toward zero.

| Ability or item | ID | Applies to | Effect |
|-----------------|----|-----------|--------|
| Guts | `0x11` | attacker | attack ×2 while the attacker has a negative status |
| Huge Power, Pure Power | `0x22`, `0x4B` | attacker | attack ×3/2 for physical moves when the roll is 32 or less (the roll is `CalcDamage`'s first `DungeonRandInt(100)`) |
| Hustle | `0x30` | attacker | attack ×3/2 for physical moves |
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

- The crit roll (`DungeonRandInt(100)`) is drawn only when the defender has neither Battle Armor (`0x0C`) nor Shell Armor (`0x13`).
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

The base damage is clamped to [1.0, 999]. The attack is clamped to 0 to 999 before the boosts. The defense is not clamped. These match Red (static, P2.6).

## RNG calls in `CalcDamage`

For bit-exact replay the order of RNG calls matters ([`rng.md`](rng.md)). Blue's `CalcDamage` makes three dungeon RNG calls, in this order (static, P2.4). The type function and `CalcStatBoosts` make no RNG call.

1. `DungeonRandInt(100)` at the top of `CalcDamage`, before the items. Its result is the **roll** that `CalcStatBoosts` receives as its sixth argument. Huge Power and Pure Power read it (boost only when the roll is 32 or less). The earlier note that this result is unused was wrong. The Ghidra call site shows four arguments because the prototype is not set. The sixth is inferred from the callee, and the stack store was not seen in the disassembly.
2. `DungeonRandInt(100)` for the critical hit, compared with the odds. It runs after the type function and the screen multipliers, and only when the defender has neither Battle Armor nor Shell Armor.
3. `DungeonRandInt(0x4000)` for the variance. It always runs.

## Accuracy and evasion

The hit check is `AccuracyCalc`, at `ov_0001::0215A928`. It is the Blue counterpart of Red's `AccuracyCalc` (static, P3a).

- Its first step is `DungeonRandInt(100)`, drawn before every early return. The result is used only at the end, as `roll < effective accuracy`. So one RNG call is consumed on every call, including forced hits and misses (P3d). Keep this draw order in any golden trace, even when the result is forced.
- The accuracy is the move's accuracy byte at `+0xE + slot`, read by `FUN_0200B3C4` (slot 0 to 2).
- Early results, as in Red:
  - Attacker status 1 (sure shot) hits; status 2 (whiffer) misses.
  - Thunder (`0x40`) always hits in rain (weather 4). In sun (weather 1) it lowers the attacker's accuracy stage by 2.
  - Compound Eyes (`0x33`) on the attacker: +2 accuracy stage.
  - Target exposed (flag at info `+0xD2`): evasion stage 10.
  - Sand Veil (`0x1D`) in a sandstorm (weather 2): +2 evasion stage.
  - Hustle (`0x30`) with a physical move: a +2 stage change. The findings give this change as an evasion stage; the stage it applies to needs a recheck.
- Penalties: a target holding the Detect Band (item 35) takes −30. A target with the Quick Dodger IQ skill (IQ bit 5) takes −30. For move `0x163` (Regular Attack), an attacker with IQ bit 4 (Sure Hit) is a sure-hit case.
- Stage tables: accuracy at `ov_0001::021AA8C4` (pointer slot `ov_0001::0215AB2C`) and evasion at `ov_0001::021AA918` (pointer slot `ov_0001::0215AB30`). Both have 21 entries and match Red's `gAccEvsStatStageMultipliers` entry by entry. Blue stores integers where Red's table holds s24.8 values. The stage is clamped to 0 to 20, and the multiplier to a cap of `0x6400`.
- `IqSkillIsEnabled` at `ov_0001::021805B0` tests bit `n` of the info's IQ bitfield at `+0x87` (mask `1 << n`). Blue calls it through a pointer (`PTR_FUN_021805C8`), so the helper is not decompiled. The bit numbers 4, 5 and 8 match Red's IQ skill indices.
- Other callers of the random outcome functions (static, P3f):
  - `DungeonRandOutcome` at `ov_0001::0219CFCC`, called from the chance wrappers `ov_0001::02159F10` (the chance is doubled when the attacker has ability `0x26`) and `ov_0001::02159F6C` (the chance gate, which returns false when the target has the flag `+0x12A` and `+0xE` is 0, and calls `FUN_0213EB00` for ability `0x4C`).
  - `DungeonRandOutcome_2` at `ov_0001::0219CFF0`, called from the AI target pick (`ov_0001::0218579C`), the AI candidate pick (`ov_0001::02185568`) and the AI move choice (`ov_0001::02192D24`).
  - The meaning of each chance argument is not traced.

## Range and targeting

Targeting is read from the move in three parts. Static, P4a to P4f.

- **Range byte** (move `+0x19`): display text only. Its only reader is `FUN_0200B304`, which is called only from the message builder `FUN_0200B188`. Nothing in the dungeon logic reads it, and Red's `GetMoveRangeID` is also display-only.
- **Targeting** (`u16` at move `+0x8` for the player and `+0xA` for the AI): read through `FUN_0200B4EC(move, i)`. The selector is `ov_0001::02191A30`, which returns `targeting[i]`, except that move `0xED` with `i == 0` returns `0x73` unless the attacker is type `0x0E`. The AI resolver `FUN_02192728` uses `i = 1`.
- **Target class** (low nibble of the targeting value, read in `ov_0001::02192274`):
  - `0`: foe.
  - `1`: not a foe (an ally or the user). Whether the user is caught here, or by a separate self check, is open (Red's `TARGET_PARTY` includes the user).
  - `2`: any target, excluding a target with `+8 == 1` or a `+0x98` value of 1 or 4.
  - `3` and `4`: always valid.
  - `5`: any target except the user, with the same exclusions as `2`.
  - `6`: an ally, not the user.
  - `7` to `15`: not handled. The function returns false. Red's `TARGET_SPECIAL` (15) is handled elsewhere.
- **Area** (high nibble, in the AI resolver `FUN_02192728` and `FUN_02192508`). The area names are Red's `TargetingFlag` names, which the findings give as red-hypotheses:

| Nibble | Blue behaviour | Red name (hypothesis) |
|--------|----------------|-----------------------|
| `0x00` | 8 neighbours; a direction test (`FUN_02180FFC`) gates each | `TARGETING_FLAG_TARGET_OTHER` |
| `0x10` | 8 neighbours; all accepted | `FRONTAL_CONE` |
| `0x20` | 8 neighbours; all accepted | `AROUND` |
| `0x30` | 20 entity slots (`+0x11364`), with the relation from `FUN_02141C08` | `ROOM` |
| `0x40` | 8 directions at distance 2 | `2_TILES_AHEAD` |
| `0x50` | A line, distance 10 (`FUN_02192618`) | `LINE` |
| `0x60` | All 20 slots | `FLOOR` |
| `0x70` | The user only | `SELF` |
| `0x80` | Cut corners, distance 1 | `CUT_CORNERS` |

The difference between `0x10` and `0x20` is not known, since both accept every neighbour. The helpers `FUN_02180FFC`, `FUN_02192618`, `FUN_02191E70` and `FUN_02141C08` were not decompiled, so the area rules are not ported yet.

- **AI condition** (`0xF00` bits, read in `FUN_02192274`). They match Red's `AI_CONDITION_*` values:
  - `0x100`: random. The chance is accuracy byte 2 (`FUN_0200B3C4(move, 2)`) against `DungeonRandInt(100)`. The field name in Red is not confirmed.
  - `0x200`: HP at or below 25% (`FUN_021818A4`).
  - `0x300`: has a status (`FUN_02181904`).
  - `0x400`: asleep (`FUN_021818D0`).
  - `0x500`: HP at or below 25% or has a status.
  - `0x600`: ghost target (type `0x0E`) with `+0xD2 == 0`.
  - The condition is checked only when the attacker has the Status Checker IQ skill (IQ bit 8, through `IqSkillIsEnabled`). Without it, only `0x100` applies.

## Move effects

The move-effect dispatch is `ov_0001::02156824`. It switches on the move ID at `move+2` and calls one handler per case. The full catalogue of 413 IDs and 262 handler addresses is in [`move-effects.md`](move-effects.md). Handler names there are Red names, mapped by move ID, and are hypotheses until each handler is read.

## Defeat and experience

Static, unless marked "runtime". The damage path and faint handler match Red by structure and parameter order. The field offsets are in [`structs.md`](structs.md).

- **Damage path.** `HandleDealingDamage_Async` (`ov_0001::0217E890`) calls the per-hit function `ov_0001::0217D5FC`, which is `HandleDealingDamageInternal_Async` in Red by structure. The per-hit function is entered from `ov_0001::0217E8C0` and `ov_0001::0217EB54`.
- **Faint.** `HandleFaint_Async` is `ov_0001::021778F4`, by structure (the same parameter order, exit-reason guard, team-leader and cutscene branches). Callers: `ov_0001::0217E278`, `ov_0001::0217E850`, `ov_0001::0217E864`, `ov_0001::0217E878` (damage path) and `ov_0001::0217F4E0` (join path).
- **KO reason.** When no item is involved, the exit reason is the ID of the move that dealt the KO, as in Red (`GetDungeonExitReasonFromMoveOrItemID`). Runtime: the reasons `0x9A`, `0x163` and `0x104` were seen. Item KOs and Wonder Orb KOs use the 500-range reasons (`DUNGEON_EXIT_REASON_START = 500`). These were not observed.
- **EXP formula.** `CalculateEXPGain` is ARM9 `020020F8`. The formula is the base yield plus the yield × (level − 1) / 10. The yield is read from the species table, which has stride `0x48` and the yield at offset `0x20`. The formula matches Red (static).
- **EXP multiplier.** The target's multiplier at info `+0xD8` gives: `0` halves the amount, `2` multiplies it by 3/2, and `1` leaves it unchanged. A result of 0 becomes 1. These match Red's `EXP_HALVED = 0`, `EXP_REGULAR = 1` and `EXP_BOOSTED = 2`. Runtime: `AddExpPoints` was called with amount 7 (multiplier 0) and 15 (multiplier 1) for species 290 at level 1. Both match. The value 2 was not observed.
- **Award.** `AddExpPoints` is `ov_0001::02183F78`. It adds to info `+0x18` (EXP), records the delta at `+0x1C8` (EXP gained this turn), and sets `gDungeon` `+0xD`. The cap comes from a literal that was not read, so the `9999999` cap is not confirmed. The lock check reads the join-location byte at info `+0x40` against `'J'` and `'G'`, in place of Red's `IsExperienceLocked`. It is called once per recipient: the attacker and each team member.
- **Recruit and join.** `ov_0001::0217F9B8` (two arguments; the name matches Red's `TryRecruitMonster`) and `ov_0001::0217F388` (`MonsterJoinSequence_Async`). When the first returns 0, the target goes to `HandleFaint` with the "left without being befriended" reason, from the constant at `DAT_0217E88C` (not read). A nonzero result from the second sets the flag at `DAT_0217E888`. Not observed.
- **Revive.** `ov_0001::0217D5FC` has two blocks that restore a target. Each sets HP to the maximum HP. The first block (orb-like) returns 0, which means no faint. The second (seed-like) falls through, as in Red's Reviver Seed. Not observed. Static, medium confidence; the field names come from their position in Red's struct.
- **Item drop on KO.** Red drops the held item for a non-leader (`SpawnDroppedItemWrapper`). The Blue equivalent was not found in `ov_0001::0217D5FC` or `ov_0001::021778F4`. Open.
- **Open discrepancy.** One faint's return address (`0x0217E868`) lies inside the range that Ghidra gives `ov_0001::0217D5FC`, which is 4,612 bytes long. A damage entry at frame 621 and a faint at frame 687 are 66 frames apart. The timing suggests that the faint belongs to the same invocation. This is an inference from timing, not a checked fact.

## Entity layout seen here

- `entity+0x70` points to the entity's info struct, as in Red (`Entity.axObj.info`, `0x28 + 0x48`).
- The info struct's two types are at `+0x56` and `+0x57`. In Red, `EntityInfo.types` is at `+0x5C`, so Blue's `EntityInfo` has 6 fewer bytes before the types. Input for [#46](https://github.com/TorratDev/rust-mm/issues/46).

## Open questions

Unresolved items from the three investigation files for this issue. Each item is either a gap in the evidence or a claim that was not checked.

**Not yet done**

- Compare observed damage numbers with the formula. This is the issue's condition, and it is **not met**. It needs a runtime check (#17, py-desmume, `verification/v7-trace`).
- A runtime check of a Huge Power roll and of a critical hit, as a separate task under #17.
- A runtime check of the hit check (no hit has been observed).

**Damage formula**

- The disassembly-level store of the sixth argument (the roll) to `CalcStatBoosts` has not been seen.
- The weather enum values (1, 3, 4, 6) and the IQ skill ID 1 (Type Advantage Master) are taken from Red, not checked in Blue.
- Red's `u24_8` round-up is compared by structure only.
- The `gDungeon` `+0x3358` and `+0x335A` Plus and Minus flags need a struct check (the pairing is inferred).
- `arg8` is not described.
- The branch that chooses between `level × 2/3` and `attack / 3` is not described.

**Accuracy and evasion**

- The weather accessor `FUN_02195C48` is not named, and the item check it makes (item 45) is not identified.
- The helper behind `PTR_FUN_021805C8` is not decompiled.
- `FUN_02008C64` as the physical-move table is inferred from Red's `IsTypePhysical`.
- The offsets `+0x24`, `+0x26`, `+0xC4` and `+0xD2` in the info struct are inferred from Red's names. `+0xC4` is listed in [`structs.md`](structs.md) as the sure-shot status.
- `FUN_0208625C(a, 0x100)` is treated as `a / 256`, which is inferred from its use.
- Hustle's stage change: the findings call it evasion, but the sign and the target stage need a recheck.
- The callers of `DungeonRandOutcome` and `DungeonRandOutcome_2` are listed but their chance arguments are not traced.

**Range and targeting**

- Helpers `FUN_02180FFC`, `FUN_02192618`, `FUN_02191E70`, `FUN_02141C08`, `FUN_021818A4`, `FUN_02181904` and `FUN_021818D0` are not decompiled. The area rules (P4d) depend on them.
- The difference between area nibbles `0x10` and `0x20` is unknown.
- Whether the user is included in target class `1` is open.
- The byte index of the random chance (`FUN_0200B3C4(move, 2)`) and the field name it maps to are unconfirmed.
- The Red `IQ_STATUS_CHECKER` gate is the Red reference only, and the Blue IQ bit mapping (bit 8) is not checked against the IQ table.

**Move effects**

- The miss path of the dispatch (Red's `dungeon_move.c` around line 290, Hi Jump Kick and Jump Kick).
- The other 8 `switch` statements in `ov_0001::02156824`.
- The 262 handler bodies. The Red names for them are hypotheses.
- Seven IDs have no handler (`none`, no effect). `MOVE_IS_WATCHING` has no Red name; its Blue handler is `ov_0001::0215CDB4`.

**Defeat and experience**

- The `9999999` EXP cap (literal at `DAT_02183FE8`, not read) and the Red `IsExperienceLocked` equivalent (the join-location check at info `+0x40`). The lock check is unconfirmed.
- The EXP multiplier value 2 has not been observed at runtime.
- The reason constant for "left without being befriended" (`DAT_0217E88C`) and the join flag (`DAT_0217E888`) are not read.
- The `IQ_ITEM_MASTER` check for Reviver Seed was not found in Blue.
- The revive field offsets (`+0x112`, `+0x114`, `+0x11E`) are named by position only.
- The item drop on KO was not located (P6-h).
- The team size behind the four `AddExpPoints` calls seen for two KOs was not logged.
- The discrepancy around `0x0217E868` (see the defeat section).
