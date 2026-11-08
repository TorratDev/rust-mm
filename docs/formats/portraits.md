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

**[blue-verified]** for position, tile order and palette, from one frame (the town dialogue with Charmander speaking, #62 B 6 and 7). The mirror rule is **medium**: one sample.

- The portrait is 25 consecutive BG0 tiles (VRAM `0x06001700` onward), placed row-major at BG0 columns 24 to 28 and rows 10 to 14 (pixels x 192 to 232, y 80 to 120). The map entries have no flip bits.
- It uses BG palette bank 14, the `kao004` slot-0 palette (16 of 16 colours). The text uses bank 15 ([text-engine.md](../re/text-engine.md#colours)).
- The frame is 7 by 7 tiles at BG0 columns 23 to 29 and rows 9 to 15 (pixels x 184 to 240, y 72 to 128). It sits 8 px above the dialogue box (box top y 136) and shares the box's right edge at x 240. Tiles and roles: [ui-screens.md](../re/ui-screens.md#3-frame-tile-roles).
- **Mirror (medium, one sample):** the displayed image is the horizontal mirror of the decoded slot 0 (1588 of 1600 pixels equal after an exact pixel hflip; 12 pixels differ, in dither rows 6, 7 and 17). The map has no flip bits, so either the game stores mirrored pixels, or the emotion or the side makes the face point that way. Whether the mirror depends on the side is not verified: this capture has the portrait right of the box. A second capture (a portrait on the left, or another speaker) would settle the rule.

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
- Which emotion slot the game picks in a frame, and whether the side or the speaker decides the mirror: not verified.
