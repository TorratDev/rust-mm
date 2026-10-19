# Combat: damage and type effectiveness

First entry points into Blue's combat code, found from the code that reads the type chart. Issue: [#50](https://github.com/TorratDev/rust-mm/issues/50). Names, addresses and tags are in [`symbols.md`](../symbols.md). Everything here is in overlay 1, the dungeon overlay ([`execution-model.md`](execution-model.md)).

Every function below matches its `pmd-red` counterpart in `src/dungeon_damage.c` by argument count, constants and calls, so it keeps that name, including `pmd-red`'s placeholder names (`sub_806E100`, ...). Confirmed by reading Blue's code, not by emulator trace.

## Functions

| Function | Role |
|----------|------|
| `CalcDamage` `ov_0001::0217BD94` | Damage formula. Same constants as Red: damage cap `999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`. Reads `gAtkStatStageMultipliers` and `gDefStatStageMultipliers`. |
| `sub_806E100` `ov_0001::0217CE88` | Type-effectiveness part of the damage calculation; called by `CalcDamage`. |
| `sub_806F370_Async` `ov_0001::0217BB94` | Applies a fixed amount of damage (34 callers). Wonder Guard (`AbilityIsActive(target, 0x35)`) zeroes it unless the combined effectiveness is "super". Then `HandleDealingDamage_Async`. |
| `HandleDealingDamage_Async` `ov_0001::0217E890` | Deals the damage. |
| `sub_806F4A4` `ov_0001::0217BA94` | Returns 1 when the move type is not immune to either of the target's two types, and 0 when it is immune to one of them. It also returns 0 for the Ghost exception (target type 14, move type 1 or 7, target not exposed). |
| `WeightWeakTypePicker` `ov_0001::0217CB3C` | Uses the chart to weigh move types against a target (AI). |
| `AbilityIsActive` `ov_0001::0218067C` | Ability check. |

Red has a fifth reader of the type chart, `TargetNotImmuneTo`, marked unused; Blue has no matching code reference.

## Tables

All four have the same values as in Red (compared entry by entry; the values are not reproduced here, see [`legal.md`](../legal.md)):

| Table | Address | Shape |
|-------|---------|-------|
| `gTypeEffectivenessChart` | `ov_0001::021AAEE8` | 18 × 18 `s16`, row = attacking type, column = defending type |
| `gEffectivenessChart` | `ov_0001::021AA588` | 4 × 4 `s32`, combines the effectiveness against the two defending types |
| `gAtkStatStageMultipliers` | `ov_0001::021AA660` | 21 × s24.8, index = attack stat stage, 1.0 at index 10 |
| `gDefStatStageMultipliers` | `ov_0001::021AA6B4` | 21 × s24.8, index = defense stat stage |

## RNG calls in `CalcDamage`

For bit-exact replay the order of RNG calls matters ([`rng.md`](rng.md)). Blue's `CalcDamage` makes the same three dungeon RNG calls as Red's, in the same order:

1. `DungeonRandInt(100)`. In Red its result goes to the stat-boost step; in Blue's decompiled output the result appears unused. Check this in the disassembly before porting.
2. `DungeonRandInt(100)` compared with the critical-hit odds.
3. `DungeonRandInt(0x4000)` for the damage variance.

## Entity layout seen here

- `entity+0x70` points to the entity's info struct, as in Red (`Entity.axObj.info`, `0x28 + 0x48`).
- The info struct's two types are at `+0x56` and `+0x57`. In Red, `EntityInfo.types` is at `+0x5C`, so Blue's `EntityInfo` has 6 fewer bytes before the types. Input for [#46](https://github.com/TorratDev/rust-mm/issues/46).

## Open questions

- Walk `CalcDamage` step by step against Red to find any Blue-only change in the formula.
- Accuracy and the other `DungeonRandOutcome` callers.
- The first `DungeonRandInt(100)` result in `CalcDamage` (see above).
