# Ground-mode script engine

**Date:** 2026-11-23
**Dump:** Pokémon Mystery Dungeon – Rescue Team Blue (EUR), `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Static evidence comes from the ghidra-cli text export of the `rescue-arm9` project (STAMP 2026-11-14, 5284 ARM9 functions, 0 failed).
**Issue:** [#362](https://github.com/TorratDev/rust-mm/issues/362) (parent [#76](https://github.com/TorratDev/rust-mm/issues/76))
**Workflow:** [workflow.md](workflow.md)
**Format spec:** [../formats/scripts.md](../formats/scripts.md) (binary layout and the storage chain)

Addresses are ARM9 unless marked `ov_0000::` (overlay 0). Names of the form `FUN_…` are Ghidra names from the export. Each claim carries one status: **blue-verified**, **red-hypothesis** or **lead**. A name taken from a debug string is marked as such. No decompiler output, ROM bytes, table values or traces are in this note. Trace checks give only the command and output hash ([verification.md](../verification.md#362--script-engine-probes)).

## Status by point

| Point | Status | What is open |
|-------|--------|--------------|
| P1 storage, lookup, record layout | **blue-verified** | Whether bank overlays are compressed in the ROM; the unread sector and element fields |
| P2 interpreter | **blue-verified** (loop, dispatch, record, arguments, waits) | Meanings of the return codes 1, 3 and 4; six frame-counter outliers; input-wait opcodes |
| P3 opcode table | Opening-run set **blue-verified** for handlers; names mostly **red-hypothesis** | Per-variant meanings of the movement, camera and window families |
| P4 actors, camera, dialogue | **blue-verified** for the calls and fields; names **red-hypothesis** | Camera modes 1–3 never ran; vtable slots +0x3C and +0x40 |
| P5 story flags and scene variables | **blue-verified** (storage in the save, names, `0x14` codes) | Readers of `0x17`; `0x10` meaning; the save base word (see P5) |
| P6 inner phases 0x0–0x11 | **blue-verified** (per-id actions) | Demo meaning of `0xE`–`0x11`; `0x9`–`0xC` not reached at runtime |

Scope note: this note maps the engine. Turning #76 into an epic with sub-issues is a follow-up made from this note.

## P1 — Storage, lookup and binary layout

Full layout in [../formats/scripts.md](../formats/scripts.md).

| Claim | Evidence | Status |
|-------|----------|--------|
| Scripts are not separate ROM files. They are ARM9 data (loader table) and data inside ground-bank overlays. No script file appears in the file survey. | Loader table and bank lookup below. `docs/file-survey.md` has no script entry. | **blue-verified** |
| Loader scripts are a table of 12-byte records in ARM9 data. Word 0 is the script pointer; the table is read at `table + idx*0xC`. The table base is at `0x020DA5D4` at runtime. | `FUN_ov_0000__02147E6C` reads word 0; its base literal is at `ov_0000::02147ED4`. Layout also in [execution-model.md](execution-model.md#loader-inventory). | **blue-verified** |
| A loader reference is built as `{ptr, kind 2, group 0xFFFF, sector 0xFF}`. | `FUN_ov_0000__02147E6C` | **blue-verified** |
| Bank scripts are data in ground-bank overlays (overlay ids 10–144). Each bank is an ordinary FAT overlay loaded at `0x0215D720` by `GroundStation_LoadBank`. | `GroundStation_LoadBank` `ov_0000::0214FD00`; runtime check below | **blue-verified** |
| A bank is not its overlay id. The overlay id is read from a table at `0x020DB8E0` indexed by `bank*8`, and the root pointer from `0x020DB8E4` (same index). | Literals `ov_0000::0214FD90` and `ov_0000::0214FC48`; `FUN_ov_0000__0214FBF0` returns the root and asserts the bank is current (`0x0214FC40`). Bank `-1` returns a default. | **blue-verified** (static); the probe saw the two tables differ, values not recorded |
| Lookup chain: bank → root → group → sector → list → element → script. The root is `{u32 group count, u32 groups ptr}`, a group is `{u32 sector count, u32 sectors ptr}`, and a sector is 0x28 bytes with its list count at +0x20 and list pointer at +0x24. The script is the pointer at `+8` of the first list element. | `FUN_ov_0000__0214D510(out, bank, group, sector)`: `SetBank` `ov_0000::0214FC50`; asserts "group < count", "sector < count", and "list != 0 unless count <= 0". | **blue-verified** |
| The output is a 12-byte reference `{u32 ptr at +0, u16 kind at +4, u16 group at +6, u8 sector at +8}`. Kind is 2 for a loader script and 5 for a bank script that replaces one. `FUN_ov_0000__0214D420` sets kind 5 when its flag argument is nonzero; `FUN_ov_0000__0214D498` does the same for loader scripts. Both then call the installer. | `FUN_ov_0000__0214D420`, `FUN_ov_0000__0214D498`; probe `script_lookup_probe.py` reads `+0`, `+4`, `+6`, `+8` | ptr at +0 **blue-verified** (runtime, 10 of 10); kind at +4 **blue-verified** (runtime: 2 on loader installs, 5 on bank installs); group at +6 and sector at +8 **red-hypothesis** (the probe reads them but does not compare them with anything) |
| Installer `FUN_ov_0000__02147C48(ctx, pos, ref)` sets the run state and copies the reference into the frame. Kind 5 (bank script) replaces a kind-2 (loader) script and saves it in ctx+0x84 (a 0x60-byte block at ctx+0x24..0x83); kind 5 asserts that the saved slot is empty. | `FUN_ov_0000__02147C48` | **blue-verified** |
| Runtime check of the chain: 10 of 10 installs (4 loader, 6 bank) match the pointer recomputed from the tables. Scenes `0xF`, `0x10` and `0` plus three chained loads. Bank pointers are all in the bank area; loader pointers are all below the ARM9 end of the code area. | Probe `script_lookup_probe.py`, [verification.md](../verification.md#362--script-engine-probes) | **blue-verified** (runtime) |
| The script is a sequence of 0x18-byte command records with no header: the pointer is the first command. Byte 0 is the opcode. | `FUN_ov_0000__021421CC` loop top; record copy at ctx+0x28/0x40 | **blue-verified** |
| Labels are opcode `0xF4` with the label id in word 0's high halfword (`s16` at +2). Jumps go by label id, not byte offset. `FUN_ov_0000__021406A8` scans from the script base for the label and asserts the scan does not cross opcode `0x00` or `0xF6`. | `FUN_ov_0000__021406A8`; `ov_0000::021421CC` case `0xF4` | **blue-verified** (scan and assert) |
| `0xF6` marks the end of a script block. | Assert text in `FUN_021406A8`; no run reached it | **red-hypothesis**: Blue must show a script's last record is `0xF6` or `0x00` |
| `0xEF` ends a script (returns 0). `0xF0` returns 1 to the stepper. Whether `0xF0` ends the script or parks it is not settled. [execution-model.md](execution-model.md) says it ends a script. | `ov_0000::02146134` (`0xEF`), `ov_0000::02146154` (`0xF0`) | `0xEF` **blue-verified**; `0xF0` return code **blue-verified**, its meaning **lead** (conflicts with execution-model.md) |

**Open (P1):** whether bank overlays are compressed (the loader uses the normal FS overlay API, so probably not); the sector record fields `+0x00`–`+0x1F` and element fields `+0`, `+4`, `+0xC` onward (a **lead** for P4 placement lists); whether element `list[1..]` holds other scripts used by `0x1D`, `0xEA` or `0xEB`.

## P2 — Interpreter

| Claim | Evidence | Status |
|-------|----------|--------|
| `FUN_ov_0000__021421CC` is the GroundScript command interpreter. Its only caller is the stepper `FUN_ov_0000__021461D0`, at `0x0214787C`. | Debug strings "AnalyzeProcess command null" (`0x021421E4`), "Script command call error …"; caller found in the export xrefs | **blue-verified** |
| Main loop top is `0x02142304`. Each step reads the cursor at ctx+0x28, advances it by 0x18, stores the record pointer at ctx+0x40 and the opcode at ctx+0x44, and copies the record to the stack. | Loop top of `ov_0000::021421CC`; runtime: record = cursor − 0x18 in 228 of 228 dispatches | **blue-verified** (static + runtime) |
| Dispatch: `cmp r2, #0xF6` then `addls pc, pc, r2 lsl #2` at `0x02142344`. Out of range (`> 0xF6`) goes to `0x02146164`, the error path. | Instructions at `0x02142344`, `0x02142348` | **blue-verified** (static) |
| The dispatch is an inline table of 0xF7 `B` instructions from `0x0214234C` to `0x02142724`. | Same | **blue-verified** (static) |
| Number of opcodes: 247 dispatch slots (`0x00`–`0xF6`). Slots `0x00`, `0x40`, `0x96`, `0xCC`–`0xCE`, `0xD0`, `0xD1` and `0xD9` go to the error handler `0x02146164` ("GroundScript command error %d %p"). `0xF2`–`0xF6` are no-ops that fall through to the loop top. | Table decode (not committed) | **blue-verified** (static). The count of distinct handlers is not computed. |
| Runtime: 228 dispatches in one 3000-frame run (boot to dungeon), 114 interpreter entries; every opcode ≤ `0xF6`; the cursor moved by exactly one record 105 times and jumped 9 times. | Probe `interp_probe.py`; counts only | **blue-verified** (runtime) |
| Argument encoding: record byte 1 is a small arg (frame counts, compare ops); `s16` at +2 is a label or variable id; `s32` at +4 is the main operand (low `s16` often an id); +8, +0xC, +0x10, +0x14 are more `s32` operands. A field of `-1` means "read from a variable". | Field use in `ov_0000::021421CC`; `FUN_ov_0000__021421CC` cases 2 and 7 call `FUN_02020F74` with id `0x14` | **blue-verified** (static) |
| Variables: ids below `0x400` are global (valid range checked in `FUN_02021108`); ids `0x400` and up are local, held in the frame at ctx+0x74. The getter is `FUN_02020F74` (ARM9 `0x02020F74`). The value switches `0xC0`–`0xC2` and the `0xCE` case record read variables through the same getter; `0xCC`–`0xCD` compare immediates. | `FUN_02020F74`, `FUN_02021108` (ARM9) | **blue-verified** (static). Runtime local reads not observed (no call with the interpreter as caller in the run). |
| Case records after a switch: `0xCC` compares with op 2, `0xCD` with op = byte 1, `0xCE` against a variable. Each record has a label that is jumped to on match. Values come from `FUN_ov_0000__021405B4`. | `FUN_ov_0000__021405B4` | **blue-verified** (static) |
| Value switches `0xC0`–`0xCB` each compute a value and then select a case: `0xC0` a variable; `0xC1`/`0xC2` compare a variable with an immediate or a variable through the ARM9 evaluator; `0xC3` the random switch (see [rng.md](rng.md)); `0xC4`/`0xC5` a flag test through ARM9 `0x02020D90`; `0xC6`–`0xCB` actor and position queries. | Group `0xC0`–`0xCB` at `ov_0000::021457FC` and handlers | **blue-verified** (calls); meanings **red-hypothesis** |
| Waits return 2. The stepper keeps ctx+0x26 = 2 and retries the same record next frame. | Stepper `FUN_ov_0000__021461D0`; return 2 seen 87 times in the run | **blue-verified** (static + runtime count) |
| Frame waits: opcodes set ctx+0x4E from byte 1 and return 2; the stepper counts it down once per frame. `0xDB` waits N frames and `0xDC` waits a random `min + rand(max − min)` frames (see [rng.md](rng.md)). `0xFFFF` at ctx+0x4E means "until done". | `ov_0000::02145E44` (`0xDB`), `ov_0000::02145E58` (`0xDC`), stepper countdown label `LAB_021466B4` | **blue-verified** (static). Runtime: 1207 of 1213 frame pairs dropped by exactly 1. |
| Six of 1213 frame pairs did not drop by 1. | Same run | **lead**: several contexts, or a reload |
| Text, menu and input waits: opcodes lock the script (`FUN_ov_0000__02147B80`) and return 2; the stepper polls `FUN_0213C0CC` each frame and resumes when the window is ready. Input (`0x3D`–`0x3F`) and key waits (`0x2C`) use the same path. | Stepper cases ov_0000 `021461D0` lines 272–425; `FUN_0213C0CC` (ov0 call) | **blue-verified** (flow); which opcode is which window is P4 |
| Fade and event waits: `0x22`–`0x28` and `0xDF` poll `ARM9 FUN_02139E64`; `0xE0` polls `FUN_02029AE0` with a frame timeout; `0xE1`/`0xE2` poll a sound/event helper with a 0xE10-frame timeout. | `ov_0000::02145ED4` group, stepper `LAB_02146710` | **blue-verified** (flow); the meanings are **red-hypothesis** |
| Sound waits print "bgm wait time over" and "me or se wait time over" if they time out. | Debug strings in the stepper | **lead** |
| Return codes from the interpreter: 0 = end, 1 = hold, 2 = wait, 3 = run on, 4 = suspend. The stepper stores the return value in ctx+0x26 and branches on it. On 0, a saved frame in ctx+0x84 is restored if ctx+0x86 is nonzero, else `FUN_ov_0000__021480E4` runs. | `FUN_ov_0000__021461D0` call at `0x0214787C` | Numeric values and branches **blue-verified** (static). The meanings of 1, 3 and 4 **red-hypothesis**. |
| Execution frame: PC at +0x28, script base at +0x2C, record at +0x40, opcode at +0x44, sub-state at +0x4C, frame counter at +0x4E, elapsed at +0x50, locals at +0x74. | `FUN_ov_0000__021421CC`, stepper | **blue-verified** (static) |

