# Layout cases 4, 6 and 7: crossroads, line and cross floors

**Date:** 2026-11-12
**Status:** blue-verified (static, and a run for case 4)
**Subsystem:** dungeon floor generation (layout cases 4, 6, 7)

Issue: [#339](https://github.com/TorratDev/rust-mm/issues/339). Parent notes: [`dungeon-generation.md`](dungeon-generation.md), [`outer-ring-floor.md`](outer-ring-floor.md) (case 3, the pattern these follow).

Evidence: ghidra-cli decompiles of the three functions in the `rescue-arm9` project (not committed). Investigator's floor-stage runs with seed `0x12345678` for layouts 4, 6 and 7, and seeds `0xCAFEBABF` and `0x0BADF00D` for layout 4 only. Layouts 6 and 7 have been run on one seed only.

## Addresses
| Address | Name | Source |
|---------|------|--------|
| `ov_0001::021512B4` | `GenerateCrossroadsFloor` (case 4) | [blue-verified] decompile |
| `ov_0001::02151160` | `GenerateLineFloor` (case 6) | [blue-verified] decompile |
| `ov_0001::02150FC0` | `GenerateCrossFloor` (case 7) | [blue-verified] decompile |
| `ov_0001::0214F6A0` | assign grid-cell connections (step 5) | [blue-verified] call site |
| `ov_0001::0214E824` | create hallways (step 6); 6th argument is the hallway flag | [blue-verified] call site |
| `ov_0001::0214DB54` | ensure grid connected (step 7) | [blue-verified] call site |
| `ov_0001::0214D448` / `0214D190` | shop / monster house (steps 9, 10) | [blue-verified] call site |
| `ov_0001::0214FF30` | extra hallways (step 11) | [blue-verified] call site |
| `ov_0001::0214E3DC` | room imperfections (step 12) | [blue-verified] call site |

## Grids and bounds
- Case 4: `InitDungeonGrid(5, 4)`. `xb = {0, 11, 22, 33, 44, 56}`, `yb = {1, 9, 16, 23, 31}`. [blue-verified]
- Case 6: `InitDungeonGrid(5, 1)`. `xb = {0, 11, 22, 33, 44, 56}`, `yb = {4, 15}`. [blue-verified]
- Case 7: `InitDungeonGrid(3, 3)`. `xb = {11, 22, 33, 44}`, `yb = {2, 11, 20, 30}`. [blue-verified]

Cell stride: `x * 0x1C2 + y * 0x1E`. Field offsets used: `+0x08` invalid, `+0x0A` holds a room, `+0x13` north, `+0x14` south, `+0x15` west, `+0x16` east. [blue-verified]

## Case 4 (crossroads)
- Corners `(0,0)`, `(4,0)`, `(0,3)`, `(4,3)` are invalid (`+0x08 = 1`). Inner cells `x 1..3, y 1..2` are anchors (`+0x0A = 0`). The other 10 valid cells are rooms.
- Cell loop, row order. Anchor: `x = Range(xb+1, xb+sx-2)`, `y = Range(yb+1, yb+sy-2)`, room byte `0xFF`. Room: `w = Range(5, sx-3)`, `h = Range(4, sy-3)`, `x = xb+2+Int(sx-3-w)`, `y = yb+2+Int(sy-3-h)`. Same as case 3. [blue-verified]
- Links set by hand, no standard connection step: `South` of `(x,y)` and `North` of `(x,y+1)` for `x 1..3`, `y 0..2` (18 bytes); `East` of `(x,y)` and `West` of `(x+1,y)` for `y 1..2`, `x 0..3` (16 bytes).
- Then create hallways with the flag set to 1, ensure connected, shop, monster house, extra hallways, imperfections. No maze step, no step 13. [blue-verified]
- Draw count: 6 anchors × 2, 4 rooms in the middle rows × 3 (their `Range(4,4)` height draws nothing), 6 other rooms × 4 = 48. [run] on seeds `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`.

## Case 6 (line)
- `InitDungeonGrid(5, 1)`, then step 3 (`AssignRooms(props+1)`), step 4 (`CreateRoomsAndAnchors`, props `+0x0D`), step 5 (connections, props `+0x05`, `+0x0B`). Unlike the standard path, step 3's return value is not checked, so `byte_4584` is not cleared. [blue-verified]
- Then the tail as for case 4. Run on one seed only: std3 129 steps, std4 25, std5 33, std6 14, std11 39, others 0. [run, one seed]

## Case 7 (cross)
- `InitDungeonGrid(3, 3)`. All cells are rooms; the four corners are invalid. Step 4 only: no step 3, no step 5.
- Eight link bytes set by hand after step 4: `(0,1).East`, `(1,1).West`, `(1,1).East`, `(2,1).West`, `(1,0).South`, `(1,1).North`, `(1,1).South`, `(1,2).North`. [blue-verified]
- Then the tail as for case 4. The secondary-terrain flag is not set for case 7. [blue-verified static; run checkpoint for layout 7, one seed]

## Shared
- Hallway flag: 1 for cases 4, 6 and 7 (the 5% random pass is skipped). [blue-verified static; stack word 1 at entry, run]
- Secondary-terrain flag: set after cases 4 and 6, not after case 7. [blue-verified]
- Case 4 and case 3 share the cell loop. The Rust port (`place_flagged_cells` in `crates/sim/src/floor.rs`) serves both.

## Red comparison
- [red-hypothesis] pmd-red's `dungeon_generation.c` uses the same grids, bounds, invalid corners and call order, and passes TRUE (disableRoomMerging) to the connection step for these layouts. Not checked against Blue here.

## Port check (#339, 2026-11-12)
- The Rust ports were compared with this note by reading the code: the three grids and bounds, the invalid corners, the anchor and room draw formulas, the hand-set links (18 and 16 bytes for case 4, 8 for case 7), the hallway flag, and the secondary-terrain flag. No mismatch was found, and the code needed no change.
- `cargo test -p rescue-sim` passes (93 tests), including the case 4 link and draw-count tests and the case 7 link test. There is no test that checks the case 6 bounds or links beyond the no-panic run.
- Whole-floor oracle rows: none added. This note records no whole-floor oracle values (steps, layout hash, stairs, rooms, terrain) for layouts 4, 6 or 7, so the table `ORACLE_FLOORS` has no rows for them.
- **Layouts lacking a whole-floor oracle run: 4 (crossroads), 6 (line) and 7 (cross), on every seed.** Case 4 has per-draw counts from runs on three seeds; cases 6 and 7 have per-stage counts on one seed. None of these are oracle totals.

## Open questions
- The whole-floor oracle run for layouts 4, 6 and 7 on seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D`: the step count to the return, the layout hash, the stairs, the room count and the terrain counts. Not run. Until it is, #339 stays open.
- The Rust oracle check (`RESCUE_ROM`) was not run from this worktree: there is no `rescue.toml` here.

## Links
- Symbols: [symbols.md](../symbols.md)
