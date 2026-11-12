# Layout case 3: outer ring floor

**Date:** 2026-10-18
**Status:** partly confirmed (whole-floor match on the retail-ROM oracle run; per-step checkpoints not run)
**Subsystem:** dungeon floor generation (layout case 3)

Issue: [#148](https://github.com/TorratDev/rust-mm/issues/148). Parent notes: [`dungeon-generation.md`](dungeon-generation.md). The whole floor matches the oracle run in [Emulator check](#emulator-check-floor-oracle): the RNG step count to the return, the tile hash, the stairs, the room count and the terrain counts. The per-step RNG draw counts are not checked on the retail ROM (`RESCUE_ROM` was unset in the port session, so no emulator run was possible there).

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
1. Grid setup: `InitDungeonGrid(grid, 6, 4)`, so case 3 uses a fixed 6 × 4 grid, not the grid size drawn by `GenerateFloor`. `InitDungeonGrid` sets "holds a room" (`+0x0A`) on every cell.
2. Border flags (P1, [blue-verified static read]): before the loop, `GenerateOuterRingFloor` clears `+0x0A` on the top and bottom rows and on the left and right columns. The 16 border cells are anchors; the 8 inner cells (x 1..4, y 1..2) stay rooms.
3. Cell loop (P2, [blue-verified static read]): row order (`y` outer, `x` inner), with boundaries `xb = {0, 5, 16, 28, 39, 51, 56}` and `yb = {2, 7, 16, 25, 30}`. With `sx`, `sy` the cell's width and height:
   - **Anchor** (`+0x0A` = 0): 2 draws, x then y: `ax = RandRange(xb[x]+1, xb[x]+1+sx-3)`, `ay` likewise. The tile is open floor with room byte `0xFF`.
   - **Room** (`+0x0A` ≠ 0): 4 draws in this order: `w = RandRange(5, sx-3)`, `h = RandRange(4, sy-3)`, `rx = xb[x]+2+RandInt(sx-3-w)`, `ry = yb[y]+2+RandInt(sy-3-h)`. No odd rounding, no 3:2 clamp, no structure draws. Rooms are numbered 0 to 7 in loop order.
   - Total: 16 × 2 + 8 × 4 = 64 draws. The RNG state after the loop should be 64 steps past the entry state. This is not checked on the emulator yet (see Open questions).
4. Link bytes (P3, [blue-verified static read]): 32 stores of 1 to `+0x13`..`+0x16` close a ring over the 16 border cells. Each border cell has two links to its ring neighbours; interior cells get none here.
5. `assign grid-cell connections` (`0214F6A0`), with the same arguments as the standard path.
6. `create hallways` (`0214E824`), with `xb` and `yb` as the boundary arrays.
7. `ensure the grid is connected` (`0214DB54`), with `xb` and `yb`.
8. Shop room (`0214D448`) and monster house (`0214D190`). Each takes a chance read through a global: `*021B45B0` and `*021B45AC`. Those globals are copies of `props +0x07` and `+0x08`, which `GenerateFloor` writes at its start. So case 3 uses the same chances as the standard path's steps 9 and 10 (see [`dungeon-generation.md`](dungeon-generation.md)).
9. Extra hallways (`0214FF30`) with `props[+0x13]` as the attempt count. This matches step 11 in [`dungeon-generation.md`](dungeon-generation.md).
10. `0214E3DC(grid, 6, 4)`: step 12, room imperfections. Its conditions and the 60 % draw match [`dungeon-generation.md`](dungeon-generation.md), so it runs on this layout's rooms the same way.

No maze step (std 8) and no step 13 (secondary structures) run here. No finalising passes (`ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions`) appear in the body. Whether those run after this function is not checked.

The port (`crates/sim/src/floor.rs`, `place_outer_ring_rooms`, and `PortedSteps::outer_ring` in `floor_gen.rs`) follows steps 1 to 10. The static evidence came from an investigator's ghidra-cli read of the decompile, not from a second read here.

The shop and monster-house steps (8) use the `props` chances that `GenerateFloor` copies into the globals, as the port's `g.props_copy_07` and `g.props_copy_08` do.

## Counts
- Static call sites in the body: 4 `DungeonRandRange`, 2 `DungeonRandInt`. This matches the "4 / 2" in [`dungeon-generation.md`](dungeon-generation.md).
- Loop draws: 16 anchors × 2 + 8 rooms × 4 = 64. The count is from the cell-type table above, not from a trace.
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

Port check (#148, 2026-11-12): the row for layout 3, seed `0x12345678`, is in `ORACLE_FLOORS` (`crates/sim/src/floor.rs`, `whole_floors_match_the_floor_oracle`). Its values are the ones above: 1683 steps to the return, layout FNV-1a `c32e3bac3f2c7b7b`, stairs (32, 20), 8 rooms, terrain (1357, 435). The Rust port passes it. The check reuses the recorded values; the emulator was not run again in that session.

## Red comparison
`pmd-red` names this layout "outer ring" [red-hypothesis]. Blue's code uses the same 4 / 2 static call count (as the table says), but the emulator check that would confirm the layout and the draw count is not done.

## Open questions
- Per-step RNG checkpoints for layout 3: extend `floor_stages.py` to force the layout byte, then compare the RNG state after the cell loop and after each callee. Not run: this machine has no `RESCUE_ROM`. The whole-floor oracle run is done and the port matches it (see above).
- Check that the 64-draw count and the cell order match the emulator. The Rust unit tests check the structure and the count against a state comparison only. The whole-floor total (1683 steps) matches, which fits the 64 draws but does not check their order.
- Finalising passes: the whole-floor match covers the tail of `generate_floor` for this layout, so the port runs the passes the oracle run ran. Which pass runs in which order for case 3 is not checked on the emulator.
- The mask constant `0x0219D078` = `0xFFFF` and the cell stride `0x1C2` were read from the pool words, per the investigator's notes (`[blue-verified static read]`).

## Links
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
- Floor generation: [dungeon-generation.md](dungeon-generation.md)
- RNG: [rng.md](rng.md)
