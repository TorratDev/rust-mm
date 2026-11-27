# Probe runtime environment: savestate library

**Date:** 2026-11-27
**Dump:** EUR retail (`NTR-APHP-EUR`), SHA-1 `6f01887356268de39085df01490347bbcc3aa330` ([supported-dumps.md](../supported-dumps.md))
**Issue:** [#648](https://github.com/TorratDev/rust-mm/issues/648) (follow-up of [#643](https://github.com/TorratDev/rust-mm/issues/643))
**Workflow:** [workflow.md](workflow.md)

Each claim below carries one status: **blue-verified**, **red-hypothesis** or **lead**. A claim with no evidence is not a fact.

## Claims

| Claim | Evidence | Status |
|-------|----------|--------|
| `make_state_library.py` rebuilds `dungeon1-f1.dst` from `RESCUE_ROM` and `RESCUE_PY` alone, writing only to the given states directory (scratch ROM copy, scratch `XDG_CONFIG_HOME`). | Trace: run 1 below; wrote the file in a scratch directory, nothing under the repo (`git status` clean). 3 min 16 s. | **blue-verified** |
| The rebuilt state is **not** byte-identical to the library copy: SHA-1 `8c629b05…` against the library's `a804b76a…`. The savestate header records its creation time, so a byte hash cannot check a rebuild. | Trace: run 1. The header line `Save created:` differs between the two files (21:08:53 against 07:36:46). | **blue-verified** for the mismatch; the header as its only cause is a **lead** (no byte diff was made). |
| The rebuilt state and the library state load to the same game state: `gDungeon` = `0x02113924`, team slots 0 and 1 species 1 and 1, `gDungeonRngState` (`021B3F58`) = `0x8E190E57`. | Trace: run 2, same output line for both files. | **blue-verified** for these three reads only; other RAM was not compared. |
| A `status_*.py` probe runs on the shared library state with no state argument: `status_probe.Probe` loads `~/rescue-re/saves/states/dungeon1-f1.dst` when it exists. | Trace: run 3, exit 0; `status_probe.py` (`RESCUE_STATE` default). | **blue-verified** |
| From that state, `status_rage.py` reads the literal at `0217EEF0` as pool pointer `021AD0E4` with value 0 (Attack) and sets `+0xAE` = 12, `+0xAF` = 2 on three wild monsters, all four offensive stages 10. | Trace: run 3. Agrees with the Enraged row of [status.md](status.md). | **blue-verified** |
| Run 3 (300 frames) ends before the first turn: no hit, no end-of-turn call. `combat_inputs.txt` runs to frame 4674 from this state; the first turn change is at frame 1378. | Trace: run 3 printed only the initial `chg` event; runs 4 and 5. | **blue-verified** |
| Re-check of the Enraged raise from the library state: when the leader hits enraged wild slot 0 (`0217E890` at frame 1529), `02189234` is called once with (attacker = leader, target = slot 0, stat 0, amount 1), returning to `0217E9E0` (the call at `0217E9DC`). Slot 0's Attack stage `+0x1C` goes 10 → 11 and no other stage changes. | Trace: run 5. Agrees with the Enraged raise in [status.md](status.md). | **blue-verified** |
| No raise when the enraged monster itself attacks: slot 0 hits the leader at frames 1440 and 1653, and `02189234` is not called. | Trace: run 5 (one `raise` in the whole run). | **blue-verified** |
| Re-check of the end-of-turn expiry: `+0xAF` drops by one per end-of-turn call per monster (2 → 1 at frame 1378 for all three). The call that reaches 0 also clears `+0xAE` (slot 0 at frame 1466, slot 1 at 1468, slot 2 at 1469). After the clear, the leader's hit on slot 0 at frame 1529 causes no raise. | Trace: run 4 (no `raise` in the whole run); run 5 shows the same one-per-turn decrement, 50 → 31 by frame 2265. Agrees with the Enraged expiry in [status.md](status.md). | **blue-verified** |
| The leader's second hit on slot 0 (frame 1709, slot 0 HP 3 → 0) caused no second raise. The hit took slot 0 to 0 HP, so the raise may be skipped for a fainted target. | Trace: run 5. The branch in `0217E890` that would skip it was not read. | **lead** |

## Trace checks

| Run | Script | Output hash (SHA-256, first 16) | Command | Claims checked |
|-----|--------|-------------------------------|---------|----------------|
| 1 | `verification/v7-trace/make_state_library.py` | state SHA-1 `8c629b05b067641a3ae6957ace70490d1a533448` | `RESCUE_ROM=pmdb.nds RESCUE_PY=~/venvs/desmume/bin/python make_state_library.py SCRATCH/states` | 1, 2 |
| 2 | scratch reader using `status_probe.Probe` (`RESCUE_STATE` set to each file) | `97bcd8d1cad5ba58` (the one summary line, identical for both) | `RESCUE_STATE=STATE python cmp_states.py verification/v7-trace STATE` | 3 |
| 3 | `verification/v7-trace/status_rage.py` | `f7f02458ab72e67a` (the probe's own lines, emulator banner removed) | `XDG_CONFIG_HOME=SCRATCH/xdg python status_rage.py SCRATCH/state-probe.nds SCRATCH/out 300 2` | 4, 5, 6 |
| 4 | `verification/v7-trace/status_rage.py`, AF = 2 | `82eb5bb1b3f4cbed` (probe lines only) | `XDG_CONFIG_HOME=SCRATCH/xdga python status_rage.py SCRATCH/rage-a.nds SCRATCH/oa 4700 2` | 6, 9 |
| 5 | `verification/v7-trace/status_rage.py`, AF = 50 | `6c6127341adbbfe1` (probe lines only) | `XDG_CONFIG_HOME=SCRATCH/xdgb python status_rage.py SCRATCH/rage-b.nds SCRATCH/ob 4700 50` | 6, 7, 8, 9, 10 |

Outputs stay in a scratch directory. The ROM copy for run 3 has a throwaway name, so DeSmuME's `.dsv` backup does not overwrite the user's save.

## Open questions

- `boot-town.dst` is not built by `make_state_library.py`: it needs a maintainer save and `ground_script_log.py` (commands in `~/rescue-re/saves/states/README`).
- Does `0217E890` skip the Enraged raise when the hit takes the target to 0 HP (run 5, frame 1709)?
- The Enraged message and `+0x11C` were still not watched at runtime.

## Links

- Status table: [status.md](status.md)
- Verification: [verification.md](../verification.md)
