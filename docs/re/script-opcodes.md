# Ground script opcodes, flow control and variables

**Date:** 2026-11-26
**Dump:** Pokémon Mystery Dungeon – Rescue Team Blue (EUR), `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Static evidence comes from the ghidra-cli text export of the `rescue-arm9` project (STAMP 2026-11-14, 5284 ARM9 functions, 0 failed) and from 32-bit pool words read from the extracted `arm9.bin` and `overlay_0000.bin`.
**Issue:** [#522](https://github.com/TorratDev/rust-mm/issues/522) (batch [#645](https://github.com/TorratDev/rust-mm/issues/645), parent [#76](https://github.com/TorratDev/rust-mm/issues/76))
**Workflow:** [workflow.md](workflow.md)

This note is the full opcode table of the interpreter `ov_0000::021421CC`. The engine around it (storage, the stepper, actors, camera, dialogue) is in [scripts.md](scripts.md). The binary layout of a script is in [../formats/scripts.md](../formats/scripts.md). Addresses are ARM9 unless marked `ov_0000::`; every interpreter handler is in overlay 0. Names in the "What" column are ours unless a Blue debug string is cited in [scripts.md](scripts.md). No decompiler output, ROM bytes or table values are in this note.

## Status by point

| Point | Status | What is open |
|-------|--------|--------------|
| P1 opcode list, operands, groups; `0214D4FC` | **blue-verified** for the dispatch, the handler of each of the 247 slots and the fields each handler reads | The bodies of most callees (window `0213Cxxx`, screen `0213Axxx`, camera `02140xxx`, sound `0201Bxxx`), so many "What" readings stay **red-hypothesis** |
| P2 flow control | **blue-verified** (static and runtime) | Who signals tokens 0 and 1 when a text box or menu closes |
| P3 variable model | **blue-verified** | Whether a scene variable is reset on load; the LOCAL2/LOCAL3 overlap with the pending slot |
| P4 runtime log | **blue-verified** for the opcodes that ran from boot to the first dungeon and back to town | Every opcode the opening does not use (list below) |

## P1 — Dispatch and the opcode table

| Claim | Evidence | Status |
|-------|----------|--------|
| `ov_0000::0214D4FC` is not a jump table. It is a tail call to `FUN_0214D510(out, bank, 0, 0)`: it loads the pool word at `0x0214D50C`, sets group and sector to 0 and branches. The pool word is `0x0214D510`. Ghidra's "could not recover jumptable" warning is only the indirect branch. Opcode `0xEC` reaches it through `FUN_0214D3A0` to get the entry script of sector (0, 0) of a bank. | Disassembly of `ov_0000::0214D4FC`; pool word read from `overlay_0000.bin` | **blue-verified** |
| The real dispatch is at `0x02142344` (`cmp r2, #0xF6; addls pc, pc, r2 lsl #2`) through 247 `B` instructions from `0x0214234C`. Opcodes above `0xF6` go to the error handler `0x02146164`. The 247 slots reach 164 distinct handlers. | Disassembly; table decoded from `overlay_0000.bin` (decode not committed) | **blue-verified** |
| Ghidra labels the loop head `switchD_..02143718`; `0x02143718` is the sub-switch of opcode `0x2D`. The decompiler's `case` numbers match the decoded table for all 247 slots. | Export of `FUN_ov_0000__021421CC` | **blue-verified** |
| Interpreter return values, stored by the stepper in ctx+0x26: 0 end (`0xEF`, or `0xEE` with no saved return), 1 suspend (`0xF0`), 2 wait, 3 run on (`0x21`), 4 finished (`0xF1`, and `0x1D`/`0x1E` when the bank changed on a context that is not the main one). | `FUN_ov_0000__021421CC`, `FUN_ov_0000__021461D0` | **blue-verified** |

### Record fields

A command is 6 words (0x18 bytes). Word 0 is `[op u8][b u8][c s16]`, word 1 is `A` (`As` is its low s16), word 2 is `B`. Words 3–5 are used only by text and menu ops: `FUN_ov_0000__021421A0` returns word 3 as the inline string pointer and, for a language index of 1 or more, adds a u16 offset from word 4 onward. Default rule for group and sector: `c < 0` takes ctx+0x30 (s16), `b < 0` takes ctx+0x32 (byte). `kind` is the s16 at ctx+8: 0 main or ground script, 1 lives, 2 object, 3 event (asserted by `0x16`–`0x18`, `0x21`, `0x98`–`0xA3`).

