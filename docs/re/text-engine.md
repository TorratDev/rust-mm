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

## Red comparison

Red's `charmap.txt` names `CENTER_ALIGN` (`#+`), `UNK_MACRO_3E` (`#>`) and `UNK_MACRO_5F_*` (`#_`), and lists `#~` as wait frames. Blue's `#+` centres the line, which matches the name. Blue's `#>` sets an absolute x, not an alignment, so the red reading is wrong there. Blue's `#~` waits frames, which matches Red's note. Blue's `#_` pushes a shade from a table, which Red does not describe. `pmd-red` is only read, not copied ([`references.md`](../references.md)).

## Open questions

- `#[ … ]` callback and the portrait link (#34).
- `$h`, `$t`, `$m`, `$n`, `$i` table contents (runtime only, overlay RAM).
- `#1`–`#9`: 1 to 11 uses each in English, all at a string start.
- Flag meanings of font glyphs; which font gets which flag.
- `kanji_b` buffer destination: loader writes `0x06206100`, runtime pointer `0x06616BE4`.
- Fallback glyph entry at `020938E0`: its +8 word is nonzero, so it may not be a valid entry.

## Links
- Symbols: [symbols.md](../symbols.md) (not changed here; see the note in the PR).
- Verification: [verification.md](../verification.md)
- Spec: [formats/text.md](../formats/text.md)