**Open (P2):** meanings of return codes 1, 3 and 4 and of stepper state 4; the six counter outliers; input wait opcodes; whether the three stepper callers (`FUN_ov_0000__021505D0`, `FUN_0215574C`, `FUN_02156CD8`, at `0x0215064C`, `0x02155780`, `0x02156D0C`) are the owner updaters. **lead**: they are probably the station/map, lives and object updaters, from the cancel strings.

## P3 — Opcode table

Handler addresses are `ov_0000` unless marked ARM9. "Seen" means the opcode dispatched in the 3000-frame boot-to-dungeon run; the run saw 36 opcodes, from frame 76 to 2582, and the most frequent were `0x80`, `0x54`, `0xE3`, `0xDB`, `0xE4`, `0x53` and `0x52`. Not seen in that run: `0x0B`, `0x6D`, `0xDC`, `0xC3`.

The meaning column is a one-line reading. Where the name is a Blue debug string, the string is the evidence; where only the handler's calls are known, the meaning is marked.

### Opcodes the opening run uses

| Opcode | Handler | Meaning | Status |
|--------|---------|---------|--------|
| `0x08` / `0x09` | `0x02142A3C` | Map or ground select: loads a ground map. Debug strings "map select", "ground select %3d %3d[%s]". | **blue-verified** (strings); the 08/09 split **red-hypothesis** |
| `0x0C`–`0x14` (`0x15` same label) | `0x02142CC8` | Select group/sector of a station, lives, object, effect or event owner. Calls the four Select and Cancel families (P4). Strings "select group or sector error". | **blue-verified** (strings and calls); per-opcode owner **lead** |
| `0x1E` | `0x02143190` | Map execute sub command: loads a sector of the current map. Falls into the `0x1D` path: `FUN_0214D1F0`, `FUN_0214D420`, `FUN_0214D510`. | **blue-verified** |
| `0x22`, `0x23` | `0x0214350C`, `0x02143528` | Call `FUN_0213A11C` / `FUN_0213A104` with byte 1; with byte 1 nonzero they return 2 and wait on `FUN_02139E64`. | Flow **blue-verified**; meaning (screen fade in/out) **red-hypothesis** |
| `0x27`, `0x28` | `0x021435B0` | Fade family with `FUN_02139FC4` / `FUN_02139EE4` and a 4-byte parameter. | Flow **blue-verified**; fade with colour **red-hypothesis** |
| `0x2D` | `0x02143710` | Speaker/portrait setup, 11 sub-modes in byte 1. Debug string "FLOOR_LEVEL". See P4. | **red-hypothesis** (message/speaker setup); **lead** for sub-modes |
| `0x37` | `0x0214393C` (group `0x32`–`0x38`) | Show message with no speaker (`FUN_0213C510`), then lock and wait. | Handler **blue-verified**; meaning **red-hypothesis** |
| `0x3B` | `0x02143B48` | Writes 0 to ctx+0x4E and returns 2; the stepper resumes when the message closes. | **red-hypothesis** (wait for message close) |
| `0x3C` | `0x02143B0C` | MENU: `FUN_0213C104` and `FUN_02154004`, returns 2. String "MENU". | **blue-verified** (string) |
| `0x44` | `0x02143C98` | Audio: `FUN_0201BFA0`, or `FUN_0201BE04` when the id is a sentinel. | **red-hypothesis** (BGM play); the sound thunks are not verified here |
| `0x47` | `0x02143D60` | Calls `thunk_FUN_0201BE04` only. | **red-hypothesis** (BGM stop) |
| `0x52` | `0x02143EB4` | Calls owner vtable +0x3C. | Call **blue-verified**; meaning **lead** |
| `0x53` | `0x02143ECC` | Calls owner vtable +0x40; bit `0x400` releases a lock (`FUN_021480E4`). | Call **blue-verified**; meaning **lead** |
| `0x54` | `0x02143F04` | Set animation: calls +0x14, stores the animation id at ctx+0x48, calls +0x30. | Calls **blue-verified**; "set animation" **red-hypothesis** |
| `0x5B` | `0x021440F0` | Move to a position mark: `FUN_02157CC8` gives the target, vtable +0x24 sets it, ctx+0x4E gets a frame count. Returns 2. | **red-hypothesis** (move to another actor's position) |
| `0x68` | `0x02144A70` | Change height, part of the movement family. | **blue-verified** (static); family specifics **lead** |
| `0x6B` | `0x021444A4` | Move toward an actor; duration 0xFFFF (until arrival). Same handler family as `0x63`, `0x64`, `0x65`. | **blue-verified** (static); "move to actor" **red-hypothesis** |
| `0x80` | `0x02144804` (`0x74` family) | Sliding move to a position, interpolated over frames (lerp `FUN_02021660`). | **blue-verified** (static and runtime: 1522 lerp calls, none from other ops) |
| `0x8B` | `0x02144B30` | Set direction from the argument (ctx+0x4A) and wait. | **blue-verified** (static); "turn" **red-hypothesis** |
| `0x91` | `0x02144DBC` | Sets ctx+0x71 from byte 1 and returns 2. | **lead** |
| `0x98`, `0x99` | `0x02144E20`, `0x02144E88` | **Camera** ops: attach camera to self or var 2/3 (`0x98`); fix camera to self (`0x99`). Setters `ov_0000::0214033C` and `ov_0000::02140358`. | **blue-verified** (setters, runtime counts); name **red-hypothesis**. Source conflict: the opcode-table pass read these as actor attach through the same setters. The camera reading is the one the setter addresses and the runtime counts support (see P4). |
| `0xA6` | `0x02145390` | Variable arithmetic through ARM9 `FUN_0201FDD8` (locals, var, value, op). | Call **blue-verified**; "arithmetic" **red-hypothesis** |
| `0xA9` | `0x021453E8` | Set two elements of a scenario variable through ARM9 `FUN_020206AC` / `FUN_020208E8`. For var 4 it resets var `0x1A`. | **blue-verified** (static) |
| `0xC4` | in value-switch group, `0x021457FC` | Flag test through ARM9 `FUN_02020D90`. Error "switch type error". | **blue-verified** (call); meaning **lead** |
| `0xDB` | `0x02145E44` | Wait N frames: ctx+0x4E = argument, returns 2. The stepper counts it down. | **blue-verified** (static; runtime countdown consistent) |
| `0xE0` | `0x02145ED4` (group `0xDF`–`0xE2`) | Wait: polls `FUN_02029AE0(arg)`, timed out by ctx+0x50 against a limit. | Flow **blue-verified**; meaning **red-hypothesis** |
| `0xE3` | `0x02145EDC` | Lock: ctx+0x4C = id, `FUN_02147B80(ctx, id)`; returns 2 while non-zero. Resumes when ctx+0x46 = −1, then branches through a table indexed by ctx+0x4C. | Flow **blue-verified**; "wait for a lock/sub-script" **red-hypothesis** |
| `0xE4` | `0x02145F00` | `FUN_02147AAC(id)`: unlock or signal, paired with `0xE3`. | **red-hypothesis** |
| `0xE9` | `0x02145FA0` | Jump to a script reference: ctx+0x28 = ctx+0x2C = the entry's +8 from the table indexed by the argument. | **blue-verified** (static) |
| `0xEF` | `0x02146134` | End the script; returns 0. Logs "Script end stack error" if ctx+0x34 ≠ 0. | **blue-verified** |
| `0xF0` | `0x02146154` | Returns 1 to the stepper. | Return **blue-verified**; meaning **lead** (see P1) |
| `0xF1` | `0x0214615C` | Returns 4 (suspend). | Return **blue-verified**; meaning **lead** |

### Other opcodes (named or grouped, not seen in the opening run)

| Opcode(s) | Handler | Meaning | Status |
|-----------|---------|---------|--------|
| `0x01`–`0x07` | `ov_0000` select group | Ground select, dungeon select, ENTER HABITAT / RESCUE / RESCUE USER / TRAINING, dungeon enter check. Debug strings. | **blue-verified** (strings) |
| `0x0A` | | Dungeon select (group) | **blue-verified** (string) |
| `0x0B` | callee `0x0214DCDC` (B-table handler not given) | Calls `FUN_0214DCDC(arg)`. Not the RNG operator (see rng.md). | **lead** |
| `0x16`–`0x18` | `0x02142F08`, `0x02142F2C`, `0x02142F50` | Cancel blank lives, objects, effects. | **blue-verified** (static) |
| `0x19` | `0x02142F74` | Add an object at the caller's position with its direction. | **blue-verified** (static) |
| `0x1A` | `0x02143034` | Add an effect the same way. | **blue-verified** (static) |
| `0x1B`, `0x1C` | | Map execute sub event (`FUN_0214D498`). | **blue-verified** (string) |
| `0x1F` | | Dungeon rescue select. | **blue-verified** (string) |
| `0x20`, `0x21` | | Parent-kind call; execute jump-talk lives script. | **blue-verified** (string) |
| `0x29`–`0x2B` | `FUN_0213D358`, `FUN_0213D33C`, `FUN_0213D2F0` (ARM9) | Message window calls. | Calls **blue-verified**; meanings **red-hypothesis** |
| `0x2C` | `FUN_0213D2CC` | "MESSAGE WAIT KEY": locks the script, returns 2. | **blue-verified** (string) |
| `0x2E`, `0x2F` | `FUN_0213C754`, `FUN_0213C66C` | Portrait face and position. | Calls **blue-verified** |
| `0x30` | `FUN_0213D254` | "MESSAGE CLOSE". | **blue-verified** (string) |
| `0x31` | `FUN_0213D1F4` | "MESSAGE ENFORCE". | **blue-verified** (string) |
| `0x3A` | | ANSWER (choice) | **blue-verified** (string) |
| `0x3D`–`0x3F` | | INPUT LIVES NAME / TEAM NAME / PASSWORD. | **blue-verified** (string) |
| `0x50`, `0x51` | `FUN_0214814C`; `FUN_02157BE8` | Copy position and direction from another script context; move to a random point in a mark. | **blue-verified** (static) |
| `0x55`, `0x56`, `0x57` | | Sign; effect; resize sprite. | Names from strings **blue-verified** |
| `0x58`–`0x60` | | Instant placement: set position; offset; random offset (`OtherRandInt` ARM9 `0x02023B84`); mark; random point in a mark; lives actor; from vars `0x34`–`0x37`; save to vars; height. | **blue-verified** (static; the vars are P5) |
| `0x61`–`0x67`, `0x69`–`0x6F` | | Walk at speed to a target; the `0x69` group also turns to face the motion (ARM9 `0x02021A28`). | **blue-verified** (static) |
| `0x71`–`0x88` | `0x021445B0` … `0x021449B4` | Move to a position with 8.8 fixed point (`<< 8`) and frame counts; four families of six (absolute, relative, target-actor) at `0x71–0x76`, `0x77–0x7C`, `0x7D–0x82`, `0x83–0x88`. | **blue-verified** (arithmetic); per-variant meanings **lead** |
| `0x89`–`0x95` | | Direction: set and wait (`0x89`); turn relative (`0x8A`, `0x8D`); from arg (`0x8B`); relative to an actor (`0x8C`); face an actor, position or mark (`0x8E`–`0x90`); target direction at +0x71 (`0x91`–`0x95`). | **blue-verified** (static) |
| `0x97` | `ov_0000::02140380` | Camera shake or offset. | **blue-verified** (setter); name **red-hypothesis** |
| `0x9A` | | Camera attach to the leader? | **lead** |
| `0x9B`–`0xA3` | | Camera modes 1–3 with a target and speed. | **blue-verified** (setters); names **red-hypothesis**; never ran |
| `0xAA` | | Checked scenario variable set ("scenario number error"). | **blue-verified** (string) |
| `0xB3`–`0xBF` | `FUN_021406A8` | Conditional branches to labels. | **blue-verified** (static) |
| `0xB7` | | BAR_SUM | **blue-verified** (string) |
| `0xC0`–`0xCB` | `ov_0000::021457FC` | Value switches (see P2). | **blue-verified** (calls) |
| `0xCC`–`0xCE` | | Case records, read by `FUN_021405B4`. Dispatched as error. | **blue-verified** (static) |
| `0xCF` | | SWITCH MESSAGE | **blue-verified** (string) |
| `0xD2`–`0xD8` | | SWITCH MENU | **blue-verified** (string) |
| `0xDC` | `0x02145E58` | Wait `min + rand(max − min)` frames. | **blue-verified** (static; not run) |
| `0xDD`, `0xDE` | | Wait on owner vtable +0x4C (animation) and +0x50 (effect). | **blue-verified** (static) |
| `0xE6`, `0xE8`, `0xEA` | | Call and stack operations ("Script stack over"). | **blue-verified** (strings) |
| `0xEB`, `0xEC` | | Map jump or call; map enter execute. | **blue-verified** (strings) |
| `0xED`, `0xEE` | | Flash stack; return. | **blue-verified** (strings) |
| `0xF2`–`0xF6` | loop top | No-op; `0xF4` is the label. | **blue-verified** (static) |
| `0x00`, `0x40`, `0x96`, `0xCC`–`0xCE`, `0xD0`, `0xD1`, `0xD9` | `0x02146164` | Unused; error "GroundScript command error %d %p". | **blue-verified** (static) |

**Grouped shapes** (dispatch shares one handler): `0x08/09`; `0x0C`–`0x15`; `0x27/28`; `0x32`–`0x38`; `0x49/4C`, `0x4A/4D`, `0x4B/4E`; `0x61/69`, `0x62/6A`, `0x63/6B`, `0x64/6C`, `0x65/6D`, `0x66/67/6E/6F`, `0x68/70`; `0x8E`–`0x90`; `0x93`–`0x95`; `0xD2`–`0xD8`; `0xDF`–`0xE2`.

**Open (P3):** the per-variant meanings of `0x71`–`0x88`, `0x0C`–`0x14` and `0x98`–`0xA3`; the exact meaning of `0x2D`, `0x91` and `0xF1`; the `0x6D` and `0xDC` runs past frame 3000.

## P4 — Actors, camera and dialogue

### Entity classes and creation

| Claim | Evidence | Status |
|-------|----------|--------|
| Four ground entity classes, each with Select, Add, Delete and Cancel. Lives (actors, Pokémon): Select `0x02155314`, Add `0x02154220`, Delete `0x021541A8`, Execute `0x02153F08`, GetTalkId `0x0215362C`, Cancel `0x0215528C`, CancelBlank `0x021551CC`. Object: Select `0x02156980`, Add `0x02156360`, Delete `0x021562E8`, Execute `0x02156140`, Cancel `0x021568FC`, CancelBlank `0x0215683C`. Effect (performers): Select `0x021579AC`, Add `0x02157540`, Delete `0x021574C8`, Execute `0x02157360`, Cancel `0x02157928`, CancelBlank `0x02157868`. Event (trigger areas): Select `0x02158118`, Add `0x02157EE0`, Delete `0x02157EAC`, Cancel `0x02158090`. | Debug strings and the function names in the export | **blue-verified** (strings) |
| Bulk creation is per group and sector. Opcodes `0x0C`–`0x14` call the four Select and four Cancel functions. | `ov_0000::02142CC8`; string "select group or sector error" | **blue-verified** |
| Runtime, boot-to-dungeon run: 13 lives selects, 7 lives adds, 13 object selects, 7 effect selects, 7 event selects. | Probe `actor_probe.py` | **blue-verified** (runtime) |

### Owner and movement fields

| Claim | Evidence | Status |
|-------|----------|--------|
| ctx is the interpreter's frame. `*ctx` is the owner vtable; `ctx[1]` the owner handle; `ctx[2]` the owner kind: 1 lives, 2 object, 3 effect. A script's `0x98`/`0x99` error "GroundScript myself kind error" when the kind is wrong. | `ov_0000::021421CC` | **blue-verified** (static) |
| Owner vtable slots: +4 id; +0x0C get position; +0x10 get height; +0x14 get direction; +0x20/+0x24 set position; +0x28 set height; +0x2C set direction; +0x30 set animation; +0x34 sign; +0x38 effect; +0x3C, +0x40 unnamed; +0x44 resize; +0x48 move with collision (returns flags; `& 9` = blocked); +0x4C and +0x50 wait-done checks. | Vtable calls made by the animation and wait handlers `0x02143EB4`, `0x02143ECC`, `0x02143F04` and the wait cases `0xDD`/`0xDE` | **blue-verified** (calls); names **red-hypothesis** (+0x3C, +0x40 meanings unchecked) |
| Positions are 8.8 fixed point (`<< 8`). The movement fields are: direction at ctx+0x4A, frame counter at +0x4E, interpolation step at +0x50, speed at +0x54, start at +0x58, target at +0x60/+0x64, target height at +0x6C, target direction at +0x71. | Stepper and handlers | **blue-verified** (static) |
| Walking and sliding are interpolated by the stepper: each frame it lerps (`FUN_02021660`) from the start to the target over the remaining frames, applies it through the owner vtable, and at zero it sets the run state back to 3. Walks stop early when collision blocks them. | Step labels `0x0214699C`, `0x02146A0C`, `0x02146800`, `0x021468BC`, `0x02146AD4`; `0x89`/`0x8A` at `0x02146B40` | **blue-verified** (static); runtime lerp calls 1522, from the `0x71`–`0x88` family only |
| The family `0x7D`–`0x88` turns the argument into frames: distance (`0x02017690`) divided by speed (`0x0208625C`). `0x71`–`0x7C` take frames directly. Only the `0x7D`/`0x83` branches were read in full. | Handlers `0x021445B0`–`0x021449B4` | **blue-verified** (arithmetic, one branch in full) |
| Position marks: `FUN_02157CC8` resolves a mark (the message string is "link_type_error1"). Runtime: 30 calls. | `FUN_02157CC8` | **blue-verified** (static + runtime count) |

### Animation

| Claim | Evidence | Status |
|-------|----------|--------|
| `0x54` sets the animation through owner +0x30; a non-zero id goes to ctx+0x48. `0x52` calls +0x3C and `0x53` calls +0x40. `0xDD` and `0xDE` wait on +0x4C and +0x50. | Handlers `0x02143EB4`, `0x02143ECC`, `0x02143F04` | **blue-verified** (calls) |
| What +0x3C and +0x40 do (flags, palette) | Not checked | **lead** |

### Camera

| Claim | Evidence | Status |
|-------|----------|--------|
| One global camera struct. Written by `FUN_021403B4` (reset, called twice from the interpreter) and by setters at `0x0214033C`, `0x02140358`, `0x02140380`, `0x021402F0`/`0x02140310`, `0x021402A4`/`0x021402C4`, `0x02140258`/`0x02140278`. Runtime: all twelve literal pools point at one struct. | Setters and literal pools | **blue-verified** (static + runtime) |
| Struct fields: target kind (+0) and id (+4); mode (+8: 0 attach/fixed, 1–3 moves); speed or frames (+0xC/+0x10); target position for modes 1–3 (+0x14/+0x18); fixed position for mode 0 (+0x1C/+0x20); shake offset (+0x24–0x38); current position (+0x3C/+0x40); a flag (+0x44). | Setter and getter code | **blue-verified** (static) |
| Getter `FUN_02140218` returns position and flag. Callers include the map stepper `FUN_021505D0`. `FUN_02140238` reports camera busy (mode ≠ 0). | Export xrefs | **blue-verified** (static) |
| Opcode mapping: `0x97` shake; `0x98` attach to self or var 2/3; `0x99` fix to self (by kind); `0x9B`–`0x9D` mode 1; `0x9E`–`0xA0` mode 2; `0xA1`–`0xA3` mode 3 (each with a target and speed). The stepper waits for `0x9B`–`0xA3` until the camera is idle. | Setters and stepper block `ov_0000::02146DB4` | Writes **blue-verified**; the names "attach", "move" **red-hypothesis** |
| Runtime: `cam_get` 5400 calls in 3000 frames; `0x98` ran 4 times and `0x99` 13 times; mode stayed 0 on all 2921 sampled frames; the position changed on 240 frames. Modes 1–3 did not run in the opening. | Probe `actor_probe.py` | **blue-verified** (runtime counts) |

### Dialogue

| Claim | Evidence | Status |
|-------|----------|--------|
| Message window: `0x29`–`0x2B` call the window setters; `0x2C` waits for a key and locks; `0x30` closes; `0x31` forces. Strings "MESSAGE WAIT KEY", "MESSAGE CLOSE", "MESSAGE ENFORCE". | `FUN_0213D358`, `FUN_0213D33C`, `FUN_0213D2F0`, `FUN_0213D2CC`, `FUN_0213D254`, `FUN_0213D1F4` | **blue-verified** (calls and strings) |
| `0x2D` sets the speaker and portrait; sub-modes in byte 1: 0 → `FUN_0213D0B8`; 1–3 → `FUN_02140570` (from ctx+8, ctx+0xC or a lives actor); 4–6 → `FUN_0214052C` with fallback `FUN_021404D4`; 7–9 → `FUN_021404E8` / `FUN_021404C0`; 10 → writes a species-based value to a table. | `ov_0000::02143710` | Calls **blue-verified**; sub-mode meanings **lead** |
| Text opcodes `0x32`–`0x39`: `0x32`/`0x33` show with a fixed speaker kind 0 or 1; `0x34`–`0x36` take the speaker from locals; `0x37` shows with no speaker (`FUN_0213C510`); `0x38` (`FUN_0213C4C0`); `0x39` by id (`FUN_0213C470`). `0x3A` ANSWER, `0x3C` MENU, `0x3D`–`0x3F` input. All lock the script and return 2; the stepper releases them by polling `FUN_0213C0CC`. | `ov_0000::0214393C` and the stepper | **blue-verified** (flow) |
| Message strings are inline in the script data, one per language. `FUN_ov_0000__021421A0` returns the record's inline pointer, and for language index ≥ 1 adds a per-language u16 offset. They are not message-table IDs. | `FUN_ov_0000__021421A0`; language index at `0x021421C8` | **blue-verified** (static); runtime: 6 of 6 `0x37` dispatches pointed at a printable NUL-terminated string (text not recorded) |
| Text engine chain: `FUN_0213C510`/`FUN_0213C560` → `FUN_0213BE70` (portrait slots) → `FUN_0213D370` → window renderer `FUN_0213D754`, which calls the `$` expander at ARM9 `0x0202DF1C` and the lookup `thunk_0201D948`. | [text-engine.md](text-engine.md#links) | **blue-verified** (static); runtime: `0x0202DF1C` ran 10 times, `FUN_0213D754` did not run in 3000 frames, so the drawing path for the opening messages is **lead** (candidate: `FUN_0213DC98`, which also calls `0201D948`) |
| Lock and release: `FUN_02147B80` locks a script; `FUN_02147AAC` releases it (`0xE3`/`0xE4`). How a script drives other actors through their own contexts is not traced. | `ov_0000::02145EDC`, `ov_0000::02145F00` | Flow **blue-verified**; rest **lead** |

**Open (P4):** camera modes 1–3 names (none ran); vtable slots +0x3C and +0x40; the `0x2D` sub-modes; which renderer draws the opening's `0x37` text; `0x5C` breaks in the stepper and may not wait (read, not checked).

## P5 — Story flags and scene variables

Names come from the descriptor debug names read at runtime (the probe `scene_probe.py`). Offsets, types and counts are not committed.

| Variable | Name | Meaning | Status |
|----------|------|---------|--------|
| `0x0E` | GROUND_ENTER | Ground entry for the scene | **blue-verified** (runtime name) |
| `0x0F` | GROUND_ENTER_LINK | Link ground entry | **blue-verified** (runtime name) |
| `0x10` | GROUND_GETOUT | "Ground left from": the return point after a dungeon or habitat. Written as the previous GROUND_ENTER. Scene `0xD` resets it to 0. Read by `FUN_02140860` case `0x33`, which compares it with 5. | Writes and reader **blue-verified** (static); meaning **red-hypothesis** |
| `0x11` | GROUND_MAP | Current map. Written by opcodes 8 and 9. | **blue-verified** (runtime name; execution-model.md [#160] records the same) |
| `0x12` | GROUND_PLACE | Place value from the map's record. Written by opcodes 8 and 9. Read by `FUN_0206F320` and passed to `ov_0000::021482CC`. Reset to 0 when it equals 10 and on rescue scenario/random entry. | **blue-verified** (runtime) |
| `0x13` | DUNGEON_SELECT | Dungeon select | **blue-verified** (runtime name) |
| `0x14` | DUNGEON_ENTER | Dungeon or scenario id. `0x50` = rescue USER (mail), `0x51` = rescue RANDOM, `0x52` = TRAINING, `-1` = none. Readers: `FUN_0206EE98` (rejects −1, `0x50`, `0x51`, `0x52`), `FUN_0206EDA8` (`0x50`), `FUN_0206EE38` (`0x51`), `FUN_0206EDE8` (`0x52`), `FUN_0206F1C0`. Writers: runner exit states 3–8, `FUN_0206EF14` (mail → `0x50`), ARM9 `RunGameMode_Async` (`0x0201E3C8`, line 161), interpreter case 7, stepper `0x021461D0` (writes 0x13, then −1). | Codes and readers **blue-verified** (static) | 
| `0x15` | DUNGEON_ENTER_INDEX | Sub-id that goes with `0x14`: scenario sub-id, random rescue id, mail id or training id. `FUN_02070890` uses it when `0x14` is `0x51`. | **blue-verified** (static) |
| `0x17` | DUNGEON_RESULT | Result of a dungeon run: the runner writes 4 (habitat), 5 (dungeon) or 7 (rescue, random, user, training). Scene ids 9–`0xC` also write it. | Writes **blue-verified** (static); readers: none in ARM9 or ov0 and no read in 3000 frames, so scripts read it by id (**lead**) |
| `0x19` | START_MODE | Written with the scene-id cell after the inner switch (value 1 for a new game). `GameLoop_Async` reads it. See [execution-model.md](execution-model.md#scene-runner-states-161-p1). | **blue-verified** (runtime) |
| `0x26` | WARP_LOCK | Cleared on a return from a dungeon. | **blue-verified** (runtime name) |
| `0x33` | DUNGEON_CLEAR_LIST | Bit list of cleared dungeons. Scene `9` sets bit `0x14` through `FUN_020208E8` [red-hypothesis: 9 = cleared]. | **blue-verified** (runtime name and write) |

**Storage.** Every variable above is a table-A id (below `0x53`), and its descriptor lies in the 0x400-byte global block. `SaveGlobalScriptVars` `0x0201F884` and `RestoreGlobalScriptVars` `0x0201F81C` copy that block raw, so the save holds all of them, including story flags such as `DUNGEON_CLEAR_LIST`, which are bit-typed entries in the same block. See [save.md](../formats/save.md#story-flags-and-scene-variables-362-p5). **blue-verified** (runtime and static).

**Reset.** `FUN_02021310` (ARM9 `0x02021310`) clears the block, applies per-descriptor defaults, then sets ids `0xE`–`0x20` explicitly. Seen at runtime at frames 52, 61, 2459 and 2562. **blue-verified**.

**Open (P5).**
- The base word for table A is `0x021EE640` in [save.md](../formats/save.md) and the block is `0x0210E640` in [execution-model.md](execution-model.md). One of them is probably a typo. This note does not resolve it and does not link them.
- Readers of `0x17`; the meaning of `0x10`.
- Vars `0x34`–`0x37` are read by `0x5E` and written by `0x5F` through ARM9 `0x020208E8`. Not traced beyond that.

## P6 — The inner phases `0x0`–`0x11` of scene-runner states 1 and 2

**Correction to [execution-model.md](execution-model.md).** The inner switch is not on `DAT_02070358`. In the decompile of `FUN_0206F7EC` the scene id is held in the cell `DAT_02070334` (set from the argument at line 28 and reassigned at line 93), and the switch reads that cell. `DAT_02070358` is a frame countdown: it is reset at entry (line 85), reset after the switch (line 221), and counted down while the state runs (lines 262–271), firing `ov_0000::0213A134` when it reaches zero.

| Claim | Evidence | Status |
|-------|----------|--------|
| The outer state is `DAT_0207035C`. It is set to 1 at entry (line 86). The state 1 and 2 path re-enters the inner switch when a script sets it back to 1 or 2. | `FUN_0206F7EC` lines 86, 115, 218 | **blue-verified** (static) |
| The inner switch reads the scene-id cell `DAT_02070334`, which holds the argument the runner was called with. Runtime: the runner was called with `0xF` and `0x10` and with `0`, matching the switch cases. | `FUN_0206F7EC` lines 28, 93; runtime from `scene_probe.py` and [execution-model.md](execution-model.md#runtime-repeat-and-forced-scene-ids-267) | **blue-verified** (static + runtime) |
| After the switch, the cell is set to 2, the state to 0 and the countdown reset. At every ground entry the cell is 2, the state 0 and the countdown 0 at runtime. | Lines 219–221 | **blue-verified** (static + runtime) |
| The entry passed to `ov_0000::0214D498` is a fixed value for each id. A `-1` entry means the default. The values are not recorded here. | Lines 34–59, 60–260 | **blue-verified** (static) |

**Per-id actions.**

| Phase | Action | Status |
|-------|--------|--------|
| `0x0` | `FUN_0206F738`, then cell = 1; default entry. Seen at New Game. | **blue-verified** (static + runtime) |
| `0x1` | Nothing; default entry. | **blue-verified** (static) |
| `0x2`, `0x3` | `FUN_02020020`; default entry. | **blue-verified** (static) |
| `0x4`, `0x6`, `0x7`, `0x8`, default | Debug assert only (empty in retail); default entry. | **blue-verified** (static) |
| `0x5` | A fixed entry different from the default. | **blue-verified** (static) |
| `0x9`–`0xC` (return from a dungeon) | Writes DUNGEON_RESULT; `WARP_LOCK` = 0; `FUN_0206F738`; `FUN_0201FDD8(0, 0x16, 1, 2)`. Reads DUNGEON_ENTER: −1 asserts; `0x51` reads the record of DUNGEON_ENTER_INDEX; otherwise the record of DUNGEON_ENTER via `FUN_02070890`. The entry is the record's +0xA short. Id `9` also sets the clear bit (P5). `FUN_0206F1C0` then picks the next GROUND_ENTER from the record and returns `0xD` if DUNGEON_ENTER is −1 or a flag fails. | **blue-verified** (static). Runtime: a dungeon return was not reached in 3000 frames, so this path is **lead** at runtime. |
| `0xD` | `FUN_0206F738`; cell = 1; START_MODE = 1; GROUND_GETOUT, GROUND_ENTER and GROUND_LINK = 0; a fixed entry. | **blue-verified** (static) |
| `0xE`, `0xF`, `0x10`, `0x11` | Fixed entries. The top switch (lines 34–59) sets the demo flag `DAT_02070338`. Runtime saw `0xF` and `0x10` on the title attract loop. "Demo or attract" **red-hypothesis**. | Entries **blue-verified** (static); runtime for `0xF`, `0x10` only |
| Demo cancel | Line 257 enters a fixed entry and calls `FUN_0206F368(0x1E)`. Seen at frame 2380 with no runner call logged. | **blue-verified** (static); runtime partial |

**After the switch.** If the scene is not a demo, the runner writes START_MODE from the cell. GROUND_GETOUT becomes the old GROUND_ENTER. GROUND_ENTER and GROUND_LINK are then set from the saved values and overridden by `FUN_0206F1C0`. **blue-verified** (static).

**Open (P6).** The `0x9`–`0xC` path was not reached at runtime; which scene ids `1`–`8` and `0xA`–`0xC` the game passes, and from which callers; whether `0xE`–`0x11` are demo or attract scenes beyond the title loop; the `0x02133CF0` address that [execution-model.md](execution-model.md) gives with the old switch cell is not re-checked here.

## Trace checks

Commands use the committed probes in `verification/v7-trace/`. `<scratch copy of pmdb.nds>` is a copy of the dump above; no committed probe checks its SHA-1. Output files stay outside the repo. Hashes and commands are in [verification.md](../verification.md#362--script-engine-probes).

| Script | Output hash | Command | Claims checked |
|--------|-------------|---------|----------------|
| `script_lookup_probe.py` | output SHA-1 `26990dcd…` (same in two runs) | `XDG_CONFIG_HOME=<fresh> SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy ~/venvs/desmume/bin/python -I verification/v7-trace/script_lookup_probe.py <scratch ROM copy> <OUT.jsonl> 3200 verification/v7-trace/boot_to_dungeon.txt verification/v7-trace` | P1 install and lookup, 10 of 10 |
| `interp_probe.py` | output SHA-1 `4301f2e7…` | `~/venvs/desmume/bin/python verification/v7-trace/interp_probe.py <scratch ROM copy> <OUT.jsonl> 3000 verification/v7-trace/boot_to_dungeon.txt verification/v7-trace` | P2 loop, dispatch and record counts; the frame-counter pairs |
| `op_probe.py` | output SHA-1 `673e0cb0…` | Same arguments as `interp_probe.py`, for `op_probe.py` | P3: the 36 opcodes of the opening run |
| `actor_probe.py` | output SHA-256 `06c5ddeb…` | `XDG_CONFIG_HOME=<fresh> ~/venvs/desmume/bin/python verification/v7-trace/actor_probe.py <scratch ROM copy> <OUT.jsonl> 3000 verification/v7-trace/boot_to_dungeon.txt verification/v7-trace` | P4: entity calls, lerp calls, camera counts |
| `scene_probe.py` | output SHA-256 `60e62055…` | `XDG_CONFIG_HOME=<fresh> ~/venvs/desmume/bin/python verification/v7-trace/scene_probe.py <scratch ROM copy> <OUT.jsonl> 3000 verification/v7-trace/boot_to_dungeon.txt verification/v7-trace` | P5 names and storage; P6 cell and entry |

Input `boot_to_dungeon.txt` has SHA-1 `b3f969cec1bb5aeb790c50e62727b5cdc5686d61`.

Static helpers used but not committed: `optable.py` and `dump_ov0.py` decode the B table and need a dump of overlay 0 bytes, which the repo must not hold. The B-table claims in P2 rest on that decode and on the export.

## Red comparison

Not done in this pass. The names in P3 and P4 are Blue's debug strings or are marked **red-hypothesis**. Comparison with `pmd-red` script code is open.

## Open questions

- P1: are bank overlays compressed; the unread sector and element fields.
- P2: meanings of return codes 1, 3 and 4; the six frame-counter outliers; input-wait opcodes; the stepper callers' roles.
- P2: the number of distinct handlers behind the 247 slots.
- P3: per-variant meanings of the movement, direction, camera and window families.
- P4: camera modes 1–3; vtable +0x3C and +0x40; the `0x2D` sub-modes; the renderer for the opening's text.
- P5: the `0x021EE640` vs `0x0210E640` base-word conflict; readers of `0x17`; meaning of `0x10`.
- P6: runtime for `0x9`–`0xC`; scene ids 1–8 and `0xA`–`0xC` and their callers.
- `0xF0`: does it end a script or park it (execution-model.md says ends).

## Links

- [format spec](../formats/scripts.md) (P1 layout)
- [save.md, story flags](../formats/save.md#story-flags-and-scene-variables-362-p5)
- [execution-model.md](execution-model.md) (overlays, scene runner)
- [text-engine.md](text-engine.md) (dialogue chain)
- [rng.md](rng.md) (opcode `0xB` is an operator; `0x6D`, `0xDC` and `0xC3` use the RNG)
- [ground-bpc.md](ground-bpc.md) (ground maps)
- [workflow.md](workflow.md), [verification.md](../verification.md)
