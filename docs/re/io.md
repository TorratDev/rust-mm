# Input, screens, sprites and file loading

How Blue reads the buttons and the touch screen, sets up the two screens, fills OAM and runs fades, and loads files from the pack archives and from loose NitroFS files. Issue: [#45](https://github.com/TorratDev/rust-mm/issues/45). Names and addresses are in [`symbols.md`](../symbols.md); the structs are in [`glossary.md`](../glossary.md).

Dump: `BlueRescueTeamEur`. Addresses are RAM addresses. Plain addresses are in the ARM9 binary (`arm9.bin`); `ov_NNNN::` marks code in an overlay. Tags: **[blue-verified]** means read in Blue's code, and where a finding is only about control flow it says so; **[red-hypothesis]** and **[tentative]** are as in [`rng.md`](rng.md). **[hypothesis]** marks a meaning the investigators did not confirm, and **[open]** marks a gap; both are as the #177 investigators tagged them. "Not verified" and "unknown" mark what the investigators did not check.

**Evidence basis.** Every finding on this page is static: decompile, cross-reference and memory-read results from three Ghidra copies of the ARM9 project. No emulator check was run for input, screens, sprites, fades or file loading. So the button-repeat timing and the frame cadence below are assumed, not measured. Nothing here is compared against `pmd-red` except where a name is marked.

## Buttons

### Frame path

**[blue-verified]**, high. The VBlank IRQ handler is `FUN_02018ACC`. `FUN_02075C5C(1, FUN_02018ACC)` registers it in IRQ slot 0, and the literal `0x02000D60` holds `0x02018ACC`. This names the handler that [`execution-model.md`](execution-model.md) lists as not located; that page should be updated to match.

The handler calls `FUN_02017B20` (`UpdateKeyState`) each frame. That calls `FUN_02019130` (`ReadKeyInput`), which:

- reads `REG_KEYINPUT` (`0x04000130`, the only reader in the project);
- masks with `0x2FFF` (literal at `0x02019198`) and computes `((KEYINPUT | *0x027FFFA8) ^ 0x2FFF) & 0x2FFF`, then adds bit `0x8000` copied from `*0x027FFFA8`;
- adds the result into a running sum at `0x020DF89C`.

Hardware keys are active-low; the XOR makes them active-high. The OR with the word at `0x027FFFA8` clears bits rather than setting them. Keys are active-low, so a 1 bit in the word (bits 0 to 11 or 13) forces that key to "released". Bits 10, 11 and 13 are not key bits, and `ReadKeyInput` masks them out.

**Shadow word at `0x027FFFA8` (#175 P2).** The formula above is **[blue-verified]**, from the literal pool at `0x02019190`: `REG_KEYINPUT` `0x04000130`, the shadow word `0x027FFFA8`, mask `0x2FFF`, and the accumulator `0x020DF89C`. Bit 15 is copied from the shadow word. Its readers are **[blue-verified]**, and all test `0x8000`:
- `FUN_02019794` (literal read at `0x020197BC`): plays a sound (`FUN_0207d704`) when bit 15 is set and `DAT_02019820` is nonzero.
- `FUN_0207F418` (literal read at `0x0207F420`): calls `FUN_0207d9ec` when bit 15 is set.
- Overlay 7 wait loop `FUN_ov_0007__02139A98` (literal at `ov_0007::02139F4C`): exits early when bit 15 is set.

**[hypothesis]** (boot probe, py-desmume, scratch and not committed, so no command or output hash is recorded in the repo; this stays a hypothesis until the probe is committed or its command and output hash are recorded): the word is `0x0` at frame 0, then `0x2C00` from frame 15 to the end of the run, and it did not change across 547 or more `KEYINPUT` changes. The `KEYINPUT` values seen were `0x3FE`, `0x3F7` (Start at frame 900), `0x37F`, `0x3BF`, `0x3EF` and `0x3DF`. Because bits 10, 11 and 13 are masked, this word changed no key in that run. Bit 15 was never set, so the three bit-15 branches were not exercised. Confidence: high for the value and for no effect on keys in the boot run; medium for the meaning of the bits. **Writer: not established.** No writer is found in Blue's ARM9. The byte pattern `a8ff7f02` appears only as literals in the readers above, and the word is not in the static image, so a write from ARM7 or firmware is the likely source.

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

**[blue-verified]** for the logic, high. The counter starts at 1 on a new direction and fires at 48. It then resets to 43, so it fires every 5 frames after the first fire, about 47 frames after the press. The timing assumes one call per VBlank; it is not measured.

### B and R taps

**[blue-verified]**, high. Counters at `0x020F276C` (B) and `0x020F276E` (R) set the tap flags in `+6`. A tap is a release after 2 to 11 frames.

### Accumulator

**[blue-verified]** for the structure, **medium** for the meaning. The accumulator at `0x020F2770` is ORed with the held keys each frame. Its int at `0x020F2768` is updated as `value *= (held | 0x054A1C41)`. The investigator reads this as an RNG mix, but the constant is not checked. Cross-check it with [`rng.md`](rng.md) before using it.

**The product at `0x020F2768` is not an RNG step (#175 P3).** **[hypothesis]**, high on the code and medium on the meaning. `rng.md` has no match: the global and dungeon LCGs use `0x5D588B65`, and `0x054A1C41` appears in no doc, Rust file or Ghidra label in the repo. The factor is odd, so the product is invertible mod `2^32`, but nothing in the imported program consumes it. The only reader and writer in the program are `FUN_02017B20` (read at `0x02017CCC`, write at `0x02017D14`), and `FUN_020181AC` writes the initial value `0x004A14C1` (write at `0x020181F4`, called from the boot path at `0x02000CDC`). Not checked: overlay or trace-harness reads.

### Snapshot and consumers

**[blue-verified]**, high. `FUN_02017E60` (`UpdateInputSnapshot`, frame action #9 in [`execution-model.md`](execution-model.md), named `LoadBufferedInputs` there, a recorded name; see [Frame action names](execution-model.md#frame-action-names-161-p4)) copies the accumulator to `0x020F27A0` and clears it. About 40 functions read `0x020F27A0`, in the ARM9 and in overlays 0, 1 and 7. Its touch work is in [Touch](#touch).

- **Title loop.** `GameLoop_Async` uses the L button (`0x200`) as a pressed edge and as held state, to call `FUN_02019634(0x13)` and `FUN_02019634(0)`. This is the title loop's L gate.
- **Dungeon directions.** `ResolveDungeonDirection` (`FUN_020271E0`, **[blue-verified]**, high) maps the D-pad to a direction code. Down `0x80` → 1, Right `0x10` → 2, Up `0x40` → 4, Left `0x20` → 8. That code indexes a 16-byte table at `0x020B219C`, which gives 8 directions without R: 0 Down, 1 DownRight, 2 Right, 3 UpRight, 4 Up, 5 UpLeft, 6 Left, 7 DownLeft.
- **R for diagonals.** With R held (`0x100`) the lookup uses the table at `0x020B218C`, which gives only the diagonals.
- **Emission.** A direction is emitted on the 4th identical frame, then on every frame while held (debounce counter `> 2`).
- **Not mapped.** A, B (except the tap flag), Select, Start and the menu bindings. See [Open questions](#open-questions).

## Touch

### Sample pipeline

**[blue-verified]** for the structure, **medium** for the interpretation.

- `FUN_0207D150` starts channel 6 through `FUN_02079CA0` and `FUN_02079CC8`. Its handler `FUN_0207D1E0` runs for command `0x10`. It decodes a packed 32-bit sample from the shared words `*0x0207D49C` and `*0x0207D498` into an 8-byte ring-buffer entry. The low 12 bits are the raw X. The other fields are not decoded.
- `FUN_0207CE64`, and `FUN_0207CE04` through `FUN_0207CDE4`, read the sample struct at `0x02137354`. A sample is valid if `+0x34 & 1 == 0`. They return X and Y from `+4` and `+6`, and a pen flag from `+8`.

### Calibration

**[blue-verified]** for the control flow, high; the parameters are **not verified**. `CalibrateTouch` (`FUN_0207CA44`) passes raw coordinates through when the flag at `DAT_0207CB6C+0x30` is zero. Otherwise it applies an affine transform with parameters at `0x0207CB70`, then clamps X to `0..0xFF` and Y to `0..0xBF`. That is a 256 × 192 screen. The source of the parameters is not checked.

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
- The gate byte is a serialised options bit. Its name is not confirmed.
- In the dungeon, a touch within ±10 px of an active slot (flag `0x8000`, stride `0x3C`, 8 slots) returns that slot index (`ResolveDungeonDirection`, above). The meaning of the actor fields `+0x4DD4` to `+0x4DDA` is not verified.

## Screens

### Boot display setup

**[blue-verified]**, high. `InitDisplayRegisters` (`FUN_02074FC8`, called from `0x02000BE0`):

- sets POWCNT1 (`0x04000304`) to `(v & 0xFFFFFDF1) | 0x8000 | 0x20E | 1`, where `v` is the register's current value;
- clears DISPCNT;
- clears the BG registers on both engines: engine A is 0x60 bytes from `0x04000008`, engine B is 0x70 bytes from `0x04001000`;
- sets `MASTER_BRIGHT` (`0x0400006C`) to 0;
- writes the affine identity to `0x04000020/26/30/36` and `0x04001020/26/30/36` (the findings' shorthand for the four offsets on each engine), so BG2 and BG3 PA and PD are 0x100.

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
- The meaning of the POWCNT1 bits. The findings give bit 15 as display swap, with the hardware documentation as the only source. This is not verified in Blue.

## Sprites (OAM) and fades

### OAM writers

**[blue-verified]** for the writers, high; the missing upload path is **low** confidence.

- `ResetSceneGraphics` (`FUN_0201AFF0`, scene load) clears BG and OBJ VRAM and the palette, fills OAM A and OAM B (`0x07000400`) with `DAT_0201B214`, and loads palette data. **[blue-verified]** #177 P4 finds its only caller at `0x02000CF4`, the boot path, which conflicts with the "scene load" label here. **[open]** Reconcile the two (see Open questions).
- `ClearOamA` (`FUN_0201ADA0`) clears VRAM at `0x06000000` (`0x13C0` words), the two buffers at `DAT_0201AE84` and `DAT_0201AE88`, `0x06400000` and `0x05000000`, and writes `0x07000000`. It looks like a scene reset. Its only caller is a computed jump (through a jump table) from a thunk at `0x0206DC18`; the table is not identified.
- `BlinkSpriteZero` (`FUN_02012A6C`) writes OAM entry 0 (`0x07000000`–`0x07000006`) on every call. It toggles attribute 0 between `0xC0` (hidden) and `DAT_02012AE4[1]`, by bit `0x10` of the counter `DAT_02012ADC`, so the sprite blinks every 16 frames. The only caller is `FUN_02018ACC` (`02018B04`), the frame callback, so this one entry is the per-frame OAM upload. **[open]** The OAM entry format (attr1 and attr2) and the sources of `DAT_02012AE4` and `DAT_02012AE0`.
- **No shadow OAM and no DMA3 upload was found.** `0x040000B0` is referenced only from the generic DMA helpers `FUN_020782C0`, `FUN_02078248` and `FUN_01FFA3D0`. The upload may sit behind an unnamed helper; this is not checked.
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

- What the A, B, Select and Start keys do, and the menu bindings (#175 P1, partly mapped). The dungeon main-loop reader `ov_0001::02166B10` is mapped against pmd-red `dungeon_main.c`; the rest is not. Suggested split: (a) finish the actions for B, Start, X and Y in that reader; (b) map the menu readers in overlay 1 against `dungeon_menu_*.c`, reading the buffer pointer first.
- The writer of the word at `0x027FFFA8`, and the bit-15 behaviour, which the boot run never exercised (#175 P2). The word's meaning is not an open question: it is the shadow mask above.
- What consumes the product at `0x020F2768`. Nothing in the imported program reads it; it is not an RNG step (#175 P3).
- The repeat timing (48, then 43) is assumed from one call per VBlank. An emulator check would settle it.
- The IRQ handler: this page names `FUN_02018ACC`. [`execution-model.md`](execution-model.md) still says the handler is not located, and needs updating.
- `FUN_02017E60` is `LoadBufferedInputs` in [`execution-model.md`](execution-model.md) (recorded name, #161 P4) and `UpdateInputSnapshot` here. The name here is the one to change.

Touch:

- The source of the calibration parameters at `0x0207CB70`.
- The ARM7 side of channel 6 (not checked).
- Which features are touch-only and which are touch-optional.
- The name of the options bit at `0x020DED14+7`. The name is not confirmed.

Screens and fades:

- P3d: which engine and BG each scene uses. This needs the DISPCNT writes from each scene loader, and the scene map from [#44](https://github.com/TorratDev/rust-mm/issues/44).
- P3e: the meanings of the POWCNT1 bits. Only the hardware documentation covers them. The investigator tagged this `[red-hypothesis]`, but the source is not Red.
- **[hypothesis]** The ARM7 meaning of IPC command 8, most likely brightness or backlight (#177 P2). The ARM7 code was not checked. **[open]** The payloads in `DAT_0207DD84`, `DAT_0207DD88` and `DAT_0207DD8C` were not read.
- **[open]** The helper that receives the two `MASTER_BRIGHT` addresses (`0x0400006C`, `0x0400106C`) from `FUN_02018578` (#177 P2).
- **[open]** The caller chain of `FUN_02018B44` (the scene reset path); the #177 P2 pass did not verify it.
- **[open]** P3: the title fade-out is not decoded. The 32-frame fade-in is located in `GameLoop_Async`. Which palette buffer is BG and which is OBJ is **[hypothesis]**.
- **[open]** Which BG each scene shows (P1, mapping not traced); see [DISPCNT writers](#dispcnt-writers-and-engine-use-177-p1).
- An inconsistency in the cross-reference: the tool attributes the POWCNT1 (`0x04000304`) accesses at `0x0207D68C` to `0x0207D6B4` to `FUN_0207D614`, but the decompile of `FUN_0207D614` has no POWCNT access. Treat the attribution as suspect until it is checked.
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
