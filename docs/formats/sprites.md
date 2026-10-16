# Creature sprites (`monster.sbin` `axNNN`)

Issue: [#33](https://github.com/TorratDev/rust-mm/issues/33). Code: `crates/formats/src/sprite.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`sprites_decode`, mutation test), `tests/tests/viewer_rom.rs`. Viewer: `monster.sbin` → `axNNN` (animation, direction, play).

**Verification status:** verified against Blue (2026-10-16). All 423 files parse; every frame of every sprite renders; Bulbasaur's 71 images checked by eye. Layout invariants (tables back to back, image sizes equal piece sizes, running VRAM tile numbers) hold in all 423 files.

## Resource IDs

`NNN` is the species index ([`data.md`](data.md)). The palettes are not in the file: piece's name one of the 14 palettes in `monster.sbin` `palet` (14 x 16 RGBX).

## Layout

SIR0. Content header, five `u32`:

| Offset | Field |
|--------|-------|
| `+0x00` | frame table pointer |
| `+0x04` | animation group table pointer |
| `+0x08` | group count (13 to 28) |
| `+0x0C` | image table pointer |
| `+0x10` | per-frame offsets pointer |

The frame table runs up to the offsets table (frame count = length / 4); the image table runs up to the content header.

**Frame** (pointer from the frame table): piece's of 10 bytes until one has attr1 bit 11 set.

| Offset | Size | Field |
|--------|------|-------|
| `+0` | 2 | image index; `-1` = reuse tiles already loaded at this piece's tile number |
| `+2` | 2 | unknown (0 in all but 5 of 122,469 piece's) |
| `+4` | 2 | OAM attr0: y in bits 0–9 (relative to 512), shape in bits 14–15 |
| `+6` | 2 | OAM attr1: x in bits 0–8 (relative to 256), bit 10 unknown, bit 11 last piece, bit 12 hflip, bit 13 vflip, size in bits 14–15 |
| `+8` | 2 | OAM attr2: tile number bits 0–9 (32-byte units in the frame's VRAM), priority bits 10–11, palette bits 12–15 |

Rendering: for each piece with an image, copy the image into the frame's VRAM at `tile * 32`; then draw the piece's last to first (earlier OAM entries are on top), each as `w/8 x h/8` 4bpp tiles in 1D order from its tile number. Color 0 is transparent.

**Image** (pointer from the image table): strips of `{u32 pointer, u32 length}` until a length of 0; pointer 0 means `length` zero bytes. The concatenation is the piece's 4bpp tiles.

**Per-frame offsets:** 16 bytes per frame, four `(i16 x, i16 y)` points. Explorers calls them head, left hand, right hand, centre `[hypothesis]`.

**Animation group** (pointer from the group table): 8 pointers, one per direction, each to a sequence of 12-byte steps ended by a zero duration:

| Offset | Size | Field |
|--------|------|-------|
| `+0` | 1 | duration in game frames |
| `+1` | 1 | flags (unknown) |
| `+2` | 2 | frame index |
| `+4` | 4 | sprite offset `(i16, i16)` |
| `+8` | 4 | second offset, probably the shadow `[hypothesis]` |

## Unknown fields

- Palette: the game draws a monster's piece's in the species' `overworldPalette` (the palette byte of its `monspara` entry), not in each piece's palette bits. Red does this in `sprite.c` (`axData->paletteNum`). 403 of 423 sprites already agree with their species. Statue (`ax422`) names palette 13 in its piece's but palette 5 in its species entry, and palette 13 is a placeholder (Red's `monster_sbin_palet.c`). The viewer uses the species palette. In Blue's ARM9 code, the species byte is read by `GetPokemonOverworldPalette` (`020020A0`, see [`symbols.md`](../symbols.md)), and the sprite loader `FUN_02028628` stores it as the monster sprite's palette. Traced further: the in-dungeon party HUD (`ov_0001::02155760`) passes the value to `FUN_020198ac`, which stores it in the object at `+0x13`. The OAM writer `FUN_02019CCC` copies each piece's own `attr2` (palette bits included). The consumer is `FUN_02019CCC`: unless the OAM entry's `unk6_1` bit is set, it replaces the piece palette bits with the object's palette byte (`+0x0F` of the object). Red sets `unk6_1` to 0 at its only use and never sets it, so the override applies to these sprites. The viewer rule (every piece uses the species palette) therefore matches the game. Open: the five piece's with a non-zero `unk2` field. In Red, `sUnknown_2025682` (the palette they would take) is never written, so it stays 0; in Blue this path is traced and the premise is different: see "Piece field +2" below. Diglett's tiny frames (group index 13, pose 277) match Red's dig-in animation (a single small piece, followed by poses 278 and up), so they are intended, not missing pixels. 19 sprites mix palettes inside one sprite, so those exceptions are open.
- Direction order is verified: group direction `0` faces the viewer (south), then clockwise: `1` south-east, `2` east, `3` north-east, `4` north (back view), `5` north-west, `6` west, `7` south-west. Checked by eye on `ax025` in the viewer. It matches Red's `DIRECTION_*` order (`include/constants/direction.h`).
- Verified in Blue (`ov_0001::0217AD64`): group 5 is sleep, group 7 is the default idle pose, group 11 is bide. The other IDs are not confirmed.
- Animation IDs set by literal calls to `SetMonsterAnimAndDir` (`ov_0001::0217AED0`), traced in Blue on 2026-10-16. The older idea that a per-slot value at dungeon `+0x158DC` supplies walk and hit IDs is wrong: that table holds sprite handles (`GetMonsterSpriteHandle`, `ov_0001::02178C04`).
  - **0 walk** `[blue-verified]`: the movement code `ov_0001::0213DD38` calls it with the facing direction when a monster moves tile to tile; the cutscene step helpers `0219E438`/`0219E480`/`0219E518` also use it to walk an actor.
  - **5 asleep, 7 idle, 11 bide** `[blue-verified]`: `GetDefaultMonsterAnimId`.
  - **6 hit reaction** `[blue-verified as the damage/knock-back pose]`: set with the direction by `ov_0001::02194368`, `021967E0` and `02196F84` (the last also plays sound `0x55` and calls `SetDefaultMonsterAnim`). Also used by cutscene code (`021A0DE0`, `021A1538`).
  - **8** `[hypothesis]`: `ov_0001::0213EE4C` sets it (keeping the direction) and waits up to 99 frames until the monster leaves it; its only caller is `HandleDealingDamage_Async`, when the target`s status byte at `+0xB1` is 13. It is a ready/counter pose; the exact state is not named.
  - **10** `[hypothesis]`: `ov_0001::02160A7C` sets it before moving the monster to a random adjacent tile in 12 steps, and `0218E19C` sets it too; it reads as a hop or warp pose.
  - **4, 13, 14, 15, 16** `[hypothesis]`: set only by story and cutscene scripts (`021A0734` 15, `021A07C0` 13, `021A415C` 14, `021A273C`/`021A298C` 15 and 16); the numbers are real, the poses are not seen. IDs 1, 2, 3, 9 and 12 are never a literal argument: any further source is computed (the cutscene wrappers `0219E694`/`0219E6AC` pass the ID through from their callers) and is not traced.
- Piece field `+2`, `[blue-verified]` (`FUN_02019CCC`, 2026-10-16): it is not a palette. Its high byte (`+3`) is added to the object`s depth key (`obj[0x16] + (field >> 8)`, clamped to 0–255) that picks the slot the OAM entry is queued in, so it reorders piece's front to back. The viewer can ignore it, which only affects overlap order of those five piece's. attr1 bit 10 is the `unk6_1` bit: when set, the writer keeps the piece`s own palette bits instead of the species palette (not measured how many piece's set it). Still unknown: step flags, the four per-frame points and the second step offset.
- Effect object's in `effect.sbin` use the same piece's and steps, without images or per-frame points: see [`effects.md`](effects.md).