### Table

Groups: FLOW, VAR (variables and flags), WAIT, SCENE (scene, map, bank, mode), ACTOR, CAM, DLG (dialogue and menus), SND, OTHER. **Status for every row:** which handler runs and which fields it reads is **blue-verified** from the decompile; the "What" text is **red-hypothesis** where the callee's body was not read.

| Op | Reads | Group | What |
|----|-------|-------|------|
| 00, 40, 96 | – | OTHER | Unused: error log, continue |
| 01 | As, b, c | SCENE | Request a mode transition: `FUN_0206F628(As, b, c)` |
| 02 | As (−1 = var `0x14`), c | SCENE | Request dungeon entry `FUN_0206F438(dungeon, c)`, then a case dispatch on 1 (0 when As = −1) |
| 03 | A, c | SCENE | Condition `FUN_020424B0(A)`; false: request `FUN_0206F5B8(c)` and case 0; true: case −1 |
| 04 | As (−1 = ask) | DLG/SCENE | As ≠ −1: var `0x13` = `FUN_020707B4(As)`; else open the selector `FUN_0213C1C4(0xC)`. Returns 2; the stepper writes var `0x13` from the choice |
| 05 | – | SCENE | ctx+0x4C = 1, return 2 |
| 06 | As (−1 = ask) | SCENE/DLG | Dungeon-enter chooser `FUN_0213C1C4(0x24)`, or var `0x14` set; return 2 |
| 07 | As | SCENE | As = −1: read var `0x14`, else var `0x14` = As; ctx+0x4C = 1 or −1; return 2 |
| 08 | As (map) | SCENE | Current map = As, adjusted map = `FUN_0214D208(As)`, bank = `FUN_0214D1F0(As)` (maps 3 and 4 give bank 3); var `0x11` = map, var `0x12` = map record +2; layer reset and load |
| 09 | As | SCENE | As 08, but the adjusted map is the previous current map and the bank is not folded |
| 0A | As (dungeon), B (map), b, c | SCENE | Map = B, `FUN_02070890(As)` entry info, `FUN_0214CB50(adjusted, {info+0xC, c}, b)` |
| 0B | As | SCENE | `FUN_0214DCDC(As)` (meaning **lead**) |
| 0C | c group, b sector | SCENE | Load all four layers (lives `02155314`, objects `02156980`, effects `021579AC`, events `02158118`) for the current bank; asserts kind 0 |
| 0D–10 | c, b | SCENE | One of those layers each |
| 11 | c, b | SCENE | Cancel all four layers (`0215528C`, `021568FC`, `02157928`, `02158090`); kind 0 |
| 12–15 | c, b | SCENE | Cancel one layer each; `0x12`–`0x14` assert kind 1, 2, 3 |
| 16, 17, 18 | – | SCENE | Cancel the blank entries of lives, objects, effects (`021551CC`, `0215683C`, `02157868`; assert kind 1, 2, 3) |
| 19, 1A | A (table index, stride 0xC, +8), B, c, b | ACTOR | Add an object (kind 2) or effect (kind 3) at this context's position (`02156360`, `02157540`) |
| 1B | c | FLOW | `FUN_0214D498(c, 0)`: build a reference to loader entry c and install it on the main context (`*0x0215D1A8`) |
| 1C | c | FLOW | As 1B with flag 1 (kind 5); asserts kind 0 |
| 1D, 1E | A (−1 = current map), c, b | SCENE | Bank = `FUN_0214D1F0(map)`; `FUN_0214D420(bank, group, sector, op == 0x1E)`; on a bank change `FUN_02147B48`; returns 4 when kind ≠ 0 |
| 1F | – (var `0x14`) | SCENE | Enter the dungeon in var `0x14`: the intro script through `FUN_0214D498` the first time, else `FUN_0206F438` |
| 20 | c | FLOW | Run loader entry c on this context's own kind (main, lives `02153F08`, object `02156140`, effect `02157360`) |
| 21 | As | ACTOR | Kind 1: look up lives As (`021550D4`), face or approach it, return 3 |
| 22–26 | c, b (`0x24` also A, B) | OTHER | Screen effect calls `0213A11C`, `0213A104`, `0213A0D4`, `0213A0BC`, `0213A0A4`; b ≠ 0 waits on `FUN_02139E64` |
| 27, 28 | c, A, B (colour bytes), b | OTHER | Screen effect with a colour (`02139FC4`, `02139EE4`); b ≠ 0 waits |
| 29, 2A, 2B | c / c / A, B | DLG | Message-box parameters (`0213D358`, `0213D33C`, `0213D2F0`) |
| 2C | – | DLG | If a box is open (`0213D2CC`): wait on token 0 |
| 2D | b (sub-mode 0–10), c, A | DLG | Bind a speaker or a name placeholder: b 0–9 pick the source (this entity, lives As, …); b = 10 puts a name from `FUN_02070710` into slot c |
| 2E | c (portrait slot), A low byte, b | DLG | Portrait slot `FUN_0213C754` ([../formats/portraits.md](../formats/portraits.md)) |
| 2F | c, A, B | DLG | Portrait slot update `FUN_0213C66C` |
| 30, 31 | – | DLG | Box close `FUN_0213D254` / force `FUN_0213D1F4` |
| 32–38 | c, words 3–5, b | DLG | Show text: speaker kind 0 (`0x32`), 1 (`0x33`), or from three stack values (`0x34`–`0x36`) through `FUN_0213C560`; no speaker `0213C510` (`0x37`); `0213C4C0` (`0x38`). Then wait on token 0 |
| 39 | c, text | DLG | Text with a condition (`0213C470`), then wait on token 0 when A ≥ 0 |
| 3A | b, A, Bs, text | DLG | Yes/no question `FUN_0213C410(b ≠ 0, A, B, text)`; wait on token 1, then poll `FUN_0213C3D8` and case-dispatch on the answer |
| 3B | – | OTHER | ctx+0x4E = 0, return 2; the stepper polls `FUN_02140860(ctx, b, c, A)` (script special) and case-dispatches on its result |
| 3C | b, A, B, text | DLG | Query window `FUN_0213C104`; the stepper polls `FUN_0213C0CC` and case-dispatches on the result |
| 3D, 3E, 3F | As / – / b | DLG | Name-entry screens (`FUN_0213C1C4` kinds 4, 5, 6); results case-dispatched; `0x3D` with result 1 copies the name to the lives record +0x36, `0x3F` compares a password |
| 41 | c low byte | SND/OTHER | `FUN_02007AF8(c)`, then `FUN_02007980` when ≥ 0 |
| 42, 43 | –, c (< 0 gives 30) | OTHER | Fade `FUN_02029B90`, `FUN_02029B60(frames)`; waited by `0xE0` |
| 44–48 | b, A (id; b = 0 maps it through `02140444`), c (< 0 gives 30) | SND | BGM start, start with fade, `0201BE78`, stop (`0201BE04`), fade out (`0201BD9C`); id `0x3E7` stops |
| 49–4E | A, c | SND | `0201BC30(A, 0x100)`, `0201BA30(A)`, `0201B9B8(A, c)`, two opcodes each |
| 4F | – | ACTOR | Clear the pending slot, owner vtable +0x1C(id, 0) |
| 50 | – | ACTOR | Copy position and direction from the linked context |
| 51 | c | ACTOR | Position from area c (`02157BE8`), vtable +0x20 |
| 52, 53 | A | ACTOR | Owner vtable +0x3C / +0x40 (flag words; `0x53` with bit `0x400` clears the pending slot) |
| 54 | c | ACTOR | Animation: ctx+0x48 = c when nonzero, vtable +0x30 |
| 55, 56, 57 | c / As, c / c | ACTOR | Vtable +0x34 / +0x38 / +0x44 |
| 58 | A, B, b | ACTOR | Set position (A << 8, B << 8), ctx+0x4E = b, return 2 |
| 59 | c, A | ACTOR | Offset move through vtable +0x48 (c << 8, A << 8), ctx+0x4E = b, return 2 |
| 5A | c, A | ACTOR | Random position (`rand(c) << 8`, `rand(A) << 8`) |
| 5B, 5C, 5D | As | ACTOR | Position from mark As / random point in mark As / position of lives As |
| 5E, 5F | A (save key) | ACTOR | Restore / save position, height and direction through vars `0x34`–`0x37` |
| 60 | A, b | ACTOR | Height A << 8 |
| 61–70 | A, B or As, c (speed), b | ACTOR | Walk: `0x61`/`0x69` absolute, `0x62`/`0x6A` relative, `0x63`/`0x6B` mark, `0x64`/`0x6C` mark with timer b, `0x65`/`0x6D` mark with a random timer, `0x66`/`0x67`/`0x6E`/`0x6F` lives As, `0x68`/`0x70` height. The `0x69` half also turns to face the motion. The stepper walks until arrival |
| 71–88 | see below | ACTOR | Timed slide |
| 89 | A byte, c (speed), b | ACTOR | Set direction now |
| 8A, 8D | A byte / c byte (mode), b | ACTOR | Rotate the direction (`FUN_02021D9C`, 10 modes) |
| 8B | c | ACTOR | Direction = c |
| 8C | As, c, b | ACTOR | Direction of lives As, rotated by c |
| 8E, 8F, 90 | As, c, b | ACTOR | Face lives As / its position / mark As |
| 91, 92 | A byte / mode | ACTOR | Turn target ctx+0x71 |
| 93, 94, 95 | As, c | ACTOR | Turn step by step toward lives As / its position / mark As, and wait |
| 97 | c, A, B | CAM | `FUN_02140380(c, A, B)`: shake |
| 98, 99, 9A | – | CAM | Camera target by kind (`0214033C`), to a position (`02140358`), reset (1, 0) |
| 9B, 9C, 9D | c (frames) | CAM | Timed move (`021402F0`/`02140310`): to the entity, to its position, default; return 2 |
| 9E, 9F, A0 | c | CAM | Second timed form (`021402A4`/`021402C4`) |
| A1, A2, A3 | c | CAM | Third timed form (`02140258`/`02140278`); `0xA3` waits with `FUN_02140238` |
| A4 | c (var) | VAR | Reset var c to its descriptor's initial value (`FUN_0202125C`) |
| A5 | c | VAR | Clear var c (`FUN_020211AC`) |
| A6 | c var, A, b op | VAR | var c = var c OP A (`FUN_0201FDD8`) |
| A7 | c var, As var, b op | VAR | var c = var c OP var As (`FUN_0201FD8C`) |
| A8 | c var, A index, B | VAR | var c[A] = B (`FUN_020208E8`) |
| A9 | c var, A, B | VAR | SetScenario: set the pair, with side effects (`FUN_020206AC`, see [story-flags.md](story-flags.md)) |
| AA | c var, A | VAR | Advance a scenario: assert its stage is A, sub += 1 |
| AB | A, c | VAR | var `0x14` = A, var `0x17` = c |
| AC | c | VAR | var `0x1D` = c |
| AD–B1 | c, b ≠ 0 | VAR | Set or clear a flag in other save tables (`0201F8A8`, `0200FFD4`, `0200FED0` area unlock, `0200FDAC`, `0201020C`) |
| B2 | c byte | VAR | `FUN_02010DFC(c)` |
| B3 | c global var, A, b label | FLOW | If var c == A, go to label b |
| B4 | As var, B, b op, c label | FLOW | If var As OP B |
| B5 | As var, Bs var, b op, c label | FLOW | If var As OP var Bs |
| B6 | As var, B index, c label | FLOW | If var As[B] ≠ 0 |
| B7 | As var, B, b op, c label | FLOW | If sum(var As) OP B |
| B8, B9, BA | c var, A, B, b label | FLOW | Scenario compare: before (A, B) / exactly / past (B < 0 ignores the sub) |
| BB | c, b label | FLOW | Story check `FUN_0201F994(c)` ([story-flags.md](story-flags.md)) |
| BC | c, b label | FLOW | `FUN_02011570(c)` |
| BD | As, c label | FLOW | `FUN_0201F8E8(As)` |
| BE | b label | FLOW | Kind 1 and `FUN_02153AEC(ctx+0xA)` |
| BF | c byte, b label | FLOW | `FUN_0213F6A0(c) > 0` |
| C0–CB | see below | FLOW | Compute a value, then case-dispatch on it |
| CC, CD, CE | c label, A, b op | FLOW | Case records (read by the scan, never executed) |
| CF | c var; `0xD0`/`0xD1` records | DLG | Message variant by the value of var c: scan the `0xD0` records for a match, `0xD1` is the default |
| D0, D1 | – | DLG | Records of `0xCF` |
| D2–D8 | b, c, As, B | DLG | Menu built from the following `0xD9` records (`FUN_0213C210`); `0xD6`–`0xD8` pick the title through a variable and `0xD0`/`0xD1` records; wait on token 1, then case-dispatch on the choice |
| D9 | – | DLG | Menu item record, 0xC-byte entries built at `0x0215D35C` |
| DA | – | WAIT | Wait on token 1, then case-dispatch on its result |
| DB | c | WAIT | Wait c frames (ctx+0x4E) |
| DC | c, A | WAIT | Wait c + rand(A − c) frames |
| DD, DE | – | WAIT | Wait while owner vtable +0x4C / +0x50 reports busy |
| DF | – | WAIT | Wait while `FUN_02139E64` (screen effects) is busy |
| E0 | c | WAIT | Wait while `FUN_02029AE0(c)` (fade) is busy, with a frame limit |
| E1, E2 | c | WAIT | Wait while `FUN_0201B940(c)` (sound) is busy, limit 0xE10 frames |
| E3 | c | WAIT | Wait on token c (`FUN_02147B80`), then case-dispatch on the token's result word |
| E4 | c | WAIT | Signal token c (`FUN_02147AAC`) |
| E5 | c, b | WAIT | Wait on token c with result b (`FUN_021479B8`, handshake bit `0x80`) |
| E6 | c | FLOW | Call label c (one saved return) |
| E7 | c | FLOW | Go to label c |
| E8 | c | FLOW | Call loader script c |
| E9 | c | FLOW | Go to loader script c |
| EA | c, b | FLOW | Call the script of group c, sector b of the current bank |
| EB | c, b | FLOW | Go to that script (A must be −1; kind 0) |
| EC | c var | SCENE | Bank = `FUN_0214D1F0(var c)`; `FUN_02147B48`; `FUN_0214D3A0(bank)` |
| ED | – | FLOW | Drop the saved return |
| EE | – | FLOW | Return |
| EF | – | FLOW | End the script (return 0) |
| F0 | – | FLOW | Suspend (return 1) |
| F1 | – | FLOW | Finish (return 4) |
| F2–F6 | – | FLOW | No-op; `0xF4` is a label (id in c); `0xF6` stops the label scan |

