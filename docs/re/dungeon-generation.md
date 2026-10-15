# Dungeon floor generation

How Blue builds a floor, and the order of its RNG calls. Issue: [#48](https://github.com/TorratDev/rust-mm/issues/48). Names, addresses and tags are in [`symbols.md`](../symbols.md). The code is in overlay 1 and matches `pmd-red`'s `src/dungeon_generation.c` function for function. The inputs and outputs are checked by emulator on the retail ROM (see below); the internal steps are read in code only.

## Entry point

`GenerateFloor` (`ov_0001::02151E7C`) is the only function in overlay 1 that calls `DungeonRandRange` exactly 4 times and `DungeonRandInt` exactly 3 times, like Red's. Its 40-odd calls come in Red's order, with the same argument counts. It runs after `RunDungeon_Async` has seeded the dungeon RNG for the floor ([`rng.md`](rng.md)), so every RNG call below comes from the per-floor seed.

## Steps

1. Open the fixed-room file (`OpenFileAndGetFileDataPtr("fixedmap", 4)`; the handle is kept at dungeon `+0x11304`) and `ResetFloor`.
2. `ProcessFixedRoom(fixedRoomNumber, floorProps)`: if the floor is one fixed room, generation ends here.
3. Grid size: `DungeonRandRange(2, 9)` and `DungeonRandRange(2, 8)`, or `DungeonRandRange(2, 5)` and `DungeonRandRange(2, 4)` when the layout byte is exactly 8. The pair is drawn again while `gridX >= 7` or `gridY >= 5`, at most 32 times, then falls back to 4 × 4. A size whose cells would be under 8 tiles (`56 / gridX` or `32 / gridY`) becomes 1. Each cell is the 56 × 32 map divided by the grid size.
4. Layout switch on `layout & 0xF`, the same enum as Red's:

| Case | Layout | Blue function | RNG calls inside (Range / Int) |
|------|--------|---------------|--------------------------------|
| 0, 8 | large | `GenerateStandardFloor(gridX, gridY, props)` | 0 / 0 |
| 1 | small | `DungeonRandInt(2)`, then `GenerateStandardFloor(4, r + 2, props)` | 0 / 0 |
| 2 | one room, monster house | `GenerateOneRoomMonsterHouseFloor` | 0 / 0 |
| 3 | outer ring | `GenerateOuterRingFloor` | 4 / 2 |
| 4 | crossroads | `GenerateCrossroadsFloor` | 4 / 2 |
| 5 | two rooms, monster house | `GenerateTwoRoomsWithMonsterHouseFloor` | 2 / 2 |
| 6 | line | `GenerateLineFloor` | 0 / 0 |
| 7 | cross | `GenerateCrossFloor` | 0 / 0 |
| 9 | beetle | `GenerateBeetleFloor` | 0 / 0 |
| 10 | outer rooms | `GenerateOuterRoomsFloor(gridX, gridY, props)` | 0 / 0 |
| 11 | medium | `DungeonRandInt(2)`, then `GenerateStandardFloor(4, r + 2, props)` | 0 / 0 |

   The RNG counts are the direct calls in each function, and they are the same in Blue and Red (the functions they call make more).
5. `ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions`, `GenerateSecondaryTerrainFormations(1, props)` (3 / 9 direct RNG calls).
6. `DungeonRandInt(100) < itemlessMonsterHouseChance` decides an itemless monster house.
7. `SpawnNonEnemies(props, itemless)` (4 / 7), `SpawnEnemies(props, itemless)` (2 / 2), `ResolveInvalidSpawns`.
8. `GetFloorType` and `StairsAlwaysReachable(x, y, FALSE)`: if the stairs cannot be reached, `ResetFloor` and fall back to `GenerateOneRoomMonsterHouseFloor`, then finalise and spawn again (same as Red).
9. Final calls (`sub_806C330`, `sub_8051654`, `sub_804B534(0, 0, 56, 32)`, `sub_804FC74`), then `CloseFile`.

## Emulator check

[`verification/v7-trace/floor_oracle.py`](../../verification/v7-trace/floor_oracle.py) replays the retail ROM to the first dungeon floor, then for each case forces the dungeon RNG state at `GenerateFloor`'s entry (and optionally the global RNG state) and records the generated floor at the first turn: a layout hash, per-field hashes of the tile grid, stairs, room count, terrain counts, and the number of dungeon RNG steps used. It stores hashes and counts only.

Results (2026-10-14, 5 seeds × 3 global RNG values, plus the floor's natural seed):

- **The floor is a function of the dungeon RNG state at `GenerateFloor`'s entry.** For each seed, terrain, spawn/visibility flags, rooms, walkability, `unkE`, monster placement and item placement are identical whatever the global RNG state; every seed gives a different layout; the same seed always gives the same floor and uses the same number of RNG steps (1,225 to 2,029 in these cases).
- **One tile byte follows the global RNG:** `+0x04` (Red `unk8`) is a graphics variant (an index into the tileset's cell table), which Red picks with the global `RandInt(4)` in `sub_80498A8`. It is cosmetic; the layout hash zeroes it.
- **The natural seed reproduces the real run:** forcing the seed the game picks itself gives the same floor (stairs `(23, 8)`, 3 rooms, 1,802 RNG steps) as the unforced replay.

So the generator's inputs are the dungeon RNG state and the floor properties. A reimplementation can be tested against `floor_oracle.py`: generate a floor from the same seed and compare the layout hash, stairs and RNG step count. The reimplementation must be original code written from this description and our own analysis of Blue, not a translation of `pmd-red`'s source ([`legal.md`](../legal.md)).

## Reimplementation, step by step

The Rust generator lives in `rescue_sim::floor` (with `rescue_sim::rng`). It is written from this page and our own reading of Blue's code, and checked against [`verification/v7-trace/floor_stages.py`](../../verification/v7-trace/floor_stages.py), which records the dungeon RNG state and the arguments at the entry of every generation step on the retail ROM. Between each pair of checkpoints, the reimplementation must make the same number of RNG calls.

RNG calls per step on the first floor of a new game (small layout, a 4 × 2 grid), for the game's own seed and for a forced seed `0x12345678`:

| Step | Blue address | Natural seed | Seed `0x12345678` | Status in Rust |
|------|--------------|-------------:|------------------:|----------------|
| `GenerateFloor` up to `GenerateStandardFloor` | `ov_0001::02151E7C` | 11 | 7 | |
| std 1: grid bounds (`i × (56 / gridX)`, `i × (32 / gridY)`) | `ov_0001::0214FEB0` | 0 | 0 | done (`grid_bounds`) |
| std 2 `InitDungeonGrid`: clear the 30-byte cells; small and medium layouts mark columns `x >= gridX / 2` or `x >= gridX × 3 / 4` invalid | `ov_0001::0214FD7C` | 0 | 0 | done (`Grid::new`) |
| std 3 `AssignRooms` (room density as last argument) | `ov_0001::0214FB4C` | 129 | 129 | done (`assign_rooms`) |
| std 4 `CreateRoomsAndAnchors` | `ov_0001::0214F7A8` | 17 | 20 | done (`place_rooms`) |
| std 5 | `ov_0001::0214F6A0` | 33 | 33 | |
| std 6 | `ov_0001::0214E824` | 19 | 18 | |
| std 7 | `ov_0001::0214DB54` | 0 | 4 | |
| std 8–10 | `ov_0001::0214CF2C`, `0214D448`, `0214D190` | 0 | 0 | |
| std 11 (last argument: floor property `+0x13`, probably the number of extra hallways) | `ov_0001::0214FF30` | 16 | 31 | |
| std 12, std 13, `ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions` | | 0 | 0 | |
| `GenerateSecondaryTerrainFormations` | `ov_0001::0214AAF8` | 1 | 1 | |
| `SpawnNonEnemies` | `ov_0001::0214B650` | 788 | 760 | |
| `SpawnEnemies` | `ov_0001::0214B338` | 766 | 738 | |
| `ResolveInvalidSpawns` | `ov_0001::0214C1A8` | 0 | 0 | |
| `StairsAlwaysReachable` up to the first turn | `ov_0001::0214A760` | 22 | 20 | |
| **Total** | | **1,802** | **1,761** | |

Each count is the number of RNG steps between the entry of that step and the entry of the next one.

What steps 2 to 4 do, as read in Blue's code (the Rust tests start from the measured state at step 3's entry and reach the measured states at steps 4 and 5 for the natural seed and for forced seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D`; the last one gives a 4 × 3 grid):

- **Grid.** The cells live in a `[gridX][15]` array of 30-byte records (column stride `0x1C2`). Bytes `+0`–`+7` are the room bounds (start x, start y, end x, end y, 16-bit, ends excluded), `+8` "invalid", `+0xA` "holds a room". Step 2 clears the record, sets "holds a room" and sets "invalid" for the unused columns. Which columns are unused comes from a byte that `GenerateFloor` sets to 0, 1 (small layout) or 2 (medium layout) (`gStandardGridMode` in [`symbols.md`](../symbols.md)).
- **Step 3.** One `DungeonRandInt(3)`; the target is `−density` rooms when the density is negative, else `density` plus that value. A 256-entry list starts with that many "room" entries and gets 64 swaps of two `DungeonRandInt(gridX × gridY)` positions (128 calls). Valid cells then take entries in column order (x outer, y inner). With an odd `gridX`, the cell at row 1 of column `(gridX − 1) / 2` never keeps its room, but it is still counted. If fewer than two rooms were counted, valid cells are visited again in column order, each becoming a room when `DungeonRandInt(100) < 60`, until one does (at most 200 passes), and a byte at `ov_0001::021B4584` is cleared.
- **Step 4.** Cells are visited row by row (y outer, x inner). Each valid cell's area starts 2 tiles after the cell's start and is 4 tiles narrower and 3 tiles shorter than the cell. A cell without a room gets an anchor: one floor tile with room number `0xFE`, at `DungeonRandRange` x and y kept 2 tiles (1 on the map's left and top grid edge) from the area's start and 4 tiles (2 on the right and bottom edge) from its end. A room takes `DungeonRandRange(5, width)` × `DungeonRandRange(4, height)`, rounded up to odd sizes when the result stays below the area's size, then limited to 3 / 2 of the other side (width first), and is placed at `DungeonRandInt` offsets in the area. Its tiles become floor (terrain type 1) with the room index (counted from 0 in visiting order). Then `DungeonRandInt(100) < 80` sets byte `+0x1D` when floor-properties byte `+0x0C` is non-zero, and bit 2 of floor-properties byte `+0x0D` sets byte `+0x1C`; when both would be set, one more `DungeonRandInt(100)` keeps `+0x1C` on 50 or more and `+0x1D` otherwise. So a room costs 5 or 6 RNG calls and an anchor 2: 3 rooms and an anchor (17) for the natural seed, 4 rooms (20) for the forced one. The floor reset (outer ring of tiles impassable, rooms cleared) and the tile layout are also in `rescue_sim::floor`.

## Dungeon struct offsets seen here

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x5B8` | `unk5C0` (set to −1 per floor) | `0x5C0` | `RunDungeon_Async` |
| `+0x3360` | `deoxysForm` (from `DungeonRandInt(4)`) | `0x3800` | `RunDungeon_Async` |
| `+0x34AE`, `+0x34B0`, `+0x34B4` | `tileset`, `unk3A10` (music), `fixedRoomNumber` | `0x3A0E`, `0x3A10`, `0x3A14` | `RunDungeon_Async` |
| `+0x11304` | fixed-room file handle | — | `GenerateFloor` |
| `+0x19DF6` | `unk1BDD4.unk1C05E` | `0x1C05E` | `RunDungeon_Async` |
| `+0x1A2F8`, `+0x1A2F9`, `+0x1A308` | `floorProperties.tileset`, `.bgMusic`, `.fixedRoomNumber` | `0x1C576`, `0x1C577`, `0x1C588` | `RunDungeon_Async` |

The gap grows from −8 at `0x5C0` to −0x4A0 at `0x3800` and −0x560 at `0x3A0E`; inside `FloorProperties`, Blue has 2 bytes fewer between `bgMusic` and `fixedRoomNumber`.

## Open questions

- Each generator's internal RNG order (sub-functions such as room and corridor placement).
- Write the reimplementation (`crates/sim`) and test it against `floor_oracle.py`.
