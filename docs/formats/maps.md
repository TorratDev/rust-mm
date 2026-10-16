# Ground maps (`ground.sbin`)

Issue: [#35](https://github.com/TorratDev/rust-mm/issues/35). Code: `crates/formats/src/ground.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`ground_maps_parse`). Viewer: `ground.sbin` → `Xm` (rendered map, one image per layer), `Xc` (tiles), `X` (palettes). Loader and decoders: [`re/ground-bpc.md`](../re/ground-bpc.md).

**Verification status:** verified against Blue (2026-10-16). BPL and BMA: every map's chunk layers decode, use only declared chunks, and leave trailing bytes exactly when the collision or data flag is set. BPC and BPA: the decoders follow overlay 0's loader; every BPC decodes to its file end, its entries name only existing tiles and palettes, and every map with its own tiles renders.

## Resource IDs

For a map `X` (`A01P01`, `H01P01W`, `S01_FRE`, ...): `X` = BPL palettes, `Xc` = BPC tiles and chunks, `Xm` = BMA chunk map. `X1`..`X4` = BPA animated tiles for slots 0-3, present when the BPC header declares tiles for that slot. 14 maps (`B01P02A`, ...) lack their own `Xc` or `X`. The names and the loader follow Red's `ground_bg.c` (`gMapFilesTable`).

## BPL (palettes)

| Offset | Size | Field |
|--------|------|-------|
| `0x0` | 2 | palette count |
| `0x2` | 2 | has palette animation |
| `0x4` | 60 per palette | 15 RGBX colors (color 0 is transparent and not stored) |

With animation: per palette `{s16 duration, s16 frame count}`, then the frames' colors, 15 per frame.

## BPC (tiles and chunks)

Header, 8 `u16`: chunk width, chunk height (2 or 3), tile count + 1, tiles in each of the 4 animated-tile (BPA) slots, chunk count + 1. Red follows it with raw data; Blue compresses both parts:

1. **Tiles**, from offset 16: `(tile count) * 32` bytes of 4bpp tiles (tile 0 is implicit and blank). Byte commands, each emitting n+1 bytes: `0x00-0x7D` literal (n = command; `0x7E`: n = next byte; `0x7F`: n = next `u16`); `0x80-0xBE` a new fill byte after the command (`0xBF`: n in the next byte, then the byte); `0xC0-0xDE` the last fill byte (`0xDF`: n follows); `0xE0-0xFE` the fill byte before the last, the two swapping (`0xFF`: n follows). Fill bytes start at 0.
2. Pad to an even offset from offset 16.
3. **Chunks**: `w * h * (chunk count)` DS tilemap entries (chunk 0 is implicit and empty), in two passes, high bytes then low bytes. Per pass: `0x00-0x7F` skips n+1 entries; `0x80+n` one byte for n+1 entries; `0xC0+n` n+1 literal bytes.

Entries name tiles in VRAM order: the BPC tiles, then each BPA slot's tiles. Palette numbers index the BPL palettes.

## BPA (animated tiles)

`u16` tiles per frame, `u16` frame count, 4 bytes per frame (timing, not decoded), then every frame's raw 4bpp tiles. The viewer draws frame 0.

## BMA (chunk map)

| Offset | Size | Field |
|--------|------|-------|
| `0x0` | 1 | width in tiles |
| `0x1` | 1 | height in tiles |
| `0x2` | 2 | chunk width, height in tiles |
| `0x4` | 2 | width, height in chunks |
| `0x6` | 2 | layer count |
| `0x8` | 2 | has data layer |
| `0xA` | 2 | has collision layer |

Then each layer, row by row (Red's `BmaLayerNrlDecompressor`): commands until the row is full. `0xC0 + n`: n+1 literal pairs; `0x80 + n`: one pair repeated n+1 times; `n < 0x80`: n+1 empty pairs. A pair is 3 bytes holding two 12-bit chunk numbers. Rows after the first XOR each value with the row above (an empty pair copies it).

## Unknown fields

- BPA frame info bytes.
- BMA layer priority.
- Which map's tiles the 14 maps without their own `Xc`/`X` use.
- Collision and data layer encoding (kept as raw bytes in `Bma::extra`).