**Slides `0x71`–`0x88`.** Six target forms times four variants. Forms (opcode offset within each block of six): +0 absolute (A << 8, B << 8), +1 relative, +2 relative with a random jitter (`rand(2A − 1)`), +3 mark As, +4 mark with jitter, +5 lives As. Variants: `0x71`–`0x76` fixed c frames, no turn; `0x77`–`0x7C` fixed c frames, turn along the path; `0x7D`–`0x82` frames from distance and speed c (`FUN_0208625C`), no turn; `0x83`–`0x88` the same with a turn. **blue-verified** from the setup code and the stepper's wait groups (`ov_0000::0214699C`, `ov_0000::02146A0C`).

**Value ops `0xC0`–`0xCB`** (the value then goes to `FUN_021405B4(ctx, value)`): `C0` var c; `C1` var c OP A (op b; op `0xB` is `rand(A)`); `C2` var c OP var As; `C3` `rand(c)`; `C4`/`C5` element 0 / 1 of var c; `C6` `FUN_02153BB0(As)`; `C7` own direction; `C8`, `C9` direction to lives As; `CA` direction to mark As; `CB` `FUN_0213F4E0(c)`. **blue-verified** (calls).

**Compare ops** (`FUN_0201FE24`): 0 true, 1 false, 2 ==, 3 >, 4 <, 5 >=, 6 <=, 7 !=, 8 `(a & b) != 0`, 9 !=, 10 bit test. **Arithmetic ops** (`FUN_0201FF48`): 0 set, 1 sub, 2 add, 3 mul, 4 div, 5 mod, 6 and, 7 or, 8 xor, 9 set bit, 10 clear bit, `0xB` `rand(b)`. **blue-verified** (static).

