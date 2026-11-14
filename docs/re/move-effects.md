# Move effects: dispatch and handler catalogue

Issue: [#50](https://github.com/TorratDev/rust-mm/issues/50). Part of the combat notes in [`combat.md`](combat.md), which covers the damage formula and the move-effect entry point. Addresses in this page are **blue-verified** (read in Blue's code or by the runtime check named below). Handler names in the Red column are **red-hypothesis**, mapped from `pmd-red` by move ID and not yet checked against each Blue handler.

## Dispatch

- The dispatch is `ov_0001::02156824`. It is the Blue counterpart of Red's `UseMoveAgainstTargets`. It switches on the move ID, a `u16` at `move+2` (`*(u16 *)(move + 2)`).
- Callers: `ov_0001::0215B384` (move executor) and `ov_0001::0215AB34`.
- The switch has 413 case IDs. The other 406 IDs go to one of 262 handler addresses, and seven IDs have no handler. Handler bodies are not catalogued here.
- Runtime (py-desmume, run by investigator C on the dump with SHA-1 `6f01887356268de39085df01490347bbcc3aa330`; 3,000 frames after the dungeon starts): the entry fired 68 times, with move IDs `0x09A`, `0x0D9`, `0x104` and `0x163`. That is the only runtime evidence here. The other IDs are static.
- One `switch`, not nine. In `ov_0001::02156824` there is one `switch` on the move ID at `move+2`, with case values from 0 to 0x19C (**blue-verified**, investigator B's findings on [#355](https://github.com/TorratDev/rust-mm/issues/355)). The other eight text hits of `switch` are the default label and seven `goto`s to it (case IDs 0x166, 0x167, 0x192, 0x194, 0x195, 0x198 and 0x19B; case 0x164 falls into the label). All of them set a local to 0, so they have no effect. The ID checks before the switch are plain `if`s (for example 0x89 and 0xFC) and are not catalogued here.
- The miss path and the first dungeon's handlers are in [First dungeon: move list, handlers and draws](#first-dungeon-move-list-handlers-and-draws-357). Not verified: Red's miss path (`dungeon_move.c` around line 290, Hi Jump Kick and Jump Kick), and the handler bodies outside that list. The hit/miss shape is in [`combat.md`](combat.md#dispatch-shape-and-hitmiss-branch-p2-186). `HandleDamagingMove` (`ov_0001::0215675C`) is the handler for 60 IDs in the catalogue below.

## Roll wrapper per move ID (P3, #186)

Investigator A's findings. Tag: **blue-verified** (decompile and x-ref). Confidence: **high** for the wrapper semantics, **medium-high** for the move mapping (direct callers and one call level only).

- `DungeonRandOutcome` (`ov_0001::0219CFCC`) returns `DungeonRandInt(100) < percentChance`. `DungeonRandOutcome_2` (`ov_0001::0219CFF0`) is the same code.
- `FUN_02159F10(entity, chance)`: returns false if the entity is invalid. `chance == 0` returns true (guaranteed). Otherwise it rolls `chance`, or `chance << 1` when the entity has ability `&` (the findings name it only as `&`).
- `FUN_02159F6C(user, target, chance)`: returns false if the floor is over, if either entity is invalid, if the target's `info + 0x12A` is 0, or if the target's HP (`info + 0xE`) is 0. `chance == 0` returns true; otherwise it rolls as `FUN_02159F10` does. If the roll succeeds, the target is not the user and the target has ability `L` (named only as `L` in the findings), it calls `FUN_0213EB00(target)` and returns false.
- **Callers.** `FUN_02159F10` has 14 call sites and `FUN_02159F6C` has 49 (63 in total; 19 pass 0, 44 pass a table constant; see [combat.md P4](combat.md#p4-outcome-wrappers-and-ai-chance-sources)). Together they have 62 distinct caller functions, including the dispatch `FUN_02156824` (see [P2](combat.md#dispatch-shape-and-hitmiss-branch-p2-186)).
- **Callers of `DungeonRandOutcome_2`.** `FUN_02192D24` (1 site) is AI code: it reads `info+0xAC` and `IsTacticSet`. The other callers are `FUN_0218579C` (2 sites), `FUN_02185568` (2), and two more via `FUN_02188030` and `FUN_02180048`. They were not decompiled, so their code is not recorded.
- **Move mapping.** 63 move IDs (case values in `ov_0001::02156824`) reach a roll wrapper through one call level. The move ID is the case value, since the repo has no move-name table. Examples:

  | Move ID | Callee | Wrapper |
  |---------|--------|---------|
  | `0x10` | `FUN_0215DD2C` | `FUN_02159F6C` |
  | `0x19` | `FUN_0215D8A8` | `FUN_02159F6C` |
  | `0x2C` | `FUN_0215D3CC` | not recorded |
  | `0x67` | `FUN_0215FC90` | not recorded |
  | `0x1` | `FUN_0215E160` | not recorded |

  The full map is in the #186 scratch folder (not in the repo). It has one false match, the case `0x19C` entry, caused by a trailing name in the decompile's JSON output. That entry is excluded from the count of 63.

**Not verified**

- Calls two or more levels below a case callee are not counted.
- The move names for the IDs (no table in the repo).
- Whether each case's callee uses the roll for accuracy, for the secondary effect, or for something else.
- `FUN_0218579C`, `FUN_02185568` and `FUN_02188030` were not decompiled.
- Runtime values of the chances: no trace. A `register_exec` hook on `FUN_02159F10` and `FUN_02159F6C` could confirm them.

## First dungeon: move list, handlers and draws (#357)

Scope: the moves that can be in play on the first dungeon's floors 1 to 3 (dungeon 0). Source: investigator C's findings on [#357](https://github.com/TorratDev/rust-mm/issues/357) (Ghidra copy `rescue-arm9-c`, export of 2026-11-12). The runtime runs were made by investigator C from the first floor's save with py-desmume; their scripts are in the investigator's scratch folder, not in this repo, so the runtime tags below cannot be rerun from here yet (see [open points](#open-points-and-conflicts-357)).

Tags: **blue-verified** (an address read in Blue, or a runtime check named in the row). **lead** (a pointer or an assumption that is not checked; the findings' "hypothesis" items are recorded as leads). Values from table words (pool chances, spawn weights, learnset levels beyond the bins below) are not copied into this page.

### P1: move list (#357)

Floors 1 and 2 share one spawn list and floor 3 adds one species; index 0 of the floor table is a placeholder. **blue-verified** (static, `rescue_data::dungeon::parse_mapparam`; runtime on floor 1 for the species seen).

| Group | Moves (ID and name) | Tag |
|-------|---------------------|-----|
| Wild, seen at runtime on floor 1 | `09A` Tackle (Wurmple), `087` Absorb (Sunkern) | **blue-verified** (runtime) |
| Wild, static only | `09A` Tackle (Pidgey), `06F` Hypnosis (Exeggcute, floor 3 only) | **blue-verified** (static spawn list); **lead** for the rule "all learnset moves at or below the spawn level" for these two species |
| Core, level 5 or below (12) | `09A` Tackle, `0D9` Growl, `104` Scratch, `0E0` Leer, `07A` Tail Whip, `05C` Low Kick, `0BC` Thundershock, `0B9` Helping Hand, `133` Water Sport, `087` Absorb, `06F` Hypnosis, `163` Regular Attack | **blue-verified** (learnsets; the two starters and partners were both level 5 on floor 1, runtime). `163` is the engine's attack entry: **lead** for whether ID `163` or ID `0` is the A-button attack in every case (see [open points](#open-points-and-conflicts-357)) |
| Core, levels 6 to 10 (13) | `025` Smokescreen, `013` Rage, `010` Bubble, `046` Withdraw, `04A` Focus Energy, `08E` Sand-Attack, `0FB` Razor Leaf, `0B6` Vine Whip, `0C0` Thunder Wave, `149` Leech Seed, `03F` Bite, `03B` Disable, `106` Ember | **blue-verified** (learnsets). **lead** for the cap of level 10, which the issue does not give and the findings assume |
| Engine | `160` Struggle (used when no PP is left) | **blue-verified** (dispatch case, forced run); **lead** for the trigger |

The starter and partner set is the 11 species the findings name by name (Bulbasaur, Charmander, Squirtle, Pikachu, Chikorita, Cyndaquil, Totodile, Machop, Meowth, Psyduck, Eevee). **lead**: taken from the species table by name, not checked against the quiz table.

Total: 26 move IDs (12 + 13 + Struggle). The names are the ones in the [catalogue](#catalogue) below. **blue-verified** (`rescue_data::moves::parse_moves` on the dump).

### Common path for every move (#357, P2)

- The dispatch `ov_0001::02156824` calls `AccuracyCalc` (`ov_0001::0215A928`) with accuracy slot 0 before its switch; the call site's return address is `021571B8`. The fourth argument is the accuracy slot, not a flag. **blue-verified** (runtime: forced miss on `09A` and `0D9`).
- `AccuracyCalc` draws one `DungeonRandInt(100)` (the call inside it is at `0215A950`) and hits when the roll is below the accuracy. **blue-verified** (runtime, one draw per call). The draw itself is `DungeonRandInt` in [rng.md](rng.md#dungeon-lcg-blue-verified).
- The handler then runs as listed in the table. Draws below are counted after this first accuracy draw.

### P2: handler, steps and draws per move (#357)

Legend: **HDM** = `ov_0001::0215675C` (HandleDamagingMove). "Draws" are in order. Changed bytes are offsets into the target's `info` (or the user's) from the forced runs; the byte values are not copied. Each row is **blue-verified** unless a sub-claim is marked **lead**.

| Move | Handler | Steps, in order | Draws | Observed change (forced run) | Sim (#421) |
|------|---------|-----------------|-------|------------------------------|---|
| `09A` Tackle, `104` Scratch, `163` Regular Attack, `0FB` Razor Leaf, `0B6` Vine Whip | `ov_0001::0215675C` (HDM, direct) | type lookup `GetMoveTypeForMonster`, power `GetMovePower`, `CalcDamage` (`ov_0001::0217BD94`) with the normal multiplier, then the second accuracy check `ov_0001::0215652C` (see miss paths) | `0217C02C` Int(100) (ability roll), `0217C698` Int(100) (critical), `0217C720` Int(0x4000) (variance), `0215A950` Int(100) (accuracy, slot 1) | target HP down (`info+0x0E`) | Tackle, Scratch: ported (`combat`, #419). Regular Attack: ported (#419). Razor Leaf, Vine Whip: refused, `arg8` lead (#421) |
| `05C` Low Kick | `ov_0001::0215C600` | multiplier from a species lookup on the target's apparent ID (`02002314`), then HDM | as HDM | target HP down. **lead**: what the lookup classifies (probably weight) | Refused: multiplier lookup is a lead (#421) |
| `0BC` Thundershock | `ov_0001::0215E314` | HDM; if it hits, the chance gate `ov_0001::02159F6C` with a table chance; if the gate passes, paralysis `021885A0(user, target, 0)` | as HDM, then one Int(100) for the gate | target HP down; no status in the run (gate failed) | Refused: gate and status call (#73, #359) |
| `106` Ember | `ov_0001::0215D040` | `0218F168()` (not read), HDM with the normal multiplier, gate, burn `0218A710(user, target, 0, 0)` | as HDM, then one Int(100) for the gate | target HP down. **lead**: `0218F168` | Refused: gate and burn call (#73, #359) |
| `010` Bubble | `ov_0001::0215DD2C` | HDM, gate, `021882D0(user, target, 1, 0, arg4)` (speed lowering, see [symbols](../symbols.md)) | as HDM, then one Int(100) for the gate | target HP down | Refused: gate and speed call, not read (#421 lead) |
| `03F` Bite | `ov_0001::0215CCCC` | HDM, gate, `021887E0(user, target, 0)` (not read) | as HDM, then one Int(100) for the gate | target HP down. **lead**: the effect of `021887E0` | Refused: gate and `021887E0`, not read (#421 lead) |
| `087` Absorb | `ov_0001::0215F314` | HDM with the normal multiplier; if it hits, heal of half the damage (minimum 1) through `0218DF38` (`HealTargetHP`, red-hypothesis name), or through `0217BCA4` (`DealDamageToEntity_Async`) on the ability branch; the check `02159F10(user, 0)` is a validity test that draws nothing | as HDM only | target HP down; user `+0xD8` set | Refused: heal `0218DF38` not read (#421 lead) |
| `160` Struggle | `ov_0001::02162C70` | HDM; then recoil of the user's max HP divided by 4 (minimum 1) through `sub_806F370_Async` (`ov_0001::0217BB94`) | as HDM only | target HP down; user HP down | Refused: trigger is a lead (#421) |
| `0D9` Growl | `ov_0001::021616F0` | `02189534(user, target, stat 0, 1, 1, 1)`: attack stage down one | none | target `+0x1C` changed (one stage) | Ported: `growl_hit_*`, `growl_miss_*`, `a_stage_at_the_bottom_*`, `a_use_of_growl_*` (#421). Blocks: [stage-change blocks](#stage-change-blocks-421) |
| `0E0` Leer, `07A` Tail Whip | `ov_0001::02161548` (shared, see P3) | `021893A4(user, target, 0, 1, 1, 1)`: defense stage down one | none | target `+0x20` changed (one stage) | Ported: `leer_and_tail_whip_*`, `leer_miss_*` (#421). Blocks: [stage-change blocks](#stage-change-blocks-421) |
| `08E` Sand-Attack | `ov_0001::0215EEF8` | `02188910(user, target, 0, 1)`: accuracy stage down one | none | target `+0x24` changed (one stage) | Ported: `sand_attack_*` (#421). Blocks: [stage-change blocks](#stage-change-blocks-421) |
| `046` Withdraw | `ov_0001::0215CF7C` | `021890C4(user, target, 0, 1)`: defense stage up one | none | target `+0x20` changed (one stage). Run against the leader as target; the self-target effect is **lead** | Refused: self-target case is a lead (#421) |
| `0B9` Helping Hand | `ov_0001::0215E3C4` | if user is not target: `02189234` twice (stat 0 and stat 1, one stage up each); otherwise a fail message | none | target `+0x1C` and `+0x1E` changed | Ported: `helping_hand_*` (#421). Targeting taken from the move row, not run |
| `0C0` Thunder Wave, `03B` Disable | `ov_0001::0215E2F8` and `ov_0001::0215CF60` (same body, see P3) | `021885A0(user, target, 1)`: paralysis | `DungeonRandRange(1, 2)` (one Int(1)) | target `+0x9B`, `+0x9C` set, `+0xE0` cleared | Refused: paralysis call (#73, #359) |
| `06F` Hypnosis | `ov_0001::0215E07C` | `0219CF28` (`CalculateStatusTurns`, duration range; for team members halved or capped by IQ skill and ability), then `0218B2D0` (`SleepStatusTarget`) | `DungeonRandRange(3, 7)` | target `+0x99`, `+0x9A` set (sleep and turns) | Refused: sleep and duration (#359, #73) |
| `025` Smokescreen | `ov_0001::0215D5BC` | `0219CF28` with a range, then `0218D778` (not read) | `DungeonRandRange(1, 6)` | target `+0xC4`, `+0xC5` set | Refused: `0218D778` not read (#421 lead) |
| `04A` Focus Energy | `ov_0001::0215D5FC` | `0218D5F0()` (not read) | `DungeonRandRange(2, 4)`, drawn inside the call. **lead**: its position relative to the accuracy draw | target `+0xC4`, `+0xC5` set | Refused: `0218D5F0` not read (#421 lead) |
| `013` Rage | `ov_0001::0215DCC8` | `0218E428(user, target, 0xC, ...)` (not read) | `DungeonRandRange(4, 10)` | target `+0xAE`, `+0xAF`, `+0x11C` set | Refused: `0218E428` not read (#421 lead) |
| `149` Leech Seed | `ov_0001::02163204` | `0218DAE4(user, target, 1)`; user `+0xD8` set | `DungeonRandRange(10, 12)` | target `+0xB8`, `+0xBC`, `+0xBD`, `+0xC0`, `+0xC1` set; user `+0xD8` set | Refused: `0218DAE4` and seed status (#73) |
| `133` Water Sport (and `0D4` Mud Sport) | `ov_0001::021617C0` | `021955B0(1)` (Mud Sport: `021955B0(0)`): a floor-wide effect | `DungeonRandRange(10, 12)` | none on target or user (floor state, not read) | Refused: floor state not read (#421 lead) |

`DungeonRandRange(a, b)` draws one `DungeonRandInt(b - a)` and adds `a` ([rng.md](rng.md#dungeon-lcg-blue-verified)). **blue-verified.** The bounds are the constants the handler passes. Roll results are not recorded here.

### Sim status after #421

Of the 26 move IDs in the P1 list, 8 are ported to `rescue-sim`: Tackle (`09A`), Scratch (`104`) and Regular Attack (`163`) through the damaging path (#419), and Growl (`0D9`), Leer (`0E0`), Tail Whip (`07A`), Sand-Attack (`08E`) and Helping Hand (`0B9`) through the stage handlers of `crates/sim/src/move_effects.rs`. Each ported handler keeps the dispatch's slot-0 hit check (one draw, no handler on a miss), and a test covers its draws and its stage change.

Not ported (18 IDs), each a refused use with a `shortcut:` at `effect_of`:

- Need a status or a stat routine whose body is not read: Thundershock (`0BC`), Ember (`106`), Bubble (`010`), Bite (`03F`), Thunder Wave (`0C0`), Disable (`03B`), Hypnosis (`06F`), Smokescreen (`025`), Focus Energy (`04A`), Rage (`013`), Leech Seed (`149`). The status codes belong to [#73](https://github.com/TorratDev/rust-mm/issues/73) and [#359](https://github.com/TorratDev/rust-mm/issues/359).
- A lead: Razor Leaf (`0FB`) and Vine Whip (`0B6`) (the `arg8` of their dispatch case is not read), Low Kick (`05C`), Absorb (`087`), Struggle (`160`), Withdraw (`046`, self-target).
- Floor state the sim does not read: Water Sport (`133`).

### Stage-change blocks (#421)

A stage-lowering change can be blocked after its hit check. The block draws nothing and changes nothing. These are **static reads** (decompile on the reviewer's copy of `rescue-arm9-r`, not run in the emulator), so they are **lead** until a run confirms them.

- **`ov_0001::02180798(user, target, msg)`.** Called by Growl's routine `02189534`, by Leer and Tail Whip's `021893A4` (when its `param_5` is 1, as the handler passes it) and by Sand-Attack's `02188910`. It returns 1 (no change) when the target's `info+0xB1` is `0x0E` (a status, not named here), or when the target has ability `0x0F` or `0x18`. The ability test is `0218067C(target, id)`, which compares `info+0x58` and `+0x59`.
- **Growl, `02189534`.** Also stops when ability `0x09` is present and the stat is 0 (the attack stat, which Growl passes). It also stops when the held-item check `021432E8(target, 0x0B)` is true (`+0x5A` bit 0 set, bit 3 clear, `+0x5C == 0x0B`). The held-item check is not modelled (#359).
- **Sand-Attack, `02188910`.** Also stops when ability `0x1E` is present and `param_3` is 0 (the handler passes 0).
- **Helping Hand, `02189234`.** No block check. The raise is not blocked by any of the above.

The sim's `change_stages` models the ability blocks (tests `growl_is_blocked_by_*`, `leer_is_blocked_by_*`, `sand_attack_is_blocked_by_*`, `each_block_applies_only_*`, `helping_hand_is_not_blocked_*`). The status check and the held-item check are `shortcut:` notes, linked to [#73](https://github.com/TorratDev/rust-mm/issues/73) and [#359](https://github.com/TorratDev/rust-mm/issues/359). The ability IDs are kept as numbers; no ability name is given here.

### Miss paths (#357, P2)

- **Dispatch miss (accuracy slot 0).** The handler is not called. No draw follows the accuracy draw, and the target's `info+0x12A` is not written. **blue-verified** (runtime: forced miss on `09A` and `0D9`, by forcing the call's return to 0).
- **Second accuracy miss (slot 1), damaging moves.** `ov_0001::0215652C` calls `AccuracyCalc` again with slot 1 (return address `02156550`) and draws one more Int(100). On a miss it prints the miss text, sets the damage struct's byte at `+0xF`, and HP does not change. The three `CalcDamage` draws are already spent, so the draw sequence matches a hit up to the second accuracy draw. The target's `info+0x12A` is set to 1. **blue-verified** (runtime: forced second miss on `09A` and `0BC`).
  - A natural second miss after a first hit is **blue-verified** (runtime). At frame 3411 of the #356 probe run `6b21d775` (which patches only the crit odds), move 260: the slot-0 draw rolled 44 (hit), the three `CalcDamage` draws followed, and the slot-1 draw rolled 91 against accuracy byte 90 (miss). See [#356 P1](combat.md#p1-hit-check-accuracycalc).
- **Secondary gate miss.** On `0BC`, `106`, `010` and `03F` the gate draws one Int(100) after the damage. If it fails, the status or stat call is skipped. **blue-verified** (static order; the run's gate failed and no status was set).
- **Absorb and Struggle** use `02159F10(user, 0)`, which is a validity test: chance 0 draws nothing. **blue-verified** (see [roll wrapper](#roll-wrapper-per-move-id-p3-186)).

### Shared handlers (#357, P3)

- **`ov_0001::0215675C` (HDM).** Direct for `09A`, `104`, `163`, `0FB`, `0B6` and case 0 (ID `000`). Wrapped by `05C`, `0BC`, `106`, `010`, `03F`, `087` and `160`, each of which calls HDM with its own multiplier or a follow-up. **blue-verified** (export decompile; runtime for the direct moves).
- **`ov_0001::02161548`.** `0E0` Leer and `07A` Tail Whip, one defense-down call. **blue-verified** (runtime).
- **`ov_0001::0215E2F8` and `ov_0001::0215CF60`.** Two functions with the same body: `0C0` Thunder Wave and `03B` Disable both call `021885A0(user, target, 1)`. **blue-verified** (export decompile; runtime for both). The Red names differ; see open points.
- **`ov_0001::0215E07C`.** Hypnosis in the list. The catalogue maps `004`, `022`, `04F`, `054` and `0E7` to it too; not checked. **lead**.
- **`ov_0001::02159F6C` (chance gate).** Thundershock, Ember, Bubble and Bite. Its roll goes through `DungeonRandOutcome` (`ov_0001::0219CFCC`), per the [roll wrapper](#roll-wrapper-per-move-id-p3-186). **blue-verified**.
- **`ov_0001::0219CF28` (`CalculateStatusTurns`).** Shared by Hypnosis and Smokescreen. **blue-verified** ([symbols](../symbols.md)).
- **One-move handlers** in the list (no sharing inside it): `021616F0`, `0215EEF8`, `0215CF7C`, `0215E3C4`, `0215D5BC`, `0215D5FC`, `0215DCC8`, `02163204`, `021617C0`, `0215F314`, `02162C70`. **blue-verified**.

Cross-references from the catalogue, outside the list and not checked: `0215CCCC` also for `0DD`, `10F`, `12F`; `0215EEF8` also for `092`, `111`; `0215CF7C` also for `03A`. **lead**.

### Open points and conflicts (#357)

1. **Resolved: dispatch accuracy call.** The [dispatch section in `combat.md`](combat.md#dispatch-shape-and-hitmiss-branch-p2-186) now records the slot-0 `AccuracyCalc` call before the switch (return `021571B8`), so it no longer says there is no accuracy roll. The slot-1 check after `CalcDamage` is in [P1](combat.md#p1-hit-check-accuracycalc). The miss paths above agree with both.
2. **Disable.** The Blue handler for `03B` has the same body as Thunder Wave (paralysis). The catalogue's Red name `DisableMoveAction` implies another effect. This page keeps the Blue reading. **lead** for the Red name.
3. **ID `0` versus `163`.** Six natural attacks had move ID `0` in the 4,500-frame run; `163` was not sampled there. The earlier 3,000-frame run recorded `163` (see the dispatch section). Which ID is the A-button attack in each case is **lead**.
4. **Runs.** The 4,500-frame run had 78 dispatch entries, nine with an accuracy call (IDs `09A`, `104` and `0D9` once each). The other 69 entries have ID `0` and no accuracy call, from the second caller `0215AB34`, which is not examined. **lead**: how the sim should call the dispatch for that caller.
5. **Status codes** belong to [#73](https://github.com/TorratDev/rust-mm/issues/73). This table gives only the bytes that change.
6. **Level cap 10** is an assumption. **lead**.
7. **Static-only spawns.** Pidgey and Exeggcute are static only; weight-zero entries are assumed not to spawn through the weighted pick. **lead**.
8. **Unnamed callees.** `0215652C` has no row in [symbols](../symbols.md): the findings give its role, not a signature. Several callees (`021885A0`, `0218B2D0`, `0218D778`, `0218D5F0`, `0218E428`, `0218DAE4`, `0218F168`, `021882D0`, `021887E0`, `021955B0`) are known from their state changes, not their bodies.
9. **Chance values** come from table words. The findings give them; they are not copied here.
10. **Chance-gate name.** The findings call `02159F6C` by the name of its roll (DungeonRandOutcome). The gate is `02159F6C`; the roll it makes is `0219CFCC`.
11. **Reproducibility.** The runtime scripts (the natural capture, the forced captures and the probe) are in the investigator's scratch folder, not in the repo. [`verification/v7-trace/`](../../verification/v7-trace/) was not used. **lead**: add them under `verification/` to rerun the runs.
12. **Helping Hand (#421).** The sim takes its targeting from the move row, as for every move. The unit test passes its own row, so the targeting is not checked against the dump here. **lead** until a ROM-gated test runs it.

## Catalogue

The `Blue handler` column is the first call (or `goto` target) of each case in the Blue switch. `—` in the Red column means Red has no handler name for that ID. `none (no effect)` means the case does nothing.

| ID | Move | Blue handler | Red handler (hypothesis) |
|----|------|--------------|--------------------------|
| 000 | MOVE_NOTHING | `ov_0001::0215675C` | HandleDamagingMove |
| 001 | MOVE_IRON_TAIL | `ov_0001::0215E160` | IronTailMoveAction |
| 002 | MOVE_ICE_BALL | `ov_0001::0215E104` | sub_805768C |
| 003 | MOVE_YAWN | `ov_0001::0215E0C0` | YawnMoveAction |
| 004 | MOVE_LOVELY_KISS | `ov_0001::0215E07C` | BasicSleepMoveAction |
| 005 | MOVE_NIGHTMARE | `ov_0001::0215E03C` | NightmareMoveAction |
| 006 | MOVE_MORNING_SUN | `ov_0001::0215DFF4` | MorningSunMoveAction |
| 007 | MOVE_VITAL_THROW | `ov_0001::0215DFDC` | VitalThrowMoveAction |
| 008 | MOVE_DIG | `ov_0001::0215DEF8` | DigMoveAction |
| 009 | MOVE_THRASH | `ov_0001::0215675C` | HandleDamagingMove |
| 00A | MOVE_SWEET_SCENT | `ov_0001::0215DED0` | SweetScentMoveAction |
| 00B | MOVE_CHARM | `ov_0001::0215DEA0` | CharmMoveAction |
| 00C | MOVE_RAIN_DANCE | `ov_0001::0215DE2C` | RainDanceMoveAction |
| 00D | MOVE_CONFUSE_RAY | `ov_0001::0215E710` | SupersonicMoveAction |
| 00E | MOVE_HAIL | `ov_0001::0215DDB8` | HailMoveAction |
| 00F | MOVE_AROMATHERAPY | `ov_0001::0215DD98` | sub_80578EC |
| 010 | MOVE_BUBBLE | `ov_0001::0215DD2C` | BubbleMoveAction |
| 011 | MOVE_ENCORE | `ov_0001::0215DD14` | EncoreMoveAction |
| 012 | MOVE_CUT | `ov_0001::0215675C` | HandleDamagingMove |
| 013 | MOVE_RAGE | `ov_0001::0215DCC8` | RageMoveAction |
| 014 | MOVE_SUPER_FANG | `ov_0001::0215DBFC` | SuperFangMoveAction |
| 015 | MOVE_PAIN_SPLIT | `ov_0001::0215DB38` | PainSplitMoveAction |
| 016 | MOVE_TORMENT | `ov_0001::0215D9B8` | TormentMoveAction |
| 017 | MOVE_STRING_SHOT | `ov_0001::0215D998` | StringShotMoveAction |
| 018 | MOVE_SWAGGER | `ov_0001::0215D958` | SwaggerMoveAction |
| 019 | MOVE_SNORE | `ov_0001::0215D8A8` | SnoreMoveAction |
| 01A | MOVE_HEAL_BELL | `ov_0001::0215DD98` | sub_80578EC |
| 01B | MOVE_SCREECH | `ov_0001::0215D878` | ScreechMoveAction |
| 01C | MOVE_ROCK_THROW | `ov_0001::0215675C` | HandleDamagingMove |
| 01D | MOVE_ROCK_SMASH | `ov_0001::02162BBC` | RockSmashMoveAction |
| 01E | MOVE_ROCK_SLIDE | `ov_0001::0215D810` | RockSlideMoveAction |
| 01F | MOVE_WEATHER_BALL | `ov_0001::0215D7A8` | WeatherBallMoveAction |
| 020 | MOVE_WHIRLPOOL | `ov_0001::0215D720` | WhirlpoolMoveAction |
| 021 | MOVE_FAKE_TEARS | `ov_0001::0215D6EC` | FakeTearsMoveAction |
| 022 | MOVE_SING | `ov_0001::0215E07C` | BasicSleepMoveAction |
| 023 | MOVE_SPITE | `ov_0001::0215D614` | SpiteMoveAction |
| 024 | MOVE_AIR_CUTTER | `ov_0001::0215675C` | HandleDamagingMove |
| 025 | MOVE_SMOKESCREEN | `ov_0001::0215D5BC` | SmokescreenMoveAction |
| 026 | MOVE_PURSUIT | `ov_0001::0215D148` | CounterMoveAction |
| 027 | MOVE_DOUBLESLAP | `ov_0001::0215675C` | HandleDamagingMove |
| 028 | MOVE_MIRROR_MOVE | `ov_0001::0215D590` | MirrorMoveMoveAction |
| 029 | MOVE_OVERHEAT | `ov_0001::0215D524` | OverheatMoveAction |
| 02A | MOVE_AURORA_BEAM | `ov_0001::0215D4A8` | AuroraBeamMoveAction |
| 02B | MOVE_MEMENTO | `ov_0001::0215D440` | MementoMoveAction |
| 02C | MOVE_OCTAZOOKA | `ov_0001::0215D3CC` | OctazookaMoveAction |
| 02D | MOVE_FLATTER | `ov_0001::0215D38C` | FlatterMoveAction |
| 02E | MOVE_ASTONISH | `ov_0001::0215D810` | RockSlideMoveAction |
| 02F | MOVE_WILL_O_WISP | `ov_0001::0215D2D0` | WillOWispMoveAction |
| 030 | MOVE_RETURN | `ov_0001::0215D1F8` | ReturnMoveAction |
| 031 | MOVE_GRUDGE | `ov_0001::0215D164` | GrudgeMoveAction |
| 032 | MOVE_STRENGTH | `ov_0001::0216370C` | HurlOrbAction |
| 033 | MOVE_COUNTER | `ov_0001::0215D148` | CounterMoveAction |
| 034 | MOVE_FLAME_WHEEL | `ov_0001::0215D0C4` | FlameWheelMoveAction |
| 035 | MOVE_FLAMETHROWER | `ov_0001::0215D040` | BasicFireMoveAction |
| 036 | MOVE_ODOR_SLEUTH | `ov_0001::0215D010` | ExposeMoveAction |
| 037 | MOVE_SHARPEN | `ov_0001::0215CF38` | BasicRaiseAttackMoveAction |
| 038 | MOVE_DOUBLE_TEAM | `ov_0001::0215CFEC` | DoubleTeamMoveAction |
| 039 | MOVE_GUST | `ov_0001::0215CFA4` | GustMoveAction |
| 03A | MOVE_HARDEN | `ov_0001::0215CF7C` | BasicRaiseDefenseMoveAction |
| 03B | MOVE_DISABLE | `ov_0001::0215CF60` | DisableMoveAction |
| 03C | MOVE_RAZOR_WIND | `ov_0001::0215CE98` | RazorWindMoveAction |
| 03D | MOVE_BIDE | `ov_0001::0215CE4C` | BideMoveAction |
| 03E | MOVE_CRUNCH | `ov_0001::0215CD34` | ShadowBallMoveAction |
| 03F | MOVE_BITE | `ov_0001::0215CCCC` | BiteMoveAction |
| 040 | MOVE_THUNDER | `ov_0001::0215CC64` | ThunderMoveAction |
| 041 | MOVE_THUNDERPUNCH | `ov_0001::0215CC64` | ThunderMoveAction |
| 042 | MOVE_ENDEAVOR | `ov_0001::0215CBC0` | EndeavorMoveAction |
| 043 | MOVE_FACADE | `ov_0001::0215CB78` | FacadeMoveAction |
| 044 | MOVE_KARATE_CHOP | `ov_0001::0215675C` | HandleDamagingMove |
| 045 | MOVE_CLAMP | `ov_0001::0215FF88` | sub_805B17C |
| 046 | MOVE_WITHDRAW | `ov_0001::0215CF7C` | BasicRaiseDefenseMoveAction |
| 047 | MOVE_CONSTRICT | `ov_0001::0215CB0C` | sub_8058580 |
| 048 | MOVE_BRICK_BREAK | `ov_0001::0215CA74` | BrickBreakMoveAction |
| 049 | MOVE_ROCK_TOMB | `ov_0001::0215CA10` | RockTombMoveAction |
| 04A | MOVE_FOCUS_ENERGY | `ov_0001::0215D5FC` | FocusEnergyMoveAction |
| 04B | MOVE_FOCUS_PUNCH | `ov_0001::0215C970` | FocusPunchMoveAction |
| 04C | MOVE_GIGA_DRAIN | `ov_0001::0215C890` | GigaDrainMoveAction |
| 04D | MOVE_REVERSAL | `ov_0001::0215C7E8` | ReversalMoveAction |
| 04E | MOVE_SMELLINGSALT | `ov_0001::0215C774` | SmellingSaltMoveAction |
| 04F | MOVE_SPORE | `ov_0001::0215E07C` | BasicSleepMoveAction |
| 050 | MOVE_LEECH_LIFE | `ov_0001::0215C890` | GigaDrainMoveAction |
| 051 | MOVE_SLASH | `ov_0001::0215675C` | HandleDamagingMove |
| 052 | MOVE_SILVER_WIND | `ov_0001::0215C508` | AncientPowerMoveAction |
| 053 | MOVE_METAL_SOUND | `ov_0001::0215C740` | MetalSoundMoveAction |
| 054 | MOVE_GRASSWHISTLE | `ov_0001::0215E07C` | BasicSleepMoveAction |
| 055 | MOVE_TICKLE | `ov_0001::0215C6DC` | TickleMoveAction |
| 056 | MOVE_SPIDER_WEB | `ov_0001::0215C6C4` | sub_805889C |
| 057 | MOVE_CRABHAMMER | `ov_0001::0215675C` | HandleDamagingMove |
| 058 | MOVE_HAZE | `ov_0001::0215C6A8` | HazeMoveAction |
| 059 | MOVE_MEAN_LOOK | `ov_0001::0215C6C4` | sub_805889C |
| 05A | MOVE_CROSS_CHOP | `ov_0001::0215675C` | HandleDamagingMove |
| 05B | MOVE_OUTRAGE | `ov_0001::0215C654` | OutrageMoveAction |
| 05C | MOVE_LOW_KICK | `ov_0001::0215C600` | LowKickMoveAction |
| 05D | MOVE_ANCIENTPOWER | `ov_0001::0215C508` | AncientPowerMoveAction |
| 05E | MOVE_SYNTHESIS | `ov_0001::0215C4C0` | SynthesisMoveAction |
| 05F | MOVE_AGILITY | `ov_0001::0215C4A0` | AgilityMoveAction |
| 060 | MOVE_RAPID_SPIN | `ov_0001::0215C44C` | RapidSpinMoveAction |
| 061 | MOVE_ICY_WIND | `ov_0001::0215CA10` | RockTombMoveAction |
| 062 | MOVE_MIND_READER | `ov_0001::0215C40C` | SureShotMoveAction |
| 063 | MOVE_COSMIC_POWER | `ov_0001::0215C3C0` | CosmicPowerMoveAction |
| 064 | MOVE_SKY_ATTACK | `ov_0001::0215FD7C` | SkyAttackMoveAction |
| 065 | MOVE_POWDER_SNOW | `ov_0001::0215FD14` | BasicIceMoveAction |
| 066 | MOVE_FOLLOW_ME | `ov_0001::02163650` | sub_805B314 |
| 067 | MOVE_METEOR_MASH | `ov_0001::0215FC90` | MeteorMashMoveAction |
| 068 | MOVE_ENDURE | `ov_0001::0215FC74` | EndureMoveAction |
| 069 | MOVE_ROLLOUT | `ov_0001::0215E104` | sub_805768C |
| 06A | MOVE_SCARY_FACE | `ov_0001::0215FC54` | ScaryFaceMoveAction |
| 06B | MOVE_PSYBEAM | `ov_0001::0215FBEC` | sub_8058C00 |
| 06C | MOVE_PSYWAVE | `ov_0001::0215FB74` | PsywaveMoveAction |
| 06D | MOVE_PSYCHIC | `ov_0001::0215FAE8` | DamageLowerDefMoveAction |
| 06E | MOVE_PSYCHO_BOOST | `ov_0001::0215FA78` | PsychoBoostMoveAction |
| 06F | MOVE_HYPNOSIS | `ov_0001::0215E07C` | BasicSleepMoveAction |
| 070 | MOVE_UPROAR | `ov_0001::0215FA60` | UproarMoveAction |
| 071 | MOVE_WATER_SPOUT | `ov_0001::0215F9B8` | WaterSpoutMoveAction |
| 072 | MOVE_SIGNAL_BEAM | `ov_0001::0215FBEC` | sub_8058C00 |
| 073 | MOVE_PSYCH_UP | `ov_0001::0215F8FC` | PsychUpMoveAction |
| 074 | MOVE_SUBMISSION | `ov_0001::0215F83C` | sub_8058E5C |
| 075 | MOVE_RECOVER | `ov_0001::0215F7F4` | sub_8058EE0 |
| 076 | MOVE_EARTHQUAKE | `ov_0001::0215F78C` | EarthquakeMoveAction |
| 077 | MOVE_NATURE_POWER | `ov_0001::0215F714` | NaturePowerMoveAction |
| 078 | MOVE_LICK | `ov_0001::0215F6AC` | LickMoveAction |
| 079 | MOVE_FLAIL | `ov_0001::0215C7E8` | ReversalMoveAction |
| 07A | MOVE_TAIL_WHIP | `ov_0001::02161548` | LeerMoveAction |
| 07B | MOVE_SELFDESTRUCT | `ov_0001::0215F654` | SelfDestructMoveAction |
| 07C | MOVE_STUN_SPORE | `ov_0001::0215F638` | StunSporeMoveAction |
| 07D | MOVE_BIND | `ov_0001::0215FF88` | sub_805B17C |
| 07E | MOVE_SHADOW_PUNCH | `ov_0001::0215675C` | HandleDamagingMove |
| 07F | MOVE_SHADOW_BALL | `ov_0001::0215CD34` | ShadowBallMoveAction |
| 080 | MOVE_CHARGE | `ov_0001::0215F5EC` | ChargeMoveAction |
| 081 | MOVE_THUNDERBOLT | `ov_0001::0215F584` | ThunderboltMoveAction |
| 082 | MOVE_MIST | `ov_0001::0215F56C` | MistMoveAction |
| 083 | MOVE_FISSURE | `ov_0001::0215F478` | FissureMoveAction |
| 084 | MOVE_EXTREMESPEED | `ov_0001::0215675C` | HandleDamagingMove |
| 085 | MOVE_EXTRASENSORY | `ov_0001::0215F410` | ExtrasensoryMoveAction |
| 086 | MOVE_SAFEGUARD | `ov_0001::0215F3F8` | SafeguardMoveAction |
| 087 | MOVE_ABSORB | `ov_0001::0215F314` | AbsorbMoveAction |
| 088 | MOVE_SKY_UPPERCUT | `ov_0001::0215675C` | HandleDamagingMove |
| 089 | MOVE_SKILL_SWAP | `ov_0001::0215F1F4` | SkillSwapMoveAction |
| 08A | MOVE_SKETCH | `ov_0001::0215F0A8` | SketchMoveAction |
| 08B | MOVE_HEADBUTT | `ov_0001::0215F040` | HeadbuttMoveAction |
| 08C | MOVE_DOUBLE_EDGE | `ov_0001::0215EF94` | DoubleEdgeMoveAction |
| 08D | MOVE_SANDSTORM | `ov_0001::0215EF20` | SandstormMoveAction |
| 08E | MOVE_SAND_ATTACK | `ov_0001::0215EEF8` | sub_8059528 |
| 08F | MOVE_SAND_TOMB | `ov_0001::0215FF88` | sub_805B17C |
| 090 | MOVE_SPARK | `ov_0001::0215F6AC` | LickMoveAction |
| 091 | MOVE_SWIFT | `ov_0001::0215675C` | HandleDamagingMove |
| 092 | MOVE_KINESIS | `ov_0001::0215EEF8` | sub_8059528 |
| 093 | MOVE_SMOG | `ov_0001::0215EE90` | SmogMoveAction |
| 094 | MOVE_GROWTH | `ov_0001::0215EE68` | GrowthMoveAction |
| 095 | MOVE_SACRED_FIRE | `ov_0001::0215EDFC` | SacredFireMoveAction |
| 096 | MOVE_SHEER_COLD | `ov_0001::0215ED24` | sub_80595EC |
| 097 | MOVE_SOLARBEAM | `ov_0001::0215EC44` | SolarBeamMoveAction |
| 098 | MOVE_SONICBOOM | `ov_0001::0215EBB0` | SonicboomMoveAction |
| 099 | MOVE_FLY | `ov_0001::0215EB10` | FlyMoveAction |
| 09A | MOVE_TACKLE | `ov_0001::0215675C` | HandleDamagingMove |
| 09B | MOVE_EXPLOSION | `ov_0001::0215EAB8` | ExplosionMoveAction |
| 09C | MOVE_DIVE | `ov_0001::0215E9D8` | DiveMoveAction |
| 09D | MOVE_FIRE_BLAST | `ov_0001::0215D040` | BasicFireMoveAction |
| 09E | MOVE_WATERFALL | `ov_0001::0215675C` | HandleDamagingMove |
| 09F | MOVE_MUDDY_WATER | `ov_0001::0215E964` | MuddyWaterMoveAction |
| 0A0 | MOVE_STOCKPILE | `ov_0001::0215E94C` | StockpileMoveAction |
| 0A1 | MOVE_SLAM | `ov_0001::0215675C` | HandleDamagingMove |
| 0A2 | MOVE_TWISTER | `ov_0001::0215E8C0` | TwisterMoveAction |
| 0A3 | MOVE_BULLET_SEED | `ov_0001::0215675C` | HandleDamagingMove |
| 0A4 | MOVE_TWINEEDLE | `ov_0001::0215E82C` | TwineedleMoveAction |
| 0A5 | MOVE_SOFTBOILED | `ov_0001::0215E7E4` | sub_80599EC |
| 0A6 | MOVE_EGG_BOMB | `ov_0001::0215675C` | HandleDamagingMove |
| 0A7 | MOVE_FAINT_ATTACK | `ov_0001::0215675C` | HandleDamagingMove |
| 0A8 | MOVE_BARRAGE | `ov_0001::0215675C` | HandleDamagingMove |
| 0A9 | MOVE_MINIMIZE | `ov_0001::0215E7C0` | MinimizeMoveAction |
| 0AA | MOVE_SEISMIC_TOSS | `ov_0001::0215E72C` | sub_8059A2C |
| 0AB | MOVE_SUPERSONIC | `ov_0001::0215E710` | SupersonicMoveAction |
| 0AC | MOVE_TAUNT | `ov_0001::0215E6F8` | TauntMoveAction |
| 0AD | MOVE_MOONLIGHT | `ov_0001::0215E6B0` | MoonlightMoveAction |
| 0AE | MOVE_PECK | `ov_0001::0215675C` | HandleDamagingMove |
| 0AF | MOVE_ARM_THRUST | `ov_0001::0215675C` | HandleDamagingMove |
| 0B0 | MOVE_HORN_ATTACK | `ov_0001::0215675C` | HandleDamagingMove |
| 0B1 | MOVE_HORN_DRILL | `ov_0001::0215E5D8` | HornDrillMoveAction |
| 0B2 | MOVE_WING_ATTACK | `ov_0001::0215675C` | HandleDamagingMove |
| 0B3 | MOVE_AERIAL_ACE | `ov_0001::0215675C` | HandleDamagingMove |
| 0B4 | MOVE_ICICLE_SPEAR | `ov_0001::0215675C` | HandleDamagingMove |
| 0B5 | MOVE_SWORDS_DANCE | `ov_0001::0215E5B0` | SwordsDanceMoveAction |
| 0B6 | MOVE_VINE_WHIP | `ov_0001::0215675C` | HandleDamagingMove |
| 0B7 | MOVE_CONVERSION | `ov_0001::0215E458` | ConversionMoveAction |
| 0B8 | MOVE_CONVERSION_2 | `ov_0001::0215E440` | Conversion2MoveAction |
| 0B9 | MOVE_HELPING_HAND | `ov_0001::0215E3C4` | HelpingHandMoveAction |
| 0BA | MOVE_IRON_DEFENSE | `ov_0001::0215E39C` | sub_8059CD8 |
| 0BB | MOVE_TELEPORT | `ov_0001::0215E37C` | WarpMoveAction |
| 0BC | MOVE_THUNDERSHOCK | `ov_0001::0215E314` | ThundershockMoveAction |
| 0BD | MOVE_SHOCK_WAVE | `ov_0001::0215675C` | HandleDamagingMove |
| 0BE | MOVE_QUICK_ATTACK | `ov_0001::0215675C` | HandleDamagingMove |
| 0BF | MOVE_SWEET_KISS | `ov_0001::0215E710` | SupersonicMoveAction |
| 0C0 | MOVE_THUNDER_WAVE | `ov_0001::0215E2F8` | ThunderWaveMoveAction |
| 0C1 | MOVE_ZAP_CANNON | `ov_0001::0215E298` | ZapCannonMoveAction |
| 0C2 | MOVE_BLOCK | `ov_0001::0215E280` | BlockMoveAction |
| 0C3 | MOVE_HOWL | `ov_0001::0215CF38` | BasicRaiseAttackMoveAction |
| 0C4 | MOVE_POISON_GAS | `ov_0001::0215E264` | PoisonGasMoveAction |
| 0C5 | MOVE_TOXIC | `ov_0001::0215E248` | ToxicMoveAction |
| 0C6 | MOVE_POISON_FANG | `ov_0001::0215E1E0` | PoisonFangMoveAction |
| 0C7 | MOVE_POISONPOWDER | `ov_0001::0215E264` | PoisonGasMoveAction |
| 0C8 | MOVE_POISON_STING | `ov_0001::02161E00` | PoisonStingMoveAction |
| 0C9 | MOVE_SPIKE_CANNON | `ov_0001::0215675C` | HandleDamagingMove |
| 0CA | MOVE_ACID_ARMOR | `ov_0001::0215E39C` | sub_8059CD8 |
| 0CB | MOVE_TAKE_DOWN | `ov_0001::0215F83C` | sub_8058E5C |
| 0CC | MOVE_JUMP_KICK | `ov_0001::02161CC0` | JumpKickMoveAction |
| 0CD | MOVE_BOUNCE | `ov_0001::02161BF0` | BounceMoveAction |
| 0CE | MOVE_HI_JUMP_KICK | `ov_0001::02161AB4` | HiJumpKickMoveAction |
| 0CF | MOVE_TRI_ATTACK | `ov_0001::02161A00` | TriAttackMoveAction |
| 0D0 | MOVE_DRAGON_CLAW | `ov_0001::0215675C` | HandleDamagingMove |
| 0D1 | MOVE_TRICK | `ov_0001::02161838` | TrickMoveAction |
| 0D2 | MOVE_TRIPLE_KICK | `ov_0001::021617F4` | TripleKickMoveAction |
| 0D3 | MOVE_DRILL_PECK | `ov_0001::0215675C` | HandleDamagingMove |
| 0D4 | MOVE_MUD_SPORT | `ov_0001::021617C0` | MudWaterSportMoveAction |
| 0D5 | MOVE_MUD_SLAP | `ov_0001::02161754` | MudSlapMoveAction |
| 0D6 | MOVE_THIEF | `ov_0001::021629D0` (thunk) | ThiefMoveAction |
| 0D7 | MOVE_AMNESIA | `ov_0001::02161720` | AmnesiaMoveAction |
| 0D8 | MOVE_NIGHT_SHADE | `ov_0001::0215E72C` | sub_8059A2C |
| 0D9 | MOVE_GROWL | `ov_0001::021616F0` | GrowlMoveAction |
| 0DA | MOVE_SLACK_OFF | `ov_0001::0215F7F4` | sub_8058EE0 |
| 0DB | MOVE_SURF | `ov_0001::021616B0` | SurfMoveAction |
| 0DC | MOVE_ROLE_PLAY | `ov_0001::021615EC` | RolePlayMoveAction |
| 0DD | MOVE_NEEDLE_ARM | `ov_0001::0215CCCC` | BiteMoveAction |
| 0DE | MOVE_DOUBLE_KICK | `ov_0001::0215675C` | HandleDamagingMove |
| 0DF | MOVE_SUNNY_DAY | `ov_0001::02161578` | SunnyDayMoveAction |
| 0E0 | MOVE_LEER | `ov_0001::02161548` | LeerMoveAction |
| 0E1 | MOVE_WISH | `ov_0001::02161530` | WishMoveAction |
| 0E2 | MOVE_FAKE_OUT | `ov_0001::021614C8` | FakeOutMoveAction |
| 0E3 | MOVE_SLEEP_TALK | `ov_0001::02161488` | sub_805A450 |
| 0E4 | MOVE_PAY_DAY | `ov_0001::021613F0` | PayDayMoveAction |
| 0E5 | MOVE_ASSIST | `ov_0001::021613B0` | AssistMoveAction |
| 0E6 | MOVE_HEAT_WAVE | `ov_0001::0215D040` | BasicFireMoveAction |
| 0E7 | MOVE_SLEEP_POWDER | `ov_0001::0215E07C` | BasicSleepMoveAction |
| 0E8 | MOVE_REST | `ov_0001::02161370` | RestMoveAction |
| 0E9 | MOVE_INGRAIN | `ov_0001::02161358` | IngrainMoveAction |
| 0EA | MOVE_CONFUSION | `ov_0001::0215FBEC` | sub_8058C00 |
| 0EB | MOVE_BODY_SLAM | `ov_0001::0215F6AC` | LickMoveAction |
| 0EC | MOVE_SWALLOW | `ov_0001::021612E0` | SwallowMoveAction |
| 0ED | MOVE_CURSE | `ov_0001::021612C8` | CurseMoveAction |
| 0EE | MOVE_FRENZY_PLANT | `ov_0001::0216129C` | sub_805A568 |
| 0EF | MOVE_HYDRO_CANNON | `ov_0001::0216129C` | sub_805A568 |
| 0F0 | MOVE_HYDRO_PUMP | `ov_0001::0215675C` | HandleDamagingMove |
| 0F1 | MOVE_HYPER_VOICE | `ov_0001::0215675C` | HandleDamagingMove |
| 0F2 | MOVE_HYPER_BEAM | `ov_0001::0216129C` | sub_805A568 |
| 0F3 | MOVE_SUPERPOWER | `ov_0001::021611F8` | SuperpowerMoveAction |
| 0F4 | MOVE_STEEL_WING | `ov_0001::02161168` | SteelWingMoveAction |
| 0F5 | MOVE_SPIT_UP | `ov_0001::02161100` | SpitUpMoveAction |
| 0F6 | MOVE_DYNAMICPUNCH | `ov_0001::021610A0` | DynamicPunchMoveAction |
| 0F7 | MOVE_GUILLOTINE | `ov_0001::0215ED24` | sub_80595EC |
| 0F8 | MOVE_VICEGRIP | `ov_0001::0215675C` | HandleDamagingMove |
| 0F9 | MOVE_KNOCK_OFF | `ov_0001::02160ED8` | KnockOffMoveAction |
| 0FA | MOVE_POUND | `ov_0001::0215675C` | HandleDamagingMove |
| 0FB | MOVE_RAZOR_LEAF | `ov_0001::0215675C` | HandleDamagingMove |
| 0FC | MOVE_BATON_PASS | `ov_0001::02162984` | SwitcherOrbAction |
| 0FD | MOVE_PETAL_DANCE | `ov_0001::0215C654` | OutrageMoveAction |
| 0FE | MOVE_SPLASH | `ov_0001::02160A7C` | SplashMoveAction |
| 0FF | MOVE_BUBBLEBEAM | `ov_0001::0215CB0C` | sub_8058580 |
| 100 | MOVE_DOOM_DESIRE | `ov_0001::02160A60` | sub_805AAD0 |
| 101 | MOVE_BELLY_DRUM | `ov_0001::02160984` | BellyDrumMoveAction |
| 102 | MOVE_BARRIER | `ov_0001::0215E39C` | sub_8059CD8 |
| 103 | MOVE_LIGHT_SCREEN | `ov_0001::0216096C` | LightScreenMoveAction |
| 104 | MOVE_SCRATCH | `ov_0001::0215675C` | HandleDamagingMove |
| 105 | MOVE_HYPER_FANG | `ov_0001::0215F410` | ExtrasensoryMoveAction |
| 106 | MOVE_EMBER | `ov_0001::0215D040` | BasicFireMoveAction |
| 107 | MOVE_SECRET_POWER | `ov_0001::021607C4` | SecretPowerMoveAction |
| 108 | MOVE_DIZZY_PUNCH | `ov_0001::0216075C` | sub_805AC90 |
| 109 | MOVE_BULK_UP | `ov_0001::02160708` | BulkUpMoveAction |
| 10A | MOVE_IMPRISON | `ov_0001::021606C0` | ObserverOrbAction |
| 10B | MOVE_FEATHERDANCE | `ov_0001::0216068C` | FeatherDanceMoveAction |
| 10C | MOVE_WHIRLWIND | `ov_0001::02160148` | RoarMoveAction |
| 10D | MOVE_BEAT_UP | `ov_0001::02160540` | BeatUpMoveAction |
| 10E | MOVE_BLIZZARD | `ov_0001::0215FD14` | BasicIceMoveAction |
| 10F | MOVE_STOMP | `ov_0001::0215CCCC` | BiteMoveAction |
| 110 | MOVE_BLAST_BURN | `ov_0001::021604F4` | BlastBurnMoveAction |
| 111 | MOVE_FLASH | `ov_0001::0215EEF8` | sub_8059528 |
| 112 | MOVE_TEETER_DANCE | `ov_0001::0215E710` | SupersonicMoveAction |
| 113 | MOVE_CRUSH_CLAW | `ov_0001::02160474` | CrushClawMoveAction |
| 114 | MOVE_BLAZE_KICK | `ov_0001::021603F0` | BlazeKickMoveAction |
| 115 | MOVE_PRESENT | `ov_0001::021602FC` | PresentMoveAction |
| 116 | MOVE_ERUPTION | `ov_0001::02160250` | EruptionMoveAction |
| 117 | MOVE_SLUDGE | `ov_0001::02161E00` | PoisonStingMoveAction |
| 118 | MOVE_SLUDGE_BOMB | `ov_0001::02161E00` | PoisonStingMoveAction |
| 119 | MOVE_GLARE | `ov_0001::02160234` | GlareMoveAction |
| 11A | MOVE_TRANSFORM | `ov_0001::021601D0` | TransformMoveAction |
| 11B | MOVE_POISON_TAIL | `ov_0001::02160168` | PoisonTailMoveAction |
| 11C | MOVE_ROAR | `ov_0001::02160148` | RoarMoveAction |
| 11D | MOVE_BONE_RUSH | `ov_0001::0215675C` | HandleDamagingMove |
| 11E | MOVE_CAMOUFLAGE | `ov_0001::02160068` | HandleColorChange |
| 11F | MOVE_COVET | `ov_0001::021629D0` (thunk) | ThiefMoveAction |
| 120 | MOVE_TAIL_GLOW | `ov_0001::02160040` | TailGlowMoveAction |
| 121 | MOVE_BONE_CLUB | `ov_0001::0215F410` | ExtrasensoryMoveAction |
| 122 | MOVE_BONEMERANG | `ov_0001::0215675C` | HandleDamagingMove |
| 123 | MOVE_FIRE_SPIN | `ov_0001::0215FF88` | sub_805B17C |
| 124 | MOVE_FIRE_PUNCH | `ov_0001::0215D040` | BasicFireMoveAction |
| 125 | MOVE_PERISH_SONG | `ov_0001::0215FF70` | PerishSongMoveAction |
| 126 | MOVE_WRAP | `ov_0001::0215FF58` | WrapMoveAction |
| 127 | MOVE_SPIKES | `ov_0001::0215FEEC` | SpikesMoveAction |
| 128 | MOVE_MAGNITUDE | `ov_0001::0215FE64` | MagnitudeMoveAction |
| 129 | MOVE_MAGICAL_LEAF | `ov_0001::0215675C` | HandleDamagingMove |
| 12A | MOVE_MAGIC_COAT | `ov_0001::0215FE4C` | MagicCoatMoveAction |
| 12B | MOVE_MUD_SHOT | `ov_0001::0215CA10` | RockTombMoveAction |
| 12C | MOVE_MACH_PUNCH | `ov_0001::0215675C` | HandleDamagingMove |
| 12D | MOVE_PROTECT | `ov_0001::02163694` | ProtectMoveAction |
| 12E | MOVE_DEFENSE_CURL | `ov_0001::0216366C` | DefenseCurlMoveAction |
| 12F | MOVE_ROLLING_KICK | `ov_0001::0215CCCC` | BiteMoveAction |
| 130 | MOVE_SUBSTITUTE | `ov_0001::02163650` | sub_805B314 |
| 131 | MOVE_DETECT | `ov_0001::02163694` | ProtectMoveAction |
| 132 | MOVE_PIN_MISSILE | `ov_0001::0215675C` | HandleDamagingMove |
| 133 | MOVE_WATER_SPORT | `ov_0001::021617C0` | MudWaterSportMoveAction |
| 134 | MOVE_WATER_GUN | `ov_0001::0215675C` | HandleDamagingMove |
| 135 | MOVE_MIST_BALL | `ov_0001::021635D0` | MistBallMoveAction |
| 136 | MOVE_WATER_PULSE | `ov_0001::0216075C` | sub_805AC90 |
| 137 | MOVE_FURY_ATTACK | `ov_0001::0215675C` | HandleDamagingMove |
| 138 | MOVE_FURY_SWIPES | `ov_0001::0215675C` | HandleDamagingMove |
| 139 | MOVE_DESTINY_BOND | `ov_0001::021635B8` | DestinyBondMoveAction |
| 13A | MOVE_FALSE_SWIPE | `ov_0001::0216358C` | FalseSwipeMoveAction |
| 13B | MOVE_FORESIGHT | `ov_0001::0215D010` | ExposeMoveAction |
| 13C | MOVE_MIRROR_COAT | `ov_0001::02163574` | MirrorCoatMoveAction |
| 13D | MOVE_FUTURE_SIGHT | `ov_0001::02160A60` | sub_805AAD0 |
| 13E | MOVE_MILK_DRINK | `ov_0001::0215E7E4` | sub_80599EC |
| 13F | MOVE_CALM_MIND | `ov_0001::02163520` | CalmMindMoveAction |
| 140 | MOVE_MEGA_DRAIN | `ov_0001::0215C890` | GigaDrainMoveAction |
| 141 | MOVE_MEGA_KICK | `ov_0001::0215675C` | HandleDamagingMove |
| 142 | MOVE_MEGA_PUNCH | `ov_0001::0215675C` | HandleDamagingMove |
| 143 | MOVE_MEGAHORN | `ov_0001::0215675C` | HandleDamagingMove |
| 144 | MOVE_HIDDEN_POWER | `ov_0001::02163500` | HiddenPowerMoveAction |
| 145 | MOVE_METAL_CLAW | `ov_0001::02163470` | MetalClawMoveAction |
| 146 | MOVE_ATTRACT | `ov_0001::02163454` | AttractMoveAction |
| 147 | MOVE_MIMIC | `ov_0001::0216330C` | MimicMoveAction |
| 148 | MOVE_FRUSTRATION | `ov_0001::02163230` | FrustrationMoveAction |
| 149 | MOVE_LEECH_SEED | `ov_0001::02163204` | LeechSeedMoveAction |
| 14A | MOVE_METRONOME | `ov_0001::021631A4` | MetronomeMoveAction |
| 14B | MOVE_DREAM_EATER | `ov_0001::02163080` | DreamEaterMoveAction |
| 14C | MOVE_ACID | `ov_0001::0215FAE8` | DamageLowerDefMoveAction |
| 14D | MOVE_MEDITATE | `ov_0001::0215CF38` | BasicRaiseAttackMoveAction |
| 14E | MOVE_SNATCH | `ov_0001::02163068` | SnatchMoveAction |
| 14F | MOVE_LUSTER_PURGE | `ov_0001::02162D18` | LusterPurgeMoveAction |
| 150 | MOVE_LEAF_BLADE | `ov_0001::0215675C` | HandleDamagingMove |
| 151 | MOVE_RECYCLE | `ov_0001::02162F28` | RecycleMoveAction |
| 152 | MOVE_REFLECT | `ov_0001::02162F10` | ReflectMoveAction |
| 153 | MOVE_REFRESH | `ov_0001::0215DD98` | sub_80578EC |
| 154 | MOVE_REVENGE | `ov_0001::0215CE4C` | BideMoveAction |
| 155 | MOVE_DRAGON_RAGE | `ov_0001::02162E7C` | DragonRageMoveAction |
| 156 | MOVE_DRAGONBREATH | `ov_0001::0215F6AC` | LickMoveAction |
| 157 | MOVE_DRAGON_DANCE | `ov_0001::02162E38` | DragonDanceMoveAction |
| 158 | MOVE_ICE_PUNCH | `ov_0001::0215FD14` | BasicIceMoveAction |
| 159 | MOVE_ICE_BEAM | `ov_0001::0215FD14` | BasicIceMoveAction |
| 15A | MOVE_FURY_CUTTER | `ov_0001::0215675C` | HandleDamagingMove |
| 15B | MOVE_COMET_PUNCH | `ov_0001::0215675C` | HandleDamagingMove |
| 15C | MOVE_SKULL_BASH | `ov_0001::02162D98` | SkullBashMoveAction |
| 15D | MOVE_LOCK_ON | `ov_0001::0215C40C` | SureShotMoveAction |
| 15E | MOVE_ROCK_BLAST | `ov_0001::0215675C` | HandleDamagingMove |
| 15F | MOVE_COTTON_SPORE | `ov_0001::0215FC54` | ScaryFaceMoveAction |
| 160 | MOVE_STRUGGLE | `ov_0001::02162C70` | StruggleMoveAction |
| 161 | MOVE_AEROBLAST | `ov_0001::0215675C` | HandleDamagingMove |
| 162 | MOVE_VOLT_TACKLE | `ov_0001::0215F83C` | sub_8058E5C |
| 163 | MOVE_REGULAR_ATTACK | `ov_0001::0215675C` | HandleDamagingMove |
| 164 | MOVE_IS_WATCHING | `ov_0001::0215CDB4` | — |
| 165 | MOVE_BIDE_2 | `ov_0001::0215CDB4` | Bide2MoveAction |
| 166 | MOVE_DOLLAR_1 | none (no effect) | — |
| 167 | MOVE_DOLLAR_2 | none (no effect) | — |
| 168 | MOVE_WIDE_SLASH | `ov_0001::0215675C` | HandleDamagingMove |
| 169 | MOVE_EXCAVATE | `ov_0001::02162BBC` | RockSmashMoveAction |
| 16A | MOVE_SPIN_SLASH | `ov_0001::0215675C` | HandleDamagingMove |
| 16B | MOVE_SEE_TRAP | `ov_0001::02162BA4` | sub_805BA44 |
| 16C | MOVE_TAKEAWAY | `ov_0001::021629D0` | ThiefAction |
| 16D | MOVE_REBOUND | `ov_0001::021629B4` | ReboundOrbAction |
| 16E | MOVE_BLOOP_SLASH | `ov_0001::0215675C` | HandleDamagingMove |
| 16F | MOVE_SWITCHER | `ov_0001::02162984` | SwitcherOrbAction |
| 170 | MOVE_BLOWBACK | `ov_0001::02160148` | RoarMoveAction |
| 171 | MOVE_WARP | `ov_0001::0215E37C` | WarpMoveAction |
| 172 | MOVE_TRANSFER | `ov_0001::021624D4` | TransferOrbAction |
| 173 | MOVE_SLOW_DOWN | `ov_0001::021624B4` | SlowDownMoveAction |
| 174 | MOVE_SPEED_BOOST | `ov_0001::0215C4A0` | AgilityMoveAction |
| 175 | MOVE_SEARCHLIGHT | `ov_0001::0216249C` | LuminousOrbAction |
| 176 | MOVE_PETRIFY | `ov_0001::02162484` | PetrifyOrbAction |
| 177 | MOVE_STAY_AWAY | `ov_0001::02162964` | StayawayOrbAction |
| 178 | MOVE_POUNCE | `ov_0001::02162468` | PounceOrbAction |
| 179 | MOVE_TRAWL | `ov_0001::02162450` | TrawlOrbAction |
| 17A | MOVE_CLEANSE | `ov_0001::02162804` | CleanseOrbAction |
| 17B | MOVE_OBSERVER | `ov_0001::021606C0` | ObserverOrbAction |
| 17C | MOVE_DECOY_MAKER | `ov_0001::02163650` | sub_805B314 |
| 17D | MOVE_SIESTA | `ov_0001::021627C0` | SlumberOrbAction |
| 17E | MOVE_TOTTER | `ov_0001::0215E710` | SupersonicMoveAction |
| 17F | MOVE_TWO_EDGE | `ov_0001::021626A4` | TwoEdgeMoveAction |
| 180 | MOVE_NO_MOVE | `ov_0001::0216268C` | SilenceOrbAction |
| 181 | MOVE_ESCAPE | `ov_0001::021623A4` | EscapeOrbAction |
| 182 | MOVE_SCAN | `ov_0001::02162674` | ScannerOrbAction |
| 183 | MOVE_POWER_EARS | `ov_0001::0216265C` | RadarOrbAction |
| 184 | MOVE_DROUGHT | `ov_0001::0216238C` | DroughtOrbAction |
| 185 | MOVE_TRAP_BUSTER | `ov_0001::02162198` | TrapbustOrbAction |
| 186 | MOVE_WILD_CALL | `ov_0001::02162098` | RollcallOrbAction |
| 187 | MOVE_INVISIFY | `ov_0001::0216207C` | InvisifyOrbAction |
| 188 | MOVE_ONE_SHOT | `ov_0001::02161FEC` | OneShotOrbAction |
| 189 | MOVE_HP_GAUGE | `ov_0001::02161FD4` | IdentifyOrbAction |
| 18A | MOVE_VACUUM_CUT | `ov_0001::02161FA0` | VacuumCutMoveAction |
| 18B | MOVE_REVIVER | `ov_0001::02161F60` | ReviverOrbAction |
| 18C | MOVE_SHOCKER | `ov_0001::02161F44` | ShockerOrbAction |
| 18D | MOVE_ECHO | `ov_0001::02161EA8` | sub_805C208 |
| 18E | MOVE_FAMISH | `ov_0001::02161E80` | sub_805C288 |
| 18F | MOVE_ONE_ROOM | `ov_0001::02161E68` | OneRoomOrbAction |
| 190 | MOVE_FILL_IN | `ov_0001::021637FC` | FillInOrbAction |
| 191 | MOVE_TRAPPER | `ov_0001::021637C8` | TrapperOrbAction |
| 192 | MOVE_POSSESS | none (no effect) | — |
| 193 | MOVE_ITEMIZE | `ov_0001::02163724` | ItemizeOrbAction |
| 194 | MOVE_MD1 | none (no effect) | — |
| 195 | MOVE_PROJECTILE | none (no effect) | — |
| 196 | MOVE_HURL | `ov_0001::0216370C` | HurlOrbAction |
| 197 | MOVE_MOBILE | `ov_0001::021636F4` | MobileOrbAction |
| 198 | MOVE_ITEM_TOSS | none (no effect) | — |
| 199 | MOVE_SEE_STAIRS | `ov_0001::021636DC` | StairsOrbAction |
| 19A | MOVE_LONG_TOSS | `ov_0001::021636C4` | LongtossOrbAction |
| 19B | MOVE_MD1_2 | none (no effect) | — |
| 19C | MOVE_PIERCE | `ov_0001::021636AC` | PierceOrbAction |
