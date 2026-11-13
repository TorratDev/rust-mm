# Fixed rooms: the fixed-room file and ProcessFixedRoom

**Date:** 2026-11-12
**Status:** partial. File format, decoder, tile setter, both placement paths, the number-4 rectangle and the staged stamp ported; spawn effects, the staged reveal and the RNG order of spawns still open. No emulator run covers the staged stamp or the number-4 rectangle yet (unit tests only). The retail whole-floor check of numbers 1 to 66 is in (see [Retail whole-floor check](#retail-whole-floor-check-p5)): it passes on all 100 rows after the P6 and P7 fixes (see [Code 0x44 and the monster draw](#code-0x44-and-the-monster-draw-p6-p7)). Before those fixes it failed on 49 rows (dungeon RNG steps and the tile hash of number 1 on one seed).
**Subsystem:** dungeon floor generation (fixed rooms)

Issue: [#342](https://github.com/TorratDev/rust-mm/issues/342), parent [#111](https://github.com/TorratDev/rust-mm/issues/111). Parent notes: [`dungeon-generation.md`](dungeon-generation.md).

## Addresses
| Address | Name | Source |
|---------|------|--------|
| `ov_0001::02150958` | `ProcessFixedRoom(number, props)`; w or h 0 -> one-room monster house, false; number < 50 -> `0214A0D4`, true; else `02150788`, false | [blue-verified] static |
| `ov_0001::0214A0D4` | places a fixed room below 50 (a w x h block of decoded tiles, wall elsewhere, number 4 rectangle) | [blue-verified] static |
| `ov_0001::02150788`, `0214A3D0`, `02149E80` | rooms 50 and up: grid placement, std4 variant, stamp (staged branch in `02149E80`) | [blue-verified] static |
| `ov_0001::0215277C` | reveal of a staged room (not ported) | [blue-verified] static, read only |
| `ov_0001::0214A348` | tile-stream decoder | [blue-verified] static |
| `ov_0001::02152B20` | per-tile setter for a tile code | [blue-verified] static; codes `0x10`-`0xDB` go through the 16-byte table (see Placement) |
| `0201A54C` | `OpenFileAndGetFileDataPtr("fixedmap")` | [blue-verified] |

## File and decoder (ported)
- `fixedmap`: SIR0 container in `dungeon.sbin`. Entry `N` is the file offset at index `N` of a `u32` array; entry = width, height, flags, tile stream. [blue-verified] for the entry; [hypothesis] that the array starts at the SIR0 content pointer (header word `+4`); the game reaches it through the file handle's `+0xC`. Not run. Read from the retail `fixedmap` (7552 bytes): the table holds 140 entries ([blue-verified] ROM data, see the findings note below).
- Decoder: byte `0x0E` then a literal code; otherwise code `b >> 4` repeated `(b & 0xF) + 1` times, row by row. [blue-verified] static; The escape is read only when the repeat count is 0 and leaves it 0, so a literal is one tile. [blue-verified] static (findings-342b).

## Placement (ported)
Sources: `~/rescue-re/scratch/findings-342.md` and `findings-342b.md` (ghidra-cli decompile and disassembly), and investigator A's findings for #342 (`~/rescue-re/scratch/findings/342-a.md`, P1-P4). All [blue-verified] static or ROM data unless marked; no emulator run covers the 2026-11-12 changes.

| Address | What | Rust |
|---------|------|------|
| `02152B20` via wrapper `0214A318` | tile setter: wrapper sets terrain `0x100`, room 0; codes 0-0xF, 0x43-0x45, table path for `0x10`-`0xDB`; codes above `0xDB` only get type 1 | `floor::set_fixed_tile` |
| `021AC208` | 220 x 16-byte table; offsets +0 item, +2 qty, +4 slot or item flags, +8 event/monster id, +0xA tag, +0xB trap (0x14 none), +0xC room, +0xD trap param, +0xE type-2 flag | passed in as `PortedSteps::fixed_table` (game data, not read by the sim) |
| `0214A0D4` | number < 50: w x h block at (5, 5), every other tile a wall (type 3 when `021B45A8`); then, for number 4, x 2..=4, y 5..=16 gets terrain `0x10` and type 0 (P2); then the `+0x34AE > 0x3F` pass; then FinalizeJunctions; no RNG | `floor::place_fixed_room` |
| `02150788` | number >= 50: AssignRooms, up to 64 picks `(int(gx), int(gy))` for a room cell, `0214A3D0`, then (grid not 1 x 1) `0214F224` from the pick, `0214E824(.., 1)`, `0214DB54`, stamp | `PortedSteps::place_fixed_room_large` |
| `0214A3D0` | step-4 variant: picked cell gets the entry size at (x0, y0) with no draw; others draw (min 5, max cell size; width 10-14 if gx < 3; height 16-24 if gy == 1); anchors 0xFF | `floor::place_fixed_grid_rooms` |
| `02149E80` | stamp of the stream into the picked cell, keeping each tile's room byte (flags bit 0 clear); flags bit 0 set: the staged branch (P1 below) | `floor::stamp_fixed_room` |

Not ported, with the reason:
- Item, trap and monster spawns of the setter (`02153190`, `02152FAC`, `02153018`) need the object system. They are recorded in `FixedRoomOut::spawns`, in setter order, and nothing runs them. A staged room's setter runs with spawn 0: it records no item and no table trap (those wait for the reveal), but it does record a table monster and a code-`0x45` trap at generation (P1). Their own RNG draws are not traced in the port; see the draw table (P3).
- Code 7's call to `0214961C` (out-of-bounds tile global): the flag `021B45A8` is kept, the call is not. Dead for this port (P4).
- The staged reveal `0215277C`: runs the setter again with spawn 1 from the stored buffer, and it is a runtime event. Stays unported; the buffer it reads is recorded in `FixedRoomOut::staged`.
- Dungeon `+0xC65C` (room byte of the last stamped tile, written by `02149E80`): not recorded; a dead store (P4).
- Cell byte `+0x1B` (set to 1 by `02150788`, meaning unknown): not modelled; a dead store (P4).
- [hypothesis] `0214A3D0` differs from `place_rooms` (height margin 4 against 3, anchors 0xFF against 0xFE, no 80 % flag draw). Taken as the findings give it; no run compares the RNG count (the earlier run for number 60: std3 to std6 = 192 steps).
- [hypothesis] After 64 missed picks the last pick's cell is stamped.
- The numbers 1-66 are checked against the retail hashes by the P5 test; the failures of the first run are fixed in the P6 and P7 section below.

## Staged rooms (P1)
**Retail data** [blue-verified ROM data]: the flags byte (entry byte 2) is 1 for entries 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 62 and 63 (12 entries). No other entry has bit 0 set. The earlier hypothesis that no retail room is staged is false. Their sizes are 5 x 5 to 7 x 8, all within the 8 x 8 buffer, and each contains tile code `0xC` (sets terrain `0x1000`, which the reveal tests).

**Stamp** [blue-verified static, `02149E80`, staged branch]: the room rectangle `(x0, y0, x1, y1)` (ends exclusive, from the grid cell) is stored at dungeon `+0xC0F0..+0xC0FC`. For each stream tile (rows outer, columns inner):
- the code goes into the 8 x 8 buffer at `+0xC61C + col * 8 + row` (`StagedRoom::codes[dx * 8 + dy]` here: `col` is the inner, x, counter of `02149E80` [blue-verified static]);
- the setter wrapper runs into a 20-byte scratch tile at `+0xC11C + col * 0xA0 + row * 0x14` with spawn 0, so its items and table traps are not recorded at generation (its table monsters and code-`0x45` trap are; see Port decisions);
- the scratch tile is copied into the map tile, keeping the map tile's room byte (`+5`);
- for tiles with `x0 + 2 <= x < x1 - 2` and `y0 + 2 <= y < y1 - 2` the map tile is overwritten with terrain word `0x0110` and tile byte `+0xA = 0xE`.

The reveal `0215277C` (not ported) reads the buffer and runs the setter with spawn 1 when an entity whose tile `(x, y-1)` has flag `0x1000` calls it. It runs only while `+0x34A9` is clear; then it clears terrain flag `0x1000`, sets `+0x34A9` to 1 and redraws. Its details are read, not run; confidence medium.

Port decisions and their status:
- The spawn gating, from a review decompile of `02152B20` [blue-verified static]: the spawn argument `param_5` gates only the item call `02153190` (with spawn 0 the id goes to `+0x67B` instead) and the table trap `02152FAC` when `+0xB != 0x14`. The table monster `02153018` is called whenever `+8 != 0`, whatever `param_5` is, and the code-`0x45` trap `02152FAC(0xB, x, y, 0)` is called whatever `param_5` is. Neither callee checks a spawn flag. So with spawn 0 the port still records a table monster and a code-`0x45` trap. The findings' "no spawns at generation" is true only for items and table traps.
- Items with spawn 0 still set the item id in `+0x67B` (the setter's own write).
- [hypothesis] The scratch tile starts cleared (`Tile::EMPTY`); the game's scratch contents before the first tile are not run.
- [hypothesis] The placeholder sets the terrain word and the low byte of `Tile::unk_e` (tile `+0xA`, the high byte kept) and nothing else; the findings name only these two fields.
- The buffer keeps the first 8 x 8 codes only.

## Number 4 rectangle (P2)
[blue-verified static, `0214A0D4`, decompiled]: after the outside-wall loop and before the `+0x34AE > 0x3F` pass, for number 4: `for y in 5..17, x in 2..5: terrain |= 0x10; type = 0`. The rectangle is x 2..=4, y 5..=16 and does not depend on the tileset flag. Entry 4 contains code `7` (also entries 3, 5, 8, 15, 16), and code 7 sets `021B45A8`, so number 4 runs with the flag set in retail. The earlier panic for number 4 with the flag is removed.

Rust: the rectangle check sits inside the per-tile loop of `place_fixed_room`, after the outside wall and before the tileset pass. Per tile this is the same order as the decompile's loops.

## Spawn RNG draws per setter (P3)
[blue-verified static] for the call tree and draw structure; the conditional draws are [hypothesis] on when they fire (state-dependent; not run). The "RandInt/RandRange" calls are the non-dungeon RNG (`02011968`, `02011924`, see [`rng.md`](rng.md)); "DInt" is `DungeonRandInt` `0219D058`.

| Setter | What it does | RNG draws |
|--------|--------------|-----------|
| `02152FAC(trap, x, y, param)` (traps; code `0x45` only, trap id 0xB; no retail table row has a trap, `+0xB != 0x14`) | `02142068` finds a free trap slot (loop over 64), fills it, stores the pointer in the tile (`+0x10`) | none (callee tree searched to depth 6) |
| `02153190(item, x, y, qty, flags)` (items; table byte `+0` nonzero with `param_5 == 1`) | `02152604(item)` (a duplicate scan, no draw); if true the item becomes `0x76`; then `02008458(&out, id, 0)`; then `021440B0` places the object (slot search, no draw) | `02008458`: none if the id is 0 or if `02008504(id)` is false (item category byte `+0xC` not 0 or 1). Otherwise one `RandRange(min, max)` (min `+0x1B`, max `+0x1C`), which draws one `RandInt(max - min)` only when `min != max` |
| `02153018(monsterId, x, y, tag, code)` (monsters; table `+8 != 0`) | for id `0x1A5` (the partner-slot id of codes `0x8D`-`0x90`) copies a party member if one exists, else id `0x3C`; any other id spawns that id. Calls `02179CFC(spawn, 1)` | `02179CFC` -> `021794CC`: `FUN_02001330` = `RandInt(10)`, then `RandInt(0x12)` repeated until nonzero (up to 100 times; expected 1.06 draws), always. Then conditional `DInt` draws: `02182B3C` (move list, `DInt(4)` per learnable move beyond 4, only on a move-cache miss in `0217A7EC`) and the AI step `02175DB4` -> `021918D4` (`DInt(8)` for a monster with `+0x98 == 1`) or `02190B80`/`02190854`/`02180C98` target draws. The `DInt(100)` of `02179CFC` is skipped (second argument 1) |

Per tile code, from the 220-entry table `021AC208` (read from the program) and a decode of the 140 retail rooms:
- no RNG: codes `0x00`-`0x0F` direct, `0x16`-`0x1B` and `0x1D` (position slots only: they write a position into dungeon `+0xC0C0 + (slot - 1) * 4`), `0x3C`, `0x42`, `0x43`, `0x44`, `0x45` (trap, no RNG);
- no RNG (items): codes `0x46`-`0x58` (item ids 225, 226, 231, 230, 47, 46, 175, 209, 48, 49, 224, 227-229, 109, 54, 110; used only by rooms 50-66). Runtime read of the item table (`*020DECF4`, 0x20 bytes per item, category `+0xC`, min/max `+0x1B/+0x1C`) at the end of generation: all 18 ids (the 17 above and `0x76`) have category 4, 5 or 8 and min = max = 0, so `02008504` is false and `02008458` takes the no-draw branch. Every retail fixed-room item code makes 0 RNG draws [blue-verified runtime];
- monster spawn (`RandInt(10)` + `RandInt(18)` x k, and the conditional `DInt` calls): codes `0x10`-`0x15`, `0x1E`-`0x32`, `0x3B`, `0x4B`, `0x59`-`0x8C`, `0x8D`-`0x90`.

Every retail table row has trap `+0xB == 0x14` (none), so `02152FAC` is called only by code `0x45` (rooms 17, 18, 19).

Staged rooms (P1) run their setters with spawn 0, so their table monsters and code-`0x45` traps are still spawned, and their monster draws happen, at generation. Only their items and table traps wait for the reveal [blue-verified static, as in Port decisions of P1].

Open: room 5 (one unattributed draw in the earlier run). It holds exactly one setter code, `0x22` (monster 144, tag 13), so it is a monster spawn: `RandInt(10)` and `RandInt(18)` x k. The one draw is most likely one of the conditional `DInt` calls [hypothesis, not isolated]. A trace hook on `0219D058` (record `LR`) over a forced room 5 would settle it.

## Dead stores (P4): do not port
- **`0214961C`**: called only from code 7 of `02152B20`. It sets global `021B3ED0` to one of two 20-byte tile templates (`021ABDE0` or `021ABDCC`), chosen by `FUN_0206DBDC(+0x34AE)`. Both templates are byte-identical (terrain 0, `+4 = 1`, room `+5 = 0xFF`, rest 0), and `GetTile` reads `021B3ED0` only for coordinates outside 0..55 x 0..31, where it returns the template as a wall with room `0xFF`. [blue-verified static]. At the entry and end of generation on the first floor the global is already `0x021ABDE0` (runtime) and the call has no visible effect. Since both templates are identical the choice never changes behaviour [blue-verified runtime for the value; the early writer is unresolved]. Keep recording `tileset_flag` only; if out-of-bounds tiles are ever needed, return a wall tile with room `0xFF` and byte `+4 = 1`.
- **Dungeon `+0xC65C`**: written only by `02149E80` (both branches, the room byte of the last stamped tile). A search of all 1710 decompiled `ov_0001` functions finds no reader. Medium-high confidence: decompiler constant folding could hide an access, and the ARM9 and other overlays were not searched. Do not model it.
- **Cell byte `+0x1B`** (the 30-byte grid cell; `&stack+0x83 - &stack+0x68` in `02150788`): set to 1 for every cell `x < gx, y < gy` before the picks. The pick test reads `+0x0A` (has room), not `+0x1B`. A search of the 20 `ov_0001` functions with the `* 0x1E` cell stride finds no read of `+0x1B`. Medium-high confidence, same caveat. Do not model it.

## Retail whole-floor check (P5)
[blue-verified runtime, probe run; the check is in `tests/tests/fixed_rooms_rom.rs`, `fixed_rooms_match_the_retail_floors`]

**Method.** At `GenerateFloor`'s entry (`ov_0001::02151E7C`) the probe writes the dungeon RNG seed and one byte at dungeon `+0x34B4` = N (the forced number). Layout byte 1, natural floor properties and the natural global state. Hashes and counts are read at `GenerateFloorEnd` (`021525B4`); `rngSteps` is the number of dungeon-RNG steps from the seed to there. The method and its caveats are in investigator b's findings for #342 (`~/rescue-re/scratch/findings/342-b.md`, P5).

**Command** (probe copy of the ROM, a throwaway copy per process; the six processes split the numbers 1 to 66; output stays outside the repo):

```sh
~/venvs/desmume/bin/python -I ~/rescue-re/scratch/342b/fx.py ROM_COPY.nds OUT.jsonl 0x12345678,0xCAFEBABF,0x0BADF00D NUMBERS
```

- Seeds: `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`. Numbers 1 to 49 have one row (the same result on all three seeds); numbers 50 to 66 have one row per seed. 100 rows in the test.
- ROM: the probe copy of `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330`.
- The rows in the test are the probe's hashes and counts only. No grid bytes, no raw output in the repo.
- The hash (`layoutFnv1a`) covers tile bytes 0 to 11 with `+0x04` zeroed, as `floor_oracle.py` does. It includes `spawn_or_visibility` (`+0x02`). It does not include the item, trap and monster lists, so it does not check the spawn RNG draws.

**Test.** `RESCUE_ROM=<dump> cargo test -p rescue-tests --test fixed_rooms_rom`. The test reads `fixedmap` from `dungeon.sbin`, and the 220-entry tile-code table at `021AC208` from the ARM9 overlay whose RAM range holds it, then runs `generate_floor` for each row with `GenState::fixed_room` set to the row's number. It needs no change to `crates/sim` (every item it uses is already `pub`).

**Result, first run** (2026-11-13, port at `c1f7be4`, `RESCUE_REQUIRE_ROM=1`): 49 mismatches in 100 rows. Stairs, rooms and terrain counts match on every row. The table fails in two ways:
- **Dungeon RNG steps, numbers 1 to 49 except 6 (48 numbers, all seeds equal):** the port takes 2 steps; the oracle takes 3 to 6. Number 6 matches (2 steps). Numbers 50 to 66 match on all three seeds. Hypothesis, not isolated: the difference is the item, trap and monster draws of the setters, which are not ported (the "Not ported" list above; the per-setter draws are in P3). Number 6 is the one row whose RNG count matches, which fits a setter with no such draw, but the probe did not record which draws happen.
- **Tile hash, number 1 on seed `0x0BADF00D` only:** port `0x6473e7f0b4b5b711`, oracle `0xb2d08f29d84e66a9`. Not isolated. The hash covers `+0x02` and `+0x0A`/`+0x0B`, which a spawn could write; the probe kept no per-tile dump, so the differing tile is not known.
- **Number 4** (the number-4 rectangle, P2) **fails** the check: its row (one row, the same on every seed) matches on the tile hash, stairs, rooms and terrain counts, and differs only in the RNG steps (port 2, oracle 3). So the rectangle gives the retail tiles for the probe's state; the difference is in the spawn draws, as for the other numbers.

Values were not changed to make rows pass.

## Code 0x44 and the monster draw (P6, P7)
**Result after the fix** (2026-11-13, this PR): `fixed_rooms_match_the_retail_floors` passes on all 100 rows, 0 mismatches (`RESCUE_ROM=<dump>`).

**Code `0x44` (P6)** [blue-verified static, decompile of `02152B20`, case `param_2 == 0x44`; retail tile bytes checked by a grid dump of number 1, seed `0x0BADF00D`]:
- `*p |= 0x800; *p &= 0xfffc; *p |= 1;` on terrain: type 1, flag `0x800`;
- `p[1] &= 0xfffd` on tile `+2`: bit 1 cleared, bit 0 not set (no spawn marker);
- `*(u8*)(p+5) = 0`: room 0.

Codes 8 and `0x43` (label `02152D34`) do set bit 0 (`p[1] |= 1; p[1] &= 0xfffd`) and write the player spawn at dungeon `+0xC0BC`. The old port called the marker for `0x44` as well; that set bit 0 on four tiles of room 1, and `resolve_invalid_spawns` (`0214C1A8`) then added terrain `0x200`. Rust: `floor::set_fixed_tile`, branch `0x44` (unit test `fixed_tile_setter_code_44_clears_bit_1_only`).

**Monster setter draw (P7)** [blue-verified runtime, probe at `GenerateFloor` (`02151E7C`) with the number forced; the call chain is observed at its last hop]:
- Each call of the monster setter `02153018` (table monster, `+8 != 0`) makes exactly one `DungeonRandInt(8)` (`0219D058`), issued from LR `021909C4` inside `02190854` (the wander-target step: branch `+0xF0 == 0`, state byte `+0x70 != 4`, no waypoint). The result only sets monster AI fields.
- Chain `02153018` -> `02179CFC` -> AI step `021918D4` -> `02191394` or `02190B80` -> `02190854`: the intermediate hops are [hypothesis]; the last LR is observed.
- Order: for each table monster in setter order, the spawn, then the `DInt(8)`; then the common tail `DungeonRandInt(100)` (`0215241C`) and `DungeonRandRange(2, 4)` (`0214B370`, one step). Total dungeon steps = 2 + number of monster calls. That reproduces the oracle's steps on all 49 numbers below 50 (1-49 have 0 to 4 monster calls each; number 6 has none).
- No other dungeon draw happens in rooms 1-49. Items (`02153190`) and traps (`02152FAC`, code `0x45`) draw nothing in these rooms. Rooms 50-66 have no monster call; their draws are the layout draws.
- Not checked: other monster states (no retail room reaches them), and whether the setter's global-RNG draws (`RandInt(10)`, `RandInt(18)`, see the table in P3) affect the dungeon RNG (they do not; they are the global generator).

Rust: `PortedSteps::place_fixed_room` (`crates/sim/src/floor_gen.rs`) draws `rng.int(8)` once per `FixedSpawn::Monster` in `FixedRoomOut::spawns`, in setter order, and discards the result. Shortcut comment there: one wander-step draw, valid for the retail monster states; the setter's global-RNG draws stay unported. Rooms 50-66 have no table monster, so no draw happens there.

## Evidence and verification status
- Sources: investigator A's findings for #342 (`~/rescue-re/scratch/findings/342-a.md`): ROM data read from `fixedmap` in `dungeon.sbin` (`pmdb.nds`), static decompiles from a ghidra-cli copy, and one py-desmume probe for the `021B3ED0` value. Tags: ROM-data and static claims are [blue-verified]; the reveal details are read, not run.
- Unit tests in `crates/sim/src/floor.rs` check the shapes and values stated above: the number-4 rectangle with and without the tileset flag and for number 3 (`number_4_rectangle_is_applied_with_or_without_the_tileset_flag`); the staged stamp's rectangle, buffer, placeholder, room bytes and the table monster recorded with spawn 0 (`staged_room_is_a_placeholder`).
- **No emulator run covers P1 or P2 yet.** Neither a forced staged room nor a forced fixed room 4 with the tileset flag has been run against the retail dump (P5 of the findings needs the forcing). The unit tests check the port against the findings, not the game.

Rust: `floor::fixed_map_entry`, `floor::TileStream`, `floor::FixedRoomOut` (with `staged: Option<StagedRoom>`), `PortedSteps::{fixed_map, fixed_table, tileset, fixed_out}`.
