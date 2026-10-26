# Execution model and overlay loading

How Blue starts, runs a frame, and moves between the title menu, ground mode and dungeons, and which overlay is loaded in each. Issue: [#44](https://github.com/TorratDev/rust-mm/issues/44). Names, addresses and tags are in [`symbols.md`](../symbols.md); this page explains how they fit together.

Dump: `BlueRescueTeamEur`. The structure matches `pmd-red`'s `src/main_loops.c` closely: most functions below were named by matching their calls, in order and with the same constant arguments, against `GameLoop_Async`, `RunGameMode_Async` and `LoadAndRunQuickSaveDungeon_Async`. The static findings were read from Ghidra (copy `rescue-arm9-a`). One emulator check was run for the per-frame RNG ([`verification.md`](../verification.md#p5--per-frame-rng-advance-44)). Tags: **blue-verified** (read in Blue's code or measured), **red-hypothesis** (taken from `pmd-red`), **sdk-hypothesis** (NitroSDK function recognised by its behaviour), **tentative** (our own name, role partly known).

## Overlays

Only three of the 145 ARM9 overlays hold code (V8 in [`verification.md`](../verification.md#v8--ghidra-loads-arm9-and-overlays-at-real-addresses)). All three load at `0x02138860`, so only one of them is resident at a time.

| Overlays | RAM | Contents | Loaded by |
|----------|-----|----------|-----------|
| 0 | `02138860` | Ground mode code (towns, menus, scripts) | `NDS_LoadOverlay_GroundMain`, at start-up and after every dungeon or overlay-7 mode |
| 1 | `02138860` | Dungeon code (RNG, combat, AI, floor generation, ...) | `LoadAndRunDungeon_Async` |
| 2–6 | `021B4900` | Text tables for the dungeon, one per language | `LoadAndRunDungeon_Async`, through `gLanguageTextOverlayIds` = {2, 4, 3, 5, 6} indexed by `GetLanguage()` (the table is at `020DF90C`) |
| 7 | `02138860` | Code for title-menu choice 5, a mode Red does not have; entry `ov_0007::02139F80` | `RunOverlay7Mode` (`0201DAC4`). Its screen is not seen, so the entry is not named |
| 8, 9 | `0215D720` | 32 bytes each, no functions; no loader in this dump | — (see the loader inventory) |
| 10–144 | `0215D720` | Ground-mode "banks" (data) | `GroundStation_LoadBank`, through `gGroundBankTable` (`020DB8E0`): 246 entries of 8 bytes, {overlay id, pointer into it}. The ids run 10–144 (135 distinct); 8 and 9 never occur |

Overlay loading uses the NitroSDK sequence `FS_LoadOverlayInfo`, `FS_InitFile`, `FS_LoadOverlayImageAsync`, `FS_WaitAsync`, `FS_CloseFile`, `FS_StartOverlay`, and `FS_UnloadOverlay` to drop one. These SDK names are recognised from behaviour (`sdk-hypothesis`).

### Loader inventory

`FS_LoadOverlayInfo` (`0207BED8`) has six call sites in the program. Each one's overlay id is listed:

| Call site | Overlay id |
|-----------|-----------|
| `NDS_LoadOverlay_GroundMain` (`0201DCEC`) | `0` (literal at `0201DD58`) |
| `RunOverlay7Mode` (`0201DAC4`) | `7` (literal at `0201DB64`) |
| `LoadAndRunDungeon_Async` (`0201DB70`), first call | from the language table `{2, 4, 3, 5, 6}` at `020DF90C` |
| `LoadAndRunDungeon_Async`, second call | `1` (literal at `0201DC9C`) |
| `GroundStation_LoadBank` (`ov_0000::0214FD00`) | `gGroundBankTable[bank]` |
| `FS_UnloadOverlay` (`0207BB28`) | its own `id` argument |

`FS_LoadOverlayImageAsync` (`0207BE40`) and `FS_StartOverlay` (`0207BC9C`) each have five callers, the load sites above other than `FS_UnloadOverlay`. The program has no other overlay loader. Overlays 8 and 9 therefore have no load path here. A load driven by data that never reaches these sites is not excluded.

## Start-up and the title menu

1. The boot code is inside the routine that starts at `0x02000B88` and ends at `02000D38` (`ldmia sp!,{r4,pc}`). The routine starts with `stmdb sp!,{r4,lr}` at `0x02000B88`, and `_start` reaches it with `bx r1` at `0x020008F8`, using the literal `0x02000918` = `0x02000B88` (#158 P1; blue-verified for the entry, the literal and the runtime hits, and the chain). The routine calls `GameLoop_Async` (`0201F2E4`) at `02000D2C`, which runs once at boot (runtime). The proposed name is `ARM9Main` (or `BootMain`). It is our name, not from the repo or from pmd-red, so the tag is tentative. `02000C90` is inside the routine, not an entry. `0x02000B4C` is a data table (the module parameter block that `_start` passes to `0x02000934`), not code. **Not verified:** the name (no source to compare with); the Ghidra function boundary at `0x02000B88` (no function is defined there, so the disassembly is the only view); the calls at `0x02000B90` to `0x02000C7C`, which are not named. The seeding at `02000CB8` and `02000CC4` is in [`rng.md`](rng.md#global-lcg-blue-verified).
2. `GameLoop_Async` (`0201F2E4`) runs the same initialisation calls as Red, in the same order: heap, debug, sound counters, `NDS_LoadOverlay_GroundMain`, then the data loaders (`LoadMonsterParameters`, `LoadItemParameters`, `LoadWazaParameters`, `LoadMailInfo`, ...). Blue adds a 4 KiB `MemoryAlloc`/`MemoryFree` check before reading the save. Then `ReadSaveFromPak`, which also restores the global RNG state.
3. Language: if no language is set (`GetLanguage() == -1`), Blue reads a record with `02077EA4` and sets the language from its first byte, apparently the console's language setting. Red has no such step.
4. Title loop, repeated forever: fade the title palette in over 32 frames, run the menu once per frame, fade out, then act on the choice:

| Choice | Action | Same in Red |
|--------|--------|-------------|
| 2 | `RunGameMode_Async(1)`, or `RunGameMode_Async(2)` after deleting the first type-7 mail | yes |
| 3 | new game: two set-up calls, then `RunGameMode_Async(0)` | yes (`MENU_NEW_GAME`) |
| 4 | `RunGameMode_Async(3)` | yes |
| 5 | `RunOverlay7Mode` | **Blue only** |

## Frame wait and interrupts

- **Per-frame wait.** `FUN_02075954(clear, mask)` (#157 P3) clears the bits of `mask` in the flag word when `clear != 0`, then loops on `FUN_0207644c` until `mask & flag` is nonzero. It is called as `FUN_02075954(1, 1)` from the frame step `FUN_02019794` (call 8 of `MainLoops_RunFrameActions`, below). The flag word is `0x027E3FF8`: the base `DAT_020759c8` is `0x027E0000` and the offset `DAT_020759cc` is `0x3FF8`. **Correction:** mask 1 is bit 0 (the value passed is 1), not bit 1. Bit 0 is VBlank: the VBlank handler `FUN_02018ACC` sets `|= 1` on the same flag word (`DAT_02018B20` = `0x027E0000`), and it is the handler registered for IRQ 0 (see the vector bullet below). Blue-verified (literal values, the handler's write, the registration). The earlier note "bit 1" was wrong. **Not verified:** the meaning of the flag word as a software copy of the IRQ flags (inferred from the bit alignment, not from a read of REG_IF at the same time); `FUN_0207644c` (the idle call in the wait loop) was not decoded.
- **IME helpers (proposed names, not in `docs/symbols.md`).** `FUN_02075b28`, `FUN_02075b5c`, `FUN_02075ba4` and `FUN_02075bdc` each save `REG_IME` (`04000208`), clear it, change one register, and restore it, so they run with interrupts masked (#157 P2). Blue-verified for the behaviour. The registers are read from the literal pool: `04000214` (REG_IF) and `04000210` (REG_IE). Proposed names, awaiting orchestrator confirmation, based on NitroSDK convention (not verified in this repo): `02075B28(v)` writes `REG_IF = v` and returns the old `REG_IF` (`OS_ResetRequestIrqMask`); `02075B5C(v)` clears the bits of `v` in `REG_IE` and returns nothing (`OS_DisableIrqMask`); `02075BA4(v)` sets the bits of `v` in `REG_IE` and returns the old value (`OS_EnableIrqMask`); `02075BDC(v)` writes `REG_IE = v` and returns the old value (`OS_SetIrqMask`). **Not verified:** that these are the SDK names for this game's build (only the behaviour is established); whether the void form of `02075B5C` returns anything in other callers.
- **Vector, dispatcher and handler table (#157 P1, blue-verified).** `_start` (`0x020008C0` to `0x020008E4`) writes the dispatcher address `0x01FFA21C` (`FUN_01ffa21c`, from the literal at `0x02000914`) into the vector slot `0x027E3FFC` (`str r0,[r1]` at `0x020008E0`). The slot is computed from the literal `0x027E0000` plus `0x3FC0 + 0x3C`. The dispatcher sits in ITCM at `0x01FFA21C`. It returns if `REG_IME == 0`; it takes `pending = REG_IE & REG_IF`; for each set bit, highest first (`LZCOUNT`), it writes `REG_IF = 0x80000000 >> lz` (acknowledge) and calls the handler at `0x027E0008 + 4*bit` (the table literal is `0x01FFA26C` = `0x027E0008`). So the table index is the IRQ bit number. `FUN_02075C5C(mask, handler)` registers handlers: for bits 0 to 2, 7 and 12 to 21 it stores the handler at `0x027E0008 + 4*bit`, the entries the dispatcher reads. For bits 3 to 6 and 8 to 11 it writes a 12-byte record `{handler, 1, 0}` at `0x02135324`, which the dispatcher does not read. VBlank is `FUN_02075C5C(1, FUN_02018ACC)`, so IRQ 0 is VBlank. Eight call sites of `FUN_02075C5C` exist (`FUN_0207ED78`, `FUN_02079D34`, `_start` at `0x02000C30`, `FUN_020749A4`, `FUN_02074A64` (two sites), `FUN_02077C68`, `FUN_02074C00`); which bit each registers was not decoded. The only reader of the table is `FUN_01ffa21c`. Medium confidence for the 12-byte records (writer only).
  - **Discrepancy with the issue:** the issue gives the vector as `03FFFFFC`. The code writes `0x027E3FFC`. No reference to `0x03FFFFFC` exists in the ARM9 binary.
  - **Not verified:** which IRQ bit each of the other seven `FUN_02075C5C` callers registers; the reader of the 12-byte records at `0x02135324`; whether `0x027E3FFC` is the same vector as the `0x03FFFFFC` the issue names; the meaning of the write to `0x027FFF9C` at `0x020008CC`; a search for other writes to `0x027E3FFC` (the `memory search` for `FCFFFF03` hung and was stopped).

## A frame

Every wait loop calls `MainLoops_RunFrameActions(0)` (`0201F268`) once per frame. It makes 19 calls, in this order. The Blue addresses are verified; the names are `red-hypothesis`, from `pmd-red`'s `src/main_loops.c` (lines 273–297, where Red lists 15 calls and marks 4 extras in Blue). Input, touch, screen and sprite steps of this frame are in [`io.md`](io.md).

| # | Blue call | Red name | Notes |
|---|-----------|----------|-------|
| 1 | `FUN_0202e5a0` | `DrawDialogueBoxString_Async` | order only; a state machine |
| 2 | `FUN_020163b8(0, 0)` | `CopyWindowBgBuffer` | argument pattern matches Red's `(NULL, WIN0)` |
| 3 | `FUN_02012310(touch)` | `nullsub_8` | body is `return;` |
| 4 | `FUN_02012bcc` | `sub_8005180` | linked-list compaction; order only |
| 5 | `FUN_02019b74` | Blue extra | same body as #4 on another list (verified duplicate) |
| 6 | `FUN_02016320` | `ToggleWindowBgBuffer` | toggles flags; order only |
| 7 | `FUN_02029bbc` | `UpdateSoundEffectCounters` | decrements three counters while above 0 |
| 8 | `FUN_02019794` | frame wait | calls `FUN_02075954(1, 1)` (above); no RNG call |
| 9 | `FUN_02017e60` | `LoadBufferedInputs` | order only; low confidence |
| 10 | `FUN_02012b34` | `CopySpritesToOam` | linked list to OAM |
| 11 | `FUN_02012998` | `sub_8005304` | loop calling `FUN_01ff9fb0` / `FUN_01ff9fc4` |
| 12 | `FUN_02019adc` | Blue extra | copy of #10 (verified duplicate) |
| 13 | `FUN_02019a08` | Blue extra | copy of #11, identical body (verified duplicate) |
| 14 | `TransferBGPaletteBuffer` | `TransferBGPaletteBuffer` | already named; copies changed 16-colour palette rows to palette RAM |
| 15 | `FUN_01ff86b8` | `xxx_call_update_bg_vram` | order only; low confidence |
| 16 | `FUN_0201a974` | `DoScheduledMemCopies` | loops over 12-byte entries calling `FUN_01ff9fc4` |
| 17 | `FUN_02019268` | `xxx_call_update_bg_sound_input` | writes registers with masks; low confidence |
| 18 | `FUN_02013078(0)` | `ResetSprites` | clears a count, OAM base `06400000`, loops over sprite entries |
| 19 | `FUN_02019f40(0)` | Blue extra | same shape as #18 with OAM base `06600000` (second screen); verified duplicate |

Red's copies of the second-screen calls pair up as (#4, #5), (#10, #12), (#11, #13) and (#18, #19). The names of #1, #2, #3, #4, #6, #7, #10, #11, #16 and #18 are in [`symbols.md`](../symbols.md). The low-confidence ones (#9, #15, #17) stay as their `FUN_` names.

## RNG per frame

Measured on the retail ROM (see [#159 in `rng.md`](rng.md#per-frame-steps-159)): the global LCG advances **4 steps per emulated frame** in the measured phase, with no button held. The boot-only probe and the dungeon-windows probe each ran twice with identical output. Of the 121 intervals between consecutive frame-routine hits (frames 2418 to 2561, boot run), 116 are one frame apart with 4 steps. The others are 2 steps (frames 2531 to 2533), 8 steps (2472 to 2474), 40 steps (2448 to 2458) and 52 steps (2459 to 2472). The frame routine `MainLoops_RunFrameActions` fires once per frame in that range.

- The sprite routine `FUN_02012648` calls `Rand32Bit` at `020126F8`. It is one of the other callers in the 42 four-step frames with the flag clear (see [#159](rng.md#per-frame-steps-159)).
- `FUN_02018ACC` runs once per frame and makes 4 steps when its flag test passes, and 2 when it fails (#159 P2). The callback slot that calls it is not identified.
- The 2-step intervals are real frames where the flag test fails, not a hook artifact of the double hit at frame 2419 (#159 P3). Frames 2532 and 2533 advanced 2 steps, with only the unconditional call.
- A and Select: the A-held and Select-held windows were run in the dungeon, where the frame routine does not run. A held changes the step count through the flag test, and its 2-step frames appear only in that window. Select gave no 2-step frames in one run, and its effect on the step count is not established. A and Select on the frame routine need a boot-phase run (frames 2418 to 2561).
- Red's `WaitForNextFrameAndAdvanceRNG` advances the global RNG once per frame, and once more while A or Select is held.

**History.** The earlier static note in this section said that no call path from `MainLoops_RunFrameActions` reaches the global RNG within four call levels, and therefore that Blue probably does not advance the RNG per frame. The emulator result above contradicts that. The static search did not follow calls through function pointers, so the path it missed may go through one (not checked).

## Scene graph

`RunGameMode_Async` (`0201E3C8`) is called by `GameLoop_Async` (callers at `0201F7A4`, `0201F7B4`, `0201F7C4` and `0201F7DC`) with four modes. Blue's `GameLoop_Async` `switch` (per decompile) reads them as follows. Mode 0 is a new game, mode 1 a continue, mode 2 a continue after deleting the first type-7 mail, mode 3 title choice 4. Case 5 calls `RunOverlay7Mode`.

- **How the mode is used.** The mode is read only when variable `0x19` (read with `FUN_02020F74(0, 0x19)`) equals 7. Mode 2 then sets state 8. Mode 3 sets state `0xB`, writes `0xB` back to variable `0x19`, and calls `FUN_0200E3FC` and `FUN_0201ED4C(3)`. Otherwise a variable value of 0 or `0xB` is kept, and any other value becomes state 1. Modes 0 and 1 are equivalent on this path. The meaning of the variable's values is inferred.
- **Scene runner `FUN_0206F7EC`** (ARM9-resident, `0206F7EC`) takes a scene id and stores it at `DAT_02070334`. Its phase (`DAT_0207035C`) takes values 0–8 and returns codes 5–10 (habitat, dungeon, rescue scenario, random, user, training) or `0xE`, `0xF` and `0x10` (exit, demo end). The phase names come from debug strings. The runner calls overlay-0 functions, so overlay 0 must be resident while it runs. It is reached by a computed jump through `thunk_FUN_0201DCAC`; the callers are `GameLoop_Async` and `RunGameMode_Async` (per decompile; the x-ref tool did not list them). Check that overlay 0 is loaded for scene ids 0–C.
- **Scene transition.** State `0xC` of `RunGameMode_Async` calls `FUN_0201DE24`, which calls `RandInt(100)` 20 times. So the transition consumes global RNG (decompile; the RNG source is inferred from the name `RandInt`).
- **Ground banks.** `GroundStation_SetBank` (`ov_0000::0214FC50`) loads or unloads `gGroundBankTable[bank]` through `GroundStation_LoadBank` (`ov_0000::0214FD00`). Its sole caller is `FUN_ov_0000__0214D510` (`ov_0000::0214D510`), which is called from `ov_0000::0214D420`, `ov_0000::021421CC` and a computed jump at `ov_0000::0214D4FC`. The table was read in Ghidra: 246 entries, overlay ids 10–144. No dump is committed. Which bank each ground scene uses is not mapped.

## Into a dungeon and back

`RunGameMode_Async` calls `LoadAndRunQuickSaveDungeon_Async` (`0201E204`), which allocates the dungeon buffers (`MemoryAlloc(…, 7)` twice), reads the quicksave if the setup asks for it, then calls `LoadAndRunDungeon_Async` (`0201DB70`):

1. `NDS_UnloadOverlay_GroundMain` (overlay 0 out).
2. Load the language text overlay (2–6) and overlay 1.
3. `RunDungeon_Async` (`0206DC20`, in the ARM9 although it calls into overlay 1): the dungeon itself, including RNG seeding ([`rng.md`](rng.md)).
4. Unload overlay 1 and the text overlay, then `NDS_LoadOverlay_GroundMain` (overlay 0 back).

In Red, step 2 is a stub with a comment that the NDS version loads the dungeon overlay there.

## Open questions

- The IRQ handler and its vector: not located (see [Frame wait and interrupts](#frame-wait-and-interrupts)).
- The entry of the boot routine, before `02000C84`.
- The caller of the 4 RNG steps per frame, and the A and Select effect (Select has no data after emulated frame 2508).
- The caller slot of `FUN_02018ACC` and the writer of its flag word (see [#159](rng.md#per-frame-steps-159)).
- What overlay 7's screen is. Its team-name strings suggest a team screen, but nothing links them to the title menu.
- The meaning of scene-runner phases 0–8 and of the variable `0x19` values.
- Which bank each ground scene uses.
- Whether modes 2 and 3 are reached in play.
- Names for the IME helpers and for `FUN_0201DE24`.
- Where the main loop of `RunDungeon_Async` advances a turn (the V7 trigger, #49).
