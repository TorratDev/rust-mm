# Ground mode: movement, collision, interaction, exits and camera

**Date:** 2026-11-26
**Dump:** Pokémon Mystery Dungeon – Rescue Team Blue (EUR), `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Static evidence comes from the ghidra-cli text export of the `rescue-arm9` project (STAMP 2026-11-14, 5284 functions, 0 failed).
**Issue:** [#524](https://github.com/TorratDev/rust-mm/issues/524) (batch [#645](https://github.com/TorratDev/rust-mm/issues/645), parent [#76](https://github.com/TorratDev/rust-mm/issues/76))
**Workflow:** [workflow.md](workflow.md)

All code here is in overlay 0 (`ov_0000::`) unless marked ARM9. Names are Ghidra `FUN_` names; the layout resembles `pmd-red`'s ground lives, object, event and camera code, but Red was not read for this note. The script side (the installer, opcodes and tokens) is in [scripts.md](scripts.md) and [script-opcodes.md](script-opcodes.md). No decompiler output, ROM bytes or table values are in this note.

## Status by point

| Point | Status | What is open |
|-------|--------|--------------|
| P1 player movement | **blue-verified** (static and runtime) | Touch control; action `0x13` (B tap, X) |
| P2 collision | **blue-verified** for the grid bit `0x80` and walls; actor blocking static | Which BMA layer fills bits `0x80` and `0x40`; actor-against-actor at runtime |
| P3 interaction and exits | **blue-verified** for object talk, step-on events and an exit, with the exit's commands | Talk to a lives NPC at runtime; how the transition request fills GROUND_ENTER |
| P4 camera | **blue-verified** for follow; bounds static | The clamp at a map corner |
| P5 runtime check | Done in the Team Base and outside it (map 9), from a save | The first town proper; a lives NPC |

## Data model

| Claim | Evidence | Status |
|-------|----------|--------|
| Three actor tables in overlay 0's BSS, each a pointer to a heap block. Lives: pointer cell `0x0215D1DC`, 24 entries of 0x1F0 bytes, id s16 at +2 (−1 = free), flags u32 at +0x11C, box (x1, y1, x2, y2) at +0x144..+0x150. Objects: `0x0215D1F8`, 16 entries of 0x1C4, id at +6, flags at +0x11C, box at +0x124..+0x130. Events: `0x0215D240`, 32 entries of 0x20, id at +2, flags at +8, box at +0xC..+0x18. | Loops in `FUN_021505D0`, `FUN_0215358C`, `FUN_02155F44`, `FUN_02157DC0`; runtime read of the three cells | **blue-verified** |
| Box units are 1/256 pixel; a map cell is 8 px. The player is lives 0, id 1. Per-lives fields: facing byte +0x142 (0 down, 1 down-right, 2 right, 3 up-right, 4 up, 5 up-left, 6 left, 7 down-left, −1 none; the dungeon codes), animation id +0x168, controller state +0x120, last event +0x13E, map-limit rect +0x28..+0x34. | Box reads divide by 0x100; runtime in the Team Base | **blue-verified** |
| Lives flag bits: `0x1` collide with grid bit `0x80`; `0x2` with grid bit `0x40`; `0x4` with lives; `0x8` with objects; `0x10` with events (and step-on); on a target, `0x40` bump runs its script and `0x80` makes it an A-talk target; `0x200` the target turns to face the mover; `0x800` player-controlled; `0x1000` another control mode (`FUN_0214FFC0`); `0x2000` touch pen down; `0x4000` a shadow-like extra (`FUN_02150D4C`). | Code that tests each bit | **blue-verified** for `0x1`–`0x800`; `0x1000` and `0x4000` **lead** |

## P1 — Player movement

| Claim | Evidence | Status |
|-------|----------|--------|
| Each frame `FUN_021505D0` (called from the scene runner region) loops the 24 lives and, for each with flag `0x800`, calls the controller `FUN_0213F7D8(life+0x120, &action, &dir, …)`. It reads the input snapshot at `0x020F27A0` (held at +0, pressed at +2) and `FUN_02150DF0(life, action, dir)` applies the result. | `FUN_021505D0`, `FUN_0213F7D8` | **blue-verified** |
| Controller state +0x120: 1 D-pad, 2 touch (switches when held bit `0x2000` changes). D-pad direction (`FUN_0213F774`): Up beats Down, Right beats Left; the result uses the facing codes above. Walk is action 7, run (B held with a direction) action 8, A pressed action 10 (talk). | `FUN_0213F7D8` case 1, `FUN_0213F774` | **blue-verified** |
| Step size: `FUN_02021B38(vec, dir, 0x100)` scales a unit vector (−1, 0 or 1 per axis) by 0x100, so a sub-step moves up to 1 px per axis. Diagonals are not normalised. `FUN_02150DF0` runs N sub-steps through `FUN_021522FC`: action 6 one, action 7 two, action 8 four. | `FUN_02021B38`, `FUN_02150DF0` cases 6–8 | **blue-verified** (static) |
| Runtime: walking right moves the box 2 px per frame, running 4 px, walking down-right 2 px on both axes (facing 1). Animation ids: walk `0x800`, run `0xA00`, idle after a stop `0x807`. Movement starts on the first frame of the press and stops on the first frame of release (no acceleration). | Per-frame box trace in the Team Base (investigator run, no hash) | **blue-verified** (runtime) |
| Sliding: when a sub-step is blocked, the code retries with (0, dy), then (dx, 0). A step that starts a script ends the movement. | `FUN_02150DF0` cases 6–8 | **blue-verified** (static); slide not run |
| B tap and X (with an option byte) give action `0x13` (`FUN_0215139C`, then `FUN_0213C1C4(7, …)`), with a 20-frame counter at state+0x10. | `FUN_0213F7D8` | **lead** ("check" or menu action) |

**Open (P1):** touch control (state 2), action 6, the exact B-tap rule; non-player lives move only by script ([script-opcodes.md](script-opcodes.md) `0x58`–`0x95`).

## P2 — Collision

| Claim | Evidence | Status |
|-------|----------|--------|
| One step `FUN_021522FC(life, vec)` moves the box, clamps it to the lives' limit rect and calls `FUN_02152F54`. Step result: 0 moved, 1 blocked, 2 blocked and a script started. | `FUN_021522FC` | **blue-verified** |
| `FUN_02152F54` converts the box to 8-px cells (the last cell uses x2 − 1) and tests in order: flag `0x1` any cell with bit `0x80` (`FUN_0214C224`); flag `0x2` bit `0x40` (`FUN_0214C198`); flag `0x4` overlap with other lives that have flag bits `0x44` (`FUN_0215358C`, strict AABB `a1 < b2 && b1 < a2`); flag `0x8` objects (`FUN_02155F44`); flag `0x10` events with flag `0x40` (`FUN_02157DC0`). A hit returns 1, or 2 when the hit actor has flag `0x40` and its script starts (`FUN_02153E04`, `FUN_02153D60`, `FUN_02153D00`). | `FUN_02152F54` and callees | **blue-verified** (static) |
| The collision grid is one byte per 8×8 px cell, row stride 0x100, pointer at `*(0x0215D1AC) + 0x544`, filled by a callback at +0x540 (allocator `FUN_0214B4D8`, freed by `FUN_0214B52C`). A cell is read at `buf + cellX + cellY * 0x100 + 0x405`. | Pool word at `0x0214C2AC`; the `0x405` offset read at runtime | **blue-verified** |
| Runtime in the Team Base (map 13): cell values 0 (free), `0x80` (wall), `0xFF` (outside padding). Walking left stopped at x1 = 176 px, the edge of a row of `0x80` cells; walking up stopped likewise. Bit `0x40` did not appear. | Grid dump and box trace (investigator run) | **blue-verified** (runtime) |
| Which BMA collision layer gives bit `0x80` and which gives `0x40` ([../formats/maps.md](../formats/maps.md)) is not established; the decoder is the callback at +0x540, not followed. | – | **lead** |

## P3 — Interaction and exits

| Claim | Evidence | Status |
|-------|----------|--------|
| Talk: action 10 calls `FUN_02151B04(life, facing)`. It probes the box one step ahead for lives and objects with flag `0x80` (`FUN_02151FDC`), then scans further along the facing direction over cells with bit `0x10` (`FUN_0214C140`), which lets the player talk across a counter. A hit runs `FUN_02153E04` (lives) or `FUN_02153D60` (object): take the target's script reference slot 2 (`FUN_02147EDC`), make the target remember the player (`FUN_02147C18`), stop the player's animation and install the script on the player's context with `FUN_02147C48`. Targets with flag `0x200` turn to face the player. | Functions named | **blue-verified** (static); counter scan **lead** |
| Runtime (map 9, outside the Team Base, the mailbox object, id 13, flags `0x184`): pressing A while facing it ran `FUN_02151B04` and then `FUN_02147C48` in the same frame, once for the object (from `0x02156190`) and once for the player (from `0x02153F00`). The object's script waits on token 2 (`0xE3`); the player's script sets an animation (`0x54`), binds the speaker (`0x2D` sub-mode 7), waits a frame, faces a mark (`0xCA`) and shows text (`0x32`); the next A shows the next text. | Run B, frames 4520–4702; investigator run (screenshot of the text box) | **blue-verified** (runtime) |
| Step-on regions are events. Event flag `0x20` is a step-on trigger, `0x40` also blocks (bump). After a successful step, a lives with flag `0x10` tests `FUN_02157DC0(0x20, box)`. With no event, +0x13E = −1; when the event differs from +0x13E and `FUN_02153D00` starts its script (event reference at +0x1C), +0x13E = the event and the step returns 2. A step-on fires once on entry and again only after leaving; starting inside a region does not fire it. | `FUN_021522FC`, `FUN_02153D00` | **blue-verified** (static and runtime: one trigger at the entry frame) |
| Exits: in the Team Base the south door is event 0 (id 357, flags `0x60`). Walking into it fires it as a step-on (`FUN_02153D00`, called from `0x0215317C`), which installs the event's script on the player's context. GROUND_MAP (var `0x11`) changes from 13 to 9 58 frames later. | Run B: step-on and installer at frame 4072, map 9 at frame 4130 | **blue-verified** (runtime) |
| The door's script is two commands on the player's context: `0x01` with A = 10 (request a mode transition through `FUN_0206F628`), then `0xF0` (suspend). The destination is not in the event or its script. The scene runner then runs loader entry `0x66` on the main context, which reads the ground variables (`0xC4`/`0xC5` checks, `0xEC` on var `0x0E` GROUND_ENTER), loads bank 9 (`0x1D`), sets map 9 (`0x08`), loads its layers (`0x0C`, `0x0D`) and the map's entry script (`0xEA`). | Run B opcode log, frames 4073–4140 | **blue-verified** (runtime) for the commands; how the transition request fills GROUND_ENTER **lead** |

## P4 — Camera

| Claim | Evidence | Status |
|-------|----------|--------|
| The camera struct is at `0x0215D2F0`: target kind (1 lives, 2 object, 3 event) and index, move mode, speed or frames, counter, centre (1/256 px), target position, shake fields, then the output scroll x, y in pixels and a valid byte. Updated each frame by `FUN_0213FE04`, reset by `FUN_021403B4`, set by `FUN_02140258`..`FUN_02140358`, read by `FUN_02140218` (callers: the BG scroll `FUN_0214B83C` and `FUN_0214F290`). | Functions named | **blue-verified** |
| Mode 0 (free movement): the centre snaps to the centre of the target's box every frame, with no smoothing. Modes 1 and 2 move toward the target by a capped step or over N frames (script camera moves, [script-opcodes.md](script-opcodes.md) `0x9B`–`0xA3`). | `FUN_0213FE04` | **blue-verified** (static); runtime: the scroll followed the player 2 px per frame and stopped with it |
| View origin: centre plus shake, minus a small fixed offset, clamped by `FUN_0214C2B0` to the map rectangle (`FUN_021489FC`) inset by half a screen (128 × 96 px). Maps of type 10 get an extra inset. So the 256 × 192 screen is centred on the target and never shows past the map edge. | `FUN_0213FE04`, `FUN_0214C2B0` | **blue-verified** (static); the clamp not reached at runtime |

## P5 — Runtime check

The investigator run for this issue (not committed, no hashes) loaded a maintainer save at the Pokémon Team Base and checked walk, run, diagonal, wall bumps, a step-on event, the exit to map 9 and talking to the mailbox object. The committed probe `verification/v7-trace/ground_script_log.py` logs the same signals (player box, GROUND_MAP, the talk, step-on and installer calls) and the opcodes; run B in [Trace checks](#trace-checks) repeats the walk with it.

Run B (committed probe, input `verification/v7-trace/ground_walk_inputs.txt`): from the save, walking right moved the box 2 px per frame (frames 3400–3425); a run into the right wall stopped at x2 = 304 px; walking left stopped at x1 = 184 px and up at y1 = 234 px (grid walls); down stopped at y2 = 360 px. Lined up with the door (x1 = 232 px), walking down fired the door's step-on at frame 4072 and the map became 9 at frame 4130. Outside, 12 px left and down to the mailbox, A at frame 4520 started the talk (P3).

**Not done:** the first town proper (the save is at the Team Base), a lives NPC (none in the rooms checked), the slide on a wall, the camera clamp. A save fixture cannot be committed; the savestate library of [#643](https://github.com/TorratDev/rust-mm/issues/643) is where a town state should go.

## Trace checks

| Script | Output hash | Command | Claims checked |
|--------|-------------|---------|----------------|
| `ground_script_log.py` (run B) | output SHA-256 `01b2e3177020ddd2…` | `XDG_CONFIG_HOME=<fresh> ~/venvs/desmume/bin/python -I verification/v7-trace/ground_script_log.py <scratch ROM copy> <OUT.jsonl> 5000 verification/v7-trace/ground_walk_inputs.txt --save <raw 256 KiB save at the Team Base>` (the save is a maintainer save, not committed; input SHA-1 `aaa93b9e…`) | P1 step sizes, P3 talk/step-on/exit, P4 follow |

## Links

- [scripts.md](scripts.md), [script-opcodes.md](script-opcodes.md)
- [../formats/maps.md](../formats/maps.md), [ground-bpc.md](ground-bpc.md)
- [execution-model.md](execution-model.md) (scene runner)
