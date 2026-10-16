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
- Animation IDs, traced in Blue on 2026-10-16. An animation group number is `entity+0x6A` (`SetMonsterAnimAndDir`, `ov_0001::0217AED0`). Two sources set it. The older idea that a per-slot value at dungeon `+0x158DC` supplies walk and hit IDs is wrong: that table holds sprite handles (`GetMonsterSpriteHandle`, `ov_0001::02178C04`).
  - **Moves:** `PlayMoveAttackerAnim` (`ov_0001::0213DD38`) asks `GetMoveAnimId` (`02023E9C`) for `(species, move)`. The ARM9 table at `02098CF0` (36 bytes per move, anim byte at `+0x10`) gives the default group. A record list at `020A0868` (12 bytes: `i16 species, i16 anim, i32 sfx`, from index `+0x20`, count `+0x1C`) overrides it for listed species. Anim 98 spins 9 steps clockwise and 99 spins 8 steps counter-clockwise (in place, using group 0); every other value is set and then waited on until the anim finishes (`entity+0x40` bit 0) or 120 frames pass. Bit 1 fires the move's effect (the hit frame); anim 9 fires at once.
  - **State and script code:** literal calls (below), plus the cutscene forced-anim byte `monster+0xDB` (`SetAnimForced` `0219E6AC`).
  - One-shot groups (`IsOneShotAnim`, `02002438`, table `0208BEAC`): 1–4. 13 and 14 are one-shots only for a few species (13: 413, 268, 51; 14: 65, 51). After a one-shot ends, `UpdateMonsterSprite` (`ov_0001::0217B120`) picks the follow-up from a 3x13 table at `021AE1D0`: 0→7 (idle), 1→7, 5→5, 6→6, others →7, and 11→11 while the status byte is 1. Group 6 returns to the default after 16 steps. `FUN_02012848` does no bounds check on the group, so every sprite must supply every group it can be asked for.
  - Meaning by group. Each row says what selects it and which named moves use it as their default (the move names are the evidence for the pose, not a pose name from the game):

    | Group | Meaning | Tag |
    |-------|---------|-----|
    | 0 | walk and turn (`0213DD38` turn loops, `021732C4` item use, cutscene steps) | blue-verified |
    | 1 | physical lunge, one-shot: Tackle, Quick Attack, Headbutt, Take Down, Horn Attack, Peck, Double-Edge (66 moves) | blue-verified (mechanism), pose from move names |
    | 2 | close-range strike, one-shot: Scratch, Pound, Bite, Vine Whip, the Punches, Mega Kick, Slash (35 moves) | blue-verified (mechanism), pose from move names |
    | 3 | ranged or special attack, one-shot: Flamethrower, Ember, Psybeam, Hydro Pump, Hyper Beam, Bubble, Sing (87 moves) | blue-verified (mechanism), pose from move names |
    | 4 | most common species override (1,928 records), one-shot; also cutscenes | blue-verified (mechanism), pose unknown |
    | 5 | asleep (`GetDefaultMonsterAnimId`); also Snore and Sleep Talk | blue-verified |
    | 6 | hit reaction (`02194368`, `021967E0`, `02196F84`); never a move default | blue-verified |
    | 7 | idle (`GetDefaultMonsterAnimId`); move default for Yawn, Charm, Encore, Slack Off | blue-verified |
    | 8 | tail or kick swing: Iron Tail, Rolling Kick, Thief, Faint Attack, Seismic Toss, Poison Tail (14 moves); also waited on in `0213EE4C` when the target's status byte `+0xB1` is 13 | blue-verified (mechanism), pose from move names |
    | 9 | quick sidestep: Double Team, Agility, Cosmic Power (3 moves); effect fires with no hit-frame wait | blue-verified (mechanism), pose from move names |
    | 10 | hop or dive: Dig, Earthquake, Bounce, Dive, Splash, Magnitude, Fissure (10 moves); also `02160A7C` (random adjacent tile, 12 steps) and `0218E19C` | blue-verified (mechanism), pose from move names |
    | 11 | bide and charge-up: Bide (`GetDefaultMonsterAnimId`), plus Focus Energy, Harden, Thunderbolt, Solarbeam, Fly, Explosion and others (67 moves) | blue-verified |
    | 12 | status and wave cast: Tail Whip, Sand-Attack, Surf, Rock Throw, Haze, Teleport and others (123 moves, the most common) | blue-verified (mechanism), pose from move names |
    | 13, 14 | species-special one-shots (see above); also cutscenes | blue-verified (mechanism), pose unknown |
    | 15, 16 | cutscene scripts only (`021A0734`, `021A273C`, `021A298C`) | blue-verified call sites, pose unknown |
    | 98, 99 | not groups: spin sentinels for Cut/Spin Slash (98) and Weather Ball, Bone Rush, Mud-Slap, Leech Seed and others (99) | blue-verified |
  - Not traced: the other fields of the move table (`+0` to `+0x18`), and the fallback when a species lacks a requested group.
- Visual review, 2026-10-16 (#133): every sprite's group sheet (first frame of each group and direction, all 423 sprites) was read at full size by two Sonnet reviewers; no defect found. Frame sheets (every frame) were seen for about 150 of the 13-group sprites in ax001–211 and the 16 non-13-group ones in that range except ax001, 004, 006, 007, 009, 025, 038, 050, 051; in ax212–422 all non-13-group sprites and part of the 13-group ones. Not seen: those frame sheets, and nothing was compared with the game on screen. Gardevoir (`ax307`) frames 216 and 225 are blank on purpose: one piece on an all-transparent image, alternated with visible frames at growing durations in groups 14 and 15 (a flicker-out, 68 step references). Statue (`ax422`) uses the same 13 frames in every group and direction.
- Piece field `+2`, `[blue-verified]` (`FUN_02019CCC`, 2026-10-16): it is not a palette. Its high byte (`+3`) is added to the object`s depth key (`obj[0x16] + (field >> 8)`, clamped to 0–255) that picks the slot the OAM entry is queued in, so it reorders piece's front to back. The viewer can ignore it, which only affects overlap order of those five piece's. attr1 bit 10 is the `unk6_1` bit: when set, the writer keeps the piece`s own palette bits instead of the species palette (not measured how many piece's set it). Still unknown: step flags, the four per-frame points and the second step offset.
- Effect object's in `effect.sbin` use the same piece's and steps, without images or per-frame points: see [`effects.md`](effects.md).
