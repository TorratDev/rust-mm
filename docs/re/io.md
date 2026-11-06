# Input, screens, sprites and file loading

How Blue reads the buttons and the touch screen, sets up the two screens, fills OAM and runs fades, and loads files from the pack archives and from loose NitroFS files. Issue: [#45](https://github.com/TorratDev/rust-mm/issues/45). Names and addresses are in [`symbols.md`](../symbols.md); the structs are in [`glossary.md`](../glossary.md).

Dump: `BlueRescueTeamEur`. Addresses are RAM addresses. Plain addresses are in the ARM9 binary (`arm9.bin`); `ov_NNNN::` marks code in an overlay. Tags: **[blue-verified]** means read in Blue's code, and where a finding is only about control flow it says so; **[red-hypothesis]** and **[tentative]** are as in [`rng.md`](rng.md). **[hypothesis]** marks a meaning the investigators did not confirm, and **[open]** marks a gap; both are as the #177 investigators tagged them. "Not verified" and "unknown" mark what the investigators did not check.

**Evidence basis.** Every finding on this page is static: decompile, cross-reference and memory-read results from three Ghidra copies of the ARM9 project. The one emulator check is the D-pad repeat timing (#175 P5, [below](#d-pad-repeat)); no emulator check was run for the rest of input, screens, sprites, fades or file loading. The frame cadence is assumed, not measured. Nothing here is compared against `pmd-red` except where a name is marked.

## Buttons

### Frame path

**[blue-verified]**, high. The VBlank IRQ handler is `FUN_02018ACC`. `FUN_02075C5C(1, FUN_02018ACC)` registers it in IRQ slot 0, and the literal `0x02000D60` holds `0x02018ACC`. [`execution-model.md`](execution-model.md) agrees: the handler is IRQ 0 (VBlank) and is `FUN_02018ACC`.

The handler calls `FUN_02017B20` (`UpdateKeyState`) each frame. That calls `FUN_02019130` (`ReadKeyInput`), which:

- reads `REG_KEYINPUT` (`0x04000130`, the only reader in the project);
- masks with `0x2FFF` (literal at `0x02019198`) and computes `((KEYINPUT | *0x027FFFA8) ^ 0x2FFF) & 0x2FFF`, then adds bit `0x8000` copied from `*0x027FFFA8`;
- adds the result into a running sum at `0x020DF89C`.

Hardware keys are active-low; the XOR makes them active-high. The OR with the word at `0x027FFFA8` clears bits rather than setting them. Keys are active-low, so a 1 bit in the word (bits 0 to 11 or 13) forces that key to "released". The mask `0x2FFF` **keeps** bits 10 and 11 (X `0x400`, Y `0x800`) and bit 13 (`0x2000`); it does not remove them (#175 P2b, **[blue-verified]**, static, high). A bit comes out pressed only if both `KEYINPUT` and the shadow word have 0 there, so X and Y reach the key path through the shadow word (see the X and Y note).

**Shadow word at `0x027FFFA8` (#175 P2).** The formula above is **[blue-verified]**, from the literal pool at `0x02019190`: `REG_KEYINPUT` `0x04000130`, the shadow word `0x027FFFA8`, mask `0x2FFF`, and the accumulator `0x020DF89C`. Bit 15 is copied from the shadow word. Its readers are **[blue-verified]**, and all test `0x8000`:
- `FUN_02019794` (literal read at `0x020197BC`): plays a sound (`FUN_0207d704`) when bit 15 is set and `DAT_02019820` is nonzero.
- `FUN_0207F418` (literal read at `0x0207F420`): calls `FUN_0207d9ec` when bit 15 is set.
- Overlay 7 wait loop `FUN_ov_0007__02139A98` (literal at `ov_0007::02139F4C`): exits early when bit 15 is set.

**[hypothesis]** (boot probe, py-desmume, scratch and not committed, so no command or output hash is recorded in the repo; this stays a hypothesis until the probe is committed or its command and output hash are recorded): the word is `0x0` at frame 0, then `0x2C00` from frame 15 to the end of the run, and it did not change across 547 or more `KEYINPUT` changes. The `KEYINPUT` values seen were `0x3FE`, `0x3F7` (Start at frame 900), `0x37F`, `0x3BF`, `0x3EF` and `0x3DF`. Bits 10, 11 and 13 of the word were 1 (`0x2C00`), which forces X, Y and bit 13 to released, so this word changed no key in that run. Bit 15 was never set, so the three bit-15 branches were not exercised. Confidence: high for the value and for no effect on keys in the boot run; medium for the meaning of the bits. **Writer: not established.** No writer is found in Blue's ARM9. The byte pattern `a8ff7f02` appears only as literals in the readers above, and the word is not in the static image, so a write from ARM7 or firmware is the likely source.

**Readers and writers of the shadow word (#175 P2b).** **[blue-verified]**, static, high. A word scan of every initialised block (ARM9 and all overlays) finds five literal-pool entries for `0x027FFFA8`, in four places, and every reference is a load: `FUN_02019130` (`ReadKeyInput`, `0x02019134`), `FUN_02019794` (`0x020197BC`), `FUN_0207F418` (`0x0207F420`) and `FUN_ov_0007__02139A98` (three references: `ov_0007::02139ADC`, `02139C54`, `02139DF4`). No ARM9 or overlay code stores to it, and the mirror `0x023FFFA8` is in no pool. So it is written from outside the imported program; the ARM7 is the likely writer (**[hypothesis]**: the ARM7 was not checked). Bit 15 is copied into the returned key value by `ReadKeyInput` (`0x0201915C` to `0x02019170`); ordinary key code does not test it afterwards. The three hooks above test `0x8000`: sound (`FUN_0207D704` with `0xC` or `0x1C`, chosen by `DAT_02019824`), `FUN_0207D9EC` followed by `FUN_0207F344(1, 1)` and `FUN_0207800C()`, and an early exit from the overlay 7 wait loop. A lid-closed or sleep signal fits this pattern (**[hypothesis]**, medium; the callees `FUN_0207D704` and `FUN_0207D9EC` were not read). `FUN_020191A0` toggles bit 15 of a different register, `0x04000204` (`REG_EXMEMCNT`), not the shadow word.

**Proposed signature (#175 P4, a proposal only; not applied to any decompile).** `void ReadKeyInput(ushort *out)`. The stored signature is `undefined FUN_02019130()`, while the decompile body is `void FUN_02019130(ushort *param_1)`. The body sets `*out` to the processed key word and also adds that word into the accumulator at `0x020DF89C` (read at `0x02019180`, write at `0x02019188`). The prototype's notes must record that side effect. The only caller is `FUN_02017B20` (`0x02017B38`), which passes the struct at `0x020F2790` and reads the output word back at once. Tags: **[blue-verified]** for the body; high for the prototype; medium on the accumulator's meaning.

### Key state

**[blue-verified]**, high. `UpdateKeyState` uses two 16-byte structs: the current one at `0x020F2790` and the previous one at `0x020F2780`. Current struct:

| Offset | Field | Meaning |
|--------|-------|---------|
| `+0` | held | active-high keys |
| `+2` | pressed | edge: `held & ~previous` |
| `+4` | repeat | the repeat output; zero unless a repeat fires |
| `+6` | flags | `0x2` = B tap, `0x100` = R tap |
| `+8` | D-pad group | the last D-pad group, `held & 0xF0` |
| `+0xC` | counter | int counter for the D-pad hold |

### D-pad repeat

**[blue-verified]** for the logic, high. The counter starts at 1 on a new direction and fires at 48. It then resets to 43, so it fires every 5 frames after the first fire, about 47 frames after the press. The timing assumes one call per VBlank.

**Measured (#175 P5, runtime, py-desmume).** `verification/v7-trace/dpad_repeat.py` boots with no input, holds Down from frame 800 for 200 frames and logs the key struct at `0x020F2790` every frame. Counting frames from the press: held and pressed are set at frame 0 and `repeat` (`+4`) is nonzero at frame 0 (counter 1), so the first direction fires at once; then `repeat` fires at frame 47 (the counter reaches 48 and is reset to 43) and every 5 frames after that (52, 57, 62, and so on to 197). So the static logic is confirmed (48, then 43, 5 frames apart, one call per frame), with the detail that the press frame itself also fires and the first repeat comes 47 frames later. Two runs gave identical logs; the command and hash are in [`verification.md`](../verification.md). Only Down was held; the other directions share the code (`held & 0xF0` is one group) but were not run.

### B and R taps

**[blue-verified]**, high. Counters at `0x020F276C` (B) and `0x020F276E` (R) set the tap flags in `+6`. A tap is a release after 2 to 11 frames.

### Accumulator

**[blue-verified]** for the structure, **medium** for the meaning. The accumulator at `0x020F2770` is ORed with the held keys each frame. Its int at `0x020F2768` is updated as `value *= (held | 0x054A1C41)`. The investigator reads this as an RNG mix, but the constant is not checked. Cross-check it with [`rng.md`](rng.md) before using it.

**The product at `0x020F2768` is not an RNG step (#175 P3).** **[hypothesis]**, high on the code and medium on the meaning. `rng.md` has no match: the global and dungeon LCGs use `0x5D588B65`, and `0x054A1C41` appears in no doc, Rust file or Ghidra label in the repo. The factor is odd, so the product is invertible mod `2^32`, but nothing in the imported program consumes it. The only reader and writer in the program are `FUN_02017B20` (read at `0x02017CCC`, write at `0x02017D14`), and `FUN_020181AC` writes the initial value `0x004A14C1` (write at `0x020181F4`, called from the boot path at `0x02000CDC`). Not checked: overlay or trace-harness reads.

**No consumer (#175 P2b).** **[blue-verified]**, static, high for literals and medium for computed access. A word scan of every block finds exactly two pool entries for `0x020F2768`: `FUN_02017B20` (`0x02017CC0`) and `FUN_020181AC` (`0x020181D0`, the init). In `FUN_02017B20` (`0x02017CC4` to `0x02017D14`) the word is multiplied by `heldKeys | 0x054A1C41` and stored back; the value is never compared, passed or copied afterwards. The accumulator at `0x020F2770` (8 bytes above) is used by `FUN_02017B20`, `FUN_02017D90`, `FUN_02017E60` and `FUN_020181AC`, all with positive offsets, so none reaches `0x020F2768` through it. No overlay has a pool entry for `0x020F2768` or `0x054A1C41` (the constant appears once, at `0x02017D30`). So the product is write-only in this build. Not covered: accesses through registers loaded from something other than a literal pool.

### Snapshot and consumers

**[blue-verified]**, high. `FUN_02017E60` (`LoadBufferedInputs`, the recorded name; frame action #9 in [`execution-model.md`](execution-model.md); see [Frame action names](execution-model.md#frame-action-names-161-p4)) copies the accumulator to `0x020F27A0` and clears it. About 40 functions read `0x020F27A0`, in the ARM9 and in overlays 0, 1 and 7. Its touch work is in [Touch](#touch).

- **Title loop.** `GameLoop_Async` uses the L button (`0x200`) as a pressed edge and as held state, to call `FUN_02019634(0x13)` and `FUN_02019634(0)`. This is the title loop's L gate.
- **Dungeon directions.** `ResolveDungeonDirection` (`FUN_020271E0`, **[blue-verified]**, high) maps the D-pad to a direction code. Down `0x80` → 1, Right `0x10` → 2, Up `0x40` → 4, Left `0x20` → 8. That code indexes a 16-byte table at `0x020B219C`, which gives 8 directions without R: 0 Down, 1 DownRight, 2 Right, 3 UpRight, 4 Up, 5 UpLeft, 6 Left, 7 DownLeft.
- **R for diagonals.** With R held (`0x100`) the lookup uses the table at `0x020B218C`, which gives only the diagonals.
- **Emission.** A direction is emitted on the 4th identical frame, then on every frame while held (debounce counter `> 2`).
- **Not mapped here.** The button actions are in [Dungeon reader actions](#dungeon-reader-actions-175-p1-a) and [Menu readers and helpers](#menu-readers-and-helpers-175-p1-b). See [Open questions](#open-questions).

### Dungeon reader actions (#175 P1 a)

Scope: the dungeon main-loop reader `FUN_02166B10` (`ov_0001::02166B10`, 727 lines in the decompile). A, Select, R and L are covered only where they gate one of B, Start, X or Y. Static decompile (Ghidra copy `rescue-arm9-a`), compared with pmd-red `dungeon_main.c`. Tags: **[blue-verified]** for the tests and branches; **[hypothesis]** for the action names; **[open]** for the callees.

**Input source (corrected).** The reader reads the snapshot at `0x020F27A0`, not the current key struct at `0x020F2790`. The overlay literal `ov_0001::02167AA8` holds `0x020F27A0` (see [Snapshot and consumers](#snapshot-and-consumers)). An earlier pass read `0x020F2790`; that was wrong. The snapshot layout is the same as the current struct: `+0` held, `+2` pressed, `+6` flags (`0x2` B tap, `0x100` R tap). The bit values below are the same for both.

**Bit values** (the reader's tests; they match pmd-red `include/gba/io_reg.h` 702 to 717): A `0x1`, B `0x2`, Select `0x4`, Start `0x8`, R `0x100`, L `0x200`, X `0x400`, Y `0x800`.

| Button | Test in the reader | Action (structure) | Tag |
|--------|--------------------|--------------------|-----|
| **A + B** (held `0x3`) | held `0x3` and the `FUN_0200B9A4` check (belly above 0) | sets the leader action to pass turn (`FUN_021418D4(1)`) and the flag at `0x663`. Matches `dungeon_main.c` 176 to 180 | blue-verified; high |
| **Select** (pressed `0x4`) | gated by `+0x15FA2 == 0` and mode byte not 6 | opens the floor map: a loop `FUN_0213AC4C(10, 0x2F)`. A toggles the byte at `0x021B4210`; B or Select leaves. Matches `dungeon_main.c` 371 to 383 | blue-verified; high |
| **B** (pressed `0x2`, mode 0, `iVar21+0x32 != 0`) | lines 258 to 264 | calls `FUN_021480E4()`, clears `0x021B4768`, calls `FUN_02017DBC` and `FUN_02017D34`. A cancel, by behaviour | blue-verified (structure); [hypothesis] for the name |
| **B tap** (`flags & 0x2`) | lines 218 to 223 | leaves the loop with action 0 | blue-verified; Red's `shortPress & B` (`dungeon_main.c` 252) not compared for the action |
| **L + B** (held `0x200`, pressed `0x2`) | lines 271 to 285 | calls `FUN_0200C538`, `FUN_021552D4`, `FUN_0215550C`, `FUN_0215570C(0)`, `FUN_021558CC`, then `FUN_02017DBC` and `FUN_02017D34` | blue-verified (structure); callees [open]; Red condition at `dungeon_main.c` 302 to 303 matches |
| **Start** (pressed `0x8`) | `bVar23` (line 299), mode byte `PTR_DAT_02167AAC+6` is 0 | calls `FUN_02166530(leader)`, sets the flag at `0x021B4768` to 1, copies the slot from `info+0x44` to `iVar21+0x33`, sets `+0x34` to `0xFF`, then `FUN_02017DBC()`. Does nothing here when the mode byte is not 0 | blue-verified (structure); [open] for `FUN_02166530` |
| **R tap** (`flags & 0x100`) | the same gate as Start (`bVar23`, line 311) | the same block as Start. Matches `dungeon_main.c` 323 for the gate; the action is not verified | blue-verified (gate); medium (action) |
| **Y** (pressed `0x800`) | lines 188 to 189 (mode 0, sets `bVar6`); lines 266 to 270 (mode not 0, held `0x800`); line 304 (mode not 0, pressed `0x800`) | mode 0: same block as Start. Mode not 0: calls `FUN_021480E4` and clears `0x021B4768` | blue-verified (code); see the X and Y note |
| **X** (pressed `0x400`) | lines 212 to 217, only when A is not pressed and no touch or other trigger is set | leaves the loop with action 0, the same exit as B tap | blue-verified (code); see the X and Y note |

**X and Y note (corrected, #175 P2b).** The reader tests X (`0x400`) and Y (`0x800`) in the key struct. `ReadKeyInput` does not mask them out: `((KEYINPUT | shadow) ^ 0x2FFF) & 0x2FFF` keeps bits 10 and 11. They arrive through the shadow word `0x027FFFA8`: a key is pressed only if both `KEYINPUT` and the shadow word have 0 in that bit. **No ARM9 or overlay code reads `REG_EXTKEYIN` (`0x04000136`)**: no literal-pool entry holds `0x04000136`, and a scan of 282,222 instructions for the immediates `0x136` and `0x04000136` finds 10 hits, all byte accesses at offset `0x136` of a struct (entity info `+0x136`), none from the I/O page. **[blue-verified]**, static, high for the formula and for the absence of reads. **[hypothesis]**, medium: the ARM7 fills the shadow word from `REG_EXTKEYIN` (X bit 10, Y bit 11, pen-down bit 13, hinge bit 15, all active-low). In the boot run the shadow word was `0x2C00` (bits 10, 11 and 13 set), which would force X and Y to released; whether that value changes while the console runs, and the idle value of `KEYINPUT` bits 10 and 11 on the ARM9, are **not verified**. A runtime probe (scratch, not committed) saw no `0x400` or `0x800` over 25,100 frames, but its boot inputs never pressed X or Y. Pressing X and Y in a probe would settle it.

**Other gates used above.** A (pressed `0x1`) is tested at lines 210 to 256, when A is not pressed, for the other triggers and action codes 0 or 1. A is also tested at line 357 in the Select loop.

**Runtime (scratch probe, not committed, so no command or hash is recorded).** Boot inputs to frame 25,100 show Start `0x8` (frames 900, 1020, 1300, 1360) and A `0x1` (frames 1140, 1330) in the snapshot, with no B or R tap. So B, B tap, R tap, L + B and the cancel path were not observed. **[hypothesis]** for the action names; the callees are not read.

**Not verified.** The callees `FUN_02166530`, `FUN_021480E4`, `FUN_021418D4`, `FUN_0213AC4C`, `FUN_0200C538`, `FUN_021552D4`, `FUN_0215550C`, `FUN_0215570C`, `FUN_021558CC`. The branch for B and Start when the mode byte is not 0 (line 297 and line 304 were read, not run). Whether B, Start and the other buttons do the same in the menu overlays (see below).

### Menu readers and helpers (#175 P1 b)

**Status: partial.** The 39 functions in overlay 1 that load the snapshot pointer are listed. 11 show their bit tests in the table below; the other 28 are classed by their keys in [The 28 other readers](#the-28-other-readers-175-p1-c). Menu names for them are hypotheses. Tags: **[blue-verified]** for the pointer, the bit tests and the call shapes; **[hypothesis]** for every behaviour label and every pmd-red match.

**Snapshot pointer.** `FUN_02017E60` (frame action #9) copies the accumulator at `0x020F2770` to `0x020F27A0` and clears the accumulator's held, pressed and flags words. Literal pool: `0x02018194` gives `0x020F2770`; `0x020181A0` gives `0x020F27A0`. Pmd-red's `gRealInputs` (`include/input.h`, 0x10 bytes) matches the current key struct at `0x020F2790` field for field; the snapshot is a separate copy that the menus and the dungeon reader read. The overlay-1 literal `ov_0001::02167AA8` holds `0x020F27A0`, and `FUN_02166B10` loads it in 22 places (inlined copies). Pool scan (`a0 27 0f 02`) finds 39 functions in overlay 1 that load the pointer; the list is in the investigator's scratch output, not in the repo.

**Direct readers.** These read snapshot bits in the function itself.

| Reader | Bits tested | Named callees | Behaviour (hypothesis) | Closest pmd-red (hypothesis) | Confidence |
|--------|-------------|---------------|------------------------|------------------------------|------------|
| `ov_0001::0215325C` | pressed `0x3` (A and B) | `DungeonRunFrameActions` | A/B prompt loop | none found | low |
| `ov_0001::021552D4` | pressed A and B | `DungeonRunFrameActions` | A/B prompt loop; `FUN_02166B10` calls it on L + B | none found | low |
| `ov_0001::0216B130` | pressed A and B | `GetLanguage` | A/B prompt | none found | low |
| `ov_0001::0216B2C0` | pressed A and B | `GetLanguage` | A/B prompt | none found | low |
| `ov_0001::0216AD10` | pressed A and B; held `+4 & 0x40` and `0x80` (repeat up and down) | `GetLeader`, `EntityIsValid` | list with cursor, leader-related | `ShowDungeonToWhichMonMenu` (team), by the leader and entity calls | low |
| `ov_0001::02174B8C` | pressed A, B, `0x10` and `0x20` (Right and Left); repeat up and down | `GetLanguage` | list with cursor and horizontal paging | none found | low |
| `ov_0001::02175178` | pressed A and B; repeat up and down | `GetLanguage` | list with cursor (7 pools) | none found | low |
| `ov_0001::0216F550` | pressed A and B | `GetTile`, `GetEntityType` | tile or item description prompt | `ShowTileDescription` (tile), by `GetTile` | low |
| `ov_0001::0216FA78` | pressed A and B | `GetFloorType` | floor-based prompt | `ShowDungeonStairsMenu` or `ShowStairsDescription` (stairs), by floor type | low |
| `ov_0001::021729FC` | pressed `0x30`, `0xC` and B | `GetLanguage` | page switch (Left and Right, Select and Start) | none found | low |
| `ov_0001::02166B10` (dungeon main reader) | held A, B, L, R, `0x800`; pressed A, B, `0x4`, `0x8`, `0x100`, `0x400`, `0x800`; tap `+6 & 0x2` | `GetTileMut`, `IsFloorOver` | dungeon main loop, not a menu | none (dungeon loop) | see dungeon reader actions |

Also `FUN_02165C80` (`ov_0001::02165EC4`) tests held B (`+0 & 2`) directly. Its callees are `GetEntityType`, `GetLanguage` and `GetTile`; it is not matched to a Red function.

Read the table as follows. The six A/B prompts are small confirm or cancel windows. The three with up and down repeat are list menus; `ov_0001::0216AD10` is the candidate for the team "which monster" menu. The rest are short choice windows. None matches `ShowDungeonTeamMenu`, `ShowDungeonMovesMenu`, `ShowDungeonItemsMenu` or `ShowDungeonOthersMenu` yet, because those are probably in the 28 unmapped readers. The dungeon reader is listed as `ov_0001::02166EC0` in one row of the investigator's table; the same function is `FUN_02166B10` everywhere else, so the row is a typo [open].

**Shared helpers.** The readers call the same helpers. Shared callees by reader count: `DungeonRunFrameActions` (26), `FUN_ov_0001__0219CEEC` (21), `FUN_ov_0001__0219CED4` (20), `FUN_ov_0001__0219CEA4` (19), `FUN_0202D230` (16), `FUN_0202C560` (15), `FUN_0202C5E4` (15), `FUN_ov_0001__0213A37C` (12), `FUN_ov_0001__0219CE90` (9). None of these loads the snapshot address, and none is among the 39 readers, so each reader reads the snapshot inline and then calls a helper to draw, wait or step.

| Helper | Body (call shape) | Name (hypothesis) | Confidence |
|--------|-------------------|-------------------|------------|
| `ov_0001::0219CEA4` | calls `PTR_FUN_0219CEB4(DAT_0219CEB8, 0x100)`; the target is `FUN_0201BC30`, the id is `0x12F` | `PlayMenuSEBack` | hypothesis, medium; back or denied |
| `ov_0001::0219CED4` | calls `PTR_FUN_0219CEE4(DAT_0219CEE8, 0x100)`; the target is `FUN_0201BC30`, the id is `0x12E` | `PlayMenuSEAccept` | hypothesis, medium; accept or confirm |
| `ov_0001::0219CE90` | calls `PTR_FUN_0219CEA0(0x130, 0x100)`; the target is `FUN_0201BC30`, the id is `0x130` | `PlayMenuSEAltAccept` | hypothesis, medium; alternate accept (Start). `0x130` is a sound id, not `REG_KEYINPUT` |
| `ov_0001::0219CEEC` | if the argument is 0 or the count is above 1, calls `FUN_0201BC30(DAT_0219CF24, 0x100)` (id `0x12D`) | `PlayMenuSECursor` | hypothesis, medium; cursor move, used with Up and Down |
| `ov_0001::0219CD64` | if the argument differs from `DAT_0219CD88`, calls `FUN_0201BC30(arg, 0x100)` | `PlayMenuSEIfChanged` | hypothesis, medium; generic |
| `ov_0001::0213AC4C` (`FUN_0213AC4C`) | called as `(10, 0x2F)` in the Select loop and `(4, 0x3E)` after the menu | `RunMenuLoop` | low; the arguments are not read |

Not named: `FUN_0202D230`, `FUN_0202D240`, `FUN_0202C560`, `FUN_0202C5E4`, `FUN_0202DE34` and `FUN_0213A37C`. Their bodies were not read.

**The helper pointers (#175 P1c, corrected).** **[blue-verified]**, high for the target and the routine. All three pointer words (`ov_0001::0219CEB4`, `0219CEE4`, `0219CEA0`) hold `0x0201BC30`, `FUN_0201BC30`, and their second arguments (`0x12F`, `0x12E`, `0x130`) are sound ids, not key registers. `FUN_0201BC30(id, volume)` is a play-sound routine: it returns when `id == DAT_0201BD78` (no sound), clamps the volume to `0x100`, looks the id up in a table at `DAT_0201BD84 + (id - 300)` (ids from 300 are effects), picks one of 5 channels (`min(4, table byte)`) and starts a voice (`FUN_0201B368`, with the mutex `FUN_02076FF0` and `FUN_02076F80`). The earlier guess that `0x130` is the low part of `REG_KEYINPUT` was wrong. These helpers are the menu sound layer, not the input layer. The sound ids' roles in the table above are **[hypothesis]**, medium, from where each is called.

### The 28 other readers (#175 P1c)

**Snapshot layout seen by the menus.** **[blue-verified]**, high. The snapshot pointer is used with `+0` held, `+2` pressed, `+4` held with repeat (Up, Down, Left and Right autorepeat) and `+6` flags. Bits, DS order: `1` A, `2` B, `4` Select, `8` Start, `0x10` Right, `0x20` Left, `0x40` Up, `0x80` Down, `0x100` R, `0x200` L, `0x400` X, `0x800` Y. Almost every menu also tests four touch-button flags in its widget struct, ORed with the key tests: `+0x27` confirm, `+0x28` cancel, `+0x29` right or next, `+0x2A` left or previous. The cursor widget functions are `FUN_0202D230` (draw and update), `FUN_0202C560` (cursor up) and `FUN_0202C5E4` (cursor down).

The 28 readers are `0213AC1C`, `02154580`, `02154EDC`, `02155054`, `021664B0`, `02168774`, `0216A0D8`, `0216AFE8`, `0216B458`, `0216B958`, `0216C638`, `0216D194`, `0216D77C`, `0216E2D8`, `0216EBFC`, `0216F898`, `0216FDA0`, `0216FF80`, `02170398`, `0217185C`, `02171AA8`, `02171BD8`, `02171D00`, `02171E40`, `02172498`, `021725BC`, `0219D140` and `02165C80`. Keys are pressed (`+2`) unless noted. Key tests **[blue-verified]**, high; the "what they are" column **[hypothesis]**, low to medium.

| Group | Readers | Keys | What they are |
|-------|---------|------|---------------|
| list: A, B, Up/Down repeat | `0216C638`, `0216F898`, `0216FDA0`, `02170398`, `0217185C`, `02171D00`, `02171E40` | A accept (SE `0x12E`), B cancel (SE `0x12F`), repeat Up `0x40` and Down `0x80` move the cursor (SE `0x12D`) | plain vertical choice lists |
| list + horizontal repeat | `02171AA8`, `02171BD8` | as above, plus repeat `0x10` and `0x20` | choice lists with two columns |
| paged list | `0216A0D8`, `0216B958` | A, B, pressed Right `0x10` and Left `0x20` = next and previous page, Up/Down repeat | `0216A0D8` reads `gTeamInventoryRef` (3-byte items) and each member's `+0x5A` held item, and tests pressed `4` and `8`: the items menu (closest Red `ShowDungeonItemsMenu`). `0216B958` (483 lines, no inventory reads) is a second paged list |
| A, B, Start | `0216D194` | A accept, Start `8` alternate accept (SE `0x130` if the entry is enabled, else SE `0x12F`), B cancel; calls `GetLeader` | leader-related list, possibly the team menu |
| A, X, B | `02168774` | A or X (`0x400`) both accept; B cancel; Up/Down repeat; calls `GetLeader`, `GetTile`, `GetEntityType` | item or tile action list; X duplicates A (Red `ShowDungeonOthersMenu` or an item action list is a guess) |
| B + Up/Down | `0216D77C` | B, Up/Down repeat, no A | scroll view |
| A/B only | `0216AFE8`, `0216B458`, `02154580`, `0219D140` | `0216AFE8` and `0216B458`: A and B; `02154580`: pressed `&3` and a stylus coordinate check; `0219D140`: A only | confirm prompts and message waits |
| R held + direction | `0216EBFC` | held R `0x100` with pressed `0x10` to `0x80`, and the move array at info `+0xF4` (`FUN_0200AC60`, `FUN_0200B110`) | the moves menu with reorder (closest Red `ShowDungeonMovesMenu`) |
| wait loops | `0213AC1C`, `021664B0`, `02154EDC`, `02155054`, `02172498`, `021725BC` | `0213AC1C`: wait until held `&0xF0` is released; `021664B0`: wait while held R (`gDungeon+0x65C/0x65D`); `02154EDC`, `02155054`: message-log scroll on repeat Down and Up; `02172498`, `021725BC`: one-shot message, repeat Down | helpers, not menus |
| pressed D-pad + A/B (touch widgets) | `0216E2D8`, `0216FF80` | pressed A, B, Right and Left with the touch flags `+0x27` to `+0x2A`; `0216FF80` reads a team flag next to `gDungeon` at `+6` | page or tab dialogs |
| held B | `02165C80` | held `&2` | reads tile and entity (`GetTile`) while B is held; not a menu |

Select appears only as pressed `4` in `0216A0D8`, and Start only as pressed `8` in `0216A0D8`, `0216D194` and `021729FC`. Mapping to Red `dungeon_menu_*.c`: the moves menu (`0216EBFC`) and the items menu (`0216A0D8`) are reasonably supported; `02168774` and `0216D194` (leader, tile and entity calls; X and Start accept) are weaker guesses for the team, others or tile-action menus; the rest are generic lists and prompts, and `0216B958` has no Red match.

**Not verified.** Any menu at run time (the boot run reaches the dungeon, not the menus); which of `02168774` and `0216D194` is the team menu; the Select and Start effects in `0216A0D8`; the Red-to-Blue mapping for every menu. The label of `ov_0001::021729FC` in the investigator's comment is garbled, and its address is used here [open].

## Touch

### Sample pipeline

**[blue-verified]** for the structure, **medium** for the interpretation.

- `FUN_0207D150` starts channel 6 through `FUN_02079CA0` and `FUN_02079CC8`. Its handler `FUN_0207D1E0` runs for command `0x10`. It decodes a packed 32-bit sample from the shared words `*0x0207D49C` and `*0x0207D498` into an 8-byte ring-buffer entry. The low 12 bits are the raw X. The other fields are not decoded.
- `FUN_0207CE64`, and `FUN_0207CE04` through `FUN_0207CDE4`, read the sample struct at `0x02137354`. A sample is valid if `+0x34 & 1 == 0`. They return X and Y from `+4` and `+6`, and a pen flag from `+8`.

### Calibration

**[blue-verified]** for the control flow. `CalibrateTouch` (`FUN_0207CA44`) passes raw coordinates through when the flag at `DAT_0207CB6C+0x30` is zero. Otherwise it applies an affine transform, then clamps X to `0..0xFF` and Y to `0..0xBF`. That is a 256 × 192 screen. The parameter block is elsewhere than the first reading suggested (#176 P1):

- `0x0207CB70` is a literal-pool word, not the parameter block. It holds `0x0213736C`, the block's address. `CalibrateTouch` reads the four parameters through this pointer (decompile at `0x0207CAB4`). **[blue-verified]**
- `DAT_0207CB6C` holds `0x02137354`, the touch-sample struct. Its enable flag is at `+0x30`, which is `0x02137384`. **[blue-verified]**
- The block is written only by `FUN_0207CF5C` (`0x0207CF5C`), called once at boot from `0x02000D24` (in the boot routine described in [`execution-model.md`](execution-model.md#start-up-and-the-title-menu)). It writes `0x0213736C` and `0x02137370`, sets `+0x30` to 1, and clears it when its argument is null. The writes go through the DS divider (busy-wait on `0x8000`). **[blue-verified]**
- The boot code at `0x02000CFC` to `0x02000D24` calls `FUN_0207D0A4(sp+0xE)` and passes that buffer to `FUN_0207CF5C` when the call returns nonzero. **[blue-verified]**
- `FUN_0207D0A4` (`0x0207D0A4`) reads a 9-field structure at `0x027FFC80` (through the pointer at `0x0207D14C`): raw X1 at `+0x58`, raw Y1 at `+0x5A`, screen X1 at `+0x5C` (u8), screen Y1 at `+0x5D` (u8), raw X2 at `+0x5E`, raw Y2 at `+0x60`, screen X2 at `+0x62` (u8), screen Y2 at `+0x63` (u8). The field order is from the call to `FUN_0207CB74`. **[blue-verified]**
- `FUN_0207CB74` (`0x0207CB74`) takes the two point pairs. It checks ranges (raw `<= 0xFFF`, screen X `<= 0xFF`, Y `<= 0xBF`, no equal points), computes the scale and offset with the divider, and returns 0 on success. If all four raw fields are 0, or the call fails, `FUN_0207D0A4` returns zeros. **[blue-verified]**
- **Source of the points:** the ARM9 code only reads `0x027FFC80`. Its only ARM9 reader is `FUN_0207D0A4` (`0x0207D0B4`, `0x0207D0C8`), and no ARM9 code writes it, so the two calibration points are loaded from outside the ARM9 program. The usual source is the DS user-information area that the firmware loader copies into main RAM. **[hypothesis]** for that identity: NitroSDK's user-info layout was not checked against a header, so the field names are ours. **[open]:** the fallback when the firmware data is empty. The code returns zeros, but `FUN_0207CF5C` still sets the enable flag, so the effect on a touch is not traced.

### Touch state

**[blue-verified]**, high. `FUN_02017E60` writes the touch struct at `0x020F27B0`. The names are the investigator's own.

| Offset | Field | Meaning |
|--------|-------|---------|
| `+0` / `+2` | position | X and Y; `0xFFFF` = no touch |
| `+8` / `+A` | tap | tap output |
| `+0x10` / `+0x12` | drag / hold | drag and hold output |
| `+0x14` / `+0x16` | hold point | the point where the hold started |
| `+0x20` | state | 0 idle, 1 pressed, 2 released, 3 held, 4 drag, 5 end |

Thresholds in the findings: a move under 7 px, and frame counts of 11 and 12. The findings do not say which count gates which transition.

### Consumers

**[blue-verified]** for the call sites; **medium** for the meaning of the gate.

- `FUN_0200C354` returns a valid touch. It is gated by the byte at `0x020DED14+7`. It is called from overlay 0.
- `FUN_0200C4F4` is the position getter, with the same gate. It has about 10 callers, including overlay 1.
- The gate byte is the `touchScreen` option, a byte in the options struct (#176 P3, below).
- In the dungeon, a touch within ±10 px of an active slot (flag `0x8000`, stride `0x3C`, 8 slots) returns that slot index (`ResolveDungeonDirection`, above). The meaning of the actor fields `+0x4DD4` to `+0x4DDA` is not verified.

### Touch-only and touch-optional consumers (#176 P2)

Touch consumers, from x-refs and the decompile of each caller. `FUN_0200C4F4` is the position getter and `FUN_0200C354` the valid-touch gate. **[blue-verified]** for the call sites and the button bits tested in each function. A **touch-only** label means no button bit is tested in that function; the caller was not checked, so the label is **[hypothesis]** for any function marked "candidate".

| Caller | Overlay | What it does with the touch | Button bits tested in the same function | Classification |
|--------|---------|------------------------------|------------------------------------------|----------------|
| `FUN_0202B3B4` | ARM9 | Hit test of the touch point against a list of rectangles (5-short entries); returns the entry id | none | touch-only (this function); caller not checked |
| `FUN_02026F5C` | ARM9 | Tests whether the touch is inside a region | none | touch-only (this function); caller not checked |
| `FUN_0202B474` | ARM9 | Touch read; 40 lines | none | touch-only ([hypothesis]; candidate, not read in full) |
| `FUN_0202CB20` | ARM9 | Touch read; 59 lines | none | touch-only ([hypothesis]; candidate, not read in full) |
| `FUN_0202BDF4` | ARM9 | Touch read | one test on bit `0x8` (Start, if read as a button) | touch-optional ([hypothesis]; candidate) |
| `FUN_0202CE90` | ARM9 | Touch read; 100 lines | one test on bit `0x8` | touch-optional ([hypothesis]; candidate) |
| `FUN_02030944` | ARM9 | Touch read; 108 lines | two tests on bit `0x8` | touch-optional ([hypothesis]; candidate) |
| `ov_0000::0213F7D8` | overlay 0 (ground) | Touch read, 3 calls | A, B, X bits (`0x1`, `0x2`, `0x400`) | touch-optional |
| `ov_0001::02165320` | overlay 1 (dungeon) | Touch read, direction resolve (`ResolveDungeonDirection`) | A bit (`0x1`) | touch-optional |

- **The ground gate.** `FUN_0200C354` has one caller, `ov_0000::0213F7D8`. Its gate is the byte at `0x020DED14+7`. That byte gates all touch input in ground mode, so ground touch input is switched off by the options byte. The byte is named `touchScreen` (#176 P3, below).
- **Dungeon.** `ResolveDungeonDirection` gives a touch direction, and the dungeon reader (`ov_0001::02166B10`, see [#175](https://github.com/TorratDev/rust-mm/issues/175)) also reads the button bits. Touch is an alternative to the buttons, not the only input. **[blue-verified]** for the call sites; medium for the classification.

**[open]** (not verified):
- Which feature each ARM9 consumer belongs to. The names are `FUN_`, and [`symbols.md`](../symbols.md) names none of them.
- Whether `FUN_0202B474` and `FUN_0202CB20` have a button path in their callers.
- Whether bit `0x8` in the three ARM9 consumers is Start or another flag.
- Runtime: none. No Blue dump is configured in this session.

### Options gate: touchScreen (#176 P3)

The gate byte is `touchScreen`, the DS touch-screen option. **[blue-verified]** for the gate, the pointer, the struct and the field order. The name is high confidence: it rests on the field-order match with pmd-red's `ReadGameOptionsBits`.

- **The gate.** `FUN_0200C354` returns 1 only when the byte at `options + 7` is nonzero and the touch position is not `0xFFFF`. **[blue-verified]**
- **The pointer.** `0x020DED14` is a global slot that holds the options struct pointer. `DAT_0200C394` = `0x020DED14`. It is zero in the static image, so it is set at run time. **[blue-verified]**
- **The struct.** `FUN_0200CB28` (`0x0200CB28`; `0x0200CB30` lies inside it) stores `0x020E7BA8` into the slot, so the struct base is `0x020E7BA8` (zero-initialised memory). `FUN_0202A778` also writes the slot. **[blue-verified]**
- **The name.** The save reader `FUN_0200C6B8` (called from `RestoreAdventureData`, `0x02010F1C`) reads the options bit stream in this order: `+5` (2 bits), `+6` (1 bit), `+7` (1 bit), `+8` (1 bit), `+0`, `+1`, `+2`, `+3` (1 bit each), `+4` (3 bits), `+9` (2 bits). Pmd-red's `ReadGameOptionsBits` (`src/game_options.c`) reads `windowColor` (2), `controls` (1), `touchScreen` (1), `playerGender` (1), `dungeonSpeed`, `FarOffPals`, `damageTurn`, `gridEnable`, `mapOption` (3) and `unkC` (2). The order matches field for field, so `+7` is `touchScreen`. **[blue-verified]** for the order; high for the name.
- **Other writers.** The full list of writers, with the value each one stores, is in [Writers of options+7](#writers-of-options7-176-p3b). The default writer and the restore reader are both there. The earlier note gave the default writer as the path `0x0200CB14`. Resolved: `0x0200CB14` is the literal pool of `FUN_0200CAA4` (READ xrefs), so the function starts at `0x0200CAA4`, with the store near `0x0200CAF0`. The reviewer verified this on `rescue-arm9-c`. **[blue-verified]**
- **Default mismatch.** Blue's default for `+7` is 1. Pmd-red's `InitializeGameOptions` sets `touchScreen = 0`. Blue's `+6` (`controls`) is also 1, where Red's default `CONTROLS_GBA` is 0. **[open]:** either Blue's defaults differ or the enum values differ.

### Writers of options+7 (#176 P3b)

**[blue-verified]** for the stores, the copy loops and the x-ref coverage, unless a row says otherwise. The source is an investigator's pass over a Ghidra copy (`rescue-arm9-a`), not the repo's `rescue-arm9` project. Nothing was written to a project.

Method: an x-ref to the slot `0x020DED14` (59 functions, each decompiled and searched for stores to `+7`); a byte search for the slot literal `14 ED 0D 02` (61 literal pools, each mapped to a decompiled function except two in unanalysed code, `0x0200C348` and `0x0203CD20`, decoded by hand from the raw bytes); an x-ref to the struct base `0x020E7BA8`; and a decompile of the menu family `0x0203C000` to `0x0203E800`.

| # | Writer | Address | Value written to `+7` | Context | Tag |
|---|--------|---------|-----------------------|---------|-----|
| 1 | `FUN_0200CAA4` (`InitializeGameOptions`) | store near `0x0200CAF0` | constant `1` | Also sets `+5 = 0`, `+6 = 1`, `+0 = 0`, `+1` to `+4 = 1`, `+9 = 0`; `+8 = 0` only if the argument is nonzero. Callers: `FUN_0201F2E4` at `0x0201F348` (boot, argument 1; `FUN_0201F2E4` is called from `0x02000D2C`); `FUN_0202A90C` at `0x0202A938` (argument 1; called from `FUN_0202A98C` and `FUN_0202A9C4`); `FUN_ov_0001__02171960` at `ov_0001::02171994` (argument 0, only when `FUN_ov_0001__02153e80(...) == 1`; called from `FUN_ov_0001__02171D00`). | blue-verified |
| 2 | `FUN_0200C6B8` (save bit-stream reader, called from `RestoreAdventureData` at `0x02010F1C`) | `0x0200C6B8` | one bit from the stream (0 or 1) | Third field read, after `+5` (2 bits) and `+6` (1 bit). | blue-verified |
| 3 | Options menu commit, in unanalysed code at `0x0203CC6C` (no function is defined there, and no x-ref reaches it, so it is called through a table or a pointer) | `0x0203CCA4` to `0x0203CCC8` | the menu working copy's `+7` (0 or 1), as part of a 10-byte block copy | When the status from `0x0202ADAC` (called at `0x0203CC78`, result on the stack) is **4**, it copies 10 bytes from `(*0x020E0584) + 8` to `*0x020DED14` with a byte loop. Statuses 1 and 5 call `FUN_0203D358(1)` and copy nothing (cancel). | copy loop and status test blue-verified; **[hypothesis]** for the call targets. The identity of `0x0202ADAC`, and what status 4 means, are not traced. |
| 4 | Menu working-copy edit: `FUN_0203D8B0` (menu input handler) | store sites `*(char*)(*(int*)*DAT_0203dd6c + 7)` in the decompile | `1` or `0`, or a toggle `(old == 0)` | Writes the **working copy**, not the live byte. The row index is `state[10]` (u16 at `+0x28`); row 2 is touchScreen. Touch hit test `FUN_0202B474(state+0x10, DAT_0203DD70)`: hit 6 sets 1, hit 7 sets 0. On row 2, a D-pad left or right (`FUN_0202DB40` returns 9 or 10) toggles `+7 = (+7 == 0)`. | blue-verified |
| 5 | Menu open: `FUN_0203D00C`, case 6 | `0x0203D00C` | none (reverse direction) | Copies the live options (`*0x020DED14`, 10 bytes) into the menu state at `+8`, then calls `FUN_0203DD74(state+8)`. This seeds the working copy; it does not write the live byte. | blue-verified |

- **State.** The menu state pointer is `*0x020E0584`, set by `FUN_0203D430`, which allocates 0x1B0 bytes. The working copy is the 10 bytes at state `+8`. The inner menu object (allocated by `FUN_0203DD74`, 0xA8 bytes) points to it. **[blue-verified]**
- **Rows.** The draw routine `FUN_0203D464` shows row 2 (`FUN_0202C488(menu+0x10, 2)`) with value 0 at x `0x50` and nonzero at x `0x88` (left and right choice). Row 1 is `+6`, row 0 is `+5` (3 values), row 3 is `+9` (2 values). **[blue-verified]** for the layout. The menu uses only 0 and 1 for `+7`. Which label is On and which is Off is **not verified** (the string ids in `FUN_0203D464` were not read).
- **Pointer swaps, not byte stores.** `FUN_0202A778` swaps the pointer in the slot. With a table argument it stores `table[0xB]` (WRITE xrefs `0x0202A8BC` and `0x0202A820`); with null it stores `FUN_0200CB1C()`, which is `0x020E7BA8`. `FUN_020660E4` (called from `FUN_02065CFC`) builds a table of 13 heap buffers, where entry 0xB is a 10-byte options buffer, and calls `FUN_0202A778(table)`; `FUN_02065F88` frees them. During that window the slot points at the 10-byte scratch buffer, not at `0x020E7BA8`. The purpose of that window (a scratch save-data set read by `FUN_0202A1AC`) is **not traced**. **[blue-verified]** for the swap.
- **Boot store of the struct base.** `FUN_0200CB28` (`0x0200CB28`, called from `FUN_0201F2E4` at `0x0201F310`) stores `0x020E7BA8` into the slot. The struct-base x-refs are only `0x0200CB1C` and `0x0200CB30`, both DATA. The earlier note named `LoadGameOptions` (`0x0200CB30`) as the store. Resolved: `0x0200CB30` lies inside `FUN_0200CB28`, so that function is the store, and the earlier attribution to `LoadGameOptions` was wrong. The reviewer verified this on `rescue-arm9-c`. **[blue-verified]**
- **Readers that do not use the byte.** No other function stores to `+7` of the slot's struct. Seven readers (`FUN_0201F268`, `FUN_020282FC`, `FUN_02028C60`, `FUN_0202DCD4`, `FUN_ov_0001__0213AC7C`, `FUN_ov_0001__0213AD54`, `FUN_ov_0007__021393B0`) pass the struct to `FUN_02012310`, which is an empty function (it only returns), or test it for the touch gate. **[blue-verified]**
- **Values.** Default: `+7 = 1` on all three non-user paths (boot, `0x0202A938`, and `ov_0001::02171994` with argument 0); `+6` is also 1. Menu: 0 or 1 only. Restore: whatever the save holds (one bit).
- **[open]** The call targets in the unanalysed block at `0x0203CC6C` (the function there is undefined), the identity of `0x0202ADAC`, and the purpose of the scratch buffer from `FUN_020660E4`.

### ARM9 sample decode and the ARM7 side (#176 P4)

**ARM7 side of channel 6: [open], blocked.** The ARM7 binary is not in the copy-A project (only `arm9.bin` is imported), and no dump is configured to extract it. This needs an ARM7 import into a Ghidra project.

The ARM9 sample is decoded from `FUN_0207D1E0` (command `0x10` path). **[blue-verified]** for the packing and the ring. The packed 32-bit sample is `CONCAT22(*0x0207D49C, *0x0207D498)`. It is written to an 8-byte ring entry at `ring + index*8`. The ring base is `DAT_0207D494+4`, the index is the `u16` at `+3`, and the wrap limit is the `u16` at `+5`.

| Entry offset | Sample bits | Width | Decode | Use | Tag |
|--------------|-------------|-------|--------|-----|-----|
| `+0` (u16) | 0 to 11 | 12 bits | raw X | raw X (agrees with the Sample pipeline above) | **[blue-verified]** |
| `+2` (u16) | 12 to 23 | 12 bits | second 12-bit value | likely raw Y | [hypothesis] (medium) |
| `+4` (u16) | 24 | 1 bit | flag | likely the pen-down test (`CalibrateTouch` tests its third input word before it calibrates) | [hypothesis] (low) |
| `+6` (u16) | 25 to 26 | 2 bits | value | not identified | [open] |

- Bits 27 to 31 are not read by this decode. **[blue-verified]**
- The command `0x10` path increments the index (`+3`), wraps it when it reaches the limit (`+5`), and calls the callback `*DAT_0207D494` with `(0x10, 0, index)`. **[blue-verified]**
- The other command (`param_3 != 0`) sets bit `(1 << channel)` in `+0xD` and calls the callback with `(channel, 4, 0)`. **[blue-verified]**
- **[open]:** the second 12-bit field as Y, the bit-24 flag, and the 2-bit field at bits 25 to 26.

## Screens

### Boot display setup

**[blue-verified]**, high. `InitDisplayRegisters` (`FUN_02074FC8`, called from `0x02000BE0`):

- sets POWCNT1 (`0x04000304`) to `(v & 0xFFFFFDF1) | 0x20E` in the boot write at `0x02000BB8`–`0x02000BCC`, where `v` is the current value; bit 15 is set in `FUN_02074FC8` and the LCD-on step is the `FUN_0207D5F4` call (see [POWCNT1](#powcnt1-and-the-lcd-power-path-177-p5));
- clears DISPCNT;
- clears the BG registers on both engines: engine A is 0x60 bytes from `0x04000008`, engine B is 0x70 bytes from `0x04001000`;
- sets `MASTER_BRIGHT` (`0x0400006C`) to 0;
- writes the affine identity to `0x04000020/26/30/36` and `0x04001020/26/30/36` (the findings' shorthand for the four offsets on each engine), so BG2 and BG3 PA and PD are 0x100.

### POWCNT1 and the LCD power path (#177 P5)

**[blue-verified]** for the writes and the call chain. **[hypothesis]** for the bit names, which come from the standard DS hardware reference. No repo doc or pmd-red source covers POWCNT.

- **[hypothesis]** Bit names: bit 0 LCD power (screens on or off); bit 1 engine A (2D) power; bit 2 3D rendering power; bit 3 3D geometry power; bit 9 engine B (2D) power; bit 15 display swap (which engine is on the top screen).
- **[blue-verified]** Boot, `0x02000BB8`–`0x02000BCC`: `POWCNT1 = (POWCNT1 & 0xFFFFFDF1) | 0x0000020E`. Bits 1, 2, 3 and 9 are cleared and then set to 1. Bits 0 and 15 are not touched here.
- **[blue-verified]** Boot, `0x02000BD0`: calls `FUN_0207D5F4`, the LCD power call.
- **[blue-verified]** `FUN_02074FC8` (`02074FD4`–`02074FFC`) sets bit 15 (`|= 0x8000`), then applies a mask and OR from the literals at `0x0207510C` and `0x02075110`. The values were not read.
- **[blue-verified]** `FUN_0207D5F4` (`0x0207D5F4`) calls `FUN_0207D614` through the pointer at `0x0207D610` (`0x0207D614`): `FUN_0207D614(param == 1, 0, 0, 1)`. The indirect call is at `0x0207D60C`.
- **[blue-verified]** `FUN_0207D614` (`0x0207D614`): with `param_1 == 0` it clears bit 0 (`& 0xFFFE`); with `param_1 == 1` it sets bit 0 (`|= 1`), after a check of a counter difference below 8 when `param_3 == 0`. The POWCNT1 pointer is `DAT_0207D700` = `0x04000304`.
- **[blue-verified]** `FUN_0207D5D8` (`0x0207D5D8`) returns bit 0 of POWCNT1 (`DAT_0207D5F0` = `0x04000304`).
- **[blue-verified]** Callers of `FUN_0207D5F4`: the boot path (`0x02000BD0`) and `FUN_0207D704` (`0x0207D7A8`, the fade).
- **[open]** The counter that the `param_3 == 0` check tests is not identified. The values at `0x0207510C` and `0x02075110`, and the exact effect of bit 15 in Blue, were not read.

### Per-frame flush

**[blue-verified]** for the mechanism, high; the flag meanings are **medium**. `FlushDisplayState` (`FUN_02019268`, frame action #17) runs in `MainLoops_RunFrameActions`. It writes the DISPCNT layer enables from a flag byte at `DAT_02019610`, sets BG priorities, and writes `BLDCNT` (`0x04000050`). The layer-enable bits (8 to 12 of DISPCNT) start from `0x1F` and flag bits clear them: bit 0 BG0, bit 1 BG1, bit 2 BG2, bit 3 BG3, bit 4 OBJ.

**Register writes (#161 P4, blue-verified; the register addresses are read from literals).**

| Register | Write |
|----------|-------|
| BG scroll (`0x04000010` `BG0HOFS` and the following scroll registers) | eight words, written through pointers that resolve to these registers |
| `BG0CNT` `0x0400000C` and `BG1CNT` `0x0400000E` | priority bits 0 to 1 |
| `DISPCNT` `0x04000000` | bits 8 to 12 (layer mask), from a value built from the flags at `DAT_02019610` and `DAT_02019614` |
| `BLDCNT` `0x04000050` | masked with `0x43`, ORed with `0x4200` (or a variable) |
| `BLDALPHA` `0x04000052` | `EVA | EVB << 8` |

`FUN_02075938` is called for the second screen (a `0x10 × 0x10` window at `(0, 0)` or `(4, 8)`) and for a size-dependent window (`(2, 0x1C)`, `(4, 0x18)`, `(6, 0x18)`). **Not verified:** the details of `FUN_02075938`, which was not traced. The Red name `xxx_call_update_bg_sound_input` is not used: nothing in the function touches sound.

`FUN_02019794` (frame action #8) calls `FUN_0207D704` with `0x0C` or `0x1C` when a condition holds. The condition is not described.

### Scene switch and reset

**[blue-verified]** for the call chain, high. `SceneSwitchReset` (`FUN_0201A110`) calls the full reset `ResetScreens` (`FUN_02018B44`) when `FUN_0207F458()` is nonzero. What `FUN_0207F458` tests is **medium**.

Callers of `FUN_0201A110`: `LoadAndRunDungeon_Async`, `NDS_LoadOverlay_GroundMain`, `RunOverlay7Mode`, `GroundStation_LoadBank`, `GroundStation_UnloadBank`, and the unnamed `FUN_0201A2C4`, `FUN_0201A404`, `FUN_0201A6F0` and `FUN_02018DB0`. Two of these (`FUN_0201A404` and `FUN_0201A6F0`) are file-loading routines in [File loading](#file-loading); see [Open questions](#open-questions).

`ResetScreens` clears VRAM at `0x06800000` and three more 0x8000-word blocks, then calls `FUN_02074E90(2, 0, 0)` and `FUN_02074F04`. It also calls the PX decoder directly for the halt screen, at `0x02018C20` (see [Halt screens](#halt-screens)).

### DISPCNT writers and engine use (#177 P1)

**[blue-verified]**, high for the list of writers. The scene-to-layer mapping is not traced. Every `DISPCNT` (`0x04000000`) write found in the program is on engine A: `FlushDisplayState` (`FUN_02019268`), `FUN_0207D704` (fade), `FUN_02074E90`, `FUN_02074F04`, `FUN_02074F50` (display-mode helpers), `FUN_02074FC8` (reset) and `FUN_02018578` (boot). No write to engine B's `DISPCNT` (`0x04001000`) was found. Engine B appears only as a `MASTER_BRIGHT` argument (`0x0400106C`).

- `FUN_02074E90(p1, p2, p3)` sets DISPCNT bits 16–17 from `p1`, bit 3 from `p3`, and `p2`. Callers: `FUN_02018B44` and `FUN_02018578`.
- `FUN_02074F04` sets bits 16–17 from a stored value, or bit 16 when that value is zero. Callers: `FUN_02018B44` and `FUN_02018578`.
- `FUN_02074F50` reads bits 16–17 into a stored value and clears them. Callers: `FUN_02018B44` and the boot path (`0x02000BE4`).
- **[open]** Which BG (BG0 to BG3) each scene shows. The per-scene layer enables come from `FlushDisplayState` through the mask at `DAT_02019610`, and the scene-to-mask mapping is not traced.

### Fade IPC and brightness (#177 P2)

**[blue-verified]** for the mapping; **[hypothesis]** for the ARM7 meaning. The fade's brightness effect goes through IPC. The ARM9 side writes only DISPCNT.

- Call chain: `FUN_0207D704` (fade) → `FUN_0207DCD8` → `FUN_0207D944` → `FUN_02079BEC`.
- `FUN_02079BEC` checks the FIFO error bit (`0x4000`) and the send-full bit (`0x2`) in `*DAT_02079C98` (`0x04000184`, `REG_IPCFIFOCNT`). It then writes `(p1 & 0x1F) | ((p3 & 1) << 5) | (p2 << 6)` to `*DAT_02079C9C` (`0x04000188`, `REG_IPCFIFOSEND`).
- `FUN_0207D944(v)` sends command `8` with payload `v`, polling until the send succeeds.
- `FUN_0207DCD8` sends two command-8 messages per call: `(p1 & 0xFF) | DAT_0207DD84`, then `(p2 & DAT_0207DD88) | DAT_0207DD8C`. `FUN_0207D614(0, 2, 0, 0)` runs between them.
- `FUN_0207D704` calls `FUN_0207DCD8` with `(p1 | local_34 << 5 | local_30 << 6) & 0xFFFF` and `p2 | p3`. Its DISPCNT writes (bits 16–17 cleared, then restored) are on the ARM9 side.
- **[hypothesis]** ARM7 side of command 8: most likely the screen brightness or backlight. The ARM7 code was not checked.
- **[open]** The payload values in `DAT_0207DD84`, `DAT_0207DD88` and `DAT_0207DD8C` were not read.
- **[blue-verified]** `REG_MASTER_BRIGHT` (`0x0400006C`, engine A) is written only in `FUN_02074FC8` (`0207507C` and `020750B4`), which the boot path calls from `02000BE0`. That function also clears DISPCNT. No per-frame ARM9 brightness write was found.
- **Correction:** `FUN_02018578` is the boot display setup (VRAM clears and DISPCNT writes), not a per-frame brightness writer. It passes `0x0400006C` (at `02018724`) and `0x0400106C` (engine B, at `020188F4`) as arguments. **[open]** The helper that receives them is not identified.

### What is not known

- Which engine and BG show the title, ground and dungeon content. Engine A is the only engine with a DISPCNT write (#177 P1, [above](#dispcnt-writers-and-engine-use-177-p1)). Which BG each scene shows is open; see [Open questions](#open-questions).
- The meaning of the POWCNT1 bits. The bit names are **[hypothesis]** from the hardware reference (#177 P5), not from Blue. See [POWCNT1 and the LCD power path](#powcnt1-and-the-lcd-power-path-177-p5).

## Sprites (OAM) and fades

### OAM writers

**[blue-verified]** for the writers. **High** for "no shadow OAM and no DMA3" (#177 P4, as investigator C gave it); **medium** for the rest. OAM is written by direct CPU writes to `0x07000000`, and `BlinkSpriteZero` is the only per-frame upload.

- `ResetSceneGraphics` (`FUN_0201AFF0`, scene load) clears BG and OBJ VRAM and the palette, fills OAM A and OAM B (`0x07000400`) with `DAT_0201B214`, and loads palette data. **[blue-verified]** #177 P4 finds its only caller at `0x02000CF4`, the boot path, which conflicts with the "scene load" label here. **[open]** Reconcile the two (see Open questions).
- `ClearOamA` (`FUN_0201ADA0`) clears VRAM at `0x06000000` (`0x13C0` words), the two buffers at `DAT_0201AE84` and `DAT_0201AE88`, `0x06400000` and `0x05000000`, and writes `0x07000000`. It looks like a scene reset. Its only caller is a computed jump (through a jump table) from a thunk at `0x0206DC18`; the table is not identified.
- `BlinkSpriteZero` (`FUN_02012A6C`) writes OAM entry 0 (`0x07000000`–`0x07000006`) on every call. It toggles attribute 0 between `0xC0` (hidden) and `DAT_02012AE4[1]`, by bit `0x10` of the counter `DAT_02012ADC`, so the sprite blinks every 16 frames. The only caller is `FUN_02018ACC` (`02018B04`), the frame callback, so this one entry is the per-frame OAM upload. **[open]** The OAM entry format (attr1 and attr2) and the sources of `DAT_02012AE4` and `DAT_02012AE0`.
- **[blue-verified]** No shadow OAM and no DMA3 upload. `0x040000B0` is referenced only from the generic DMA helpers `FUN_020782C0`, `FUN_02078248` and `FUN_01FFA3D0`. OAM is written by direct CPU writes, not DMA. The per-frame OAM write is `BlinkSpriteZero` alone.
- **[blue-verified]** `0x040000D4`–`0x040000DF` (DMA3) has no references in the program. `FUN_02078424` is a plain CPU fill loop, not DMA. `FUN_02018578` (boot display init) uses it to fill `0x400` bytes at `0x07000000` with `0xC0`.
- **[blue-verified]** The DMA0 transfer on the frame path is not an OAM upload. `FUN_0201966C`, called from the frame callback `FUN_02018ACC`, resets DMA0 (`FUN_02078154(0)`), then starts a 4-byte transfer (`FUN_02078350(0, src, dst, 4)`, source at `DAT_02019760`). Its destinations are `DAT_02019764`, `DAT_0201976C`, `DAT_02019768` and `DAT_02019770`, which hold `0x04000040`, `0x04000042`, `0x04000044` and `0x04000046` (WIN0H, WIN1H, WIN0V, WIN1V). The fixed value is `0xC0`. The timing bits are `0x10000000` (`FUN_020780A0`). **[hypothesis]** The HBlank meaning of that timing. **[open]** What the window DMA shows on screen.

### Fades

**[blue-verified]** for the control flow of `FUN_0207D704`; **medium** for the register it drives.

- `FUN_0207D704` saves DISPCNT, waits on a VBlank counter (`DAT_0207D934`), calls `FUN_0207DCD8` once per step, and restores the state.
- `FUN_0207DCD8` sends two parameters through `FUN_0207D944` as IPC command 8. See [Fade IPC and brightness](#fade-ipc-and-brightness-177-p2).
- The brightness register `0x0400006C` is written to 0 at reset in `FUN_02074FC8`. It is passed as a parameter from `FUN_02018578` (at `0x02018724`). `FUN_02018578` also passes `0x0400106C` (at `0x020188F4`). **[blue-verified]**. No per-frame ARM9 writer was found (#177 P2).
- `BLDCNT` (`0x04000050`) is written per frame in `FlushDisplayState`.
- The title fade is located (#177 P3, see [Title fade](#title-fade-177-p3)). [`execution-model.md`](execution-model.md) describes a 32-frame title fade. No `0x04000054` (BLDY) reference exists in the program.

### Title fade (#177 P3)

**[blue-verified]** for the loop and the helpers, high on the structure. **[hypothesis]** for the buffer identities.

- `GameLoop_Async` (`0x0201F2E4`) runs `while (*DAT_0201F808 < 0x20)`, a 32-frame counter at `0x0201F808`. Each iteration increments the counter, then for `i = 0..0xEF` (240 entries) calls `FUN_02011D24(i, src1 + i*4, counter, 0)` and `FUN_02011B4C(i, src2 + i*4, counter, 0)`, then `MainLoops_RunFrameActions(0)`.
- `FUN_02011D24` (`0x02011D24`) and `FUN_02011B4C` (`0x02011B4C`) clamp the brightness to 0..31. Each writes its source colour, scaled as `c * brightness / 256` (the divide is `FUN_0208625C`), as a 15-bit RGB word. `FUN_02011D24` writes into `DAT_02011E44 + i*2` and `FUN_02011B4C` into `DAT_02011C6C + i*2`. Each also sets a dirty flag for entry `i`.
- So the fade is software. It scales palette values per frame, and the result reaches palette RAM through the per-frame palette transfer (`TransferBGPaletteBuffer`, frame action #14 in `MainLoops_RunFrameActions`). This matches the absence of BLDY and brightness register writes.
- **[hypothesis]** Which buffer is the BG palette and which is the OBJ palette.
- **[open]** The fade-out is not decoded. The 0x20 count is the fade-in.

## File loading

### Archive index and TOC

**[blue-verified]**, high. The name table at `0x02096EE4` gives archive indices. Index 0 is unused.

| Index | Archive |
|-------|---------|
| 1 | `sample` |
| 2 | `system` |
| 3 | `titlemenu` |
| 4 | `dungeon` |
| 5 | `monster` |
| 6 | `effect` |
| 7 | `ground` |
| 8 | `ornament` |
| 9 | `sound` |

`LoadArchiveTocs` (`FUN_0201A6F0`, called from `0x02000CE8` at boot) reads each archive's 16-byte TOC records into one global table. A record has:

- a name, 8 bytes (`+0` to `+7`);
- an offset at `+8`;
- a 24-bit size at `+0xC`;
- the archive index at `+0xF`.

The findings give the size and the index as separate fields. Their sharing one 32-bit word is our reading, not checked.

`sound.sbin` (index 9) is therefore loaded by this boot reader. [`formats/archive.md`](../formats/archive.md) excludes it; see the note there.

Lookup and read:

- `FindArchiveEntry` (`FUN_0201A56C(name, archive)`) binary-searches the archive's TOC range. Its return value is not checked.
- `FUN_0201A404` reads the payload in 64-byte chunks.
- `FUN_0201A188` and `FUN_0201A0B8` apply the SIR0 pointer fix-up (see [Decompression](#decompression)).

### Consumers at boot and in `GameLoop_Async`

**[blue-verified]** for the names and call sites, high; the destination pointers are **low** confidence.

| Consumer | Call site | File key | Archive | Bytes |
|----------|-----------|----------|---------|-------|
| `LoadKanjiFonts` (`FUN_020160A4`), boot | `0x02000CEC` | `kanji_a`, then `kanji_b` | 2 | `kanji_a` 0xA000, `kanji_b` 0x6400 |
| `LoadMonsterParameters` | `0x0201F304` in `GameLoop_Async` | `monspara` | 2 | 0xA000 |
| `LoadWazaParameters` (`FUN_0200B888`) | `0x0201F494` in `GameLoop_Async` | `wazapar{a,g,f,i,s}` | 2 | 0x18000 |
| `LoadItemParameters` (`FUN_02008614`) | `0x0201F490` in `GameLoop_Async` | `itempar{a,g,f,i,s}` | 2 | 0xC000 |

- The suffix of `wazapar` and `itempar` is chosen by `GetLanguage()`. If it returns -1, the suffix is 0.
- The name keys for these files are at `0x0208FA78`, `0x02091CD0` and `0x020913AC`. The findings do not map each key to its row.
- `FUN_02028628` opens archives 3 and 5, with names built by `sprintf` from an ID. Its caller `FUN_02027C58` is not mapped.

The names `LoadMonsterParameters`, `LoadWazaParameters` and `LoadItemParameters` are the ones used in [`execution-model.md`](execution-model.md) and [`data-tables.md`](data-tables.md).

### Halt screens

**[blue-verified]**, high. `LoadHaltScreen` (`FUN_02018C98(index)`, through `FUN_02018C54`) opens a loose NitroFS file, not an archive entry. The name table at `0x02094DBC` holds `nitro_haltscreen{,_ger,_fra,_ita,_spa}.sbin`, indexed by language. The data is then decoded by the PX decoder (see [Decompression](#decompression)).

Anomalies the findings did not resolve:

- The copy destinations `0x06608000` (monster), `0x06408000` (moves) and `0x06214000` (items) are not obviously RAM addresses. The cause is unknown.
- `FUN_02018C98` reads into `0x02094E58`, which is also where an embedded `AT4PX` sub-file sits. Either the variable is not what it looks like, or the read overlaps that data.
- The string `"/sound.sbin"` at `0x020973B4` is referenced from `FUN_0201C070` (at `0x0201C0BC`). How it is used is not resolved.

## Decompression

### PX decoder

**[blue-verified]** for the decoder, high; the caller list is **medium**. `PxDecompress` (`FUN_02013388`) accepts a header with the magic `AT4P` (header size 0x12) or `AT3P` (header size 0x10). Byte 4 `'N'` selects raw nibble mode. Otherwise the decoder reads the eight control nibbles and decodes literal and back-reference tokens. Helpers: `FUN_02013260` writes a byte, `FUN_020132D8` reads one back, `FUN_02013344` initialises the output, and `FUN_02013230` finishes it.

The control nibbles and the size field agree with [`formats/compression.md`](../formats/compression.md). The `AT3P` variant is new there. The byte-4 mode is not in that document, and it does not match the five-byte magic `AT4PX` given there. See [Open questions](#open-questions).

### Dispatcher

**[blue-verified]**, high for the slot; **medium** for the caller list. `FUN_02013858` jumps through the slot at `0x02013864`, which holds `0x02013388`. The slot has one read reference, and no writer is found statically. The callers are `ResetScreens` (`FUN_02018B44`, direct, at `0x02018C20`, for the halt screen), `FUN_02028628`, `FUN_02029734` (from `FUN_02027C58`), and functions in overlay 0 and overlay 1. Only the halt-screen caller is known to decode a particular file.

### SIR0 relocation

**[blue-verified]**, high. `RelocateSir0` (`FUN_0201A0B8`) is the relocation pass: big-endian base-128 offsets, each added to the base. The `SIRO` flip is in `FUN_0201A134` and `FUN_0201A188`. This agrees with [`formats/archive.md`](../formats/archive.md).

### Not found, and a negative

- **[blue-verified]** A negative result: `FUN_01FF9FC4` is a trampoline into `FUN_0207853C`, a halfword-aligned copy. It is not a decompressor. Its roughly 50 callers are copies, not decoders.
- **Not verified:** BLZ (overlays are not in `arm9.bin`), LZ10, LZ11, Huffman and the ground BPC codec are not located in the ARM9. A magic search for `AT4P` found only the embedded sub-file at `0x02094E58`. BLZ is listed for the ARM9 static area in [`formats/compression.md`](../formats/compression.md) and [`nds-container.md`](../formats/nds-container.md); the findings here cover only decoder code in `arm9.bin`. Finding the other decoders depends on [#42](https://github.com/TorratDev/rust-mm/issues/42) and [V8 (#18)](../verification.md#v8--ghidra-loads-arm9-and-overlays-at-real-addresses), and the ground BPC codec on [#160](https://github.com/TorratDev/rust-mm/issues/160).

## Open questions

Input:

- What the A, B, Select and Start keys do, and the menu bindings (#175 P1, partly mapped). Part (a), the dungeon reader `ov_0001::02166B10`, has its actions recorded above; the callees are open. Part (b): the 28 other readers are classed by their keys (#175 P1c), the helper pointers are resolved (play-sound routine `FUN_0201BC30`), and the menu names are hypotheses; which reader is the team menu, and any menu at run time, are open.
- Whether X and Y can come out pressed at run time (#175 P2b). They do reach the key path through the shadow word `0x027FFFA8`, and no code reads `REG_EXTKEYIN`. Open: the ARM7 side (what fills the shadow word and its value while a key is held) and the idle value of `KEYINPUT` bits 10 and 11 on the ARM9. A probe that presses X and Y would settle it.
- The writer of the word at `0x027FFFA8`: none in the ARM9 or overlays (#175 P2b, all five pool references are loads); the ARM7 is the likely writer (**[hypothesis]**). The bit-15 hooks were never exercised at run time, and their callees `FUN_0207D704` and `FUN_0207D9EC` are unread.
- The product at `0x020F2768` has no consumer in this build (write-only, #175 P2b); the meaning of the constant `0x054A1C41` is open.
- The repeat timing is measured for Down (#175 P5); Up, Left and Right were not run.

Touch:

- The ARM7 side of channel 6 is out of reach statically. The Ghidra project `rescue-arm9` holds only the ARM9 and the overlays, so the ARM7 code cannot be read from it. It needs an ARM7 import; see [ARM9 sample decode and the ARM7 side](#arm9-sample-decode-and-the-arm7-side-176-p4).
- The call targets in the unanalysed commit block at `0x0203CC6C`, which hold the options-menu commit, and the identity of `0x0202ADAC`. See [Writers of options+7](#writers-of-options7-176-p3b).

Screens and fades:

- P3d: which engine and BG each scene uses. This needs the DISPCNT writes from each scene loader, and the scene map from [#44](https://github.com/TorratDev/rust-mm/issues/44).
- **[open]** P3e (#177 P5): the bit 15 effect in Blue, the values at `0x0207510C` and `0x02075110` (`FUN_02074FC8`), and the counter behind the `param_3 == 0` check in `FUN_0207D614`. The bit names are **[hypothesis]** from the hardware reference, not from Blue or Red.
- **[hypothesis]** The ARM7 meaning of IPC command 8, most likely brightness or backlight (#177 P2). The ARM7 code was not checked. **[open]** The payloads in `DAT_0207DD84`, `DAT_0207DD88` and `DAT_0207DD8C` were not read.
- **[open]** The helper that receives the two `MASTER_BRIGHT` addresses (`0x0400006C`, `0x0400106C`) from `FUN_02018578` (#177 P2).
- **[open]** The caller chain of `FUN_02018B44` (the scene reset path); the #177 P2 pass did not verify it.
- **[open]** P3: the title fade-out is not decoded. The 32-frame fade-in is located in `GameLoop_Async`. Which palette buffer is BG and which is OBJ is **[hypothesis]**.
- **[open]** Which BG each scene shows (P1, mapping not traced); see [DISPCNT writers](#dispcnt-writers-and-engine-use-177-p1).
- **[blue-verified]** Closed (#177 P5): the cross-reference attribution is correct. The POWCNT1 (`0x04000304`) accesses at `0x0207D68C`, `0x0207D694`, `0x0207D6A4` and `0x0207D6B4` go through `DAT_0207D700` = `0x04000304` and are in `FUN_0207D614`, which sets and clears bit 0 through that pointer. The `0x0207D5DC` read in `FUN_0207D5D8` goes through `DAT_0207D5F0` = `0x04000304`. This settles the earlier note that the decompile of `FUN_0207D614` had no POWCNT access.
- **[open]** P4a: the OAM entry format (attr1 and attr2) and the sources of `DAT_02012AE4` and `DAT_02012AE0`. The jump table that holds `0x0206DC18`, and so the scene that calls `ClearOamA` (`FUN_0201ADA0`), is not identified.
- **[open]** The window DMA through DMA0 (`FUN_0201966C`): what the WIN0H, WIN1H, WIN0V and WIN1V values show on screen. The HBlank timing is **[hypothesis]**, a guess.
- **[open]** `FUN_0201AFF0`: the boot caller `0x02000CF4` (#177 P4) and the "scene load" label in the OAM writers conflict. Reconcile which is right.
- `FUN_0201A404` and `FUN_0201A6F0` are file-loading routines, yet the cross-references list them as callers of the scene reset `FUN_0201A110`. Either the reset is reached from file loading, or the cross-reference is wrong. Not checked.

Files:

- The copy destinations `0x06608000`, `0x06408000` and `0x06214000`. Not obviously RAM addresses; the cause is unknown. The findings suggest checking them with the py-desmume trace before any claim is made.
- The read target `0x02094E58` overlaps an embedded `AT4PX` sub-file.
- The `/sound.sbin` string at `0x020973B4`, referenced from `FUN_0201C070`: its use is unresolved.
- Which files use the `AT3P` header, if any. Not checked.
- Byte 4 of the PX header: the findings say `'N'` selects raw nibble mode, but `formats/compression.md` gives the five-byte magic `AT4PX`. Check the mode byte against the decompile.
- BLZ, LZ10, LZ11, Huffman and the ground BPC codec: not located in the ARM9. Depends on [#42](https://github.com/TorratDev/rust-mm/issues/42) and [#18](https://github.com/TorratDev/rust-mm/issues/18) for BLZ, and on [#160](https://github.com/TorratDev/rust-mm/issues/160) for the ground banks.
