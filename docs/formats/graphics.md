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
| `effect.sbin`, `ornament.sbin`, `titlemenu.sbin`, `sample.sbin` | effects, title screens, UI | not specified yet (open) |

No Nitro containers (NCLR, NCGR, NSCR, NCER, NANR) are used.

## Open items

- `effect.sbin` (`efbg`, `efob` SIR0), `ornament.sbin`, `titlemenu.sbin` and `sample.sbin` layouts are not decoded; the viewer shows their AT4PX parts as raw tile sheets and RGBX parts as palettes.
- `nitro_haltscreen*.sbin` decodes (AT4PX, 49152 bytes, 8bpp-indexed text) but its pixel layout (two 128-pixel halves) is not pinned down.
