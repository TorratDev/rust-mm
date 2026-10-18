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
| ov_0001::0214E3DC | room imperfections (std 12) | ghidra-cli decompile; conditions and the 60 % draw match [`dungeon-generation.md`](dungeon-generation.md), step 12 |
| ov_0001::021B45B0 | shop chance copy | ghidra-cli: literal `0x021525E4` points here; `GenerateFloor` copies `props +0x07` into it |
| ov_0001::021B45AC | monster-house chance copy | ghidra-cli: literal `0x021525E8` points here; `GenerateFloor` copies `props +0x08` into it |
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
7. Shop room (`0214D448`) and monster house (`0214D190`). Each takes a chance read through a global: `*021B45B0` and `*021B45AC`. Those globals are copies of `props +0x07` and `+0x08`, which `GenerateFloor` writes at its start. So case 3 uses the same chances as the standard path's steps 9 and 10 (see [`dungeon-generation.md`](dungeon-generation.md)).
8. Extra hallways (`0214FF30`) with `props[+0x13]` as the attempt count. This matches step 11 in [`dungeon-generation.md`](dungeon-generation.md).
9. `0214E3DC(grid, 6, 4)`: step 12, room imperfections. Its conditions and the 60 % draw match [`dungeon-generation.md`](dungeon-generation.md), so it runs on this layout's rooms the same way.

No maze step (std 8) and no finalising passes (`ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions`) appear in the body. Whether those run after this function is not checked.

## Counts
- Static call sites in the body: 4 `DungeonRandRange`, 2 `DungeonRandInt`. This matches the "4 / 2" in [`dungeon-generation.md`](dungeon-generation.md).
- Per cell: 2 `DungeonRandRange` in both branches, plus 2 `DungeonRandInt` in the room branch. Dynamic counts depend on how many cells hold a room, so they are not fixed by the static count.
- The body runs 24 cell iterations. A figure of 64 draws in the loop (from an earlier investigation) does not match this structure. It is not used here.
- Callees' dynamic counts (std 5 to 11) are in [`dungeon-generation.md`](dungeon-generation.md) for the standard path, not measured for this layout.

## Emulator check (floor oracle)
- Command: `floor_oracle.py ROM OUT.jsonl --seeds 0x12345678 --layouts 3`, run from `verification/v7-trace/` with py-desmume 0.0.9 in a scratch venv.
- ROM: the supported dump `BlueRescueTeamEur`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330`.
- Script: `floor_oracle.py`, SHA-1 `5515cabe4adadb664fd862227ace80e9c0d87acf`.
- Result for layout 3, seed `0x12345678` (`rngAtGenerate` 305419896):
  - RNG steps: 1705 from `GenerateFloor`'s entry to the first turn, 1683 to its return (`0x021525B4`).
  - Rooms: 8 (distinct room numbers, excluding `0xFF`). Stairs at (32, 20). Terrain counts: type 0: 1357, type 1: 435.
  - Layout FNV-1a 64 at the first turn: `13806b64f622e3c1`. At the return: `c32e3bac3f2c7b7b`.
  - Layout SHA-256 (prefix) at the first turn: `cb9d5f74c4d52865`.

What this run confirms: the floor builds with 8 rooms on the 6 × 4 grid, and it gives a whole-floor fixture for a reimplementation. What it does not confirm: the per-step draw count (4 Range / 2 Int static, and the dynamic count per cell). The oracle reports totals only; `floor_stages.py` has no option to force a layout, so per-step checkpoints for case 3 need that script extended.

## Red comparison
`pmd-red` names this layout "outer ring" [red-hypothesis]. Blue's code uses the same 4 / 2 static call count (as the table says), but the emulator check that would confirm the layout and the draw count is not done.

## Open questions
- Per-step RNG checkpoints for layout 3: extend `floor_stages.py` to force the layout byte, then compare the RNG state after the cell loop and after each callee. The whole-floor oracle run is done (see above).
- Map the post-loop flag offsets to named cell fields.
- Confirm the mask constant (`DAT_0219d078`, assumed `0xFFFF`) and the cell stride literal.
- Check what follows this function: the finalising passes, and whether `GenerateFloor` runs them for this layout.

## Links
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
- Floor generation: [dungeon-generation.md](dungeon-generation.md)
- RNG: [rng.md](rng.md)
