# Dungeon floor generation

How Blue builds a floor, and the order of its RNG calls. Issue: [#48](https://github.com/TorratDev/rust-mm/issues/48). Names, addresses and tags are in [`symbols.md`](../symbols.md). The code is in overlay 1 and matches `pmd-red`'s `src/dungeon_generation.c` function for function. Not confirmed by emulator trace yet.

## Entry point

`GenerateFloor` (`ov_0001::02151E7C`) is the only function in overlay 1 that calls `DungeonRandRange` exactly 4 times and `DungeonRandInt` exactly 3 times, like Red's. Its 40-odd calls come in Red's order, with the same argument counts. It runs after `RunDungeon_Async` has seeded the dungeon RNG for the floor ([`rng.md`](rng.md)), so every RNG call below comes from the per-floor seed.

## Steps

1. Open the fixed-room file (`OpenFileAndGetFileDataPtr("fixedmap", 4)`; the handle is kept at dungeon `+0x11304`) and `ResetFloor`.
2. `ProcessFixedRoom(fixedRoomNumber, floorProps)`: if the floor is one fixed room, generation ends here.
3. Grid size: `DungeonRandRange(2, 9)` and `DungeonRandRange(2, 8)`, or `DungeonRandRange(2, 5)` and `DungeonRandRange(2, 4)` for a smaller grid. Each cell is the 56 × 32 map divided by the grid size.
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

## For bit-exact generation

The order of RNG calls in `GenerateFloor` and the direct calls of every generator match Red. Before porting, compare each generator's internal call order with Red's source and confirm the result on the retail ROM: dump the tile grid after `GenerateFloor` for a known floor seed (dungeon `+0x670`) and compare. The grid layout is in [`structs.md`](structs.md#tile): 56 × 32 tiles of 0x14 bytes at dungeon `+0x34B8`.

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
- Emulator check of one generated floor.
