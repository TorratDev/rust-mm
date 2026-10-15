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
| `GenerateFloor` up to `GenerateStandardFloor` | `ov_0001::02151E7C` | 11 | 7 | partly: the entry, grid size, layout switch and retry loops (`floor_gen::generate_floor`), checked for the forced seeds; ResetFloor, the layout generators and the spawns are behind `FloorSteps` |
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

## Prologue and retry loops in Rust

`rescue_sim::floor::plan_layout` covers the grid size draw and the layout switch (`GenerateFloor` steps 3 and 4). Its RNG calls are the grid draw and, for layouts 1 and 11, one `DungeonRandInt(2)` for the grid height `r + 2`. Forced seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D` reach the state recorded at `GenerateStandardFloor`'s entry, and seed `0x12345678` takes 7 RNG steps. Both match the table above.

`rescue_sim::floor_gen::generate_floor` runs the whole of `GenerateFloor` in the order of the disassembly below: the entry reset, the grid draw and layout switch, the fixed-room branch, both retry loops, the fallbacks and the final steps. The steps it calls that are not ported yet (ResetFloor's body, the layout generators, the spawns, the fixed-room placements) are behind the `FloorSteps` trait. The driver can stop at `GenerateStandardFloor`'s entry (`GenStop::StandardFloor`), which is the checkpoint the forced-seed tests use.

`attempt_has_rooms` and `stairs_check` are the two acceptance tests of the loops (`rescue_sim::floor`).

### Control flow (read from the overlay 1 disassembly)

The disassembly was made with Capstone on `rescue-re/code/overlay_0001.bin` (RAM base `0x02138860`). The addresses below are Blue's; the names of unnamed callees are not known.

- **Grid draw** (`0x021520CC`–`0x02152170`): draws `DungeonRandRange(2, 9)` then `(2, 8)`, or `(2, 5)` then `(2, 4)` when the layout byte is 8. Accepts when `grid_x < 7` and `grid_y < 5`. A counter starts at 32 and drops on each rejected draw; when it reaches 0 the grid is 4 × 4. So at most **32 draws**.
- **Layout switch** (`0x021521C0`): jump table on `layout & 0xF`. Nibbles 12–15 fall through to the standard generator (`b 0x2152250`), so they are not an error case.
- **Outer loop** (10 tries, register `sb`, `0x0215200C`–`0x021524B8`). At each try it sets the stairs pair A (`+0xC0B8`, `+0xC0BA`) and pair B (`+0xC0BC`, `+0xC0BE`) to `-1`. After the spawns it checks the stairs (see below); a failure starts the next try.
- **Inner loop** (10 tries, register `sl`, `0x02152044`–`0x021523D0`). At each try it:
  - stores `sl` in `+0x34B6` (the attempt index of the layout loop);
  - from try 2 on, clears `gFloorPropsByteC` (`021B45C0`);
  - clears the retry flag at `021B45A4`, sets the pair at `021B45C8` to `-1`, calls `ResetFloor`, sets pair A to `-1` and clears `+0x34A8`;
  - if `+0x34B4` (fixed room) is non-zero, calls `ProcessFixedRoom`. A non-zero result ends the inner loop; zero skips to the finalising steps without a layout;
  - otherwise runs the layout switch.
- **Finalising** (`0x021522EC`): `ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, then the retry flag at `021B45A4`. If it is set, the next inner try starts. Nothing in overlay 1 sets it (only `GenerateFloor` clears and reads it), so in overlay 1 it is always 0 and the check never fires.
- **Layout acceptance** (`0x02152304`–`0x021523C0`). Over all tiles (x 0..56, y 0..32): a tile counts when its terrain type is 1 (floor) and its room byte is at most `0xF0`. Those tiles are the room tiles (`fp`). Room bytes below 64 mark a distinct-room array (`rooms`). Accept when `fp >= 30` and `rooms >= 2`; otherwise the next inner try. This is `attempt_has_rooms`.
- **Inner fallback** (after try 10): pair `021B45C8` set to `-1`, `GenerateOneRoomMonsterHouseFloor`, `+0x34A8 = 1`.
- **Then**: `FinalizeJunctions`. `GenerateSecondaryTerrainFormations(1, props)` runs **only when the layout set a flag** (`[sp+0x10]`). The flag is set by the standard path (layouts 0, 8, 12–15), layout 1, layout 11, layouts 3, 4, 6 and 10, and not by 2, 5, 7 and 9. This differs from the step 5 entry above, which says the call is unconditional.
- **Spawns**: `DungeonRandInt(100) < props+0x19` gives `itemless`; `SpawnNonEnemies(props, itemless)`, `SpawnEnemies(props, itemless)`, `ResolveInvalidSpawns`.
- **Stairs check** (`0x0215244C`–`0x021524B0`, `stairs_check`): if pair A has a `-1` component, fail. If `GetFloorType() == 1` (`0x0206DB90`), accept. Otherwise, if pair B has a `-1` component, fail; else accept when `StairsAlwaysReachable(B.x, B.y, FALSE)` is non-zero.
- **Outer fallback** (after try 10): pair `021B45C8` = `-1`, `ResetFloor`, `GenerateOneRoomMonsterHouseFloor`, `+0x34A8 = 1`, `FinalizeJunctions`, `SpawnNonEnemies(props, 0)`, `SpawnEnemies(props, 0)`, `ResolveInvalidSpawns`.
- **End**: `+0x34AA` is set from `021B45CC` (`< 0` → 0, else `0x2149AF8(props)` and 1), then `0x2151D4C(0, 0, 56, 32)`, an optional call on `021B45A8`, and `CloseFile`.

### Still not done

- ResetFloor's body (`0x0214BDBC`): the driver calls it through `FloorSteps::reset_floor`. The body also clears the 8 × 8 cell records, a 64-entry pointer table and stairs pair B, and marks the walls next to the map edge. Not decoded in full yet.
- The fixed-room placements `0x0214A0D4` (numbers below 50) and `0x02150788` (from 50 on). They read the fixed-room file, which is game data and not in the repo, so they stay behind `FloorSteps`.
- Unnamed callees: `0x02179060` (called with the pair C position and `0x17C`), `0x02149AF8` (called with the props pointer), `0x0214C13C`, `0x02151D4C` (called with `0, 0, 56, 32`).
- The layout generators, the spawns, the stairs marker and `GenerateStandardFloor` steps 5 and up: their own issues. The driver calls them through `FloorSteps`.
- Checks against the retail ROM for the natural seed (no ROM in this environment; the forced seeds are checked).

## Open questions

- Each generator's internal RNG order (sub-functions such as room and corridor placement).
- Write the reimplementation (`crates/sim`) and test it against `floor_oracle.py`.
