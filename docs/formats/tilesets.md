# Dungeon tilesets (`dungeon.sbin` `bNN*`)

Issue: [#35](https://github.com/TorratDev/rust-mm/issues/35). Code: `crates/formats/src/tileset.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`tilesets_decode`), `tests/tests/viewer_rom.rs`. Viewer: `dungeon.sbin` → any `bNN*` part.

**Verification status:** verified against Blue (2026-10-16). 76 tilesets; all load, all metatiles render, every autotile entry names an existing metatile. Tileset 0 checked by eye (a water and sand cave). The autotile bit order is a hypothesis.

## Parts of tileset `NN`

| Part | Format | Contents |
|------|--------|----------|
| `bNNfon` | AT4PX | 4bpp 8x8 tiles |
| `bNNcel` | AT4PX | metatiles: 9 tilemap entries (3x3 tiles, 24x24 pixels), 18 bytes each; entry = tile bits 0–9, hflip bit 10, vflip bit 11, palette bits 12–15 |
| `bNNpal` | raw | 12 palettes x 16 RGBX colors |
| `bNNcex` | SIR0 + AT4PX | autotile table, 2352 bytes |
| `bNNcanm` | SIR0 | palette animation |

Tilesets 22, 26, 27, 34, 38, 57, 58 and 61 have only `pal` and `canm` (palette variants of other tilesets); 64–75 have no `cex`. In some tilesets (4, 64, ...) one metatile names a tile past the tile data; the autotile table never uses it, and it is drawn blank.

## Autotile table (`cex`)

`table[terrain * 768 + mask * 3 + variant]` is a metatile number; 48 more bytes follow (unknown).

- terrain 0, 1, 2: wall, secondary (water or lava), floor `[hypothesis from the pictures]`;
- mask: a bit is set when that neighbour has the same terrain; order S, SE, E, NE, N, NW, W, SW `[hypothesis]` (single-bit masks extend the blob toward one side; E and W not told apart yet);
- variant 0–2: alternative pictures.

## Palette animation (`canm`)

The content header is a table of pointers, one per animated color slot (0 = not animated). Each record: `u16` color count, `u16` duration in game frames, then the colors as RGBX.

## Resource IDs and the dungeon link

`NN` is the tileset number. Each floor picks it through `FloorProperties.tileset` (byte 2) in `mapparam` ([`data.md`](data.md)).

## Unknown fields

- The 48 bytes after the autotile table.
- Which palette and color each `canm` slot animates.
- The E and W bits of the neighbour mask.
