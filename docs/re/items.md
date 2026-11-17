# Items: pickup, use, throw, held items, traps, shops

Findings for [#360](https://github.com/TorratDev/rust-mm/issues/360), points P1 to P8. Names and tags are in [`symbols.md`](../symbols.md); status tags are defined in [`workflow.md`](workflow.md#status-tags). Overlay 1 addresses are written `ov_0001::XXXXXXXX`; ARM9 addresses have no prefix. Function names in the export are `FUN_ov_0001__XXXXXXXX`; the names below are ours, and a Red name is a `[red-hypothesis]` label.

**Evidence.** Static only, from the text export `~/rescue-re/export` (ghidra-cli output, see [`ghidra-setup.md`](../ghidra-setup.md#7-text-export-for-read-only-questions)) and a read of `reference/pmd-red`. The investigators did not run Ghidra, and no emulator trace backs any row on this page, so no claim here is trace-confirmed. Every address was checked to exist in the export. A `[lead]` is a pointer, not a fact.

## Shared map

**Action dispatcher: `ov_0001::0218453C`** [blue-verified]. A `switch` on the u16 action id at `info+0x42`. The ids equal Red's `enum DungeonAction` (id match blue-verified; the names are red-hypothesis).

| Action | Red name | Blue handler |
|---|---|---|
| 0x08 | PLACE_ITEM (drop) | `ov_0001::02173CCC` |
| 0x09 | PICK_UP_PLAYER | `ov_0001::02174828` (sets `info+0x46 = 1`, then jumps indirectly towards `021441D8`) |
| 0x0A | swap bag item with held item | `ov_0001::02173A48` |
| 0x0B, 0x27, 0x41 | THROW_ITEM_PLAYER, THROW_STRAIGHT, THROW_ARC | shared arm: `021803B8` check, then `021766A8` and `021732C4` |
| 0x0C, 0x0D, 0x12 | show info, eat/drink, use TM | `ov_0001::0217371C` |
| 0x23 | EAT_AI | thunk to `0217371C` |
| 0x24 | THROW_ITEM_AI | thunk to `021732C4` |
| 0x25 | trap step (action helper) | `02197D88(.., 0, 0)` |
| 0x31 | USE_ORB | `ov_0001::02191A7C` |
| 0x36, 0x37 | GIVE_ITEM, TAKE_ITEM | `ov_0001::02174280`, `ov_0001::021740EC` (not read in detail) |
| 0x38 | USE_ITEM | `ov_0001::02173EF4` (only sets the `+0xD9` flag, see P4) |
| 0x3C, 0x3D | SET_ITEM, UNSET_ITEM | `ov_0001::02174680`, `ov_0001::02174594` (flag `0x10` "L+R throw" marker) |
| 0x3F | PICK_UP_AI | thunk `021853A8` to `ov_0001::02184E50` |

**Action parameter records.** `info+0x46`, stride 6; `ov_0001::02141720(entity, idx)` returns the `Item*` (via `021417CC`). Selector byte: `1..0x14` = bag slot (1-based), `0x80` = the item on the floor underfoot, `0x81` = the held item (`info+0x5A`). Read from `02173CCC`, `02173A48`, `021732C4`, `02141744`, `021415A4` [blue-verified]. `ov_0001::021416F4` reads an entity pointer from `gDungeon+0x11314 + 4*index`.

**Item table (arm9)** [blue-verified]. Entry stride `0x20`; `+0x0C` category (`020082FC`), `+0x18` u16 move id for orbs (`02007670`). Categories match Red's `CATEGORY_*` by use: 0 thrown line, 1 thrown arc, 2 berry/seed/vitamin, 3 food/gummi, 4 held, 5 TM/HM, 6 Poké (money), 8 other, 9 orb, 10 link box. Category 6 is the money branch, 1 the arc throw, 5 the TM branch (read in the code); the other labels are by numeric match with Red [red-hypothesis].

## P1. Pickup, drop, bag, money, floor items

**Bag and limits** [blue-verified]
- Team bag = 20 `Item` slots (3 bytes, bit 1 = exists) at the start of the team inventory (`*0x020DECF8`, see P7). Limit 20 (`0x14`) in loops in `ov_0001::021441D8`, `ov_0001::02142B70`, `02007884`, `02007B68`.
- `02007884` adds to the bag (first slot with bit 1 clear; returns 0 on success, 1 when full). `02007B68` compacts the bag after a removal.
- Stack cap 99 (`qty + qty' > 98` clamps to 99). Only item ids below 9 stack (same id, not sticky, same bit-8 flag); the search prefers the fullest non-99 match and also checks the held-item slot. Ids 1 to 8 are the thrown stacks (category 0 and 1).
- When `gDungeon+0x64C == 0` or the entity is not the leader (`info+6 != 0`), only the held slot (`info+0x5A`) is a candidate, not the bag. What sets `+0x64C` is not traced.

**Entry points** [blue-verified]
- Step-on: `ov_0001::02165B00`, called from `0218766C`. It reads the tile object (`+0x10`) and the entity type: type 3 (item) calls `021441D8(entity+4, 1)` unless the item has sticky bit 2 (then it writes `gDungeon+0x5B8 = 4`); type 2 (trap) goes on to the trap trigger (P5).
- Leader pickup: `ov_0001::021441D8(pos*, silent?)`. Order: `02180B68(entity, 1)` gate (non-zero refuses; meaning not read); category 6 goes to money (`02007350` amount from a table indexed by quantity, `AddToTeamMoney 02007688`, sound `0x14C`, `02143F98` removes the floor item); else the stacking rules; else the first free slot; bag full prints the "cannot pick up" message (`param_2 == 0` suppresses it).
- AI/ally pickup: `ov_0001::02184E50` (action 0x3F), a copy of the same body with guards: tile object exists and is not a trap, `0214067C` false, `info+7 == 0`, `info+8 != 1`, `info+0x40` not `'J'` or `'G'`, `info+0x98 != 1`.
- Money never takes a bag slot. Item id `0x69` is the Poké item; `02142B70` and `020076CC` convert any `0x69` in the bag to money. Which dungeon-exit path calls `02142B70` is not traced. The 99,999 clamp is in `AddToTeamMoney`.

**Drop: `ov_0001::02173CCC`** (action 8) [blue-verified]. Refuses: selector `0x80`, sticky plus marker flags on the bag item, tile terrain `0x200`, tile type `(tile & 3) != 1`, tile already has an object. Otherwise clears item bit `0x10`, calls `SpawnItem 021440B0(pos, item, 1)`, zeroes the slot and compacts. Related: `02173A48` swap with the held item (refuses sticky pairs; writes `info+0x5A..5C`). Consume helpers: `02141680(entity, idx)` (clear the selected item, or remove through `02143F98` for `0x80`), `02141744(rec, i)`, `021415A4` (TM variant).

**Floor items and `numItems`** [blue-verified]. `numItems` is the s16 at `gDungeon+0x3424`. A search of every function containing `0x3424` (only `ov_0001`) gives:
- Readers: `ov_0001::02142504` (per-frame draw/update loop, `for i < numItems`, called from `0213AD54`) and `ov_0001::02193E10` (scatter: collects up to 25 floor items in a 2-tile box and re-places them around the caller).
- Writers: `02144714` (sets 0, floor-gen item placement), `0214BDBC` (ResetFloor), `021440B0` (SpawnItem), `02143F98` (RemoveItemAt: if the tile object is type 3 it clears tile `+0x10`/`+0x12` and the entity type word, subtracts the shop totals (P6) when the tile has bit `0x20`, then sets `numItems = 1 + highest used index` over the 64 slots).
- So `numItems` is a high-water mark, not a count. `02143F98` is the single removal function (pickup, drop-swap, give/take).
- Slot data at `gDungeon+0x3364 + 3*i`; entity pointers at `+0x113B4` ([`structs.md`](structs.md)).

Reconciled with investigator b, who did not find a `numItems` reader: investigator a's search found the two readers above, so the P7 doubt that it is read at all is withdrawn; the high-water-mark part agrees.

Not verified: the money table at `DAT_02007360`, the meaning of `+0x64C`, `02180B68`, other limits in other modes.

## P2. Using an item

- **No handler table: one `switch` on the item id**, `ov_0001::02146880(isThrown, flagB, p3, user, target, item*)` [blue-verified]. The Red equivalent is `ApplyItemEffect` [red-hypothesis]. Callers: `0217371C` (eat/use, `isThrown = 0`), `02144A54` and `02144F3C` (thrown item hits a target, `isThrown = 1`).
- Eat/use flow, `0217371C`: `02141720(entity, 0)`, can-use check `021457B0` (refuses sticky bit 8, or `info+0xCB == 1` with a food-type item `020073CC`), message, `02143C8C`, `02141680` consumes, `02146880(0,0,0,entity,entity,&item)`, then `0219037C`.
- Dispatcher body: a held-item auto-equip branch first (non-leader user, category not 2; `0218163C` gate, `02143BF8`) that can write `info+0x5A` instead of using the item. Then sticky messages and `switch(item.id)`: ids `1..6` go to `021467F4`, `7` and `8` to `021467BC`; categories 5 (TM) and 10 (link box) are handled before the switch; ids `0x35..0x7B` are individual calls (for example `0x36 021455B0`, `0x37 02146774`, `0x38 02146740`, `0x3C 02146570`, `0x3F 0214650C`, `0x40 021463E0`, `0x48 021460A8`, `0x49 02146254`, `0x4E..0x51 02145E88..02145EB8`, `0x52 0x53 0x55 0x67 0x68 0214608C`, `0x56..0x66 02145EC8` with a type index 1 to 17, `0x77..0x7B` five calls, thunks into the `0218xxxx` status code for `0x39 0x3B 0x3D 0x4B 0x4C`, `0xE9 02145E84`). Any other id prints "nothing happens". The case count is 124 labels over about 60 distinct handlers. The id groups match Red's `ApplyItemEffect` [red-hypothesis]; the effect of each handler is not decoded.
- **Orbs** (category 9) skip `02146880`: action 0x31 goes to `ov_0001::02191A7C`, which maps item id to move id (`02007670`, table `+0x18`), builds a move with `InitPokemonMove 0200B568`, and calls `0215B384(entity, 0, itemId, ..)`, which calls `UseMoveAgainstTargets 02156824` with the item id. Orb effects are the move handlers ([`move-effects.md`](move-effects.md)). Ids `0xB6` and `0xBF` take another path (`02141680(.., 0x7A)`, then `0215B384` with flags `|0xC`). Argument order of `0215B384` not read.
- TMs: action 0x12 reaches `0217371C`; `02146880` takes the category-5 branch (teach-move screen, not read).
- Confidence: high on structure, medium on id-to-handler association (by position only).

## P3. Throwing

- **Entry `ov_0001::021732C4(entity)`** (actions 0x0B, 0x27, 0x41 via `021803B8`, and AI action 0x24) [blue-verified]:
  1. Item from `02141720(entity, 0)`. Refuses a sticky item for selector `0x81`, or sticky for category 0/1. `02180B68(entity, 1)` gate.
  2. Consumes one: category 0/1 with quantity above 1 decrements (the bag re-adds via `021440B0`); else the slot is cleared and compacted.
  3. Held items: `HasHeldItem(entity, 0x29)` (Red No-Aim Scope) sets `info+0x44 = DungeonRandInt(8)`; `0x27` (Red Curve Band) sets the arc flag; `0x0F` (Red Pierce Band) sets range 99 and the pass-through flag.
  4. Range from `switch(info+0xC6)`: cases 0 to 2 read (range 10; 99; pierce 99); the rest is cut off in the export [medium].
  5. Category 1 (arc) uses `0219BD04` to pick a target tile and then `ov_0001::02144A54(thrower, item, thrower_pos, target_pos, flags)`; otherwise (line) `ov_0001::02144F3C(thrower, item, thrower_pos, direction, flags)`.
  6. After: `0217ADF8` (face), `0219037C`.
- **Path.** `02144A54` steps the projectile in `12 * (|dx| + |dy|)` sub-steps (12 per tile), with height from `02017A60`. `02144F3C` walks up to 6 tiles per leg along the 8 directions with a wall-edge fixup and stops at a wall or the first monster unless the pierce flag is set.
- **Hit test `ov_0001::0214498C(thrower, target)`** [blue-verified]. Miss when the target has `info+8 == 1` or `info+0x98` is 1 or 4. Otherwise `hit = DungeonRandInt(100) < *DAT_ov_0001__02144A50`, one draw per target per throw; then overrides: thrower holds `0x28` (Red Whiff Specs) always miss, else holds `0x2A` (Red Lockon Specs) always hit, target holds `0x25` (Red Dodge Scarf) always miss. In `02144A54`, a target holding `0x26` (Red Bounce Band) skips the roll and prints the bounce message.
- **Outcome.** `02146880(1, flagB, p3, thrower, target, item)`. `021467F4` is the likely thrown-damage handler for ids 1 to 6 [hypothesis, unread]. An enemy that is hit gets `0217ADBC(target, 8)`.
- **Landing `ov_0001::02143918(thrower, pos*, item, flagOrDirection, offset*)`**. If nothing was hit, the item goes to the end tile (floor type `(tile & 3)` non-zero, not `0x200`, no object), else the ring offsets at `DAT_ov_0001__02143BDC` (list ends at sentinel 99) with the same checks; `021437A8` and `021440B0` place it; with no free tile it disappears. No RNG draw in `02143918` or `021437A8`: landing is deterministic. A trap on the landing tile consumes the item (`02142ADC == 2` branch).
- **RNG summary.** `DungeonRandInt(8)` for No-Aim Scope in `021732C4`; `DungeonRandInt(100)` per target in `0214498C`; nothing else in `02144A54` and `02144F3C`. Whether `02146880`'s thrown branch draws damage RNG is not traced.
- Not verified: the hit-chance constant (read the 4-byte pointer at `ov_0001::02144A50`, then the word it points to); the `info+0xC6` range table; the data source of `021803B8` / `020022A4`; any runtime trace of a throw (a py-desmume hook on `0219D058` during a throw would settle the draw count).

## P4. Held items

- **Storage**: a 3-byte `Item` at `info+0x5A` (flags `+0x5A`, quantity `+0x5B`, id `+0x5C`). **`HasHeldItem` = `ov_0001::021432E8(entity, id)`** [blue-verified]: exists bit set, sticky bit 8 clear, id equal. 60 call sites; 59 (id, function) pairs have a constant id, so one call uses a variable id [lead].
- **Constant ids to callers** (grep over `ov_0001` function files for `021432E8(`; ids blue-verified, Red names by numeric match with `reference/pmd-red/include/constants/item.h` [red-hypothesis]). New beyond [`combat.md`](combat.md): `0x26` in `02144A54`/`02144F3C`, `0x1C` in the trap trigger `02197D88`, `0x2C` in `UseMoveAgainstTargets 02156824`.

| id (hex) | Red name | callers (ov_0001::) |
|---|---|---|
| 0x09 | Mobile Scarf | 02165B00, 02180C98, 02180DF8, 02180FFC, 02181124, 02181264, 0218141C, 02181F30, 021820B0, 021822DC, 02182440, 02182488, 0218453C, 02187420 |
| 0x0A | Heal Ribbon | 02185C70, 021861E8 |
| 0x0B | Twist Band | 02188D8C, 02189534 |
| 0x0C | Scope Lens | 0217BD94 |
| 0x0D | Patsy Band | 0217BD94 |
| 0x0E | Nostick Cap | 02197AF8 |
| 0x0F | Pierce Band | 021732C4 |
| 0x10 | Joy Ribbon | 0217D5FC |
| 0x11 | X Ray Specs | 0213C0F8 |
| 0x12 | Persim Band | 0218E27C |
| 0x13 | Power Band | 0217BD94 |
| 0x14 | Pecha Scarf | 0218A140, 0218A410 |
| 0x15 | Insomniscope | 0218B140 |
| 0x16 | Warp Scarf | 021861E8 |
| 0x17 | Tight Belt | 021861E8 |
| 0x18 | Sneak Scarf | 021906D0 |
| 0x1A | Goggle Specs | 02180608 |
| 0x1B | Diet Ribbon | 021861E8, 0218C284 |
| 0x1C | Trap Scarf | 02197D88 |
| 0x1D | Racket Band | 021906D0 |
| 0x1E | Def Scarf | 0217BD94 |
| 0x1F | Stamina Band | 021861E8 |
| 0x21 | Special Band | 0217BD94 |
| 0x22 | Zinc Band | 0217BD94 |
| 0x23 | Detect Band | 0215A928 |
| 0x24 | Alert Specs | 02160ED8, 02161838, 021629D0 |
| 0x25 | Dodge Scarf | 0214498C |
| 0x26 | Bounce Band | 02144A54, 02144F3C |
| 0x27 | Curve Band | 021732C4 |
| 0x28 | Whiff Specs | 0214498C |
| 0x29 | No Aim Scope | 021732C4 |
| 0x2A | Lockon Specs | 0214498C, 02144A54, 02144F3C |
| 0x2B | Munch Belt | 0217BD94, 021861E8 |
| 0x2C | Pass Scarf | 02156824 |
| 0x2D | Weather Band | 02195C48 |
| 0x2E | Friend Bow | 0217F9B8 |

- **Direct readers of `info+0x5C`** that bypass `HasHeldItem` [lead, unread]: `0216E99C`, `0216D614`, `02169A68`, `021558CC`, `02176D68`, `02160ED8`, `0218579C` (AIDecideUseItem), `02173800`, `021629D0`, `0219E7B8`, `02142DD0`, `02179990`, `02161838`, `02196F84`, `0219ADFC` and others (about 20). A sweep by `HasHeldItem` callers misses these.
- **Auto-equip and use**: the `02146880` equip branch and `02173A48` write `info+0x5A..5C`; `02173EF4` (action 0x38) sets `+0xD9 = 1` on the entity from `021416F4`, which the AI held-item step reads ([`ai.md`](ai.md)). The stat effects of held items are in [`combat.md`](combat.md).
- Confidence: high for constant-id sites, medium for id-to-name mapping (no Blue string table read).

## P5. Traps

**P5a. Who calls the trap writer** [blue-verified, static]. `ov_0001::02198824` (random trap placement) is called from the ARM9 function `RunDungeon_Async 0206DC20` at `0206E440`, an ARM9-to-overlay call, which is why it has no overlay-local xref (`xrefs.tsv`: `0206e440 FUN_0206dc20 UNCONDITIONAL_CALL 02198824`). `0206DC20` is called from `LoadAndRunDungeon_Async 0201DB70` at `0201DC60`. It sits in the non-resume branch: `02179E2C`, then `02198824`, `02144714`, then `+0x684 = +0x67C` and `+0x680 = 0` (P6), then `02152ABC`, `0214809C`. `GenerateFloor 02151E7C` runs earlier in the same loop body. No runtime hook result exists (see Open questions).

**P5b. Step trigger** [blue-verified reading]. A trap is the tile object (tile `+0x10`) with entity type 2 (`02142ADC` returns the type; `02142ACC` returns the 2-byte trap record: byte 0 = trap id, byte 1 = flag). All paths end in the dispatcher `ov_0001::02197D88(entity, pos, a3, a4)`:
- `02165B00` (leader step, from `0218766C`): type 2 with the see-traps effect (`021805B0(e, 0x13)`) and trap visible flag (`+0x20`) 0 sets it to 1, redraws (`02148B7C`) and does not fire. Otherwise, with record byte 1 equal to 0 or 2 it calls `02197D88(leader, leader+4, 0, 1)`; byte 1 equal to 1 does not fire on the leader.
- `02187420` (any monster step, from `0218766C`): same reveal, then fires when byte 1 is 0, the trap is visible and `info+6 == 0`; or byte 1 is 1 and `info+6 != 0`; or byte 1 is 2 and `info+6 == 0`. Which side each byte-1 value means is [hypothesis].
- `0218453C` action helper (action 0x25): `02197D88(e, e+4, 0, 0)`.

Dispatcher shape: reads the object, checks it is a trap, `02198220(id)` (name lookup), sets entity `+0x20 = 1` (seen), prints the sprung message (`0213F50C(0, 0x15C)`, `0213F2F8`), runs `0217BB20(target, flag)` when byte 1 is 1 or 2, then `switch(trap id)`, then `02183BB4` if the target lives, and `021982C0(pos, 1)` for consuming traps (ids 4 and 0x12). When the target is not the leader's team, `a4 != 0` and the id is not 0x11, a `DungeonRandInt(100)` under 15 makes the monster avoid it (message `02154B8C`); with `a3 == 0` that path returns early.

**P5c. Handler map** (switch in `02197D88`; case-to-address blue-verified, names red-hypothesis from Red's `TrapID` order, ids 0 and 8 to 0x13 low confidence). Effects of 0xB to 0x13 are read from callee names only.

| id | handler | effect (from callees) |
|----|---------|-----------------------|
| 0 | `02196F84` | launch: push towards a direction (`0217AED0`, `0217ADF8`, `0214332C`); role unverified |
| 1 | `02197CB8` | mud trap, stat drop ([`rng.md`](rng.md)) |
| 2 | `02197AF8` | sticky: `021432E8(m, 0xE)` guard, one random bag pick `DungeonRandInt(n)` among items with bit 1, not sticky |
| 3 | `02197828` | grimy: item category, a `DungeonRandInt(100)` rate roll, `021448A0(item, 0x54, 2)` changes the item id |
| 4 | `0219752C` | summon: `DungeonRandInt(3)`, `DungeonRandInt(8)`, species `021388D4`, spawn `02179CFC`; consumes the trap |
| 5, 0x1B | `02197684` via `021980A8` | pitfall: `0206D49C`, `02141CC8`, `02196F3C(tile, 0x1B)`, `021778F4`, `0206DB90` (leave floor) |
| 6 | `02197AB4` -> `021938C8` | warp |
| 7 | `021972B4` | gust: `DungeonRandInt(8)` then push `021934D4` ([`rng.md`](rng.md)) |
| 8 | `02197AD8` -> `0218E27C` | spin/confuse family |
| 9 | `02197A70` -> `0218B2D0` | sleep family |
| 0xA | `02197A4C` -> `021882D0` | slow family |
| 0xB | `021972E8` | random status pick (`0200B598`, guard `02180870`) |
| 0xC | `02197A2C` -> `0218A410` | status |
| 0xD, 0xE | `021979FC`, `021979CC` -> `02194BFC` | explosion-style damage |
| 0xF | `02197414` | random pick from a local list (`02175C34`): PP or move effect |
| 0x10 | inline | status for 15 turns via `0217BCA4` |
| 0x11 | `02197404` | tail call through a function pointer slot (wonder tile, indirect) |
| 0x12 | `0219706C` | monster-house style spawn list; consumes the trap |
| 0x13 | inline | status for 10 turns via `0217BCA4` |
| 0x14 to 0x1A | none | empty case bodies |

Placement: `02198824` writes tile `+0x10` and entity `+0x20` (1 only for id 0x11); `02142068` claims the entity; `021983F8` places a given id at runtime (callers `02198330`, `0215FEEC`); `02152FAC` is for fixed rooms.

## P6. Kecleon shops

Names are ours; call and read structure blue-verified, counter meanings [hypothesis] from the arithmetic.

- **State** (`gDungeon`): `+0x688`, `+0x689` leader in shop now and previous (`0219635C`, from terrain bit `0x20` of the leader tile). `+0x65E` stole-from-Kecleon, `+0x65F` set with it (zeroed at floor setup, `0206DC20` line 232). `+0x67C` value of shop-stock items on shop tiles; `+0x680` value of the player's own items on shop tiles (credit); `+0x684` baseline stock value. Debt = `+0x684 - +0x67C`.
- **Item flag bit 2** = shop stock. `02196084` sets it on sold items; `02195E18` and `02195C88` clear it.
- **Enter/leave** `ov_0001::0219635C(a)`, called after movement and warp steps (from `0218766C`, `021657A8`, `02160A7C`, `02172FAC`, `0218F3F8`, `02191A7C`, `021934D4`, `021938C8`, `02193E10`, `02194368`, `02194F44`, `02196768`). Needs a shopkeeper in the room: loops the 16 slots at `+0x11324` with `info+8 == 1` and `021816EC`, same room `02142AD4`. Entering prints a Kecleon greeting (`0215439C`, species `0x17C`). Leaving with debt (`02196320`) calls the pay path.
- **Taking and putting down.** `02143F98` subtracts the price when the tile has bit `0x20` and the item is valid (`0200747C`): from `+0x680` for an own item (price `+8`, `0200816C`) or from `+0x67C` for stock (price `+4`, `020081D0`). `021440B0` adds the same amounts.
- **Paying `02195E18(flag)`**: debt, yes/no prompt (`02153E80`); if money (`*0x020DECF8 + 0x234`) is below the debt it prints a message and returns 2; else sound `0x14C`, `02007688(-debt)`, then clears bit 2 on floor items outside the shop, bag slots and monsters' held items, and resets counters. **Selling `02196084(flag)`**: with credit and a yes, `02007688(+credit)`, then each own item on shop tiles is removed, flagged as stock if sellable, and put back; `+0x684` grows by the stock change. A sold item becomes stock.
- **Hostile `02195C88`** (from `0219635C` and `02196278`, the latter from `02172FAC` at `0217311C`): sets `+0x65E = +0x65F = 1`, prints a message, clears shop bits on floor, bag and held items, clears `info+8` on all 20 slots, calls `02181E58` (speed), zeroes the three counters. Readers of `+0x65E`: `021826A0`, `02181E58` (speed stage +1 for non-team Kecleon, [`turns.md`](turns.md)), `021657A8`, `02172030`, `02172FAC`, `021794CC`, `021623A4`, `021778F4`, `0219706C`.
- **Shopkeepers**: spawned by `02179060(x, y, 0x17C, 0)` at `02152534` ([`dungeon-generation.md`](dungeon-generation.md)).
- Confidence medium. Not read: `02153E80`, `0219CD64`, stacks in `02143F98`, shopkeeper AI while `+0x65E` is 0, the thief-carrying case in `021657A8` and `02160A7C`.

## P7. Team inventory layout

Largely in [`structs.md`](structs.md) ("Team inventory"). Added here:
- Access: u32 at `0x020DECF8` (`gTeamInventoryRef`; 125 reader functions), then offsets from that pointer; total size `0x23C`. A trace reader needs one extra deref.
- `+0x000` 20 `Item` slots (flags bit 1 exists, bit 2 shop stock, bit 8 sticky, bit 0x10 L+R throw flag; quantity; id). `+0x03C` 240 u16 storage counts. `+0x21C` 8 shop items (id, quantity). `+0x22C` 4 ware items. `+0x234` money (s32). `+0x238` savings (s32).
- Dungeon floor items: 64 entity pointers at `gDungeon+0x113B4`, `Item` of entity `i` at `gDungeon+0x3364 + 3*i`, `numItems` at `+0x3424` (see P1).
- In the dungeon the shop code uses the same structure (`02195C88` walks the bag, `02195E18` reads money at `+0x234`): no separate copy.

## P8. Open data points

- **Price direction.** Item record `+4` is what the player pays for shop stock (`020081D0`, `02008298`; feeds `+0x67C`); `+8` is what the shop pays for the player's items (`0200816C`, `02008234`; feeds `+0x680`). So `+4` buy, `+8` sell; the parser already names them `buy_price` and `sell_price` (`crates/data/src/item.rs`). Both getters multiply by quantity for stackable items (`02008504`). [blue-verified] direction from the arithmetic; values not compared. Medium-high.
- **Item byte `+0x1F`.** The export has 26 reads of the item-table pointer (`020DECF4`) in 19 functions. The accessors cover `+0` name, `+4`, `+8`, `+0xC` category, `+0x10`, `+0x14 + i`, `+0x1A`, `+0x1B + i` (min/max stack roll, `020083D4`, `02008458`), `+0x1D`, `+0x1E`; none touches `+0x1F`. [blue-verified, negative] within those reads; overlays could hold a literal copy of the pointer. Medium.

## Open questions

- Hit-chance constant behind `ov_0001::02144A50`, the `info+0xC6` range table, `021803B8`, `02180B68`, `+0x64C`, the money table `DAT_02007360`.
- Per-id effect of the item handlers (`02146880` arms) and of trap handlers 0xB to 0x13; meaning of trap record byte 1 and of `info+6` on the monster-step path.
- The roughly 20 direct readers of `info+0x5C`.
- Runtime: no py-desmume check was recorded for the throw RNG draw count, for `02198824`'s caller, or for the bag layout. Investigator b announced a runtime-check section that was not posted, so nothing here is trace-confirmed.
- Shop prompts and AI (`02153E80`, shopkeeper behaviour).

## Links
- Symbols: [symbols.md](../symbols.md)
- Structs: [structs.md](structs.md); move handlers: [move-effects.md](move-effects.md); RNG: [rng.md](rng.md)
- Verification: [verification.md](../verification.md)
