# Progression: level-up, moves, recruiting, IQ, evolution, quiz

**Date:** 2026-11-20
**Status:** blue-verified (per section tag below; names are hypotheses unless said)
**Subsystem:** dungeon progression, party, ARM9 data tables

Issues: [#361](https://github.com/TorratDev/rust-mm/issues/361) (runtime checks, P1 to P3) and [#75](https://github.com/TorratDev/rust-mm/issues/75) (static P1 to P7). This note merges the investigators' comments on both. Addresses are `ov_0001::` unless plain (ARM9). Static facts come from the Ghidra text export (STAMP 2026-11-14). Runtime facts come from py-desmume on the dump with SHA-1 `6f01887356268de39085df01490347bbcc3aa330`; the probes are scratch files, not in the repo. "Forced" means a probe wrote a register or byte to pass a gate; each forced step is named. Tags: **blue-verified**, **red-hypothesis**, **lead** ([workflow](workflow.md#status-tags)). Table contents are not copied ([`legal.md`](../legal.md)): the notes give addresses, shapes and rules.

## Addresses

| Address | Name | Source |
|---------|------|--------|
| `02001C48` | level-record loader (`lvmpNNN`) | blue-verified |
| `ov_0001::02183BB4` | end-of-action EXP, level-up and evolution pass (`EnemyEvolution` in symbols.md) | blue-verified |
| `ov_0001::021833BC` | per-level loop | blue-verified |
| `ov_0001::02183994` | raise level by N | blue-verified (static) |
| `ov_0001::02182E5C` | move learning, once per level | blue-verified |
| `02001AF4` | moves learned at exactly one level | blue-verified |
| `ov_0001::02182A94` | random move removal (non-team) | blue-verified (static) |
| `ov_0001::0216D77C` | forget-a-move menu | blue-verified (entry at runtime) |
| `ov_0001::0217F9B8` | recruit roll (`TryRecruitMonster` by structure) | blue-verified |
| `ov_0001::0217F388` | join sequence (`MonsterJoinSequence_Async`) | blue-verified |
| `ov_0001::0217F7EC` | recruit-allowed check (friend area) | blue-verified (flow); name red-hypothesis |
| `ov_0001::021B473C` | join flag (byte) | blue-verified (writers) |
| `ov_0001::02178068` | commit recruits at dungeon end | blue-verified (static) |
| `02008EC4`, `02009000`, `02009014`, `02003130` | friend-area count, owned test, owned set, roster insert | blue-verified (static) |
| `ov_0001::02145EC8` | gummi IQ gain | blue-verified (static) |
| `02004464`, `02004250`, `0200429C`, `020042F8`, `0200439C` | IQ unlock test, bit test, default mask, set with group exclusion, toggle | blue-verified (static) |
| `ov_0001::021804CC` | rebuild active skill bits | blue-verified (static) |
| `0200475C`, `0200468C`, `02004590`, `02047D74` | town evolution check, apply, record copy, menu | blue-verified (static) |
| `ov_0001::02182900`, `02001738` | dungeon evolution apply, candidate list | blue-verified (static) |
| `0206C590`, `0206C394`, `0206C978`, `0206C6E4` | quiz question draw, nature pick, partner filter, state machine | blue-verified (static) |

## P1. Level-up [blue-verified; runtime with forced EXP]

- **Where the total is written.** `AddExpPoints` (`02183F78`) only adds to `info+0x1C8` and sets `gDungeon+0xD` ([combat.md](combat.md#defeat-and-experience)). `02183BB4` is the consumer: it runs when `gDungeon+0xD` is nonzero and clears it, loops over the 20 slots at `gDungeon+0x11364`, and for each slot with HP (`+0xE`) nonzero, `+0xD6` zero, `+0x1C8` nonzero and level (`+0x9`) not 100 does `info+0x18 += info+0x1C8`. It then prints the EXP message and calls `021833BC(_, entity, 1, 1)`, runs the evolution branch if `+0x11B` is set (P6.2), and zeroes `+0x1C8` and `+0x11B`. So level-up happens at turn-phase boundaries, not in the damage handler. Callers: 14 sites in the overlay (including `021409E4` and `02140A9C`, the turn loops) and ARM9 `0206E6C8`.
- **Per-level loop** `021833BC`. Returns 0 at once if the info is invalid or `+0x40` is `'J'` or `'G'`. Otherwise, from `L = level`: `L += 1`, stop at `L >= 101`, load the record with `02001C48`, stop when the record's EXP is above `info+0x18`. For each level gained: level = `L`; max HP (`+0x10`) and HP (`+0xE`) both add the HP gain, max HP is capped at 999 (literal at `02183640`), then HP is clamped to max HP; the four stat bytes at `+0x14..+0x17` each add their gain, capped at 255; then `021804CC` (skill bits) and `0218E3E4` (refresh, not read) run; then `02182E5C` (P2). Several levels can come from one call. The species used is `info+2`, the current species. No RNG anywhere. The code is the same for the leader, partners and wild monsters.
- **Loader** `02001C48(out, species, level)`: caches the species in a global; on a change it builds the `lvmpNNN` file name from the species number, loads it, unpacks it, and copies the 12-byte record at index `max(level-1, 0)`. Record layout: EXP total to reach the level (u32), HP gain (u16), then four byte gains, then two zero bytes. This matches `rescue_data::species::parse_level_data` (record `i` is level `i+1`). The byte order Atk, SpAtk, Def, SpDef is read from the order of the adds into `+0x14..+0x17`; the stat names are [red-hypothesis] ([`structs.md`](structs.md) has `+0x14 atk[2]`, `+0x16 def[2]`). Stat at level `n` is the species base plus the sum of the gains for levels 2 to `n` (`02178E9C`, `02178EFC`, `02178F5C`).
- **HP rule.** Max HP and current HP rise by the same gain; there is no heal to full. **Runtime** (forced: the probe wrote a larger EXP total at `021833BC` entry): species 158 level 5 to 6, max HP +4 and HP +4, stats +3, +1, +1, +1; a total in the right range gave two levels in one call, with `02182E5C` once per level.
- **Raise by N** `02183994(_, entity, n, announce, flag)` (wrapper `021465C8`; five more callers `021A19B0`, `021A2480`, `021A2DC0`, `021A3940`, `021A47E0`): level = `min(level + n, 100)`, sets `info+0x18` to the record's EXP, calls `021833BC`, then `02183720` for the stat difference. It is an effect, not the EXP path. [lead] Which item calls it, and the `announce` and `flag` values of each caller.
- **Level down** `02183120` (called by `021836C8`) loops down and subtracts the gains; not needed now.
- **Level 100.** `021833BC` stops at 100, `02183BB4` skips level-100 members, `AddExpPoints` returns at 100, `02183994` clamps at 100. No other cap branch.
- **Sim:** `combat::add_exp` reads `levels[level]` as the next threshold, which matches. Missing in the sim: the gains, the 999 and 255 caps, and the HP bump (shortcut noted in [combat.md](combat.md#defeat-and-experience)).

## P2. Move learning [blue-verified; flow at runtime]

- **Which moves.** `02001AF4(out, species, level, IQ)` returns the moves the species learns at exactly `level` (up to 16). It walks the species' own learnset stream (level byte, then a 1- or 2-byte move id) and does not walk pre-evolutions. Species 0, `0x1A4` and `0x1A5` learn nothing. Four move ids (`0xEE`, `0xEF`, `0x110`, `0x162`) are dropped unless IQ (`info+0xC`) is at least 333 (thresholds read at runtime from the pointers near `02001C0C`). A move the monster already knows is not filtered.
- **Not this function:** `02001844` is the ground-side relearn list (callers `0203F880`, `020402C8`), not the level-up walk.
- **Per level** `02182E5C(_, entity, announce, interactive)`: for a non-team monster (`+6 != 0`) both flags are forced to 0. With `n` moves returned, **one** is learned: `DungeonRandInt(n)` picks it (the draw is made whenever `n != 0`, including `n = 1`) and it goes into the first free slot of an 8-entry local copy of the four moves at `info+0xF4` (6 bytes each), marked new. While the copy holds 5 or more moves (at most 30 passes): a team member (`+6 == 0`, partner included; the test is not "leader") with `interactive` set gets the forget menu `0216D77C(entity, list, 1, 0)` (cancel off; picking the new move declines it); anyone else goes to `02182A94`, which removes a uniformly random move with `DungeonRandInt(count)`, and the removed one can be the new move. The first four entries are written back; a kept new move prints the learned message when `announce` is set.
- **Callers:** `021833BC` only. `0216D77C` is also reached from `02182BD4`. [lead] what `02182BD4` does with it.
- **Runtime (forced setup).** Species 158 learns a move at level 7 (3 moves after); with all four slots filled by the probe, `0216D77C` was entered with `r2 = 1`, `r3 = 0` about 150 frames after the level-up and the game waited for input. `02182A94` was not reached. The non-team removal branch and the menu's result handling are static only.
- **Sim:** one `DungeonRandInt` draw per level with a learnable move, always; one more per forced drop. The forget prompt is a shortcut until the sim has a prompt (use the wild rule). The level-up draws share the dungeon stream with the damage draws, and the pass runs after the turn's actions, so the order matters for traces.

## P3. Recruiting [blue-verified; gates forced at runtime]

- **Call site.** `0217D5FC` calls `0217F9B8(attacker, target)` at `0217E814` after the target would faint. The attempt is set only when the attacker is a team member and the target is not, and is cleared when the attacker is a non-leader team monster, so only the leader's KO can recruit (static, medium: the branch order was read, not traced). `0217F9B8` returns 0: normal faint (`0217E858`). Nonzero: the join `0217F388` at `0217E82C`.
- **Join result.** After the call returns (see "Address check" below): nonzero stores 1 to the join flag and the target does not faint here; zero faints the target with reason 501 (`DUNGEON_EXIT_LEFT_WITHOUT_BEING_BEFRIENDED` in Red, [red-hypothesis] for the name).
- **Gates, in order** (a failed gate returns 0 and draws no RNG):
  1. Fixed-room number (`gDungeon+0x34B4`) 4, 5, 9 or 15 passes; `0x2C` to `0x30` needs `gDungeon+0x64D`; `0x31` needs `+0x64A` and an event-flag test (`02010B1C`); otherwise the per-dungeon byte read by `02006400(gDungeon+0x63C)` (`recruitingEnabled`, dungeon record byte `+2`) must be nonzero. **Runtime:** this byte is 0 for the first dungeon, so no probe could recruit there naturally.
  2. `gDungeon+0x64E` must be 0.
  3. About a dozen special species (a literal list near `0217FB28`) fail if already in the roster (`020012C4`: scan of the 0x40-byte records, bit 0 valid, species at `+6`). This is the only "already owned" gate; ordinary species can be recruited again.
  4. `0217F7EC(species)` nonzero: `gDungeon+0x649` nonzero; `0219F9C4(species)` nonzero; a few species always fail (and `0x97` when `+0x668 == 1`); and the friend-area test: if species byte `+0x12` is nonzero it passes, else the species' area (`+0x16`) must be owned (P4). Names are [red-hypothesis]; **runtime:** it returned 0 for species 290 natively (`+0x649` was 0).
  5. Adjacent on both axes (`|dx| <= 1`, `|dy| <= 1`, diagonals count).
  6. Target `+0x40` is not `'J'` and `+0x98` is not 1.
  7. `02141C08(target, recruiter)` nonzero (reach/visibility: refuses on the status bytes `+0xC7`, `+0xC9`, then a same-room test). Runtime: 1 natively.
  8. `0217F8E4()` runs here, before the roll: it rebuilds the 6-cell formation array (`021B4250`) from the four team slots. It has a side effect even if the roll fails.
- **Roll.** `DungeonRandInt(1000)` (0 to 999), drawn once whenever gates 1 to 7 passed, even if the species is unrecruitable. `rate` is the species recruit rate (record `+0x40`, s16, per mille via `020021B0`); `-999` fails after the draw. Add 100 (s16 at `021AA1EC`) if the recruiter's held item is `0x2E` (held, not sticky; "Friend Bow" is [red-hypothesis]). Add `gRecruitRateByLevel[recruiter level]` (s16 array at `021AA96C`, 101 entries, rising in steps from 0 at low levels; the index is `info+9` of the **recruiter**, which settles the data-tables hypothesis; runtime log: recruiter level 5). Success iff `rate_total > roll`, so the chance is `clamp(rate_total, 0, 1000) / 1000`.
- **Capacity gates** (after the roll; a failure returns 0): the target's size (species byte `+9`) must fit a run of free cells in the 6-cell array; a free entry among the four 0x54-byte party records at `+0x6740` (bit 0 clear); not all four team entity slots (`gDungeon+0x11314`) live. So the in-dungeon team cap is 4 entities, leader included. `0217F254` (caller `021A8CD4`) is the same gate set without the roll.
- **Join** `0217F388(recruiter, target, record)`, `record` being the 0x44-byte snapshot from `02176D68`. Order: face the recruiter; prompt `02153E80` (decline: return 0, so the caller faints the target with 501); re-check a free party slot; set the area owned (`02009014`, so the join **unlocks the area**); `HandleFaint_Async(target, 500, recruiter)` removes the wild monster (reason 500); fill the party record (species `+0xC`, `+8` = `0xFFFF` marks "recruited this run", IQ `+6` = 1, default skill bits via `0200429C`); `02179990` spawns the ally (failure: message, slot cleared, return 1); a second prompt for the nickname (`02168240` keyboard loop, [red-hypothesis] for the role); the "joined" message; species `0x97` also sets `gDungeon+4 = 1` and `+0x11 = 3`. Returns 1.
- **Runtime** (gates forced: `+0x649` = 1, the table byte at `0217FA74` = 1, `0217F7EC` = 1, `02141C08` = 1, roll = 0; the natural draw was 683): `0217F9B8` returned 1, `0217F388` ran for species 290, prompt 1 returned 1, `HandleFaint_Async` ran for the target, `02179990` returned 1, prompt 2 was forced to 0 (the keyboard would block), and the join returned 1 about 400 frames later. **Not seen:** a natural recruit.
- **Join flag** (`021B473C`). A write watch saw a store of 1 right after the join returned 1, and a store of 0 four frames later in `0218453C` (per-turn code; callers `021408F8`, `02140AC0`; static store at `021846FC`, the watch pc `02184704` is the instruction after the store). A third writer, `0215BE14`, also clears it (store at `0215BF14`, static only, not seen at runtime) and reads it at `0215BFFC`. Other reader: `0215B384` (`0215B7E8`). It means "a monster joined this turn"; the runtime watch saw the clear in `0218453C`, and `0215BE14` can clear it too. [blue-verified] writers and timing; the readers' purpose is a [lead].
- **Address check (the 501 vs flag discrepancy).** The export has, in `0217D5FC`: the join call at `0217E82C`; a literal load at `0217E834` from `0217E888` (the flag pointer); a **write** at `0217E83C` (the flag store); a conditional jump at `0217E840` to the common exit `0217E87C` when the join returned nonzero; a literal load at `0217E844` from `0217E88C` (501); the faint call at `0217E850`; the plain faint call at `0217E864`. So the flag store is `0217E83C` and the 501 reason is loaded at `0217E844`. The runtime watch reported `0217E844` as the store's pc because it fires after the store, with the program counter already past it (an inference from the two sources agreeing, not a separate check). [combat.md](combat.md#defeat-and-experience) now reads this way. Return address `0217E868` is the plain faint at `0217E864`.

## P4. Friend areas [blue-verified, static; table read at the title screen]

- **Species to area.** Species record byte `+0x16` (`020022DC`) is the area id (0 to 57). Eleven species have byte `+0x12` nonzero: their area need not be owned to recruit. Meaning [red-hypothesis] "recruitable without the area".
- **Area table** at `02092138`: 58 entries of 8 bytes, `s16 capacity`, `s16 field2` (0 to 3, unidentified), `u32 field3` (zero or a round number; looks like a price, [red-hypothesis]). Capacities sum to 413, the roster size. Readers `02008EC4` and `02009230`.
- **Roster.** 413 entries of 0x40 bytes at `*020DDF84`, partitioned by area (area `a` owns `capacity[a]` slots after the running sum). Entry bit 0 valid, `+4` = `'A'` special, `+6` species. Owned flags: 58 bytes at `*020DED04`; `02009000` reads, `02009014` sets. The starting set of owned areas was not traced.
- **Where a recruit goes.** In the dungeon it is only a 0x54 party record. At the end of the dungeon `02178068` builds a per-area list: with room, each recruit is inserted with `02003130`; with a full area, a selection screen (`02175178`) makes the player release monsters (old ones or the recruit), then the survivors are inserted. `02003130` adds nothing if the area is not owned or its block is full, and marks the species seen (`020115BC`, [red-hypothesis]). A guard on entries with `+4` equal to `0x44` or `0x45` re-prompts; it is not understood (low).
- **Sim:** needs the area table parser (58 x 8 bytes, test: capacity sum 413), an owned set, and the dungeon-exit flow; not modelled.

## P5. IQ [blue-verified, static]

- **Stores** (all s16): roster record `+0x0A` (10 bits in the save), party record `+0x06`, entity info `+0x0C`. New hero and partner (`02003510`) and a recruit start at IQ 1. The ally spawn (`02179990`) copies party to entity; `02179128` writes back.
- **Only gain: gummis** `02145EC8(user, target, gummiType)`. For a team member whose `+0x40` is not `'J'` or `'G'`: `IQ += T[type1][gummi] + T[type2][gummi]` over the target's two types (`info+0x56`, `+0x57`), clamped to 1 to 999; then `021804CC` and `02175E60` (messages for newly unlocked skills). The IQ table is 18 x 18 s16 at `02092308`; a second table at `02092590` (same shape) drives belly. Tables not copied. Level-up, evolution and recruiting do not change IQ.
- **Unlock** `02004464(iq, skill) = skill != 0 && threshold[skill] <= iq`, `threshold` being 24 words at `02091F48`. `02004404` lists all unlocked skills. Skill names are [red-hypothesis] except index 1, which matches the Type-Advantage Master note in [combat.md](combat.md).
- **Correction to [`structs.md`](structs.md):** a set bit in `+0x84..+0x86` means the skill is **enabled** (the player's on/off choice), not blocked. Evidence: `021804CC` sets bit `n` of `+0x87` when `02004464(IQ, n)` and `02004250(info+0x84, 1<<n)` are both true; the default writer `0200429C` sets bits 2, 3 and 22 (and 18 if its flag is nonzero); `0200439C` toggles. The same 24 bits are the roster `+0x18` and party `+0x42` fields shown by the IQ menu.
- **Active skills** `+0x87..+0x89` = unlocked AND enabled, rebuilt from scratch by `021804CC` (callers include the gummi, spawn `02179990`, `021833BC`, `02183120`). Wild monsters ignore IQ: a fixed set of bits, plus bit 18 when `+0xD4` is nonzero and bit 22 when the level reaches a cap read from `021AA1F8`.
- **Exclusive groups.** `020042F8(p, n)` sets bit `n` after clearing every bit with the same group id; the group array (24 words) is at `0208FA84`. Seven groups of two to four skills; the others stand alone. Enabling one skill switches off its group mates.
- **Partner at start:** IQ 1, enabled and active {2, 3, 22}; skills 21 and 23 are unlocked at IQ 1 but off by default.
- **Not verified:** skill names, whether a gummi IQ persists to the roster at dungeon exit (party-to-roster copy not traced), the meaning of `+0x40` `'J'`/`'G'`, the flag argument of `0200429C` at the roster-creation sites.
- **Sim:** per member IQ (1 to 999) and a 24-bit enabled mask; derive the active bits when either changes.

## P6. Evolution [blue-verified, static; no runtime run]

### P6.1 Town evolution
- **Data.** Species record (0x48 bytes): `+0x34` pre-evolution species (s16), `+0x36` method (u16), `+0x38` parameter (s16), `+0x3A` extra condition (u16); `020020C8` copies the four. Methods: 1 level, 2 IQ, 3 item, 13 none (needs only the extra condition's item); method 0 with a pre-evolution is only produced by the Ninjask case. In `crates/data/src/species.rs` the names are stand-ins: (pre, method) and (param, cond).
- **Check** `0200475C(rosterRec, out, hasItemsGiven)` on the roster record, for each species `s` whose pre-evolution is the member and whose method is nonzero: (1) the area of `s` must be owned (else flag `0x20`) and have room (count below capacity, the member's own slot freed when the area is shared; else `0x40`); (2) method 1 blocks below the level (flag 2), method 2 blocks below the IQ (`0x10`), method 3 needs the item among the two offered or in the 20-slot bag (`02007AF8`; else `0x08`); (3) the extra condition: 4 needs one item, 8, 9 and 10 each another, 5 needs attack above defense, 6 defense above attack, 7 equal (the Tyrogue line), 11 and 12 depend on a random byte (`out[2] & 1`, from `02011968(0xFF)` drawn at each menu check on the ARM9 generator, so re-rolled at each check; the Wurmple line); (4) unblocked sets flag bit 0 and the target; the last unblocked species wins. No candidate gives flag 4.
- **Apply** `0200468C` then `02004590`: copies the record, sets EXP to the table value of the new species at the same level, writes the new species, records the evolution level at `+8` (or `+9` if `+8` is used; `020017D0` reads these for the learn chain), renames only when the nickname equals the old species name, inserts through `02003130`, clears the old slot, removes the offered items from the bag. Level, IQ, skill bits, moves and stats carry over; no stat recompute was seen. **Ninjask** (species 316) also builds a Shedinja (317) record if a roster slot exists.
- Menu: state machine `02047D74` (entered from `020485D8`). Whether it is the "Luminous Spring" is a [lead] (no string check). Item id to name mapping for the condition items: not done.

### P6.2 Dungeon evolution (wild monsters)
- **Trigger.** `0217D5FC` sets `info+0x11B = 1` and `gDungeon+0xD = 1` when the attacker is **not** a team member and the dungeon record byte `+1` (`02006130(gDungeon+0x63C)`) is nonzero. `02183BB4` then acts if `gDungeon+0x10 == 0`. Team members reach this branch only through an item or story effect that sets `+0x11B`. [lead] Which dungeon rows allow it (the table has nonzero bytes in later rows; the row-to-dungeon-name mapping was not done).
- **Target.** `02001738(species, buf, 0, 0)` lists species whose pre-evolution is the current species and, with `param_3 == 0`, whose body size (species byte `+9`) equals the current one; it does not check method, level, IQ or item. With `n` candidates, one `DungeonRandInt(n)` draw picks a start, then a forward scan with wrap finds the first candidate that is in the floor's species list (`021389A0`) and has a nonzero entry in the per-species table (`02178C04`). The scan draws nothing. [blue-verified, disassembly for the argument]
- **Apply** `02182900`: `info+4 = info+2 = c`, EXP set to the table value, `0217930C` re-derives types and abilities, animation and messages; level, HP, IQ and moves stay. Target 316 also spawns 317 next to the entity when 317 is on the floor list. A team member gets its stat deltas (`02183720`) afterwards.

## P7. Personality quiz and starter [blue-verified, static]

- **State machine** `0206C6E4` (details in [`rng.md`](rng.md), #170). The tracker (200 bytes) is the global at `020E077C`. Eight questions are asked: `+0x40` counts up and the ninth pass opens the gender menu. Case 4 reveals nature and starter; cases 5 to 14 are dialogue, partner menu, nickname, end.
- **Questions.** 55 records (plus a bonus record at `020B84F4`, index 55) behind the pointer array at `020B9848`. Record (12 bytes): text id (u16), zero (u16), answer menu pointer, effects pointer. Answer entries are 12 bytes, ended by a sentinel; the value is the effects row index. Effects: `rows[value][13]` bytes with **stride 13** (this settles the "13 or 16" open point in rng.md). Category table `020B970C` (55 bytes, 14 categories): once a question is picked, every question of its category is marked used, so the 8 questions come from 8 distinct categories. The pick is `RandInt(0x37)` in a retry loop until the flag is not 1.
- **Bonus question.** An answer value of 99 (one answer of question 9) sets `+0x44 = 0x37` and returns to state 1 without counting a question or adding effects; the bonus record then plays.
- **Score and nature.** `totals[i] += effects[value][i]` for 13 words at `+0x4C`. `0206C394`: `n = RandInt(13)`, then 12 steps of `u = (u+1) % 13`, `n = u` when `totals[n] < totals[u]` strictly. The maximum wins; a tie goes to the candidate met first walking up from the random start, so the draw is only a tie-break.
- **Gender** is asked (a menu after the 8 questions), stored at `+0xB8` and in the options byte; it is not derived.
- **Starter** is the u16 at `020B96D8 + nature*4 + gender*2` (13 natures by 2). **Partner:** 10 candidate species at `020B993C`, filtered by `0206C978` to drop any candidate that shares a non-zero type (species record bytes `+0x13`, `+0x14`) with the starter; the starter is always excluded. The choice is written to `+0xA` of the tracker. Tables not copied.
- **RNG.** Both draws are the global generator (`RandInt`, `02011968`), not `DungeonRand`. The state at quiz time depends on every earlier draw, so a sim needs a seed state to match a retail result. Case 15 also reseeds the Mersenne Twister (not used by the result).
- **Not verified:** runtime run; where the tracker's starter (`+8`) and partner (`+0xA`) go next (roster creation); the leader's and partner's starting level, moves and IQ (IQ 1 is P5); personality names.

## Red comparison
The recruit gate order, the roll shape, the exclusive IQ groups and the evolution fields follow Red's structures; every Red name above is [red-hypothesis]. Two differences were read in Blue: the level term indexes the recruiter's level, and the in-dungeon team cap comes from four slots rather than a setup value.

## Open questions (follow-ups)
- A natural recruit from a savestate in a dungeon with recruiting enabled (record 1 or later), to observe gates 1 to 8 and the roll without forcing.
- Level-up with natural EXP, and the stat gains against `parse_level_data` record by record.
- The non-team random removal `02182A94` and the forget menu's result at runtime.
- Callers of `02183994` and their flags; `02182BD4`; the readers of the join flag.
- Dungeon evolution: the dungeon rows that allow it, and a runtime trace.
- Town evolution runtime, condition item names, and whether the menu is the Luminous Spring.
- Roster-creation consumers of the quiz result and the starting level, moves and IQ.
- Initial friend-area ownership, the dungeon-end commit at runtime, and area table fields 2 and 3.
- IQ persistence to the roster at dungeon exit; skill names.

## Links
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
