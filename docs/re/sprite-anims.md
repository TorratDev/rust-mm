# Sprite animation groups: selectors, runtime writes and visual review

**Date:** 2026-10-18
**Status:** blue-verified for the selectors and the setter; the pose names are hypotheses from move names; nothing checked on screen for groups 8–16
**Subsystem:** creature sprites (`monster.sbin` `axNNN`, issue [#133](https://github.com/TorratDev/rust-mm/issues/133))

Blue copy `rescue-arm9-a` for the selectors, `rescue-arm9-b` and the py-desmume harness for the runtime facts, copy C for the visual review. All addresses are ARM9 or overlay 1 RAM, as marked. Spec: [`formats/sprites.md`](../formats/sprites.md). Symbols: [`symbols.md`](../symbols.md).

Tags: **[blue-verified]** read in Blue's code, or confirmed by the runtime harness; **[hypothesis]** a reading not yet checked; **[unverified]** not checked.

## Selectors for groups 8–16

Three ways a group is chosen (blue-verified):

1. **Move table.** `GetMoveAnimId` (`02023E9C`) returns the default byte at `+0x10` of the move entry, unless a species override record matches. Tallied over all 413 move entries: the defaults are 1, 2, 3, 7, 8, 9, 10, 11, 12, 98 and 99. Groups 13 to 16 never appear as a default.
2. **Override list.** 12-byte records at `020A0868` (`{i16 species, i16 anim, i32 sfx}`). Groups 8, 10, 11, 12 and 99 appear; 13 to 16 do not.
3. **Event scripts.** Groups 13 to 16 are set only by cutscene code in overlay 1, on scripted actors.

Literal sets: group 8 is set by a literal call in `0213EE4C`, which waits for the animation to end. Group 10 has a literal call in `02160A7C`: a dash-and-return lunge. The overlay 1 move constant at `02160A7C` is not resolved.

| Group | Defaults and overrides | Event scripts | Tag |
|-------|------------------------|---------------|-----|
| 8 | 14 default moves (Iron Tail, Poison Tail, Rolling Kick, Rapid Spin, Wide Slash, Aerial Ace, Flail, Seismic Toss, Hurl, Thief, Whirlwind, Submission, Faint Attack, Smellingsalt), 33 override records | none | blue-verified (selector); pose from move names |
| 9 | 3 default moves (Double Team, Agility, Cosmic Power) | none | hypothesis, weak: no literal set found |
| 10 | 10 default moves (Dig, Dive, Bounce, Splash, Earthquake, Fissure, Magnitude, Trap Buster, One-Room, Trapper), 6 overrides | none | blue-verified (selector); pose medium-low |
| 11 | bide and charge-up moves (67) | none | blue-verified |
| 12 | 123 default moves: the generic "use move" pose, the most common | none | blue-verified (selector) |
| 13 | none | `021A07C0`, actor slot 3, with dialogue waits | blue-verified (selector); cutscene-only |
| 14 | none | `021A415C`, actors 0xB and 0xC | blue-verified (selector); cutscene-only |
| 15 | none | `021A0734`, `021A273C`, `021A298C`, `021A2E08` | blue-verified (selector); cutscene-only |
| 16 | none | `021A273C`, `021A28A8`, after 15, with dialogue | blue-verified (selector); cutscene-only |

Group 13 and 14 are also one-shot animations for a few species (see [`formats/sprites.md`](../formats/sprites.md)). That is a property of the species, not a selector, so it does not contradict the table above.

Pose names: the table gives no pose name for 8 to 16 from the code. The move names are the only evidence, and the on-screen pose was not checked. That check is split to [#140](https://github.com/TorratDev/rust-mm/issues/140).

## Runtime facts

- `ov_0001::0217AED0` writes an entity's group to `+0x6A` and its direction to `+0x6C`, for type-1 entities only (blue-verified, `rescue-arm9-b`).
- [unverified] Emulator observation: in the py-desmume harness, team slot 1 is the left creature of the pair. Entity x is 19 for slot 1 and 20 for slot 0. Writing group 6 to each slot appeared to move the matching creature (frame 14). No harness command or frame hash is recorded here, and the capture is not in the repository; the evidence moves to [#140](https://github.com/TorratDev/rust-mm/issues/140).
- [unverified] Reported observation: the on-screen palette of team slot 1 is blue, with a blue flame tail. The decoded `ax004` with its `monspara` palette byte is orange. This is not explained. Possible causes, not yet tested: another palette byte for this entity, a shiny or alternate path, the `unk6_1` palette-keep bit, or a misread palette field. The evidence moves to [#140](https://github.com/TorratDev/rust-mm/issues/140). The palette rule in `formats/sprites.md` is unchanged.
- [unverified] Reported readout: the team table's `info+2` reads slot 0 as 158 and slot 1 as 4, and the on-screen colours disagree with that. Whether `info+2` is the species ID is open, so the sprites are not labelled by slot yet. The evidence moves to [#140](https://github.com/TorratDev/rust-mm/issues/140).
- [unverified] Captures of groups 0, 5, 6 and 7 in all eight directions for team slot 1 exist only in a scratch folder, not in the repository. Group 0 was compared by eye and the silhouettes look like the decoded species 4 and 158 frames, but the palette differs, so it is not a match. Groups 5 to 7 were captured and not reviewed. The evidence moves to [#140](https://github.com/TorratDev/rust-mm/issues/140).
- The second starter is chosen by the personality-quiz answers. Not attempted.

## Visual review (copy C)

Scope: 30 unique sprites, not 31. The list is ax001 to ax016 (16), plus the other 28-group sprites 025, 052, 054, 066, 104, 133, 152, 155, 158, 277, 280, 283, 325 (13, since 001, 004 and 007 are already in the first 16), plus ax307 (1). Group and frame sheets were read for ax001 to ax016 and ax307; group sheets only for the other 13.

Automated checks on all 30 (earlier run): no bad image references, no empty frames, no colour index above 15, piece sizes match the image sizes.

Visual read at 3x scale, with a scratch Python port of the decoder. The port was checked only against ax307's blank frames 216 and 225. Nothing was compared on screen, and one-pixel or palette-only defects could be missed.

No defects found. Noted, not defects:

- ax007 and ax009, group 4: shell-only poses in all directions. This looks like a withdraw pose; not checked against the game.
- ax307 (Gardevoir), groups 14 and 15: pale tiles. The existing doc calls this a fade, intentional.
- ax307, frames 216 and 225: blank, as documented.
- ax005 (Charmander): a white crescent in several frames. Probably a slash or claw effect; not confirmed.

## Open

- The runtime pose check for groups 8 to 16, the palette of team slot 1, the `info+2` slot-species check, and the second starter: [#140](https://github.com/TorratDev/rust-mm/issues/140).
- Group 9 meaning (three moves, no literal set).
- The overlay 1 move constant at `02160A7C`.
- The move entry layout after `+0x08`.

## Links
- Spec: [formats/sprites.md](../formats/sprites.md)
- Symbols: [symbols.md](../symbols.md)
- Effects: [formats/effects.md](../formats/effects.md)
