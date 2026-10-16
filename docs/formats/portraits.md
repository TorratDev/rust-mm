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

A missing emotion has both pointers 0. The decoded image is 40x40 pixels, 4bpp, as 5x5 tiles in row-major order. Color 0 is drawn (portraits fill their frame).

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
- Which text control code switches portraits ([`text.md`](text.md)): not identified.
