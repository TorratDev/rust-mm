# Runtime batch A (#644): what one session ran, and what it did not reach

Tracker: [#644](https://github.com/TorratDev/rust-mm/issues/644). Environment: [#643](https://github.com/TorratDev/rust-mm/issues/643). This note records commands, savestate, and output hashes only. No ROM bytes, savestates, or traces are in the repo ([`workflow.md`](../workflow.md)). Outputs stay in a scratch directory.

## Setup

- ROM: the retail EUR dump `NTR-APHP-EUR` (crc `A28B5A7A`), copied to a throwaway name in scratch. `XDG_CONFIG_HOME` pointed at a scratch directory.
- Interpreter: `~/venvs/desmume/bin/python -I`, DeSmuME 0.9.12.
- Savestate: `dungeon1-f1.dst` (floor 1 of the first dungeon, made by `evolution_probe.py --savestate` after `boot_to_dungeon.txt` and 60 frames). SHA-1 of the state file: `a804b76ab9439e977090b41ca1f17a923b24760c`. The savestate library is outside the repo, at `~/rescue-re/saves/states/`. Its `README` is not written yet (see below).
- Runs were sequential. Two runs with the same ROM name and config directory clash over the `.dsv` backup, so one of them was stopped and rerun.

## Member results

| Issue | Probe | Result | Output SHA-1 | Status |
|---|---|---|---|---|
| #639 | `evolution_probe.py --force --force-returns --frames 480` from `dungeon1-f1.dst` | Ran. One `main` entry, 3 `cand`, 3 `apply`, 11 `tbl`, 1815 `rand`. Final entities: slot 0 species 159 (from 158), slot 1 species 5 (from 4), slots 2-4 species 291, 191, 191 at level 1 with `+0x11B` set. The forced apply ran for the hero, partner and one wild target. The town paths (`0200475C`, `0200468C`, `02004590`, `02047D74`) were not run. | `fc9adca98d28cdafdbf91c911febd10237ef1428` | Partly reached. Forced, emulator-verified for the floor pass. |
| #641 | `friend_areas_probe.py` from `dungeon1-f1.dst`, 1500 frames | Ran. Hooks `02003510` (2 hits) and `0206DC20` (1 hit) fired. The dungeon-end commit `02178068`, `0217F7EC`, and the other listed hooks were **not reached**, so this savestate never reached floor exit. Friend-flag count at the end: 15 nonzero (static count in the issue: 11). | `28643cdc3bc3d1adfe8d4449de99d4e40a85383e` | Not reached. The floor-exit savestate is still needed. |
| #612 | `natural_recruit_probe.py` from `dungeon1-f1.dst` | Refused: `recruitingEnabled` is 0 in dungeon 0 of this savestate, so no natural recruit can happen here. Negative control with `--allow-disabled`, 3000 frames: `0217F9B8` hit once at frame 626 (species 290, recruiter level 5), returned 0; rate total 0, no recruit. The probe hooks `0217F9B8`, not the `0217F388` named in the issue. | `210af4272b8f19fcdb990c46ea01b100bb09c662` | Negative result from a disabled-recruit floor. Not an unforced join. |
| #642 | `levelup_probe.py` from the boot route, 12000 frames, natural run | Ran. `levels_dump` output SHA-1 `30ffdbb32f2f5bf8c122ed6cc6958ece129b0b02`. One level-gaining call; every gain matched the parsed level record (`all_match: true`). One interactive move-learn call (`0x02182E5C`). The forget menu (`0216D77C`) and the non-team removal (`02182A94`) were **not reached**, so the menu-key cases and the `DungeonRandInt` removal draw are open. | `2b1f265308a3d9b39a2b95a5328184a115340c38` | Partly reached. Gains verified; forget menu and removal open. |
| #624 | `status_effects.py` (12 samples), `status_leech_seed.py` (3000 frames), `status_rage.py` (1500 frames, AF = 1) | All three ran to completion (rc 0). Speed-down setter: `+0xE0` equals the up-counter count in 12 of 12 samples. Leech Seed: drain (5 hits) and heal (5) events recorded at end of turn. Enraged: stage-change events and end-of-turn expiry recorded; the hit at frame 1441 fired. Outputs are in scratch. | effects `eba40e4ad5edf32905cb2572e8928d7d7e500666`, leech `b153a100665916096ceb58164870c363f037cd66`, rage `5e3f6766196dfe423bc5d8f99db6061c2619a6f8` | Ran. Rows in `docs/re/status.md` not yet updated with these results. |

## Static finding for #641 (read-only, from the export `ram/0206dc20_FUN_0206dc20.c`)

- `0206DC20` is the whole dungeon-run function, not an exit routine. Its `param_1` is the run struct. It does setup first (`+0x58`, `+0x5C`, the floor tables), then play, and writes the result code at `param_1+0x60` on each branch (`0xFFFF`, `0xFFFE`, and `1`, `2`, `3`, `4` at lines 465 to 571 of the export).
- The exit chain sits at the tail: `LAB_0206eca0` reads `+0x60` (lines 588 and 592), calls `02178068(1)` for codes 1, 2, 4, `02178068(0)` for -2, and `02177FD8` then `02178068(0)` for -1. The exit hooks `0206ED24`, `0206ED3C`, `0206ED4C` are inside this function's span.
- So a forced exit cannot be a plain call with a chosen `+0x60`. It would need `+0x60` set and a jump into `LAB_0206eca0` with the function's stack frame and saved registers intact. That is an unverified emulator hack, not a clean run, and it has not been tried.
- The run that reached `0206DC20` at frame 24952 (boot) was this function's entry, which is dungeon setup. It is not evidence of an exit.

## Forced floor exit (#641), attempt 1: failed

- Probe: `friend_areas_probe.py --force-exit 1 --frames 600` (FORCED). It captures `0206DC20`'s entry `r0` and `sp`, writes result code 1 at the run struct's `+0x60`, sets `r5` = struct, `sp` = entry sp - 0x20 (the prologue frame), and resumes at `0206ECA0`, the tail label.
- Output: the resume ran, but no exit hook fired (`0206ED24`, `02178068`, and the other exit hooks are all in `not_reached`). The emulator then stopped with `ARM9: Undefined instruction: 0x264309B0 PC=0x0211128C`.
- Reading: a mid-function resume is not safe with only `r5` and `sp` restored. The tail reads more than `r5`, and the stack frame or the saved registers are not in the state the epilogue expects. Output SHA-1 of the partial JSON: `a4f4e1e50f7dfb1cdb7e4b66b6aa6aeb6af3d388` (its `summary.forced` is set).
- Status: #641 is still not reached. A floor exit needs either a natural floor end (a walk to the stairs, not reached yet) or a correct full frame restore (not attempted).

## Forced floor exit (#641), attempt 2: failed

- Same forced resume at `0206ECA0` with result code -2 (`--force-exit -2 --frames 600`, FORCED). The code -2 path skips the `02142B70` call, which lives in overlay `ov_0001`, so the overlay hypothesis for attempt 1 was not the cause.
- Output: the emulator stopped again at `ARM9: Undefined instruction: 0x264309B0 PC=0x0211128C`, and no exit hook fired.
- Reading: the crash is in the resume itself (the restored `r5` and `sp`, or other registers the tail depends on), not in the exit code. Full frame restore or a natural floor exit is still needed for #641.

## Forced four-move state (#642): forget menu not reached

- Probe: `levelup_probe.py --force-four-moves --frames 12000` (FORCED). It writes four valid move records (ids in bytes 0 to 5 of each slot) onto the hero (species 158, info `0x2113FB0`) and partner (species 4, info `0x2114184`) after boot, then plays `combat_inputs.txt` naturally.
- Output: one level-gaining call and one learn call on entity `0x2124F4C`, with `pre_valid` 5. That entity is not one of the forced records, so the learn did not run on a full four-move team member. Forget menu calls: 0. Removal calls: 0. Gains matched the levels dump (`all_match: true`). Output SHA-1 is not recorded here.
- Status: #642 forget menu and non-team removal are still not reached. Next step is to force the level-up on the forced hero itself, or to identify entity `0x2124F4C` from the forced record list.

## Forced pending EXP, forget menu reached (#642)

- Probe: `levelup_probe.py --force-four-moves --force-pending-exp 200000` (FORCED). The hero (species 158, entity `0x2124ED8`) starts at level 5 with pending EXP 200000 written at `info+0x1C8`, and four valid moves in slots 0 to 3. Turn inputs are the natural `combat_inputs.txt`.
- Output (3000 frames and 8000 frames): one EXP pass, two learn calls on the hero (`pre_valid` 4), and one forget-menu call (`0216D77C`, r2 = 1, r3 = 0). The forget menu's pre-state shows the hero's move list at entry.
- The menu never returned: its return value is `null` in both runs. The probe presses A every 120 frames and stops after 3000 menu frames, so the menu stayed open. The cases for choose, cancel and new move are not tested. Non-team removal (`02182A94`) was not reached.
- Status: forget menu entry is reached under this forced state; its result and the removal are still open. Output JSON SHA-1 not recorded here.

## Escape Orb exit, result code 2 (#641), forced item

- Probe: `escape_orb_probe.py --boot` (FORCED). The team bag is empty and the first floor has no Escape Orb (the floor-item bytes are `01 07 69`, `01 04 69`, `01 06 69`, none of them item 200). The probe writes item 200 into bag slot 0 (`01 01 C8`) and sets the bag gate `gDungeon+0x64C` to 1, tagged forced. The bag menu is opened with the `items_bag_probe.py` keys (`W X DOWN A A A`).
- Output: `EscapeOrbAction` (`021623A4`, move 181) fires at frame 784. The slot clears after the use. `0206ED24` and `02178068(1)` fire at frame 1033, and the result code read at the run struct's `+0x60` is **2**. The struct is `0206DC20`'s entry `r0` (`0x027E3A24`), which the tail also holds in `r5`.
- Reading: this matches the Red mapping. An escape sets the escaped reason (`unk11 = 1`), which is not a cleared run, so it gives code 2 ([`reference/pmd-red/src/run_dungeon.c`](../../reference/pmd-red/src/run_dungeon.c), lines 520 to 660). Blue's item 200 as the Escape Orb is confirmed by the move hook only, not by the item table.
- Not reached: `0206ED3C` (code -2), `0206ED4C` (code -1), `02177FD8`, and `0206DC20` itself (its entry is seen only on the boot route, and the savestate route starts after it). Codes 1 and 4 are not tested.
- Caveat: `friend_areas_probe.py` reads the result code at `gDungeon+0x60`. On this run that value was `0`, while the run struct gives `2`. So the friend probe's code column is not the result code. This is recorded, not fixed here.
- Output JSON: `escape-orb-boot.json` in scratch, SHA-1 `ebad7d3439e51636c22422625c5acafa7d185cbb`.

## Code -1 by the dungeon wind, not a defeat (#641)

- Probe: `defeat_probe.py` (natural, nothing forced). It boots the normal route, presses A every 30 frames, and logs the leader's HP every 300 frames.
- Output: the leader went from 30 HP to level 6 (34 max HP) through natural wild KOs, and was still alive (33 HP) at frame 40500. The run ended at frame 40530 with `02177FD8` (r0 = -2), `0206ED4C`, and `02178068(0)`, and the result code read at the run struct's `+0x60` was **-1**.
- **Correction:** this was not a leader defeat. The run ended with the wind's blow-out (`DUNGEON_EXIT_BLOWN_OUT_UNSEEN_FORCE`, reason `0x21E`). Red's `dungeon_wind.c` raises that faint when the wind countdown reaches zero (`windTurns < 1` in `windPhase` 3, lines 77 to 81). The leader's faint reason was `0x21E` in every later run, which is the same wind exit. So the code -1 here is the wind exit, and a natural leader defeat has not been observed.
- Not reached: codes 1, 4, and -2 (`0206ED3C`). Code 2 was reached above with the Escape Orb.
- Output JSON in scratch: `defeat.json`.

## Forced distress count, abandon attempt (#641, code -2): not reached

- Probe: `defeat_probe.py --force-rescues 3` (FORCED). It writes 3 at `gDungeon+0x66B` after boot. That offset is Blue `unk644.unk37`, inferred from Red's struct layout and not checked on the ROM. Red offers the distress-call prompt on a leader faint only when this count is above 0, and the yes answer sets the abandon flag (`unk6`), so the exit is code -2.
- Output: 28 faint calls (`021778F4`), the leader's last at frame 40334 with reason `0x21E`. The exit at frame 40530 was code **-1** (`02177FD8` with r0 = -2, `0206ED4C`, `02178068(0)`), the same as the natural defeat. No prompt was seen: the distress prompt's function was not hooked, and Red's exit-reason exclusion list does not map cleanly onto the Blue reason codes (the Blue codes such as `0x163` and `0x21E` are not Red's enum values).
- Status: code -2 is not reached. Next, check that `+0x66B` holds 3 at the faint (the write may not persist or the offset is wrong), and hook the yes/no prompt to see whether it is offered.
- Follow-up (checked on the Blue export, `021778F4_HandleFaint_Async.c`): the distress count is `gDungeon+0x66B` (read directly by the faint handler), so the forced offset is confirmed. The prompt (`AskYesNo`, `0x02153E80`) is skipped when the faint reason is `0x21C` or one of two other excluded values, and when `missionKind` is 1. Reason `0x21E` (blow-out) is one of the excluded values.
- Two more runs with A every 25 and 45 frames (forced count 3, `unk37` read as 3 at every faint): the leader faints with reason `0x21E` in both, the prompt is not offered (`AskYesNo` not hit), and the exit is code -1 again (frames 40650 and 56835). So with these inputs the leader's natural faint on this floor is the excluded blow-out, and the distress path is not reached. Code -2 needs a leader faint with a reason outside the exclusions.

