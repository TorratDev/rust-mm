# Compression: Nitro codecs and PX

Issue: [#31](https://github.com/TorratDev/rust-mm/issues/31). Code: `crates/formats/src/compress.rs`. BLZ (overlays) is in [`nds-container.md`](nds-container.md#blz-backwards-lz).

**Verification status:** LZ10, LZ11, Huffman 4/8 and RLE are implemented and unit-tested with hand-built vectors. None of them has been checked against Blue's files. The survey's header guesses are leads, not findings.

## Blob header

| Offset | Size | Field |
|--------|------|-------|
| `0x00` | 1 | type: `0x10` LZ10, `0x11` LZ11, `0x20` Huffman 4-bit, `0x28` Huffman 8-bit, `0x30` RLE |
| `0x01` | 3 | decompressed size (little-endian). If zero, the real size is the `u32` at `0x04` and the body starts at `0x08`. |

The decoder stops at the declared size. The limit is checked before allocation (default 16 MiB, `decompress_limited` to set it).

## Codecs

- **LZ10.** A flag byte covers 8 tokens, MSB first. `0` = literal. `1` = two bytes: length `(b0 >> 4) + 3`, distance `(((b0 & 0xF) << 8) | b1) + 1`.
- **LZ11.** As LZ10, but the first byte of a back-reference picks a form: `b0 >> 4 == 0` is 3 bytes with length `12 bits + 0x11`; `== 1` is 4 bytes with length `16 bits + 0x111`; otherwise 2 bytes with length `(b0 >> 4) + 1`. Distance is always `+ 1`.
- **Huffman.** The size byte at `body` gives a tree of `(size + 1) * 2` bytes, root right after it. Node byte: child offset in bits 0–5, left-leaf flag bit 6, right-leaf flag bit 7. The bitstream is 32-bit little-endian words read MSB first. 4-bit mode packs two symbols per byte, low nibble first.
- **RLE.** Flag bit 7 set = run of `(flag & 0x7F) + 3` copies of the next byte. Clear = `(flag & 0x7F) + 1` literal bytes.

## Open items

- Nitro encoders are not written, so the Nitro codecs have no round-trip tests. PX has a literal-only encoder in its tests.

## PX: AT4PX and PKDPX

Code: `crates/formats/src/px.rs`. Tests: unit tests (literal round trip, patterns, back-references, bad input) and `tests/tests/formats_rom.rs`.

**Verification status:** verified against Blue (2026-10-16). All 154 AT4PX sub-files decode to exactly their stored size; portraits, sprites, tilesets and the halt screens decode to clean images. No PKDPX is present in this dump; the 32-bit size variant is implemented from the same rule.

Header, as read statically from `FUN_02013388` (`PxDecompress`, [`re/io.md`](../re/io.md#decompression), #178 P4). **[static]**

| Offset | Size | Field |
|--------|------|-------|
| `0x00` | 4 | magic `AT4P` (header 0x12) or `AT3P` (header 0x10). The decoder compares it byte by byte; any other magic returns 0. |
| `0x04` | 1 | mode byte. `'N'` (`0x4E`) selects raw mode: copy from offset 7, count in bytes 5–6. Other values take the coded path. The other values are not enumerated (open, see #277). |
| `0x05` | 2 | `u16` container length, header included. The coded loop runs while the position, counted from the start of the header, is below it. In `'N'` mode the same field is the raw byte count. |
| `0x07` | 9 | nine control bytes at offsets 7–15, read as `f[0..8]`. Each is compared, plus 3, with the token's high nibble plus 3, and mapped to codes `0x1F` down to `0x17`. Byte 15 is the ninth control value, treated like the other eight. |
| `0x10` | 2 | `AT4P` only: decoded size, checked against the caller's expected size when `param_2 != 0`. Absent for `AT3P`. |
| `0x12` (`AT4P`) / `0x10` (`AT3P`) | | stream |

The five-byte form `AT4PX` in older notes has `X` at byte 4. `X` is not `'N'`, so it takes the coded path. This is a static inference, not checked against the decoder's mode table.

**PKDPX is not accepted by this decoder.** The compare in `FUN_02013388` admits only `AT4P` and `AT3P`, so the 32-bit `PKDPX` variant described in the crate is not supported by Blue's decoder as read. Whether another routine handles it is open (**#277**). The crate's PKDPX code has no Blue evidence.

**Entry and dispatcher** (blue-verified, [`re/io.md`](../re/io.md#decompression)). The decoder is `FUN_02013388` (`PxDecompress`). A dispatcher, `FUN_02013858`, jumps through the slot at `0x02013864`, which holds `0x02013388`. The decoder accepts the `AT3P` magic (header 0x10) as well as `AT4P` (header 0x12). The caller list is medium confidence; only the halt-screen caller is known to decode a particular file.

A command byte covers 8 items, MSB first. Bit set: copy one literal byte. Bit clear: read byte `b`, `h = b >> 4`, `l = b & 0xF`:

- `h == f[i]` (first match): write 4 nibbles as 2 bytes `n0<<4|n1`, `n2<<4|n3`. `i == 0`: all four are `l`. `i` in 1–4: all `l` except `n[i-1] = l - 1`. `i` in 5–8: all `l` except `n[i-5] = l + 1` (mod 16).
- otherwise: back-reference of `h + 3` bytes starting `0x1000 - ((l << 8) | next)` bytes before the end of the output.

The pattern rule was settled on Blue's portraits: the other three candidate readings (base `l±1` with one nibble back at `l`, swapped nibble order) leave stray pixels and score worse on a smoothness count over all 13 Bulbasaur faces.

## Which codecs Blue uses

- **BLZ**: ARM9 static area and all 145 overlays ([`nds-container.md`](nds-container.md)).
- **AT4PX**: inside the pack archives (portrait and learnset payloads, tilesets, title and halt screens).
- **Nitro LZ10/LZ11/Huffman/RLE**: not found in this dump. Sub-files starting `10 00 00 00` (`hpNfont*`, `fontsp`) are 8x8 fonts with a glyph count of 16, not LZ10. **[static, negative, low confidence]**: a magic search of the ARM9 found no BLZ, LZ10, LZ11 or Huffman decoder; `FUN_02013388` is the only PX decoder found. Absence is **not proven**. Open, see **#277**.
- **Ground BPC tiles**: compressed with a scheme not identified yet ([`maps.md`](maps.md)).

`message_*.str` is **not compressed**: `20 34 00 00` is the first entry of a `u32` offset table (0x3420 / 4 = 3336 strings, [`text.md`](text.md)).
