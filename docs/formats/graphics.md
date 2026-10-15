# Graphics primitives

Issue: [#32](https://github.com/TorratDev/rust-mm/issues/32). Code: `crates/formats/src/gfx.rs`.

**Verification status:** implemented and unit-tested. Blue's graphics are not yet located (the V4 survey found no Nitro container magics among the top-level files), so nothing has been checked against a real image.

- **Palette.** BGR555 little-endian: red in bits 0–4, green 5–9, blue 10–14. Each channel is widened to 8 bits by repeating its top bits (31 → 255, 16 → 132). Index 0 can be made transparent for sprite palettes.
- **4bpp tile.** 32 bytes, 8×8 pixels. Low nibble is the left pixel.
- **8bpp tile.** 64 bytes, one index per pixel.
- **Tilemap entry.** 16 bits: tile index bits 0–9, horizontal flip bit 10, vertical flip bit 11, palette bank bits 12–15.
- **Rendering.** `render_tile` applies flips and a palette bank. Out-of-range indices render as transparent.

## Open items

- Find the archives that hold graphics. Candidates are `monster.sbin`, `dungeon.sbin`, `ground.sbin` and `system.sbin`, all tagged *unknown*.
- Nitro containers (NCLR, NCGR, NSCR, NCER, NANR) are not implemented. Add them only if the survey finds them.
