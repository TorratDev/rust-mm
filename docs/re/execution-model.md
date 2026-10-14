# Execution model and overlay loading

How Blue starts, runs a frame, and moves between the title menu, ground mode and dungeons, and which overlay is loaded in each. Issue: [#44](https://github.com/TorratDev/rust-mm/issues/44). Names, addresses and tags are in [`symbols.md`](../symbols.md); this page explains how they fit together.

Dump: `BlueRescueTeamEur`. The structure matches `pmd-red`'s `src/main_loops.c` closely: most functions below were named by matching their calls, in order and with the same constant arguments, against `GameLoop_Async`, `RunGameMode_Async` and `LoadAndRunQuickSaveDungeon_Async`. Not confirmed by an emulator trace yet.

## Overlays

Only three of the 145 ARM9 overlays hold code (V8 in [`verification.md`](../verification.md#v8--ghidra-loads-arm9-and-overlays-at-real-addresses)). All three load at `0x02138860`, so only one of them is resident at a time.

| Overlays | RAM | Contents | Loaded by |
|----------|-----|----------|-----------|
| 0 | `02138860` | Ground mode code (towns, menus, scripts) | `NDS_LoadOverlay_GroundMain`, at start-up and after every dungeon or overlay-7 mode |
| 1 | `02138860` | Dungeon code (RNG, combat, AI, floor generation, ...) | `LoadAndRunDungeon_Async` |
| 2–6 | `021B4900` | Text tables for the dungeon, one per language | `LoadAndRunDungeon_Async`, through `gLanguageTextOverlayIds` = {2, 4, 3, 5, 6} indexed by `GetLanguage()` |
| 7 | `02138860` | Code for title-menu choice 5, a mode Red does not have; entry `ov_0007::02139F80` | `RunOverlay7Mode` |
| 8, 9 | `0215D720` | 32 bytes each; no loader found yet | — |
| 10–144 | `0215D720` | Ground-mode "banks" (data) | `GroundStation_SetBank` (in overlay 0), through `gGroundBankTable`: 246 entries of {overlay id, pointer into it} |

Overlay loading uses the NitroSDK sequence `FS_LoadOverlayInfo`, `FS_InitFile`, `FS_LoadOverlayImageAsync`, `FS_WaitAsync`, `FS_CloseFile`, `FS_StartOverlay`, and `FS_UnloadOverlay` to drop one. These SDK names are recognised from behaviour (`sdk-hypothesis`).

## Start-up and the title menu

1. Boot code (`02000C90`–`02000D2C`, not yet inside a named function) seeds the global RNG from the console MAC address ([`rng.md`](rng.md)), runs more SDK set-up, and calls `GameLoop_Async`.
2. `GameLoop_Async` (`0201F2E4`) runs the same initialisation calls as Red, in the same order: heap, debug, sound counters, `NDS_LoadOverlay_GroundMain`, then the data loaders (`LoadMonsterParameters`, `LoadItemParameters`, `LoadWazaParameters`, `LoadMailInfo`, ...). Blue adds a 4 KiB `MemoryAlloc`/`MemoryFree` check before reading the save. Then `ReadSaveFromPak`, which also restores the global RNG state.
3. Language: if no language is set (`GetLanguage() == -1`), Blue reads a record with `02077EA4` and sets the language from its first byte, apparently the console's language setting. Red has no such step.
4. Title loop, repeated forever: fade the title palette in over 32 frames, run the menu once per frame, fade out, then act on the choice:

| Choice | Action | Same in Red |
|--------|--------|-------------|
| 2 | `RunGameMode_Async(1)`, or `RunGameMode_Async(2)` after deleting the first type-7 mail | yes |
| 3 | new game: two set-up calls, then `RunGameMode_Async(0)` | yes (`MENU_NEW_GAME`) |
| 4 | `RunGameMode_Async(3)` | yes |
| 5 | `RunOverlay7Mode` | **Blue only** |

## A frame

Every wait loop calls `MainLoops_RunFrameActions(0)` (`0201F268`) once per frame. It calls 19 functions; among the ones named so far is `TransferBGPaletteBuffer`, which copies changed 16-colour palette rows to palette RAM.

**RNG per frame:** Red's frame step calls `WaitForNextFrameAndAdvanceRNG`, which advances the global RNG every frame, and once more while A or Select is held. No call path from Blue's `MainLoops_RunFrameActions` reaches the global RNG functions within four call levels (static analysis; calls through function pointers are not followed). So Blue probably does not advance the RNG per frame. This needs an emulator check (#17). Sprite animations do use the global RNG (`02012648`), so the number of frames still matters.

## Into a dungeon and back

`RunGameMode_Async` calls `LoadAndRunQuickSaveDungeon_Async` (`0201E204`), which allocates the dungeon buffers (`MemoryAlloc(…, 7)` twice), reads the quicksave if the setup asks for it, then calls `LoadAndRunDungeon_Async` (`0201DB70`):

1. `NDS_UnloadOverlay_GroundMain` (overlay 0 out).
2. Load the language text overlay (2–6) and overlay 1.
3. `RunDungeon_Async` (`0206DC20`, in the ARM9 although it calls into overlay 1): the dungeon itself, including RNG seeding ([`rng.md`](rng.md)).
4. Unload overlay 1 and the text overlay, then `NDS_LoadOverlay_GroundMain` (overlay 0 back).

In Red, step 2 is a stub with a comment that the NDS version loads the dungeon overlay there.

## Open questions

- What title-menu choice 5 / overlay 7 is (candidates: a DS-only Wi-Fi or rescue feature).
- What uses overlays 8 and 9.
- The 19 calls of `MainLoops_RunFrameActions`: name them against Red's list (Red notes 2–3 extra sprite calls in Blue for the second screen).
- Where the main loop of `RunDungeon_Async` advances a turn (the V7 trigger, #49).
- Confirm the per-frame RNG behaviour by emulator.