## Code -2 by the distress prompt (#641), forced HP and forced count

- Probe: `defeat_probe.py --force-hp 1 --force-rescues 3 --a-every 30` (FORCED twice: the leader's HP is set to 1 at info `+0xE` and the distress count to 3 at `gDungeon+0x66B`, both after boot). The rest is the normal run.
- Output: the leader's faint at frame 432 with reason `0x9A` (a normal KO, not one of the excluded values), `unk37` = 3. `AskYesNo` ran at frame 512 (message `35372012`). The exit `0206ED3C` fired at frame 710 with the result code **-2** read from the run struct, then `02178068(0)`.
- Reading: the prompt answer (A, default) set the abandon flag, so the exit is -2, as in Red's `run_dungeon.c` and the Blue tail. The answer was not read directly from `AskYesNo`'s return, so "yes" is inferred from the -2 code.
- Forced inputs: the 1 HP and the count 3 are not natural. A natural run reaches code -2 only if the leader is KO'd by a monster hit with a non-excluded reason and the count is above 0. Not checked on a savestate with a natural count.
- Output JSON in scratch: `ko-attack.json`.

## Open

- A savestate at a dungeon floor exit (for #641) and one with a learnable four-move team member (for #642 forget menu). Neither was made.
- `~/rescue-re/saves/states/README` (required by #643) is not written yet.
- `verification/v7-trace/levels_dump/Cargo.lock` is created by the build and is not part of this change.
