# Portraits (`monster.sbin` `kaoNNN`)

Issue: [#34](https://github.com/TorratDev/rust-mm/issues/34). Code: `crates/formats/src/portrait.rs`. Tests: unit test; `tests/tests/formats_rom.rs` (`portraits_decode`), `tests/tests/viewer_rom.rs`. Viewer: select `monster.sbin` → `kaoNNN`.

**Verification status:** verified against Blue (2026-10-16). 73 files, all decode; images checked by eye (Bulbasaur's 13 faces).

## Resource IDs

`NNN` is the species index (`monspara` order, [`data.md`](data.md)): `kao001` Bulbasaur, `kao025` Pikachu. Only 73 species have a portrait file. Within a file, the slot number is the emotion.

## Layout

SIR0 ([`archive.md`](archive.md#sir0)). The content header is 13 pairs, one per emotion slot:

| Offset | Size | Field |
|--------|------|-------|
| `+0` | 4 | pointer to the palette: 16 RGBX colors (64 bytes) |
| `+4` | 4 | pointer to the image: AT4PX, 800 bytes decoded |

A missing emotion has both pointers 0. The decoded image is 40x40 pixels, 4bpp, as 5x5 tiles in row-major order. Color 0 is drawn (portraits fill their frame). **Not confirmed by #62:** the slot-0 image of `kao004` in the #62 capture uses only palette indices 1 to f, so that frame does not show whether colour 0 is drawn.

## Placement in the dialogue window (#62)

**[blue-verified]** for position, tile order and palette, from one frame (the town dialogue with Charmander speaking, #62 B 6 and 7). The mirror is covered in the next section.

- The portrait is 25 consecutive BG0 tiles (VRAM `0x06001700` onward), placed row-major at BG0 columns 24 to 28 and rows 10 to 14 (pixels x 192 to 232, y 80 to 120). The map entries have no flip bits.
- It uses BG palette bank 14, the `kao004` slot-0 palette (16 of 16 colours). The text uses bank 15 ([text-engine.md](../re/text-engine.md#colours)).
- The frame is 7 by 7 tiles at BG0 columns 23 to 29 and rows 9 to 15 (pixels x 184 to 240, y 72 to 128). It sits 8 px above the dialogue box (box top y 136) and shares the box's right edge at x 240. Tiles and roles: [ui-screens.md](../re/ui-screens.md#3-frame-tile-roles).
- **Mirror:** the flip is set by one descriptor byte, see [Mirror rule](#mirror-rule-62-b-follow-up). The map itself has no flip bits.

## Mirror rule (#62 B follow-up)

**Tags:** the flag byte and the flip routine are **[blue-verified]** (static, high). The position table is **[blue-verified]** (static decode and a RAM dump, high). The actor-id emotion exceptions are **[blue-verified]** (static) with **medium** confidence on the id ranges; their meaning ("pre-mirrored art") is **[hypothesis]**. The two runtime samples agree with the rule. The 12-pixel residual is **[medium]**.

**The flag.** The portrait is mirrored by one byte of its descriptor, not by side, speaker or emotion. `FUN_0202ee6c` (the dialogue-window opener) receives the descriptor as `param_8`. If the portrait is present, at its end:

- `*(param_8 + 0xD) == 0`: `FUN_01ff916c(1, image, 0xE)` loads the image as it is;
- otherwise: `FUN_01ff9010(1, image, 0xE)` loads it mirrored.

Both load the 40x40 image into BG graphics slot 1, palette bank 14 (`FUN_02011cb4(0xE0 + i, palette)` earlier in the same function). `FUN_01ff9010` calls `FUN_01ff8f14`, which swaps tile column c with column 4 − c in every tile row and reverses the nibbles of each 32-bit tile row (`FUN_01ff9040`). That is an exact pixel hflip of the 40 columns.

`param_8` fields: `+4` pointer to the array of {palette pointer, image pointer} pairs (8 bytes per emotion); `+8` and `+0xA` x and y in tiles; `+0xC` emotion slot index; `+0xD` mirror flag; `+0xE` a second flag (0 gives window field 5, nonzero gives 7; meaning not checked).

**Where the flag comes from (script portraits).** The portrait slot setup `ov_0000::0213c754` (callers `ov_0000::0213c5d4`, `021421cc`) takes a position id `p` from the script. It stores `x = T[p].x`, `y = T[p].y` and `slot+0x1D = T[p].flag`. `T` is a 21-entry table of 6-byte records `{s16 x, s16 y, u8 flag, pad}` at ov_0000 RAM `0x02159DA0` (read from the emulator; the code reads it at `0213c820` and `0213c830`). The slots are 10 × 0x24 bytes at `*0x0215D184 + 0x43C`, and `param_8 = slot + 0x10`, so `param_8+0xD` is `slot+0x1D`.

| Position | x | y | Mirror flag |
|---|---|---|---|
| 0 | 2 | 8 | 0 |
| 1 | 12 | 5 | 0 |
| 2 | 2 | 8 | 0 |
| 3 | 23 | 8 | 1 |
| 4 | 7 | 8 | 0 |
| 5 | 18 | 8 | 1 |
| 6 | 13 | 5 | 1 |
| 7 | 2 | 8 | 1 |
| 8 | 23 | 8 | 0 |
| 9 | 7 | 8 | 1 |
| 10 | 18 | 8 | 0 |
| 11 | 12 | 1 | 0 |
| 12 | 2 | 2 | 0 |
| 13 | 23 | 2 | 1 |
| 14 | 7 | 1 | 0 |
| 15 | 18 | 1 | 1 |
| 16 | 13 | 1 | 1 |
| 17 | 2 | 2 | 1 |
| 18 | 23 | 2 | 0 |
| 19 | 7 | 1 | 1 |
| 20 | 18 | 1 | 0 |

Positions 0 to 10 use y 8 or 5 (lower-screen rows); 11 to 20 use y 1 or 2. The same x appears with both flag values (x = 2 at positions 0 and 7; x = 7 at 4 and 9; x = 18 at 5 and 10; x = 23 at 3 and 8). So the flag is a per-position choice made by the script, not derived from the side.

Placement, checked on both captures: the image origin is (x + 1, y + 2) tiles, and the frame origin is (x, y + 1).

**Actor-id emotion exceptions.** In the same function, the emotion `e = param & 0xF` is remapped before the load, by the actor id `sVar1` (the value `*slot`, not the kao species):

- actor 0x47: `e < 6` gives `e + 6`, always;
- actor 0x53: if flag ≠ 0 and `e == 0`, flag := 0 and `e += 1`;
- actor 0x73: if flag ≠ 0 and `e < 2`, flag := 0 and `e += 2`;
- actor ids 0x4D to 0x52: if flag ≠ 0 and `e < 4`, flag := 0 and `e += 4`. The findings give this range and also a branch test `0x4D <= id < 0x4E`; which one the game uses is not settled.

For these characters the mirrored faces are stored as extra emotion slots, and the runtime flip is turned off. That reading ("pre-mirrored art") is a hypothesis.

**Runtime samples (both agree with the rule).**

- Charmander, script slot 1: position 3 gives (23, 8, flag 1), so `slot+0x1D` = 1. The displayed image is decoded `kao004` slot 0, mirrored: 1588 of 1600 pixels equal after a pixel hflip; palette 16 of 16. The portrait is right of the box (cols 24 to 28, rows 10 to 14).
- Kecleon shop, `FUN_0202ee6c` entry hooked with py-desmume `register_exec`: `param_8 = 0x0211140C`, x = 2, y = 8, emotion 6, flag 0. The displayed image is decoded `kao380` slot 6 (emotion 0 + 6 from the actor-id rule), not mirrored: 1600 of 1600 pixels identical; palette 16 of 16. The portrait is left of the box (cols 3 to 7). `kao380` has only slots 0, 1, 6 and 7.
- Forcing `T[0].flag = 1` before the Kecleon dialogue did not mirror it. Kecleon's descriptor is not built by `0213c754` (its slot 1 has a null image pointer), so the shop path does not read the table. This is not a counter-example.

**Residual (medium).** The mirrored Charmander image matches the decoded file after an exact hflip except for 12 of 1600 pixels (±1 palette index in the dithered background, rows 6 and 7 of tile row 0, and row 17). The game's mirror routine is an exact flip. Whether the game's own AT4PX decode differs from `rescue-formats` for this file was not checked (the Kecleon image decodes 1600 of 1600). Unexplained.

**Rendering rule (for P2 and P4).** The portrait is 40x40 at (x + 1, y + 2) tiles, and the frame is 7x7 at (x, y + 1). Mirror if the descriptor flag is set. For script dialogue the flag is `T[position].flag` after the actor-id remap; for the Kecleon shop it is 0. The renderer needs the dialogue command's position id and emotion, which are script arguments, not text codes.

**Not verified.** Speakers and sides other than the two observed (partner at position 3, Kecleon shop). The table rule rests on code plus one mirrored sample. The Rust versus game decode of the 12 pixels. The exact id range in the actor-id exceptions. The `+0xE` flag. Who builds Kecleon's descriptor (`0x0211140C`).

## Emotion slots `[red-hypothesis]`

Named after Red's `EMOTION_*` constants, consistent with what the pictures show:

| Slot | Emotion | Slot | Emotion |
|------|---------|------|---------|
| 0 | normal | 7 | shouting |
| 1 | happy | 8 | teary-eyed |
| 2 | pained | 9 | determined |
| 3 | angry | 10 | joyous |
| 4 | worried | 11 | inspired |
| 5 | sad | 12 | surprised |
| 6 | crying | | |

## Unknown fields

- Whether `MonsterDataEntry.dialogueSprites` (bit per emotion) always matches the present slots: not checked.
- Which text control code switches portraits: none. The portrait is not a text code. It is the `param_8` struct of the window-open routine `FUN_0202ee6c`, and the `#[ … ]` callback is null in every sampled caller (#62 RE-a, [text.md](text.md#divergences-from-the-runtime-62)).
- Which emotion slot the game picks for a script line: the actor-id remap is known ([Mirror rule](#mirror-rule-62-b-follow-up)), but the script's emotion argument is not traced.
- Speakers and positions beyond the two observed at run time: not verified.
