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
| #624 | `status_effects.py`, `status_leech_seed.py`, `status_rage.py` | Running when this note was written. Results to follow in a later commit. | — | Pending. |

## Open

- A savestate at a dungeon floor exit (for #641) and one with a learnable four-move team member (for #642 forget menu). Neither was made.
- `~/rescue-re/saves/states/README` (required by #643) is not written yet.
- `verification/v7-trace/levels_dump/Cargo.lock` is created by the build and is not part of this change.
