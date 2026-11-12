# Layout case 10: GenerateOuterRoomsFloor

**Date:** 2026-11-12
**Status:** [blue-verified] static (decompile and disassembly); the row link-byte sets are medium confidence; no emulator hash or RNG count was run for this layout
**Subsystem:** dungeon floor generation (layout case 10)

Issue: [#341](https://github.com/TorratDev/rust-mm/issues/341), parent [#111](https://github.com/TorratDev/rust-mm/issues/111). Parent notes: [`dungeon-generation.md`](dungeon-generation.md), [`layout-cases-4-6-7.md`](layout-cases-4-6-7.md).

## Addresses
| Address | Name | Source |
|---------|------|--------|
| `ov_0001::02150A10` | `GenerateOuterRoomsFloor(gridX, gridY, props)` (case 10) | [blue-verified] decompile |
| `ov_0001::02151E7C` | `GenerateFloor`; draws the grid size, then calls case 10 and sets the secondary-terrain flag | [blue-verified] decompile |
| `ov_0001::0214FEB0` / `0214FD7C` | grid positions (step 1) / `InitDungeonGrid` (step 2) | [blue-verified] |
| `ov_0001::0214F7A8` | `CreateRoomsAndAnchors` (step 4) | [blue-verified] |
| `ov_0001::0214E824` | create hallways (step 6), hallway flag 0 | [blue-verified] disassembly |
| `ov_0001::0214DB54`, `0214CF2C`, `0214D448`, `0214D190`, `0214FF30`, `0214E3DC`, `0214F6F0` | steps 7 to 13 | [blue-verified] call sites |

## What it does
- Grid size comes from the `GenerateFloor` draw (gx 2 to 6, gy 2 to 4); the function itself makes no RNG call. [blue-verified]
- Step 1 stepwise bounds, step 2 mode 0. All cells hold a room; interior cells (`x` in `1..gx-2`, `y` in `1..gy-2`) get invalid (`+0x08` = 1). [blue-verified]
- Step 4 on the ring cells only (invalid cells are skipped). No step 3 and no step 5. [blue-verified]
- Link bytes by hand: rows `y = 0` and `y = gy-1` give East and West to cells `x = 1..gx-2`; columns `x = 0` and `x = gx-1` give North to `y = 1..gy-2` and South to `y = 0..gy-3`. The pair `(gy-2, gy-1)` in the columns gets none; step 7 joins it. [blue-verified] for the column loop (disassembly), [hypothesis] for the row sets (Blue decompile only, medium confidence).
- Then step 6 (flag 0), steps 7 to 13 in order; the secondary-terrain flag is set. [blue-verified]

## Rust
`floor::outer_rooms_grid`, `floor::link_outer_rooms`, `PortedSteps::outer_rooms` (shares `standard_tail` with the standard path). Not run against the oracle. The trace-harness `PORTED_LAYOUTS` list still holds only layout 1 and needs layout 10 added before a scenario can use it.
