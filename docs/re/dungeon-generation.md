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
| 3 | outer ring | `GenerateOuterRingFloor` ([notes](outer-ring-floor.md)) | 4 / 2 |
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
9. Final calls, then `CloseFile`: Blue `0x02179060` (at `0x02152534`), the shop pass `0x02149AF8` (at `0x02152560`), `0x02151D4C` (at `0x02152588`) and `0x0214C13C` (at `0x0215259C`), in that order (see "End of `GenerateFloor`" below). Red: `sub_806C330`, `sub_8051654`, `sub_804B534(0, 0, 56, 32)`, `sub_804FC74`.

## ResetFloor (`ov_0001::0214BDBC`)

Read from the decompile of `rescue-arm9-a` and compared with `pmd-red`'s `ResetFloor` (static analysis; no emulator check was run for this section). The body calls only `GetTileMut` and `ResetTile`, and makes no RNG calls.

Order of operations:

1. Map tiles, x = 0 to 55 (outer loop) and y = 0 to 31 (inner loop), 1792 tiles. Each tile gets `ResetTile(GetTileMut(x, y))`. Then `terrainFlags |= 0x10` (impassable wall) when any of its 8 neighbours is outside the 56 × 32 map. That marks the map's own perimeter ring: the tiles with x = 0 or 55, or y = 0 or 31. Tiles next to the edge are not marked.
2. Stairs pair B: `+0xC0BC` and `+0xC0BE` are set to `0xFFFF` (−1).
3. The 8 × 8 fixed-room cell records (`unkE27C` in Red): `ResetTile` on each. The cell base is `DAT_0214C0A4` (`0xC11C`, see the pool-word table below). The stride is `0x14` per x and `0xA0` per y, so each cell is one 0x14-byte tile record.
4. Item count: `+0x3424` (u16) = 0.
5. Traps: for `i` = 0 to 63, the first 4 bytes of the entity that `traps[i]` points to (the pointer table at `+0x114B4`) are set to 0. There is no NULL check, so a NULL entry writes to address 0.

`ResetTile` (`ov_0001::0214C0A8`) writes the whole 0x14-byte record: `u16 +0` = 0, `u16 +2` = 0, `u8 +4` = 0, `u8 +5` = 0xFF, `u8 +6` to `+0xA` = 0, `u32 +0xC` = 0 and `u32 +0x10` = 0. The names of `+2` and `+6` to `+9` come from Red's `map.h`, not from Blue, so they are not verified.

**Red match.** `pmd-red`'s `ResetFloor` (`src/dungeon_generation.c:4000`) has the same steps in the same order, with the same results. The differences are in source shape only:

- Red tests `(x+1, y-1)` twice and not `(x+1, y)`; Blue tests `(x+1, y)` and not `(x+1, y-1)`. Both lie outside the map exactly when x = 55, so the set of walls is the same.
- Red's trap reset writes `type = 0` on an `Entity`, where `type` is a u32 at `+0` (`include/structs/dungeon_entity.h:336`). Blue's 4-byte zero is the same write.
- Red indexes the 8 × 8 cells as `[x][y]`; Blue's index is in the other order. Every cell is reset, so the order does not change the result.

