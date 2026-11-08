# Text: message tables, encoding, control codes and fonts

Issue: [#36](https://github.com/TorratDev/rust-mm/issues/36). Code: `crates/formats/src/text.rs`, `crates/formats/src/font.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`messages_decode`, `fonts_decode`). Viewer: `message_X.str`, `system.sbin` → `kanji_a`, `kanji_b`, `font*`. Runtime and code evidence: [`re/text-engine.md`](../re/text-engine.md).

**Verification status:** the byte rules and the `#` and `$` syntax are `blue-verified` from the ARM9 code (2026-10-18, see [`re/text-engine.md`](../re/text-engine.md)). In the earlier check (2026-10-16) all five languages parsed (3336 strings each), `.str` and `.bin` agreed string for string, and every string tokenized. Control code *meanings* are partly `[red-hypothesis]`. The #62 runtime checks changed several meanings (see [divergences from the runtime](#divergences-from-the-runtime-62)). The ROM-gated check of the decoder has not been run against a dump yet.

## Message tables (`message_e/f/g/i/s`)

`X` is the language: e English, f French, g German, i Italian, s Spanish. The game reads the `.str` file for the language in the table at `020df8c4`, indexed by `GetLanguage()` (order `e, g, f, i, s`; the enum values are not verified).

- **`.str`**: `u32` offsets, one per string (first offset / 4 = count, 3336), then NUL-terminated strings. **Not compressed**: the survey's "Huffman 4-bit" came from the first offset, `0x3420`.
- **`.bin`**: the same strings (3335; the `.str` has one more, empty, entry) as records `{u16 record size in 16-bit words, string, NUL, pad to even}`. **The game does not use them**: no `message_*.bin` name appears in the ARM9 or the overlays (`blue-verified`).

### Message IDs

Every text read goes through one lookup (`FUN_0201d948`, about 80 callers) `[blue-verified]`:

- If bit 15 of the ID is set, the string comes from a pointer table in ARM9 RAM (`020df8d8`), indexed by `id & 0x7FFF`.
- Otherwise the ID is 1-based: the string is at `.str` offset `(id - 1) * 4`. ID 0 reads the same entry as ID 1 `[medium]`.

## Encoding

- Single bytes: Latin-1 (`é` = `0xE9`); `0x0A` = new line. The draw code takes a plain character code, and Latin text is ASCII-like `[blue-verified]`.
- `0x81`–`0x84` and `0x87` start a two-byte game glyph (`Token::Glyph`), as the glyph scanner checks them `[blue-verified]`. Examples: `81 60` wave dash, `81 F4` note, `84 xx` and `87 xx` button icons. Other lead bytes are plain Latin-1.
- `~XX` is a hex-byte escape: the character with code `XX` (`~27` apostrophe, `~2c` comma) `[blue-verified]`.

## Control codes (`#`, typed as `text::Token`)

Each code takes the arguments listed here, so they never show as text. Parser `FUN_02014098` via trampoline `FUN_020143c8` `[blue-verified]`; the meanings are `[red-hypothesis]` unless marked.

| Code | Arguments | Token | Action |
|------|-----------|-------|--------|
| `#C<x>` / `#R` | 1 char / none | `Color`, `ColorEnd` | set text colour `x` (index via `FUN_02013cdc`) / restore the one saved slot |
| `#c<x>` / `#r` | 1 char / none | `AltColor`, `AltColorEnd` | set the colour for names, and restore the one alternative saved slot |
| `#=<b>` | 1 byte | `SetX` | x = `b` |
| `#yb` | 1 byte | `SetY` | y = `b` |
| `#.<b>` | 1 byte | `AddX` | x += `b` |
| `#n` | none | `LineFeed` | x = origin word `+0x0c` (0 at run time), y += 11. A `\n` in the dialogue loop sets x = 4 instead (see [divergences](#divergences-from-the-runtime-62)) |
| `#:` | none | `SaveX` | save x |
| `#;<b>` | 1 byte | `SavedX` | x = saved x + `b` |
| `#+` | none | `Centre` | centre the line: x = (box width × 8 − text width) / 2 `[blue-verified]` |
| `#>NNN.` | decimal digits, ending in `.` | `SetXAbs` | x = `NNN`, absolute (`FUN_020143e8`). Not an alignment operator |
| `#_<b>` | 1 byte | `Shade` | colour = the byte at `0x020EA808` + b, a raw palette index (run time; see [divergences](#divergences-from-the-runtime-62)) |
| `#S<b><c>` | 2 bytes | `SetTable` | `DAT_020143c0[b & 0x7F]` = `c & 0x7F` |
| `#W` | none | `WaitButton` | wait for a button `[blue-verified]` |
| `#P` | none | `PageBreak` | sets y to `DAT_020143c4` = `0x270F`, which forces the page-full path: wait for a button, then the page scrolls up over 18 frames, or is cleared at once in windows opened with flag `0x10` |
| `#p` | none | `PageBreakFlagged` | as `#P`, but the page is cleared at once instead of scrolled |
| `#~<b>` | 1 byte | `WaitFrames` | wait `b` frames |
| `#[ … ]` | up to `]` | `Tag` | span passed to the callback at window state `+0x24`. The callback is 0 in every sampled caller and no message table uses `#[`; not a portrait switch. The bytes are kept |
| `#1`–`#9` | none | literal | drawn as the character `#` and the digit (see [divergences](#divergences-from-the-runtime-62)); decoded today as `Code(c)` |
| other | none | `Code` | not identified; decoded as `Code(c)` |

An optional `.` after the byte of `#=`, `#y` and `#_` is skipped, so `#=@.x` is `SetX(0x40)` then `x` `[blue-verified]`: `decompile 02014098`, `if (pcVar5[3] == '.') param_2 = pcVar5 + 4`. `#.`, `#;` and `#~` do not skip it. `#>NNN.` ends with that dot.

A code with its arguments missing at the end of a string decodes as `Code`. `#>` followed by no digits, or without the closing `.`, also decodes as `Code('>')`.

## Divergences from the runtime (#62)

Found in #62 RE-a, by decompile and py-desmume run: [re/text-engine.md](../re/text-engine.md#runtime-behaviour-62-re-a). The table above is corrected where these apply.

1. **`#P`:** the y "reset" is not a plain reset. y is first set to `0x270F`, which forces the page-full path. The reset to y = 4 happens after the wait, with a scroll or a clear.
2. **`#p`:** the flag means the page is cleared at once instead of scrolled.
3. **`#1`–`#9`:** literal text, not codes. The parser returns the string pointer unchanged, so the caller draws `#` and the digit. The decoder emits `Code(c)` for them today; the renderer should draw them as text. This is a P1 code change, not made here.
4. **`#[ … ]`:** the callback is null in every sampled caller, and `#[` never occurs in the five message tables. The "portrait change" guess is dropped. The portrait is set by the window-open argument of `FUN_0202ee6c` (`param_8`), not by a text code.
5. **Newline:** `\n` sets x = 4 in the dialogue loop. `#n` sets x to the origin word (0 at run time). They differ, but `#n` is not used in the tables.
6. **`#_` table:** `#_` reads the table directly at `0x020EA808`, while `#S` writes `0x020EA7D8 + (b & 0x7F)`. So `#S` index `0x30 + k` sets `#_k`.

## Variables (`$`, typed as `Var` and `Number`)

Expander `FUN_0202df1c`, run before drawing `[blue-verified]` for the syntax:

| Sequence | Token | Meaning |
|----------|-------|---------|
| `$$` | text `$` | a literal `$` |
| `$-` | none | two-byte no-op |
| `$dN` | `Var { d, N }` | number from table entry `N` (printed `%d`) |
| `$vNW` | `Number { zero_pad: false, N, W }` | number, index `N`, width `W` (`%*d`) |
| `$VNW` | `Number { zero_pad: true, N, W }` | as `$v`, zero-padded (`%0*d`) |
| `$mN` | `Var { m, N }` | monster name (stride 0x50) |
| `$mm` | `Var { m, None }` | runtime monster id (`FUN_02002ea4`) |
| `$iN` | `Var { i, N }` | item name (stride 0x50) |
| `$nN` | `Var { n, N }` | name (stride 0x50) |
| `$t` | `Var { t, None }` | team name (fallback `????`) |
| `$h` | `Var { h, None }` | string at `DAT_0202e374` (runtime) |
| `$` + other letter | letter only | the letter is printed without `$` |

Both `$v` and `$V` read **two** digits: the index, then the width.

Format strings are in ARM9 RAM: `%d` (`020b2e90`), `%*d` (`020b2e94`), `%0*d` (`020b2e98`).

## Fonts

Loader `FUN_020160a4` opens `kanji_a` and `kanji_b` from `system.sbin`, and the lookup is `FUN_01ff8e40` `[blue-verified]`.

- **`kanji_a`, `kanji_b`** (`system.sbin`): the variable-width dialogue fonts. SIR0 content header `{u32 glyph count, u32 table pointer}`. Entries are 12 bytes: `u32` bitmap pointer, `u16` character code, `u16` advance width, `u16` 0, `u16` flags. The table is sorted by code and searched by binary search on the code.
- **Bitmap:** 12 rows of 6 bytes, 4bpp, low nibble on the left.
- **Advance:** width + the spacing word at `0x020EA7CC` (0 at frame 900) `[blue-verified]`.
- **Flags** `[red-hypothesis]`: `0xFF02` and `0xFF03` (the 1-bit and multi-level glyphs), and `0xFF00` (10 glyphs in `kanji_a`, codes `0x839F`–`0x83A8`, width 6). The flags' meaning is not confirmed.
- **Counts:** `kanji_a` has 473 glyphs and `kanji_b` 231. They share 230 codes, but the advance widths agree for only 12 of them.
- **Font selector** `0x020EA7EC`: 0 selects `kanji_a` (at frame 900).
- **`font`, `fontF/G/I/S`** (`system.sbin`), **`hpNfont*`**, **`fontsp`**: fixed 8x8 fonts, `u32` count then 32-byte 4bpp tiles. The lettered ones add a few language-specific glyphs.
- **`fontd8x8P1`**, `fontpal`, `fontsppa`: a further 8x8 sheet and font palettes (layout not checked).

## Open

Not verified; do not treat these as known:

- `#[ … ]` callback: null in every sampled caller and unused in the message tables. Not a portrait switch (#62 RE-a).
- Contents of the `$h`, `$t`, `$m`, `$n` and `$i` tables (runtime only).
- `#1`–`#9`: literal text, as the divergences section says (#62 RE-a). They are used at a string start in the help topic titles.
- `kanji_b` buffer destination: the loader writes to `0x06206100`, but the runtime pointer is `0x06616BE4`.
- Fallback glyph entry at `0x020938E0` (bitmap `0x020939E0`, code `0x81A1`, width 7): its +8 word is nonzero, so it may not be a valid entry.
- Two-byte game glyphs (`Token::Glyph`) have no Unicode mapping yet. "Readable UTF-8" holds for the Latin-1 text and the control codes; glyphs stay as codes until their images are matched to characters.
