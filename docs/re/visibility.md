# Visibility: what the leader sees and what the map reveals

**Date:** 2026-11-13
**Dump:** `BlueRescueTeamEur` (SHA-1 `6f01887356268de39085df01490347bbcc3aa330`, see [`supported-dumps.md`](../supported-dumps.md))
**Issue:** [#354](https://github.com/TorratDev/rust-mm/issues/354)
**Workflow:** [workflow.md](workflow.md)

Each claim below carries one status: **blue-verified**, **red-hypothesis** or **lead**. A claim with no evidence is not a fact. Addresses are overlay 1 (`ov_0001::`) unless marked as ARM9. "Dungeon +0x..." is an offset into the dungeon struct. The names in this note (for example "reveal routine") are working labels from the #354 investigator; none of them is in [symbols.md](../symbols.md) yet.

## Summary

Two things are stored or computed separately:

- **Seen now** is computed each turn from the leader's position, its room and the floor's darkness setting. No per-tile "visible" bit was found.
- **Revealed on the map** is bit 0 of each tile's `+2` word. The reveal routine sets it. No writer that clears it was found.

Monsters and items use a separate range test for whether a viewer can see a target. Sprites on a lit floor also pass a screen-size box. Dark floors are not verified at runtime. See the open questions.

## Rule for a leader with no status

Inputs: the darkness setting `D` (floor properties `+0x16` & 3, held at view block `+0x15FA1`), the leader's tile (`x`, `y`), the tile's room id (tile `+5`; `0xFF` = corridor), the room records, and bit 0 of each tile.

Range for corridors: `R = D`, and `R = 2` when `D = 0` (a lit floor). Use the claims in the table for the conditions.

**Each turn** (skip everything when the leader is blinded, `+0x15FA2 = 1`):

1. **Seen now.**
   - If the leader stands in a room: every tile in `[x0-1, x1] x [y0-1, y1]` of that room's record.
   - Otherwise: every tile with `|dx| <= R` and `|dy| <= R` from the leader.
2. **Reveal.** For the same set:
   - In a room: if the room record's revealed byte is 0, set it to 1 and set bit 0 (and bit 1) on the room rect. If it is already 1, do nothing.
   - In a corridor: set bit 0 (and bit 1) on the square every turn.
3. **Sprites** (not tiles): on a lit floor, a sprite is drawn when `|dx| <= 6` and `|dy| <= 5` from the leader. On a dark floor it also needs the test in step 1.

A whole-floor reveal (`0213B460`) sets bit 0 on every tile. It is called from one place, which is not yet read (open questions).

## View block

The leader's view state is one block of the dungeon struct. `0213C63C` resets it on a fresh floor. `0213C0F8` refreshes it each turn.

| Offset (dungeon) | Meaning | Writer | Status |
|---|---|---|---|
| `+0x15F80` / `+0x15F82` | leader x, y in tiles | `0213C0F8` | blue-verified |
| `+0x15F90` | leader entity pointer | `0213C0F8` | blue-verified |
| `+0x15FA0` | screen attribute: 3, or 2 while `+0x15FB2` is set. Read by `0217B120` and `021794CC` | `0213C63C`, `0213AD54` | blue-verified |
| `+0x15FA1` | visibility range `D` = floor props `+0x16` & 3 (props base `+0x1A2F6`, so the byte is `+0x1A30C`). Fresh floor only | `0213C63C` | blue-verified |
| `+0x15FA2` | blinded: 1 when leader info `+0xC9 == 1` | `0213C0F8` | blue-verified |
| `+0x15FA3` | whole floor known. Set by `0213B460` and at floor start; cleared by `0213C63C` | `0213B460`, `0213C63C` | blue-verified; floor-start writer is a lead |
| `+0x15FA4` | lit floor: 1 when `D == 0` (also set to 1 at dungeon start by `RunDungeon_Async`) | `0213C63C` | blue-verified |
| `+0x15FA5`, `+0x15FA6`, `+0x15FA9` | copies of leader info `+0xCD`, `+0xCE`, `+0xCF`. The first two are forced to 1 while the leader holds item `0x11` | `0213C0F8` | blue-verified |
| `+0x15FA7` | sees invisible monsters and traps: 1 when `02180608(leader)` (info `+0xC9 == 3` or item `0x1A` held) | `0213C0F8` | blue-verified |
| `+0x15FA8` | 1 when leader info `+0xC9 == 2` (tints entities on the minimap) | `0213C0F8` | blue-verified |

## Claims

| Claim | Evidence | Status |
|-------|----------|--------|
| P1. Corridor range: a tile is seen when `\|dx\| <= R` and `\|dy\| <= R` from the viewer, with `R = +0x15FA1`, and 0 read as 2. | `ov_0001::02147DC4` (reveal), `ov_0001::0219C6BC` (in-view test), `ov_0001::0213C63C` (sets `+0x15FA1`); users `02193CC0`, `0219706C`, `0219C4F8` | blue-verified |
| P1. Room rule: a tile inside a room's record is seen when `x0-1 <= x <= x1` and `y0-1 <= y <= y1`, that is the room plus one wall ring. The room branch takes no range. | `ov_0001::0219C6BC` (tests `x < x1+1`, and the same for y); room table at dungeon `+0xE264`, 0x1C bytes per room | blue-verified |
| Room record: `+1` revealed byte; `+2`/`+4` = x0/y0; `+6`/`+8` = x1/y1 (16-bit). | `ov_0001::02147DC4` reads and writes them | blue-verified |
| P1. Darkness: `D` = floor props `+0x16` & 3 is copied to `+0x15FA1` on a fresh floor. `D = 0` is lit (`+0x15FA4 = 1`, corridor R becomes 2). `D` of 1 or 2 is dark (corridor R is 1 or 2). | `ov_0001::0213C63C`; props at dungeon `+0x1A30C` | blue-verified |
| P1. The in-view test takes the room id from the viewer's tile (`param_1`) and tests the target's position against that room's rect, or against the range square when the viewer has no room. | `ov_0001::0219C6BC`; checked by the reviewer of #406 in the export (STAMP present), not at runtime | blue-verified |
| P1. Sprite on a lit floor: visible when `\|dx\| <= 6` and `\|dy\| <= 5` from the leader. On a dark floor it also needs the room/range test. | `ov_0001::0213C4DC` | blue-verified |
| P1 runtime: on the first floor the leader's room reveal matched the room rule. The corridor radius was not checked (no corridor was crossed). A dark floor was never entered. | Investigator c's probe. No script name or output hash was given. | lead |
| The darkness bits take only the values 0, 1 and 2 across the floor-property records. 3 was not seen. | Investigator c's scan of the record table. No script recorded. | lead |
| P2. Tile word `+2`: bit 0 = discovered. This is the bit the minimap reads. | `ov_0001::02147DC4`, `ov_0001::0213CD5C`, `ov_0001::0213B460` | blue-verified |
| P2. Bit 1 is written together with bit 0 by both reveal paths. The name "lit" is the investigator's; no reader that uses bit 1 for lit was found. | `ov_0001::02147DC4`, `ov_0001::0213B460` | lead |
| P2. Reveal routine `02147DC4(pos)`: returns at once when blinded (`+0x15FA2`). Corridor: sets bits 0 and 1 on the `(2R+1)²` square. Room: returns if the room's revealed byte is 1; else sets it to 1 and sets the bits over the room rect. For each tile it repaints the minimap cell. | `ov_0001::02147DC4`; minimap repaint `ov_0001::0213CD5C` | blue-verified |
| P2. Callers of the reveal (step, arrival and display paths): `02160A7C`, `021766AC`, `02177F74`, `02187B14`, `02187B34`, `02191A7C`, `021934D4`, `021938C8`, `02194368`, `02194F44`, `02196768`. | Xrefs from investigator c's Ghidra export | blue-verified |
| P2. Whole-floor reveal `0213B460` sets bit 0 on every tile, sets `+0x15FA3 = 1`, and repaints the minimap. Its one caller is `0216249C`. | `ov_0001::0213B460`, `ov_0001::0216249C` | blue-verified |
| P2 cadence: the reveal runs on every step. Only the arrival reveal was checked at runtime, and the write hooks did not catch a write. | Investigator c. No hash. | lead |
| P2. Bit 2 (`0x4`) of the tile word is the trap marker, not visibility. It is written by `SpawnNonEnemies`, `ov_0001::0217D5FC` and `ov_0001::0214C244`, and read by the BG drawer. | `ov_0001::02148B7C` (tests `flags & 4`) | blue-verified |
| P2. The minimap shows a cell as discovered when bit 0 is set or `+0x15FA3 = 1`. It hides entities when `+0x15FA2 = 1` and tints them when `+0x15FA8 = 1`. | `ov_0001::0213CD5C` | blue-verified |
| P2. Entity field `+0x20` is a per-entity visible flag. The trap setter `ov_0001::02193C80` sets it. | `ov_0001::02193C80` | blue-verified |
| P3. The terrain drawer `ov_0001::02148B7C` never tests bit 0. It chooses tile sets from terrain bits `0x1000`, `0x20`, `0x200` and flag bit `0x4`. | `ov_0001::02148B7C` | blue-verified |
| P3. Hypothesis: terrain is not fogged on the upper screen, so dark floors dim nothing in BG. | Follows from the row above. Not tested on a dark floor. | lead |
| P3. Dark floors hide sprites: `ov_0001::02141CC8` (draw predicate) and `ov_0001::02141E58` (leader sees monster) drop monsters, items and hidden traps that fail `0213C4DC` or `0219C6BC`. Team monsters are always drawn. Invisible monsters are drawn only with `+0x15FA7`. | `ov_0001::02141CC8`, `ov_0001::02141E58`, `ov_0001::0213C4DC` | blue-verified |
| P3. Hypothesis: the BG overlay writer `ov_0001::0214814C` (called from menu code, cleared by `ov_0001::021480E4`) picks a visible or dark set per cell with `ov_0001::02141AE0`. It looks like a target-highlight for menus, not fog. | `ov_0001::0214814C`, `ov_0001::02141AE0` | lead |
| P3. Windows: engine A has windows 0 and 1 enabled on the first floor. `+0x15FB2` changes `+0x15FA0` to 2 and moves a window origin through `ov_0001::02019658`. No register value was tied to visibility. | Investigator c's register probe. No hash recorded. See [io.md](io.md). | lead |
| P4. `ov_0001::02180994` is a relation code, not a tile-visibility test. It returns 0 when both arguments are the same entity. It returns 2 for "not a target" (info `+8 == 1`, info `+0x98` of 4 or 1, and the `p3`/`p4` cases). Otherwise it looks up a 24-entry table. | `ov_0001::02180994`; 24 call sites in 15 functions (xrefs.tsv in the export, not listed here) | blue-verified (structure) |
| P4. `02183FF0` returns the leader only when the relation is 0 and the follow test is true. Relation 0 is read as "not an opponent". The meaning of relation 1 against 2 is not known. | `ov_0001::02183FF0`, `ov_0001::02141B48` | lead |
| P4. Monster-to-target visibility is `ov_0001::02141C08(a, b)`: both valid, target `+0x20` set; for monster viewers, fail when the viewer lacks `02180608` and the target is invisible, or when the viewer is blinded; then the in-view test `0219C6BC`. | `ov_0001::02141C08`, `ov_0001::0219C6BC` | blue-verified |
| P4. The follow test `ov_0001::02141B48` ends in `ov_0001::0219C5C4`: the room rect with walls, or Chebyshev distance 0 or 1, or distance 2 with `ov_0001::0219C3CC` (not read). It uses a fixed range, not `+0x15FA1`. | `ov_0001::02141B48`, `ov_0001::0219C5C4` | blue-verified |
| P5. Blinker, info `+0xC9 == 1`: `+0x15FA2 = 1`, so the reveal returns and monsters cannot see (`02141C08` fails for a monster viewer). The minimap hides entities. The status value and its names are in [status.md](status.md#muzzled-transform-invisible-mobile-blinker-189-p2). | `ov_0001::02147DC4`, `ov_0001::02141C08`, `ov_0001::0213CD5C`, `ov_0001::0213C0F8` | blue-verified |
| P5. Info `+0xC9 == 2`: `+0x15FA8 = 1`, which tints every entity on the minimap. | `ov_0001::0213C0F8`, `ov_0001::0213CD5C` | blue-verified |
| P5. Info `+0xC9 == 3`: `02180608` is true (`+0x15FA7 = 1`). The viewer sees invisible monsters and traps, and the invisible-target rule is skipped in `02180994` and `02141C08`. | `ov_0001::02180608`, `ov_0001::0213C0F8` | blue-verified |
| P5. Invisible target, info `+0xC7 == 1`: not seen unless the viewer has `+0xC9 == 3` or holds item `0x1A`. Writer: `ov_0001::0218CE68` (see [status.md](status.md#muzzled-transform-invisible-mobile-blinker-189-p2)). | `ov_0001::02141C08`, `ov_0001::02141E58`, `ov_0001::02141CC8`, `ov_0001::02180994` | blue-verified |
| P5. Held item `0x11` forces `+0x15FA5` and `+0x15FA6` to 1 (the leader sees all monsters and items). Item ids `0x11` and `0x1A` are the investigator's reading; neither is matched to an item name in Blue. | `ov_0001::0213C0F8` | blue-verified for the forcing; id names are a lead |
| P5. Info `+0xCD`, `+0xCE`, `+0xCF` are copied to `+0x15FA5`, `+0x15FA6`, `+0x15FA9`. Their meaning (see-through for monsters, items, stairs) is not checked. | `ov_0001::0213C0F8` | blue-verified (copy) |
| P5. Info `+0x98` in {1, 4} and `+8 == 1` change only `02180994` (the relation), not tile visibility. | `ov_0001::02180994` | blue-verified |

## Addresses

| Address | Name (working label) | Source |
|---------|----------------------|--------|
| ov_0001::02147DC4 | reveal routine | #354 P2, decompile |
| ov_0001::0219C6BC | in-view test (room rect or range square) | #354 P1, P4 |
| ov_0001::0213C63C | fresh-floor reset of the view block | #354 P1 |
| ov_0001::0213C0F8 | per-turn view refresh from the leader | #354 P1, P5 |
| ov_0001::0213AD54 | writes `+0x15FA0` | #354 view block |
| ov_0001::0213B460 | whole-floor reveal | #354 P2 |
| ov_0001::0216249C | caller of the whole-floor reveal | #354 P2 |
| ov_0001::0213C4DC | sprite visibility (lit box, dark-floor test) | #354 P1 |
| ov_0001::0213CD5C | minimap cell repaint | #354 P2 |
| ov_0001::02141C08 | monster-to-target visibility | #354 P4 |
| ov_0001::02141B48 | follow test (AI) | #354 P4 |
| ov_0001::0219C5C4 | room rect or distance test used by `02141B48` | #354 P4 |
| ov_0001::0219C3CC | distance-2 test; not read | #354 P4 |
| ov_0001::02180994 | relation code (target filter) | #354 P4 |
| ov_0001::02183FF0 | leader lookup, relation 0 required | #354 P4 |
| ov_0001::02180608 | sees invisible: info `+0xC9 == 3` or item `0x1A` | #354 P5 |
| ov_0001::02148B7C | BG terrain drawer | #354 P3 |
| ov_0001::0214814C | BG overlay writer for menus | #354 P3 |
| ov_0001::021480E4 | clears the overlay | #354 P3 |
| ov_0001::02141AE0 | room/range test used by the overlay | #354 P3 |
| ov_0001::02141CC8 | sprite draw predicate | #354 P3 |
| ov_0001::02141E58 | leader sees monster | #354 P3 |
| ov_0001::02193C80 | trap setter (sets entity `+0x20`) | #354 P2 |
| ov_0001::02193CC0, ov_0001::0219706C, ov_0001::0219C4F8 | corridor-range users | #354 P1 |
| ov_0001::02019658 | moves a window origin | #354 P3 |
| ov_0001::0218CE68 | invisible setter | [status.md](status.md) |
| SpawnNonEnemies, ov_0001::0217D5FC, ov_0001::0214C244 | trap-marker writers (bit 2) | #354 P2 |

## Routines

Paraphrase only. No decompiler output is copied here.

| Function | Address | What it does (paraphrase) |
|----------|---------|---------------------------|
| reveal routine | `ov_0001::02147DC4` | Marks the leader's room or corridor as discovered. Returns when blinded. Repaints the minimap cells it touches. |
| in-view test | `ov_0001::0219C6BC` | For a viewer and a target: the room rect with walls, or the square of half-width R. |
| fresh-floor reset | `ov_0001::0213C63C` | Copies the darkness setting into the view block, sets the lit flag, clears the whole-floor flag. |
| view refresh | `ov_0001::0213C0F8` | Copies the leader's position and status into the view block each turn. |
| whole-floor reveal | `ov_0001::0213B460` | Sets bit 0 on every tile and the whole-floor flag. |
| sprite box | `ov_0001::0213C4DC` | Screen-size test for drawing a sprite. Dark floors add the in-view test. |
| minimap repaint | `ov_0001::0213CD5C` | Draws one minimap cell from the discovered bit and the view flags. |
| monster visibility | `ov_0001::02141C08` | Whether one entity can see another (status checks, then the in-view test). |
| follow test | `ov_0001::02141B48` | Follow range check used by AI, with its own room-or-distance test. |
| relation code | `ov_0001::02180994` | Returns the relation between two entities (see P4). Not a visibility test. |
| sees invisible | `ov_0001::02180608` | True for an entity with status 3 or holding item `0x1A`. |
| draw predicate | `ov_0001::02141CC8` | Decides whether a sprite is drawn. |
| leader sees monster | `ov_0001::02141E58` | Drops monsters and items that fail the sprite and in-view tests. |
| BG terrain drawer | `ov_0001::02148B7C` | Picks BG tile sets from terrain bits and bit 2. Does not read bit 0. |
| BG overlay writer | `ov_0001::0214814C` | Fills a 6-cell BG overlay for menus, using `02141AE0`. |

## Trace checks

No trace check is recorded for this note yet. Investigator c's runtime probes (the first-floor room rect, the arrival reveal, the window registers) were not saved with a script name or an output hash, so they are not evidence here. Record each one as a `verification/v7-trace/` run with its hash before it counts.

| Script | Output hash | Command | Claims checked |
|--------|-------------|---------|----------------|
|        |             |         |                |

## Red comparison

Not compared in this note. The investigator did not read `pmd-red`'s visibility code. The Red names for the blinker values (blinker, cross-eyed, eyedrops) are in [status.md](status.md). They stay **red-hypothesis** until a Blue check matches them.

## Open questions

- **Room rect asymmetry.** `0219C6BC` tests `x0-1 <= x <= x1` (and y). The wall ring is one tile on the low side only. Check whether the stored `x1`/`y1` already include the far wall before implementing a far-side ring.
- **Bit 1.** Both reveal paths set it. No reader was found, so its meaning ("lit") is not established.
- **Map bounds.** The findings do not say whether the room rect or the range square is clipped to the map.
- **Dark floor and corridor radius.** No dark floor was entered and no corridor crossed at runtime. The dark-floor corridor range is from the static reading only. Why the setting uses only 2 bits is also unknown.
- **Floor-start writer of `+0x15FA3`.** `RunDungeon_Async` is named, but its address is not in the findings.
- **Reveal cadence.** The findings show the callers, not that each one runs on every step.
- **Whole-floor caller.** `0216249C` is the only caller of `0213B460`. What calls `0216249C` was not read.
- **Relation values.** The meaning of relation 1 against 2 in `02180994`, and `0219C3CC`, were not read.
- **Item ids and flags.** Items `0x11` and `0x1A` are not matched to names in Blue. The meaning of the copied flags `+0xCD`, `+0xCE`, `+0xCF` was not checked.
- **Dark floors (P3).** Hypothesis: dark floors hide sprites and leave the upper-screen terrain unchanged. To test: pick a floor whose floor properties have a non-zero darkness setting (from `mapparam`), capture the upper screen and the window registers at one frame, and record a v7-trace run with its hash.
- **Names.** The labels here are working names. No rows were added to [symbols.md](../symbols.md), because `tools/ghidra/RescueApplySymbols.java` renames functions in the Ghidra project from that table. Add them after the names are checked.

## Links

- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
- Related: [ai.md](ai.md) (relation code, relabelled for #354), [status.md](status.md), [io.md](io.md)
- Follow-up (implementation of seen and revealed tiles for a leader with no status): [#407](https://github.com/TorratDev/rust-mm/issues/407)
