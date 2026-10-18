# Sprite animation groups: selectors, runtime writes and visual review

**Date:** 2026-10-18 (updated for #140)
**Status:** blue-verified for the selectors, the setter, the palette rule and the cutscene call sites; the on-screen checks below are visual (scratch captures, not in the repository); pose names for groups 8 to 16 are hypotheses from move names
**Subsystem:** creature sprites (`monster.sbin` `axNNN`, issues [#133](https://github.com/TorratDev/rust-mm/issues/133) and [#140](https://github.com/TorratDev/rust-mm/issues/140))

Blue copy `rescue-arm9-a` for the selectors, `rescue-arm9-b` and the py-desmume harness for the runtime facts, copy C for the visual review. All addresses are ARM9 or overlay 1 RAM, as marked. Spec: [`formats/sprites.md`](../formats/sprites.md). Symbols: [`symbols.md`](../symbols.md).

Tags: **[blue-verified]** read in Blue's code, or reproduced by a repeatable run with a matching hash; **[visual]** read by eye from captures; **[medium]** a visual reading that agrees with the other evidence but has not been checked by a second method; **[unverified]** not checked, or withdrawn if so marked.

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
| 13 | none | `021A07C0` on actor slot 3 (`0219F534(3)`), with dialogue waits | blue-verified (decompile); cutscene-only |
| 14 | none | `021A415C` on actors 0xB and 0xC (`0219F534(0xB)`, `0219F534(0xC)`) | blue-verified (decompile); cutscene-only |
| 15 | none | `021A0734`, `021A273C`, `021A28A8`, `021A2E08`, `021A298C` | blue-verified (decompile); cutscene-only |
| 16 | none | `021A273C`, `021A28A8`, `021A298C`, after 15, with dialogue | blue-verified (decompile); cutscene-only |

Correction to #133: `021A298C` sets groups 15 and 16, not 13, and `021A28A8` also sets 15. Group 13 is set by `021A07C0` only.

Group 13 and 14 are also one-shot animations for a few species (see [`formats/sprites.md`](../formats/sprites.md)). That is a property of the species, not a selector, so it does not contradict the table above.

Pose names: the on-screen pose check (#140) covered groups 0, 5, 6, 7, and 13 to 16 (forced, visual only). Groups 8 to 10 and 12 were not checked on screen. The names in the table come from move names and the decompile, not from the game.

## Runtime facts

- `ov_0001::0217AED0` writes an entity's group to `+0x6A` and its direction to `+0x6C`, for type-1 entities only (blue-verified, from the decompile).
- **Team layout** [visual, medium]: team slot 1 is the left creature of the pair, slot 0 the right one. Entity x is 19 for slot 1 and 20 for slot 0. Writing group 6 to each slot moves the matching creature. The screen hashes were identical across two runs of the scratch capture script, which is not in the repository.
- **Species of the slots** [visual, medium]: slot 1 is species 4 (Charmander, orange on screen), slot 0 is species 158 (Totodile, blue). The `info+2` readout (158 for slot 0, 4 for slot 1) is consistent with the screen for both slots. The writer of `info+2` is not traced, so the readout is not confirmed as the species ID.
- **Palette** [withdrawn]: the earlier report of a blue on-screen colour for slot 1, against decoded orange, was a capture-script channel swap. py-desmume's framebuffer is BGRX. With the channels corrected, Charmander is orange on screen, matching `ax004`. The palette rule itself is blue-verified, with a repeatable OAM check in [`formats/sprites.md`](../formats/sprites.md#palette).
- **Pose check, groups 0, 6, 7** [visual, medium]: with the palette ignored, at 4x crops, the screen silhouettes match the decoded `ax158` (slot 0) and `ax004` (slot 1) in all eight directions. Two runs gave 128 identical screen hashes. The capture script is scratch, not in the repository.
- **Group 5** [visual]: matches for slot 0. For slot 1 the capture shows lying while the decoded frame at that step is standing. Open: the frame index was not checked.
- **Cutscene groups 13 to 16** [visual, unverified poses]: forced captures (group and direction written to a type-1 entity, with the setter's type check skipped) read back correctly in 32 of 32 cases, and two runs gave the same log hash. The poses look the same in all eight directions, which matches the decoded frames for those groups. The frame indices are not matched, and no real cutscene was reached, so the timing is not confirmed. The colour remarks from that capture path are not recorded, since the path may have had the same BGRX bug.
- The second starter is chosen by the personality-quiz answers. Not attempted.

## Visual review (copy C, #133)

Scope: 30 unique sprites, not 31. The list is ax001 to ax016 (16), plus the other 28-group sprites 025, 052, 054, 066, 104, 133, 152, 155, 158, 277, 280, 283, 325 (13, since 001, 004 and 007 are already in the first 16), plus ax307 (1). Group and frame sheets were read for ax001 to ax016 and ax307; group sheets only for the other 13.

Automated checks on all 30 (earlier run): no bad image references, no empty frames, no colour index above 15, piece sizes match the image sizes.

Visual read at 3x scale, with a scratch Python port of the decoder. The port was checked only against ax307's blank frames 216 and 225. Nothing was compared on screen, and one-pixel or palette-only defects could be missed.

No defects found. Noted, not defects:

- ax007 and ax009, group 4: shell-only poses in all directions. This looks like a withdraw pose; not checked against the game.
- ax307 (Gardevoir), groups 14 and 15: pale tiles. The existing doc calls this a fade, intentional.
- ax307, frames 216 and 225: blank, as documented.
- ax005 (Charmander): a white crescent in several frames. Probably a slash or claw effect; not confirmed.

## Open

- The second starter (the partner is set by the personality-quiz answers).
- Real cutscene timing for groups 13 to 16, and frame-index matching of their on-screen poses.
- The writer of `info+2`.
- The per-sprite palette override branch of `FUN_0201997C` (static only).
- Group 5 in slot 1: lying on screen while the decoded frame at that step is standing.
- On-screen poses of groups 8 to 10 and 12.
- Group 9 meaning (three moves, no literal set found) and the overlay 1 move constant at `02160A7C`.
- The move entry layout after `+0x08`.

## Links
- Spec: [formats/sprites.md](../formats/sprites.md)
- Symbols: [symbols.md](../symbols.md)
- Effects: [formats/effects.md](../formats/effects.md)
