# Layout cases 5 and 9: two rooms with a monster house, and the beetle floor

**Date:** 2026-11-12
**Status:** blue-verified (static decompile and stage runs on seeds `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`); whole-floor oracle rows for layouts 5 and 9 match on all three seeds (see [Emulator check](#emulator-check-floor-oracle))
**Subsystem:** dungeon floor generation (layout cases 5, 9)

Issue: [#340](https://github.com/TorratDev/rust-mm/issues/340), parent [#111](https://github.com/TorratDev/rust-mm/issues/111). Parent notes: [`dungeon-generation.md`](dungeon-generation.md), [`layout-cases-4-6-7.md`](layout-cases-4-6-7.md) (the tail these share).

Evidence: ghidra-cli decompiles in the `rescue-arm9-c` project (not committed). Stage checkpoints from the investigator's floor-stage runs. The oracle rows (steps, layoutFnv1a, stairs, rooms, terrain) are in `ORACLE_FLOORS` in `crates/sim/src/floor.rs`; the raw output is not in the repository.

## Addresses
| Address | Name | Source |
|---------|------|--------|
| `ov_0001::021504F4` | `GenerateTwoRoomsWithMonsterHouseFloor` (case 5) | [blue-verified] decompile; call site `0215229C` |
| `ov_0001::02150DFC` | `GenerateBeetleFloor` (case 9) | [blue-verified] decompile; call site `021522D0` |
| `ov_0001::02150CA8` | merge two vertically adjacent room cells (case 9 only; unnamed in Blue) | [blue-verified] decompile |
| `ov_0001::0214F7A8` | `CreateRoomsAndAnchors` (step 4) | [blue-verified] call site |
| `ov_0001::0214E824` | create hallways (step 6); 6th argument is the hallway flag | [blue-verified] call site |
| `ov_0001::0214DB54` | ensure grid connected (step 7) | [blue-verified] call site |
| `ov_0001::0214D448` / `0214D190` | shop / monster house (steps 9, 10) | [blue-verified] call site |
| `ov_0001::0214FF30` | extra hallways (step 11) | [blue-verified] call site |
| `ov_0001::0214E3DC` | room imperfections (step 12) | [blue-verified] call site |

Cell stride `x * 0x1C2 + y * 0x1E`. Cell shorts `+0`, `+2`, `+4`, `+6` are `x0`, `y0`, `x1`, `y1` (the ends are exclusive). `+0x0A` holds a room, `+0x15` west and `+0x16` east link bytes, `+0x12` and `+0x11` and `+0x0B` are the merge marks below. [blue-verified]

## Case 5 (two rooms with a monster house)
- `InitDungeonGrid(2, 1)`. `xb = {2, 0x1C, 0x36}`, `yb = {2, 0x1E}`. [blue-verified]
- No `CreateRoomsAndAnchors`. Each room, for `i = 0, 1` in order (room id `i`), makes four draws in this order: `w = Range(10, xb[i+1] - xb[i] - 3)` (max 23), `h = Range(16, yb1 - yb0 - 3)` (max 25), `x = xb[i] + 1 + Int(maxw - w)`, `y = yb0 + 1 + Int(maxh - h)`. Eight draws in all. The cell rectangle is `(x, y, x+w, y+h)` and `+0x0A = 1`. Tiles get the terrain bits `(t & 0xFFFC) | 1` and the room id in tile byte `+5`. [blue-verified static; [run] 8 steps from the entry to step 6 on all three seeds]
- Links by hand: `(0,0).East` (sets `+0x16` of cell 0 and `+0x15` of cell 1). [blue-verified]
- Then only step 6 (hallway flag **0**) and step 10 with chance **999** (the literal at `021506AC`, so the house is always attempted). No ensure-connected, shop, extra hallways, imperfections, maze or step 13. [blue-verified static; [run] step 6 to step 10 is 7 steps, step 10 to the reset is 129 steps]
- The monster-house flag is set when step 10 makes a house, as for case 2 (`0x021506B0`). Dungeon `+0x34A8` follows that flag. [medium: the findings say GenerateFloor sets +0x34A8 after case 5; the link to the flag is inferred from case 2]
- The secondary-terrain flag is not set (`GenerateSecondaryTerrainFormations` does not run). [run: no checkpoint on any case-5 run]
- [hypothesis] Hallway flag 0 keeps the 5 % random hallway pass of step 6 on. Only the step count was observed, not the pass.

## Case 9 (beetle, 3 × 3)
- `InitDungeonGrid(3, 3)`. `xb = {5, 15, 35, 50}`, `yb = {2, 11, 20, 30}`. All nine cells are rooms; none is invalid. [blue-verified]
- Step 4 `CreateRoomsAndAnchors(grid, 3, 3, xb, yb, props+0x0D)`, the same call as case 6. No step 3 or 5. [run] step 4 to step 6 is 45 steps on all three seeds; the function itself makes no direct draw.
- Links by hand for each row `y = 0..2`: `(0,y).East`, `(1,y).West`, `(1,y).East`, `(2,y).West` (12 bytes; offsets `0x16`, `0x1D7`, `0x1D8`, `0x399` for each row). [blue-verified]
- Step 6 `CreateHallways(grid, 3, 3, xb, yb, flag = 1)`. [blue-verified; [run] stack word 1, 24 steps to step 7]
- Two merges with `02150CA8(x = 1, y = 0, dy)`, for `dy = 1` and then `dy = 2`. Cell `A = (x, y)` and `B = (x, y+dy)`: the rectangle is `x0 = min(A.x0, B.x0)`, `x1 = max(A.x1, B.x1)`, `y0 = A.y0`, `y1 = B.y1`. The rectangle is floored with the room id of the tile at `(A.x0, A.y0)`. The rectangle is written into `A`, `A.+0x12 = 1`, `B.+0x12 = 1`, `B.+0x0B = 0`, `B.+0x11 = 1`. No randomness. The middle column becomes one tall room. [blue-verified static for the rectangle and the room id; medium for the meaning of `+0x0B`, `+0x11`, `+0x12`]
- Tail in order: step 7 `EnsureGridConnected(3, 3)`, step 9 shop (chance `*021B45B0`), step 10 monster house (chance `*021B45AC`), step 11 extra hallways (last argument `props+0x13`), step 12 imperfections. The globals are the same as case 4. No maze step and no step 13. [blue-verified]
- The secondary-terrain flag is not set, and `+0x34A8` is not set by this function. [blue-verified]
- [run] Steps 7, 9 and 10 draw nothing on the first floor (chances 0); step 11 to step 12 is 39, 32 or 31 steps for the three seeds; the reset after step 12 is 0.

## Emulator check (floor oracle)
- **Command:** `floor_oracle.py ROM OUT.jsonl --seeds 0x12345678,0xCAFEBABF,0x0BADF00D --layouts 5,9,10`, run from `verification/v7-trace/` with py-desmume (DeSmuME 0.9.12). `OUT.jsonl` is not committed. The same run covers layout 10 ([`layout-case-10.md`](layout-case-10.md#emulator-check-floor-oracle)).
- **ROM:** the probe copy of the supported dump, SHA-1 `6f01887356268de39085df01490347bbcc3aa330`.
- **Date:** 2026-11-12, values taken from the `generateEnd` record of each run (steps = `rngSteps` at `GenerateFloor`'s return; entry = the forced seed).
- **What is recorded:** hashes and counts only (step count, FNV-1a 64 of the tiles, stairs, room count, terrain counts). No grid, no raw output (see [`docs/legal.md`](../legal.md)).
- **Rows added** to `ORACLE_FLOORS` (`crates/sim/src/floor.rs`): layouts 5 and 9 on each of the three seeds, six rows here. They were copied from the generated row file by command, not retyped. `whole_floors_match_the_floor_oracle` passes on all of them with no change to any row value.

## Red comparison
- [red-hypothesis] The pmd-red layout for a two-room monster house and the beetle floor are not checked against this note. Only the Blue values above are used.

## Open questions
- The meaning of cell `+0x0B`, `+0x11` and `+0x12` written by `02150CA8`.
- Whether hallway flag 0 turns on the random pass in case 5 (not traced).
- The per-draw order inside `CreateRoomsAndAnchors` for case 9 (the port reuses the step-4 function, so this is not decomposed here).
- The whole-floor oracle check for layouts 5 and 9 is done (see Emulator check). The per-stage `floor_stages.py` runs with a forced layout byte are not repeated here.
- Retail floors: no retail floor uses layout 5. Layout 9 is used by 28 floors (mapparam scan, see the investigator notes).

## Links
- Symbols: [symbols.md](../symbols.md)
