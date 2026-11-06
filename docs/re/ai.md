# Enemy and ally AI

How Blue decides what an AI-controlled monster does on its turn. Issue: [#52](https://github.com/TorratDev/rust-mm/issues/52), under epic [#5](https://github.com/TorratDev/rust-mm/issues/5). Names, addresses and tags are in [`symbols.md`](../symbols.md). The code is in overlay 1 ([`execution-model.md`](execution-model.md)), except addresses without an `ov_0001::` prefix, which are in the ARM9 binary.

**Evidence: static only.** Everything here comes from read-only Ghidra reads of overlay 1 and the ARM9 binary, from overlay byte reads, and from the pinned `pmd-red` (commit `013475aa04f5be3191e5527c186d9bfceae7cae0`, see [`references.md`](../references.md)). **No py-desmume runtime check was run for any point on this page.** Every "not verified" note below still stands.

The `RunMonsterAI` branch match, the `aiTarget` accessors and the IQ field belong to [#164](https://github.com/TorratDev/rust-mm/issues/164). Their results are in [`structs.md`](structs.md) ([aiTarget readers](structs.md#aitarget-readers-164) and [Entity info](structs.md#entity-info)). The step list below was matched against Red branch by branch (#164 P1b).

Tags are as in [`symbols.md`](../symbols.md): **blue-verified** (role read in Blue's code), **red-hypothesis** (role taken from `pmd-red`, not yet checked in Blue), **tentative** (our own name, role partly known). Confidence is given where the source gave it.

## Decision flow (`RunMonsterAI`)

`RunMonsterAI` is `ov_0001::02188030`. It reads the monster struct through the entity's info pointer (`entity+0x70`); offsets below are from that struct. It returns nothing. The decision is the non-zero action in `+0x42` together with the early returns. The steps run in this order. Every Red branch of `RunMonsterAI` matches Blue in order and gating (#164 P1b, static). The callees are not re-compared there, and some are covered on this page:

1. **Petrified-ally gate.** If the `u16` at `+0x00` has `0x8000` set (the petrified-ally swap flag), return. Before returning, if the frozen status at `+0xA0` is 6 (petrified), call `0218FC18(self, self)`. Nothing else runs while the flag is set. [blue-verified for the flag and the status; red-hypothesis for the name, which matches Red's `EndFrozenClassStatus`; medium]
2. **Clear the decoy tracker.** Set `+0xDD` to 0. [blue-verified by structure (#164 P1b): written in the matching Red branches, steps 2 and 6. The name `decoyAITracker` and `DECOY_AI_NONE` are red-hypothesis; the offset `+0xDD` is from the shift table, medium]
3. **Behaviour gate.** If `monsterBehavior` (`+0x98`) is not 0 and `0219EFCC` returns 0, return. `0219EFCC` is a switch on `+0x98` that returns 0 for behaviours 2, 4, 10, 11, 12 and above 36, and 1 otherwise. Behaviour 0 (FIXED_ENEMY) skips the check, as Red's `FIXED_ENEMY ||` does. The switch agrees with Red's `ShouldRunMonsterAI` (`src/dungeon_cutscene.c:1093`) for all IDs 0-36. [blue-verified; high]
4. **Held item.** If `+0x98` is not 1 (RESCUE_TARGET) and `+0xD9` (`useHeldItem` in Red) is set: call `0218163C` (`CheckVariousConditions`). If it returns true, clear `+0xD9`, post the "couldn't be used" message (`02141930`, then `02154BF0`; the string comes from a per-language table) and return. Otherwise call `0218579C` (item AI, see [Item AI](#item-ai)). If the action in `+0x42` is then non-zero, return. [blue-verified structure; red-hypothesis names]
5. **Status gate.** `02180048` (`HasStatusThatPreventsActing`) matches Red's `HasStatusThatPreventsActing` (`src/status_checks.c:18-100`) branch for branch. If the monster cannot act, it sets the action to PASS_TURN (`021418AC(+0x42, 1)`), posts the status messages and returns 1; the caller then returns. The function also has the decoy-curse, blinker and cross-eyed side effects, with the same structure as Red. [blue-verified structure; high]
6. **Decoy scan.** If `dungeon+0x335C` (`decoyIsActive`) is set, loop over the 20 active entities at `dungeon+0x11364`. Take the first entity that is valid, has curse status `+0xB3` equal to 2 (STATUS_DECOY) and passes `02141C08` (`CanSeeTarget`). Set `+0xDD` to 1 if the applier flag `+0xB4` is 0 (team), else 2 (wild). [red: `DECOY_AI_TEAM` = 1, `DECOY_AI_WILD` = 2. The values are written here (blue-verified by structure, #164 P1b); the names are red-hypothesis.] [blue-verified; medium; the offsets `+0xB3` and `+0xB4` are not checked against [`structs.md`](structs.md)]
7. **Clear the action.** `021418C0(+0x42)` (`ClearMonsterActionFields`).
8. **Rescue branch** (`+0x98 == 1`). Call `02141878` (`SetActionPassTurnOrWalk`) with a direction from `DungeonRandInt(8)` and the target one tile above the monster. Return. [blue-verified]
9. **Item use, second call.** Call `0218579C` again. If the action is non-zero, return.
10. **IQ 21, Dedicated Traveler.** `021805B0(self, 0x15)` reads the IQ-skill bits at `+0x87` (settled by #164 P3b: they are derived from IQ `+0x0C`, see [structs.md](structs.md#entity-info)) and tests bit `1 << 21` through `02004250` (`IsIQSkillSet`), which checks the bit across the three IQ bytes. The pool word `0x021805C8` holds `0x02004250`. [blue-verified; high]
    - **IQ set** (Red also calls `ChooseAIMove` in this branch, #164 P1b):
      - If confused (`+0xAC == 2`), call `02141878` and return.
      - Otherwise, if `GetCanMoveFlag` (`0200234C`) is set, call `021918D4(self, TRUE)` (`AIMovement`).
      - If the action is above PASS_TURN, return.
      - Call `02192D24` (`ChooseAIMove`).
      - If the action is not NOTHING: if it is not PASS_TURN, clear `+0x71` (`aiNotNextToTarget`), `+0x116` (`aiAllySkip`; the offset comes from Blue only) and `+0x119` (waiting). Return. `RunMonsterAI` is therefore a writer of `aiAllySkip` and `waiting` as well as `aiNotNextToTarget` (blue-verified writes; probable-high for the field names, which are red-hypothesis; offsets medium, from the shift table).
    - **IQ not set:**
      - Call `02192D24`. If the action is non-zero, return.
      - If confused, call `02141878`. Otherwise, if `GetCanMoveFlag` is set, call `021918D4(self, TRUE)`.

Investigator A compared the two IQ branches with Red's `dungeon_ai.c` and found the same structure, with the three flag clears only in the IQ-set branch in both. Investigator C read Red's IQ-set branch as skipping the move choice. #164 P1b settles it: Red calls `ChooseAIMove` in the IQ-set branch, as A found, and Blue matches. See [Ally AI](#ally-ai), P4-f.

**What ends the turn.** The early returns above and the non-zero action in `+0x42`. Whether the caller reads it this way was not traced (P1-h, medium). Tracing the caller belongs to [#49](https://github.com/TorratDev/rust-mm/issues/49).

## Move and target selection

Entry point: `ov_0001::02192D24` (`ChooseAIMove`), called from step 10 of `RunMonsterAI`. Red's counterpart is `ChooseAIMove` (`src/dungeon_ai_attack.c:53`).

- **Confusion gate (P2.1).** If `+0xAC == 2` and `DungeonRandOutcome_2(70)` is true (that is, `DungeonRandInt(100) < 70`), the move choice is skipped. The 70 comes from the pool word `0x021933B8`, which points to `0x021AA184`, a halfword of value 70. Red's `gAIConfusedAttackChance` is 70 (investigator B's reading of Red; A did not check it). [blue-verified value and structure; red-hypothesis name for `+0xAC == 2`; high on the value, medium on the name]
- **Bide and charge block (P2.2).** If `+0xAE` is non-zero, take the move in slot `+0xB0` if it exists and `0215A2C0` accepts it, then set action `0x15`. Red's bide block is at `dungeon_ai_attack.c` lines 76-102. [blue-verified structure; medium. `0215A2C0` is not decompiled.]
- **Weights (P2.3).** Each move entry is 6 bytes at `+0xF4 + i*6`. Bit 0 means the slot exists; bit 2 means it is enabled for AI. The weight is `GetMoveAIWeight` (`0200B428`), which returns the move-table byte at `+0x0D` (stride `0x24`). The regular attack's weight is `table[numEnabled]`, where the pool word `0x021933BC` points to `0x021AA3A8`. The first two halfwords are 100 and 20, which match Red's `gAiRegularAttackWeights = {100, 20, 30, 40, 50}`. Entries 2 to 4 are not read. The weight is 2 when IQ bit 11 (Weak-Type Picker) is set. [blue-verified offsets and the first two values; red-hypothesis for the rest; high on offsets, medium on values past index 1]
- **IQ bits used here (P2.4).** `021805B0(entity, n)` tests bit `n` through `02004250`. The bit numbers match Red's `IQSkillID` enum (`IQ_NONE` = 0): 6 PP Checker (linked-move pass in `02192D24`), 11 Weak-Type Picker (weight filter and the `02192728` path), 22 Item Master (see [Item AI](#item-ai)), 23 Exclusive Move-User (regular attack suppressed). The base offset `+0x87` is the IQ-skill set, derived from IQ by `021804CC` (#164 P3b; see [`structs.md`](structs.md#entity-info)). [blue-verified bit numbers (through Red's enum) and base offset (#164 P3b); medium on the Red names]
- **Random-chance case: correction to the issue's premise (P2.5).** The issue names a "0x100 random chance". The value `0x100` is not a roll range. It is one value of the `0xF00` nibble of a move's AI field, which selects the condition. In `02192274` (`IsAITargetEligible`; Red's `src/dungeon_ai_attack.c:608`), `(param & 0xF00) == 0x100` marks the random case. The chance is `0200B3C4(move, 2)`, which reads the move table at `+0x0E + 2 = +0x10`, the accuracy array's index 2 (Red's `ACCURACY_AI_CONDITION_RANDOM_CHANCE`). The move is eligible when `DungeonRandInt(100) < chance`. Red's check has the same effect. [blue-verified; high]
- **Other `0xF00` cases (P2.6).** Only in the path where the caller's flag `param_5` is non-zero: `0x200` HP at or below 25% (`021818A4`); `0x300` a negative status (`02181904`); `0x400` asleep (`021818D0`); `0x500` HP at or below 25% or a status; `0x600` a Ghost-type target whose `exposed` byte `+0xD2` is clear (the type bytes `+0x56`/`+0x57` are both `0x0E`). The order and conditions match Red's `IsAITargetEligible`. [blue-verified structure; the helper bodies are not verified; medium-high]
- **Weighted pick (P2.7). Settled for Blue (#193 P3).** [blue-verified] Blue draws `r = DungeonRandInt(total)` (roll read at `0x0219325C`) and takes the first entry `i` with `cumsum[i] >= r`. The disassembly at `0x02193260`..`0x021932D0` shows `cmp r8, r6` followed by `blt`. In 263 forced picks over three weight layouts, every pick matched this rule; 210 matched the earlier rule `r < cumsum[i]`. The boundary checks gave the same result on both repeats. Entries with a zero flag or a zero weight are skipped. The earlier text gave `cumsum[i] >= roll` to Red; that is **[red-hypothesis]**, and Red's source was not read for #193, so no Blue/Red difference is claimed. Runs (scratch `ai_rng2.py`, force mode, script SHA-1 `1f6e144159e77538eae24a29b7c7ce12245b9415`): output SHA-1 `c72e111ab5b197e22851453b6f2596cdb61ecc37`, the repeat identical. Confidence: high for Blue. This row is also in [`blue-vs-red.md`](../blue-vs-red.md).
- **Weak-Type Picker filter (P2.8).** With IQ bit 11 set, only the entries with the maximum weight stay. [blue-verified; high]
- **Regular attack.** The regular attack is an extra entry after the move slots, with its weight from the table above. Its flag in the filter and pick loops is not verified (P2.9, see [Open questions](#open-questions)).

## Item AI

Entry point: `ov_0001::0218579C` (`AIDecideUseItem`). Red's counterpart is `AIDecideUseItem` (`src/dungeon_ai_items.c:46`).

- **Structure (P5.1).** The function calls `0218163C` and clears `+0xD9` if that returns true. Then:
  - If `+0xD9` is 0: loop over toolbox indices 1 to 21 (`0x16` exclusive). Index 1 is the held item (category value `0x81`). Indices 2 and up are the team inventory, 3 bytes each, from `gTeamInventoryRef` (the pool word `0x02185C6C` points to `0x020DECF8`). They are visited only when the team-leader flag `+0x07` is set; otherwise the function returns. The ground-item branch is dead, because the loop starts at 1, as Red's own comment says.
  - If `+0xD9` is not 0: the held-item branch. Category 1 is an arc throw; categories 2 and 3 are a throw along a line (action `0x23`); the last case is eating.
  - The item AI is gated by Item Master (IQ bit 22), which `021805B0(entity, 0x16)` tests.
  [blue-verified structure and bit 22; red-hypothesis for `+0xD9` as `useHeldItem`; high on structure, medium on the field name]
- **Self-use roll (P5.2).** For each inventory item with the exists flag set and the sticky flag clear (flags `&1`, `&2` and `&8` in the 3-byte entry), test AI-flag byte 0 (`020080A4(itemID, 0)`; the flags are listed in [`data-tables.md`](data-tables.md)). Get `w = 021473A8(entity, item, 2)` and roll `DungeonRandOutcome_2(w)`. Orb category 9 is skipped for non-team monsters before the roll. An orb gives action `0x31`; other items give `0x23`. [blue-verified flow; red-hypothesis for the meaning of the flag bits; high on the flow, medium on the flags]
- **Item weights (P5.3).** `021473A8(entity, itemPtr, flags)` is a switch on the item ID's low byte (`itemPtr+2`). It returns percentages, for example `0x46`, `0x32`, `0x28`, `0x50`, `0x64`, `0x1E`, `0x14`, `0x0F`, `0x0A` and `0x05`. Flag bit 0 gates some cases. It has the same shape as Red's `GetAIUseItemProbability` (`dungeon_ai_item_weight.c`); the cases are not compared one by one. [blue-verified structure; red-hypothesis for the mapping; medium]
- **Arc targets (P5.4).** `02185568(entity, item, out, ignoreRoll)` walks all 20 monsters, skips self, and requires `02180994(..., 0, 1) == 1` (target visibility, not read here) and a Chebyshev distance under 11. If `ignoreRoll` is 0, it rolls `DungeonRandOutcome_2(021473A8(target, item, 1))` for each target. Then `DungeonRandInt(count)` picks the target. This matches Red's `GetPossibleAIArcItemTargets` and `GetPossibleAIThrownItemDirections` (`dungeon_ai_items.c:256` and `:297`). [blue-verified structure; red-hypothesis for the mapping; medium-high]
- **Line targets (P5.5).** `02185690(entity, flagIndex, out, ignoreRoll)` loops over flag 1 (ally) and flag 2 (enemy). For each monster it calls `021853CC(entity, target, out, dir, ignore)`. Then it rolls `DungeonRandOutcome_2(table[i])` over the thrown-probability table, whose pool word is `0x02185C60` and whose address is `0x021B485C`, and calls `DungeonRandInt(count)`. [blue-verified structure; red-hypothesis for the flag mapping; medium]
- **Tables not in the file (P5.6). Not verified.** The thrown-probability table at `0x021B485C` is in overlay 1's BSS (`0x021B40A0`–`0x021B48FF`), so the file holds no values and no static writer was found. Its values need a runtime write trace; **runtime not run**, open, see #275 and [#194 P2b](#helpers-tables-and-open-items-194). The count at `0x021B3F44` is not a table count: it is a runtime counter of arc-target candidates (P2a in the same section). Red's `gAIThrownItemProbabilities` is the candidate for the table; not verified. The earlier candidate `gAIThrowItemActionChoiceCount` is wrong for `0x021B3F44`.
- **RNG order (P5.7).** Self-use: one `DungeonRandOutcome_2` per eligible item. Arc: one draw per target in `02185568` when the roll is not ignored, then `DungeonRandInt(count)`. Line: `DungeonRandOutcome_2` per direction, then `DungeonRandInt(count)`. [blue-verified; high] The full list is in [RNG calls](#rng-calls).

## Movement and pathfinding

- **Dispatcher (P3-a).** `ov_0001::021918D4` (`AIMovement`, red-hypothesis name) clears `+0x71`, `+0x72` and `+0x73` on entry (`aiNotNextToTarget`, `aiTargetingEnemy`, `aiTurningAround`, as Red's `AIMovement` does). Then it applies the gates: BE_PATIENT (tactic 7) with HP at or below max/2 gives no action; WAIT_THERE (tactic 9) gives no action; a non-leader (`+7 == 0`) with belly 0 (`+0x10E`) gives no action. Then:
  - if `+0x98 == 1`, a random walk with `DungeonRandInt(8)`;
  - otherwise the run-away test `02180B68`, then either `AvoidEnemies` (`02190B80`) or the item gate (`02191800`, see [Item AI](#item-ai)) and target search (`02191394`);
  - finally `DecideMovement` (`02190FEC`) if a target was found.
  [blue-verified; high; the names are red-hypothesis]
- **Run-away predicate (P3-b).** `ov_0001::02180BA0` (named `AIShouldRunAway`, tentative; wrapper `02180B68`, which also calls `02180AF4` to show the ability's effect). It returns true when: the ability `'+'` is active and HP is below max/2; tactic 10 (GET_AWAY) is set; or tactic 6 (AVOID_TROUBLE) is set and HP is at or below max/2. The ability is not identified by name. [blue-verified; high for the predicate]
- **Target search (P3-c).** `ov_0001::02191394` (`AIFindTarget`, Red's name). It runs a candidate loop over the team or wild list (0x14, 0x10 or 4 entries) and picks the nearest valid target. It sets `+0x70` to 1 (`AI_CHASE_TARGET`), the target position at `+0x80`/`+0x82`, the target pointer at `+0x78`, the generation ID at `+0x74`, and `+0x72` to 1 (`aiTargetingEnemy`).
  - If a candidate is found, tactic 8 (KEEP_YOUR_DISTANCE) is set, no teammate is visible (`02180444`), and the target is within one tile in x and y, then `+0x73` (`aiTurningAround`) is set to 1.
  - If tactic 1 (GO_THE_OTHER_WAY) is not set and the entity is a team member (`+6 == 0`), chase the visible leader from `02183FF0` (`GetLeaderIfVisible`).
  - If tactic 1 is set and the entity is the leader (`+7 != 0`), walk the 3x3 area for an item tile (entity type 3) and target it. Red's comment calls this branch unreachable, because the leader is never AI-controlled. In Blue it is reachable in the code; see P4-g.
  #164 P2b: the whole function matches Red's target search in `dungeon_ai_movement.c` (about lines 140 to 296), branch by branch, including the chase and remembered-target block at Red 277 to 296 (objective 1; then objective 2 with the 4 previous positions, or else 6). Its end calls `02190854` (`Wander`). The accessor list is in [structs.md](structs.md#aitarget-readers-164).
  [blue-verified code; high for the structure; names red-hypothesis]
- **Avoid enemies (P3-d). Not verified.** `ov_0001::02190B80` (red-hypothesis name `AvoidEnemies`, medium). It picks the enemy with the largest distance and steps away from it, or falls back to the wander routine `02190854`. Red's source says its own `AvoidEnemies` does not match Blue (the note cites decomp.me `QNmaT`). This shape agrees with that note. The function is not named in [`symbols.md`](../symbols.md) yet.
- **Decide movement (P3-e).** `ov_0001::02190FEC` (`DecideMovement`) takes the target from `+0x80`/`+0x82`, and the direction from `0201187C` (`GetDirectionTowardsPosition` in [`symbols.md`](../symbols.md)). If `+0x73` is set, it adds 4 to the direction. Tactic 3 (AVOID_THE_FIRST_HIT), through `02180C6C`, keeps a distance of 2 and turns 4.
  - It tries the direction, then ±45°, ±90° and ±135°, using `02180DF8` (`CanAIMonsterMoveInDirection`). That function tests, in order: walls; the monster house (terrain `0x40` with `gDungeon+0x661`); traps (IQ 14, TRAP_AVOIDER); lava (IQ 20, LAVA_EVADER); all-terrain (IQ 12); super mobile (IQ 13). Red's order is the same.
  - **Blocked in front.** If `+6 == 0` (the team-member test used in P3-c and P4-a) and `recalculateFollow` (`+0x117`) is 0, it sets `+0x71` = 1, the `aiAllySkip` candidate `+0x116` = 1, and waiting `+0x119` = 1, with a pass action. Red's `DecideMovement` does this at lines 358-361.
  - **Trailing branch (no move found).** A leader (`+7 != 0`) clears `+0x71` and `+0x116`. A non-leader with `+0x71` set sets `+0x116` = 1. Red's lines 436-443 match.
  [blue-verified flow; high; field names medium]
- **`aiAllySkip` writers (P3-f).** The offset is `+0x116`, which is Red's `0x144` shifted by −0x2E. The writers are the two sites in P3-e. It is also cleared in `RunMonsterAI` after the action branch (with `+0x71` and `+0x119`). The readers were not located; they should be in the turn loop, which is [#49](https://github.com/TorratDev/rust-mm/issues/49)'s scope. [red-hypothesis for the field identity; medium-high; no trace]
- **Wander (P3-g).** `ov_0001::02190854` (`AIWander`, tentative). It picks a random direction with `DungeonRandInt(8)`, tests each direction with `02180DF8`, and falls back to a random target and state 6. Its RNG calls are in [RNG calls](#rng-calls). [blue-verified; high]

## Ally AI

- **Follow the leader (P4-a).** `ov_0001::02183FF0` returns 0 when the entity is not a team member (`+6 != 0`), returns 0 when the leader's `+0xB3` is 2 or the visibility check fails, and otherwise returns the leader. It is called from `02191394` (P3-c). [blue-verified; high]
- **Tactics read by the ally path (P4-b).** The IDs are blue-verified; the names are red-hypotheses.

  | Tactic | Where it is read | Effect |
  |--------|------------------|--------|
  | 1 GO_THE_OTHER_WAY | `02191394`, leader branch | no leader follow; the leader searches for an item |
  | 3 AVOID_THE_FIRST_HIT | `02180C6C`, called from `02190FEC` | keeps a distance of 2, turns 4 |
  | 6 AVOID_TROUBLE | `02180BA0` | runs away at HP at or below max/2 |
  | 7 BE_PATIENT | `021918D4` | no action at HP at or below max/2 |
  | 8 KEEP_YOUR_DISTANCE | `02191394` | sets `+0x73` when adjacent to the target and no teammate is visible |
  | 9 WAIT_THERE | `021918D4` | no action |
  | 10 GET_AWAY | `02180BA0` | runs away |

  Tactics 0 (LETS_GO_TOGETHER) and 2 (GO_AFTER_FOES) were not seen in these functions. They were not searched for; they may be in the P1 or P2 code.
- **IQ skills (P4-c).** `021805B0` is a tail call to `IsIQSkillSet` (`02004250`) with the entity's IQ field (`+0x87`) and a one-bit mask. The IDs match Red's `IQSkillID` enum: in `02180DF8` the order is 12 ALL_TERRAIN_HIKER, 13 SUPER_MOBILE, 14 TRAP_AVOIDER, 15 HOUSE_AVOIDER and 20 LAVA_EVADER, the same as Red's `CanAIMonsterMoveInDirection`. IQ 21 (DEDICATED_TRAVELER) is tested in `RunMonsterAI` step 10. [blue-verified bit test and IDs; blue-verified for the field offset `+0x87` (#164 P3b; Red's IQ flags are at `0x90`)]
- **Petrified-ally swap flag `0x8000` (P4-d).** The writer is `ov_0001::02165C80`, which has the same shape as Red's `sub_805EC4C` (`src/dungeon_main.c:854-925`). It checks the adjacent tile's entity (type 1), its statuses and a Yes/No prompt. It sets both actions to walk (`021418AC(..., 2)`), gives the petrified ally direction +4, sets the targets, and writes `|= 0x8000` to the walker (the leader) and to the petrified ally. It also sets `gDungeon+0xE` to 1. [blue-verified writes, high; red-hypothesis identity, medium]. Cleared by `02140ED0` (turn loop), which then sets `0x4000`; see [Swap flags](turns.md#swap-flags-181-p2). The AI path does not set it in the swept range (#181 P2).
  - Other writers in the scan: `ov_0001::02175178` clears `0x8000` for team members chosen in a menu (`&= 0x7FFF`) and sets it on option 0. Its purpose is not identified (menu code). **[open]:** #181 investigator B's scan found no store from `02175178` under its patterns, so this claim is not confirmed.
  - ARM9 hits at `020191A0`, `02026370`, `02026810`, `02074FC8` and `0207F5A0` were not checked; they are likely unrelated.
  - The reader is step 1 of `RunMonsterAI`.
- **Ally swap flag `1 << 5` (P4-e). No Blue writer found.** Red's enum has `MOVEMENT_FLAG_SWAPPING_PLACES` (`dungeon_entity.h:368`), but `pmd-red`'s source never assigns it. In Blue, no decompiled line ORs `0x20` into the info word at `+0`. The `0x20` hits are tile flags (`GetTileMut`, `0214D448` and `0214CA3C`), move flags (`+0xF4` and `+0x10C`, in `0215D9B8` and `0215F0A8`) and a `u16` at `+0x144` in `0217E890` (`HandleDealingDamage_Async`). None of them is the movement word. Confidence medium: a store written in a form the decompiler folds differently would be missed. Tracing the swap action would confirm it. #181 P2 (investigator B) scanned the decompiled functions of `ov_0001` from `02138860` to `021921B0`, about 73%, and found no writer; ARM9 and the other overlays were not swept ([turns.md](turns.md#swap-flags-181-p2)).
- **Dedicated Traveler order (P4-f). Resolved by #164 P1b.** Investigator C reads Red's `RunMonsterAI` as skipping the move choice when IQ 21 is set, going straight to `AIMovement` (`dungeon_ai.c:101`). Blue's code calls `02192D24` in both IQ branches and calls `021918D4` only after a check. Investigator A found the same structure as Red in both branches. Red's source supports A's reading, and Blue matches it in both branches. This is static evidence only; no runtime check has been run.
- **Leader item search (P4-g). Reachability open.** The leader's item search in P3-c is reachable in Blue (leader with GO_THE_OTHER_WAY). Red says it is unreachable. Whether a leader ever runs `RunMonsterAI` in Blue is part of P1. [blue-verified code; reachability open; medium that the branch is live]

## Helpers, tables and open items (#194)

Static only. Source: investigator A's read-only reads of overlay 1 and the ARM9 binary (Ghidra copy `rescue-arm9-a`, nothing renamed), and the decompiles and disassembly named below. **No runtime check was run** in this section; each runtime item is marked "runtime not run". Open items are tracked in [#275](https://github.com/TorratDev/rust-mm/issues/275).

**Resolved: no RNG reach (P1a, P1b, P1c call 1). [static]**

- **P1a, `ov_0001::0213A348`.** Tail-jumps through the literal at `0213A374` (`0x0202D240`). That is a field reset: it clears fields at `0x02133C94`, calls `02012314` (a no-op) and `02017D34` (field init). No RNG call is reached. Evidence: disassembly of `ov_0001::0213a348`; literal read at `0213a374`; decompiles of `0202d240`, `02012314` and `02017d34`.
- **P1b, `02086468` and `02086470`.** Computed jumps into an unrolled ARM divide kernel at `0x020864C4`–`0x0208664C` (`adcs`/`subcc`, `bx`, no `bl`). The `blx` at `0x0208666C` belongs to a separate list walker. No RNG call is reached. Medium-high: the instruction walk stopped at 140 instructions.
- **P1c, `02076AEC`, call 1.** The literal `0x02076B2C` is `0x0207C94C`, a struct copy with no calls. No RNG call is reached. Call 2 is open (see below).

**Counter and table (P2a, P2b). [static]**

- **P2a, `0x021B3F44`.** Not a table count, and not `gAIThrowItemActionChoiceCount`. It is a runtime counter of arc-target candidates, read through the pointer `DAT_ov_0001__02185688`. `FUN_02185568` resets it (write at `02185588`) and increments it for each valid target (write at `02185670`). It is read in `FUN_0218579C`. The file holds 0 at that address. High confidence. Runtime value: not run.
- **P2b, table `0x021B485C`.** In overlay 1's BSS (`0x021B40A0`–`0x021B48FF`), so the file holds no values. Static reads: `FUN_0218579C` (`0218581C`, `02185BE8`), and a DATA reference from `FUN_021853CC` (`02185548`). The pointer slot `0x02185C60` holds `0x021B485C`. No static writer found. The values need a runtime write trace: **runtime not run**; open, see #275.

**Helper structures (P3a to P3g). [static]** These are #194's labels, separate from the movement labels P3-a to P3-g above. Field names are not invented here; the offsets are the bytes as read.

- **P3a, `ov_0001::021818A4`.** Returns `(+0xE s16) <= FUN_0208625C(+0x10 s16, 4)`, a signed divide by 4. The field identities are not verified, so there is no "low HP" name yet.
- **P3b, `ov_0001::021818D0`.** True when the mode byte at `+0x99` is 1, 3 or 5.
- **P3c, `ov_0001::02181904`.** Returns 1 when `+0x99` is 1, 3 or 4. Otherwise it checks `+0x9B`, `+0xA0` (5 or 0), `+0xAC`, `+0xB3` (1 or 2), `+0xB8` (1), `+0xC4` (2), `+0xDA`, the move slots at `+0xF4 + 6i` (flag bits 0 and 1) and `+0xE9` to `+0xED`. It returns 0 only if all of them fail.
- **P3d, `ov_0001::02191A30`.** Returns `0x73` when the item ID at `+2` is `0xED`, `param_3 == 0` and `FUN_02180644(entity, 0xE) == 0`. Otherwise it returns `FUN_0200B4EC(param_2, param_3)`.
- **P3e, `ov_0001::02180644(entity, n)`.** True when `n == 0`, or when `n` equals the AI-struct byte `+0x56` or `+0x57`.
- **P3f, `ov_0001::0215A2C0` (bide match).** If `FUN_02142AE4` is nonzero, it scans up to 100 four-byte rows at `DAT_0215A340` (a `u16` ID, and a byte at `+2`). It returns 1 when the ID matches a row and the AI-struct byte `+0xAE` matches that row's byte. The row values are not read.
- **P3g, `ov_0001::02190FEC`, labelled "team-member gate". Label unverified.** It tests AI-struct `+6 == 0` and `+0x117 == 0`. The writer of `+6` was not found, so the label is not confirmed.

**Open, see #275. [static, incomplete]**

- **P1c call 2, `02076AEC`.** Through `*(param+0x3c) - 4`. Runtime only. Open, see #275.
- **P1d, `020731AC`.** A module loader. `FUN_02073584` calls it when flag `0x1000000` is set. Its entry `*(puVar5+7)` is read from the file header by `FUN_020725DC`. The target is file data, so it needs a runtime check (**runtime not run**). Open, see #275.
- **P1e, `02076280`.** Dispatches through slots `0x0207635C`–`0x02076368` (pointers in the `0x021353A0` region, set at runtime) and a computed jump at `0x020762E0` (the switch table is not read). Open, see #275.
- **P1f, `0207D500`.** Walks a linked list, calling `(*node)(node[1])`. Callers: `FUN_0207D704` (two sites), reached from `FUN_02019794`. The callbacks are registered at runtime (**runtime not run**). Open, see #275.
- **P2b values, table `0x021B485C`.** Needs a runtime write trace (**runtime not run**). Open, see #275.
- **P3h, stack aliasing in `02192D24` (P2.9).** Unresolved. The locals are `auStack_5c` (8 bytes), `local_50[4]`, `local_4c[4]`, `local_48[7]` and `local_2c`. The stride-8 claim is not checked. Open, see #275.
- **P2.3 table entries 2 to 4.** Not read. Open, see #275.

## RNG calls

Every `DungeonRand*` call reached from `RunMonsterAI`, in the order the code runs, with its gate. The walk was static: a closure from `02188030` over direct and pool-resolved calls. It reached 684 functions, and 26 of them call an RNG directly. Only the first parent chain found for each node is recorded, so a site may have other paths. Depths are given only where the walk stated them. The walk left seven indirect call sites unresolved (see P6-g). The walk output is not in the repo.

| # | Site (routine) | RNG call | Gate | Red comparison |
|---|----------------|----------|------|----------------|
| 1 | `RunMonsterAI`, rescue branch | `DungeonRandInt(8)` | `+0x98 == 1` | `dungeon_ai.c:92`, `DungeonRandInt(NUM_DIRECTIONS)` |
| 2 | `0218579C` (`AIDecideUseItem`), first or second call | `DungeonRandOutcome_2(pct)` in the loop of `020080A4` and `020082FC` | IQ 22 path with `+0xD9 == 0` | `dungeon_ai_items.c:78`, approximate |
| 3 | `0218579C` | `DungeonRandInt(*DAT_021B3F44)` | IQ 22 path, `iVar7 == 1`, after `02185568`; the cap is the runtime arc-target counter at `0x021B3F44` (P2a), not a table count | `:103` and `:222`, approximate |
| 4 | `0218579C` | `DungeonRandOutcome_2(table[i])`; the pointer at `0x02185C60` gives the table at `0x021B485C` | loop, `+0xD9 == 0` path | `:180` and `:238`, approximate |
| 5 | `0218579C` | `DungeonRandOutcome_2(table[i])` | `+0x5A & 1` path | `:331`, approximate |
| 6 | `0218579C` | `DungeonRandInt(*DAT_021B3F44)` | `+0x5A & 1` path, `iVar4 == 1`, after `02185568`; the cap is the arc-target counter at `0x021B3F44` (P2a) | approximate |
| 7 | `02180048` (`HasStatusThatPreventsActing`) | `DungeonRandInt(8)` | decoy curse (`+0xB3 == 2`) | exact (decoy branch of `status_checks.c`) |
| 8 | `02180048` | `DungeonRandInt(2)` | blinker (`+0xC9 == 1`), cannot move | exact (blinker branch) |
| 9 | `02180048` | `DungeonRandInt(8)` | blinker, random direction | exact |
| 10 | `02180048` | `DungeonRandInt(8)` | cross-eyed (`+0xC9 == 2`) | exact |
| 11 | `02192D24` (`ChooseAIMove`) | `DungeonRandOutcome_2(70)` | `+0xAC == 2` (confused); the 70 is at `0x021AA184` | `dungeon_ai_attack.c:72`, exact flow |
| 12 | `02192D24` | `DungeonRandInt(total)` | weighted move pick | `dungeon_ai_attack.c:275`, `:519`, `:815` |
| 13 | `021918D4` (`AIMovement`) | `DungeonRandInt(8)` | `+0x98 == 1` | `dungeon_ai_movement.c:74` |
| 14 | nested under `02192D24`: `02192728`, `02191F84`, `02192274` | `DungeonRandInt` (2 sites) | not stated | red attack and move helpers |
| 15 | nested under `021918D4`: `02190B80`, `02190854` (6 sites), `02180C98` | `DungeonRandInt` | not stated | red movement helpers, `dungeon_ai_movement.c:523-722` |
| 16 | nested under `0218579C`: `02185568` (1 site) and `0219CFF0` | `DungeonRandOutcome_2` | not stated | the AI callers of `DungeonRandOutcome_2` in [`rng.md`](rng.md) |
| 17 | global LCG, through message display | `Rand32Bit` in `0217B120` (2), `02155760` (1), `0217AF58` (2) and `02012648` (1); `RandInt` in `0219DCF0` (2) | not reached in the #193 run (`02154BF0` never ran; see P6-f). Static chain: `02154BF0` → `02154910` → `0213AFA8` → `0213AD54` → `02142504` or `0219E89C` | none: Red has no AI message RNG; see P6-f |

Notes:

- **P6-a to P6-c.** Row 1 is the only `RunMonsterAI` RNG call of its own. Rows 7 to 10 are the four RNG sites of the status gate, all exact against Red. The item-use routine has 5 direct sites in Blue (rows 2 to 6); Red's `AIDecideUseItem` has 6 across both item categories. The site-by-site mapping is approximate. P5 reconciles it. [blue-verified sites; high for the status gate; medium for the item mapping]
- **P6-d.** The confusion roll (row 11) uses `DungeonRandOutcome_2(70)`, with 70 read from overlay bytes. Red's value of `gAIConfusedAttackChance` is not checked in A's notes. [blue-verified value; high]
- **P6-e.** `DungeonRandOutcome_2` has these AI callers: `0218579C` (3 sites), `02185568` (1 site, depth 2) and `02192D24` (1 site). This confirms the list in [`rng.md`](rng.md). [blue-verified; high]
- **P6-f. Global LCG through messages: not confirmed (#193 P2).** [blue-verified for what ran; medium confidence] The message display `02154BF0` never ran in the #193 run (4700 frames of combat, 303 `RunMonsterAI` calls). Its callee `02154910` ran 101 times from other callers. In all 101 calls there was no RNG hit, and the global LCG state did not change. `0213AFA8` and `0213AD54` run about once per frame (4577 and 4576 entries), and no call to either happened inside a `02154910` call. The global LCG draws in the run come from per-frame code: `Rand32Bit` (its return addresses `0x02011994` and `0x0201199C`), the frame handler call `0x02018B04`, `0x02018B0C`, `0x02155820`, `0x0217B2C4` (the `0217B120` sprite function) and `0x02149594`, most of them inside the per-frame calls. So the static chain `02154BF0` → `02154910` → `0213AFA8` → `0213AD54` → `02142504` → `0217B120` is not a message-display path in this run. Caveats: `02154BF0` itself was not hit; `02154910` may queue text that the frame routine draws later; the "couldn't be used" message (RunMonsterAI step 4) was not shown. **[hypothesis]** `02154BF0` draws nothing, because its only body is a gate and a call to `02154910`, which drew nothing in 101 calls. Runs (scratch `ai_rng2.py`, observe mode): output SHA-1 `697496e31934b34cd8990eb69271d440b79fd5f2`, the repeat identical; a pilot run without the `02154910` hook gave output SHA-1 `d078eaeb6fa641803a96d026ab03f94e6ba97fb7`. The same note is in [`rng.md`](rng.md), which this change does not edit.
- **P6-g. Gaps.** The walk left seven indirect call sites unresolved. They can hide RNG calls. See [Open questions](#open-questions).

## Open questions

- **P6-g, unresolved call sites.** A counts nine functions with indirect calls. #194 P1 resolved three of the seven sites (`0213A348` (P1a), `02086468` and `02086470` (P1b)) and call 1 of `02076AEC` (P1c), statically, with no RNG reach. Four remain open, see #275: `020731AC` (depth 11, P1d), `02076280` (depth 8, P1e), `0207D500` (depth 7, P1f), and `02076AEC` call 2 (depth 9, P1c). `0208625C` (depth 2, a signed-division helper) and `021644B8` (depth 3, a switch predicate) were read by hand and do not call the RNG. Details are in [Helpers, tables and open items (#194)](#helpers-tables-and-open-items-194).
- **P6 walk limits.** Only the first parent chain is recorded per node. The closure is static, so calls made through pointers that were not resolved are missing.
- **P5.6 tables (not verified).** The thrown-probability table at `0x021B485C` needs a runtime write trace (**runtime not run**). Open, see #275 and [#194 P2b](#helpers-tables-and-open-items-194). The count at `0x021B3F44` is a runtime arc-target counter (P2a), not a table count.
- **P5 and P6-b count.** Blue's `0218579C` has 5 direct RNG sites; Red's `AIDecideUseItem` has 6 across both item categories. P5 must reconcile this.
- **P2.7 weighted pick (Blue settled, #193 P3).** The Blue rule is confirmed at runtime (see the P2.7 note above). The Red side is not checked: no Red source was read. Check Red's source before a port copies its rule.
- **P2.9 decompiler artifact (unverified).** The filter and pick loops in `02192D24` index a local at stride 8 up to entry 4 (the regular attack), but only entries 0 to 3 are written in the visible code. Stack aliasing is likely. Treat the regular-attack flag in those loops as unverified until the stack layout is checked in Ghidra. Open, see #275 (#194 P3h).
- **P2.2 and P2.6 helpers.** `0215A2C0` (bide match) is described in [#194 P3f](#helpers-tables-and-open-items-194), but its row values are not read. The bodies of `021818A4`, `02181904`, `021818D0` and `02191A30` are described in P3a to P3d of the same section; the field identities are not verified.
- **P2.3 table entries 2 to 4.** Not read. Open, see #275.
- **P1-c offsets.** `+0xB3` and `+0xB4` are not checked against [`structs.md`](structs.md). Open, see #275.
- **P1-h caller.** How the turn loop reads the action in `+0x42` after `RunMonsterAI` returns. [#49](https://github.com/TorratDev/rust-mm/issues/49)
- **P3-d `AvoidEnemies`.** The mismatch with Red's source is noted but not checked. The name is not in [`symbols.md`](../symbols.md).
- **P3-e gate wording.** Investigator C calls the `+6 == 0` gate "non-team-member", which disagrees with its own P3-c and P4-a, where `+6 == 0` is the team-member test. The code test is `+6 == 0`.
- **P3-f readers of `aiAllySkip`.** In the turn loop, [#49](https://github.com/TorratDev/rust-mm/issues/49).
- **P4-d identity of the `0x8000` writer and the menu writer `02175178`.** The name of `02165C80` rests on Red's `sub_805EC4C`. The purpose of `02175178` is not identified. ARM9 hits not checked.
- **P4-e swap flag `1 << 5`.** No writer found. A trace of the swap action would settle it.
- **P4-g leader item search.** Whether a leader runs `RunMonsterAI` in Blue.
- **Red's `gAIConfusedAttackChance`.** The value 70 is read from Blue; the Red constant is cited as 70 by investigator B and was not checked by investigator A.
- **Shared helpers outside this page.** `0215A2C0`, `02180994` (target visibility), `02163AD4`, `02181904`, `021818A4`, `021818D0`, `02191A30` and `021853CC` are used here and belong to [#49](https://github.com/TorratDev/rust-mm/issues/49), [#50](https://github.com/TorratDev/rust-mm/issues/50) and [#51](https://github.com/TorratDev/rust-mm/issues/51).
- **Runtime check (#193, partial).** A py-desmume hook on `RunMonsterAI` entry (`0x02188030`) counted the RNG hits in each call (see [Runtime check (#193 P1)](#runtime-check-193-p1) below). A hook on the message display `02154BF0` was set; it never ran, so P6-f is not confirmed. Rows 1, 2 to 6, 7 to 11, 13, 15, 16 and 17 are still static only.

### Runtime check (#193 P1)

**Method.** Script `ai_rng2.py` (scratch, not in the repo; SHA-1 `1f6e144159e77538eae24a29b7c7ce12245b9415`), built on `verification/v7-trace/retail_trace.py`, on a scratch copy of the supported dump. It boots to the first dungeon frame, then plays 4700 frames of combat on a new game's first floor. Hooks: `RunMonsterAI` entry (`0x02188030`), `0x02154BF0`, `0x02154910`, `0x0213AFA8`, `0x0213AD54`, and every `DungeonRand*` and `Rand*` entry. Each RNG hit is credited to every call active at that moment. Runs used `XDG_CONFIG_HOME` in a scratch folder. Output SHA-1 (observe mode, run o1): `697496e31934b34cd8990eb69271d440b79fd5f2`; the repeat o2 is identical.

**Results** [blue-verified; high for the counts; one floor, one behaviour value, no items]:

- 303 `RunMonsterAI` calls, all with `+0x98 = 0`. `+0x06` was 1 in 154 calls and 0 in 149. Every call returned with a balanced stack.
- **Only `DungeonRandInt` was drawn.** Per call: 284 calls drew once, 17 drew twice, 1 drew three times and 1 drew four times. `DungeonRand16Bit` hits equal the `DungeonRandInt` hits in every call, and the dungeon LCG advanced by exactly that many steps in all 303 calls (checked by iterating the LCG).
- **No other draws.** No `DungeonRandOutcome`, `DungeonRandOutcome_2` or `DungeonRandRange`. No `Rand16Bit`, `Rand32Bit`, `RandInt` or `RandRange`: the global LCG did not change inside any call.
- **The first draw is the weighted pick (row 12).** Its cap is the total of the weights, read at `0x0219325C`, in all 303 calls.
- **Extra draws after the pick (row 14, nested helpers).** Caps 8 (11 calls), 1 (3 calls), 3 (3 calls), and the pairs 3, 3 and 3, 3, 3 (one call each). They follow the pick, which fits `02191F84` and `02192728`. The caps are not mapped to functions.
- **Action in `+0x42`.** Unchanged in most calls (2 to 2 in 151, 1 to 1 in 144). Six calls ended with 0x15 (move) or 0x32 (regular attack) from another value. The values 1 and 2 are not decoded.

**Corrections to the table above.**

- Row 12 is confirmed as the first draw of the typical turn: 284 of 303 calls draw once, and that draw is the pick.
- Rows 1 and 13 (`+0x98 == 1`) were not exercised: every observed call had behaviour 0.
- Rows 2 to 6 (items), 7 to 10 (status gate), 11 (confusion), 15 and 16 were not exercised. Row 17 was not reached (see P6-f). The static list is not contradicted, but these rows are unverified at runtime.

**Repeats.** Force mode (run f1, forcing the weighted-pick roll): output SHA-1 `c72e111ab5b197e22851453b6f2596cdb61ecc37`; the repeat f2 is identical. Used for P3 above. A pilot run with an earlier script version (no `02154910` hook) gave the same AI counts; its output SHA-1 is `d078eaeb6fa641803a96d026ab03f94e6ba97fb7`.

**Not verified.** Rows not exercised (above). The caps of the nested draws are not mapped. The `+0x42` values 1 and 2. A run with a rescue target, a confused or blinded monster, or items would exercise rows 1 to 11.

## Links

- Symbols: [`symbols.md`](../symbols.md)
- RNG: [`rng.md`](rng.md)
- Turn loop: [`turns.md`](turns.md)
- Structs: [`structs.md`](structs.md)
- Blue vs Red: [`../blue-vs-red.md`](../blue-vs-red.md)
- Data tables: [`data-tables.md`](data-tables.md)
- RunMonsterAI, `aiTarget` and the IQ field: [#164](https://github.com/TorratDev/rust-mm/issues/164), results in [`structs.md`](structs.md)
