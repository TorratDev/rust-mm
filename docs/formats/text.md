# Text: message tables, encoding, control codes and fonts

Issue: [#36](https://github.com/TorratDev/rust-mm/issues/36). Code: `crates/formats/src/text.rs`, `crates/formats/src/font.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`messages_decode`, `fonts_decode`). Viewer: `message_X.str`, `system.sbin` → `kanji_a`, `kanji_b`, `font*`.

**Verification status:** verified against Blue (2026-10-16): all five languages parse (3336 strings each), `.str` and `.bin` agree string for string, every string tokenizes; `kanji_a` (473 glyphs) covers printable ASCII and renders legibly. Control code meanings are partly `[red-hypothesis]`.

## Message tables (`message_e/f/g/i/s`)

`X` is the language: e English, f French, g German, i Italian, s Spanish. Message IDs are string indices (the same in every language; 0–17 are the type names).

- **`.str`**: `u32` offsets, one per string (first offset / 4 = count, 3336), then NUL-terminated strings. **Not compressed**: the survey's "Huffman 4-bit" came from the first offset, `0x3420`.
- **`.bin`**: the same strings (3335; the `.str` has one more, empty, entry) as records `{u16 record size in 16-bit words, string, NUL, pad to even}`.

## Encoding

- Single bytes: Latin-1 (`é` = `0xE9`); `0x0A` = new line.
- `0x81`–`0x9F` start a two-byte game glyph (Shift-JIS-like codes: `81 60` wave dash, `81 F4` note, `83 BF 83 C4` after amounts of money (a currency glyph pair `[hypothesis]`), `87 xx` and `84 xx` button icons). The fonts map these codes to glyphs.

## Control codes (typed as `text::Token`)

| Code | Token | Meaning |
|------|-------|---------|
| `#C<x>` / `#R` | `Color`, `ColorEnd` | text color `x` (letters and digits seen) on / off |
| `#c<x>` / `#r` | `AltColor`, `AltColorEnd` | second color pair, used around names |
| `#P` | `PageBreak` | wait for a button, new page `[red-hypothesis]` |
| `#W` | `WaitButton` | wait for a button `[red-hypothesis]` |
| `#=<b>` | `SetX` | move the cursor to x = byte `b` |
| `#y<b>` | `SetY` | move the cursor vertically `[hypothesis]` |
| `#+` | `Code('+')` | centre the line `[red-hypothesis]`, Red's `CENTER_ALIGN`: 616 uses in English, 254 at a string start |
| `#>` | `Code('>')` | alignment operator `[red-hypothesis]`, Red's `UNK_MACRO_3E`: 45 uses, each just before a number or name |
| `#1`–`#9` | `Code` | not identified: 1 to 11 uses each, all at a string start, in the help topic titles |
| `$m<n>`, `$i<n>`, `$d<n>`, `$n<n>`, `$t`, `$h` | `Var` | monster (`POKEMON_n`), item (`MOVE_ITEM_n`), number (`VALUE_n`), name (`NAME_n`), team name, friend area `[red-hypothesis]`, from Red's `charmap.txt` |
| `$v<n>` | `Var` | not identified: `$v0` 11 uses, `$v1` 3 uses in English |
| `~XX` | (text) | literal character with hex code `XX` (`~27` apostrophe, `~2c` comma) |

## Fonts

- **`kanji_a`, `kanji_b`** (`system.sbin`): the variable-width dialogue fonts. SIR0 content header `{u32 glyph count, u32 table pointer}`; 12-byte entries `{u32 bitmap pointer, u16 character code, u16 advance width, u16 0, u16 flags}` (flags `0xFF02` or `0xFF03`), sorted by code. A bitmap is 12x12, 4bpp, rows of 6 bytes, low nibble on the left.
- **`font`, `fontF/G/I/S`** (`system.sbin`), **`hpNfont*`**, **`fontsp`**: fixed 8x8 fonts, `u32` count then 32-byte 4bpp tiles. The lettered ones add a few language-specific glyphs.
- **`fontd8x8P1`**, `fontpal`, `fontsppa`: a further 8x8 sheet and font palettes (layout not checked).

## Unknown fields

- `#1`–`#9` and `$v`; which code changes the portrait.
- Whether `#+` and `#>` behave as Red's `CENTER_ALIGN` and alignment macro in Blue. The counts above agree with that reading; a renderer check settles it.
- Red's `charmap.txt` also lists `#~` (wait frames) and `#_` (`UNK_MACRO_5F_*`). Blue's strings have not been checked for them yet.
- Glyph flags; how `kanji_a` and `kanji_b` split the character set.
- Two-byte game glyphs (`Token::Glyph`) have no Unicode mapping yet. The "readable UTF-8" goal holds for the Latin-1 text and the control codes only; glyphs stay as codes until their images are matched to characters.
