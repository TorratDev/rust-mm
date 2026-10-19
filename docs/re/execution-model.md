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

`FS_LoadOverlayImageAsync` (`0207BE40`) and `FS_StartOverlay` (`0207BC9C`) each have one caller per site above. The program has no other overlay loader. Overlays 8 and 9 therefore have no load path here. A load driven by data that never reaches these sites is not excluded.

## Start-up and the title menu

1. The boot code (`02000C84`–`02000D38`) calls `GameLoop_Async` (`0201F2E4`) at `02000D2C`. The routine ends at `02000D38` (`ldmia sp!,{r4,pc}`). `02000C90` is inside the routine, not an entry. The entry lies before `02000C84` and is not found yet. Its RNG seeding from the console MAC address is taken from [`rng.md`](rng.md) and was not rechecked here.
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

- **Per-frame wait.** `FUN_02075954(1, 1)` is called from the frame step `FUN_02019794` (call 8 of `MainLoops_RunFrameActions`, below). It clears bit 1 of an interrupt-flag word at `DAT_020759c8 + 0x3ff8`, then loops on `FUN_0207644c` until bit 1 is set again. The base `DAT_020759c8` is not resolved. Reading mask 1 as VBlank is an inference (medium confidence). Structure is **blue-verified**; the name is not given.
- **IME.** `FUN_02075b28`, `FUN_02075b5c`, `FUN_02075ba4` and `FUN_02075bdc` read and write `04000208` (IME). They look like enable and disable helpers. Their names are unconfirmed.
- **IRQ handler and vector: not located.** `04000180` is touched only by `FUN_02079d34`. `FUN_020748d0` writes REG_IF (`04000214`) at `02074974`, which is not established as the handler. `FUN_0207d704` reads REG_IF at `0207d88c`.

## A frame

Every wait loop calls `MainLoops_RunFrameActions(0)` (`0201F268`) once per frame. It makes 19 calls, in this order. The Blue addresses are verified; the names are `red-hypothesis`, from `pmd-red`'s `src/main_loops.c` (lines 273–297, where Red lists 15 calls and marks 4 extras in Blue).

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

Measured on the retail ROM (see [`verification.md`](../verification.md#p5--per-frame-rng-advance-44)): the global LCG (`sPRNGState`, `020DF704`) advances **4 steps per emulated frame** in the measured phase, with no button held. 117 of 122 frame intervals show exactly 4 steps. The others show 2, 8, 36 and 56. The state chain checks out with 0 mismatches. The frame routine `MainLoops_RunFrameActions` fires once per frame from emulated frame 2418 on.

- The sprite routine `FUN_02012648` made no calls in any interval, so it is not the source. It does call `Rand32Bit` at `020126F8`.
- The caller of the 4 steps is not identified. The next candidates are the callers of `Rand32Bit` (`0201198C`) on the frame path.
- The 2-step intervals may be hook artifacts from a double hit at frame 2419. Not resolved.
- A and Select: 7 A-held intervals gave 2, 4, 8 and 56 steps, too few and too mixed to show an effect. No frame-routine hits after emulated frame 2508, so Select has no data.
- Red's `WaitForNextFrameAndAdvanceRNG` advances the global RNG once per frame, and once more while A or Select is held.

**History.** The static note that replaced this section said that no call path from `MainLoops_RunFrameActions` reaches the global RNG within four call levels, and therefore that Blue probably does not advance the RNG per frame. The emulator result above contradicts that. The static search did not follow calls through function pointers, so the path it missed may go through one (not checked).

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
- Whether the 2-step frame intervals are real or a hook artifact of the double hit at frame 2419.
- What overlay 7's screen is. Its team-name strings suggest a team screen, but nothing links them to the title menu.
- The meaning of scene-runner phases 0–8 and of the variable `0x19` values.
- Which bank each ground scene uses.
- Whether modes 2 and 3 are reached in play.
- Names for the IME helpers and for `FUN_0201DE24`.
- Where the main loop of `RunDungeon_Async` advances a turn (the V7 trigger, #49).