The Rust `PortedSteps` (see the Prologue section) resets the map and stairs pair B only. The rest of this body is for the port in [#111](https://github.com/TorratDev/rust-mm/issues/111).

### Overlay 1 pool words (#179)

Five overlay 1 pool words, read from `rescue-re/code/overlay_0001.bin` at RAM base `0x02138860` by the reviewer. `ghidra-cli` `memory read` fails on overlay pool words, so this is the only source for them. Values: **[blue-verified]** (read from the file, one read so far). Meanings: **[hypothesis]**.

| Pool word | Value | Meaning |
|-----------|-------|---------|
| `DAT_0214C0A4` | `0xC11C` | [hypothesis] Confirms the 8 × 8 cell base `+0xC11C` |
| `DAT_02179124` | `0x584` | [hypothesis] The spawn-request list is at `gDungeon + 0x584`, not Red's `0x57C` |
| `DAT_02149E78` | `0x021B45CC` | [hypothesis] The shop bounds are the overlay global `021B45CC`, not `gDungeon + 0xC0E0..0xC0EC` |
| `DAT_021525F0` | `0x021B45CC` | [hypothesis] Same global |
| `DAT_021525FC` | `0x021B45C8` | [hypothesis] The shop-centre pair |

A second read of these values is still open (#179 P1), so the "Still not done" bullets below stay open.

## Emulator check

[`verification/v7-trace/floor_oracle.py`](../../verification/v7-trace/floor_oracle.py) replays the retail ROM to the first dungeon floor, then for each case forces the dungeon RNG state at `GenerateFloor`'s entry (and optionally the global RNG state) and records the generated floor at the first turn: a layout hash, per-field hashes of the tile grid, stairs, room count, terrain counts, and the number of dungeon RNG steps used. It stores hashes and counts only.

Results (2026-10-14, 5 seeds × 3 global RNG values, plus the floor's natural seed):

- **The floor is a function of the dungeon RNG state at `GenerateFloor`'s entry.** For each seed, terrain, spawn/visibility flags, rooms, walkability, `unkE`, monster placement and item placement are identical whatever the global RNG state; every seed gives a different layout; the same seed always gives the same floor and uses the same number of RNG steps (1,225 to 2,029 in these cases).
- **One tile byte follows the global RNG:** `+0x04` (Red `unk8`) is a graphics variant (an index into the tileset's cell table), which Red picks with the global `RandInt(4)` in `sub_80498A8`. It is cosmetic; the layout hash zeroes it.
- **The natural seed reproduces the real run:** forcing the seed the game picks itself gives the same floor (stairs `(23, 8)`, 3 rooms, 1,802 RNG steps) as the unforced replay.

### Whole-floor golden test

`floor_oracle.py` also records the same hashes and counts at `GenerateFloor`'s return (`0x021525B4`, `generateEnd`), where `rescue_sim::floor_gen::generate_floor` ends, and can force the floor-properties layout byte (`--layouts`, written at dungeon `+0x1A2F6`, the properties `SpawnNonEnemies` gets in `r0`, at `GenerateFloor`'s entry). Its `layoutFnv1a`, the same as `rescue_sim::floor::layout_fnv1a`, is FNV-1a 64 over the first 12 bytes of each of the 56 × 32 tile records, row by row, with the graphics variant (`+0x04`) fed as 0. The last 8 bytes, the monster and object pointers, are left out: they are RAM addresses of entities the floor generator does not make. At `GenerateFloor`'s return the walkable-neighbour bytes are still all zero in the game, and the Rust floor never sets them.

The test `whole_floors_match_the_floor_oracle` runs `generate_floor` from the recorded entry state and checks the RNG step count, `layoutFnv1a`, stairs, room count and terrain counts for the natural seed and `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`, each on layouts 1 (the first floor's), 0, 8, 11 and 2 (one-room monster house): 20 floors, 872 to 18,935 RNG steps, all matching. The expected values are committed in the test, so it runs without the ROM; regenerating them needs the ROM:

```
floor_oracle.py ROM out.jsonl --seeds 0x12345678,0xCAFEBABF,0x0BADF00D --layouts 1,0,8,11,2
floor_oracle.py ROM out.jsonl --layouts 1,0,8,11,2
```

The first-turn values (`rngSteps` 1,802 for the natural seed and 1,761 for `0x12345678`) include the 22 / 20 steps after `GenerateFloor` returns, which are not ported.

So the generator's inputs are the dungeon RNG state and the floor properties. A reimplementation can be tested against `floor_oracle.py`: generate a floor from the same seed and compare the layout hash, stairs and RNG step count. The reimplementation must be original code written from this description and our own analysis of Blue, not a translation of `pmd-red`'s source ([`legal.md`](../legal.md)).

## Reimplementation, step by step

The Rust generator lives in `rescue_sim::floor` (with `rescue_sim::rng`). It is written from this page and our own reading of Blue's code, and checked against [`verification/v7-trace/floor_stages.py`](../../verification/v7-trace/floor_stages.py), which records the dungeon RNG state and the arguments at the entry of every generation step on the retail ROM. Between each pair of checkpoints, the reimplementation must make the same number of RNG calls.

RNG calls per step on the first floor of a new game (small layout, a 4 × 2 grid), for the game's own seed and for a forced seed `0x12345678`:

| Step | Blue address | Natural seed | Seed `0x12345678` | Status in Rust |
|------|--------------|-------------:|------------------:|----------------|
| `GenerateFloor` up to `GenerateStandardFloor` | `ov_0001::02151E7C` | 11 | 7 | done for the standard and one-room layouts (`floor_gen::generate_floor` with `floor_gen::PortedSteps`), checked end to end, see "Prologue and retry loops in Rust" |
| std 1: grid bounds (`i × (56 / gridX)`, `i × (32 / gridY)`) | `ov_0001::0214FEB0` | 0 | 0 | done (`grid_bounds`) |
| std 2 `InitDungeonGrid`: clear the 30-byte cells; small and medium layouts mark columns `x >= gridX / 2` or `x >= gridX × 3 / 4` invalid | `ov_0001::0214FD7C` | 0 | 0 | done (`Grid::new`) |
| std 3 `AssignRooms` (room density as last argument) | `ov_0001::0214FB4C` | 129 | 129 | done (`assign_rooms`) |
| std 4 `CreateRoomsAndAnchors` | `ov_0001::0214F7A8` | 17 | 20 | done (`place_rooms`) |
| std 5 `AssignGridCellConnections` (walk length: props `+0x05`) | `ov_0001::0214F6A0` (calls `0214F224`) | 33 | 33 | done (`assign_connections`), walk length 15 for the first floor |
| std 6 `CreateHallways` (calls `0214E0C4` for each hallway) | `ov_0001::0214E824` | 19 | 18 | done (`create_hallways`); checked against the retail RNG states and the tile hash for four seeds |
| std 7 `EnsureConnectedGrid` `[red-hypothesis]` (calls `0214E0C4`) | `ov_0001::0214DB54` | 0 | 4 | done (`ensure_connected_grid`); checked against the retail RNG states and the tile hash for four seeds |
| std 8 `GenerateMazeRoom` `[red-hypothesis]` (chance: props `+0x09`) | `ov_0001::0214CF2C` | 0 | 0 | done (`generate_maze_room`); fires only with the gate forced, see below |
| std 9 `GenerateKecleonShop` `[red-hypothesis]` (chance: props `+0x07`) | `ov_0001::0214D448` | 0 | 0 | done (`generate_shop`) |
| std 10 `GenerateMonsterHouse` `[red-hypothesis]` (chance: props `+0x08`) | `ov_0001::0214D190` | 0 | 0 | done (`generate_monster_house`) |
| std 11 `GenerateExtraHallways` `[red-hypothesis]` (last argument: floor property `+0x13`, the number of attempts; 5 on the first floor) | `ov_0001::0214FF30` | 16 | 31 | done (`generate_extra_hallways`) |
| std 12, std 13, `ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions` | | 0 | 0 | |
| `GenerateSecondaryTerrainFormations` | `ov_0001::0214AAF8` | 1 | 1 | done (`generate_secondary_terrain`); off on the first floor, see below. The 1 step is the itemless-house draw that follows it |
| `SpawnNonEnemies` | `ov_0001::0214B650` | 788 | 760 | done (`spawn_non_enemies`) |
| `SpawnEnemies` | `ov_0001::0214B338` | 766 | 738 | done (`spawn_enemies`) |
| `ResolveInvalidSpawns` | `ov_0001::0214C1A8` | 0 | 0 | done (`resolve_invalid_spawns`) |
| `StairsAlwaysReachable` up to `GenerateFloor`'s return | `ov_0001::0214A760` | 0 | 0 | done (`stairs_always_reachable`, `mark_hallway_junctions`; see "End of `GenerateFloor`") |
| `GenerateFloor`'s return (`0x021525B4`) up to the first turn | | 22 | 20 | code after `GenerateFloor`, not ported |
| **Total** | | **1,802** | **1,761** | |

Each count is the number of RNG steps between the entry of that step and the entry of the next one.

What steps 2 to 4 do, as read in Blue's code (the Rust tests start from the measured state at step 3's entry and reach the measured states at steps 4 and 5 for the natural seed and for forced seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D`; the last one gives a 4 × 3 grid):

- **Grid.** The cells live in a `[gridX][15]` array of 30-byte records (column stride `0x1C2`). Bytes `+0`–`+7` are the room bounds (start x, start y, end x, end y, 16-bit, ends excluded), `+8` "invalid", `+0xA` "holds a room". Step 2 clears the record, sets "holds a room" and sets "invalid" for the unused columns. Which columns are unused comes from a byte that `GenerateFloor` sets to 0, 1 (small layout) or 2 (medium layout) (`gStandardGridMode` in [`symbols.md`](../symbols.md)).
- **Step 3.** One `DungeonRandInt(3)`; the target is `−density` rooms when the density is negative, else `density` plus that value. A 256-entry list starts with that many "room" entries and gets 64 swaps of two `DungeonRandInt(gridX × gridY)` positions (128 calls). Valid cells then take entries in column order (x outer, y inner). With an odd `gridX`, the cell at row 1 of column `(gridX − 1) / 2` never keeps its room, but it is still counted. If fewer than two rooms were counted, valid cells are visited again in column order, each becoming a room when `DungeonRandInt(100) < 60`, until one does (at most 200 passes), and a byte at `ov_0001::021B4584` is cleared.
- **Step 4.** Cells are visited row by row (y outer, x inner). Each valid cell's area starts 2 tiles after the cell's start and is 4 tiles narrower and 3 tiles shorter than the cell. A cell without a room gets an anchor: one floor tile with room number `0xFE`, at `DungeonRandRange` x and y kept 2 tiles (1 on the map's left and top grid edge) from the area's start and 4 tiles (2 on the right and bottom edge) from its end. A room takes `DungeonRandRange(5, width)` × `DungeonRandRange(4, height)`, rounded up to odd sizes when the result stays below the area's size, then limited to 3 / 2 of the other side (width first), and is placed at `DungeonRandInt` offsets in the area. Its tiles become floor (terrain type 1) with the room index (counted from 0 in visiting order). Then `DungeonRandInt(100) < 80` sets byte `+0x1D` when floor-properties byte `+0x0C` is non-zero, and bit 2 of floor-properties byte `+0x0D` sets byte `+0x1C`; when both would be set, one more `DungeonRandInt(100)` keeps `+0x1C` on 50 or more and `+0x1D` otherwise. So a room costs 5 or 6 RNG calls and an anchor 2: 3 rooms and an anchor (17) for the natural seed, 4 rooms (20) for the forced one. The floor reset (outer ring of tiles impassable, rooms cleared) and the tile layout are also in `rescue_sim::floor`.

Step 5 (`ov_0001::0214F6A0`, the wrapper; the work is in `0214F224`), as read from the disassembly:

- **Start.** The wrapper draws `x` with `DungeonRandInt(gridX)`, then `y` with `DungeonRandInt(gridY)`, and passes them on with the grid and the floor properties. The start cell is not checked, so it can be an invalid one.
- **Connection fields.** The 30-byte cell has four link bytes, one per side: `+0x13` north (`-y`), `+0x14` south (`+y`), `+0x15` west (`-x`), `+0x16` east (`+x`). Linking a cell to a neighbour sets the bit on both sides, the cell's own and the neighbour's opposite one. They are `GridCell::north`, `south`, `west` and `east`.
- **Walk.** Floor-properties byte `+0x05` gives the number of steps. A step draws `DungeonRandInt(8)` and then `DungeonRandInt(4)`; if the first draw is below 4 the direction becomes the second. The walker turns clockwise (east, north, west, south) while the side is off the grid or the neighbour is invalid, then links to the neighbour and moves there. A direction is never reset between steps.
- **Dead-end pass.** Unless floor-properties byte `+0x0B` is non-zero, passes run over the grid column by column until one adds no link. A valid cell with no room (`+0x0A` clear) and exactly one link gets one: `DungeonRandInt(4)` picks a start direction, which turns clockwise for up to eight tries until one is inside the grid and not linked. The cell then links there only if the cell east of it is valid. Blue reads that east cell for all four directions, not the cell it links to; the port keeps that. Past the last column that cell is outside the grid; the port reads it as valid, which is what a cleared record (`ResetFloor` clears the 8 × 8 records) gives. Not confirmed on the retail ROM.
- **RNG budget.** The wrapper's two draws, one draw for the first direction, two per walk step, and one per dead end handled. For the first floor the budget is 33 steps for the natural, `0x12345678` and `0x0BADF00D` seeds and 34 for `0xCAFEBABF`, with walk length 15; the dead-end pass draws nothing on these grids (every cell has a room). The step-5 test checks the state at the entry of step 6 for all four seeds.
- **Invalid cell in the walk's direction.** If the cell the walk would enter is invalid, the step ends there: no move, no link, and no turn. Only the grid edge turns the walker (clockwise, and it retries within the same step). Found by comparing the links with the retail ROM for `0x12345678`.
- Cells outside the grid are not represented in `Grid`, so the walk never leaves it. A cell with no open side would loop forever in Blue; the port stops that walk step instead. Every grid the draw produces has a usable neighbour, so this does not happen.

## Step 6: hallways (`ov_0001::0214E824`)

Read from the disassembly; the names are ours. The function takes the grid, the grid size, the x and y boundaries from step 1 (`xs`, `ys`) and a flag (the second stack word; zero on the first floor, where `floor_stages.py` records it).

- **Setup.** Each valid cell copies its four link bytes (`+0x13`–`+0x16`) into four "dig" bytes (`+0x17` north, `+0x18` south, `+0x19` west, `+0x1A` east). Before the copy, links that point off the grid are cleared (north on row 0, west on column 0, south on the last row, east on the last column). Invalid cells get zero dig bytes.
- **Hallways.** For each valid cell in column order (x outer, y inner), it takes an anchor point: a random point inside the room (`DungeonRandRange(start + 1, end − 1)` on each axis, x then y), or the cell's start when it has no room. Then for each side with a dig byte set, it looks at the neighbour on that side. If the neighbour is valid, it draws the neighbour's anchor on the axis that the hallway needs (x for north and south, y for west and east; no draw without a room) and digs the hallway with `0214E0C4`. Both cells then get `+0x0B` set, and the dig bytes of the pair are cleared, whether or not the hallway was dug.
- **Digging** (`0214E0C4`). Two shapes, chosen by the flag. A north or south hallway goes down the column of the start point to the row of the cell's boundary `ys`, along that row to the end column, then to the end point. A west or east hallway goes along the row of the start point to the column of the cell's boundary `xs`, down that column, then to the end point. Each straight run changes at most 56 tiles. A run stops the whole hallway at a floor tile that is not the start point. A tile that is not floor becomes floor (terrain type 1; the room number is unchanged).
- **Extra links** (unless the flag is set). For each cell in column order, one draw `DungeonRandInt(100) < 5`. If it passes and the cell is a valid room cell with `+0x0B`, `+0x12` clear and `+0x09` clear, one draw `DungeonRandInt(4)` picks a side. If the neighbour on that side is valid, has a room, `+0x0B` set, `+0x09` and `+0x12` clear, the pair is linked: both cells get the union of their bounds, the tiles of that rectangle become floor of the cell's room number, and `+0x12` is set on both (`+0x11` on the cell that is not the one the draw started from, or on this cell for north and west). Which bounds the pair takes depends on the side (see `extra_west`, `extra_north`, `extra_east`, `extra_south`).
- **Not known yet.** The roles of `+0x09` and `+0x11`; the caller that sets the flag; the out-of-map tile that the game returns for coordinates outside 56 × 32 (the port ignores writes there and reads them as not floor, which the first four floors do not exercise).
- **Checks.** The RNG state at the entry of step 7 and `tilesFnv1a` (FNV-1a 64 over the terrain flags and room of the 56 × 32 tiles, row by row, from `floor_stages.py`) match the retail ROM for the natural seed and `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`.

## Step 7: cells left without a hallway (`ov_0001::0214DB54`)

Read from the disassembly; the names are ours. The arguments are the grid, its size, `xs` and `ys` from step 1 (the sixth argument is not read).

- **First pass** (column order, x outer, y inner). It looks at each valid cell with `+0x0B` and `+0x11` clear, that is, one that step 6 neither linked nor merged.
  - A cell without a room, or with `+0x09` set, has its start tile turned back to wall: terrain type 0, and the low three bits of the tile's second flag word cleared. No RNG.
  - A room cell draws its anchor (`DungeonRandRange(start + 1, end − 1)`, x then y), then takes the first neighbour in the order north, south, west, east that is valid, has `+0x0B` set and has `+0x12` clear. Neighbour anchor: if it has a room, two draws (x then y; only the one the hallway needs is used), otherwise its start tile. It digs a hallway with `0214E0C4` with the same shapes and boundary arguments as step 6 (vertical first for north and south), then sets `+0x0B` on the cell and the link bytes on both cells (`+0x13`–`+0x16`). A cell with no usable neighbour stays unlinked, after its two draws.
- **Second pass.** Every valid cell that is still without `+0x0B`, `+0x11` and `+0x0F` loses its room: every tile in its bounds becomes terrain type 0 with room `0xFF`. The role of `+0x0F` is not known; nothing in steps 1 to 7 sets it.
- **RNG budget.** A room cell that finds a neighbour with a room costs 4 draws (the `0x12345678` floor), one without a room 2. The natural seed and the other forced seeds have no unlinked cell and draw nothing.
- **Checks.** The RNG state and `tilesFnv1a` at the entry of step 8 match the retail ROM for the natural seed and `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`. Only `0x12345678` digs.

## Steps 8 to 10: maze room, shop and monster house

Read from the disassembly; the names are ours. Each takes the grid, its size and a chance in percent (floor properties `+0x09`, `+0x07`, `+0x08`). All three are 0 on the first floor, so they draw nothing there. Checked by forcing the chance argument (`floor_stages.py --force-chance 8=100,9=100,10=100`, which replaces `r3` at each step's entry) on the natural seed and `0x12345678`, `0xCAFEBABF`, `0x0BADF00D` in four configurations (each step alone, then all three), and 23 more forced seeds with all three: 27 floors, 40 runs; the RNG state and `tilesFnv1a` at the entry of the next three steps match, and so do the floor-wide data below. The chain also took the retry path of `GenerateFloor` on some seeds; only the first attempt is compared.

Cell fields (30-byte record) that these steps read or set: `+0x0C` shop (`GridCell::shop`), `+0x0E` monster house (`monster_house`), `+0x10` maze (`maze`). They read `+0x08` invalid, `+0x09`, `+0x0A` holds a room, `+0x0B` hallway, `+0x0F`, `+0x11`, `+0x12`, `+0x1D` (`structure`). `+0x1D` is not read by steps 8 and 10 and `+0x1C` is not read by any of them (step 12 and 13 read them, below).

- **Step 8.** `chance != 0`, then `DungeonRandInt(100) < chance`, then dungeon `+0x34B6` (a 16-bit value) must be negative. `GenerateFloor` stores the layout attempt index there (0 to 9), so in a normal run the gate is closed: the step costs 1 draw with a chance and 0 without, and never builds a maze. Nothing else in overlay 1 writes a negative value that we have found. The `--maze-gate` option writes `-1` there at step 8's entry, which is how the maze body is checked. Candidates: valid cells with a room and a hallway (`+0x0B`) and none of `+0x09`, `+0x0C`, `+0x0E`, `+0x0F`, `+0x11`, and an odd width and an odd height (end minus start). A 256-entry list has one `true` at index 0; 64 swaps of two `DungeonRandInt(count)` draws; the k-th candidate in column order (x outer) is chosen, where k is the index of the set entry. The maze (`0214CC7C`, second argument 0) sets `+0x10` and byte `021B4590`, and walls paths through the room: along each side at every second position (north, then east, south, west) from each non-floor tile just outside the room, then from every floor tile at odd offsets 3 tiles in from the room's edges (after walling the tile to its west). Each path (`0214CAB8`) draws `DungeonRandInt(4)`, walls the tile it is on, takes the first of four directions (east, north, west, south, starting at the drawn one) whose tile two steps away is inside the room and floor, walls the tile between and moves; it ends when no direction works. Terrain type 0 is the wall.
- **Step 9.** Always sets the pair at `021B45C8` (two `u16`) to `0xFFFF`. Skipped, with no draw, when `021B4594` is set (a monster house exists; step 10 sets it, other writers unknown) or `GetFloorType() == 2`. Then `chance != 0` and one draw `< chance`. Two lists of 0 to 14 are shuffled with 200 swaps each (two `DungeonRandInt(15)` per swap, 800 draws in all). The scan takes the columns of the first list that are inside the grid and, for each, all rows of the second list that are inside the grid, and picks the first valid cell with a hallway, a room, no `+0x09`, `+0x10`, `+0x11`, `+0x12`, `+0x1D`, and width above 4 and height above 3. The cell gets `+0x0C`; byte `021B458C` is set; the shop area is the room minus one tile on each side (`x0 = start_x + 1`, `x1 = end_x - 1`, `y0 = start_y + 1`, `y1 = end_y - 1`, or `end_y` when that leaves fewer than 3 rows), stored at `021B45CC`; the area's tiles get terrain bit `0x20` and lose the low spawn bits `0x8` and `0x1`; the centre (`(x0 + x1) / 2`, `(y0 + y1) / 2`) goes to `021B45C8`; the smallest and largest x and y of the area go to dungeon `+0xC0E0` (initialised to `9999`, `9999`, `-9999`, `-9999`). All tiles of the room get spawn bit `0x10`.
- **Step 10.** `chance != 0`, one draw `< chance`, then `021B458C` clear (no shop), dungeon `+0x678` (16 bit) zero and `GetFloorType() == 0`. Candidates as in step 8 without the width and height test and without `+0x0E`, but with `+0x10` clear; the same shuffle picks one. The cell gets `+0x0E`, byte `021B4594` is set, its tiles get terrain bit `0x40`, and the room number goes to dungeon `+0x34AC`. A candidate-less floor draws only the chance.
- **RNG budget** (forced chance 100): step 8 costs 1 (gate closed) or 1 plus 2 × 64 plus the maze paths (about 140 to 160 with the gate open on these floors); step 9 costs 1 plus 800 whether or not a room qualifies; step 10 costs 1 plus 128 plus 0 (a shop exists, so only the first draw). `GetFloorType() == 2` and dungeon `+0x678` are not reachable on the first floor, so those two gates and the `021B4594` skip are read from the code only.

### Step 11: extra hallways

Read from the disassembly; the names are ours. Arguments: grid, grid width and height, and the attempt count (floor properties `+0x13`, 5 on the first floor). Checked on the natural seed and `0x12345678`, `0xCAFEBABF` and `0x0BADF00D`: the RNG state (16, 31, 10 and 10 steps) and `tilesFnv1a` at the entry of step 12 match the retail ROM. The branch that stops a turn east on the small and medium layouts (`gStandardGridMode` 1 and 2) is read from the disassembly; no checked floor reaches it.

Per attempt:

1. `DungeonRandInt(gridX)`, `DungeonRandInt(gridY)` pick a cell. It must be valid (`+0x08` clear), hold a room (`+0x0A`), have `+0x0B` set and not be a maze (`+0x10`); otherwise the attempt ends with no more draws.
2. A point in the room: `DungeonRandRange(start_x, end_x)`, then `DungeonRandRange(start_y, end_y)`. Then `DungeonRandInt(4)` times 2 gives a direction in the 8-way table (0 south, 2 east, 4 north, 6 west). Three rounds turn it away from the grid edge: 0 to 2 on the last row, 2 to 4 on the last column, 4 to 6 on the first row, 6 to 0 on the first column.
3. From the point it steps in that direction while the tile's room number is unchanged, then while the tile is floor. It stops the attempt if that tile is terrain type 2, if the 5 x 5 block around it leaves the map, or if the tile to its left or right is floor.
4. `DungeonRandInt(3) + 3` is the run length. While the tile is inside the map (not within 1 of the edge), not floor and without the wall flag `0x10`: the tile becomes floor unless one of its four 2 x 2 corner blocks would be all floor; the loop ends if the tile left or right of it is floor; each step uses up one of the run; at the end of a run it draws a new length (`DungeonRandInt(3) + 3`) and `DungeonRandInt(100)`, below 50 turns to the right (direction + 2) and otherwise to the left (direction - 2); then the loop ends when the tile is past x = 31 (small layout) or x = 47 (medium) and the direction is east. Then it steps.

## Steps 12 and 13 and the finalising passes

Read from the disassembly; the names are ours. The cell flags `+0x1C` (`GridCell::imperfect`) and `+0x1D` (`structure`) are set in step 4: `+0x1C` from floor-properties byte `+0x0D` bit 2, `+0x1D` (80 % chance) from byte `+0x0C`, and when both come up a draw `< 50` clears one of them. Step 12 reads `+0x1C`; step 13 reads `+0x1D` and skips cells with `+0x1C`. Both draw nothing on the first floor (no cell has either flag), so the RNG budget there is 0, as the issue says.

### Step 12: room imperfections (`ov_0001::0214E3DC`)

Arguments: grid, grid width and height. Cells in column order (x outer, y inner). A cell qualifies when `+0x08`, `+0x09`, `+0x10`, `+0x11` and `+0x12` are clear, `+0x0A` (room) and `+0x0B` (hallway) are set and `+0x1C` is set. It is used if `DungeonRandInt(100) >= 60`. The room then gets `(width + height) / 4` rounds (1 if that is 0). A round runs twice, once per side:

- `DungeonRandInt(4)` picks a corner (0 top left, 1 top right, 2 bottom right, 3 bottom left), and the side picks a direction along one wall from that corner (table in `CORNER_WALKS`).
- From the corner it walks up to 10 tiles while inside the room and not on floor. At the first floor tile it stops.
- That tile becomes wall (terrain type 0) when none of its 8 neighbours has a hallway tile (floor with room `0xFF`) in its own 3 x 3 block, and its south, east, north and west neighbours are floor or not exactly as the corner says (`CORNER_SHAPES`: the two neighbours on the room side are floor, the two outside are not).

### Step 13: secondary structures (`ov_0001::0214F6F0`, helper `0214C244`)

Arguments: grid, grid width and height. Cells in row order (y outer, x inner). A cell qualifies when `+0x08`, `+0x0E`, `+0x12` and `+0x1C` are clear and `+0x0A` and `+0x1D` are set. `0214C244` draws `DungeonRandInt(6)`. 0 does nothing. The others spend one unit of the budget in `gFloorPropsByteC` (`021B45C0`, a copy of floor-properties byte `+0x0C`; the check comes first, except that kind 3 draws its four points before it) and set `+0x09` on the cell once they build something. Every kind needs a minimum room size, checked before it spends anything. The tile writes use `0214CA90` (terrain type 2, the secondary terrain, plus extra terrain bits) and `0214CA3C` (bit `0x20` of every room tile's second flag word).

| Kind | Needs | Builds |
|------|-------|--------|
| 1 | nothing | both sides odd: a maze (`0214CC7C` with its second argument 1; paths turn into terrain type 2 on the room's own tiles). Otherwise a plus of five tiles around the centre if the room is at least 5 x 5, else the centre tile |
| 2 | both sides odd | 64 draws of `DungeonRandInt(width)`, `DungeonRandInt(height)`; the tile at the offset becomes terrain type 2 when the sum is odd |
| 3 | at least 5 x 5 | two points `DungeonRandRange(start + 2, end - 3)` (x, y, x, y); the rectangle between them, ends included |
| 4 | at least 6 x 6 | the ring around the 2 x 2 centre block (a 4 x 4 square), with extra terrain bit `0x4`; the centre tiles get bits in the second flag word |
| 5 | nothing | `DungeonRandInt(2)`: a vertical line of terrain type 2 through the middle column (nonzero) or a horizontal one through the middle row, unless a hallway tile is on or next to the line; the half before the line gets bit `0x80` |

The maze of kind 1 differs from step 8 in one way: `0214DB18` (the wall write) clears the terrain type, then sets type 2 if its second argument is nonzero and the tile's room is the one the maze started in. The tiles written straight (the one to the west of an inner lattice point) always become type 2.

### Tile passes

- `ResetInnerBoundaryTileRows` (`02149A70`): for every x, the tiles on rows 1 and 30 are reset (`0214C0A8`), and those at x = 0 and x = 55 get the impassable wall flag `0x10`.
- `EnsureImpassableTilesAreWalls` (`0214C0E0`): every tile with flag `0x10` becomes terrain type 0.
- `FinalizeJunctions` (`0214D900`): in column order, a floor tile with no room sets flag `0x8` on each of its west, north, south and east neighbours that is in a room, and turns such a neighbour from terrain type 2 into floor; a floor tile with room `0xFE` (an anchor) gets room `0xFF`. A last call (`0214D82C`) lists the flag-`0x8` tiles per room in the dungeon struct; it does not touch the tile grid and is not ported.

### Checks

The RNG state and `tilesFnv1a` match the retail ROM at the entries of step 13, `ResetInnerBoundaryTileRows`, `EnsureImpassableTilesAreWalls`, `FinalizeJunctions` and `GenerateSecondaryTerrainFormations`, for 27 floors (the natural seed, the three forced seeds and 23 more) and two budgets. The first floor has no flagged cell, so `floor_stages.py --force-structures N` writes `+0x1C` (x + y a multiple of 3) and `+0x1D` (all) into the cells at std12's entry, and the budget `N` into `021B45C0`. Of the 54 runs, 34 reach the end of the first layout attempt; in the other 20 the game rejects the layout (too few room tiles) and starts over after `EnsureImpassableTilesAreWalls`, so only the checkpoints up to that call are compared. The seeds exercise every kind (and the maze, plus and budget-exhausted paths of kind 1), both sides of kind 5, and corner cuts. Not exercised by any floor: the single-tile branch of kind 1 (rooms under 5 x 5), read from the disassembly only.

### Secondary terrain formations (`ov_0001::0214AAF8`)

Read from the disassembly; the names are ours. Arguments: a flag (1) and the floor properties. It returns at once unless `flag & props[+0x0D]` is non-zero (bit 0 of byte `+0x0D`), so it draws nothing on the first floor: the one RNG step between its entry and `SpawnNonEnemies` there is the itemless-house draw (`DungeonRandInt(100)`) that `GenerateFloor` makes right after it. The tile grid is the same at both checkpoints. The terrain it writes is terrain type 2, the same value as the secondary structures of step 13; the tileset decides whether it is water, lava or a chasm. It needs no tileset argument.

`DungeonRandInt(8)` picks the number of stripes from the table 1, 1, 1, 2, 2, 2, 3, 3. After the stripes come `props[+0x15]` ponds, then one pass over the map.

- **Stripe.** `DungeonRandInt(100) < 50` starts at the bottom row going up, otherwise at the top row going down. Then `DungeonRandInt(50) + 10` (tiles until the edge is lumped), `DungeonRandRange(2, 54)` for the column, and `DungeonRandInt(6) + 2` for the length of the first run. Each step of a run looks at the tile (when its column is inside the map): terrain type 2 ends the stripe, otherwise a wall tile without the impassable flag `0x10` becomes type 2 (`0214B2F8`). Then the run counter drops, the head moves, and a head that leaves the map (row below 0 or at 32) ends the run. The lump counter also drops; when it reaches 0 (once, if the stripe is long enough) there are 64 rounds of two `DungeonRandInt(7) - 3` offsets (x then y) from the head: a tile in `2 <= x < 54`, `2 <= y < 30` with at least one of its 8 neighbours of type 2 becomes type 2; then every tile within 3 of the head in the same range, x outer and y inner, with 4 or more neighbours of type 2 becomes type 2 (in place, so later tiles see earlier ones). At the end of a run: after a vertical one, `DungeonRandInt(100) < 50` turns left, otherwise right, for a sideways run; after a sideways one the stripe turns back to its first vertical direction (no draw). If the head is off the map the stripe ends (after that draw); otherwise a new run length is drawn (`DungeonRandInt(6) + 2`).
- **Pond.** Up to 200 tries of `DungeonRandRange(0, 56)` (x) and `DungeonRandRange(0, 32)` (y), until `1 <= x < 55` and `1 <= y < 31`; if none fits the pond is skipped. A 10 x 10 block has its border set and its inside clear. 80 rounds draw `DungeonRandInt(8) + 1` twice (row, column): the cell is set when any of its four neighbours is set. Every cell still clear becomes type 2 on the map tile `(x + row - 5, y + column - 5)` by the same wall rule (out of the map: nothing).
- **Last pass.** For every tile of type 2 (column order): if the terrain word has any of `0x160` (shop, monster house, and `0x100`) or bit 0 of the second flag word is set, it becomes floor (type 1); otherwise if it is not inside `2 <= x <= 54`, `2 <= y <= 30` it becomes wall (type 0).
- **Check.** The oracle option `floor_stages.py --force-secondary N` sets bit 0 of `+0x0D` and `+0x15 = N` at the step's entry. For the natural seed, `0x12345678`, `0xCAFEBABF`, `0x0BADF00D` and 16 more forced seeds the Rust step reaches the RNG state and `tilesFnv1a` of `SpawnNonEnemies`' entry (stripe counts 1 to 3, ponds 0, 3, 4 and 12; the first floor's stripes cost about 520 to 650 draws with 3 ponds). Without the option every floor keeps its tile hash and costs the single itemless draw.
- **Not exercised.** The 200-try limit of a pond, and the last pass turning flagged tiles (`0x160`, spawn bit 0) into floor: no checked floor has such a tile under the secondary terrain. Both are read from the disassembly only. Stripes that run into their own path, the lumpy edge, and the edge cut of the last pass are all reached.

### Spawn markers (`ov_0001::0214B650`, `SpawnNonEnemies`)

Read from the disassembly; the names are ours. Arguments: the floor properties and `isEmptyMonsterHouse` (the itemless-house draw from `GenerateFloor`). The step only marks tiles (bits of the second tile word) and sets two position pairs; the monsters, items and traps themselves are made later, so it needs none of the spawn tables. Floor-properties bytes: `+0x0F` items, `+0x10` traps, `+0x14` buried items. Each list below is scanned in column order (x outer, y inner); "room tile" means terrain type 1 with a room number other than `0xFF`, and "plain" means none of terrain bits `0x20` (shop), `0x8` (junction) and `0x100`.

1. **Stairs**, only if pair B (`+0xC0BC`, `+0xC0BE`) has a `-1` part. Candidates: plain room tiles with neither spawn bit `0x8` nor `0x10`. If there are any, `DungeonRandInt(n)` picks one: it gets spawn bit `0x1` and loses `0x2`, its room goes to `021B4598`, and its position to pair B. When `GetFloorType() == 2`, every floor tile of that room gets terrain bit `0x40` and the room number goes to dungeon `+0x34AC`.
2. **Items.** Candidates: plain room tiles without terrain bit `0x40`. If `+0x0F` is not 0: `DungeonRandRange(c - 2, c + 2)`, raised to 1 if lower. With candidates and a count above 0: shuffle, `DungeonRandInt(n)` start, spawn bit `0x2` on that many consecutive entries (wrapping to the start).
3. **Buried items.** Candidates: every tile of terrain type 0 (the outer ring included). Same as 2 with `+0x14` and no minimum.
4. **Monster-house items**, skipped when `isEmptyMonsterHouse`. Candidates: tiles with terrain bit `0x40` and neither `0x20` nor `0x8`. With `n` of them: `DungeonRandRange(5n/10, 8n/10)`, raised to 6, lowered to a limit of 7 (a 16-bit constant at `021AA194`, read from the overlay and from the emulator). Shuffle, start, then per entry `DungeonRandInt(2)`: non-zero sets spawn bit `0x2`; zero sets `0x4` if the byte at dungeon `+0x64D` is non-zero.
5. **Traps.** Candidates: plain room tiles without spawn bit `0x2` (so after steps 1 to 4). `DungeonRandRange(t >> 1, t)` with `t` from `+0x10`, a call even when `t` is 0; with a count above 0, at most 56: shuffle, start, spawn bit `0x4`.
6. **Leader**, only if pair A (`+0xC0B8`, `+0xC0BA`) has a `-1` part. Candidates: plain room tiles with none of spawn bits `0x2`, `0x4`, `0x8`. `DungeonRandInt(n)` picks one for pair A.

The shuffle (`0214BD40`) is `2n` swaps of two entries picked by `DungeonRandInt(n)` twice. Direct calls: 4 `DungeonRandRange` (items, buried, traps; the monster-house range) and 7 `DungeonRandInt` (stairs, three starts, the trap start, the house per-entry draw, the leader), as the issue counts. `floor_stages.py` now records, at the entry of `SpawnEnemies`, `spawnFnv1a` (FNV-1a over the second tile word), `tilesFnv1a`, both pairs, `021B4598` and `+0x34AC`; at `SpawnNonEnemies`'s entry it records the arguments and the three counts, and `--force-spawn ITEMS,BURIED,TRAPS[,TRAPFLAG]` overrides them (and `+0x64D`).

**Checks.** `rescue_sim::floor::spawn_non_enemies` reaches the RNG state of `SpawnEnemies`'s entry (788 steps on the natural seed, 760 on `0x12345678`), with the same spawn hash, terrain hash, stairs and leader, for the natural seed, the three forced seeds, and forced counts (more items, buried items and traps, trap flag on and off) and forced monster houses (`--force-chance 10=100`). Not exercised on the retail ROM: `GetFloorType() == 2` and `isEmptyMonsterHouse` (the first floor has neither); both are covered by unit tests against the disassembly.

### Enemy spawn spots (`ov_0001::0214B338`, `SpawnEnemies`)

Read from the disassembly; the names are ours. Arguments: the floor properties and `isEmptyMonsterHouse`. Like the previous step it only marks tiles, here with the blocked bit `0x8` of the second tile word; the species and the monsters come from later steps, so it needs neither the spawn tables nor the monster structs. Floor-properties byte `+0x06` is the monster density `d`. Scans are in column order (x outer, y inner); "room tile" means terrain type 1, a room number other than `0xFF`, and neither terrain bit `0x20` (shop) nor `0x100`.

1. **Floor spots.** The count is `DungeonRandRange(d / 2, d)` raised to 1 when `d >= 1`, and 0 otherwise. Candidates: room tiles without terrain bit `0x8`, without spawn bits `0x1` and `0x2`, and not the leader's tile (pair A, `+0xC0B8`, `+0xC0BA`). With any candidate, a non-zero 16-bit value at dungeon `+0x678` adds 1 to the count (so it can place one spot on a density-0 floor). With a count above 0: shuffle, `DungeonRandInt(n)` start, that many consecutive entries get spawn bit `0x8` (wrapping).
2. **Monster house**, only when the byte at dungeon `+0x34A8` is non-zero. The limit is a 16-bit constant at `021AA088` (12), or 3 when `isEmptyMonsterHouse`, times 3 / 2 (18 or 4). Candidates: room tiles with terrain bit `0x40`, not the leader's tile; existing spawn bits do not exclude them. With any candidate, `DungeonRandRange(7 n / 10, 8 n / 10)` raised to 1 and cut to the limit; shuffle, start, marks as above.

Direct calls: 2 `DungeonRandRange` (floor count, house count) and 2 `DungeonRandInt` (the two starts), as the issue counts. `floor_stages.py` records, at the entry of `SpawnEnemies`, its inputs (`enemies`: `itemless`, `density`, `hasHouse`, `extra`, `houseLimit`) and, at the entry of `ResolveInvalidSpawns`, `placed` (spawn and tile hashes); `--force-enemies N` overrides the density.

**Checks.** `rescue_sim::floor::spawn_enemies` has unit tests against the disassembly (gates, counts, draw order, the 18 and 4 limits, the leader's tile). On the retail ROM it reaches the RNG state and the spawn hash of the `ResolveInvalidSpawns` entry (766 steps on the natural seed, 738 on `0x12345678`) for the natural seed, the three forced seeds and three forced monster houses with `--force-enemies 8`, and on the one-room fallback, which takes the house branch (see "End of `GenerateFloor`").

### End of `GenerateFloor`

Read from the disassembly; the names are ours unless marked. Only the shop pass draws from the RNG, and it runs only when a shop was made. `ResetFloor`, the P2 list update, `0x02151D4C` and `0x0214C13C` make no RNG calls (decompile check). Static Ghidra analysis; no emulator check was run for these calls.

- **`ResolveInvalidSpawns`** (`ov_0001::0214C1A8`, `[red-hypothesis]` name). For every tile: when the terrain type is not 1, the trap marker (spawn bit `0x4`) is cleared, and so is the item marker (`0x2`) when the tile has terrain bit `0x100` or `0x10` (a buried item in a plain wall stays). A tile with the stairs marker (`0x1`) gets terrain bit `0x200` and loses the trap marker; a tile with the item marker loses the trap marker.
- **`GetFloorType`** (`0206DB90`, `[red-hypothesis]` name): 2 when dungeon byte `+0x668` is 1 and bytes `+0x641` and `+0x63D` are equal; else 1 when the fixed-room number `+0x34B4` is from 1 to 49; else 0.
- **`StairsAlwaysReachable(x, y, markUnreachable)`** (`ov_0001::0214A760`, `[red-hypothesis]` name). It builds a 56 × 32 byte map: a tile without terrain bit `0x4` blocks when its terrain type is not 1, and is also "secondary" when its type is 2. With `markUnreachable`, terrain bit `0x8000` is cleared on every tile first. The stairs tile is the start; when `(x, y)` is not stairs pair B, the answer is no. The game then sweeps the map, x outer and y inner, until a sweep adds nothing: a tile marked to visit is visited and marks its four sides when they are not visited, blocking or secondary, and its diagonals when the diagonal tile is also free (mask `0x87`) and neither of the two tiles beside the step blocks. Last, every tile that is neither visited, blocking nor secondary is unreachable: with `markUnreachable` it gets terrain bit `0x8000`; without, the answer is no unless the tile has terrain bit `0x100`. The port uses a flood fill, which reaches the same set.
- **After the stairs fallbacks.** Four calls run in this order:
  1. `0x02179060(x, y, 0x17C, 0)` at `0x02152534`, when the shop-centre pair (set by step 9) is set. It adds or updates an entry in an 8-entry list (below).
  2. The shop pass `0x02149AF8(props)` at `0x02152560`, when the shop area is set, which makes `+0x34AA` 1; otherwise `+0x34AA` is 0 (below).
  3. `0x02151D4C(0, 0, 56, 32)` at `0x02152588`: junction marking (below).
  4. `0x0214C13C` at `0x0215259C`, when its flag is set (below).

  Then `CloseFile` at `0x021525B0`.
- **Shop pass** (`0x02149AF8`, Red `sub_8051654`). The shop bounds are four ints at the pointer `DAT_02149E78`: minX, minY, maxX, maxY (see Still not done). The RNG draws, in order:
  1. Width: `DungeonRandRange(3, w - 2)`, retried up to 20 times; it stops on the first draw. Every draw in this range is at least 3, so it stops after one draw. Height: `DungeonRandRange(3, h - 2)`, the same.
  2. Cut count: one `DungeonRandRange(2, 4)`, drawn once and used as the limit for both the west/east loop and the north/south loop.
  3. West and east cuts: while the area is wider than the narrowest width, `DungeonRandInt(100) < 50` removes the west column, else the east one. Each removed tile loses terrain bit `0x20`. The cut stops early when the width is at or below the narrowest.
  4. North and south cuts: the same rule for the rows.
  5. No draws. Tiles of the area with both `0x20` and `0x8` lose `0x20`.
  6. Item markers: over the 3 × 3 block from `(x0 + x1) / 2 − 1` to `+ 1` (and the same for y), each tile with `0x20`, without `0x40` and without `0x8`, gets the item marker (`spawn |= 0x2`) when `DungeonRandInt(100)` is below its entry in the chance table. The draw happens only for tiles that pass this filter. The chances are 16-bit values, rows by y. Floor-properties byte `+0x18` picks the layout, and each layout is 0x12 bytes (3 × 3 shorts) in the tables at `021AAA38` (data, not read).

  Blue's `/ 2` is a signed divide, as in Red's C. Blue's loop guards match Red's `break` checks, which run before each draw. Red's comment says its first two loops run once; Blue has the same loops, and the port keeps them for RNG parity. The Red name of the chance table, `sKecleonShopItemSpawnChances`, is `[red-hypothesis]`.
- **`0x02151D4C(x0, y0, x1, y1)`** (`MarkNaturalJunctions`, Red `sub_804B534`, `src/dungeon_generation.c:422`), called with `(0, 0, 56, 32)`. Every tile loses terrain bit `0x4` (Red's `CORNER_CUTTABLE`). An open tile (type 1) with room `0xFF` gets the junction bit `0x8` (`NATURAL_JUNCTION`) when more than two of its sides are open. The east side is tested only for `x < 54`, and the south side only for `x < 30`. The south test compares `x`, not `y`, so at `y = 31` with `x < 30` it reads `GetTile(x, 32)`, the out-of-bounds tile (`gCurTilesetOobTile`). Red has the same bug, commented at `dungeon_generation.c:432`. The terrain type of that tile is not verified, so the effect on the output is unknown. No RNG calls.
- **`0x0214C13C`** (`ConvertSecondaryToNormalAndSecondary`, Red `sub_804FC74`, `src/dungeon_generation.c:3942`). Every tile of terrain type 2 becomes type 3: the low two bits of `terrainFlags` (`+0x00`, 16-bit) are cleared and then set to 3, in the walk over x 0 to 55 and y 0 to 31. No RNG calls. It runs only when the flag `DAT_ov_0001__021525D8` is non-zero. That flag is set at the start of `GenerateFloor`, from `PTR_DAT_021525D4[dungeon + 0x34AE] == 2`: `+0x34AE` is `tileset`, and the table is at `0x02091EFC`. Red's equivalent is `gDungeonWaterType[tileset] == DUNGEON_WATER_TYPE_WATER` (`dungeon_generation.c:170`); Blue's table value 2 is the same `WATER`. The table's contents beyond index 9 are not checked (#37).
- **`GenerateOneRoomMonsterHouseFloor`** (`ov_0001::021506B0`, `[red-hypothesis]` name), layout case 2 and both fallbacks: a 1 × 1 grid whose cell holds a room and has the hallway mark (`+0x0A`, `+0x0B`), with bounds `(2, 2)` to `(54, 30)`. Every tile in it becomes open floor of room 0, then step 10 runs with chance 999. Its RNG draws are step 10's (129 steps in the fallback run below). The stairs fallback leaves pair A as the last attempt left it, because `ResetFloor` clears only pair B.

`floor_stages.py` records checkpoints `StairsAlwaysReachable`, `FinishShop` (`0x02149AF8`) and `GenerateFloorEnd` (`0x021525B4`, right after `CloseFile`), with `placed` (spawn and tile hashes) at each and the pairs and shop area at the last. `--force-unreachable` makes every stairs check fail, to reach the fallback; `--force-shop-chances` replaces the shop's 3 × 3 chances with the test's own values, so the tests do not need the game's table.

**Checks.** On the retail ROM, starting from the `SpawnEnemies` entry, the Rust steps reach the spawn hash at `ResolveInvalidSpawns`, the tile hash at `StairsAlwaysReachable`, the stairs check's acceptance, and the RNG state, spawn hash and tile hash at `GenerateFloorEnd`. This holds for the natural seed, the three forced seeds and three forced monster houses. GenerateFloor's tail draws nothing on those floors: the 22 / 20 steps of the table (21 and 19 for `0xCAFEBABF` and `0x0BADF00D`) all come after `GenerateFloor` returns. The fallback (`--force-unreachable`, natural seed) and three forced shops (`--force-chance 9=100`) are checked the same way. Unit tests cover the reachability branches the retail floors do not reach (no-spawn tiles, marking, wrong position), `GetFloorType`, `0x0214C13C` and the `x < 30` quirk.

## Dungeon struct offsets seen here

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x5B8` | `unk5C0` (set to −1 per floor) | `0x5C0` | `RunDungeon_Async` |
| `+0x3360` | `deoxysForm` (from `DungeonRandInt(4)`) | `0x3800` | `RunDungeon_Async` |
| `+0x34AE`, `+0x34B0`, `+0x34B4` | `tileset`, `unk3A10` (music), `fixedRoomNumber` | `0x3A0E`, `0x3A10`, `0x3A14` | `RunDungeon_Async` |
| `+0x11304` | fixed-room file handle | — | `GenerateFloor` |
| `+0x19DF6` | `unk1BDD4.unk1C05E` | `0x1C05E` | `RunDungeon_Async` |
| `+0x34AC`, `+0x34B6`, `+0x678`, `+0xC0E0`..`+0xC0EC` | monster-house room, attempt index (maze gate), blocks the monster house (step 10), shop area bounds | — | steps 8 to 10 |
| `+0x34A8`, `+0x678` | monster house present (byte), extra enemy spot (16 bit) | - | `SpawnEnemies`, steps 8 to 10 |
| `+0xC0B8`, `+0xC0BA` / `+0xC0BC`, `+0xC0BE` / `+0x64D` | leader start / stairs positions (`-1` when unset) / trap flag for monster-house tiles | — | `SpawnNonEnemies` |
| `+0x1A2F8`, `+0x1A2F9`, `+0x1A308` | `floorProperties.tileset`, `.bgMusic`, `.fixedRoomNumber` | `0x1C576`, `0x1C577`, `0x1C588` | `RunDungeon_Async` |

The gap grows from −8 at `0x5C0` to −0x4A0 at `0x3800` and −0x560 at `0x3A0E`; inside `FloorProperties`, Blue has 2 bytes fewer between `bgMusic` and `fixedRoomNumber`.

## Prologue and retry loops in Rust

`rescue_sim::floor::plan_layout` covers the grid size draw and the layout switch (`GenerateFloor` steps 3 and 4). Its RNG calls are the grid draw and, for layouts 1 and 11, one `DungeonRandInt(2)` for the grid height `r + 2`. Forced seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D` reach the state recorded at `GenerateStandardFloor`'s entry, and seed `0x12345678` takes 7 RNG steps. Both match the table above.

`rescue_sim::floor_gen::generate_floor` runs the whole of `GenerateFloor` in the order of the disassembly below: the entry reset, the grid draw and layout switch, the fixed-room branch, both retry loops, the fallbacks and the final steps. The steps it calls are behind the `FloorSteps` trait. The driver can stop at `GenerateStandardFloor`'s entry (`GenStop::StandardFloor`), which is the checkpoint the forced-seed tests use.

`rescue_sim::floor_gen::PortedSteps` implements `FloorSteps` with the ported functions: `ResetFloor` as a fresh map plus stairs pair B unset, `GenerateStandardFloor` as steps 1 to 13 in order, the one-room monster house, the finalising passes, the spawns and the end of `GenerateFloor`. The floor-wide data of steps 8 to 10 and the spawn pairs live in `GenState`. The fixed rooms and layout cases 3 to 7, 9 and 10 are not ported and panic. Room density is read from props `+0x01` (the offset `pmd-red`'s struct gives; `[red-hypothesis]`). The test `generate_floor_matches_the_retail_floors_end_to_end` runs `generate_floor` from the RNG state at `GenerateFloor`'s entry and reaches the RNG state, spawn hash, tile hash, leader and stairs at `GenerateFloorEnd` for the natural seed, the three forced seeds, their three forced monster houses and their three forced shops. The natural seed's entry state is its step-3 state taken 11 LCG steps back (the prologue's budget). The first floor's props that the recorded runs pin: layout 1, room density 5, walk 15, monster density 4, 2 items, 5 extra hallways, all chances 0.

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
- **End**: `+0x34AA` is set from the shop area (`< 0` → 0, else `0x2149AF8(props)` and 1), then `0x2151D4C(0, 0, 56, 32)`, then `0x214C13C` when the byte at `021B45A8` is non-zero, then `CloseFile`.

### Still not done

- The fixed-room placements `0x0214A0D4` (numbers below 50) and `0x02150788` (from 50 on). They read the fixed-room file, which is game data and not in the repo, so they stay behind `FloorSteps`.
- The fixed rooms and the layout generators (cases 3 to 7, 9 and 10) are tracked in [#148](https://github.com/TorratDev/rust-mm/issues/148). The enemy spawns, the stairs marker and `GenerateStandardFloor` steps 6 and up are their own issues. The driver calls them through `FloorSteps`.
- Unresolved in the #48 findings:
  - `DAT_0214C0A4`, the base of ResetFloor's 8 × 8 cell records. It is an overlay pool word and could not be read: `memory read` fails on it, and overlay-form addresses are not accepted. The `+0xC11C` offset in `structs.md` is Red's and is not confirmed in Blue. The value is read from the overlay file (`0xC11C`, see the pool-word table above); the second read is open.
  - `DAT_02179124`, the base of the P2 list (`*PTR_gDungeon + DAT_02179124`). Value read from the overlay file: `0x584`, see the pool-word table above. The read value `0x584` puts the list at `gDungeon + 0x584`, not Red's `0x57C`: a [hypothesis] until the second read (#179 P1). Blue's consumer of the list (Red `sub_806C3C0`, which spawns each active entry with `SpawnWildMon`) is not located.
  - `DAT_02149E78`, the shop-bounds pointer. The bounds are four ints there, not in `gDungeon`. The table in the struct notes lists `+0xC0E0` to `+0xC0EC` for them; that matches only if the pointer targets those offsets. Unverified. The pointer's value is read from the overlay file as `0x021B45CC` (see the pool-word table above). That the target is an overlay global and not `gDungeon + 0xC0E0` is a [hypothesis].
  - The item-chance table at `021AAA38`: 16 layouts of 0x12 bytes, data. Its Red name is `[red-hypothesis]`.
  - The terrain type of the out-of-bounds tile that the south test reads at `y = 31` (P5b). It needs the tileset data or a runtime read.

## Open questions

- Each generator's internal RNG order (sub-functions such as room and corridor placement).
- Write the reimplementation (`crates/sim`) and test it against `floor_oracle.py`.