## P2 — Flow control

| Claim | Evidence | Status |
|-------|----------|--------|
| Before a command runs, the cursor ctx+0x28 advances by 0x18 and ctx+0x40 holds the old cursor. A branch writes ctx+0x28. | Loop head `ov_0000::02142304` | **blue-verified** |
| Label jumps (`0xE7`, the call `0xE6` and every conditional `0xB3`–`0xBF`) go through `FUN_021406A8(ctx, id)`: scan from the script base ctx+0x2C for an `0xF4` record whose halfword 1 is the id; the cursor becomes the record after it. The scan asserts on opcode 0 or `0xF6`. | `FUN_ov_0000__021406A8` | **blue-verified** |
| One call level only. `0xE6` and `0xE8` save cursor, base and group/sector at ctx+0x34/+0x38/+0x3C, and log if a return is already saved. `0xEE` restores them (and returns 0 when none is saved). `0xED` drops the saved return. | Cases `0xE6`, `0xE8`, `0xED`, `0xEE` | **blue-verified** (static); runtime: 17 of 17 `0xEE` returned to the record after their `0xE8` |
| Case dispatch `FUN_021405B4(ctx, value)`: from the cursor, each following `0xCC` (equal to A), `0xCD` (op b against A) or `0xCE` (op b against var As) record is tested; a match jumps to its label c; the first other record ends the scan and is the default path. A SWITCH is therefore a value op, its case records, then the default code. | `FUN_ov_0000__021405B4` | **blue-verified** |
| Jumps to another script: `0xE8`/`0xE9` set cursor and base to word 0 of loader entry c (table at `0x020DA5D4`, stride 0xC); `0xEA`/`0xEB` take the first script of group c, sector b of the current bank through `FUN_0214D510`, and update ctx+0x30/+0x32. `0x1B`, `0x1C` and `0x20` do not jump: they install a reference in a context's pending slot (`FUN_02147C48`). `0x1D`, `0x1E` and `0xEC` load a bank and its entry script. | Cases above | **blue-verified** (static); runtime: 21 of 21 `0xE8`/`0xE9` landed on the loader entry's pointer |
| Stepper states (ctx+0x26, `FUN_021461D0`): 0 idle, 1 suspended, 2 waiting, 3 running, 4 finished. State 3 calls the interpreter and stores its result. Result 0 copies the pending slot (ctx+0x84) into the active slot (ctx+0x24) when ctx+0x86 is set, else clears it. In state 2 a switch on the last opcode (ctx+0x44) tests the wait; when it passes it sets state 3 (often after a case dispatch) and runs on in the same frame. | `FUN_ov_0000__021461D0` | **blue-verified** (static) |

