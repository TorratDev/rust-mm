# Text engine: encoding, `#` and `$` codes, string lookup, fonts

**Date:** 2026-10-18
**Status:** blue-verified for the byte rules, the code syntax and the lookup and font layout; hypothesis for what the unnamed codes do
**Subsystem:** text (issue [#36](https://github.com/TorratDev/rust-mm/issues/36))

Dump: `BlueRescueTeamEur` ([`supported-dumps.md`](../supported-dumps.md)). Addresses are ARM9 RAM unless marked `ov_`. Spec this note supports: [`formats/text.md`](../formats/text.md).

## Addresses

| Address | Name | Source |
|---------|------|--------|
| `01ff8d80` | glyph lead-byte test (`0x81`–`0x84`, `0x87`) | ghidra-cli decompile |
| `02010f9c` | scanner: two-byte leads `0x81`–`0x84`, `0x87` | ghidra-cli decompile |
| `01ff994c` | draw: plain character code; descender test (`p j q y g`) | ghidra-cli decompile |
| `0202df1c` | `$` expander: `$$`, `$-`, `$dN`, `$vNW`, `$VNW`, `$mN`, `$mm`, `$iN`, `$nN`, `$t`, `$h` | ghidra-cli decompile |
| `02014098` | `#` code parser (reached through trampoline `020143c8`) | ghidra-cli decompile |
| `020143e8` | `#>` absolute x | ghidra-cli decompile |
| `02013cdc` | colour index from a character (digits 0–9; letters map to 2, 4, 5, 6, 7, 15) | ghidra-cli decompile |
| `0208625c` | divide used by `#+` centring | ghidra-cli decompile |
| `0201d948` | text lookup, about 80 callers (`thunk_FUN_0201d948`) | ghidra-cli decompile, x-ref |
| `020df8c4` | language pointer table `e, g, f, i, s` | `memory read` |
| `020df8d8` | ARM9 resident string pointer table (IDs with bit 15) | `memory read` |
| `020974ac` | resident debug strings `#CCscenario try dummy#R`, `Seek error` | `memory read` |
| `01ff8e40` | glyph lookup: binary search on code at +4, fallback `020938E0` | ghidra-cli decompile, runtime (frame 900) |
| `01ff994c` | glyph advance = width + `*0x020EA7CC` | ghidra-cli decompile, runtime |
| `020160a4` | font loader (`kanji_a` at `02093b88`, `kanji_b` at `02093b90`) | ghidra-cli decompile, runtime |
| `020EA7EC` | font selector (index into the array at `020EA7F8`; 0 at frame 900) | runtime |
| `ov_*` | `$h`, `$t`, `$m`, `$n`, `$i` table contents | not reached; runtime-only |

## Evidence

- **Ghidra (ghidra-cli, `rescue-arm9-a` / `-b`):** decompiles and x-refs of the functions above. The `#` table and `$` table were read from `FUN_02014098` and `FUN_0202df1c`.
- **Format strings in RAM:** `020b2e90` = `%d`, `020b2e94` = `%*d`, `020b2e98` = `%0*d`, `020b2ea0` = `$`, `020b2e84` = `: `.
- **Runtime (py-desmume, `verification/v7-trace/pydesmume_dump.py`, copy C):** `kanji_a` content at `0x0601CB4C` and `kanji_b` at `0x06616BE4` at frame 900, from the retail V3 dump (SHA-1 match). Renders of `A`, `B`, `1`, `i`, `m` read correctly. Outputs stay in the scratch folder; only the command is recorded here.
- **Not checked:** the `.str` byte counts of each code in the real `message_e.str` (the dump is not in the repo; the ROM-gated tests check counts once a dump is configured).

## Findings

**Encoding** `[blue-verified]`
- Only `0x81`–`0x84` and `0x87` lead two-byte glyphs. The older `0x81`–`0x9F` rule is wrong for this scanner and for the expander.
- `~XX` is a hex-byte escape.
- Latin text is single-byte.

**`#` codes** `[blue-verified]` for the arguments and the action; meanings `[red-hypothesis]` unless noted
- Argument layout, from `02014098`: `#=b`, `#yb`, `#.b`, `#;b`, `#_b`, `#~b` take one byte; `#Sbc` two; `#Cc`, `#cc` one char; `#n`, `#:`, `#+`, `#P`, `#p`, `#W`, `#R`, `#r` none; `#>NNN.` decimal digits ending in `.`; `#[ … ]` a span up to `]`.
- Optional dot `[blue-verified]`: after the byte of `#=`, `#y` and `#_`, the parser skips a `.` (`if (pcVar5[3] == '.') param_2 = pcVar5 + 4`, `decompile 02014098`). It does not for `#.`, `#;` or `#~`. `#>NNN.` uses the same dot as its terminator. The decoder does this; 60 strings in the five `message_*.str` (12 per file) have `#=@.` and would leak a `.` otherwise.
- `#+` centres: x = (box width × 8 − text width) / 2 `[blue-verified]`. Red's meaning matches.
- `#>` sets an absolute x. It is not an alignment operator (the red note said alignment).
- `#_b` pushes a shade from table `DAT_020143bc` (the old doc said unknown).
- `#p` is `#P` with a flag set (flag meaning open).
- `#[ … ]` passes its bytes to a callback reached through state + 0x12. The callback is not traced. It may be the portrait change (#34).
- `#1`–`#9` and any other letter: `Code`, meaning not identified.

**`$` codes** `[blue-verified]` for the syntax; table contents `[unverified]`
- `$v` and `$V` read two digits (index, width).
- `$-` is a two-byte no-op.
- Unhandled letters print without the `$`.

**String lookup** `[blue-verified]`
- Bit 15 set: string from `020df8d8`, index `id & 0x7FFF`.
- Otherwise `.str` offset `(id - 1) * 4`. ID 0 also reads entry 0 (ID 0 and ID 1 alias) `[medium]`: it depends on the stack layout around `local_194`.
- Language order `020df8c4`: `e, g, f, i, s`; the enum values behind `GetLanguage()` are unverified.
- No `message_*.bin` name appears in ARM9 or the overlays: the `.bin` files are not used.

**Fonts** `[blue-verified]` for layout and widths; flag meanings `[red-hypothesis]`
- Entry 12 bytes: bitmap pointer, code, advance width, 0, flags.
- Bitmap 12 × 6 bytes, 4bpp, low nibble left.
- Advance = width + `*0x020EA7CC` (0 at frame 900).
- Flags `0xFF02`, `0xFF03` and `0xFF00`. `0xFF02` glyphs use only nibble values 0 and 15; `0xFF03` glyphs use 0–14.
- 473 glyphs in `kanji_a`, 231 in `kanji_b`, 230 shared codes; widths agree for 12 of them.

## Runtime behaviour (#62 RE-a)

**Status:** the code and the colour map are `[blue-verified]` (decompile and py-desmume runtime). The meanings below are the findings' own: `[hypothesis]` where marked. Runtime checks are scratch runs (`rt/st2.py` on the post_accept savestate). Their outputs and hashes are not in the repo, so this section records no hash. Spec differences are in [formats/text.md](../formats/text.md#divergences-from-the-runtime-62).

### Consumers

| Consumer | Address | What it is |
|----------|---------|------------|
| Dialogue or message window | `FUN_0202e5a0` (state machine, once per frame), opened by `FUN_0202ee6c` (through `0202edf0`, `0202f19c`, `0202f234`, `0202f280`, `0202f2b8`) | typewriter text, `#W` and `#P` waits, page scroll, cursor. About 45 callers. [blue-verified] |
| Static string drawer | `FUN_020144d0` (callers `02014428`, `0201467c`, `020146c8`, `02014714`) | draws a whole string at once (labels, menus). Calls `FUN_02014098` directly. Terminator parameter; `0x1b` stops; `\r` and `\n` add the line pitch parameter to y; `0x1d` adds 5 to y; `0x60` adds 6 to x. [blue-verified] |
| Dungeon message log | `ov_0001::021546f0` | one log line per call into the lower screen. Ring of 10 entries, 0x40 bytes each, x starts at 4, line pitch 0x0b. No typewriter. Stops at `\0`, `\r` and `\n`. [blue-verified] |

Every consumer calls the parser `FUN_02014098` (trampoline `FUN_020143c8`). The glyph draw is `FUN_01ff9f34(x, y, code, colour, window)`, which calls `FUN_01ff994c` (bitmap blit into the window's 4bpp tile buffer) and returns the advance. `FUN_01ff8d80` reads one character or two-byte glyph code.

### Text state

The state is a struct at `0x02132E1C` (`short *param_3` in the parser) **[blue-verified]**, decompile and runtime:

| Offset | Meaning |
|--------|---------|
| `+0x00` / `+0x02` | pen x / y, in window pixels |
| `+0x04` | saved x (`#:`) |
| `+0x0c` (u32) | line-origin x used by `#n` (0 at run time) |
| `+0x10` (u32) | current colour index (7 when the window opens) |
| `+0x14` (u32) | previous colour, one slot (`#C` and `#_` save here; `#R` restores) |
| `+0x18` (u32) | previous colour for `#c` and `#r` (separate slot) |
| `+0x1c` | page flag (`#p` = 1, `#P` = 0) |
| `+0x20` | "waiting" flag (set by `#W`, `#P`, `#p`) |
| `+0x21` | "return to caller" flag (set by `#~`, cleared by `#[`) |
| `+0x08` / `+0x0a` | prompt arrow position: `#W` sets (x + win.x × 8 − 2, y + win.y × 8 + 3). At the end of a page it resets to (0x78, (win_y + 5) × 8 − 6) |
| `+0x24` | `#[ … ]` callback pointer (argument 2 of `FUN_0202ee6c`) |
| `+0x2c` / `+0x30` | frames to wait / frames waited |

### Codes as drawn

| Code | Effect when drawn | Tag |
|------|-------------------|-----|
| `#C<x>` / `#R` | colour index = `FUN_02013cdc(x)` (see [Colours](#colours)). `#R` restores the one saved slot; there is no stack (`#C a #C b #R` gives a) | [blue-verified] |
| `#c<x>` / `#r` | the same, with its own saved slot | [blue-verified] |
| `#_<b>` | colour = the byte at table `0x020EA808` + b: a raw palette index, bypassing the letter map. `#S` writes `0x020EA7D8 + (b & 0x7F)`, so `#S` index `0x30 + k` sets `#_k`. The table is runtime-only: all 0 at frame 900 except entry `0x10`, which is 3 | [blue-verified] (code); table contents runtime-only |
| `#=<b>`, `#y<b>`, `#.<b>`, `#:`, `#;<b>`, `#>NNN.`, `#+` | pen moves, as in [formats/text.md](../formats/text.md) | [blue-verified] |
| `#n` | x = `+0x0c` (0), y += 11 | [blue-verified] |
| `\n` / `\r` in the dialogue loop | x = 4 (not the `#n` origin), y += 11 | [blue-verified] |
| `#W` | stop drawing, show the prompt arrow after the last glyph, wait for a button, then continue on the same page (state 7). Which buttons advance it: [hypothesis] (input struct `0x020F27A0`) | [blue-verified] (code) |
| `#P` | sets y = `DAT_020143c4` = `0x270F` (9999). That trips the page-full test (y > `0x22`), so state 8 runs: the arrow at the bottom, a wait, then y = 4 and the page scrolls up over 18 frames (state 10, `FUN_01ff8a88`). In windows opened with flag `0x10` the page is cleared at once instead | [blue-verified] |
| `#p` | as `#P`, with `+0x1c` = 1: the page is cleared at once (`FUN_01ff818c`) instead of scrolled | [blue-verified] |
| `#~<b>` | stores b in `+0x2c` and sets `+0x21`. The frame ends; state 2 counts frames and continues after b frames | [blue-verified] (code); runtime not reproduced (no string uses it) |
| `#[ … ]` | skips to `]` and calls `+0x24` if it is non-null; the callback may return a new text pointer | [blue-verified] (code). The callback is 0 in every sampled caller, and `#[` appears in none of the five `message_*.str` |
| `#S<b><c>` | table write, see `#_` | [blue-verified] (code) |
| `#1` to `#9` | not a code. The parser's default branch returns the string pointer unchanged (`if (cVar1 != '~') return pcVar5`), so the caller draws a literal `#` and then the digit. These are the numbering in help topic titles | [blue-verified] (code) |

**The portrait is not a text code.** It is the `param_8` struct of `FUN_0202ee6c`: the palette is copied to BG palette slots `0xe0` to `0xef` (runtime bank 14), while the window's text bank is 15. No `#` code selects it, so the portrait is independent of the `#` codes ([ui-screens.md](ui-screens.md#8-portrait-placement)) **[blue-verified]** (code), medium (about 10 callers sampled).

**Codes that occur.** In all five languages (`message_{e,g,f,i,s}.str`): `#+ #C #R #c #r #P #W #= #> #y`, and `#1` to `#9` in e, g and f only (help titles). Counts in the English table: `#+` 616, `#C` 864, `#R` 863, `#c` 341, `#r` 345, `#P` 425, `#W` 172, `#=` 24, `#>` 45, `#y` 8. **Never occur:** `#_ #S #~ #p #. #; #: #n #[` **[blue-verified]**.

### Colours

`FUN_02013cdc(ch)` **[blue-verified]**:

- `ch < 0x10` gives `ch`.
- `'0'` to `'?'` (0x30 to 0x3F) gives `ch - 0x30`.
- Letters: `C`, `D`, `N` give 6; `E`, `S`, `T` give 15; `G`, `M` give 5; `H`, `I` give 4; `W` gives 2.
- Every other letter, and any out-of-range value, gives 7 (the default).

The palette is `system.sbin` `fontpal`, file layout 128 entries of 4 bytes (r, g, b, 0x80), 8 banks of 16. Runtime BG palette bank 15 on both screens equals `fontpal` bank 0 (RGB555 of the same values). Text colour index k is palette index k of that bank **[blue-verified]** (runtime post_accept).

| Index | RGB (`fontpal` bank 0) | Used by (English `#C` and `#c` letters) |
|-------|------------------------|------------------------------------------|
| 0 | transparent | |
| 1 | `274f6f` (dark blue) | |
| 2 | `ff875f` (red-orange) | `#C2`, `#c2`, `W` |
| 3 | `000000` | outline and shadow colour (see [Glyph recolour](#glyph-recolour)) |
| 4 | `5fff5f` (green) | `#C4`, `#c4`, `H`, `I` (items) |
| 5 | `07ffff` (cyan) | `#C5`, `#c5`, `G`, `M` |
| 6 | `ffff07` (yellow) | `#C6`, `#c6`, `C`, `D`, `N` (names; "Charmander:" is yellow on screen) |
| 7 | `ffffff` (white) | default |
| 15 | `afc7e7` (light blue) | `E`, `S`, `T` |

English argument counts: `#C` G 319, 6 174, 4 171, 5 79, 2 46, N 32, D 23, I 11, M 7, C 2. `#c`: 5 184, 2 85, 6 58, 4 14.

Other banks of `fontpal` exist (banks 1 to 7). Which bank a window uses is a portrait and palette question (P2). **[hypothesis]**: the bank is chosen by the window and BG palette setup, not by the text code.

### Glyph recolour

**[blue-verified]**, `FUN_01ff994c`, high. The findings' own confidence: high for the mask, medium for the shadow direction.

- The colour mask table at `0x020939A0` has entry k = `(k − 1) × 0x11111111`, so k = 0 gives `0x66666666` and k = 1 gives 0. Per nibble, a pixel is `(p & 1) + (mask & p)`. A solid glyph pixel (value 15) becomes colour index k, and blank stays 0.
- The font flags are the low byte of the glyph entry's flags word (+10):
  - bit 0 set (`0xFF03`): pixels are copied as they are, so the colour code is ignored (multi-level, pre-coloured glyphs);
  - bit 1 set (`0xFF02`, `0xFF03`): an outline or shadow is drawn when the global at `0x020EA7C8` is non-zero (1 at run time). Neighbouring blank pixels get the nibble `*0x020EA7E0` = `0x33333333`, i.e. index 3 (black);
  - `0xFF00`: recoloured, no outline.
- Advance = glyph width + the word at `0x020EA7CC` (0 at run time).
- Descender shift: when the font selector `0x020EA7EC` is 1, the glyphs `p j q y g` are drawn 2 px lower, and code `0x8199` 2 px higher.
- The dialogue loop does not wrap long lines (no width test). Line breaks come from `\n` in the string.

### Dialogue speed and waits

**[blue-verified]**, high for the speed and the page model (runtime `rt/st2.py`); code only for `#~` and fast forward.

- Window open (`FUN_0202ee6c`) sets glyphs per frame `0x02132DD4` = 1 and the inter-glyph delay `0x02132DAC` = 1. Runtime: one glyph per frame, x advancing 4 to 5 px per frame, first line y = 4, second line y = 15 (pitch 11). The loop draws up to `*0x02132DD4` glyphs per frame, then waits `*0x02132DAC` frames.
- **Page:** three lines (y = 4, 15, 26). A line that would start at y = 37 (> `0x22`) is "page full": the text waits for a button, then scrolls or clears, as in the `#P` and `#p` rows above. In windows opened with flag `0x10`, y wraps to 4 with no wait.
- **Fast forward** `[blue-verified]` code, `[hypothesis]` for the buttons: if window flag `0x40` is set, flag `0x02` is clear, and the input struct byte `[1] & 3` is non-zero, the fast flag `0x02132DA0` is 1 and glyphs per frame becomes 99999, so the rest of the page appears in one frame. Window flag `0x04` plays a sound on each frame of waiting (`FUN_02029a00`).
- **Prompt arrow:** a hardware sprite (`FUN_02012c0c`), drawn only while the frame counter `0x02132D9C` has bit 3 set, so it blinks 8 frames on and 8 off. Positions are as in the state table above.
- **State values of `0x02132DC4`** seen at run time: 9 (idle after the last page), 2 (typing), 1 (drawing), 7 (`#W` wait), 8 (page full), 10 (scroll, 18 frames), and 6, 3, 4, 5, 11 (end and close).

### Not verified

- The `$` variable tables; the `FUN_02014790` text-width details beyond `#+`.
- Which buttons advance `#W` (the input struct is `0x020F27A0`; the bit identity is a hypothesis).
- The dungeon message-log colour source.
- `#~` and fast forward at run time (no string uses `#~`; the fast-forward button bits are unidentified).

## Red comparison

Red's `charmap.txt` names `CENTER_ALIGN` (`#+`), `UNK_MACRO_3E` (`#>`) and `UNK_MACRO_5F_*` (`#_`), and lists `#~` as wait frames. Blue's `#+` centres the line, which matches the name. Blue's `#>` sets an absolute x, not an alignment, so the red reading is wrong there. Blue's `#~` waits frames, which matches Red's note. `pmd-red` is only read, not copied ([`references.md`](../references.md)).

## Open questions

- `#[ … ]` callback: null in every sampled caller, and no message table uses `#[` (#62 RE-a). The portrait is not selected by a text code ([ui-screens.md](ui-screens.md#8-portrait-placement)).
- `$h`, `$t`, `$m`, `$n`, `$i` table contents (runtime only, overlay RAM).
- `#1`–`#9`: 1 to 11 uses each in English, all at a string start. **Resolved in #62 RE-a:** the parser returns the pointer unchanged, so the caller draws a literal `#` and the digit.
- `kanji_b` buffer destination: loader writes `0x06206100`, runtime pointer `0x06616BE4`.
- Fallback glyph entry at `020938E0`: its +8 word is nonzero, so it may not be a valid entry.

## Links
- Symbols: [symbols.md](../symbols.md) (not changed here; see the note in the PR).
- Verification: [verification.md](../verification.md)
- Spec: [formats/text.md](../formats/text.md)
