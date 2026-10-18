# Layout case 3: outer ring floor

**Date:** 2026-10-18
**Status:** hypothesis
**Subsystem:** dungeon floor generation (layout case 3)

Issue: [#148](https://github.com/TorratDev/rust-mm/issues/148). Parent notes: [`dungeon-generation.md`](dungeon-generation.md). Status is `hypothesis` because the RNG draw count and the room placements have not been checked on the retail ROM (`RESCUE_ROM` was unset, so no emulator run was possible).

Evidence: ghidra-cli decompile of `ov_0001::0215174C` and its callees, in the `rescue-arm9` project. The decompiler output is not committed here.

## Addresses
| Address | Name | Source |
|---------|------|--------|
| ov_0001::0215174C | GenerateOuterRingFloor | ghidra-cli decompile; name from [`symbols.md`](../symbols.md) |
| ov_0001::0214FD7C | InitDungeonGrid (std 2) | ghidra-cli decompile; called with a 6 × 4 grid |
| ov_0001::0214F6A0 | assign grid-cell connections (std 5) | ghidra-cli decompile |
| ov_0001::0214E824 | create hallways (std 6) | ghidra-cli decompile |
| ov_0001::0214DB54 | ensure the grid is connected (std 7) | ghidra-cli decompile |
| ov_0001::0214D448 | shop room (std 9) | ghidra-cli decompile; the argument is a value read through a global |
| ov_0001::0214D190 | monster house (std 10) | ghidra-cli decompile; the argument is a value read through a global |
| ov_0001::0214FF30 | extra hallways (std 11) | ghidra-cli decompile; last argument is `props[+0x13]` |
| ov_0001::0214E3DC | not named | ghidra-cli decompile; 2 `DungeonRandInt` call sites, purpose not read yet |
| ov_0001::0219D014 | DungeonRandRange | [`rng.md`](rng.md) |
| ov_0001::0219D058 | DungeonRandInt | [`rng.md`](rng.md) |

## Steps (as read in the decompile)
1. Grid setup: `InitDungeonGrid(grid, 6, 4)`, so case 3 uses a fixed 6 × 4 grid, not the grid size drawn by `GenerateFloor`. Cells are 30 bytes, with the column stride from [`dungeon-generation.md`](dungeon-generation.md), step 2.
2. Cell loop: 6 columns by 4 rows, 24 cells, in row order (`y` outer, `x` inner). Each cell is a room or an anchor, decided by the cell's "holds a room" byte (`+0x0A`):
   - **No room:** an anchor with room number `0xFF`. Two `DungeonRandRange` draws place it inside the cell's area.
   - **Room:** two `DungeonRandRange` draws give the rectangle's position and size, and two `DungeonRandInt` draws place it within the area. The room takes the next room index, counted from 0.
3. Flags: after the loop, a fixed set of cell byte offsets is set to 1. The offsets fall on the outer ring of the 6 × 4 grid and on a few others. Not yet mapped to named fields (inferred from the offsets).
4. `assign grid-cell connections` (`0214F6A0`), with the same arguments as the standard path.
5. `create hallways` (`0214E824`), with a pointer to the stack value that holds the dig list.
6. `ensure the grid is connected` (`0214DB54`).
7. Shop room (`0214D448`) and monster house (`0214D190`). Both take a value read through a global, not `props`. The standard path's chances come from `props +0x07` and `+0x08` (see [`dungeon-generation.md`](dungeon-generation.md)). The globals' values are not known.
8. Extra hallways (`0214FF30`) with `props[+0x13]` as the attempt count. This matches step 11 in [`dungeon-generation.md`](dungeon-generation.md).
9. `0214E3DC(grid, 6, 4)`: not read yet.

No maze step (std 8) and no finalising passes (`ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions`) appear in the body. Whether those run after this function is not checked.

## Counts
- Static call sites in the body: 4 `DungeonRandRange`, 2 `DungeonRandInt`. This matches the "4 / 2" in [`dungeon-generation.md`](dungeon-generation.md).
- Per cell: 2 `DungeonRandRange` in both branches, plus 2 `DungeonRandInt` in the room branch. Dynamic counts depend on how many cells hold a room, so they are not fixed by the static count.
- The body runs 24 cell iterations. A figure of 64 draws in the loop (from an earlier investigation) does not match this structure. It is not used here.
- Callees' dynamic counts (std 5 to 11) are in [`dungeon-generation.md`](dungeon-generation.md) for the standard path, not measured for this layout.

## Red comparison
`pmd-red` names this layout "outer ring" [red-hypothesis]. Blue's code uses the same 4 / 2 static call count (as the table says), but the emulator check that would confirm the layout and the draw count is not done.

## Open questions
- Emulator check: run a floor that takes layout 3 with `floor_stages.py` / `floor_oracle.py` and compare the RNG state after the cell loop and after each callee. Needs `RESCUE_ROM` and `py-desmume`.
- Name `0214E3DC` (2 Int draws) and find its role.
- Find the meaning of the two globals that feed the shop and monster-house calls (`DAT_02151bec`, `DAT_02151bf0`).
- Map the post-loop flag offsets to named cell fields.
- Confirm the mask constant (`DAT_0219d078`, assumed `0xFFFF`) and the cell stride literal.
- Check what follows this function: the finalising passes, and whether `GenerateFloor` runs them for this layout.

## Links
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
- Floor generation: [dungeon-generation.md](dungeon-generation.md)
- RNG: [rng.md](rng.md)
