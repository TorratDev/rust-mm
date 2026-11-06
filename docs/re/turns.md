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

**Phase 2** (3 passes). Each pass collects team members whose `aiAllySkip` (`+0x116`) is set. The leader is reset. The others are bucketed by Chebyshev distance to the leader: 0, 1, and 2 or more (capped at 2). Each bucket is LIFO, in slot order of insertion. Each monster acts with `RunMonsterAI(1)`, Execute, `FlushTempCutsceneFlags` and `TryForcedLoss`. Phase 2 has no speed gate. The bucket list is not an overrun; see **Bucket layout (#184 P2)** below.

**Bucket layout (#184 P2).** **[blue-verified]**, high confidence. Frame: `sub sp,sp,#0x64` at `02140AC4`. The sentinel array is at `sp+0x20` (`r6`), with 8-byte nodes `{value, next}`. Nodes 0, 1 and 2 are the three buckets, initialised in the loop at `02140C50` to `02140C68` (`value = 0`, `next` = the next node). Node 3 is a fourth terminator at `sp+0x38`, zeroed at `02140C70` and `02140C74`. The decompile shows it as the locals after `local_68[6]`, which looks like a write past that array; the disassembly places it at `sp+0x38`, inside the frame, so it is not an overrun. The member nodes are at `sp+0x40` to `sp+0x5F`, one per team slot (index 0 to 3; `cmp r8,#0x4` at `02140D18`). Insertion (`02140CFC` to `02140D10`) prepends a node `{monster, bucket.next}`. The bucket index is the Chebyshev distance `max(|dy|, |dx|)` from the leader (`FUN_0219C23C()`); 3 or more takes index 2. The walk from sentinel 0 (`02140D20` to `02140D74`) visits each bucket's monsters, last inserted first, and skips value-0 nodes. A leader (`+7 != 0`) entry clears `+0x116` and `+0x117` (`02140CB4` and `02140CB8`) and is not inserted. Each visited monster gets `+0x117 = 1` and `+0x116 = 0`, then `RunMonsterAI(1)`, Execute, `FlushTempCutsceneFlags`, `TryForcedLoss` and the validity check. There are three passes (`[sp,#0x0]` below 3). The 4-slot team array is why the bound is 4 member nodes. Port consequence: three sentinels and one terminator, four member nodes; no out-of-bounds write.

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
2. The mobility class. The overrides below apply only when `tileset <= 0x3F` (not a fixed boss floor; Red `DUNGEON_OUT_ON_RESCUE = 63`). The base class is `GetCrossableTerrain(species)` (`0218267C`); see [Mobility class source](#mobility-class-source-184-p1).
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

**Bit `0x4` is not read by movement.** `TERRAIN_TYPE_CORNER_CUTTABLE` (`0x4`) is read neither by the mask writer nor by `CanMoveInDirection`. Corner cutting at runtime is decided by the nonwall rule above. Red's header says the bit is "seemingly only used during dungeon generation", which these two functions agree with. Tag blue-verified for the two functions. Confidence medium. The sweep of overlay 1 for other readers is below.

**Readers and writers of bit `0x4` (#184 P3).** **[blue-verified]** for the call set. Confidence medium-high: the search is a pattern search over decompile text, not a proof of absence.

- Reader: `FUN_ov_0001__0214A760` (`StairsAlwaysReachable`) only. It tests `(*tile & 4) == 0` in two places: a non-NORMAL tile sets bit 1 of the work byte, and a SECONDARY tile sets bit 2. Same as Red `dungeon_generation.c:5610` and `:5616` **[red-hypothesis]**.
- Clearer: `FUN_ov_0001__02151D4C` (`MarkNaturalJunctions`) clears it on every tile (already documented).
- Writer: `FUN_ov_0001__0214CA90(tile, flag)` sets the secondary type and ORs `flag` (Red `SetTerrainSecondaryWithFlag`, **[red-hypothesis]**). It is called 24 times with `flag = 4`, from `FUN_ov_0001__0214C244` (the 5 by 5 block shapes of Red `dungeon_generation.c:3784-3797`, **[red-hypothesis]**). No other writer.
- Not readers (false hits): `& 4` or `0xFFFB` on the second halfword (`+2`, the spawn flags) in `0214B650`, `0214C1A8`, `0214DB54`, `02148B7C` and `02198824`; and the `0xFFFB` local masks in `0213CD5C` and its aliases.
- Method: every tile access in overlay 1 goes through `FUN_02149A08` (read, `GetTile`) or `FUN_02149990` (copy), the only two functions that index the pointer grid (`dungeon + 0xC660`). The grid-init `FUN_021498F8` is the only other code that touches it. All 153 callers were decompiled, plus a further 2,024 functions of the overlay (the whole function list, duplicates and misaligned aliases included), searched for `& 4`, `& 0xfffb` and `| 4` on the first halfword of a tile.
- Not covered: reads of the terrain word through a pointer obtained from anywhere other than these two accessors (none found); the ARM9 and the other overlays (the ARM9 never indexes the grid, but no tile-accessor references there were checked). The port's `TERRAIN_FLAG_04` in `crates/sim/src/floor.rs:2514` and the `0x4` row of [`structs.md`](structs.md) already match.

### Mobility class source (#184 P1)

**[blue-verified]** (decompile, disassembly, pool words and a trace), high confidence. The Red names and the Red comparisons are **[red-hypothesis]**. There is no species mobility table in overlay 1. The class comes from two parts.

- **Per-species byte.** `FUN_0200238C(species)` (ARM9) returns byte `+0x15` of the species' record in `monspara`: `*(u8 *)(*(u32 *)0x020DDF80 + species * 0x48 + 0x15)`. This is Red's `GetMovementType`. `0x020DDF80` points at the first record, which `LoadMonsterParameters` (`FUN_02003794`) sets. The file is copied to VRAM at `0x06608000` (see #178 P2) and fixed up by `FUN_0201A134`. The first record is at `0x06609AD0` (file offset `0x1AD0`), and the SIR0 relocation data starts at `0x06611210`, which gives `(0x06611210 - 0x06609AD0) / 0x48` = 424 records. The record size is `0x48`, the field is a u8 at offset `+0x15`, and the index is the species id (s16), with no bounds check. Callers of `FUN_0200238C`: `0217B120` (`0217B360`), `FUN_0218267C` (`02182684`) and `FUN_021760B4` (`02176128`).
- **Remap, in `FUN_0218267C`.** `v = FUN_0200238C(species)`. If `v < 4`, the class is `v`. Otherwise the class is `remap[v]`, a byte read from **`0x021B4834`**, which is 8 bytes of overlay RAM. The pool word is `0x0218269C` and the load is `ldrbcs r0,[r1,r0]` at `02182690`. The return range is classes 0 to 3 (regular, liquid, crevice, wall).
- **Who writes `remap`.** `FUN_ov_0001__0214969C` (dungeon setup; pool word `0x021498F0`, stores from `02149878`). It sets entries 0 to 3 to 0, 1, 2 and 3. Then it reads `t`, the byte at `0x02091EFC[tileset]` (the tileset is the s16 at dungeon `+0x34AE`). If `t == 1` (lava), it sets `remap[4] = 1` and `remap[5] = 0`; otherwise it sets `remap[4] = 0` and `remap[5] = 1`. This matches Red's `dungeon_map_access.c:112-120` **[red-hypothesis]**: LAVA is treated as LIQUID and WATER as REGULAR in a lava dungeon, and the reverse elsewhere. So class 4 crosses liquid on lava floors only. The Fire-type crossing in Red that this resembles was not checked here.
  - `0214969C` has no direct caller. It is reached through the dungeon setup, which was not traced.
  - The table at `0x02091EFC` is not otherwise described in this file.
- **Trace (#184 P1).** A py-desmume probe (`probe.py`) on a scratch ROM copy, with the boot inputs from [`boot_to_dungeon.txt`](../../verification/v7-trace/boot_to_dungeon.txt), 90 frames, and `XDG_CONFIG_HOME` set to a scratch directory. Command: `~/venvs/desmume/bin/python -I probe.py rom184.nds`, run twice. Both outputs have SHA-1 `a83ecc73d1838d320b502094e4a790a09e1b308f` (identical). `probe.py` SHA-1 `6fcecdf4d21608490357a80a2c641d38e16d104c`. Result: the `+0x15` byte takes **5 distinct values among the 424 records, all in 0 to 5**, and value 1 does not occur. Per-species values are not recorded (legal rule 3).
- **Not verified.** The dungeon-setup caller chain of `0214969C`. Which tileset ids have table value 1 (lava); only the structure was read.
- **Port consequence.** `GetCrossableTerrain` needs only the monster record byte `+0x15` and the two remap entries written at setup. No new table to commit.

## Running and swapping

**Run decision** (`02166B10`, the leader's input and action selection). Tag blue-verified for the write pattern; red-hypothesis for the mapping of the fields to Red's names. Confidence medium.

- When B is held (`*keys & 2`) or the press flag is set, and the belly test `FUN_0200B9A4` on the leader is nonzero: the leader's action index (`+0x46`) becomes 0 (run), and the run flag `dungeon+0x65C` becomes 1 when `info+0xAC` is not 2 (not confused). Red's names: `actionUseIndex` and `unk28`.
- Otherwise the action index becomes 1 (walk), and the run flag is not set.

The belly test is settled in Blue; see [Run flag and belly test](#run-flag-and-belly-test-181-p3). The exact conditions are in that section.

**Run frames** (`02140FAC`, `RunLeaderTurn_Async`). When `dungeon+0x65C` or `dungeon+0x663` (Red `unk2F`) is nonzero and the leader's display did not act, it calls `DungeonRunFrameActions(0xC)`. Tag blue-verified, confidence high for the checks. Red: `unk28 || unk2F`.

### Run continuation checks (#181 P1)

**[blue-verified]** for the order and the tests; the labels are ours. Source: static decompile of `ov_0001::02165F78` (`FUN_ov_0001__02165F78`, 167 lines) and `FUN_ov_0001__021664B0`. The check is called from the leader's input loop in `02166B10`, lines 116 to 124. Confidence: high for the order and the conditions, medium for the labels.

**Where it is decided.** At the top of the leader's input loop (`02166B10`), when the run flag `dungeon+0x65C` is set, the code calls `FUN_ov_0001__02165F78`. If it returns nonzero, the leader gets action `+0x42 = 2` (walk) with `+0x46 = 0`, and the function returns. If it returns 0, the code calls `FUN_ov_0001__021664B0`, which is StopRunning.

**StopRunning** (`FUN_ov_0001__021664B0`, `0x021664B0`). It sets `+0x65D |= +0x65C`, sets `+0x65C = 0`, and sets `+0x17C` and `+0x17E` to `0xFFFF` (clearing the touch or route target). Then it waits, one frame at a time (`FUN_0213AFA8(0x54)`), while the R key (mask `0x100`) is held and `+0x65D` is set. **[blue-verified]**

Other callers of StopRunning: `FUN_ov_0001__0215439C` (`0x02154528`), `FUN_ov_0001__02154910` (`0x02154AAC`), `FUN_ov_0001__021861E8` (DoEndOfTurnEffects, `0x02186640`, when the belly reaches 0), `FUN_ov_0001__02195104` (4 sites) and 4 more sites in `02166B10`. Those events also stop a run. Only the belly-empty one is classified here. **[blue-verified]**

**Stop conditions of `FUN_ov_0001__02165F78`.** A return of 0 stops the run. Setup: E is the leader (`FUN_0219C23C`); (x, y) is `E+4`, `E+6`; the direction is `info+0x44` (0 to 7; odd means diagonal, tested as `dir & 1`). The step offsets come from two direction tables (pool words `0x021664A8` and `0x021664AC`, stride 4). The tile lookup is `FUN_ov_0001__02149A08` (stride `0x14`: `+0x00` terrain word, `+0x05` room, `+0x06` walkable-neighbour mask, `+0x0C` monster, `+0x10` object). The entity type comes from `02142ADC`: 1 monster, 2 trap, 3 item.

In order:

1. Run flag `+0x65C` is zero: stop.
2. An object on the leader's own tile (`tile+0x10` nonzero; an item or a trap): stop.
3. `FUN_02165F58(E, x, y)` is nonzero: stop. It calls `FUN_021821E4`, which returns 1 for a position that is out of bounds, a wall (terrain `& 0x10`), a non-monster entity, or terrain that does not match the movement type from `FUN_0218267C(info+2)`. For the leader's own tile, this means standing on terrain that its movement type does not treat as plain.
4. `FUN_02181384(E, dir)` is zero: stop (cannot step ahead). It is false when the tile ahead has wall bit `0x10`, has a monster (`+0x0C`), or the own tile's walkable-neighbour mask (`+0x06`) lacks the bit for `dir` (table at pool `0x02181418`).
5. Room edge or doorway. `r` is the own tile's room (`+0x05`). If `r` is `0xFF` (corridor), the tile ahead must also be `0xFF`, or stop (entering a room). If `r` is not `0xFF`, own terrain bit `0x8` (`TERRAIN_NATURAL_JUNCTION`): stop.
6. Monsters and stairs ahead. For the three tiles `dir-1`, `dir` and `dir+1` (mod 8): a monster on one of them, or terrain bit `0x200` (stairs), stops.
7. Items. A 3 × 3 scan around the leader. An item (type 3) on a tile that is not one of the three behind the leader (`dir+3`, `dir+4`, `dir+5`) stops. The decompile drops the argument of `02142ADC`; the disassembly at `0x021661E4` to `0x02166200` shows that `r0` is the object pointer.
8. Traps. A visible trap (type 2; `trap+0x20` nonzero, or the dungeon byte at `+0x15FA7` nonzero) stops. For a cardinal heading (`dir` even) only the cross tiles, in the leader's row or column, count. For a diagonal heading every tile of the 3 × 3 counts.
9. Room change beside the path. For each cardinal neighbour (not the tile directly behind, and not blocked by the helper in check 3): when `r` is not `0xFF` and the neighbour's room differs from `r`, stop.
10. Side openings (cardinal heading only). For the three tiles on each side, perpendicular to the heading: if the middle one is open (`FUN_02165F58 == 0`) and either corner is blocked (nonzero), stop. This is the corridor junction test: a side branch or a doorway has an open middle and a blocked corner.
11. Otherwise return 1. A diagonal heading skips check 10.

So the junction checks are 5, 9 and 10; items are 7; monsters are 4 and 6, and traps 8; stairs are 6.

**Not verified:** the meaning of `+0x15FA7` and of `entity+0x20` (**[hypothesis]**); the movement-type helper `FUN_0218267C` and its return values; the contents of the direction tables; the Red equivalent (the check order is not compared with Red's `CheckLeaderTile` in this pass).

### Run flag and belly test (#181 P3)

**Run flag.** **[blue-verified]** for the behaviour; **[red-hypothesis]** for the name. Confidence: high for "run flag", medium for "= Red `unk28`".

- `dungeon+0x65C` is set to 1 at one site (`02166B10`, line 552). It is cleared only by StopRunning. It is read by `FUN_ov_0001__02165F78` (check 1), by `02166B10` at line 116, and by `RunLeaderTurn_Async` (`02140FAC`, together with `+0x663`).
- `+0x65D` is "was running": StopRunning ORs the run flag into it, and `02166B10` line 701 clears it.
- `+0x663` is set by `FUN_021418D4(1)` and cleared at line 72.
- The match to Red `unk28` (and `+0x663` to `unk2F`) rests on the offset-shift table in [`structs.md`](structs.md) and the table in this file, where `+0x65C` is Red `0x66C`. It does not rest on a new read.

**Run decision and belly test.** **[blue-verified]** for the conditions; medium for the labels. Lines 543 to 553 of `02166B10`:

- If `(keys & 2) == 0` and the touch-route flag is false, or `FUN_0200B9A4(info+0x10E) == 0`, the action index `+0x46` becomes 1 (walk), and the run flag stays 0.
- Otherwise, if `info+0xAC` is not 2 (not confused), the run flag `+0x65C` becomes 1. In both cases `+0x46` becomes 0 (run).

The same belly test gates the B+A case (lines 180 to 183: action 1, and `+0x663 = 1`) and lines 577 to 581 (B held, same test).

**The belly test (settles P5.1).** `FUN_0200B9A4` reads the belly at `info+0x10E`. The belly is two signed halfwords: the low one (`+0x10E`) is the integer part, and the high one (`+0x110`) is a fraction in thousandths. **[blue-verified]**

- The belly decrement, `FUN_0200BAB8` (called from `021861E8`, line 106), subtracts halfword by halfword. When the high halfword goes below 0, it adds one thousand to it and subtracts 1 from the low halfword. A negative low halfword becomes 0.
- `FUN_0200B9A4` returns `lo + (hi != 0)` when `lo != 0`, and `(hi != 0)` when `lo == 0`. That is a **ceiling**. So the run gate is "the belly is not exactly 0", and a belly of 0.xxx still runs.
- Starving uses the same test. `021861E8` (lines 123 to 130) treats `FUN_0200B9A4 == 0` as empty, which calls StopRunning and writes `+0x118`. The run gate and the starving check therefore agree.

Tag: **[blue-verified]** for the arithmetic and the shared use. **[red-hypothesis]** that Red's function is also a ceiling; Red's run check was not read. Confidence: high for Blue, low for the Red match. A port must use the ceiling, not a floor: with a floor, a belly with a zero integer part would count as empty.

### Swap flags (#181 P2)

- **`0x8000`, the petrified-ally swap.** **[blue-verified]**, high for the writers in the swept range. Only `FUN_ov_0001__02165C80` (`ov_0001::02165C80`) ORs `0x8000` into the `u16` at `info+0x00`, of the walker (the leader) and of the adjacent ally. Its only caller is `FUN_ov_0001__02166B10` at `02167C68` (the player move and run path). It also sets `gDungeon+0xE` to 1.
  - Preconditions in the decompile of `02165C80`: the adjacent tile holds an entity of type 1; the statuses at `info+0xA0` of both are not in {1, 2, 3, 4, 5, 7}; the byte at `+0xAC` is not 2 for either; the byte at `+0x99` is in {0, 2, 4}; `0215A23C(ally, 0)` is zero; `02181264(walker, dir)` is nonzero; and when the second parameter is nonzero and `021822DC(...)` is nonzero, a Yes/No prompt is shown. Both actions are set to walk (`021418AC(..., 2)`), and the ally's direction is `(walker dir + 4) & 7`.
  - **Cleared** by `FUN_ov_0001__02140ED0`, called from the turn loop `FUN_ov_0001__02140FAC` at `02141148`. If `gDungeon+0xE` is nonzero, `02140ED0` clears it. Then, for each of the 20 entity slots at `gDungeon+0x11364` that is valid and has `0x8000`, it runs `02185C70`, `02183BB4`, `02188030(e, 0)`, `0218453C`, `0219DA90` and `0206D7B0(0)`, and then sets `info &= 0x7FFF` and `info |= 0x4000`. `02140ED0` returns early, with `0x8000` still set, when `FUN_0214067C()` is nonzero. So `0x8000` marks a one-turn swap in progress, and `0x4000` follows it. **[blue-verified]**
  - **The AI path does not set it** (**[blue-verified]** in the swept range only). No other `|= 0x8000` on an entity word appears in the swept range. The hit in `0214A760` is a tile flag flood fill, not an entity. ARM9 and the other overlays were not swept, so this is not excluded there.
  - **Open:** the menu writer `ov_0001::02175178`, which [ai.md](ai.md) lists as clearing `0x8000` and setting it on option 0. Investigator B's scan found no store from `02175178` under its patterns, so that claim is not confirmed here.
- **`1 << 5`, the normal swap.** **[hypothesis]**: no Blue writer found. The scan covered the decompiled functions of `ov_0001` from `02138860` up to `021921B0`, about 1250 of 1710 (about 73%), searching for ORs and ANDs into the entity word. No `|= 0x20` is applied to `info+0x00`. Every `0x20` hit is a non-entity word (tile flags, move flags). Not excluded: writes in the unswept part of `ov_0001` (above `021921B0`), in ARM9 or other overlays, or in a form the decompiler folds differently. Red's source does not assign `MOVEMENT_FLAG_SWAPPING_PLACES` either, which is consistent. Confidence: medium. A write-watch trace of an ally swap on `info+0x00` bit 5 would settle it.

## Per-turn effects

`DoEndOfTurnEffects` (`021861E8`) returns `bool` in Blue, where Red's `DoEndOfTurnEffects_Async` returns `void`. The behaviour matches on the paths checked; see [`blue-vs-red.md`](../blue-vs-red.md). Tag blue-verified (callers), confidence medium. It is called per monster from `ExecuteEntityDungeonAction` (`0218453C`, call at `02184DF8`) when the entity is valid, `aiAllySkip` is clear and it is not the special case (Red `dungeon_action_execution.c:308-312`), and from the end-of-turn passes above.

**DisplayActions gate and order (#182 P4).** The label `DisplayActions_Async` (`ov_0001::0218766C`, call to `DoEndOfTurnEffects` at `02187B68`) is confirmed. Its structure matches Red's `dungeon_entity_movement.c:146`: the speed multiplier, a loop over the 20 monsters that tests `numMoveTiles` (`+0x1C0`) with `waiting` clear, the direction fix, the `0x2000` flag with the pixel position update, the 24-frame move loop, a cleanup pass, and then two passes (non-leaders, then leaders by `+0x7`). Inside the `0x2000` branch, in this order: for the leader, DiscoverMinimap (`FUN_02147DC4`), `FUN_0219635C(0)` and `FUN_02165B00`; for others, TryInteractWithTile (`FUN_02187420`). Then TryForcedLoss (`FUN_0206D7B0(0)`), `DoEndOfTurnEffects` (`021861E8`), EnemyEvolution (`FUN_02183BB4`), PotentiallyCreateMusicBox (`FUN_02143200`), TryTriggerMonsterHouseWithMsg (`FUN_02187200`), and then the position-changed flag with the tail (`FUN_021760B4`, `FUN_02181F30`, `FUN_021938C8`). The validity and floor-over checks between the calls match Red. The Blue call ignores the `bool` result of `DoEndOfTurnEffects`. Tag blue-verified for the structure and call order; red-hypothesis for the names; confidence high.

**Belly** (P6.1). Only the team leader (info `+0x7`) has its belly decreased, in the belly block of `DoEndOfTurnEffects`. The decrement is read from a table by index. The base index is 10, and held items adjust it: Tight Belt, Stamina Band, Energy Saver, Diet Ribbon, Heal Ribbon and Munch Belt. The index is clamped to the table length. Tag blue-verified for the gate (leader only); red-hypothesis for the arithmetic, which matches Red by inspection. Confidence medium. The decrement is a table lookup, scaled by 0.1 (s48.16). The table is at `0x021AA824`: 20 entries of 8 bytes, with the high word 0 and the low word a 16.16 value. The index is clamped to 0 to 19 (#182 P1). The table matches Red's `gBellyGoDownValues` **[red-hypothesis for the Red name]**. Its values are not recorded here (legal rule 3). The scale factor is a pool word at `0x02187158`. The linked-moves term adds an entry of a second table at `0x021AA434` to the decrement when the counter at `+0x125` is above 1, then clears the counter. Its values are not recorded either. Tag blue-verified for the table address, shape, indexing and scale; the Red match is red-hypothesis.

**Starving** (P6.2). When the belly reaches 0, the empty-belly alert counter at `dungeon+0x669` goes up, capped at 10. Messages are chosen on counter values 1, 2 and 3, as in Red's `EMPTY_BELLY_ALERT` steps. The entity's `bellyEmpty` (`+0x118`) is set to TRUE. Tag blue-verified for the counter.

**Empty-belly branch (#182 P1).** Once the belly rounds to 0 (`FUN_0200B9A4 == 0`), `DoEndOfTurnEffects` runs, in order: StopRunning (`FUN_021664B0`, Red `sub_805E804`), the camera (`FUN_02140E14`), DisplayActions (`FUN_0218766C(0)`), the validity and floor-over checks (`02142AE4`, `0214067C`), the alert counter `dungeon+0x669` (+1, capped at 10, messages on 1, 2 and 3), `FUN_0218F21C` (TrySendImmobilizeSleepEndMsg), the hunger damage, and last `bellyEmpty` (`+0x118`) = 1. The hunger damage is one call, `FUN_0217BCA4(entity, 1, 0xE, 0x211)` (DealDamageToEntity_Async): 1 HP per call, residual type `0xE` (Red `RESIDUAL_DAMAGE_HUNGER`, red-hypothesis) and exit reason `0x211` = 529 (Red `DUNGEON_EXIT_FAINTED_FROM_HUNGER`, red-hypothesis). When the belly is not 0, `+0x669` is reset to 0; after the damage, a nonzero belly drops the message, as in Red. Tag blue-verified for the call and its constants. Confidence high for the call; medium for the damage core, which was not followed into `HandleDealingDamage_Async`. Not verified: a runtime hunger tick.

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

Cadence: the per-turn caller is inside `sub_8044AB4` (`0214076C`), which runs only when `gSpeedTurns[1][fractionalTurn + 1] != 0`. So the counter moves 6 times per 24 fractional turns. Tag blue-verified, confidence medium. **Callers (#182 P2).** The resolver has 13 callers in all, as in Red: the per-turn caller `0214076C`, the 11 sites in the table below, and the ARM9 dungeon main loop `0206E658` (in `FUN_0206DC20`, `func_0x021953b4(1, 0)`, right after `02187200`; blue-verified; Red has two sites in its dungeon loop, and this is one of them). The table gives each site and its enclosing function. Blue-verified for the sites and the written offsets; the Red names are red-hypothesis.

| Blue call | Enclosing function | Role | Red |
|-----------|--------------------|------|-----|
| `0215DDE8` | `FUN_0215DDB8` | sets `gDungeon+0xC10C` (weather 5, hail) to the duration global, resolver `(1, 0)`, message if 0; called from the dispatcher `FUN_02156824` at `0215955C` | `HailMoveAction` |
| `0215DE5C` | `FUN_0215DE2C` | same with `+0xC10B` (weather 4, rain); from `02156824` at `02159310` | `RainDanceMoveAction` |
| `0215EF50` | `FUN_0215EF20` | same with `+0xC109` (weather 2, sandstorm); from `02156824` at `021587D8` | `SandstormMoveAction` |
| `021615A8` | `FUN_02161578` | same with `+0xC108` (weather 1, sunny); from `02156824` at `0215940C` | `SunnyDayMoveAction` |
| `02195C38` | `FUN_02195BFC` | floor setup: reads the floor weather byte (`+0x1A2FA`); if it is 8, a random value from `FUN_0219D058(8)`; then `FUN_02195B34(w)` and `FUN_02176B54` (artificial weather abilities), then resolver `(0, 1)`; called from the ARM9 loop (`func_0x02195bfc`) | `sub_807E5AC` (`weather.c:56`) |
| `021A279C`, `021A28E0`, `021A2BC0` | `FUN_021A273C`, `FUN_021A28A8`, `FUN_021A298C` | cutscene scripts; the first two set base weather `+0xC105 = 7` (snow) before `(0, 1)` | Articuno cutscene (`dungeon_cutscene_articuno.c:154, 189, 216`) |
| `021A55BC`, `021A5678`, `021A5758` | `FUN_021A5560`, `FUN_021A5640`, `FUN_021A56FC` | cutscene scripts; `+0xC105 = 4` (rain) before `(0, 1)` | Raikou cutscene (`dungeon_cutscene_raikou.c:106, 128, 152`) |

The weather source indices match Red **[red-hypothesis for the Red comparison]**: `0xC107 + i`, with the enum clear 0, sunny 1, sandstorm 2, cloudy 3, rain 4, hail 5, fog 6, snow 7. The four move handlers have no direct callers in x-ref except the dispatcher. No item or orb caller exists besides these four. Confidence high for the four move handlers and the floor setup; medium for the cutscene names. The third Articuno function (`021A298C`) shows no `+0xC105` write in the part decompiled.

**Wind** (P6.5), in `UpdateWindTurns` (`02195104`). It is called at `02141200` only when `RunLeaderTurn_Async` returns TRUE, and returns early when there is no leader. It decrements `windTurns` (`+0x656`) while it is above 0. Phase 0 starts below 250 turns, phase 1 below 150, and phase 2 below 50. The phase is stored in `windPhase` (`+0x66A`). Each phase logs a message, repositions the camera and sets the next phase. Tag blue-verified for phases 0 to 2; red-hypothesis otherwise. Confidence medium.

**Wind phase 3 and 4 (#182 P3).** The final branch of `UpdateWindTurns` runs when `windPhase` is 3 or 4. While `windTurns > 0` it returns. When `windTurns <= 0`, it runs StopRunning (`FUN_021664B0`), the camera `FUN_021766AC(leader, 1)` and DisplayActions `FUN_0218766C(leader)`. If the floor is not over, it shows the message from the table at `0x021B187C`, calls `FUN_0213EE9C(tilesetTable[+0x34AE], 3)` (Red `sub_80426C8`, table `0x021AAB58`), sets `windPhase = 4`, and then calls `FUN_021778F4(leader, 0x21E, leader)`. `0x21E` = 542 is Red's `DUNGEON_EXIT_BLOWN_OUT_UNSEEN_FORCE`, so the call is the faint handler `HandleFaint_Async`. Phases 0 to 2 use the thresholds `0xF9`, `0x95` and `0x31` (Red `< 0xFA`, `< 0x96`, `< 0x32`) and the tables at `0x021B18B8`, `0x021B18A4` and `0x021B1890`; the effect argument is the phase index. On entry, `windTurns` is decremented, and when it is below 1 `windPhase` is set to 3 before the dispatch, as in Red. Leaving `windPhase = 4` makes the else branch run again on later calls, as in Red. Tag blue-verified for the decompile and constants; [red-hypothesis] for the Red names and the Red comparison (`sub_80426C8`, `DUNGEON_EXIT_BLOWN_OUT_UNSEEN_FORCE`, the Red thresholds, "as in Red"); confidence high for the decompile. Not verified: the faint body (`FUN_021778F4` was not read to its end), the identity of `HandleFaint_Async` (medium), and the text at `0x021B187C`.

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
- **Speed counters.** Whether Blue applies the same stage-specific counter limit as Red was not verified. The Blue addresses of `BoostSpeed` and `LowerSpeed` were not located; `GetMovementSpeed` is `FUN_020023A8`.
- **P4.5, water and lava mobility.** Resolved for the mechanism (#184 P1, see [Mobility class source](#mobility-class-source-184-p1)): `GetCrossableTerrain` maps the species byte values 4 and 5 through the 8-byte table at `0x021B4834`, which dungeon setup writes from the tileset type. `DAT_0218269C` is the pool word for that address. Still open: the dungeon-setup caller chain of `0214969C`, and which tileset ids have type 1 (lava).
- **Edge masks.** The `x < 2` wall byte `0x1F` is written as a value, while the other three edge cases clear bits. Check the disassembly of `02148F50`.
- **IQ test and `+0xC7`.** The body of `021805B0` was not decompiled. The name of `+0xC7` as the invisible-class status is inferred from the value 3.
- Closed (#184 P3): the readers and writers of bit `0x4` are in the bit `0x4` paragraph under [Movement](#movement).
- **P5.1, belly test.** The ceiling reading is settled in Blue, see [Run flag and belly test](#run-flag-and-belly-test-181-p3). Still open: that Red's run check is also a ceiling (Red's run check was not read); that the argument `info+0x10E` matches Red's belly field; and the Red name of the action-index field `+0x46`.
- **P5.3, run-stop conditions.** The 11 ordered checks are recorded in [Run continuation checks](#run-continuation-checks-181-p1). Still open: the mapping to Red's `CheckLeaderTile` (`dungeon_main.c:928`), the contents of the direction tables, and the return values of `FUN_0218267C`.
- **P5.4 and P5.5, swap flags (#52).** In Blue, the petrified-ally flag `0x8000` has one writer in the swept range, `02165C80`, and is cleared by `02140ED0`; see [Swap flags](#swap-flags-181-p2). Still open: the AI path outside the swept range, and the menu writer `02175178`. The normal-swap flag `1 << 5` has no Blue writer found in the swept range (a hypothesis). Red's setter of `0x8000` is `sub_805EC4C` (`dungeon_main.c:860`), and Red's `src` never assigns `MOVEMENT_FLAG_SWAPPING_PLACES`.
- **P6.1, belly constant.** Resolved for the table address, shape, indexing and scale (see the Belly paragraph under [Per-turn effects](#per-turn-effects)); the table values are not recorded (legal rule 3).
- **P6.2, hunger damage.** Resolved: the Blue call is `FUN_0217BCA4(entity, 1, 0xE, 0x211)` (see the empty-belly paragraph under [Per-turn effects](#per-turn-effects)).
- **P6.4, callers of the weather resolver.** Resolved: 13 callers, classified in the table under Weather (#182 P2).
- **P6.5, wind phase 3.** Resolved: see the wind paragraph under [Per-turn effects](#per-turn-effects).
- **RNG in the status chain** of `TickStatusAndHealthRegen` (#51).
- **`DisplayActions_Async` label.** Resolved (#182 P4): the label is confirmed, and the call order matches Red (see the DisplayActions paragraph under [Per-turn effects](#per-turn-effects)).
- **`DoEndOfTurnEffects` return type.** Blue returns `bool` and Red returns `void`. Check whether the Blue callers use the return value.