### Sync tokens (how scripts wait for each other)

| Claim | Evidence | Status |
|-------|----------|--------|
| `FUN_02147B80(ctx, token)` makes a context wait: ctx+0x46 = token, state 2, and a global dirty byte is set. Token 0 with no message box open does not wait; it case-dispatches on 0 at once (`FUN_0213D2D4`). Users: `0x2C`, `0x32`–`0x3A`, `0xD2`–`0xD8`, `0xDA`, `0xE3`. | `FUN_ov_0000__02147B80` | **blue-verified** |
| `FUN_02147AAC(token)` signals a token: it sets the token's flag, zeroes its result word and sets the dirty byte. Callers: `0xE4` and the input controller `FUN_0213F7D8` (token 2). | `FUN_ov_0000__02147AAC`, export x-refs | **blue-verified** |
| `FUN_021479B8(ctx, token, result)` (`0xE5`) stores a result word for the token and waits on `token | 0x80`, a handshake that is released only after the signal has been seen. | `FUN_ov_0000__021479B8` | **blue-verified** |
| Release: once per frame the scene runner calls `FUN_0214074C`. When the dirty byte is set, it walks tokens 0..0x80, and for each signalled token clears ctx+0x46 on every waiting context of the main context, the 24 lives, the 16 objects and the 32 events (`FUN_02147AE8` through `0214D384`, `02153CA4`, `021560E4`, `02157304`). A waiter of `0xE3`/`0xE5`/`0xDA` then case-dispatches on the token's result word. | `FUN_ov_0000__0214074C`, `FUN_ov_0000__02147AE8` and its four callers | **blue-verified** (static) |
| So `0xE3` c / `0xE4` c is how one script waits for another: the cutscene's main script waits on a token, and an actor's script signals it when its move ends. | Pairing in the runtime log: `0xE3` 45 and `0xE4` 43 times in the boot run, mostly alternating with `0xDB` and the actor ops | **blue-verified** (runtime counts); "cutscene pattern" **red-hypothesis** |
| Who signals tokens 0 and 1 when a text box or menu closes is not found: the only callers of `FUN_02147AAC` are `0xE4` and `FUN_0213F7D8`. The window state that the stepper polls is `+0x420 == 3` with the result at `+0x430` of the window struct (`FUN_0213C0CC`). | Export x-refs | **lead** |

