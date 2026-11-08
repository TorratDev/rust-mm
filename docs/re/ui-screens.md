# UI screens: engine A layers, frame tiles, cursor, log box and assets

**Date:** 2026-11-08
**Dump:** BlueRescueTeamEur (EUR), the retail dump the #62 investigators used
**Issue:** [#62](https://github.com/TorratDev/rust-mm/issues/62) (text rendering is in [text-engine.md](text-engine.md))
**Workflow:** [workflow.md](workflow.md)

Sources: the #62 findings B (window frame, cursor, dialogue box, portrait) and C (lower-screen dungeon layout, BG3 scroll shadow, WIN0 and alpha log box, assets). Captures are #61 frames: `post_accept` (town dialogue with a portrait), `afterq` (quicksave resume windows with a cursor), `gu` (dungeon Quicksave/Give Up menu) and `dng1`/`b2` (dungeon). Tags follow the findings: **[blue-verified]** (read in Blue's ARM9 or overlays, or matched to a runtime frame) and **[hypothesis]** (inferred). The findings' `[hypothesis]` tag is not the same as the `red-hypothesis` status of [workflow.md](workflow.md#status-tags), which is for `pmd-red` facts; this note has none.

## Claims

| Claim | Evidence | Status |
|-------|----------|--------|
| All UI is drawn on engine A (bottom screen): DISPCNT `0x08817F10`, BG0 to BG3 and OBJ on, WIN0 and WIN1 on | `io.bin` regs in three captures (#62 B 1), section 1 | [blue-verified], high |
| BG0 holds the frame, the text canvas, the portrait (town frame) and the dungeon HUD; BG1 is the window fill; BG3 is the dungeon floor | `io.bin`, map dumps, `comp.py` (10 of 49152 px differ on `dng1`, 83 on `b2`) (#62 C 1) | [blue-verified], high |
| The frame tiles are `system.sbin` `font` (136 tiles), loaded by `FUN_0201aff0` at BG tile `0x278` | VRAM equal to the file in 135 of 136 tiles (post_accept) and byte-match in the dungeon | [blue-verified], high |
| BG palette 15 (both engines) is `fontpal` palette 0; index `0xF0` | 16 of 16 colours match post_accept VRAM (#62 B 3) | [blue-verified], high. The selector `FUN_02015fd8` was not checked: [hypothesis] for palette 1 vs 0 |
| The cursor and wait marker are OBJ sprites from `fontsp` at OBJ tile `1008 + i`, palette from `fontsppa` (OBJ palette 15) | OAM entries and VRAM tiles match 16 of 16 (#62 B 4) | [blue-verified] for source and slot, high; the roles of tiles 5 to 15 are low |
| The dialogue box and the dungeon message log use the same frame tiles (corner `0x297`, side `0x2DA`, fill `0x2DB`) in the same cells | tile map of both frames (#62 B 2, C 3) | [blue-verified], high |
| The portrait is 25 consecutive BG0 tiles (`0xB8` to `0xD0`) at cols 24 to 28, rows 10 to 14, palette bank 14, with its own 7x7 frame | VRAM and map of post_accept (#62 B 6) | [blue-verified], high for position and palette |
| The portrait is shown horizontally mirrored from the decoded slot 0 of `kao004` | 1588 of 1600 pixels equal after a pixel hflip; one sample | [blue-verified] for the difference, medium for the rule (one sample; see [portraits.md](../formats/portraits.md)) |
| The dungeon floor scroll is the shadow array at `0x020F38D0` (engine B `0x020F38C0`), copied to the BG scroll registers by `FUN_02019268` | decompile of `FUN_02019268`; shadow bytes in `dng1` and `b2` (#62 C 2) | [blue-verified], high |
| The translucent log box is BG1 through WIN0, rectangle x 19 to 236, y 139 to 172, alpha 8/16 over BG2, BG3 and OBJ | `FUN_0201966c` decompile, HBlank DMA table at `0x020F2460`, blend arithmetic (#62 C 4) | [blue-verified], high (the box was read once, see Not verified) |
| The status-digit tiles (`0x2CC` to `0x2D7`) and the minimap drawing rule | not located | not found (#62 C 6) |

## Addresses

| Address | Name | Source |
|---------|------|--------|
| `0x08817F10` | DISPCNT, engine A (BG0 to BG3, OBJ, WIN0 and WIN1 on, screen base +0x10000) | #62 B 1, C 1 |
| `0x06000000` | BG0 and BG1 char base | #62 B 1 |
| `0x06008000` | BG2 and BG3 char base (C reads it as `0x8000` relative to VRAM) | #62 B 1, C 1 |
| `0x06010000`, `0x06010800`, `0x06011000`, `0x06012000` | screen bases: BG0, BG1, BG2, BG3 | #62 B 1, C 1 |
| `0x06004F00` / `0x06204F00` | font tiles loaded to BG tile `0x278` (engine A / B) | #62 B 2 |
| `0x06407E00` / `0x06607E00` | `fontsp` loaded to OBJ tile `1008` (engine A / B) | #62 B 4 |
| `0x06006000` | `panelnn` loaded to BG tile `0x300` | #62 B 5, C 5 |
| `0x020F38D0` | BG scroll shadow, engine A (8 signed 16-bit words: BG0 x, y through BG3 x, y) | #62 C 2 |
| `0x020F38C0` | BG scroll shadow, engine B | #62 C 2 |
| `0x020F2138` | pointer; the DMA source is its target + 4 | #62 C 4 |
| `0x020F2460` | HBlank DMA table (WIN0H per scanline; `0x13ED` on rows 139 to 172, `0xFFFF` elsewhere) | #62 C 4 |
| `0x04000040` | WIN0H (`0x04000040`) | #62 C 4 |
| `0x02096FC8` | ARM9 string `panelnn` (sheet name) | #62 B 5 |
| `0x0208FA28` | ARM9 format string `kao%03d` | #62 B 6 |

## Routines

| Function | Address | What it does (paraphrase) |
|----------|---------|---------------------------|
| `FUN_0201aff0` | ARM9, UI graphics init | loads `font` to BG tile `0x278`, `fontsp` to OBJ tile `1008`, `fontsppa` to OBJ palette 15, `panelnn` to BG tile `0x300` (#62 B 2, 4, 5; C 5) |
| `FUN_0201ae94` | ARM9 | copies `fontpal` to RAM `0x021019D4` and writes 16 entries from palette index `0xF0` (#62 B 3) |
| `FUN_02015fd8` | ARM9 | returns the palette selector (0 or 1); meaning not checked (#62 B 3) |
| `FUN_02019268` | ARM9, frame action 17 (`FlushDisplayState`) | copies the BG scroll shadows to the BG scroll registers, sets the layer enables and BG priorities (#62 C 2; [io.md](io.md#per-frame-flush)) |
| `FUN_0201966c` | ARM9 (WIN0 setup) | sets DMA 0 from `*(0x020F2138) + 4` into WIN0H, and WIN0V to `0x00C0` (#62 C 4) |
| `FUN_02001ff8` | ARM9 | opens a `monster.sbin` entry by name `kao%03d` (#62 B 6) |
| `FUN_0202ae50` | ARM9 | the consumer of the decoded portrait (#62 B 6) |

## Evidence

- **Runtime:** three captures (`post_accept`, `afterq`, `gu`) and `dng1` / `b2` (#61 captures). Frames were compared against VRAM, OAM, palette and `io.bin` dumps. The compositor used for `dng1` and `b2` is `comp.py` (#62 C), a scratch script, not in the repo. Its output is not recorded here.
- **Static:** decompiles of `FUN_0201aff0`, `FUN_0201ae94`, `FUN_02019268` and `FUN_0201966c` (Ghidra copies `rescue-arm9-b` and `rescue-arm9-c`). No command or output hash is recorded in the repo because the scripts are scratch files.
- **Byte matches:** `system.sbin` files `font`, `fontpal`, `fontsp`, `fontsppa` and `panelnn` against VRAM; `dungeon.sbin` `etcfont` against OBJ VRAM.

## 1. Engine A layers

**[blue-verified]**, high for the layout (#62 B 1, C 1). DISPCNT `0x08817F10`: BG0 to BG3 and OBJ on, WIN0 and WIN1 on, char base 0, screen base +0x10000, all BGs 4bpp.

| Layer | BGCNT | Priority | Size | Screen base | Char base | Content |
|-------|-------|----------|------|-------------|-----------|---------|
| BG0 | `0x0000` | 0 | 256x256 | `0x06010000` | `0x06000000` | frame, text canvas, portrait (town frame), dungeon HUD (status line, minimap canvas, message-log frame and text, five touch buttons) |
| BG1 | `0x0101` | 1 | 256x256 | `0x06010800` | `0x06000000` | window fill: tile `0x27A` (blank) by default; tile `0x2DB` (blue) in cols 2 to 29, rows 17 to 21. Shown only through WIN0 (dungeon) |
| BG2 | `0x420A` | 2 | 512x256 | `0x06011000` | `0x06008000` | scene. Empty in the dungeon capture (map all 0) |
| BG3 | `0x440B` | 3 | 512x256 | `0x06012000` | `0x06008000` | dungeon floor, scrolled by the camera (see [BG3 floor scroll](#7-bg3-floor-scroll)) |

- OBJ: 1D mapping, 32-byte boundary. All dungeon sprites at priority 3; the minimap player dot at priority 0 (#62 C 1).
- Order at equal priority: OBJ over BG, lower BG number over higher (#62 C 1).
- Backdrop colour = palette entry 0.
- **Engine B** (top) has the same layout with its own copy of the font tiles (#62 B 1), but its UI was not covered (party menu is open, #62 C 5).
- The findings disagree on what BG2 and BG3 hold: B reads them as scene layers from three frames; C finds BG2 empty and BG3 as the floor in the dungeon. Both are consistent with the capture they cover.

## 2. BG0 layout (tile map)

**[blue-verified]**, high for the tile positions (#62 C 3; B 2 for the frame). Tiles are map entries `& 0x3FF`, with the palette in bits 12 to 15. The font tiles are BG tile `0x278 + i` (font index `i`).

| Element | Cells | Tiles | Palette | Source |
|---------|-------|-------|---------|--------|
| Status line (`B1F Lv8 HP34/34`, HP bar) | row 0, cols 0 to 28 (x 0 to 231, y 0 to 7) | col 0 = `0x279`; glyphs `0x279` to `0x2BE` | 15 | font tiles (match). Digits `0x2CC` to `0x2D7` are drawn at run time; source not located |
| Minimap canvas | cols 1 to 28, rows 1 to 16 (x 8 to 231, y 8 to 135) | `0x0B8` to `0x277`, consecutive, row-major | 14 and 15 | run-time bitmap into a tile canvas, not file data. Pattern file candidate: `dungeon.sbin` `zmappat` [hypothesis] |
| Message-log frame and dialogue box frame | cols 2 to 29, rows 17 to 21 (x 16 to 240, y 136 to 176) | corner `0x297` (rows 17, 21), sides `0x2DA` (rows 18 to 20) | 15 | `font` tiles 31 and 98 |
| Text canvas | cols 3 to 28, rows 17 to 21 (26 x 5 tiles, x 24 to 231, y 136 to 176) | `0x002` to `0x083`, consecutive | 15 | text written by the text engine ([text-engine.md](text-engine.md)); two lines visible at about y 143 and 153 |
| Touch buttons: Throw, Moves, Items, Team, Menu | rows 22 and 23; cols 1, 7, 13, 19, 25, each 5 tiles wide (x 8 to 47, 56 to 95, 104 to 143, 152 to 191, 200 to 239; y 176 to 191) | `0x300 + 10k + dx` (row 22), `+5` (row 23); 50 tiles `0x300` to `0x331` | 15 | `panelnn` (SIR0, 1648 bytes: 20-byte header, then 50 tiles). The `n` variant is the one in this English capture; the mapping of `f`, `g`, `i`, `s` is not confirmed (only `n`) |

- Col 0 and the gaps (cols 6, 12, 18, 24, 30 and beyond) are empty in rows 22 and 23 (#62 C 3).
- **Town frame (post_accept):** the dialogue box is cols 2 to 29, rows 17 to 21. The top edge (3 pixel rows, f, e, d) is baked into the canvas tiles from `0x002` (pixel rows 0 to 2 of each tile), and the bottom edge (d, e, f) into the tiles from `0x06A` (pixel rows 5 to 7) (#62 B 2). Fill = BG1 `0x2DB` over the same cells.

## 3. Frame tile roles

**[blue-verified]** for the indices and the placement, high (#62 B 2). Font index `i` = BG tile `i + 0x278`, using BG palette 15.

| Index `i` | BG tile | Role |
|-----------|---------|------|
| 1 | `0x279` | transparent; BG0 col 0 |
| 2 | `0x27A` | blank; BG1 default |
| 31 | `0x297` | dialogue or log box corner (top-left; top-right is h-flipped, bottom-left v-flipped, bottom-right hv-flipped) |
| 98 | `0x2DA` | dialogue or log box side (right side h-flipped) |
| 99 | `0x2DB` | solid colour index 1: window fill (BG1) |
| 100 | `0x2DC` | portrait frame corner (TR h-flipped, BL v-flipped, BR hv-flipped) |
| 101 | `0x2DD` | portrait frame top edge (bottom v-flipped) |
| 102 | `0x2DE` | portrait frame left side (right h-flipped) |
| `0x60` | `0x278` + `0x60` | menu window corner: TL; TR h-flipped; BL v-flipped; BR hv |
| `0x61` | `0x278` + `0x61` | menu window horizontal edge (top as-is, bottom v-flipped) |
| `0x62` | `0x2DA` | menu window vertical side (right h-flipped) |

- The BG tile column is the font index plus `0x278` (the rule in section 2). `0x62` is decimal 98, the same tile as the dialogue side (`i=98`), so the menu and box sides are one tile.
- Menu windows (afterq, gu): interior = text canvas tiles over the BG1 fill `i=99`. Windows seen: info window (cols 2 to 28, rows 3 to 15), small windows (rows 16 to 20), and a Yes/No window split by a divider using `0x60` and `0x62` (#62 B 2).
- The portrait frame style differs from the box corner (`i=31` versus `i=100`), so the two are different tiles (#62 B 2).

## 4. Palettes

- BG palette 15 (both engines) = `system.sbin` `fontpal` palette 0. `fontpal` has 16 RGBX entries per palette (4 bytes each, 5-bit truncated after quantising). Entries are copied by `FUN_0201ae94` to RAM `0x021019D4` and written from palette index `0xF0` (#62 B 3, C 4). **[blue-verified]**, 16 of 16 colours.
- Palettes 1 and 2 to 6 of `fontpal` exist; in this ROM palette 1 is identical to palette 0. The selector (`FUN_02015fd8() == 1`) chooses between them. **[hypothesis]** for the meaning (not checked, B 3).
- BG palette 14 holds the portrait (`kao004` slot 0 palette, 16 of 16). **[blue-verified]** (#62 B 6).
- BG palette 14 is also the minimap and HUD alt palette (`0x1686`, `0`, `0x1C`, and so on). It was not found in `system.sbin`: source unknown (#62 C 4). **[unverified]**.
- OBJ palette 15 = `fontsppa` (16 colours) (#62 C 4). **[blue-verified]**.
- The colour indices used by the text (`#C` and the letter map) and their RGB values are in [text-engine.md](text-engine.md#colours).

## 5. Cursor and wait marker (`fontsp`)

**[blue-verified]**, high for source, slot and palette; **low** for the roles of tiles 5 to 15 (#62 B 4, C 4).

- `system.sbin` `fontsp` is 16 tiles after a count word of 16. Loaded by `FUN_0201aff0` to OBJ VRAM `0x06407E00` (engine A) or `0x06607E00` (engine B) = OBJ tile `1008 + i`. Palette: `fontsppa` to OBJ palette 15 (palette index `0x1F0`) (#62 B 4).
- OBJ mode 0 (normal), priority 0, 4bpp.
- Tile roles (from the picture of `fontsp.png`, visual only):
  - 0 and 1: down arrow, the wait-for-button marker. One 16x8 OAM entry at tile 1008, at (120, 171) in post_accept. That is the centre of the box, inside the bottom edge.
  - 2 and 3: left and right arrows (16x8-ish).
  - 4: right-pointing cursor. One 8x8 OAM entry at tile 1012, at (192, 151) in afterq, 8 px left of the "No" text.
  - 5: small triangle; 6: line; 7 to 10: corner brackets; 11 and 12: up and "!" triangles. **[low]**, visual only.
- Blink: the findings do not say whether the `fontsp` marker blinks. The text engine's prompt arrow is a separate hardware sprite that is drawn only while a frame-counter bit is set (8 frames on, 8 off); whether it is the `fontsp` marker was not checked. See [text-engine.md](text-engine.md#dialogue-speed-and-waits) (#62 A 6).

## 6. WIN0 and the alpha log box

**[blue-verified]** for the table and the blend arithmetic; the table was read once (#62 C 4).

- `FUN_0201966c` (`02019738`) sets DMA 0 from `*(0x020F2138) + 4` into WIN0H (`0x04000040`). WIN0V = `0x00C0` (full height).
- Captured table at `0x020F2460`: every row is `0xFFFF` (empty) except rows 139 to 172, which are `0x13ED`, i.e. x 19 to 236.
- So the translucent log box is the rectangle x 19 to 236, y 139 to 172. The rounded look is the BG0 frame plus this rectangle.
- WINOUT = `0x003D` (BG1 not drawn outside the window). WININ = `0x3F3F`.
- **Blend:** BLDCNT = `0x1C42` (BG1 first target, alpha mode, second targets BG2, BG3 and OBJ), BLDALPHA = `0x0808`, so out = (BG1 x 8 + below x 8) / 16, where `below` is the floor or sprite pixel.
- Check: BG1 blue (32, 74, 106) over floor (164, 222, 0) gives (98, 148, 53); the capture has (96, 144, 48). **[blue-verified]** for the formula. The result is close, not exact: the capture is 2 to 5 units lower per channel, and the findings do not explain the gap.
- **Not verified:** whether the box resizes (not seen; the Rust renderer should use the constant rectangle). The 10 px in `dng1` (minimap dot corner, x 24 to 27, y 92 to 94) and 83 px in `b2` (around the leader and damage number, x 127 to 150, y 132 to 158) that do not match: sprite-animation or alpha related, not examined.

## 7. BG3 floor scroll

**[blue-verified]**, high (#62 C 2). This resolves the scroll point in the #61 notes. The floor is on BG3, not BG0.

- `FUN_02019268` (`FlushDisplayState`, called from the vblank commit at `02000c50`) copies the shadow arrays to the BG registers. Engine A shadow = `0x020F38D0` (8 signed 16-bit values: BG0 x, y through BG3 x, y). Engine B shadow = `0x020F38C0`. The copy writes the registers `0x04000010..1C` and `0x04001010..1C`; the findings give the literals as `DAT_020195c4` and `DAT_020195e0` without saying which is which.
- **Capture values** (shadow bytes): `dng1` BG3 = (-20, 460); `b2` BG3 = (4, 412). BG0 to BG2 are (0, 0) in both. These equal the camera s16 values at `+0x15F88` and `+0x15F8A` of #61 P4.
- The 512x256 map wraps: x mod 512, y mod 256 (460 becomes 204).
- Floor render test: sample BG3 at ((px + camx) mod 512, (py + camy) mod 256). **[blue-verified]**, high: the scratch compositor with this rule matches both frames (10 and 83 px differ, see [Not verified](#not-verified)).
- BG3 uses BG palettes 0 to 10 (the map's palette bits). Tile data at char base `0x8000`, tiles 1 to 983. The source is the #60 tileset (`bpc`/`bpl`); the identity of the palette and tile files was not re-matched here. **[hypothesis]** for the file ids.
- **Why it matters:** the BG scroll registers read back 0 (write-only), so a read of the registers gives no scroll. Read the shadow.

## 8. Portrait placement

**[blue-verified]**, high for position and palette; the mirror rule is **medium** (one sample) (#62 B 6, B 7).

- Portrait: 25 consecutive BG0 tiles `0xB8` to `0xD0` (VRAM `0x06001700` onward), placed row-major at BG0 cols 24 to 28, rows 10 to 14 (pixels x 192 to 232, y 80 to 120). Map entries have no flip bits. Palette bank 14.
- Frame: BG0 cols 23 to 29, rows 9 to 15 (7 x 7 tiles; pixels x 184 to 240, y 72 to 128). Corner `i=100`, edges `i=101` and `102` (see section 3).
- Placement relative to the box: the frame bottom is at y 128, the box top at y 136 (an 8 px gap). The right edge is x 240, the same as the box.
- **Mirror:** the displayed image is the horizontal mirror of the decoded slot 0 of `kao004` (1588 of 1600 pixels equal after a pixel hflip; 12 pixels differ in dither rows 6, 7 and 17). The map has no flip bits, so either the game stores mirrored pixels or the emotion or side makes the portrait face that way. Whether the mirror depends on the side (here the portrait is right of the box, Charmander) is **not verified**: one sample. Also: palette index 0 is unused in this image (pixels are 1 to f), so "colour 0 is drawn" is unverified here.
- What selects the portrait: not a text code (#62 A 7). See [portraits.md](../formats/portraits.md).

## 9. Assets

| File | Where it goes | Tag and evidence |
|------|---------------|-----------------|
| `system.sbin` `font` (136 tiles, 32 bytes each, after a count word of 136; header word `0x88`) | BG tile `0x278` (`0x06004F00` engine A, `0x06204F00` engine B) | [blue-verified], 135 of 136 tiles equal VRAM (#62 B 2) |
| `system.sbin` `fontpal` (`0x200` bytes; palette 0 used) | BG palette 15, via `FUN_0201ae94` | [blue-verified], 16 of 16 (#62 B 3) |
| `system.sbin` `fontsp` (16 tiles after a count of 16) | OBJ tile `1008` (`0x06407E00`, `0x06607E00`) | [blue-verified], 16 of 16 (#62 B 4) |
| `system.sbin` `fontsppa` (16 colours) | OBJ palette 15 (`0x1F0`) | [blue-verified] (#62 B 4, C 4) |
| `system.sbin` `panelnn` (SIR0, 1648 bytes: 20-byte header, 50 tiles) | BG tile `0x300` (`0x06006000`) | [blue-verified] byte match at `+20`, and `0x302` at `+84` (#62 C 3, B 5). The `panelnf`, `panelng`, `panelni`, `panelns` variants exist; only `n` is confirmed for this capture |
| `dungeon.sbin` `etcfont` (SIR0, 50 tiles, 1604 bytes) | OBJ tile 512 onward (shadow tiles 513 and 520, minimap dot 533, damage number 534) | [blue-verified] byte matches (#62 C 4) |
| `monster.sbin` `kaoNNN` | opened by name `kao%03d` (`FUN_02001ff8`); consumer `FUN_0202ae50` | [blue-verified] (#62 B 6); see [portraits.md](../formats/portraits.md) |
| `dungeon.sbin` `hp5font`, `levfont`, `zmappat`, `subwp0`, `subwp0p`, `colvec`, `etcfonta`, `fixedmap`, `mapparam` | not traced to the capture | not verified (#62 C 4) |
| item sprite `440` (`itempat` or `ornament`) | OBJ tile 440 | found in `dungeon.sbin` `0xf49f4` and `ornament.sbin` `0x16550` (#62 C 4) |

## Not verified

- Engine B's dungeon layout (#62 C did not cover it; the party menu is not covered either).
- The palette selector `FUN_02015fd8` and what chooses palette 0 or 1.
- BG1's default tile `0x27A`: B calls it blank, C says it renders grey. Not reconciled.
- The status-digit tile source (tiles `0x2CC` to `0x2D7`) and the minimap drawing rule (what it draws per cell, and the colours of palettes 14 and 15 indexes). The #62 C investigator's table has the minimap source as "not found" (low).
- Roles of the cursor tiles 5 to 15 (visual only).
- The `panelnf`, `panelnn` language mapping beyond `n`.
- Whether the log box resizes. Only one `dng1` table was read.
- The mirror rule for the portrait (one sample).
- BG2 and BG3 use in other dungeon states (stairs and item lists); only the `dng1` and `b2` frames were checked.

## Open questions

- What writes the minimap canvas tiles `0x0B8` to `0x277` at run time (C 6 says a hook writes BG VRAM there; not located).
- What the 10 px and 83 px residuals in the two compositor checks are (C 5).
- Whether the marker blinks (the findings do not say).

## Red comparison

None in the findings. No `pmd-red` file or function was compared for this note.

## Links

- Text engine and colours: [text-engine.md](text-engine.md)
- Text spec: [formats/text.md](../formats/text.md)
- Portraits: [formats/portraits.md](../formats/portraits.md)
- I/O registers and per-frame flush: [io.md](io.md)
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
