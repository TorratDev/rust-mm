# Effects (`effect.sbin`)

Issue: [#33](https://github.com/TorratDev/rust-mm/issues/33) (the sprite layout is shared). Code: `crates/formats/src/effect.rs`, which reuses `sprite.rs` for pieces and animation steps. Tests: unit tests; `tests/tests/formats_rom.rs` (`effects_decode`); `tests/tests/viewer_rom.rs`. Viewer: `effect.sbin` → `efobNNN` (sprite playback), `efbgNNN` (tile sheet and frame list). Index: formats `effect object`, `effect background`.

**Verification status:** verified against Blue (2026-10-16): all 139 `efob` objects and 8 `efbg` backgrounds parse; every pose of every object renders; a rendered object shows a coherent shape (one checked by eye). Frame and tilemap meanings of backgrounds are open.

The layout follows Red's `dump_effect_sbin.py` in the `pmd-red` reference (`EfoFileData`, `EfbFileData`, `ax_pose`, `ax_anim`). The pose table length, which Red's script derives from neighbouring addresses, is confirmed here by the animation blocks.

## Resource IDs

Each `efobNNN` and `efbgNNN` file is a SIR0 sub-file of `effect.sbin` ([`archive.md`](archive.md)). The number is the file's index. Which effect the game plays for which event is not mapped.

## `efob`: effect objects

The content header is eight `u32`:

| Offset | Size | Field |
|--------|------|-------|
| `0x00` | 4 | pose table pointer |
| `0x04` | 4 | animation table pointer |
| `0x08` | 4 | animation count |
| `0x0C` | 4 | sprite data pointer, always 0 |
| `0x10` | 4 | positions pointer, always 0 |
| `0x14` | 4 | tile data pointer (4bpp) |
| `0x18` | 4 | palette pointer (16 RGBX colors, 64 bytes) |
| `0x1C` | 4 | tile count |

- **Pose table.** Pointers to pose arrays, with no stored length. The table ends at the first later table: the animation blocks (below), the animation table, the tile data, the palette or the header. Null entries are empty poses. A pose is a list of 10-byte pieces, the same as creature sprite frames ([`sprites.md`](sprites.md)), ending at the piece with the last-piece flag. Every piece has image `-1`: it draws tiles already in the object's own tile data.
- **Animation table.** `animation count` pointers. Each points to a block of 8 pointers (one per facing direction, 32 bytes), and each direction points to steps of 12 bytes ended by a zero duration, as in creature sprites. Every step names a pose that exists.
- **Tiles and palette.** `tile count` 4bpp tiles, and one 16-color RGBX palette for all pieces.
- Effect objects have no per-frame reference points, so `Sprite::offsets` is empty.

## `efbg`: effect backgrounds

The content header is five `u32`:

| Offset | Size | Field |
|--------|------|-------|
| `0x00` | 4 | frame count |
| `0x04` | 4 | frame table pointer |
| `0x08` | 4 | tile count minus one |
| `0x0C` | 4 | tile data pointer (4bpp, `tile count + 1` tiles) |
| `0x10` | 4 | palette pointer (256 RGBX colors, 1024 bytes) |

The palette sits 1024 bytes before the tiles in every file. Each frame starts with 10 `u16` values, then tilemap entries in the `gfx::MapEntry` layout (tile index bits 0–9, flips, palette bank bits 12–15). A frame ends where the next table or frame starts; the length is not stored.

## Unknown fields

- Which animation group is which effect, and the direction order.
- Step `flags` and the second point (`shadow`), as for sprites ([`sprites.md`](sprites.md#unknown-fields)).
- The piece field `+2` (as for sprites) and the object's palette bank.
- Background frame header: `[0]` steps by 33 from frame to frame in every background (a horizontal offset `[hypothesis]`); `[2]` and `[3]` are 33 and 27 in every frame shown (a map size `[hypothesis]`); `[4]` and `[5..9]` are not identified.
- Background maps: in the first six frames, `efbg000`, `efbg001`, `efbg003`, `efbg005` and `efbg006` hold 891 entries (33 × 27), but `efbg002` and `efbg004` hold 3–21 and `efbg007` 334–725. Whether short maps are partial updates or sparse maps is open. Frames are not rendered yet.
- Which palette bank each background tile uses.