## P3 — Variable model

| Claim | Evidence | Status |
|-------|----------|--------|
| `FUN_02021108(&out, base, id)` resolves a variable. Ids below `0x400` use descriptor table A at `0x02098060` (16-byte entries, 83 valid ids `0`..`0x52`; the range assert is stripped in retail) and the data block at `0x0210E640` + offset (bytes). Ids `0x400` and up use table B at `0x020977F8` and the data at `base` + offset × 4. `base` is ctx+0x74 for the ops that pass it, 0 for the global-only ops (`0xA9`–`0xAC`, `0xB3`, `0xB8`–`0xBA`, `0xC4`, `0xC5`, `0x5E`, `0x5F`). | `FUN_02021108`; the `lsl #2` at `0x0202118C` | **blue-verified** |
| The data block base word is `0x0210E640`. The pool words at `0x020211A0` (resolver), `0x0201F87C` and `0x0201F8A4` (save and restore) all hold it. [../formats/save.md](../formats/save.md) said `0x021EE640`; that was a typo and is fixed there. | Pool words read from `arm9.bin` | **blue-verified** |
| Descriptor: `+0` type (1 bit, 2 and 7 u8, 3 s8, 4 u16, 5 s16, 6 s32, 8 computed), `+2` unused in A, `+4` offset, `+6` bit shift, `+8` count, `+0xA` initial value, `+0xC` debug name pointer. The used part of the block ends near offset `0xE5`; the block is 0x400 bytes. | Decompile and runtime read of the names | **blue-verified** |
| Locals: table B has 4 valid entries, `0x400`–`0x403` (LOCAL0–LOCAL3), type s16, at ctx+0x74, +0x7C, +0x84 and +0x8C. LOCAL2 and LOCAL3 fall inside the pending slot (ctx+0x84..+0xE4) by the arithmetic; no local was used in the runs. Locals are not saved. | `FUN_02021108` | **blue-verified** (static); the overlap **lead** |
| Type 8 (computed, no storage) only for: 2 LANG_MODE (`FUN_0202A040`), `0x18` SCRIPT_MODE (`FUN_0206F348() != 0`), `0x21` FRIEND_SUM (`FUN_02002D70`), `0x22` UNIT_SUM (`FUN_02002BE8(0)`), `0x23` GOLD and `0x24` BANK_GOLD (team struct +0x234 / +0x238), `0x27` HERO_TALK_KIND (4 or 5 from `FUN_0200CB1C`). The getter's cases for `0x25` and `0x26` are unreachable. Setters accept type 8 only for `0x23` and `0x24`. | `FUN_02020F74`, `FUN_02020B48` | **blue-verified** |
| Getters: `FUN_02020F74(base, id)` element 0, `FUN_02020D90(base, id, index)` one element (bit n for type 1), `FUN_02020848(id, &a, &b)` elements 0 and 1, `FUN_02020884` the sum. Setters: `FUN_02020B48` element 0, `FUN_020208E8` one element, `FUN_020206AC` SetScenario, `FUN_0201FDD8`/`FUN_0201FD8C` compound, `FUN_0202125C` reset to initial, `FUN_020211AC` clear. | ARM9 functions above | **blue-verified** |
| Saving: `SaveGlobalScriptVars` `0x0201F884` and `RestoreGlobalScriptVars` `0x0201F81C` copy the whole block raw, so every table-A variable of types 1–7 is saved, scene-like ones included. Type-8 values are not in the block. `FUN_02021310` (new game, failed load) zeroes the block, writes each descriptor's initial value, resets the scenarios and then sets ids `0xE`–`0x20` explicitly (values not recorded here). | ARM9 functions above | **blue-verified**; whether any variable is reset on load **lead** |

