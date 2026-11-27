# Story flags, event variables and progression gates

**Date:** 2026-11-26
**Dump:** Pokémon Mystery Dungeon – Rescue Team Blue (EUR), `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Static evidence comes from the ghidra-cli text export of the `rescue-arm9` project (STAMP 2026-11-14, 5284 functions, 0 failed). Variable names are Blue's own debug strings, read at runtime from the descriptors' name pointers.
**Issue:** [#525](https://github.com/TorratDev/rust-mm/issues/525) (batch [#645](https://github.com/TorratDev/rust-mm/issues/645), parent [#76](https://github.com/TorratDev/rust-mm/issues/76))
**Workflow:** [workflow.md](workflow.md)

The variable storage itself (descriptor tables, getters, setters, what is saved) is in [script-opcodes.md](script-opcodes.md#p3--variable-model). This note covers the story variables, what writes them, what reads them, and the post-dungeon result. No decompiler output, ROM bytes or table values are in this note.

## Status by point

| Point | Status | What is open |
|-------|--------|--------------|
| P1 story variables, scenario record | **blue-verified** | Fields `+0`, `+6`, `+8`, `+0x10` of the dungeon record |
| P2 writes from a new game to the first clear and back | **blue-verified** (runtime, one path) | Later stages; opcode `0xAA`; when `FUN_02020020` fires |
| P3 gates | **blue-verified** for the check table, the dungeon setup flags and the bag gate | Which UI screen each other caller is |
| P4 writers of the post-dungeon result | **blue-verified** for every store; value 1 at runtime | Values 2, 4, −2, 5 at runtime; what sets dungeon `+3`, `+6`, `+0x11` |

## P1 — Story variables

| Claim | Evidence | Status |
|-------|----------|--------|
| The "group" of `FUN_02020518(group, stage, sub)` is a variable id. Ids 3 to `0xD` are SCENARIO_SELECT (3), SCENARIO_MAIN (4) and SCENARIO_SUB1–SUB9 (5–`0xD`). Each is two u8 elements: element 0 the stage, element 1 the sub-step. Stage `0x3A` is the "unset" value the checkers log on. So "group 4 past stage 2" means SCENARIO_MAIN stage > 2. | Descriptor table A, runtime names | **blue-verified** |
| Checkers, all ARM9: `FUN_02020518(g, s, u)` "past": stage > s, or stage == s and u ≥ 0 and sub > u. `FUN_02020614` "before": stage < s, or stage == s and u ≥ 0 and sub < u. `FUN_020205B0` "exactly": stage == s and (u < 0 or sub == u). `FUN_02020848` reads both. | Functions named | **blue-verified** |
| `FUN_020206AC(g, stage, sub)` is SetScenario: it writes both elements, clears CLEAR_COUNT (var `0x1A`) when SCENARIO_MAIN changes, and calls `FUN_02010DFC(n)` at a few thresholds of groups 4, 5 and `0xD`. Script opcode `0xA9` calls it; `0xAA` advances the sub-step after asserting the stage. | `FUN_020206AC`; [script-opcodes.md](script-opcodes.md) | **blue-verified** |
| Other story-related ids: `0x0E` GROUND_ENTER, `0x0F` GROUND_ENTER_LINK, `0x10` GROUND_GETOUT, `0x11` GROUND_MAP, `0x12` GROUND_PLACE, `0x13` DUNGEON_SELECT, `0x14` DUNGEON_ENTER, `0x15` DUNGEON_ENTER_INDEX, `0x16` DUNGEON_ENTER_FREQUENCY, `0x17` DUNGEON_RESULT, `0x19` START_MODE, `0x1A` CLEAR_COUNT, `0x2D`–`0x2F` RESCUE_SCENARIO_ORDER/JOB/CONQUEST_LIST (64-bit lists), `0x32` DUNGEON_ENTER_LIST and `0x33` DUNGEON_CLEAR_LIST (96 bits each), `0x3B` EVENT_LOCAL, `0x43` EVENT_S08E01 (two s16). | Runtime read of the descriptor names | **blue-verified** |
| All of them are in the 0x400-byte block that the save copies raw, so the story state is saved as a whole. Money and savings are type 8 and live in the team struct. | [script-opcodes.md](script-opcodes.md#p3--variable-model), [../formats/save.md](../formats/save.md#story-flags-and-scene-variables-362-p5) | **blue-verified** |
| `FUN_02070890(id)` is a dungeon record table, not a scene-variable record: base `0x020B9DD8`, 0x53 records of 0x12 bytes. It is indexed by DUNGEON_ENTER (`0x14`), or by DUNGEON_ENTER_INDEX (`0x15`) when `0x14` is `0x51`. Fields read by code: `+2` the ground map after a clear, `+4` the ground map for other results, `+0xA` the loader-script index for scene-runner ids 9–`0xC`, `+0xC` byte area number, `+0xE` s16 area id used with the area lists (`FUN_02070664`), `+0x11` a byte flag. | `FUN_02070890`, `FUN_0206F1C0`, `FUN_0206F7EC`, `FUN_02070664` | **blue-verified** (static); other fields unread |

## P2 — What advances them, new game to the first clear and back

Runtime: the probe [`ground_script_log.py`](../../verification/v7-trace/ground_script_log.py) (run A in [Trace checks](#trace-checks)) and an earlier investigator run with the same hooks plus a diff of the block each frame, which repeated the dungeon part four times with identical frames. Frame numbers are from power-on with `boot_to_dungeon.txt`.

| Step | Frame (about) | Write | Writer | Status |
|------|---------------|-------|--------|--------|
| 1 | 2459, 2562 | INITALL: the block is reset to its defaults (`FUN_02021310`) | ARM9 | **blue-verified** |
| 2 | 2575–2579 | Scene runner(0); START_MODE = 1; GROUND_ENTER; then SCENARIO_SELECT = (0, 0) and SCENARIO_MAIN = (1, 0) | Runner `0206F7EC`; script `0xA9` (caller `ov_0000::021453F4`) | **blue-verified** |
| 3 | 10743 | BASE_KIND, PARTNER_TALK_KIND | Ground loop code (`0x0201E120`, `0x0201E160`) | **blue-verified** |
| 4 | 10775 | SCENARIO_MAIN = (2, 0), then (2, 1); GROUND_* and partner kinds through `0xA6`; DUNGEON_ENTER = −1, DUNGEON_RESULT = 0, PLAYER_KIND | Script `0xA9`, `0xA6` and `0xAB`/`0xAC` | **blue-verified** |
| 5 | 24907 | RESCUE_SCENARIO_ORDER_LIST[0], DUNGEON_SELECT, RESCUE_SCENARIO_JOB_LIST[0]: area 0 unlocked | `FUN_0200FED0(0, 1)` from script `0xAF` | **blue-verified** |
| 6 | 24939 | Dungeon entry: DUNGEON_ENTER = 0 (the dungeon), DUNGEON_ENTER_INDEX, DUNGEON_ENTER_LIST[0] = 1, START_MODE, DUNGEON_RESULT | Runner literal region (`0x020700D0`..`0x02070118`) | **blue-verified** |
| 7 | 25010–29212 | Three floors; no variable writes inside the dungeon | – | **blue-verified** |
| 8 | 32383–32387 | End of the dungeon (result 1, see P4); EVENT_S08E01[0] = 1 | Post-dungeon switch of `RunGameMode_Async` (`0x0201EB30`) | **blue-verified** |
| 9 | 32389–32390 | Runner case 9: DUNGEON_RESULT = 9, WARP_LOCK = 0, DUNGEON_ENTER_FREQUENCY, DUNGEON_CLEAR_LIST[dungeon] = 1 (only for runner id 9), START_MODE, GROUND_GETOUT and GROUND_ENTER from the dungeon record `+2` | Runner `0206F7EC` | **blue-verified** |
| 10 | 32391–35402 | Loader script from the record's `+0xA`: SCENARIO_MAIN = (2, 2); banks load; GROUND_MAP = 9 (back in town) | Script `0xA9`; opcodes `0x1E`, `0x08` | **blue-verified** |

So from a new game to the end of the first dungeon only SCENARIO_MAIN changes among the scenario variables: (0, 0) → (1, 0) → (2, 0) → (2, 1) → (2, 2) after the clear, every time through opcode `0xA9`.

Run A of the committed probe repeats steps 1–6 with the same frames: INITALL at 52, 61, 2459 and 2562; SCENARIO_MAIN = (1, 0) at 2579 and (2, 0), (2, 1) at 10775, all from the `0xA9` handler (return address `ov_0000::021453F8`). Run A stops at the first floor (frame 25020: the input does not clear it), so steps 7–10 rest on the investigator run.

| Claim | Evidence | Status |
|-------|----------|--------|
| Script opcodes that write story state: `0xA4`–`0xA8` (generic variable writes), `0xA9` SetScenario, `0xAA` advance, `0xAB`/`0xAC` (DUNGEON_ENTER, DUNGEON_RESULT, var `0x1D`), `0xAD`–`0xB1` (other save tables, area lists), `0xB2`. Native writers: the scene runner (START_MODE, GROUND_*, DUNGEON_*, the clear list), `RunGameMode_Async` (EVENT_S08E01, START_MODE) and `FUN_02020020`. | [script-opcodes.md](script-opcodes.md) | **blue-verified** (static) |
| `FUN_02020020`, called every frame from runner states 2 and 3, advances the sub-scenarios and area lists when SCENARIO_MAIN is in a middle range and some flags hold. It did not fire in the span above. | `FUN_02020020` (static read only) | **lead** for when it fires |

## P3 — Gates

| Claim | Evidence | Status |
|-------|----------|--------|
| `FUN_0201F994(id)` (CheckStoryFlag) is a 30-case switch of scenario checks. Ids 0–20 test SCENARIO_MAIN (for example id 0 "stage > 2", id 6 "stage > 0x11", id 9 "stage > 0x12"); ids 21–26 test SUB2, SUB3, SUB4, SUB5 and SUB7; ids 27–29 call other checks (`FUN_0200FE6C(0x29)`, `FUN_0200881C() == 6` the top rescue rank, `FUN_02010158`). Script opcode `0xBB` reads the same switch. It has 90 call sites in 47 functions. | `FUN_0201F994`, export x-refs | **blue-verified** (static) |
| Dungeon setup flags: `FUN_0201DEF4` (from `RunGameMode_Async`) fills setup bytes `+7` (id 8), `+8` (id 3), `+9` (ids `0x18`–`0x1A`), `+0xA` (id 7), `+0xB` (id 0), `+0xC` (id 5). `RunDungeon_Async` copies them to dungeon `+0x64D`, `+0x649`, `+0x64A`, `+0x64B`, `+0x64C`, `+0x64E`. The bag gate `+0x64C` ([items.md](items.md#pickup-gate-and-0x64c-464)) is id 0, SCENARIO_MAIN stage > 2, so the bag is off in the first dungeon, where SCENARIO_MAIN is (2, 1). | `FUN_0201DEF4`, `RunDungeon_Async`; runtime: all six flags read 0 at the first floor | **blue-verified** |
| World map: `FUN_020703B8` builds the selectable places (when id 1 holds: each area below `0x2E` whose record has `+0x11` set and whose conquest bit, var `0x2F`, is set). `FUN_020704FC` and `FUN_020705F4` test ids 5 and 1 with the job list (var `0x2E`). `FUN_02070664(dungeon)` returns a dungeon's kind 1–4. Area unlock and lock writers: `FUN_0200FED0`, `FUN_0200FFD4`, `FUN_0200FDAC`, `FUN_02020020`. | Functions named | **blue-verified** (static) |
| Menus and UI: id 0 is read by `FUN_02008758`, `FUN_02051BD4`, `FUN_02067AE0` (top-screen menu) and the message token `0x74` in `FUN_0202DF1C`; `FUN_02067A1C` takes 12 ids from a list to choose which icons show; id 6 gates the friend-area list (`FUN_0214D208`, `FUN_0206A6DC`, …); ids 8, 9, `0xB`–`0xF` have their own caller groups. Which screen each caller draws is not mapped. | Export x-refs | callers **blue-verified**; screen names **lead** |

**How to settle the screen names:** break on `FUN_0201F994` while each screen opens and record the caller per screen.

## P4 — Writers of the post-dungeon result `setup+0x60`

All stores are in `RunDungeon_Async` (`0206DC20`) except value 5. The end reason is a halfword at dungeon `+0x1AC30`, written by `ov_0001::0219C898(reason, …)`, which also fills the end-of-run record at `+0x1AC1C`. `ov_0001::0219C868` tests "reason < `0x226`" (a defeat) and `ov_0001::0219C81C` tests "reason is `0x227`, `0x228` or `0x22A`".

| Value | Store | Condition | Status |
|-------|-------|-----------|--------|
| 3 | `0x0206E904` | dungeon `+3` ≠ 0 after the main loop (quicksave and quit); also sets setup `+0x63`/`+0x64` (floor, room) | **blue-verified** (static); runtime in [execution-model.md](execution-model.md) #267 P4 |
| −1 | `0x0206EB8C` | defeat and dungeon `+6` == 0 | **blue-verified** (static); runtime in #267 P4 |
| −2 | `0x0206EBA4` | defeat and dungeon `+6` ≠ 0 (rescue variant; also copies floor, room and the `+0x66C` seed to setup `+0x68..+0x6C`) | **blue-verified** (static) |
| 4 | `0x0206EC6C` | no defeat, reason in {`0x227`, `0x228`, `0x22A`}, dungeon `+0x668` == 1 (mission kind "rescue by mail"); setup `+0x62` = dungeon `+0x664` | **blue-verified** (static) |
| 1 | `0x0206EC78` | the same with `+0x668` ≠ 1: a normal clear | **blue-verified** (static and runtime) |
| 2 | `0x0206EC98` | no defeat, other reasons (`0x226`, `0x229`, above `0x22A`) | **blue-verified** (static) |
| 5 | `FUN_0201E204` | no quicksave to load (`FUN_0202AFC0() == 0`) | **blue-verified** (static) |

`RunDungeon_Async` passes reason `0x226` when dungeon `+0x11` == 1, `0x229` for 2, `0x22A` for 3, `0x227` for 4, `0x228` for a rescue-type floor end, and `0x227` at the floor-limit exit (`0x0206EACC`). Defeat reasons (below `0x226`) come from `ov_0001::02171750` and `ov_0001::021778F4`. Runtime: the first dungeon ended through the floor-limit exit with reason `0x227` and result 1 (dungeon `+0x668` = 0, `+6` = 0), the same in four runs. **blue-verified**. The meanings of dungeon `+0x11` values 1–4 and of `+6` are **lead**.

## Trace checks

| Script | Output hash | Command | Claims checked |
|--------|-------------|---------|----------------|
| `ground_script_log.py` (run A) | see [script-opcodes.md](script-opcodes.md#trace-checks) | see there | P2 order of writes |
| Investigator probes for #525 (not committed) | log SHA-256 prefixes `e8551690ea182521` (boot run), `560270757163f7b0` (dungeon auto-walk), `0239f6479dd26ed2` (end reason) | py-desmume exec hooks on the setters, the runner, `0214D498`, `0214D420`, `0201E3C8`, `0219C898` and a per-frame block diff | P2, P4 value 1 |

## Links

- [script-opcodes.md](script-opcodes.md) (variable model, opcodes `0xA4`–`0xBF`)
- [scripts.md](scripts.md#p5--story-flags-and-scene-variables) (scene variables)
- [execution-model.md](execution-model.md) (scene runner, post-dungeon switch)
- [items.md](items.md) (bag gate), [../formats/save.md](../formats/save.md)
