# Execution model and overlay loading

How Blue starts, runs a frame, and moves between the title menu, ground mode and dungeons, and which overlay is loaded in each. Issue: [#44](https://github.com/TorratDev/rust-mm/issues/44). Names, addresses and tags are in [`symbols.md`](../symbols.md); this page explains how they fit together.

Dump: `BlueRescueTeamEur`. The structure matches `pmd-red`'s `src/main_loops.c` closely: most functions below were named by matching their calls, in order and with the same constant arguments, against `GameLoop_Async`, `RunGameMode_Async` and `LoadAndRunQuickSaveDungeon_Async`. The static findings were read from Ghidra (copy `rescue-arm9-a`). One emulator check was run for the per-frame RNG ([`verification.md`](../verification.md#p5--per-frame-rng-advance-44)). Tags: **blue-verified** (read in Blue's code or measured), **red-hypothesis** (taken from `pmd-red`), **sdk-hypothesis** (NitroSDK function recognised by its behaviour), **tentative** (our own name, role partly known).

## Overlays

Only three of the 145 ARM9 overlays hold code (V8 in [`verification.md`](../verification.md#v8--ghidra-loads-arm9-and-overlays-at-real-addresses)). All three load at `0x02138860`, so only one of them is resident at a time.

| Overlays | RAM | Contents | Loaded by |
|----------|-----|----------|-----------|
| 0 | `02138860` | Ground mode code (towns, menus, scripts) | `NDS_LoadOverlay_GroundMain` (`0201DCEC`). Its three callers, the only ones of the overlay-0 loader: `GameLoop_Async` (`0201F2E4`, call at `0201F2FC`, start-up), `LoadAndRunDungeon_Async` (`0201DB70`, after every dungeon) and `RunOverlay7Mode` (`0201DAC4`, after overlay 7). Tag: **blue-verified** (static). |
| 1 | `02138860` | Dungeon code (RNG, combat, AI, floor generation, ...) | `LoadAndRunDungeon_Async` |
| 2–6 | `021B4900` | Text tables for the dungeon, one per language | `LoadAndRunDungeon_Async`, through `gLanguageTextOverlayIds` = {2, 4, 3, 5, 6} indexed by `GetLanguage()` (the table is at `020DF90C`) |
| 7 | `02138860` | Code for title-menu choice 5, a mode Red does not have; entry `ov_0007::02139F80` (blue-verified). Its screen was seen only under fault injection: a species sprite grid with the message "Please close the DS system. To end this adventure, press B" ([details](#overlay-7-the-choice-5-screen-161-p3)) | `RunOverlay7Mode` (`0201DAC4`), called from `GameLoop_Async` case 5 |
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
| 5 | `RunOverlay7Mode` | **Blue only** (the route is static; the real title path is open, see below) |

### Overlay 7: the choice-5 screen (#161 P3)

- **Route (static, blue-verified).** `GameLoop_Async` (`0x0201F2E4`) calls `FUN_020628BC` (`0x0201F69C`) and stores the result in `r8` (`0x0201F6A0`). The loop breaks on 2, 3, 4 or 5. Its `switch` has `case 5: RunOverlay7Mode()`. `RunOverlay7Mode` (`0x0201DAC4`) is called only from `0x0201F7E8`. It loads overlay 7 (the literal at `0x0201DB64`), calls `ov_0007::02139F80`, unloads overlay 7 and reloads overlay 0.
- **Who returns 5 (static, decompile).** Inside `FUN_020628BC`, the value 5 comes from state `0xF`, which runs when `FUN_0206978C()` returns 3. The other states return 1 to 4. Whether the normal title path (with a save) reaches state `0xF` is **[open]**.
- **The entry (decompile; blue-verified up to the strings).** It reads the language, allocates and clears its state, and loads a palette file (`palet…`) and a sprite file (`ax_03d…`). The species comes from `FUN_02002F88`, which `structs.md` names `FindFirstActiveRosterRecord`; with no roster record the default index is 1. It fades in, runs a 32-step loop (`FUN_ov_0007__021394B0`, `FUN_ov_0007__021393B0`), calls `FUN_ov_0007__02139A98`, fades out, frees its memory and returns. Strings: `palet…`, `ax_03d…`, `sprite file error`. The sub-functions beyond the entry are not decompiled. The names of the sprite and palette files are abbreviated here because the findings give them that way.
- **What it shows (fault injection, screenshot).** With the title result forced to 5 (see the #161 P3 entry in [`verification.md`](../verification.md)): a grid of about 100 species sprites on a blue background, one pink sprite in the lower centre, and a message box with a DS icon: "Please close the DS system. To end this adventure, press B". Two frames of one run differed in pixels; the change was not analysed.
- **Buttons (fault injection).** A has no visible effect over 500 frames (investigator B's run). B leaves overlay 7 about 120 frames after entry. The title function then runs again (the title return hook counts 1 → 8), and the screen shows the title logo and story text. The New Game / Adventure Log menu was not captured in our repeat run up to frame 4500. Investigator B's run showed that menu at frame 4200 with a different input file; the difference is **[open]**.
- **Meaning.** **[hypothesis]**: an end-of-adventure screen ("To end this adventure"), with the sprite grid as its background. It is not linked to a save, and the real title route to choice 5 is not tested.

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
| 9 | `FUN_02017e60` | `LoadBufferedInputs` (recorded name, red-hypothesis) | copies the buffered input and clears four fields (Red's steps); the touch decode is Blue-only |
| 10 | `FUN_02012b34` | `CopySpritesToOam` | linked list to OAM |
| 11 | `FUN_02012998` | `sub_8005304` | loop calling `FUN_01ff9fb0` / `FUN_01ff9fc4` |
| 12 | `FUN_02019adc` | Blue extra | copy of #10 (verified duplicate) |
| 13 | `FUN_02019a08` | Blue extra | copy of #11, identical body (verified duplicate) |
| 14 | `TransferBGPaletteBuffer` | `TransferBGPaletteBuffer` | already named; copies changed 16-colour palette rows to palette RAM |
| 15 | `FUN_01ff86b8` | `xxx_call_update_bg_vram` (recorded name, medium) | flushes scheduled copies to VRAM-side buffers, gated by the flag at `+0x45` |
| 16 | `FUN_0201a974` | `DoScheduledMemCopies` | loops over 12-byte entries calling `FUN_01ff9fc4` |
| 17 | `FUN_02019268` | `FlushDisplayState` (recorded name; Red's `xxx_call_update_bg_sound_input` is not supported) | writes the display registers (see [`io.md`](io.md#per-frame-flush)) |
| 18 | `FUN_02013078(0)` | `ResetSprites` | clears a count, OAM base `06400000`, loops over sprite entries |
| 19 | `FUN_02019f40(0)` | Blue extra | same shape as #18 with OAM base `06600000` (second screen); verified duplicate |

Red's copies of the second-screen calls pair up as (#4, #5), (#10, #12), (#11, #13) and (#18, #19). The names of #1, #2, #3, #4, #6, #7, #10, #11, #16 and #18 are in [`symbols.md`](../symbols.md). #9, #15 and #17 are named in [#161 P4](#frame-action-names-161-p4); the names are recorded there, and the table above keeps the `FUN_` names.

## RNG per frame

Measured on the retail ROM (see [#159 in `rng.md`](rng.md#per-frame-steps-159)): the global LCG advances **4 steps per emulated frame** in the measured phase, with no button held. The boot-only probe and the dungeon-windows probe each ran twice with identical output. Of the 121 intervals between consecutive frame-routine hits (frames 2418 to 2561, boot run), 116 are one frame apart with 4 steps. The others are 2 steps (frames 2531 to 2533), 8 steps (2472 to 2474), 40 steps (2448 to 2458) and 52 steps (2459 to 2472). The frame routine `MainLoops_RunFrameActions` fires once per frame in that range.

- The sprite routine `FUN_02012648` calls `Rand32Bit` at `020126F8`. It is one of the other callers in the 42 four-step frames with the flag clear (see [#159](rng.md#per-frame-steps-159)).
- `FUN_02018ACC` runs once per frame and makes 4 steps when its flag test passes, and 2 when it fails (#159 P2). It is the VBlank IRQ handler, registered in IRQ slot 0 by `FUN_02075C5C(1, FUN_02018ACC)` (see [Frame wait and interrupts](#frame-wait-and-interrupts) and [`io.md`](io.md#frame-path)); the dispatcher `FUN_01FFA21C` calls it.
- The 2-step intervals are real frames where the flag test fails, not a hook artifact of the double hit at frame 2419 (#159 P3). Frames 2532 and 2533 advanced 2 steps, with only the unconditional call.
- A and Select: the A-held and Select-held windows were run in the dungeon, where the frame routine does not run. A held changes the step count through the flag test, and its 2-step frames appear only in that window. Select gave no 2-step frames in one run, and its effect on the step count is not established. A and Select on the frame routine need a boot-phase run (frames 2418 to 2561).
- Red's `WaitForNextFrameAndAdvanceRNG` advances the global RNG once per frame, and once more while A or Select is held.

**History.** The earlier static note in this section said that no call path from `MainLoops_RunFrameActions` reaches the global RNG within four call levels, and therefore that Blue probably does not advance the RNG per frame. The emulator result above contradicts that. The static search did not follow calls through function pointers, so the path it missed may go through one (not checked).

## Scene graph

`RunGameMode_Async` (`0201E3C8`) is called by `GameLoop_Async` (callers at `0201F7A4`, `0201F7B4`, `0201F7C4` and `0201F7DC`) with four modes. Blue's `GameLoop_Async` `switch` (per decompile) reads them as follows. Mode 0 is a new game, mode 1 a continue, mode 2 a continue after deleting the first type-7 mail, mode 3 title choice 4. Case 5 calls `RunOverlay7Mode`.

- **How the mode is used.** The mode is read only when variable `0x19` (read with `FUN_02020F74(0, 0x19)`) equals 7. Mode 2 then sets state 8. Mode 3 sets state `0xB`, writes `0xB` back to variable `0x19`, and calls `FUN_0200E3FC` and `FUN_0201ED4C(3)`. Otherwise a variable value of 0 or `0xB` is kept, and any other value becomes state 1. Modes 0 and 1 are equivalent on this path. The meaning of the variable's values is inferred.
- **Scene runner `FUN_0206F7EC`** (ARM9-resident, `0206F7EC`) takes a scene id and stores it at `DAT_02070334`. Its phase (`DAT_0207035C`) takes values 0–8 and returns codes 5–10 (habitat, dungeon, rescue scenario, random, user, training) or `0xE`, `0xF` and `0x10` (exit, demo end). The phase names and the variable values are in [Scene runner states](#scene-runner-states-161-p1). The runner calls overlay-0 functions, so overlay 0 must be resident while it runs. It is reached by a computed jump through `thunk_FUN_0201DCAC`; the callers are `GameLoop_Async` and `RunGameMode_Async` (per decompile; the x-ref tool did not list them). Overlay 0 residency for each scene id is in **Overlay 0 residency at the scene runner** (under the ground chain, below).
- **Item and money loss (renamed; not a transition, #161 P2).** State `0xC` of `RunGameMode_Async` (`0201E6F4`) calls `FUN_0201DE24`, which does three things. (1) For each of the 20 team inventory slots, if `RandInt(100) < 50`, it zeroes the 3-byte item (`gTeamInventoryRef`, `0x020DECF8`). (2) `FUN_02007B68` moves the non-empty items to the front and zeroes the rest. (3) For each roster record (`0x40` bytes, roster pointer `0x020DDF84`, count 413): if `flags & 1` and `flags & 2`, it clears the byte at `+0x1C` (the held item's id); then `teamMoney` (`+0x234`) is set to 0. Proposed name: `RemoveMoneyAndRandomItems` (Red's name, applied to Blue; red-hypothesis). `FUN_02007B68` is `FillInventoryGaps` by behaviour. The three steps and their order match Red's `RemoveMoneyAndRandomItems` (`src/main_loops.c` 1041–1059). **Tag:** blue-verified (structure, offsets, call site); red-hypothesis for the name and the mode mapping. **Not verified:** which game event reaches state `0xC` in Blue (the mode mapping; Red's `STARTMODE_DUNGEON_LOST` value was not found); whether roster flag bits 1 and 2 are "exists" and "on team" in Blue; the roster `+0x1C` width (only the low byte, the id, is cleared, while Red clears the whole `heldItem.id`).
- **Ground banks.** `GroundStation_SetBank` (`ov_0000::0214FC50`) loads or unloads `gGroundBankTable[bank]` through `GroundStation_LoadBank` (`ov_0000::0214FD00`). Its sole caller is `FUN_ov_0000__0214D510` (`ov_0000::0214D510`), which is called from `ov_0000::0214D420`, `ov_0000::021421CC` and a computed jump at `ov_0000::0214D4FC`. The table was read in Ghidra: 246 entries, overlay ids 10–144. No dump is committed. The chain from a scene to a bank is in [Ground chain](#ground-chain-and-overlay-0-160). Which bank each ground scene uses is not mapped; the scene table stays out until the maintainer approves it.

### Scene runner states (#161 P1)

Investigator B's findings. Tag: **blue-verified** (code and debug strings read from the Ghidra copy). Confidence: high for the state to string to return mapping and for the variable `0x19` writes; medium for the meaning of the scene variables `0x10`, `0x12`, `0x14`, `0x15` and `0x17` (their readers are not traced).

The scene runner is `FUN_0206F7EC`. Its state is `DAT_0207035C` (`0x02133CE0`). Its final `switch` has cases 0 to 8, and each case prints a debug string and returns a code.

| State | Debug string | Return code | Scene variables written |
|-------|--------------|-------------|-------------------------|
| 0 (also 1, 2 and default) | `GAME END` | `0xE` | `0x19 = 1`, then `FUN_02029B60(0x10)` |
| 3 | `select HABITAT` | 5 | `0x14 = -1`, `0x15 = -1`, `0x19 = 4`, `0x17 = 4`, `0x10 = var 0xE` |
| 4 | `select DUNGEON` | 6 | `0x14 = 0` then `-1`, `0x19 = 5`, `0x17 = 5` |
| 5 | `enter RESCUE SCENARIO %d %d` (plus `name %s` and `%s`) | 7 | `0x14 = id`, `0x15 = FUN_02070854(id)`, `FUN_020208E8(0, 0x32, id, 1)`, `0x19 = 7`, `0x17 = 7`, `0x12` reset to 0 when it is 10 |
| 6 | `enter RESCUE RANDOM %d` | 8 | `0x14 = 0x51`, `0x15 = id`, `0x19 = 7`, `0x17 = 7`, `0x12` reset to 0 when it is 10 |
| 7 | `enter RESCUE USER %d` | 9 | `0x14 = 0x50`, `0x15 = *PTR_DAT_020703A8`, `0x19 = 7`, `0x17 = 7` |
| 8 | `enter TRAINING %d` | 10 | `0x14 = 0x52`, `0x15 = id`, `0x19 = 7`, `0x17 = 7` |

- The demo flag `DAT_02070338` (`0x02133CD0`) changes the exit: state 10 with the flag set returns `0x10` (`DEMO CANCEL`), and state 9 returns `0xF` (`DEMO END`). Tag: medium, from the decompile output (it tests `(state != 9) && (state == 10)`), not the disassembly.
- The scene id is stored at `*DAT_02070334` (`0x02133CEC`).
- **Variable `0x19`.** `1` is the exit (`GAME END`), with inner phase `0xD`. `4` is habitat, `5` dungeon, and `7` the rescue scenario, random, user and training, which is the value `GameLoop_Async` reads and compares with 7 in `RunGameMode_Async`. During states 1 and 2, `0x19` is set to the inner phase value (`FUN_02020B48(0, 0x19, *piVar2)`), so it takes more values there than 1, 4, 5 and 7. `RunGameMode_Async` writes `0xB` (mode 3, above).

**Not verified**

- The inner phases `0x0` to `0x11` of states 1 and 2 (the second `switch` on `DAT_02070358`, `0x02133CF0`). They load the scene and are not named here.
- Scene variables `0x10`, `0x12`, `0x14`, `0x15` and `0x17` are not traced.
- Whether ground mode reads variable `0x19` beyond `RunGameMode_Async`.

### Ground chain and overlay 0 (#160)

Investigator A's findings. **Decision:** no scene-to-bank table is committed, derived or raw. A tool derives each scene's bank from the user's own dump; the method is in [Where the map value comes from](#where-the-map-value-comes-from-160-p3). Tag: **blue-verified** (decompile and xrefs, in overlay 0 unless stated).

| Routine | Role |
|---------|------|
| `FUN_0214D510(out, bank, group, sector)` | Loads a ground bank, then reads group, sector and script from the bank data. It calls `GroundStation_SetBank(bank)` first. |
| `GroundStation_SetBank(bank)` `0214FC50` | If `bank` differs from the current bank, calls `GroundStation_LoadBank` (or `UnloadBank` for a negative bank). Returns 1 if the bank changed. |
| `GroundStation_LoadBank(bank)` `0214FD00` | Unloads, then reads `gGroundBankTable[bank * 8]` (`PTR_gGroundBankTable`), `FS_LoadOverlayInfo`, `FS_LoadOverlayImageAsync` and `FS_StartOverlay`. |
| `FUN_0214D420(bank, group, sector, flag)` | Calls `FUN_0214D510` with `bank` as the first argument, then `FUN_02147C48`. Its one caller is `FUN_021421CC` at `0x02143268`. |
| `FUN_0214D3A0(param)` | Calls the thunk `FUN_0214D4FC`, then `FUN_02147C48`. Called from `FUN_021421CC` case `0xEC`. |
| `FUN_0214D4FC` | Thunk: calls through the pointer at `PTR_FUN_0214D510` (`0x0214D50C`). Ghidra did not recover the jump table. |
| `FUN_0214D1F0(v)` | Returns `3` for `v` 3 or 4, and `v` otherwise. |
| `FUN_021421CC(param)` | The script and command processor. It holds the calls to `FUN_0214D420` (`0x02143268`) and `FUN_0214D510` (`0x02146064`). |

**Bank argument at each call site.**

1. Map command `case 0xEC`: the map value from `FUN_02020F74` goes through `FUN_0214D1F0`, is stored at `*psVar1`, and is passed to `FUN_0214D3A0`.
2. Ground command path (around lines 642 to 657): `local_2d4 = FUN_0214D1F0(...)` of the map value (or of the default at `DAT_02143158` when `local_b4 == 0xFFFFFFFF`). It is passed to `FUN_0214D420`, and `*psVar1` is set to it if it changed.
3. Group-sector command (line 2185): `FUN_0214D510(local_58, *psVar1, group, sector)`, which uses the current bank set by cases 1 and 2.

`psVar1` is `DAT_02143130`, a pointer to the current bank short.

**Mapping rule (from the code, not a table).** The bank index is the map value, except that map values 3 and 4 both give bank 3. The index then selects `gGroundBankTable[bank * 8]` in `GroundStation_LoadBank`. Opcode 9 is an exception, and so is the adjusted map; see the caveats below.

### Where the map value comes from (#160 P3)

**[blue-verified]** for the sites and the variable reads, unless tagged. Source: the decompile of `FUN_ov_0000__021421CC` and the x-refs of the three BSS words. Names are the Ghidra `FUN_` names.

The map value is never stored in a scene table. It is an operand of a script command, or the content of a script variable, read by the ground script interpreter `FUN_ov_0000__021421CC`. The interpreter keeps three shorts in overlay 0's BSS: the current map at `0x0215D340` (`DAT_02143158`), the adjusted map at `0x0215D33C` (`DAT_0214315c`), and the current bank at `0x0215D348` (`DAT_02143130`). `FUN_ov_0000__021481EC` sets all three to -1 at reset.

Script commands are 0x18-byte records at the script cursor. Byte 0 of word 0 is the opcode. Word 1 is the operand, and `0xFFFFFFFF` means "use the current value". The bank is written only at four sites, all inside `FUN_021421CC`:

| Opcode | Map value used | Bank written |
|--------|----------------|--------------|
| 8 (shares a label with 9; calls at `0x02142A54` and `0x02142A68`) | word 1, stored first into the current map | `FUN_0214D1F0(current map)`. The adjusted map gets `FUN_0214D208(map)`, but it is not used for the bank. |
| 9 (other branch of the same label, `0x02142AC0`) | word 1 | copied unfolded from the adjusted map, which is set to the map value itself. No 3/4 fold and no `FUN_0214D208`. |
| `0x1D` and `0x1E` (label `0x021431AC`; the call at `0x021431F0`) | word 1, or the current map when word 1 is `0xFFFFFFFF` | `FUN_0214D1F0(map)`, then `FUN_0214D420(bank, group, sector, flag)`. The current bank is set if it changed. |
| `0xEC` (`0x021460B8`) | `FUN_02020F74(param_1 + 0x1D, halfword 1)`: the value of the script variable whose id is halfword 1 | `FUN_0214D1F0(var)`, then the current bank, then `FUN_0214D3A0(bank)`. |

`0xEB` and `0xEA` (group-sector load), and the `FUN_0214D510` call at `0x02146064`, do not compute a bank. They pass the existing current bank.

**The script variable getter (`FUN_02020F74`, ARM9 `0x02020F74`).** It takes `(base, id)`, where `base` is 0 for global and scene variables, and `param_1 + 0x1D` (a per-script local block) for ids of `0x400` and above. For ids below `0x400` the descriptor is at `0x02098060 + id * 0x10`, and the data is the block at `0x0210E640` plus the offset in the descriptor. The type is the short at descriptor `+0`. **[blue-verified]** for the layout; **[medium]** for the type-8 branch, which is computed by a switch (ids 2, `0x18`, and `0x21` to `0x27`; for example id 2 returns `FUN_0202A040()`).

The current map is scene variable `0x11`, a signed 16-bit value at `0x0210E670`. The interpreter writes it at the end of opcodes 8 and 9 (`FUN_02020B48(0, 0x11, map)`). So a tool that reads the variable block at `0x0210E670` gets the current map without running the scripts. Variable `0x12` receives a field from the map's 0xC-byte record in the table at `0x020BF650`. **[blue-verified]** for 0x11 and 0x12. The other scene variables are not traced.

**Caveats to the mapping rule.**

- Opcode 9 stores the bank unfolded, so a map value of 3 or 4 gives bank 3 through opcodes 8, `0x1D` and `0x1E` only. **[blue-verified]** for the code. In a bounded static walk no opcode 9 with 3 or 4 exists; see [Opcode 9 with map 3 or 4](#opcode-9-with-map-3-or-4-258-p2) for the counts and the coverage limits.
- The adjusted map from `FUN_0214D208` is a different number. It adds an offset from `FUN_0202A040()` for a few map ids, and for others uses scene variables `0x29` and `0x2A`. It does not select the bank. **[blue-verified]**
- Opcode `0xEC` needs the value of its variable, which the scripts do not give. A static walk cannot resolve it without the variable's initial or saved value.

**Derivation methods.** Neither method commits a table. Both run on the user's own dump, and the pairs are written only to a scratch directory outside the repository.

1. **Static walk.** Start from the scene's entry script (see [Entry script of a scene](#entry-script-of-a-scene-258-p1)), and walk the scripts it reaches. For each 0x18-byte command with opcode 8, 9, `0x1D` or `0x1E`, read word 1 (if it is `0xFFFFFFFF`, use the previous map). The bank is 3 for opcodes 8, `0x1D` and `0x1E` when the map is 3 or 4, and the map itself otherwise. Opcode 9 is always unfolded. For `0xEC`, use the variable's initial or saved value, or method 2.
2. **Runtime.** With py-desmume, break on `GroundStation_SetBank` (`ov_0000::0214FC50`), or read the current bank short at `0x0215D348` and the current map at `0x0215D340` (variable `0x11` at `0x0210E670`) at each scene start. This catches opcode `0xEC` and the paths that the static walk misses. Not run for this section.

The entry script of each scene is traced for the mechanism; see [Entry script of a scene](#entry-script-of-a-scene-258-p1) (#258 P1).

**Overlay 0 and the bank overlays (#160 P2, blue-verified for the address ranges).** Overlay 0 is at `0x02138860` with size 150016 (`0x24A00`). Its BSS is at `0x0215D260` with size 1216, so overlay 0 with its BSS ends at `0x0215D720`. The ground banks (overlays 10 to 144) load at `0x0215D720`, where overlay 0 ends. So a bank does not overwrite overlay 0's code or BSS. The ground code (`FUN_021421CC`, `FUN_0214D510`, `FUN_0214D420`, `FUN_0214D3A0`, `FUN_0214D4FC`, `FUN_0214D1F0`, `GroundStation_SetBank` and `GroundStation_LoadBank`) is in overlay 0 and keeps running while a bank is loaded. Overlays 1 and 7 share the address `0x02138860` with overlay 0, so loading either one replaces it.

**Overlay 0 residency at the scene runner (#160 P2).** Static answer [blue-verified]: residency does not depend on the scene id. The runner needs overlay 0 for every id (decompile). Overlay 0 is loaded at boot (`GameLoop_Async` `0201F2E4`, call at `0201F2FC`). Two paths unload it, and each reloads it before control returns to a runner caller: `LoadAndRunDungeon_Async` (`0201DB70`: unload at `0201DB7C`, reload through `NDS_LoadOverlay_GroundMain` `0201DCEC` at `0201DC88`) and `RunOverlay7Mode` (`0201DAC4`: unload at `0201DACC`, reload at `0201DB58`). Bank loads (`GroundStation_LoadBank` `0214FD00`) write at `0x0215D720` and up, above overlay 0's BSS, so they do not overwrite its code.

Runtime [blue-verified] (RAM bytes at the runner entry, not hook bookkeeping): overlay 0 code (`10402de9 44109fe5` at `0x0214FCA4`) is intact at the runner entries for scene ids `0xF` (frame 74), `0x10` (frame 614) and `0` (frame 2575), and after five bank loads. Probe [`verification/v7-trace/overlay0_residency.py`](../../verification/v7-trace/overlay0_residency.py); command and result in [`verification.md`](../verification.md#160-p2--overlay-0-residency-at-the-scene-runner). The current-bank variable at `0x0215D704` (inside overlay 0's BSS) reads -1 before the first bank load.

Ids 1 to `0xC`: [static only]. No input reached them. **Not verified:** the reload after a dungeon and the overlay-7 path were not seen at runtime (the run stops inside the dungeon); the load-failure path is untested (retail `FUN_02029dac` does nothing); runner entries during the dungeon were not covered. The text overlay's unload at `0201DBE8` is settled in [Into a dungeon and back](#into-a-dungeon-and-back) (#160 P4).

**Not verified**

- The other scene variables read by `FUN_02020F74` (apart from `0x11` and `0x12`) are not traced. The meaning of `FUN_02020F74` and of the current-map pointer `DAT_02143158` is in [Where the map value comes from](#where-the-map-value-comes-from-160-p3).
- The list of ground scenes 0 to C and their banks is not written down: no table is committed (see the decision above). A tool derives it from the user's dump.
- The entry script of each scene (#160 P3-d): the mechanism is traced (#258 P1, see [Entry script of a scene](#entry-script-of-a-scene-258-p1)). Still open there: the scene-id to index mapping was observed only for ids `0xF`, `0x10` and `0`, and the variables that select ids 9 to `0xC`. Follow-up: [#267](https://github.com/TorratDev/rust-mm/issues/267).
- The jump table behind `FUN_0214D4FC` (not recovered at `0x0214D508`). It may also map scenes to banks.
- Whether the `case 0xEC` and command paths are the only ways `*psVar1` changes (`FUN_02147B48` and other writers were not checked).
- Residency of overlay 0 for scene ids 1 to `0xC`: not reached by the boot run, so only the decompile supports it (see the residency paragraph above).
- Whether any path loads overlay 1 or 7 during the ground scenes.
- The bank overlays' own BSS and static initialisers, against the overlay 0 BSS range.

### Entry script of a scene (#258 P1)

**Tags.** [blue-verified] for the call sites, the structure and the runtime order, unless tagged. Names of the three managers are [red-hypothesis]. No scene-id table is committed (legal rule 3): the mapping is derived from the user's dump by the method below.

**Per-frame steppers.** Three functions step the script contexts of entities every frame:

- `FUN_021505D0`: 24 entities, stride `0x1F0`, array pointer in the BSS word `0x0215D1DC` (set by `FUN_0215559C`). Red-hypothesis role: people and monsters.
- `FUN_0215574C`: 16 entities, stride `0x1C4`, array word `0x0215D1F8` (set by `FUN_02156B6C`). Red-hypothesis role: objects.
- `FUN_02156CD8`: 16 entities, stride `0x1C4`, array word `0x0215D210` (set by `FUN_02157B98`). Red-hypothesis role: effects.

Each calls `FUN_021461D0(ctx)` with `ctx = entity + 0x38`. That step reads the state halfword at `+0x26`: 0 is idle, 1 and 2 are start and wait, 3 is running (it calls the interpreter `FUN_021421CC`), and 4 is finished.

The three are called from the ARM9 scene runner `FUN_0206F7EC` once per frame, in this order: `0214C0A8`, `02157DBC`, `021505D0`, `0215574C`, `02156CD8`, `0213E8F4`, `0213BC1C`. The calls are `bl` words between `0x0206FEAC` and `0x0206FEC4`. Ghidra shows no callers for them because the ARM9 calls into overlay 0 are not resolved in the project. All three fire at runner frame 79 (runtime).

**The fourth caller, the master context.** `0x0214C0A8` has no Ghidra function. From its raw words it asserts that the global is set, calls `FUN_0214DCAC`, then calls `FUN_021461D0(*0x0215D1A8)`. The master context is allocated by `FUN_0214D804` (size `0x110`) and configured by `FUN_02148024` (group and sector both -1). Its runtime address is `0x021304F4`, the same at every scene. In a 20,000-frame run the step saw one master context, four lives contexts, two objects contexts and two effects contexts. **[blue-verified]**

**Two stages.**

1. **The ARM9 loader script.** [blue-verified] The runner reads the scene id from `*DAT_02070334` (`0x02133CEC`) and switches on it to give a script index, with -1 as the default. For ids 9 to `0xC` the index comes from the scenario record `FUN_02070890` (scene variable `0x14`, or `0x15` when `0x14` is `0x51`, field `+10`). For a few other ids it is one constant each, taken from a block the runner keeps on its stack. Then the runner calls `FUN_0214D498(index, 0)`, substituting the default constant when the index is -1. The index selects a 12-byte entry of an ARM9 table at `0x020DA5D4`, whose first field is a script pointer into ARM9 data. `FUN_0214D498` builds a reference of kind 2 with `FUN_02147E6C`, and `FUN_02147C48` installs it on the master context. The loader scripts use the same 0x18-byte format as the bank scripts, but they are not all short: some have a bank load (opcode `0x1E`) and a wait, others are long state scripts. The table has 406 entries, indices 0 to 405, from `0x020DA5D4` to `0x020DB8DC` (see [#267 P1c](#runtime-repeat-and-forced-scene-ids-267)). [blue-verified static, high for the count]
2. **The bank's entry script.** [blue-verified] The `0x1E` command calls `FUN_0214D420(bank, group, sector, flag)`, which calls `FUN_0214D510`. `FUN_0214D510` writes a kind-2 reference to the first element of the sector's script list (`element + 8`). `FUN_0214D420` changes it to kind 5 only when its fourth argument (`flag`) is nonzero. The interpreter passes that argument as `(opcode == 0x1E)`, and `FUN_0214D498` does the same. `FUN_02147C48` installs the reference on the master context after the loader script has run. The sector record is 0x28 bytes, with the count at `+0x20` and the list pointer at `+0x24`.

The bank layout behind this: the ARM9 table at `0x020DB8E0` has 246 entries of 8 bytes, `{overlay id, root pointer}`. The root is `{u32 group count, u32 groups pointer}`, and a group entry is `{u32 sector count, u32 sectors pointer}`. The group and sector of a `0x1E` command come from its high halfword and byte 1.

**Runtime order** (`p258e`). For scene ids `0xF`, `0x10` and `0`, the master context first gets the loader script (kind 2), then `FUN_0214D420` runs, then one frame later it gets the bank script (kind 5, installed with the flag set by the `0x1E` load). Later script-driven scene changes repeat stage 2 only: kind-5 installs at frames 1136, 1270 and 1789 with new group numbers, and no new `FUN_0214D498` call. This is how scripts chain without the runner.

**Method (static walk and runtime cross-check).**

1. Start points: the ARM9 loader entries (the pointer field of each 12-byte entry; the first walk used only entries 0 to 55 of the 406, see [#267 P1c](#runtime-repeat-and-forced-scene-ids-267)), and every sector-list element of every bank (`element + 8`).
2. Walk 0x18-byte records. Byte 0 is the opcode, the low half of word 1 is a signed operand (-1 means "use the current value"), and `0xF0` ends a script. The cursor advances 0x18 per command.
3. Scene id to index: take it from the runner's switch on the user's dump, in the tool and not in a committed table. Or read the argument of `FUN_0214D498` at runtime.
4. Cross-check at runtime: break on `FUN_0214D498` (the index; `lr` `0x0206FDB4`, `0x0206FDC4` or `0x0206FE9C` for the three runner paths), on `FUN_02147C48` with `r0 = *0x0215D1A8` (the script pointer and its kind: 2 is a loader script, or a bank script installed by a `0x1D` load; 5 means the install came with the flag set, which is a `0x1E` load), and on `FUN_0214D420` (bank, group, sector).

**Not verified.**

- The natural scene-id to index mapping for ids 1 to `0xE`. Ids `0xF`, `0x10` and `0` were seen at runtime; ids 1 to `0xE` were not reached by the boot run [open]. The runner's selection for ids 1 to `0x11` was run with a forced id, see [Runtime repeat and forced scene ids](#runtime-repeat-and-forced-scene-ids-267).
- How scene variables `0x14` and `0x15` are set for the scenario ids, and the `FUN_02070890` record format.
- Whether a loader script branches on variables before its load command. The table was walked linearly, and its first commands are straight-line.
- The `0x0214C0A8` neighbourhood has no function in the Ghidra project; the read was of raw words, and the project was not edited.
- The manager names (people, objects, effects) are Red-hypothesis, not checked against Blue strings.

Confidence: high for the call chain, the two-stage mechanism and the structure; medium for the full scene-id coverage ([#267](https://github.com/TorratDev/rust-mm/issues/267)).

### Runtime repeat and forced scene ids (#267)

Probe: [`verification/v7-trace/scene_entry_probe.py`](../../verification/v7-trace/scene_entry_probe.py). Hooks (exec): the scene runner `0x0206F7EC` (`r0` = scene id), `FUN_0214D498` (`r0` = index; also the 12-byte entry at `0x020DA5D4 + 12 * index`: pointer and two words), `FUN_0214D420` (bank, group, sector, flag) and `FUN_02147C48`. Commands, ROM SHA-1, input SHA-1 and output SHA-1s are in [`verification.md`](../verification.md#267-p1--scene-entry-probe). Limit: new-game boot state only.

- **P1a. Repeat of the #258 scenes `0`, `0xF` and `0x10`.** [blue-verified runtime, high] 3200 frames, no forced id. Runner id `0xF` at frame 74 (`lr 0x0201F4A0`) gives `FUN_0214D498` index `0x74` at frame 76 (`lr 0x0206FDB4`), then the master install and bank 235 group 1 sector 0 flag 1 at frame 76. Id `0x10` at frame 614 (`lr 0x0201F4B4`) gives index `0x75` at 616, then bank 6 group 5 sector 0 flag 1. Bank loads with flag 1 follow at frames 1133 (bank 5 group 29), 1270 (bank 234 group 1) and 1789 (bank 229 group 2) with no `FUN_0214D498` call. A `FUN_0214D498` call at frame 2380 has index `0x72` and `lr 0x0206FE9C` (the third runner path, no runner entry). Id `0` at frame 2575 (`lr 0x0201E710`) gives index `0x66` at 2579 (`lr 0x0206FDC4`, the default-substitution path), then bank 228 group 1 sector 0 flag 1. This matches the #258 mapping for `0xF`, `0x10` and `0`, and adds the frame-2380 call with index `0x72`.
- **P1b. Selection for ids 1 to `0x11`, forced.** [blue-verified runtime for the selection code; high; **forced input, not natural transitions**] The boot input only reaches ids `0xF`, `0x10` and `0`. The probe's `--runner-sid ID` overwrites `r0` (the scene id) at the first runner entry (frame 74), 300 frames per id; the event `forced_runner_sid` marks it. The state around it (scene variables, save) is the new-game boot state, so the runner's own code selects the index but the context is not a real transition. Index passed to `FUN_0214D498`:
  - ids 1, 2, 3, 4, 6, 7, 8: `0x66`, `lr 0x0206FDC4`, bank 228 group 1 sector 0 flag 1 (same as id 0);
  - id 5: `0x70`, `lr 0x0206FDB4`; no bank load before frame 114; the probe then saw a runner entry with id `0x10` at frame 111 and index `0x75` (the flow after the forced id, not analysed);
  - ids 9 to `0xC`: `0x167` (359), `lr 0x0206FDB4`, bank 228 group 1 sector 0 flag 1. The index comes from the scenario record `FUN_02070890` with the boot-state scene variables `0x14` and `0x15`; other game states give other indices (not tested);
  - id `0xD`: `0x71`, no bank load in 300 frames (the `0xD` path sets flags at `0x02070338`–`0x02070340` differently);
  - id `0xE`: `0x73`, bank 236 group 1 sector 0 flag 1;
  - id `0xF`: `0x74`, bank 235 group 1 sector 0 flag 1 (matches natural);
  - id `0x10`: `0x75`, bank 6 group 5 sector 0 flag 1 (matches natural);
  - id `0x11`: `0x76`, bank 229 group 3 sector 0 flag 1.

  This agrees with the constants in the decompile of `FUN_0206F7EC` (`0x70`, `0x71`, `0x73`, `0x74`, `0x75`, `0x76`, default `0x66`) and with the `0x72` constant (the frame-2380 call, third path).
- **P1c. The loader table has 406 entries.** [blue-verified static, high for the count] The table runs from `0x020DA5D4` to `0x020DB8DC`: 406 entries of 12 bytes (`0x1308` bytes). The word after the last entry, at `0x020DB8DC`, is a 4-byte tail, and the bank table starts at `0x020DB8E0`. Layout, read from the bytes: `{u32 script pointer, u16 index+1, u16 kind, u32 name pointer}`. The u16 is index+1 for all 406 (1 to `0x196`). `kind` takes small values (4, 5, 7, 8 and 9 seen; entries 0 to 12 have 4, 13 to 30 have 5). The name pointer points into an ARM9 string table at `0x020C0A38` to `0x020C28xx`. Examples: index 0 `WAIT_END_TALK_FUNC`, 56 `NORMAL_EVENT`, 116 `DEMO_03`, 359 `GETOUT_M01E02A`, 405 `MOVE_DEBUG_CAMERA`. So the table holds named event scripts, not only bank-load stubs.
  - **Null scripts.** 8 entries have script pointer 0 (indices 56, 57, 357, 400, 401, 402, 403 and 404). The other 398 have a script, and no two share a start.
  - **No code bound.** The reference builder `FUN_02147E6C` (`ov_0000::02147E6C`, overlay 0) reads the entry at `*(PTR + index * 0xC)` with no compare. `FUN_0214D498` only logs the index and passes it on. The other references to the table are at `02145FB0`, `02142FB0` and `02143058`, all in the interpreter `FUN_ov_0000__021421CC`; those uses were not decompiled. [blue-verified static, medium-high]
  - **Runtime.** The index 359 read at runtime (`FUN_0214D498`) matches the static entry: script pointer `0x020CF1D8`, name `GETOUT_M01E02A`. Only the entry pointer and fields were logged, and the layout and the null entries were not confirmed at runtime.
  - **Opcode `0x1E` recount.** [blue-verified static, medium] The earlier note that loader scripts are short (`1E`, wait, `F0`) holds only for some of them. Walking each script from its start to the next start (the method in P2, with the last script bounded at its first `0xF0`) gives **290 `0x1E` commands in 179 of the 398 scripts**, with 74 distinct (word 1, word 2) pairs and 74 distinct bank values. None are in the first 56 entries. The 290 is an upper bound: 12 gaps between consecutive starts are not a multiple of 0x18, so some of the walk is over data between scripts. Not checked: which of the `0x1E` commands are reachable (branch targets), and the bank 3 and 4 folding.
- **P1d. Natural reach of ids 1 to `0xE`.** [reached in normal play for 1, 2, 5, 9, 10 and 12; id 11 has no natural route; see P4 below] The boot input (new game, intro) enters only `0xF`, `0x10` and `0` (runner callers `lr 0x0201F4A0`, `0x0201F4B4`, `0x0201E710`). Ids 1 to `0xE` need the mode dispatcher to call the runner with those ids, which needs game progress or a save; the supported setup has none (see the #260 entry in [`verification.md`](../verification.md)). Scenario ids 9 to `0xC` also depend on scene variables `0x14` and `0x15`. The static map of ids, callers and conditions is P3 below, and the natural runs are P4.

- **P3. Static map of scene ids and start modes.** [blue-verified, static decompile; the pmd-red names are red-hypothesis, cited by name only from `pret/pmd-red 013475aa src/main_loops.c`]

  Blue's main loop is `FUN_0201E3C8`, the `RunGameMode_Async` of pmd-red. `GameLoop` (`FUN_0201F2E4`) calls it with argument 2 (`0x0201F7A4`), 1 (`0x0201F7B4`), 3 (`0x0201F7C4`) or 0 (`0x0201F7DC`). Calls to the scene runner go through the thunk `0x0201DCA8`, which jumps to `FUN_0206F7EC`. Five `bl` sites reach the thunk:

  - `0x0201E6B8` (lr `0x0201E6BC`), in `FUN_0201E3C8`: `runner(5)`, after the world map confirms a dungeon.
  - `0x0201E70C` (lr `0x0201E710`), in `FUN_0201E3C8`: `runner(mode)`, the generic call. It is reached for every mode except 4, 5, 7 and 8.
  - `0x0201F49C` (lr `0x0201F4A0`): `runner(0xF)`, every boot. Seen at runtime at frame 74 (P1a).
  - `0x0201F4B0` (lr `0x0201F4B4`): `runner(0x10)`, every boot. Seen at runtime at frame 614 (P1a). Between the `0xF` and `0x10` calls the boot code clears `EVENT_LOCAL` (`FUN_020211AC(0, 0x3B)`).
  - `0x0201F4CC` (lr `0x0201F4D0`): `runner(0x11)`, only if var `0x3B` is 0 after the `0x10` call.

  **Initial mode.** The start mode is script variable `0x19` (read by `FUN_02020F74(0, 0x19)`). It is the same enum as pmd-red's `START_MODE` (0x18), values 0 to 12 [red-hypothesis for the variable's name]. If the value is 7 (a quicksave), `GameLoop` argument 2 gives mode 8, and argument 3 gives mode 0xB and writes `0x19 = 0xB`. Arguments 0 and 1 keep 7. Otherwise a value of 0 stays 0, 0xB stays 0xB, and any other value becomes 1.

  | id | Blue caller and condition | pmd-red STARTMODE | natural action | reaches the runner |
  |---|---|---|---|---|
  | 0 | `0x0201E70C`; initial mode when `0x19` is 0; `GameLoop` argument 0 | NEW_GAME | new game at the title | yes; seen at runtime (boot run, frame 2575, `lr 0x0201E710`) |
  | 1 | `0x0201E70C`; initial mode when `0x19` is not 0, 7 or 0xB; `GameLoop` argument 1 | CONTINUE_GAME | Continue at the title | yes (expected; not run) |
  | 2 | `0x0201E70C`; set after the friend-areas map (mode 4) and when the world map is cancelled or invalid (mode 5) | GROUND | leave the friend-areas map or cancel the world map without entering | yes |
  | 3 | none: nothing assigns mode 3 (the value 3 is only a dungeon return code). Argument 3 with `0x19` = 7 gives mode 0xB, not 3 | STARTMODE_3 | none found | no |
  | 4 | not passed to the runner: the loop handles it inline (friend-areas map, `FUN_0201F960`, `FUN_02027C58`). Entered when the runner returns 5 | FRIEND_AREAS | select HABITAT (runner state 3, return code 5) | no |
  | 5 | `0x0201E6B8` (lr `0x0201E6BC`), explicit, after the world map confirms a dungeon. Entered when the runner returns 6 | DUNGEON_FROM_WORLD_MAP | select a dungeon on the world map and confirm | yes |
  | 6 | none: the generic call would pass it, but nothing assigns it (the value 6 is only a runner return code) | STARTMODE_6 | none found | no |
  | 7 | not passed to the runner. Initial mode only, when `0x19` is 7 and argument is 0 or 1 | CONTINUE_QUICKSAVE | continue a dungeon quicksave (the maintainer's save has none; see [#260](../verification.md)) | no |
  | 8 | not passed to the runner. Initial mode only, when `0x19` is 7 and argument is 2 (`0x0201F7A4`) | STARTMODE_8 | continue the quicksave; the findings tie it to deleting the first type-7 mail (not run) | no |
  | 9 | `0x0201E70C`; post-dungeon result 1 or 4; or runner return 7, 8 or 10 with `FUN_0206F034` failing (code 0xD) | DUNGEON_WON | clear the dungeon | yes |
  | 0xA (10) | `0x0201E70C`; post-dungeon result 2; or the quicksave continue path (code 0xC) | not checked | a dungeon that ends with result 2 (what sets it is not verified) | yes |
  | 0xB (11) | `0x0201E70C`; post-dungeon result 5, 0 or any other value; argument 3 with `0x19` = 7 (sets `0x19` = 0xB); runner return 0 with the quicksave marker check failing | not checked | a dungeon that ends with another result; quicksave continue | yes |
  | 0xC (12) | `0x0201E70C`; post-dungeon result -1; or runner return 9 with `FUN_0206F034` failing (code 0xB) | DUNGEON_LOST | fainted | yes |
  | 0xD (13) | none. Code 0xD is a dungeon-path return code, never a runner id | STARTMODE_13 (DEBUG_SCRIPT) | debug only | no |
  | 0xE (14) | none. Red calls 14 at boot; Blue has no call with 0xE | STARTMODE_14 (DEMO_01) | boot demo | no |
  | 0xF (15) | `0x0201F49C` (lr `0x0201F4A0`), every boot | STARTMODE_15 (DEMO_02, see the open name question) | boot demo | yes |
  | 0x10 (16) | `0x0201F4B0` (lr `0x0201F4B4`), every boot | STARTMODE_16 (DEMO_03, see the open name question) | boot demo | yes |
  | 0x11 (17) | `0x0201F4CC` (lr `0x0201F4D0`), only if var `0x3B` is 0 after `0x10` | STARTMODE_17 (DEMO_04, see the open name question) | boot demo | yes, conditional; not reached in the 3200-frame boot run |

  The Red names are [red-hypothesis]: the same numeric value in pmd-red. Blue's call sites and the runner's switch are [blue-verified].

  **Runner return codes and the post-dungeon switch** ([blue-verified], `FUN_0201E3C8`):

  - Return `0xE`: the loop returns 0 and leaves to the title.
  - Return `5`: mode 4. Return `6`: mode 5.
  - Returns `7`, `8`, `9` and `10` take the dungeon branch. If `FUN_0206F034` (pmd-red `sub_80991E0`) fails, returns 7, 8 and 10 give code `0xD` and mode 9, and return 9 gives code `0xB` and mode 12.
  - Returns 0 and 2 go through the quicksave resume. Return 0 with the quicksave marker check (`FUN_0202A11C` against `DAT_0201EB78`) failing gives mode 11.
  - Post-dungeon switch on the result field (`setup+0x60`, `local_60`): 3 or -2 return 1 and leave the loop; 1 or 4 give mode 9 and also write var `0x43` (`FUN_020208E8`); 2 gives mode 10; -1 gives mode 12 (`FUN_0200E3FC`); 5 and other values give mode 11 (`FUN_0200E3FC`).
  - [blue-verified] In `FUN_0201E3C8`, `local_60 == 3` and `local_60 == -2` return 1 before the switch, so the switch's case 3 is dead. Case 0 breaks to the default: mode 11 with `FUN_0200E3FC`.
  - Mode 11 (0xB) runs `FUN_0201DD60` before its runner call, and mode 12 (0xC) runs `FUN_0201DE24`. The pmd-red names are RemoveAllMoneyAndItems and RemoveMoneyAndRandomItems [red-hypothesis].

  **What this means for P1b.** The loop never passes ids 4, 7 and 8 to the runner (4 is inline; 7 and 8 skip it), and nothing writes 3 or 6. Ids 0xD and 0xE have no caller. So the P1b forced results for ids 3, 4, 6, 7, 8, 0xD and 0xE describe the runner's selection code only, not a natural transition. The P1b results for ids 1, 2, 5, 9 to 0xC and 0x11 can occur naturally, but their runs were forced.

  **Not verified.**

  - Overlay callers of the runner thunk and of `FUN_0206F7EC`. The x-refs were taken from the ARM9 only.
  - The Red names for 0xF, 0x10 and 0x11 are not settled. One reading gives the same numeric value (Red 15, 16, 17). The other reads Blue's calls as an offset (Red 14 to 0xF, 15 to 0x10, 17 to 0x11), at medium confidence. It also counts four Red calls against Blue's three. Not decided here.
  - The meaning of argument 3 differs between the readings: title choice 4 in one, the quicksave menu in the other. Both agree that it gives mode 0xB when `0x19` is 7.
  - Which dungeon code writes the result values 1, 2, 4, 5 and -1 to `setup+0x60` is not verified.
  - The constants behind the 1 and 2 written to var `0x43` (registers r6 and r7 at `0x0201EB20`) were not read. The flag conditions for ids 0xF and 0x10 (`FUN_02020848(4, ...)`) were not read.
  - Whether other code writes `0x19` with values that would create mode 3 or 6 was not swept.
  - The reach after a save (the last column) is inferred from the control flow, not run.

  Confidence: high for the call sites and the loop's dispatch; medium for the Red names and for the reach after a save.

- **P4. Natural reach of the scene ids (#267).** [blue-verified runtime] Runs of normal play from the maintainer's post-Thunderwave save. Ids 1, 2, 5 and 12 come from investigator B's full input run, which was run twice with identical output. Ids 5, 9 and 10 also come from the maintainer's live run, which is a single run. The index is `FUN_0214D498`'s argument, the bank is the `FUN_0214D420` load with flag 1 that follows it, and the result is the post-dungeon value at `setup+0x60` (P3). Hashes and commands are in [`verification.md`](../verification.md#267-p4--natural-reach-of-the-scene-ids).

  | id | natural action | frame | index | bank | evidence | tag |
  |---|---|---|---|---|---|---|
  | 1 | title Continue (`GameLoop` argument 1) | 1652 | `0x66` (lr `0x0206FDC4`) | 13/17 (live run) | B full run (runner at f1652, both runs); live run (index `0x66`) | [blue-verified runtime] |
  | 2 | leave the Square to the west; destination menu "Pokemon Team Base"; A | 16074 | `0x66` (lr `0x0206FDC4`) | not logged in the full run; bank 9/0 at f585 in a menu-savestate run | B full run, two identical runs | [blue-verified runtime] |
  | 5 | Dungeons menu on the path south of Team Base; A on Tiny Woods (`lr 0x0201E6BC`) | 18564 (B); 12149 and 62651 (live) | `0x70` (lr `0x0206FDB4`) | not logged in the full run | B full run; live run, index `0x70` at 12152 and 62654 | [blue-verified runtime] |
  | 9 | enter and clear a dungeon (maintainer's account); the result was not logged | 33624 | `0x167` (f33627) | 5/11, flag 1 | live run only | [blue-verified runtime, single run] |
  | 10 | leave a dungeon early (maintainer's account, "ending a mission"); the result was not logged | 73146 | `0x168` (f73149) | 5/11, flag 1 | live run only | [blue-verified runtime, single run] |
  | 12 | Others, Quicksave or Give Up, Give Up, Yes; the result is -1 | 20493 (runner) | `0x167` (lr `0x0206FDB4`, index frame not logged) | 13/7, flag 1 (savestate run at f912) | B full run, two identical runs; the savestate run confirms result -1 at f908 | [blue-verified runtime] |
  | 0x11 | boot demo: the boot runner call, taken when var `0x3B` is 0 after the `0x10` call | 933 | not logged (boot call, lr `0x0201F4D0`) | not logged | B full run, both runs (runner at f933); live run (runner at f933, same lr) | [blue-verified runtime] |

  - **Index for ids 9 to 0xC depends on the game state.** The live run's id 10 index is `0x168`, but the forced P1b value for ids 9 to `0xC` is `0x167` (boot-state scene variables). Ids 9 (live) and 12 (B) both give `0x167`. So the P1b values for 9 to `0xC` are not fixed, as P1b warned.
  - **Dungeon results seen at runtime.**
    - `-1`: Give Up. The savestate run with the result hook shows result -1 at f908, then the runner with id 12 (the static faint path uses the same value).
    - `3`: quicksave and quit. Quicksave, then Yes, gives result 3 at f1587 with no runner call, and the game returns to the title. Title Continue then shows "Resuming from quicksave"; with Yes, the loop takes argument 1 at f61 and starts the dungeon at f62 with no runner call. Continue from a quicksave does not reach the runner.
    - `1` and `4` (clear, rescue success) and `2` (a successful exit that is not a clear): written as in P3, but not observed. The live run's id 9 and id 10 are consistent with them, but the result was not logged.
    - `5` (a quicksave read failed) and `0` (nothing writes it in the dungeon driver): not observed.
  - **The dungeon driver calls `FUN_0214D498` with index 0** (`lr 0x0206DBC4`, `r1` = 4) at the start of a dungeon. [blue-verified runtime]. The count differs by run. The full run (two identical runs) has one such call, at f18672. Five calls (f357 to f360) are in investigator B's dungeon-start savestate runs, which are exploration runs and not the full run. The maintainer's live run has eight, spread over the dungeon (f12257, f15192, f21813, f21814, f62761, f67816, f68391 and f68876).
  - **Ids not reached in normal play.**
    - 3: nothing assigns mode 3. The value 3 is the quicksave-and-quit result, which returns 1 and never becomes a mode (P3). [blue-verified static; runtime result 3 above]
    - 4: the loop handles it inline and never passes it to the runner (P3). It is not a runner id.
    - 6: nothing assigns mode 6. The value 6 is a runner return code that gives mode 5 (P3).
    - 7: skips the runner. A quicksave resume reaches dungeon start with no runner call (above).
    - 8: skips the runner. Its only route is the initial mode (`0x19` = 7 with `GameLoop` argument 2). That route was not run.
    - 0xD: nothing calls it. Code `0xD` is a dungeon-path return code (P3).
    - 0xE: nothing in Blue calls it (P3).
    - 11: no natural route found [static, medium]. Mode 11 comes from result 5 (a failed quicksave read), from result 0 (no writer found), from the initial mode `0xB`, from the title path with argument 3 (which needs a pending quicksave), and from a quicksave marker mismatch. Ordinary play from this save reaches none of them without an invalid quicksave or the title option. Quicksave Continue gives argument 1, not 3 (runtime), so it does not reach 11. Not run.
  - **Replay of the live run does not match it.** Replaying the live run's key log (`--replay`, 75086 frames) gives the same event log in two runs, but the replay diverges at frame 2167, just after Continue, and reaches only ids 0xF, 0x10, 0x11 and 1. The likely cause is touch input that the key log does not hold: the maintainer clicked the window to take focus, and in that window a click is a DS touchscreen tap. Not verified. The live run is single-run evidence, accepted by the maintainer. Ids 2, 5 and 12 rest on B's full run, which reproduced exactly.

  Confidence: high for ids 1, 2, 5 and 12 (two identical full runs, consistent with P3); medium for ids 9 and 10 (one live run, results not logged); static, medium for id 11.

### Opcode 9 with map 3 or 4 (#258 P2)

**Result: 0 occurrences** in the bounded static walk below. [blue-verified] for the walk, with the coverage limits stated.

**Method** (scripts scratch only, not in the repo). Start points: every sector-list element of the 246 banks (5,398 references, 536 distinct script starts, no malformed bank root) and the first 56 ARM9 loader entries (indices 0 to 55 of the 406; the other 342 scripts were not walked here). From each start, walk 0x18-byte records up to the next start in the same image, or to the first `0xF0` for the last script, so branch targets inside a script are included. The walk covered 100,078 records in the banks and 2,817 in the ARM9 scripts; this includes over-walk.

**Counts** (no script bytes recorded):

| Opcode | Records | With map value 3 or 4 | With value -1 |
|--------|---------|-----------------------|---------------|
| 8 | 439 | 5 | 6 |
| **9** | **10** | **0** | **0** |
| `0x1D` | 67 (banks) and 1 (ARM9) | 0 | 54 |
| `0x1E` | 29 (banks) and 253 (ARM9) | 3 (ARM9) | 11 |

So opcode 9 appears 10 times in the scripts the walk reaches, and never with 3 or 4. Opcodes 8 and `0x1E` carry 3 or 4 as documented above, and fold to bank 3 through `FUN_0214D1F0`.

**Cross-check for false negatives.** A blind scan of every bank image, at every 24-byte phase used by a script start, finds 155 words with low byte 9. Two have word 1 equal to 3 or 4. Both sit in blocks of zero records (the four records on each side are all zero opcodes), so they are padding or non-script data, not commands. **[blue-verified]** for this reading.

**Limits** (not verified).

- Scripts not reachable from the sector lists or the ARM9 table, such as scripts in overlay 0 or built at runtime, were not walked.
- 2,838 sector records have no list (count 0), and no script comes from them.
- `-1` means the current value at runtime. No opcode 9 record has `-1`, so no runtime-resolved 3 or 4 comes from opcode 9.
- Opcode `0xEC` (variable operand) is out of scope here.
- The linear walk can run past a script's last `0xF0` into non-code bytes. That can only add false positives, and none were found.

Confidence: high that no opcode 9 with 3 or 4 exists in the 592 scripts walked (536 bank scripts and the first 56 ARM9 loader scripts; the other 342 ARM9 scripts were not checked for opcode 9); medium for "in any script", because of the reachability limits.

### Frame action names (#161 P4)

Investigator B's findings. Tags: **blue-verified** (structure and register addresses); names are **red-hypothesis**, and the names here are recorded, not applied to `symbols.md`.

- **#9 `FUN_02017E60`: `LoadBufferedInputs`** (Red's name; the first half matches Red's). It copies the 16-byte buffered input at `DAT_02018194` to the real input struct at `DAT_020181A0`, clears four `u16` fields (`+0`, `+2`, `+4`, `+6`, the same four Red clears), and resets an unused struct. Blue adds a touch decode (`FUN_0207CE64` or `FUN_0207CDE4`, chosen by `DAT_0201819C`) and a state machine on its result (a `switch` on `DAT_02018198+0x10`). Red's source notes that this place is different in Blue. Confidence: medium-high for the copy and clear; medium for the touch part (not traced).
- **#15 `FUN_01FF86B8`: `xxx_call_update_bg_vram`** (name supported, medium). It is flag-gated. It copies two blocks of 32 × 2 of `0x40` bytes (`FUN_0201A8FC`, `FUN_0201A884`), then calls `FUN_01FF8574` on two lists of `0x48`-byte records (count at `+4`, source and size at `+0x30`, `+0x34` and `+0x38`). If the flag at `+0x45` is set, it copies `0xD00` bytes through `FUN_01FF9FC4`. This is a flush of scheduled copies, which fits "update BG VRAM". **Not verified:** `FUN_01FF8574`'s destination memory.
- **#17 `FUN_02019268`: `FlushDisplayState`** (the name in [`io.md`](io.md#per-frame-flush)). The register writes are in `io.md`; this function also writes EVA and EVB at `0x04000052` (BLDALPHA, from the literal pool). Red's name `xxx_call_update_bg_sound_input` is not supported: nothing in the function touches sound. Confidence: high for the register writes; the name is recorded.
- **Not verified (#161 P4):** `FUN_02075938` (the second-screen window or sprite call), not traced; `FUN_01FF8574`'s destination memory (#15); the touch-decode function behind `FUN_0207CE64` and `FUN_0207CDE4` (#9).

## Into a dungeon and back

`RunGameMode_Async` calls `LoadAndRunQuickSaveDungeon_Async` (`0201E204`), which allocates the dungeon buffers (`MemoryAlloc(…, 7)` twice), reads the quicksave if the setup asks for it, then calls `LoadAndRunDungeon_Async` (`0201DB70`):

1. `NDS_UnloadOverlay_GroundMain` (overlay 0 out).
2. Load and start the language text overlay (2–6), then call `FS_UnloadOverlay` on it (`FUN_0207BB28`, called at `0201DBE8`). That unload only drops the overlay's registered callbacks in its address range. It leaves the image in RAM, so the text tables stay resident through the dungeon. Then load and start overlay 1. **[blue-verified]** for the calls, and for the residency at runtime on one dump (#160 P4, see [`verification.md`](../verification.md#160-p4--text-overlay-residency-across-a-dungeon-run)).
3. `RunDungeon_Async` (`0206DC20`, in the ARM9 although it calls into overlay 1): the dungeon itself, including RNG seeding ([`rng.md`](rng.md)).
4. Unload overlay 1 only (the text overlay is not unloaded a second time), then `NDS_LoadOverlay_GroundMain` (overlay 0 back).

In Red, step 2 is a stub with a comment that the NDS version loads the dungeon overlay there.

## Open questions

- The IRQ vector slot is `0x027E3FFC`, the dispatcher is `FUN_01FFA21C`, and the handler table is at `0x027E0008`; IRQ 0 is VBlank (see [Frame wait and interrupts](#frame-wait-and-interrupts), #157). Not verified: which bit each of the other seven `FUN_02075C5C` callers registers.
- The entry of the boot routine, before `02000C84`.
- The caller of the 4 RNG steps per frame, and the A and Select effect (Select has no data after emulated frame 2508).
- The writer of the flag word that `FUN_02018ACC` tests (see [#159](rng.md#per-frame-steps-159)). Its caller is the IRQ dispatcher (VBlank, IRQ 0; [`io.md`](io.md#frame-path)).
- Overlay 7 on the real title path (#161 P3). The screen is seen only under fault injection (see [Overlay 7](#overlay-7-the-choice-5-screen-161-p3)). The fresh-boot title menu shows New Game and Adventure Log only, so choice 5 is not offered in that boot. Whether state `0xF` of `FUN_020628BC` runs with a save is open and needs a save.
- Scene-runner inner phases `0x0` to `0x11` of states 1 and 2 (the switch on `DAT_02070358`, not named), and the scene variables `0x10`, `0x12`, `0x14`, `0x15` and `0x17` (their readers are not traced). The phase names and the variable `0x19` values are in [Scene runner states](#scene-runner-states-161-p1).
- Whether modes 2 and 3 are reached in play.
- Names for the IME helpers (proposed in #157 P2, awaiting confirmation) and the rename of `FUN_0201DE24` to `RemoveMoneyAndRandomItems` (proposed in [Scene graph](#scene-graph), awaiting confirmation).
- Where the main loop of `RunDungeon_Async` advances a turn (the V7 trigger, #49).
