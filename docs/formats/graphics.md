# Graphics primitives

Issue: [#32](https://github.com/TorratDev/rust-mm/issues/32). Code: `crates/formats/src/gfx.rs`.

**Verification status:** 4bpp tiles and tilemap entries verified against Blue (portraits, sprites, tilesets render correctly, 2026-10-16). BGR555 is unit-tested only: Blue's files use RGBX (below).

- **Palette.** BGR555 little-endian: red in bits 0–4, green 5–9, blue 10–14. Each channel is widened to 8 bits by repeating its top bits (31 → 255, 16 → 132). Index 0 can be made transparent for sprite palettes.
- **4bpp tile.** 32 bytes, 8×8 pixels. Low nibble is the left pixel.
- **8bpp tile.** 64 bytes, one index per pixel.
- **Tilemap entry.** 16 bits: tile index bits 0–9, horizontal flip bit 10, vertical flip bit 11, palette bank bits 12–15.
- **Rendering.** `render_tile` applies flips and a palette bank. Out-of-range indices render as transparent.

## Blue's palettes: RGBX

Code: `crates/formats/src/graphics.rs`. **Verified** against Blue (2026-10-16).

Blue does not store BGR555 palettes in its files. Every palette is 4 bytes per color: `r, g, b, x`, with `x` = `0x80` (dungeon, sprite, portrait palettes) or `0` (ground BPL). 16 colors = 64 bytes. Color 0 is transparent for sprites and tiles drawn over other layers; portraits draw it opaque. BGR555 (above) is still what the hardware uses, so it matters for RAM dumps.

`graphics.rs` also holds `IndexedImage`, the `(palette, index)` image the portrait, sprite and tileset decoders draw into, and `tile_sheet` for browsing raw tiles.

## Where graphics live

| Archive | Contents | Spec |
|---------|----------|------|
| `monster.sbin` | `axNNN` sprites, `kaoNNN` portraits, `palet` | [`sprites.md`](sprites.md), [`portraits.md`](portraits.md) |
| `dungeon.sbin` | `bNN*` tilesets, fonts, UI parts | [`tilesets.md`](tilesets.md) |
| `ground.sbin` | town and scene maps | [`maps.md`](maps.md) |
| `system.sbin` | fonts | [`text.md`](text.md#fonts) |
| `effect.sbin` | effect objects and backgrounds | [`effects.md`](effects.md) |
| `titlemenu.sbin` | title menu backdrops (`titlenK`, `subdef<lang>`) | [below](#title-menu-screens) |
| `ornament.sbin`, `sample.sbin` | ornaments, samples | not specified yet (open) |

No Nitro containers (NCLR, NCGR, NSCR, NCER, NANR) are used.

## Title menu screens

Code: `crates/formats/src/titlemenu.rs`. Evidence: investigator A on [#380](https://github.com/TorratDev/rust-mm/issues/380), **[blue-verified]** by VRAM and palette-RAM byte comparison at the menu (EUR dump, py-desmume). The parser itself is unit-tested on synthetic data; a ROM-gated test over the real files is still to do.

- **Files** (`titlemenu.sbin`): `titlen0`, `titlen1`, `titlen2` (bottom backdrop, one picked at random per title entry) and `subdef<lang>` (top logo; `subdef`, `subdefG`, `subdefF`, `subdefI`, `subdefS`). Each has a palette file with a trailing `p` (`titlen0p`, `subdefp`, ...).
- **Screen file.** AT4PX. The decoded blob is:

| Offset | Size | Content |
|---|---|---|
| `0x0000` | `0x800` | map 0: 32×32 `u16` entries ([tilemap entry](#graphics-primitives) above) |
| `0x0800` | `0x800` | map 1: 32×32 entries (`titlen` uses only the first 24 rows) |
| `0x1000` | rest | 4bpp tiles (769 / 749 / 768 for `titlen0` / `1` / `2`) |

- **Palette file.** RGBX, 224 colours (896 bytes): 16 banks of 16, 14 used.
- **Layers.** Engine A (bottom): BG3 = `titlenK` map 1, BG2 = map 0 (all zero). Engine B (top): BG2 = `subdef` map 0 (the logo), BG3 = map 1 (a fill of entry `0x014F`). Tiles go to the engine's BG tile base `+0x8000`.
- **Fade.** Software, 32 steps in and 32 out; the counter `n` goes 0 to 32 and back. Per colour, with `r, g, b` the RGBX bytes and `lvl = min(n, 31)`: each channel is `c * lvl >> 8` masked to 5 bits, packed as BGR555 (`r | g << 5 | b << 10`). The end value is `c * 31 >> 8` (255 gives 30, white reads `0x7BDE`), not `c >> 3`: a plain `>> 3` matches only 188 (A) and 125 (B) of 224 palette entries, the formula 224 of 224. Implemented as `titlemenu::fade_color`.

## Open items

- `effect.sbin` objects (`efob`) and backgrounds (`efbg`) are decoded ([`effects.md`](effects.md)); their frame meanings are open.
- `ornament.sbin` and `sample.sbin` layouts are not decoded, nor the world-map parts of `titlemenu.sbin`; the viewer shows their AT4PX parts as raw tile sheets and RGBX parts as palettes.
- `nitro_haltscreen*.sbin` decodes (AT4PX, 49152 bytes, 8bpp-indexed text) but its pixel layout (two 128-pixel halves) is not pinned down.
