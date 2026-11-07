# Dungeon tilesets (`dungeon.sbin` `bNN*`)

Issue: [#35](https://github.com/TorratDev/rust-mm/issues/35). Autotile findings: #60 (P3). Code: `crates/formats/src/tileset.rs`. Tests: unit tests; `tests/tests/formats_rom.rs` (`tilesets_decode`), `tests/tests/viewer_rom.rs`. Viewer: `dungeon.sbin` → any `bNN*` part.

**Verification status:** file layout verified against Blue (2026-10-16): 76 tilesets; all load, all metatiles render, every autotile entry names an existing metatile. Tileset 0 checked by eye (a water and sand cave). The autotile rules below are a static reading of the Blue ARM9 and overlays (Ghidra, #60); they have not been run in the emulator. See [Not verified](#not-verified).

## Parts of tileset `NN`

| Part | Format | Contents |
|------|--------|----------|
| `bNNfon` | AT4PX | 4bpp 8x8 tiles |
| `bNNcel` | AT4PX | metatiles: 9 tilemap entries (3x3 tiles, 24x24 pixels), 18 bytes each; entry = tile bits 0–9, hflip bit 10, vflip bit 11, palette bits 12–15 |
| `bNNpal` | raw | 12 palettes x 16 RGBX colors |
| `bNNcex` | SIR0 + AT4PX | autotile table, 2352 bytes (0x930) |
| `bNNcanm` | SIR0 | palette animation |
| `bNNemap0` | SIR0 + AT4PX | fixed 24x24 metatile map, 576 bytes decoded (0x240); tilesets 64–75 only |

Tilesets 22, 26, 27, 34, 38, 57, 58 and 61 have only `pal` and `canm` (palette variants of other tilesets); their `fon`, `cel` and `cex` come from the remapped file number (see [Tileset file remap](#tileset-file-remap)). Tilesets 64–75 have `emap0` instead of `cex`. In some tilesets (4, 64, ...) one metatile names a tile past the tile data; the autotile table never uses it, and it is drawn blank.

## Cell terrain

Each dungeon cell has a `u16` terrain value at cell+0. Its type is `terrain & 3` [blue-verified, `FUN_ov_0001__021492ec` (ov_0001::021492ec), same mask as `TERRAIN_TYPE_MASK` in `crates/sim/src/floor.rs`]:

| Type | Meaning |
|------|---------|
| 0 | wall |
| 1 | floor |
| 2 | secondary (water or lava) |
| 3 | both (secondary converted) |

## Autotile table (`cex`)

The loader `FUN_ov_0001__0214969C` (ov_0001::0214969c) reads the 2352-byte `cex` for tilesets below 0x40 into the dungeon buffer at `dungeon+0x109C2` [blue-verified]. The table is three blocks of 768 bytes (256 masks x 3 variants each). The game reads entry `index * 3 + variant`, where `index = block base | mask`:

| Block | Base | Cell type | Used for |
|-------|------|-----------|----------|
| 0 | 0x000 | 0 | wall cells |
| 1 | 0x100 | 2 and 3 | secondary and both cells (the same block; the mask rule differs, see below) |
| 2 | 0x200 | 1 | floor cells |

The index never exceeds 0x2FF, so the last byte read is 0x2FF·3+2 = 2303. The 48 bytes after that are never read ([Unknown fields](#unknown-fields)).

### Neighbour mask

The mask is 8 bits. For the cell at (x, y), y grows downward [blue-verified, `FUN_ov_0001__021492ec` and `FUN_ov_0001__02149a08` (ov_0001::02149a08)]:

| Bit | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|-----|---|---|---|---|---|---|---|---|
| Neighbour | S (x, y+1) | SE (x+1, y+1) | E (x+1, y) | NE (x+1, y-1) | N (x, y-1) | NW (x-1, y-1) | W (x-1, y) | SW (x-1, y+1) |

E is bit 2 and W is bit 6. Each bit is set by a rule that depends on the cell's own type `t` and the neighbour's type `n`:

| Cell type `t` | Block | Bit set when |
|---------------|-------|--------------|
| 0 (wall) | 0 | `n == 0` |
| 2 (secondary) | 1 | `n == 2` |
| 3 (both) | 1 | `n == 3` |
| 1 (floor) | 2 | `n != 0` (floor, secondary or both count) |

So a secondary cell next to a "both" cell is an edge on both sides, and a floor cell treats secondary and both as floor-adjacent. The rule is not "same terrain" for floor.

**Out of bounds is a wall.** A neighbour with x < 0, y < 0, x ≥ 0x38 or y ≥ 0x20 is the out-of-bounds tile (`ov_0001::021ABDCC` or `ov_0001::021ABDE0`, chosen by `FUN_0206dbdc` from the tileset), whose terrain is 0 [blue-verified, static bytes]. So a wall cell at the edge gets the bit set, and a floor or secondary cell gets it cleared.

### Variant

`variant = RandInt(4)` [blue-verified, `FUN_02011968` = `((Rand32Bit() & 0xFFFF) * 4) >> 16`], then `3 → 0`. So variant 0 is picked with probability 1/2 and variants 1 and 2 with 1/4 each.

- The draw uses the **global** LCG (`sPRNGState`, 020DF704). The dungeon LCG (`gDungeonRngState`, ov_0001::021B3F58) is not touched.
- One draw is made for every cell recomputed, before the fixed-room check below. Each draw is two LCG steps.
- If `FUN_0206d49c` is true, the variant is forced to 0. It is true when the `s16` `fixedRoomNumber` at `dungeon+0x34B4` is in 1..0x31 [blue-verified]. pmd-red's name for this is `IsFloorwideFixedRoom` [red-hypothesis].
- The chosen metatile is stored at cell+4 and stays until that cell is recomputed. The renderer reads cell+4 and never consults the table while drawing.

A full pass (`FUN_ov_0001__021495e4`, ov_0001::021495e4) loops y 0..31 (outer) and x 0..55 (inner), so it makes 1792 draws. Other callers recompute one cell or a small neighbourhood after a change; each recomputed cell makes one draw [blue-verified, static].

### Metatile

For tilesets below 0x40:

```
index  = block base | mask
v      = variant (see above)
meta   = cex[index * 3 + v]
```

### Fixed maps (tilesets 64–75)

Tilesets 64–75 load `b%02demap0` (`emap0`) instead of `cex`, into the same buffer [blue-verified, `FUN_ov_0001__0214969C`]. It is a 24x24 metatile map, row-major with stride 24:

```
meta = emap0[y * 24 + x]   if x < 24 and y < 24
meta = 0                   otherwise
```

There is no neighbour logic, no variant and no RNG draw.

### Overlay copy (preview)

The ov_0000 copy of the loader (`FUN_ov_0000__0215869C`, ov_0000::0215869c) and its per-cell routine `FUN_ov_0000__02158354` use the same algorithm for the dungeon map preview [blue-verified, one investigator]. The preview always uses variant 0 and makes no RNG draw. This copy compares the mapped file number, not the raw tileset number.

## Tileset file remap

For `fon`, `cel` and `cex` the loader uses a file number `byte_table[tileset]` from the table at `0x02090268` (ARM9) [blue-verified, static]: the `ldrb` at `ov_0001::0214973c` and `ov_0001::02149798` runs before their `sprintf`. `emap0`, `pal` and `canm` use the raw tileset number: in the `>= 0x40` branch the `ldrsh` of `dungeon+0x34AE` at `ov_0001::02149784` goes unchanged into `sprintf("b%02demap0")` at `ov_0001::021497e4`, and `FUN_ov_0001__0214969C` passes the raw `s16` for `"b%02dpal"` (pool `ov_0001::021498c4`) and `"b%02dcanm"` (`ov_0001::021498e4`) [blue-verified, static]. The table is the identity except:

| Tileset | File |
|---------|------|
| 22 | 16 |
| 26 | 20 |
| 27 | 23 |
| 34 | 28 |
| 38 | 30 |
| 57 | 45 |
| 58 | 45 |
| 61 | 48 |

The `< 0x40` and `>= 0x40` checks in `FUN_ov_0001__021492ec` and `FUN_ov_0001__0214969C` use the raw tileset number (`s16` at `dungeon+0x34AE`).

## Palette animation (`canm`)

The content header is a table of pointers, one per animated color slot (0 = not animated). Each record: `u16` color count, `u16` duration in game frames, then the colors as RGBX.

## Resource IDs and the dungeon link

`NN` is the tileset number. Each floor picks it through `FloorProperties.tileset` (byte 2) in `mapparam` ([`data.md`](data.md)).

## Unknown fields

- The 48 bytes after the autotile table. The game never reads them: the only readers of the table are two `ldrb` at `ov_0001::0214936c` and `ov_0001::021495d0`, and they stop at index 2303. The 48 bytes are 16 triples (the repeated triple is usually of shape a,b,a). Their meaning is unknown; treat them as padding and do not model them.
- Which palette and color each `canm` slot animates.

## Not verified

- No emulator run (py-desmume). The stored cell bytes were not compared with a table lookup, and the RNG step count was not traced. A check: hook `ov_0001::0214936c` and read `sPRNGState` before and after.
- The ov_0000 preview copy was decompiled by one investigator only; the other did not read it.
- Which game events trigger the per-cell recomputes (callers `FUN_ov_0001__02147AD0`, `02147C38`, `02194BFC`, `021637FC`, `0215277C`, `02194570` are located but not classified).
- Decoded size of every `emap0` file: the 576-byte size was not confirmed for each file.
- The meaning of the 48 tail bytes.
