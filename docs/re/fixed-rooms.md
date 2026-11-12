# Fixed rooms: the fixed-room file and ProcessFixedRoom

**Date:** 2026-11-12
**Status:** partial. File format, decoder, tile setter, both placement paths, the number-4 rectangle and the staged stamp ported; spawn effects, the staged reveal and the RNG order of spawns still open. No emulator run covers the staged stamp or the number-4 rectangle yet (unit tests only).
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
- Item, trap and monster spawns of the setter (`02153190`, `02152FAC`, `02153018`) need the object system. They are recorded in `FixedRoomOut::spawns`, in setter order, and nothing runs them. A staged room's setter runs with spawn 0, so it records none (P1). Their own RNG draws are not traced in the port; see the draw table (P3).
- Code 7's call to `0214961C` (out-of-bounds tile global): the flag `021B45A8` is kept, the call is not. Dead for this port (P4).
- The staged reveal `0215277C`: runs the setter again with spawn 1 from the stored buffer, and it is a runtime event. Stays unported; the buffer it reads is recorded in `FixedRoomOut::staged`.
- Dungeon `+0xC65C` (room byte of the last stamped tile, written by `02149E80`): not recorded; a dead store (P4).
- Cell byte `+0x1B` (set to 1 by `02150788`, meaning unknown): not modelled; a dead store (P4).
- [hypothesis] `0214A3D0` differs from `place_rooms` (height margin 4 against 3, anchors 0xFF against 0xFE, no 80 % flag draw). Taken as the findings give it; no run compares the RNG count (the earlier run for number 60: std3 to std6 = 192 steps).
- [hypothesis] After 64 missed picks the last pick's cell is stamped.
- The numbers 1-66 are not checked against a retail tile hash.

## Staged rooms (P1)
**Retail data** [blue-verified ROM data]: the flags byte (entry byte 2) is 1 for entries 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 62 and 63 (12 entries). No other entry has bit 0 set. The earlier hypothesis that no retail room is staged is false. Their sizes are 5 x 5 to 7 x 8, all within the 8 x 8 buffer, and each contains tile code `0xC` (sets terrain `0x1000`, which the reveal tests).

**Stamp** [blue-verified static, `02149E80`, staged branch]: the room rectangle `(x0, y0, x1, y1)` (ends exclusive, from the grid cell) is stored at dungeon `+0xC0F0..+0xC0FC`. For each stream tile (rows outer, columns inner):
- the code goes into the 8 x 8 buffer at `+0xC61C + col * 8 + row` (`StagedRoom::codes[dx * 8 + dy]` here: [hypothesis] that `col` is the x offset);
- the setter wrapper runs into a 20-byte scratch tile at `+0xC11C + col * 0xA0 + row * 0x14` with spawn 0, so no spawn is recorded at generation;
- the scratch tile is copied into the map tile, keeping the map tile's room byte (`+5`);
- for tiles with `x0 + 2 <= x < x1 - 2` and `y0 + 2 <= y < y1 - 2` the map tile is overwritten with terrain word `0x0110` and tile byte `+0xA = 0xE`.

The reveal `0215277C` (not ported) reads the buffer and runs the setter with spawn 1 when the player is next to the tile with flag `0x1000`; it clears `+0x34A9` and redraws. Its details are read, not run; confidence medium.

Port decisions and their status:
- The spawn gating follows the spawn argument: with spawn 0 the setter records no monster (`0x59`-`0x8C` and the table's monster id), and no trap for code `0x45`. The earlier port recorded these with spawn 0. The findings state "no spawns at generation" for staged rooms [blue-verified static]; the gating itself is the port's model of the spawn argument.
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

Staged rooms (P1) run their setters with spawn 0, so they draw nothing at generation; only rooms 0-49 and the unflagged 53, 61, 64-66 spawn during generation.

Open: room 5 (one unattributed draw in the earlier run). It holds exactly one setter code, `0x22` (monster 144, tag 13), so it is a monster spawn: `RandInt(10)` and `RandInt(18)` x k. The one draw is most likely one of the conditional `DInt` calls [hypothesis, not isolated]. A trace hook on `0219D058` (record `LR`) over a forced room 5 would settle it.

## Dead stores (P4): do not port
- **`0214961C`**: called only from code 7 of `02152B20`. It sets global `021B3ED0` to one of two 20-byte tile templates (`021ABDE0` or `021ABDCC`), chosen by `FUN_0206DBDC(+0x34AE)`. Both templates are byte-identical (terrain 0, `+4 = 1`, room `+5 = 0xFF`, rest 0), and `GetTile` reads `021B3ED0` only for coordinates outside 0..55 x 0..31, where it returns the template as a wall with room `0xFF`. [blue-verified static]. At the entry and end of generation on the first floor the global is already `0x021ABDE0` (runtime) and the call has no visible effect. Since both templates are identical the choice never changes behaviour [blue-verified runtime for the value; the early writer is unresolved]. Keep recording `tileset_flag` only; if out-of-bounds tiles are ever needed, return a wall tile with room `0xFF` and byte `+4 = 1`.
- **Dungeon `+0xC65C`**: written only by `02149E80` (both branches, the room byte of the last stamped tile). A search of all 1710 decompiled `ov_0001` functions finds no reader. Medium-high confidence: decompiler constant folding could hide an access, and the ARM9 and other overlays were not searched. Do not model it.
- **Cell byte `+0x1B`** (the 30-byte grid cell; `&stack+0x83 - &stack+0x68` in `02150788`): set to 1 for every cell `x < gx, y < gy` before the picks. The pick test reads `+0x0A` (has room), not `+0x1B`. A search of the 20 `ov_0001` functions with the `* 0x1E` cell stride finds no read of `+0x1B`. Medium-high confidence, same caveat. Do not model it.

## Evidence and verification status
- Sources: investigator A's findings for #342 (`~/rescue-re/scratch/findings/342-a.md`): ROM data read from `fixedmap` in `dungeon.sbin` (`pmdb.nds`), static decompiles from a ghidra-cli copy, and one py-desmume probe for the `021B3ED0` value. Tags: ROM-data and static claims are [blue-verified]; the reveal details are read, not run.
- Unit tests in `crates/sim/src/floor.rs` check the shapes and values stated above: the number-4 rectangle with and without the tileset flag and for number 3 (`number_4_rectangle_is_applied_with_or_without_the_tileset_flag`); the staged stamp's rectangle, buffer, placeholder, room bytes and empty spawns (`staged_room_is_a_placeholder_with_no_spawns`).
- **No emulator run covers P1 or P2 yet.** Neither a forced staged room nor a forced fixed room 4 with the tileset flag has been run against the retail dump (P5 of the findings needs the forcing). The unit tests check the port against the findings, not the game.

Rust: `floor::fixed_map_entry`, `floor::TileStream`, `floor::FixedRoomOut` (with `staged: Option<StagedRoom>`), `PortedSteps::{fixed_map, fixed_table, tileset, fixed_out}`.
