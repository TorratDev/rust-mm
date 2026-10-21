# Turn loop

How a dungeon floor advances, turn by turn. Issue: [#49](https://github.com/TorratDev/rust-mm/issues/49). Names, addresses and tags are in [`symbols.md`](../symbols.md). The code is in overlay 1 ([`execution-model.md`](execution-model.md)) and matches `pmd-red`'s `src/dungeon_engine.c` call for call. Confirmed by emulator on the retail ROM: the trigger fires on every fractional turn and `fractionalTurn` cycles through 0–23 (V7).

The sections added for #49 (speed, other monsters' turns, movement, running and swapping, per-turn effects) rest on static evidence only: Ghidra decompiles, xrefs and overlay 1 bytes (`RAM base 0x02138860`), compared with `pmd-red` at commit `013475aa04f5be3191e5527c186d9bfceae7cae0`. None of them has an emulator check. The V7 check above is the only runtime evidence on this page.

## The floor loop

`RunDungeon_Async` (`0206DC20`) runs each floor with:

```
param = TRUE
do {
    RunFractionalTurn_Async(param)   // ov_0001::021411BC
    param = FALSE
} while (!IsFloorOver())            // ov_0001::0214067C
```

Before the loop it clears `dungeon+0x648` (Red: `unk644.unk10`, the floor-end reason that `IsFloorOver` sets to 1 or 2).

## One fractional turn

`RunFractionalTurn_Async` (`021411BC`) runs these steps in order, calling `IsFloorOver` (`0214067C`) before each one and returning as soon as it is true. The order and call sites are blue-verified (x-refs from `021411BC`, matching Red `src/dungeon_engine.c:44-77`). Confidence high.

1. `TrySpawnMonsterAndActivatePlusMinus_Async` (`0214078C`), gated on `gSpeedTurns[1][fractionalTurn] != 0`.
2. `RunLeaderTurn_Async(param)` (`02140FAC`). See [Speed](#speed).
3. Only if step 2 returned TRUE: `UpdateWindTurns` (`02195104`, called from `02141200`). See [Per-turn effects](#per-turn-effects).
4. `sub_8044574_Async` (`02140AC0`): team non-leaders. See [Other monsters' turns](#other-monsters-turns).
5. `sub_8044820_Async` (`021408F8`, called from `0214122C`): wild monsters. See [Other monsters' turns](#other-monsters-turns).
6. `sub_8044AB4` (`021406F8`, called from `0214123C`): gated on `gSpeedTurns[1][fractionalTurn + 1] != 0`. It runs the weather tick and the forced-loss check. See [Per-turn effects](#per-turn-effects).
7. `fractionalTurn` (`dungeon+0x650`, Red: `unk644.fractionalTurn`) goes up by one and wraps to 0 at 24.

Names by content, not applied in Blue: step 5 is what the investigators suggest calling `RunWildMonsterTurns_Async`; step 6 is `TickWeatherAndCheckForcedLoss`. Nothing on this page renames them.

Status countdowns, and the per-turn burn and poison damage, are covered in [`status.md`](status.md).

The 24 fractional turns let monsters with different speeds act different numbers of times: `RunLeaderTurn_Async` looks up `gSpeedTurns[CalcSpeedStage(leader)][fractionalTurn]` (`ov_0001::021ABC84`, `s16[5][25]`, same values as Red) to decide whether the leader acts in this fractional turn.

## Speed

Tags: the stage formula, the table and the gate are blue-verified, with high confidence. The speed sources are red-hypothesis, with medium confidence, because their Blue callers were not located.

**Stage.** `CalcSpeedStage` (`02181E58`) returns the speed stage, clamped to 0–4, and stores it at `+0xE0`. It matches Red `src/dungeon_logic.c:324` term by term:

- +1 for each nonzero speed-up counter (`+0xE4` to `+0xE8`).
- −1 for each nonzero speed-down counter (`+0xE9` to `+0xED`). Red's speed-down counters are the same array as its speed-up counters 5 to 9; the two overflow into each other at Red `0x108` and `0x10D`.
- −1 if the burn-class status is paralysis (`+0x9B`, value 4).
- + the species movement speed from `FUN_020023A8` (not named in Blue; Red `pokemon.c:754`).
- +1 if the monster is ice type (6) and the apparent weather (`FUN_02195C48`) is snow (7).
- +1 if the apparent ID (`+0x04`) is 419 (Deoxys in Speed form).
- +1 if the species is 380 (Kecleon), the monster is not a team member (`+0x06`), and `gDungeon+0x65E` (Red `stoleFromKecleon`) is set.

**Table.** `gSpeedTurns` (`ov_0001::021ABC84`, `s16[5][25]`) is byte-identical to Red `src/dungeon_engine.c:36`. The gated turns in each 24 fractional turns are:

| Speed stage | Gated turns per 24 | Fractional turns (0–23) |
|-------------|--------------------|-------------------------|
| 0 | 3 | 7, 15, 23 |
| 1 | 6 | 3, 7, 11, 15, 19, 23 |
| 2 | 12 | every odd turn, 1 to 23 |
| 3 | 18 | 1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15, 17, 18, 19, 21, 22, 23 |
| 4 | 24 | every turn |

**Gate.** A monster acts in a fractional turn only if `gSpeedTurns[CalcSpeedStage(monster)][fractionalTurn] != 0`. The same gate is used by the leader (`RunLeaderTurn_Async`, `02140FAC`), team non-leaders (`02140AC0`) and wild monsters (`021408F8`). All three also require `attacking` (`+0x11A`) to be 0. Red: `dungeon_engine.c:79-100`, `208-240`, `351`. Tag blue-verified, confidence high.

The two turn-loop steps that are not tied to a monster's speed use row 1 directly: the spawn step at `fractionalTurn`, and the weather step at `fractionalTurn + 1`.

**Speed sources** (red-hypothesis for the callers; Red names used):

- Speed up, through `BoostSpeed` (Red `move_orb_effects_1.c:1348`), which writes the first free speed-up slot with the turn count: Agility (`move_orb_actions_1.c:957`), Ancient Power (`:938`, self), Dragon Dance (`move_orb_actions_4.c:309`), Quick Seed (`dungeon_item_action.c:422`), and the end-of-turn boost (`dungeon_turn_effects.c:193`, 0x7F turns).
- Slow, through `LowerSpeed` (Red `move_orb_effects_1.c:1392`), which writes the first free speed-down slot with `CalculateStatusTurns(...)+1`, once per stage: Bubble (`move_orb_actions_1.c:176`), String Shot (`:296`), `sub_8058580` (`:761` and `:791`), Scary Face (`move_orb_actions_2.c:104`), Secret Power (`move_orb_actions_3.c:709`), Slow Down (`move_orb_actions_4.c:605`), Curse on self (`move_orb_effects_2.c:528`), and the slow trap (`trap.c:496`).

Counters are not stat changes in themselves. Snow, Deoxys Speed, Kecleon and paralysis are fixed terms in `CalcSpeedStage`.

**Counter countdown.** Each gated pass decrements the counters in `TickStatusAndHealthRegen` (`02185C70`, decompile lines 127–134): `+0xE9+i` and `+0xE4+i`. It runs once per monster that passes the gate, not once per fractional turn. Tag blue-verified, confidence high. Red: `dungeon_turn_effects.c:549-560`.

**Actions per turn.**

- Leader: after its action, `RunLeaderTurn_Async` clears `speedStageChanged` (`+0xD5`). The loop repeats while `unkBC` (offset not checked) is nonzero or the speed stage changed. Each repeat sets `fractionalTurn` to 0. Tag blue-verified (the loop tail and its checks), confidence medium-high. Red: `dungeon_engine.c:142-144`.
- Team non-leader: one `ExecuteEntityDungeonAction` cycle per gated turn. The cycle repeats, up to 3 cycles in total, only while Execute returns TRUE, which Red sets only after an interrupted action. Blue `0218453C` returns 1 on the same path. Tag blue-verified, confidence high.
- Wild: exactly one cycle per gated turn. Tag blue-verified, confidence high.

## Other monsters' turns

Tags: the order, the gates and the call sites are blue-verified, with high confidence. The writers of `aiAllySkip` are red-hypothesis, with medium confidence.

Team loop, `sub_8044574_Async` (`02140AC0`). The team array is at `dungeon+0x11314`, 4 slots.

**Phase 1** (slots 0 to 3, leader skipped). Per monster:

- Gate: the speed gate, `!attacking` and `!(flags & 0x8000)`. Flag `0x8000` is the petrified-ally swap flag (see [Running and swapping](#running-and-swapping)).
- If `flags & 0x4000`: clear it and skip this monster.
- Otherwise: `recalculateFollow` (`+0x117`) = 0, `TickStatusAndHealthRegen`, `EnemyEvolution` (`02183BB4`), then up to 3 cycles of `RunMonsterAI(0)` (`02188030`) and `ExecuteEntityDungeonAction` (`0218453C`), repeating a cycle only while Execute returns TRUE. Then `FlushTempCutsceneFlags` (`0219DA90`, Red `sub_8086AC0`) and `TryForcedLoss` (`0206D7B0`).

**Phase 2** (3 passes). Each pass collects team members whose `aiAllySkip` (`+0x116`) is set. The leader is reset. The others are bucketed by Chebyshev distance to the leader: 0, 1, and 2 or more (capped at 2). Each bucket is LIFO, in slot order of insertion. Each monster acts with `RunMonsterAI(1)`, Execute, `FlushTempCutsceneFlags` and `TryForcedLoss`. Phase 2 has no speed gate. The bucket array has an overrun in the decompile; see the open questions.

**End of the team loop.** If the floor is not over, each team member still flagged `aiAllySkip` gets `DoEndOfTurnEffects` (`021861E8`), then `EnemyEvolution`, and the flag is cleared.

Wild loop, `sub_8044820_Async` (`021408F8`). It has 16 slots at `dungeon+0x11324`. Per monster:

- Skip if `flags & 0x8000`. If `flags & 0x4000` is set, it is cleared and the monster is skipped. Evidence: Ghidra decompile of `ov_0001::021408F8` (the `else` branch runs `flags &= 0xBFFF`), reviewer spot check 2 in `rescue-arm9-c`; this confirms investigator A's notes.
- Clear `aiAllySkip`, then the gate: the speed gate and `!attacking`.
- If the gate passes: clear `flags & 0xC000`, then `recalculateFollow` = 0, `TickStatusAndHealthRegen`, `EnemyEvolution`, one cycle of `RunMonsterAI(0)`, Execute, `FlushTempCutsceneFlags` and `TryForcedLoss`. `IsFloorOver` is checked between steps, and the loop breaks when it is true.
- End: if the floor is not over, each wild monster still flagged `aiAllySkip` gets `DoEndOfTurnEffects` and `EnemyEvolution` (if still valid), then the flag is cleared.

**Who sets `aiAllySkip`** (red-hypothesis, from `pmd-red`'s `dungeon_ai_movement.c`, lines 310–448): `DecideMovement` sets it on allies at line 361 (team members only: `!isNotTeamMember`, not recalculating follow, a monster in front), and on any non-leader with `aiNotNextToTarget` at line 443, which includes wild monsters. Lines 436–441 clear it for the leader. The Blue writers were not located; they belong to the AI (#52).

**Execute runs end-of-turn effects too.** Red's `ExecuteEntityDungeonAction_Async` runs `DoEndOfTurnEffects` and `EnemyEvolution` inside the action when `aiAllySkip` is clear (`dungeon_action_execution.c:312-313`). Blue `0218453C` has the same check on `+0x116` at its tail; the call is at `02184DF8`. So the phase-2 end-of-turn pass reaches only the monsters that stayed skipped. Tag blue-verified (call and gate, from the decompile), confidence high.

`ExecuteEntityDungeonAction` is named here by its content only; the Blue name is not set.

## Movement

Tags: `CanMoveInDirection` and the walkable-mask writer are blue-verified, with high confidence. The direction and bitmask tables were read from overlay 1 bytes and equal Red's.

Direction tables: `gAdjacentTileOffsets` at `ov_0001::021A98B4` (s16 x, s16 y, stride 4) gives direction 0 as (0, 1), 1 as (1, 1), 2 as (1, 0), 3 as (1, −1), 4 as (0, −1), 5 as (−1, −1), 6 as (−1, 0), 7 as (−1, 1). The direction bitmasks at `021AE2A0` are `01 02 04 08 10 20 40 80`.

**`CanMoveInDirection` (`0218141C`)**, in this order:

1. The destination tile with `terrainFlags & 0x10` (impassable wall) is blocked. A monster on the destination tile (tile `+0xC`) blocks it.
2. The mobility class. The overrides below apply only when `tileset <= 0x3F` (not a fixed boss floor; Red `DUNGEON_OUT_ON_RESCUE = 63`). The base class is `GetCrossableTerrain(species)` (`0218267C`).
   - Status `+0xC7` = 3 (STATUS_MOBILE), or held item 9 (Mobile Scarf): class 3 (wall) in all directions.
   - Otherwise IQ skill 12 (all-terrain hiker): class 2 (crevice).
   - Otherwise IQ skill 13 (super mobile): class 3 for even directions, class 2 for odd ones. Red's comment says Super Mobile cannot break walls diagonally.
3. The source tile's mask byte at `+6 + class` (regular 0, liquid 1, crevice 2, wall 3), ANDed with `1 << (dir & 7)`, must be nonzero.

Tag blue-verified for the code path, confidence high. Not verified: the body of the IQ test (`021805B0`, bit `1 << id` at info `+0x87`) was taken from its operands, not decompiled. Naming `+0xC7` as the invisible-class status is inferred from the value 3.

**Walk branch** (`0218453C`, case `ACTION_WALK` = 2). Tag blue-verified, confidence high for the branch structure and medium for the wall-break helper.

- If a status blocks movement (shadow hold, constriction, ingrain, wrap, or wrapped), the walk does not move.
- If the info flags have `0x8000` (swap with a petrified ally), the move check is skipped.
- Otherwise `CanMoveInDirection` is checked. If it is blocked, the action becomes `ACTION_NOTHING` and info `+0x119` (`waiting`) is set to 1.
- Otherwise the move helper (`02177700`) runs, then the step animation (`02187EE0`). For a super mobile monster without a Mobile Scarf, `FUN_02147B9C` (Red `sub_804AE08`; identity not confirmed) runs the wall break.

**Walkable-neighbour masks** (`02148F50`, Red `sub_8049BB0`, `dungeon_map_access.c:341`). It writes four bytes per tile: `+6` regular, `+7` liquid, `+8` crevice, `+9` wall. Tag blue-verified, confidence high. Terrain values are taken with `& 3` (NORMAL 1, SECONDARY 2, wall 0):

- Regular: a cardinal bit when the neighbour is exactly NORMAL. A diagonal bit when the diagonal is exactly NORMAL and both orthogonal neighbours are not walls. A diagonal step therefore never cuts a wall corner.
- Liquid: NORMAL or SECONDARY, with the same diagonal rule.
- Crevice: any nonwall for a cardinal bit. A diagonal needs all three tiles to be nonwall.
- Wall: all bits, minus the map edges. For `x < 2` the byte is `0x1F`; `y < 2` clears `0x38`; `x > 0x35` clears `0x0E`; `y > 0x1D` clears `0x83`. The three clears equal Red's `~(8|0x10|0x20)`, `~(2|4|8)` and `~(0x80|1|2)`.

**Bit `0x4` is not read by movement.** `TERRAIN_TYPE_CORNER_CUTTABLE` (`0x4`) is read neither by the mask writer nor by `CanMoveInDirection`. Corner cutting at runtime is decided by the nonwall rule above. Red's header says the bit is "seemingly only used during dungeon generation", which these two functions agree with. Tag blue-verified for the two functions and red-hypothesis that no other runtime reader exists. Confidence medium. A search of every `tile & 4` reader in overlay 1 is open.

## Running and swapping

**Run decision** (`02166B10`, the leader's input and action selection). Tag blue-verified for the write pattern; red-hypothesis for the mapping of the fields to Red's names. Confidence medium.

- When B is held (`*keys & 2`) or the press flag is set, and the belly test `FUN_0200B9A4` on the leader is nonzero: the leader's action index (`+0x46`) becomes 0 (run), and the run flag `dungeon+0x65C` becomes 1. Red's names: `actionUseIndex` and `unk28`.
- Otherwise the action index becomes 1 (walk), and the run flag is not set.

The belly test is not yet settled; see the open questions.

**Run frames** (`02140FAC`, `RunLeaderTurn_Async`). When `dungeon+0x65C` or `dungeon+0x663` (Red `unk2F`) is nonzero and the leader's display did not act, it calls `DungeonRunFrameActions(0xC)`. Tag blue-verified, confidence high for the checks. Red: `unk28 || unk2F`.

## Per-turn effects

`DoEndOfTurnEffects` (`021861E8`) returns `bool` in Blue, where Red's `DoEndOfTurnEffects_Async` returns `void`. The behaviour matches on the paths checked; see [`blue-vs-red.md`](../blue-vs-red.md). Tag blue-verified (callers), confidence medium. It is called per monster from `ExecuteEntityDungeonAction` (`0218453C`, call at `02184DF8`) when the entity is valid, `aiAllySkip` is clear and it is not the special case (Red `dungeon_action_execution.c:308-312`), and from the end-of-turn passes above.

**Belly** (P6.1). Only the team leader (info `+0x7`) has its belly decreased, in the belly block of `DoEndOfTurnEffects`. The decrement is read from a table by index. The base index is 10, and held items adjust it: Tight Belt, Stamina Band, Energy Saver, Diet Ribbon, Heal Ribbon and Munch Belt. The index is clamped to the table length. Tag blue-verified for the gate (leader only); red-hypothesis for the arithmetic, which matches Red by inspection. Confidence medium. The decrement constant, `gBellyGoDownValues` and the linked-moves term are not checked.

**Starving** (P6.2). When the belly reaches 0, the empty-belly alert counter at `dungeon+0x669` goes up, capped at 10. Messages are chosen on counter values 1, 2 and 3, as in Red's `EMPTY_BELLY_ALERT` steps. The entity's `bellyEmpty` (`+0x118`) is set to TRUE. Tag blue-verified for the counter; the hunger damage call was not checked. Confidence medium.

**HP regeneration** (P6.3), in `TickStatusAndHealthRegen` (`02185C70`). HP regen runs first, and is skipped if `bellyEmpty` is set or the status is 2 or 3 (poisoned or badly poisoned).

- Only team members (`+0x06` clear) get a regen speed, read by `FUN_02002368` from the species ID (`+0x02`). Heal Ribbon (item 10), Wish (status 6) and Rain Dish (ability 3 in weather 4) add to it.
- The total is clamped to 30–500.
- Each pass adds maxHP (`+0x10`) to the accumulator at `+0x1C4`. Each time the accumulator reaches the regen speed, HP (`+0x0E`) goes up by 1, capped at maxHP.

The status countdowns after HP regen belong to status countdowns (#51). Tag blue-verified for the arithmetic, red-hypothesis for the names; confidence high. Callers: `021409CC` (`sub_8044820`), `02140BA0` (`sub_8044574`), `02140F40` (`FUN_02140ED0`) and `021410A4` (`RunLeaderTurn_Async`). So regen runs once per gated turn of each monster, not once per fractional turn. Not checked: whether the status chain calls RNG.

**Weather** (P6.4). The weather resolver `021953B4` (Red `sub_807EAA0`, `src/weather.c:251`) runs in this order:

1. For each of 8 weather sources (the byte at `gDungeon+0xC107+i`, with its natural value at `+0xC10F+i`), move the value toward the natural value and take the highest.
2. Set the current weather.
3. Decrement `weatherDamageCounter` (`+0xC106`), or set it to 9 if it is 0.
4. Tick the mud and water-sport timers.

Tag blue-verified for the tick and the reset (step 3); red-hypothesis for the rest of the mapping. `DoEndOfTurnEffects` applies hail and sandstorm damage only when the counter is 0. The damage is skipped for ice types (hail) and for ground, rock and steel types (sandstorm). Tag red-hypothesis.

Cadence: the per-turn caller is inside `sub_8044AB4` (`0214076C`), which runs only when `gSpeedTurns[1][fractionalTurn + 1] != 0`. So the counter moves 6 times per 24 fractional turns. Tag blue-verified, confidence medium. Eleven other callers are not classified: `0215DDE8`, `0215DE5C`, `0215EF50`, `021615A8`, `02195C38`, `021A2BC0`, `021A28E0`, `021A279C`, `021A5758`, `021A5678` and `021A55BC`. Red's `weather.c:56` is the floor-setup call.

**Wind** (P6.5), in `UpdateWindTurns` (`02195104`). It is called at `02141200` only when `RunLeaderTurn_Async` returns TRUE, and returns early when there is no leader. It decrements `windTurns` (`+0x656`) while it is above 0. Phase 0 starts below 250 turns, phase 1 below 150, and phase 2 below 50. The phase is stored in `windPhase` (`+0x66A`). Each phase logs a message, repositions the camera and sets the next phase. Tag blue-verified for phases 0 to 2; red-hypothesis otherwise. Confidence medium. Phase 3 is open.

**RNG per turn** (P6.6). Tag blue-verified, confidence medium because the status chain was not checked in full.

- `DoEndOfTurnEffects` makes two `DungeonRandInt(100)` calls, in this order. First, Warp Scarf (item 22), only when the entity holds it and its turns-since counter (`+0x137`) is set; the roll is compared with the chance table. Second, Shed Skin, unconditionally, before the ability check (ability 0x21). Red does the same.
- None is visible in the decompiles of `UpdateWindTurns`, the weather resolver, or `TickStatusAndHealthRegen`. The status chain of `TickStatusAndHealthRegen` is not checked in full (#51).
The monsters' AI decision procedure, `RunMonsterAI` (`ov_0001::02188030`), is described in [`ai.md`](ai.md). How step 3 reaches it has not been traced.

## For golden traces (V7)

- **Trigger:** the entry of `RunFractionalTurn_Async`, RAM `0x021411BC`, while overlay 1 is loaded. Overlays 0 and 7 use the same RAM range when no dungeon runs, so the hook must only count hits after the dungeon has started (for example after `LoadAndRunDungeon_Async` has loaded overlay 1).
- **Per-trigger state:** `fractionalTurn` at `dungeon+0x650`, with the dungeon struct at the pointer stored in `gDungeon` (`0x020E0788`), plus the RNG state from [`rng.md`](rng.md).
- One player turn spans several fractional turns. To dump once per player action instead, hook `RunLeaderTurn_Async` (`ov_0001::02140FAC`) and keep the hits where it returns true.

## Dungeon struct offsets seen so far

| Blue offset | Red field | Red offset | Notes |
|-------------|-----------|------------|-------|
| `+0x648` | `unk644.unk10` (floor-end reason) | `0x654` | |
| `+0x650` | `unk644.fractionalTurn` | `0x660` | wraps at 24 |
| `+0x656` | not matched (`windTurns` in the notes) | — | counts down the wind; [Per-turn effects](#per-turn-effects) |
| `+0x65C` | `unk28` | `0x66C` | run flag; [Running and swapping](#running-and-swapping) |
| `+0x668` | `unk644.missionKind` | `0x678` | [`rng.md`](rng.md) |
| `+0x669` | not matched | — | empty-belly alert counter, capped at 10 |
| `+0x66A` | not matched (`windPhase` in the notes) | — | wind phase 0 to 2 |
| `+0x66C` | `unk644.prngPreseed23Bit` | `0x67C` | [`rng.md`](rng.md) |
| `+0x670` | `unk644.unk3C` (floor seed) | `0x680` | [`rng.md`](rng.md) |
| `+0x15FB0`, `+0x15FB1` | `unk181e8.unk18218`, `unk18219` | `0x18218`, `0x18219` | Blue's struct is about 0x2268 bytes shorter before these |

`unk644` starts at `0x638` (Red `0x644`); its fields from `+0x14` on are 4 bytes earlier inside it. The full map is in [`structs.md`](structs.md).

## Entity info offsets seen so far

| Blue offset | Red field | Red offset | Notes |
|-------------|-----------|------------|-------|
| `+0xD5` | `speedStageChanged` | `0xF8` | the leader repeats while it is set; [Speed](#speed) |
| `+0xE0` | `speedStage` | `0x104` | written by `CalcSpeedStage` |
| `+0x11A` | `attacking` | `0x148` | gate of all three turn loops; [Speed](#speed) |
| `+0x1C4` | `unk1F8` (HP regeneration counter) | `0x1F8` | [Per-turn effects](#per-turn-effects) |

The full map is in [`structs.md`](structs.md).

## Open questions

- **P2-4, `aiAllySkip` writers.** The Blue writers of `+0x116` were not located. Red's are in `DecideMovement` (`dungeon_ai_movement.c` lines 361, 443 and 436–441). This depends on the AI (#52).
- **P2-6, phase-2 bucket overrun.** The decompile of `sub_8044574_Async` ends a bucket list past its 6-slot array (`local_68+6`). It works only if the next stack slot is zero; Ghidra shows `local_50` zeroed there. Check the disassembly before porting. Confidence low.
- **Speed counters.** Whether Blue applies the same stage-specific counter limit as Red was not verified. The Blue addresses of `BoostSpeed` and `LowerSpeed` were not located; `GetMovementSpeed` is `FUN_020023A8`.
- **P4.5, water and lava mobility.** `GetCrossableTerrain` (`0218267C`) maps movement types 4 and 5 to regular or liquid by the dungeon's liquid. The table at `DAT_0218269C` and the dungeon liquid test were not decoded. The species movement values are not checked.
- **Edge masks.** The `x < 2` wall byte `0x1F` is written as a value, while the other three edge cases clear bits. Check the disassembly of `02148F50`.
- **IQ test and `+0xC7`.** The body of `021805B0` was not decompiled. The name of `+0xC7` as the invisible-class status is inferred from the value 3.
- **P4.4, bit `0x4` readers.** A search of every `tile & 4` reader in overlay 1 has not been done.
- **P5.1, belly test.** `FUN_0200B9A4` returns `lo+1` when `lo != 0`, and `hi != 0` when `lo == 0`. A belly between 0 and 1 (zero integer part) may count as nonzero here, where Red's `FixedPointToInt(belly) != 0` gives 0. Settle it before porting. The argument `info+0x10E` is not checked against Red's belly field. The action-index field `+0x46` is not matched to a Red name.
- **P5.3, run-stop conditions.** Not traced. Map the tests in `02166B10` (lines 600 to 690) against Red's `CheckLeaderTile` (`dungeon_main.c:928`).
- **P5.4 and P5.5, swap flags and setters (#52).** The setter of `0x8000` (the petrified-ally swap) was not found in Blue; Red's is `sub_805EC4C` (`dungeon_main.c:860`). The normal-swap flag `1 << 5` (`MOVEMENT_FLAG_SWAPPING_PLACES`) has no setter found in Red's `src`.
- **P6.1, belly constant.** The decrement constant is not checked against Blue. Red's value is 0.1 in s48.16 (6554). `gBellyGoDownValues` and the linked-moves term are not checked.
- **P6.2, hunger damage.** Red uses `DealDamageToEntity_Async(entity, 1, RESIDUAL_DAMAGE_HUNGER, DUNGEON_EXIT_FAINTED_FROM_HUNGER)`. The Blue call was not confirmed.
- **P6.4, 11 unclassified callers** of the weather resolver: `0215DDE8`, `0215DE5C`, `0215EF50`, `021615A8`, `02195C38`, `021A2BC0`, `021A28E0`, `021A279C`, `021A5758`, `021A5678` and `021A55BC`.
- **P6.5, wind phase 3.** The notes say `windTurns` below 1 sets `windPhase` to 3. The branch was not checked.
- **RNG in the status chain** of `TickStatusAndHealthRegen` (#51).
- **`DisplayActions_Async` label.** `DoEndOfTurnEffects` has a caller at `02187B68`, inside the function labelled `DisplayActions_Async`. That label was not checked against Red.
- **`DoEndOfTurnEffects` return type.** Blue returns `bool` and Red returns `void`. Check whether the Blue callers use the return value.
