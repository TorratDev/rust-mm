# Compression: Nitro codecs

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

- `message_*.str` starts with `20 34 00 00`. The survey guessed Huffman 4-bit, but this is unverified. Decode it with each codec and check the result.
- `nitro_haltscreen*.sbin` start with `AT4P`, which may be an AT4PX container (not implemented).
- Encoders are not written, so there are no round-trip tests yet.