## P4 — Runtime log

Probe `verification/v7-trace/ground_script_log.py` hooks the dispatch at `0x02142340` (opcode in r2, ctx = r6 − 0x24) together with the variable setters, the scene runner, the loader and bank installs and the ground interaction calls. Run A goes from boot with `boot_to_dungeon.txt` through the first dungeon and back to town. Command and hash are in [Trace checks](#trace-checks). The counts below come from that run.

| Claim | Evidence | Status |
|-------|----------|--------|
| Run A, boot to the first dungeon floor (frames 0–25020; the input does not clear the floor): 603 interpreter dispatches on 9 contexts, by kind 0: 147, 1: 347, 2: 65, 3: 44; 71 distinct opcodes. The log also has 3 rows at frame 25017 with a context below `0x02000000`: overlay 1 code at the same address after overlay 0 was unloaded, not dispatches. | Run A | **blue-verified** (runtime) |
| Counts (hex opcode, count): DB 65, E3 45, E4 43, 54 34, 80 24, 30 22, 53 18, 6B 17, E8 17, EE 17, 91 14, 2E 14, 34 13, 4C 13, 52 12, 8B 12, CF 12, A6 11, 56 11, 2D 10, 0C 9, 22 9, DE 9, F1 8, 23 8, 3B 8, 1E 7, 08 7, EF 7, 44 6, C4 6, 37 6, 70 6, F0 5, 39 5, 33 5, 47 4, 98 4, A9 4, E9 4, 4D 4, E2 4; 3 or fewer: 5B, 99, 48, A5, E7, 92, 3C, F4, BE, 68, 11, 28, E0, 27, C5, F2, AB, AC, 26, C0, 25, 45, DF, DD, 3D, 0D, E5, 02, AF. | Run A | **blue-verified** (runtime) |
| Flow: 17 `0xE8` calls and 17 `0xEE` returns; 4 `0xE9`; 3 `0xE7`; 7 `0x1E` bank loads; 8 `0xF1`, 5 `0xF0`, 7 `0xEF`. Sync: `0xE3` 45 and `0xE4` 43, mostly between `0xDB` waits and actor commands. Actor commands ran only on kinds 1–3; kind 0 ran loads, screen effects, dialogue and flow. | Run A | **blue-verified** (runtime) |
| Run B (a save at the Team Base, walk, exit, talk) adds `0x01`, `0x10`, `0x1D`, `0x32`, `0xB4`, `0xB6`, `0xBB`, `0xCA`, `0xEA`, `0xEC` (see [ground-mode.md](ground-mode.md#p3--interaction-and-exits)). | Run B | **blue-verified** (runtime) |
| 166 opcodes ran in neither run (`0x00` and the other error slots included): 00, 03–07, 09–0B, 0E, 0F, 12–1C, 1F–21, 24, 29–2C, 2F, 31, 35, 36, 38, 3A, 3E–43, 46, 49–4B, 4E–51, 55, 57–5A, 5C–67, 69, 6A, 6C–6F, 71–7F, 81–8A, 8C–90, 93–97, 9A–A4, A7, A8, AA, AD, AE, B0–B3, B5, B7–BA, BC, BD, BF, C1–C3, C6–C9, CB–CE, D0–DA, DC, E1, E6, EB, ED, F3, F5, F6. They rest on the static reading only. | Runs A and B | – |

## Trace checks

| Script | Output hash | Command | Claims checked |
|--------|-------------|---------|----------------|
| `ground_script_log.py` (run A) | output SHA-256 `a4ffb5d2cbf8e766dc3ce2bc780715948be6e6a77cd4c88035e4f5f21cd52bb7` | `XDG_CONFIG_HOME=<fresh> ~/venvs/desmume/bin/python -I verification/v7-trace/ground_script_log.py <scratch ROM copy> <OUT.jsonl> 36000 verification/v7-trace/boot_to_dungeon.txt` | P2 jumps and returns; P4 counts; [story-flags.md](story-flags.md) P2 |
| `ground_script_log.py` (run B) | see [ground-mode.md](ground-mode.md#trace-checks) | see there | P4 opcodes of a walk, an exit and a talk |

Input `boot_to_dungeon.txt` has SHA-1 `b3f969cec1bb5aeb790c50e62727b5cdc5686d61`. An earlier investigator run of the same hook (frames 0 to 24960, 603 dispatches) had output SHA-256 `49ef8a97795f753ae83a46f5c79af1a9c5bfda277844d81906b03b013799844b`.

## Links

- [scripts.md](scripts.md) (engine, stepper, actors, camera, dialogue)
- [story-flags.md](story-flags.md) (scenario variables and gates)
- [ground-mode.md](ground-mode.md) (movement, collision, interaction)
- [../formats/scripts.md](../formats/scripts.md), [../formats/save.md](../formats/save.md)
