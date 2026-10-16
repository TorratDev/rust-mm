# Ground maps (`ground.sbin`)

Issue: [#35](https://github.com/TorratDev/rust-mm/issues/35). Code: `crates/formats/src/ground.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`ground_maps_parse`). Viewer: `ground.sbin` → `Xm` (chunk-number map), `X` (palettes).

**Verification status:** partial. BPL and BMA verified against Blue (2026-10-16): every map's chunk layers decode, use only declared chunks, and leave trailing bytes exactly when the collision or data flag is set. **BPC tile data is compressed in Blue with a scheme not yet identified** (Red stores it raw), so maps do not render yet.

## Resource IDs

For a map `X` (`A01P01`, `H01P01W`, `S01_FRE`, ...): `X` = BPL palettes, `Xc` = BPC tiles and chunks, `Xm` = BMA chunk map. Some maps have extra parts (`X1`, `X2`, headers like `10 00 06 00`; animated tiles `[hypothesis]`). The names and the loader follow Red's `ground_bg.c` (`gMapFilesTable`).

## BPL (palettes)

| Offset | Size | Field |
|--------|------|-------|
| `0x0` | 2 | palette count |
| `0x2` | 2 | has palette animation |
| `0x4` | 60 per palette | 15 RGBX colors (color 0 is transparent and not stored) |

With animation: per palette `{s16 duration, s16 frame count}`, then the frames' colors, 15 per frame.

## BPC (tiles and chunks)

Header, 8 `u16`: chunk width, chunk height (2 or 3), tile count + 1, tiles in each of the 4 animated-tile slots, chunk count + 1. Red follows it with raw 4bpp tiles (tile 0 is implicit) and then 9 (or 4) tilemap entries per chunk. Blue's payload is smaller than that and compressed; simple run/literal schemes (byte and word units) do not fit. Next step: find the loader in the ARM9 or overlays with ghidra-cli.

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

- BPC compression (blocks rendering).
- Collision and data layer encoding (kept as raw bytes in `Bma::extra`).
- The `X1` and `X2` parts.
