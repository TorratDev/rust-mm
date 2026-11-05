# Dungeon state structures

Layout of the dungeon struct, the entity struct and the entity info struct in Blue, compared field by field with `pmd-red`'s `include/structs/str_dungeon.h` and `include/structs/dungeon_entity.h`. Issue: [#46](https://github.com/TorratDev/rust-mm/issues/46). Function names and addresses are in [`symbols.md`](../symbols.md).

**Method:** a field is listed only when a Blue function already matched to its `pmd-red` counterpart (same calls, constants and arguments) reads or writes it at that offset. The Evidence column names that function. Fields not listed are unknown in Blue: do not assume Red's offset, because Blue's structs drop bytes at several points. The fields read by the V7 trace (`gDungeon`, `fractionalTurn`, the seeds, `teamPokemon`/`wildPokemon`, entity `type` and position, info `id`, `level`, `HP`, `maxHPStat`, `speedStage`) decode to consistent values on the retail ROM by emulator (V7 in [`verification.md`](../verification.md#v7--scriptable-ds-emulator-for-ram-and-rng-dumps)); the rest is checked in code only.

Tags: **[blue-verified]** unless marked *probable* (one access, consistent with its neighbours) or *red-hypothesis*. A Blue function name taken from `pmd-red` is *red-hypothesis* on its own; the offsets the function reads or writes can still be blue-verified.

## Entity

Same size as Red, 0x74 bytes, and the same offsets wherever checked; Red's `Entity` is the working hypothesis for the rest. The routine `021428F8` sets the team, wild, item and trap entity pointers 0x74 apart. It does not allocate the infos.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x00` | `type` (0 = no entity) | `0x00` | `EntityIsValid` |
| `+0x20` | `isVisible` | `0x20` | `SpawnItem` |
| `+0x70` | `axObj.info` (pointer to the entity info; for an item entity, its `Item`) | `0x70` | `GetLeader`, `AbilityIsActive`, `GetItemInfo`, ... |
| `+0x04`, `+0x06` | `pos.x`, `pos.y` (s16) | `0x04` | `0219082C` copies them into the info's `aiTarget.aiTargetPos`; `021563CC` passes `entity+4` as a position |

## Entity info

Pointed to by `entity+0x70`. Blue's struct is shorter than Red's (0x208): bytes are missing before the types, in the status block, around the speed counters and before the belly. The **Shift** column is Blue offset minus Red offset.

**Size: 0x1D4 (468 bytes), measured in the emulator (#163 P1; blue-verified, runtime).** In a dungeon at its start, the team and wild info pointers (`entity+0x70`) are all multiples of `0x1D4` from base `0x02113FB0`: the five occupied sampled slots are at offsets 0, `0x1D4`, `0x750`, `0x924` and `0x0AF0`, and team members 2 and 3 are empty. Ten slots (k = 0 to 9) were checked as `base + k*0x1D4`: each holds a plausible record (species at `+0x2`, level at `+0x9`, HP at `+0xE`, max HP at `+0x10`), and empty slots are zero. Two runs gave byte-identical output. The stride is the only value in the shift table's bound (`0x1C8` to `0x208`, blue-verified) that divides the measured offsets. Red's size is `0x208`, which was not re-measured. The array's placement and its writers are in the next subsection (#163 P2). **Not verified:** whether every record fills all `0x1D4` bytes (only the fields above were checked); other floors and species, and the slot count (slots 0 to 39 were read, only 0 to 6 were occupied); the rule that a record size equals the array stride in general (it rests on the array being a plain array).

| Blue | Red field | Red | Shift | Evidence |
|------|-----------|-----|-------|----------|
| `+0x02` | `id` (species) | `0x02` | 0 | `CalcSpeedStage`, `TickStatusAndHealthRegen` |
| `+0x04` | `apparentID` | `0x04` | 0 | `CalcSpeedStage`, `CalcDamage` |
| `+0x06` | `isNotTeamMember` | `0x06` | 0 | `CalcSpeedStage`, `CalcDamage`: selects the level path, `ldrb r0,[r5,#6]` at `0x0217c270` in `ov_0001::0217BD94` (#185, P3, P6) |
| `+0x07` | `isTeamLeader` | `0x07` | 0 | `GetLeader`, `CalcDamage`: first condition of the damage-1 test, `byte[info+7] == 0` (#185, P3) |
| `+0x09` | `level` | `0x09` | 0 | `CalcDamage`: `level << 1` divided by 3 on the non-team-member path (`0x0217c2a8`, `0x0217c2b0`) (#185, P3, P6) |
| `+0x0C` | `IQ` (s16; Red `IQ`) | `0x14` | −8 | Writers: `021794CC` sets 1 for every new monster (callers `02179990`, `02179CFC`, `02179874`); `02179990` (ally spawn) copies it from the record `param_4+6` (the source of `param_4+6` is not traced); `02145EC8` (gummi IQ gain) adds the two type tables and clamps to at least 1 and at most a cap read from the pool at `0214607C` (the value was not read). Readers: `02182E5C` passes it to `FUN_02001AF4` (`GetMovesLearnedAtLevel`) as the learn gate, where 333 is an IQ threshold and not a level; `021804CC` derives the IQ-skill bits at `+0x87`; `02175E60` diffs the skill list before and after a gummi. [blue-verified by structure; the name is hypothesis; medium for the completeness of the writer list] |
| `+0x0E` | `HP` | `0x0E` | 0 | `TickStatusAndHealthRegen` |
| `+0x10` | `maxHPStat` | `0x10` | 0 | `TickStatusAndHealthRegen` |
| `+0x14` | `atk[2]` (u8, read at `+0x14 + splitIndex`) | `0x14` (*red-hypothesis* layout) | 0 | `CalcDamage` (`ov_0001::0217BD94`): base power, `gDungeon+0x140 = movePower + byte[+0x14 + splitIndex]`. Blue offset measured on 14 captured hits (#185, P5) |
| `+0x16` | `def[2]` | `0x16` | 0 | `CalcDamage` (`ov_0001::0217BD94`): target defence, `gDungeon+0x142` (#185, P5) |
| `+0x18` | `exp` | — | — | `AddExpPoints` (`ov_0001::02183F78`) adds to it (blue-verified structure) |
| `+0x1C` | `offensiveStages` (s16 at `+0x1C + 2 × splitIndex`, clamped to 0 to 20) | `0x1C` (*red-hypothesis* layout) | 0 | `CalcDamage` (`ov_0001::0217BD94`), stored at `gDungeon+0x13E`. Blue offset measured on 14 captured hits (#185, P5) |
| `+0x28` | `flashFireBoost` | `0x28` | 0 | `CalcDamage` (*probable*) |
| `+0x2C` | `offensiveMultipliers` (s24.8 at `+0x2C + 4 × splitIndex`) | `0x2C` (*red-hypothesis* layout) | 0 | `CalcDamage` (`ov_0001::0217BD94`). All 14 captured hits had `0x100` (1.0) (#185, P5) |
| `+0x34` | `defensiveMultipliers` (s24.8 at `+0x34 + 4 × splitIndex`) | `0x34` (*red-hypothesis* layout) | 0 | `CalcDamage` (`ov_0001::0217BD94`). All 14 captured hits had `0x100` (1.0) (#185, P5) |
| `+0x3C`, `+0x3E` | `hiddenPower` base power, type | `0x3C` | 0 | `GetMovePower`, `GetMoveTypeForMonster` |
| `+0x40` | dungeon location of the join | — | — | `AddExpPoints` compares it with `'J'` and `'G'` for the EXP lock, in place of Red's `IsExperienceLocked` (lock not confirmed) |
| `+0x42` | `action` block (*probable*: its `direction` is at `+0x44`) | `0x44` | −2 | `ov_0001::02188030` (`RunMonsterAI`): byte compared with 0, then written |
| `+0x44` | `direction` (u8) | `0x46` | −2 | `DisplayActions_Async` (`ov_0001::0218766C`) compares it with the direction from `FUN_0201187C` (the helper's name `GetDirectionTowardsPosition` is *red-hypothesis*) |
| `+0x56` | `types[2]` | `0x5C` | −6 | `sub_806F370_Async`, `sub_806F4A4` |
| `+0x58` | `abilities[2]` | `0x5E` | −6 | `AbilityIsActive` |
| `+0x5A` | `heldItem` (3-byte `Item`: `flags` `+0`, `id` `+2`) | `0x60` | −6 | `HasHeldItem` (flag `1` exists, `8` sticky) |
| `+0x70` | `aiTarget` (`aiTargetPos` `+0x10` = info `+0x80`, written by `0219082C`; `aiObjective` `+0`, `aiTargetSpawnGenID` `+4`, `aiTarget` `+8` are written by the same unnamed writer; readers partly found, see [aiTarget readers](#aitarget-readers-164); blue-verified (writer), offsets high (#164 P2b)) | `0x78` | −8 | `0219082C` (blue-verified writer; unnamed in Blue) |
| `+0x71` | `aiNotNextToTarget` (inside `aiTarget`; blue-verified by structure, #164 P1b; the name is red-hypothesis, the offset medium) | `0x79` | −8 | `ov_0001::02188030` (`RunMonsterAI`) clears it when the action is above PASS_TURN (blue-verified write); `ov_0001::02190FEC` (`DecideMovement`) sets it |
| `+0x87` | IQ skills, one bit per skill (bit *n* is tested with mask `1 << n`). Derived from IQ (`+0x0C`) and the blocked mask at `+0x84` by `021804CC` (see below) | — | — | `ov_0001::021805B0` (`IqSkillIsEnabled`) reads it. `021804CC` is its sole writer besides `020042F8` bit-sets. Bits 4, 5 and 8 are used in the hit and AI paths |
| `+0x8A` | `tactic` | `0x94` | −0xA | `IsTacticSet` |
| `+0x99`, `+0x9A` | `sleepClassStatus` status, turns | `0xA8` | −0xF | `TickStatusAndHealthRegen` |
| `+0x9B`, `+0x9C` | `burnClassStatus` status, turns | `0xAC` | −0x11 | `CalcSpeedStage` (paralysis), `TickStatusAndHealthRegen` |
| `+0x9D` | burn damage countdown | — | — | `BurnedStatusTarget` (`ov_0001::0218A710`) sets it to 0; `DoEndOfTurnEffects_Async` (`ov_0001::021861E8`) decrements it and resets it to 20 |
| `+0x9E` | bad-poison damage index | — | — | `BadlyPoisonedStatusTarget` (`ov_0001::0218A140`) sets it to 0; `DoEndOfTurnEffects_Async` (`ov_0001::021861E8`) increments it up to `0x1D` |
| `+0xA0` | `frozenClassStatus.status` | `0xB0` | −0x10 | `TickStatusAndHealthRegen` |
| `+0xA8` | `frozenClassStatus` counter | `0xB8` | −0x10 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`; decremented by `FUN_ov_0001__0219bf1c`) |
| `+0xAC`, `+0xAD` | cringe status, counter (*red-hypothesis* names; the class is guessed from the shift). Written by the confusion applier: `+0xAC = 2` (`STATUS_CONFUSED`) at `0x0218E2D0`, `+0xAD = turns + 1` at `0x0218E2D8` (blue-verified write; `ApplyConfusion` is red-hypothesis) | `0xBC`, `0xBD` | −0x10 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) only. Writers in `ov_0001::0218E27C` only; others not established (see Open questions) |
| `+0xAE` | `bideClassStatus.status` | `0xC0` | −0x12 | `CalcDamage` |
| `+0xB1` | `reflectClassStatus.status` | `0xC4` | −0x13 | `CalcDamage`, `TickStatusAndHealthRegen` |
| `+0xB2` | `reflectClassStatus` counter | `0xC5` | −0x13 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) |
| `+0xB3`, `+0xB6` | curse status, counter (*red-hypothesis* names; the class is guessed from the shift). No write site found in reach (see Open questions) | `0xC8`, `0xCB` | −0x15 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`), and `RunMonsterAI` (`ov_0001::02188030`, decoy test, blue-verified read) |
| `+0xB8`, `+0xC1` | leech seed status, counter (*red-hypothesis* names; the class is guessed from the shift). Cleared by `ov_0001::0218F610` (`+0xB8 = 0` at `0x0218F63C`, then `+0xC0 = 0xFF`; blue-verified write). No nonzero write to `+0xB8` and no write to `+0xC1` found | `0xD0`, `0xD9` | −0x18 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`), and `ov_0001::0218F610` (switch on `+0xB8`, messages for 1 and 2) |
| `+0xC4` | `sureShotClassStatus.status` | `0xDC` | −0x18 | `CalcDamage` |
| `+0xC5` | `sureShotClassStatus` counter | `0xDD` | −0x18 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) |
| `+0xC7` | mobility status checked against 3 (STATUS_MOBILE); Red `invisibleClassStatus.status` is inferred from the value, not checked | not matched | — | `CanMoveInDirection` (`ov_0001::0218141C`) |
| `+0xC9`, `+0xCA` | invisible class, status and turns ([hypothesis] for the class name; blue-verified write sites). Setters `ov_0001::0218BBB4` (status 1), `ov_0001::0218BAC8` (status 2), `ov_0001::0218B9F4` (status 3); turns = the turn roll + 1 in each | not matched | — | `FUN_ov_0001__0218F304` (`ov_0001::0218F3BC`, switch and clear), `FUN_ov_0001__0217930C` (reset, with `+0xCB`..`+0xCC`) |
| `+0xCB`, `+0xCC` | blinker class, status and turns ([hypothesis] for the class name; blue-verified write sites). Setter `ov_0001::0218C1AC` (status 1), turns = the turn roll + 1 | not matched | — | `FUN_ov_0001__0218F27C` (`ov_0001::0218F2EC`, read and clear), `FUN_ov_0001__0217930C` (reset, with `+0xC9`..`+0xCA`) |
| `+0xD5` | `speedStageChanged` | `0xF8` | −0x23 | `RunLeaderTurn_Async` |
| `+0xD8` | `expMultiplier` (`0` halved, `1` regular, `2` boosted) | — | — | `AddExpPoints` (`ov_0001::02183F78`); `0` and `1` seen at runtime, `2` not |
| `+0xDD` | `decoyAITracker` (blue-verified by structure, #164 P1b; the name is red-hypothesis, the offset medium; TEAM = 1, WILD = 2) | `0x100` | −0x23 | `ov_0001::02188030` (`RunMonsterAI`) clears it, then sets it to 1 or 2 for a decoy (blue-verified writes) |
| `+0xE0` | `speedStage` | `0x104` | −0x24 | `CalcSpeedStage` |
| `+0xE4` | `speedUpCounters[5]` | `0x108` | −0x24 | `CalcSpeedStage` |
| `+0xE9` | `speedDownCounters[5]` | `0x10D` | −0x24 | `CalcSpeedStage` |
| `+0xF4` | `moves` (4 × 6-byte `Move`, see below) | `0x118` | −0x24 | `0216E56C` (passes `info+0xF4` as the move list, sets `moveFlags2` bit 2 of entry *i* at `+0xF5 + 6i`) |
| `+0x10E` | `belly` | `0x13C` | −0x2E | `CalcDamage` (*probable*): the 4-byte value passed to `FUN_0200b9a4` (call at `ov_0001::0217be0c`). Its zero test matches Red's `FixedPointToInt(belly) == 0` for ordinary values. `FUN_0200b9a4`'s identity is inferred, so the row stays *probable* (#185, P3) |
| `+0x116` | `aiAllySkip` (blue-verified by structure, #164 P1b; the name is red-hypothesis, the offset medium) | `0x144` | −0x2E | `sub_8044574_Async` (`ov_0001::02140AC0`) writes it; `RunMonsterAI` (`ov_0001::02188030`) clears it (blue-verified write) |
| `+0x117` | `recalculateFollow` | `0x145` | −0x2E | `sub_8044574_Async` (`ov_0001::02140AC0`) writes it |
| `+0x118` | `bellyEmpty` | `0x146` | −0x2E | `TickStatusAndHealthRegen` |
| `+0x119` | `waiting` (blue-verified by structure, #164 P1b; the name is red-hypothesis, the offset medium) | `0x147` | −0x2E | `DisplayActions_Async` (`ov_0001::0218766C`); `RunMonsterAI` (`ov_0001::02188030`) clears it (blue-verified write) |
| `+0x11A` | `attacking` | `0x148` | −0x2E | `RunLeaderTurn_Async` |
| `+0x120` | `visualFlags` | `0x14E` | −0x2E | `CalcDamage` |
| `+0x122` | `previousVisualFlags` | `0x150` | −0x2E | `CalcDamage` |
| `+0x124` | `unk152` | `0x152` | −0x2E | `CalcDamage` |
| `+0x137` | Warp Scarf turns-since counter (no Red field matched) | not matched | — | `DoEndOfTurnEffects` (`ov_0001::021861E8`): the Warp Scarf roll happens only when it is nonzero |
| `+0x138`, `+0x13A` | `targetPos.x`, `.y` (u16; Red's block starts at `0x16C`) | `0x16C` | −0x34 | `DisplayActions_Async` (`ov_0001::0218766C`) |
| `+0x1C0` | `numMoveTiles` | `0x1F4` | −0x34 | `DisplayActions_Async` (`ov_0001::0218766C`) |
| `+0x1C4` | `unk1F8` (HP regeneration counter) | `0x1F8` | −0x34 | `TickStatusAndHealthRegen` |
| `+0x1C8` | `expGainedInTurn` (may lie past the end of the struct: the size bound is 0x1C8 to 0x208) | — | — | `AddExpPoints` (`ov_0001::02183F78`) records the EXP gained here |

The IQ field is `+0x0C` (the row above). The IQ-skill bits at `+0x87` are rebuilt from scratch by `021804CC` (#164 P3b, [blue-verified structure]):
- Enemies (`info+6 != 0`): set bits 8, 6 and 2, bit 12 if `info+0xD4` is non-zero, and bit 22 if the level is at or above the cap. Set `+0x8A` to 2. IQ is not used.
- Team members (`info+6 == 0`): clear `+0x87` to `+0x89`. Then, for each n from 1 to 23, set bit n of `+0x87` when `table[n] <= IQ` (`FUN_02004464`, the table at `02091F48`) and bit n of the 3-byte blocked mask at `+0x84` is clear (`FUN_02004250`; a set bit means blocked). The bit setter is `FUN_020042F8`.

The table values were read from data at a pool pointer. The mapping from skill number to name is not done. [red-hypothesis for the Red IQ names]

The status records are smaller in Blue: the distance between consecutive ones shrinks (for example, reflect to sure shot is 0x13 bytes in Blue and 0x18 in Red), so each status record needs its own check.

### Placement and writers (#163 P2)

Static evidence from the investigator's Ghidra run on `rescue-arm9-a` (all addresses in `ov_0001`, the retail Blue overlay). No runtime run for this part. **[blue-verified]** for the addresses, literal values and decompiles. **[hypothesis]** for the proposed names below, which are ours, not from `pmd-red`.

There is no separate allocator. The info array is a fixed array inside the Dungeon struct, and each entity's `+0x70` is set at spawn to

`info = gDungeon + 0x68C + slot * 0x1D4`, with `gDungeon = *(0x020E0788)`.

The array has 20 slots: team members in slots 0 to 3 (base literal `0x68C`) and enemies in slots 4 to 19 (base literal `0xDDC` = `0x68C + 4 * 0x1D4`). Team and enemy are one contiguous array. This matches the P1 measurement: base `0x02113FB0` = `gDungeon` `0x02113924` + `0x68C`, and the offsets `0x750` (= `4 * 0x1D4`), `0x924` and `0xAF0`. The total size would be `20 * 0x1D4 = 0x2480`, which is not confirmed.

The stride `0x1D4` is an immediate in each spawn writer (`mov rX,#0x1d4`, then `mla`). Writers, all decompiled:

| Function | Role | Entity pointer array | Slots | Base literal | Stride and store |
|----------|------|----------------------|-------|--------------|------------------|
| `FUN_ov_0001__021422dc` (caller `02179a28` in `02179990`) | spawn team member | `gDungeon + 0x11314` | 4 | `0x68C` at `02142490` | `mov` at `0x02142398`, `mla` at `0x021423a0`, `str +0x70` at `0x021423a4` |
| `FUN_ov_0001__02142120` (caller `02179d34` in `02179cfc`) | spawn enemy | `gDungeon + 0x11324` | 16 | `0xDDC` at `0x021422d8` | `mov` at `0x021421e4`, `mla` at `0x021421ec` |
| `FUN_ov_0001__02199194` (from `021992fc` when its `param_2` is non-zero) | restore team from suspend save | `+0x11314` | slot `param_3` | `0x68C` at `0x021992f8` | not re-read |
| `FUN_ov_0001__0219902c` (from `021992fc` when its `param_2` is zero) | restore enemy from suspend save | `+0x11324` | slot `param_3` | `0xDDC` at `0x02199190` | not re-read |

`021999d4` loops over 4 team and 16 enemy slots through `021992fc`, so the suspend restore rewrites the same slots.

Each writer also sets `entity +0x00` to 1 (valid), the entity's slot byte at `+0x24`, and info `+0x02` (species). Info `+0x04` and `+0x06` are 0 for a team member and 1 for an enemy (`+0x06` is `isNotTeamMember` above). It also sets the footprint slot and length at info `+0x135` and `+0x136`, which are not interpreted here.

Proposed names (not applied, and not added to [`symbols.md`](../symbols.md), because its rows are applied to the Ghidra project as renames): `FUN_ov_0001__021422dc` as `SetupTeamEntity`, `FUN_ov_0001__02142120` as `SetupEnemyEntity`, `FUN_ov_0001__02199194` as `RestoreTeamEntity`, `FUN_ov_0001__0219902c` as `RestoreEnemyEntity`.

**Not verified:**
- Whether the whole `0x2480` is cleared at floor start: the reset of the array was not read.
- The total size `0x2480` itself.
- Other writers of `+0x70` with a non-immediate stride. The byte search covered `mov r?,#0x1d4` for r0 to r12 in the `mov` encoding and the 4-byte literal. One literal hit at `0x0219CAFE` is data or unaligned and was ignored. The search is not exhaustive.
- Other overlays (non-dungeon overlays) were not checked.
- The field layout is out of scope here.

### aiTarget readers (#164)

Investigator B (#164 P2b), static Ghidra only; no runtime run. **[blue-verified]** for the accesses, the offsets and the decompiles; **[hypothesis]** for the roles, and **[red-hypothesis]** for the names taken from `pmd-red`.

Red reference: `pmd-red` `dungeon_ai_movement.c` 277–281 tests `aiObjective == AI_CHASE_TARGET || == AI_CHASE_REMEMBERED_TARGET`, then `aiTarget->spawnGenID == aiTargetSpawnGenID`. `AIObjective` in `dungeon_entity.h`: 1 chase target, 2 chase remembered target, 3 roam, 4 leave room, 5 run away, 6 stand still, 7 take item. Blue's `+0x70` is Red's `+0x78` (shift −8), so `aiObjective` is at info `+0x70` and `aiTargetSpawnGenID` at `+0x74`.

**Method.** B decompiled all 1759 overlay-1 functions (`ov_0001::*`) with ghidra-cli and searched the decompiled C for `+ 0x70)` (non-pointer-typed), `+ 0x74)`, `+ 0x78)`, `[0x70]`, `[0x74]` and `[0x78]`. The decompiled text does not depend on which register holds the base, which closes the gap in the earlier byte search. Hits through a scaled pointer (for example `puVar + 0x78`, which is byte `+0xF0`) are not `aiTarget` and were dropped by hand. Pointer-typed `+0x70` loads (`entity->info`) were excluded by the filter, so byte-typed `aiObjective` reads are not hidden by it.

**Accessors in overlay 1.** This list replaces the earlier partial list. **[blue-verified]** for the accesses (decompile); roles are **[hypothesis]**.

| Function (address) | Field(s) | Access | What it does |
|---|---|---|---|
| `0219082C` (called from `021938C8` at `02193B38`) | `+0x70`, `+0x74`, `+0x78` | write | init: objective 0, genID 0, target NULL (also writes `+0x80`, the position). Red's `aiTarget` reset |
| `02191394` | `+0x70`, `+0x74`, `+0x78` | write and read | target search; see the branch match below |
| `02190854` | `+0x70` | write 3, 6, 4, 3, 6, 6; read `!= 4` | roam, leave room and stand still (Red `Wander`) |
| `02190B80` | `+0x70` | write 6, 5, 5, 5 | AvoidEnemies: run away (5) or stand still (6) |
| `02190FEC` | `+0x70` | read `== 1` | DecideMovement chase test |
| `02189768` | `+0x70`, `+0x78`, `+0x74`, `+0x72` | read `+0x70 == 1` and `+0x78 == param_2`; write 6, NULL, 0 and `+0x72` = 0 | when an entity is petrified (`info+0xA0 = 6`), each team member chasing it goes to stand still. Loops over the 4 team slots (dungeon `+0x11314`) |
| `0218D384` | `+0x70`, `+0x78`, `+0x74` | write 6, NULL, 0 | for each of the 0x14 slots (dungeon `+0x11364`): reset to stand still. Caller `02163650` at `0216365C` |
| `0216C638` | `+0x70`, `+0x78`, `+0x7C`, `+0x74` | write 6, NULL, 0, 0 | reset to stand still on one entity. Caller `02168774` at `02168B68` |
| `021778F4` | `+0x78` | read `== param_1`, write NULL | entity removal: each of the 0x14 slots whose `aiTarget` is the removed entity gets `aiTarget = NULL` (objective and genID untouched) |
| `02175A00` | `+0x74` read, `+0x78` write | for each of the 0x14 slots, finds the entity whose `+0x26` equals `info+0x74` and stores it in `info+0x78` | re-resolves the target pointer from the genID. No direct caller (the x-ref is empty); probably called through a pointer or table, not found |

**`02191394` is Red's target search, including the chase branch (B: yes, and more).** It is Red's whole target-search function in `dungeon_ai_movement.c` (the function that ends in `Wander(pokemon); return TRUE;`, Red lines ~140–290). Branch by branch:
- The decoy and team-vs-wild list choice (0x14, 4 or 0x10 entries) matches Red's `decoyIsActive` and `isNotTeamMember` choice of list (maxPossibleTargets 20, 4 and 16).
- The candidate loop matches: valid entries, petrify skip, the distance-5 gate when `CanCrossWalls`, and the nearest target with an early exit at distance under 2.
- On a hit: objective 1, position at `+0x80`/`+0x82`, target at `+0x78`, genID at `+0x74` (the target's `+0x26`), `+0x72` = 1 (`aiTargetingEnemy`), and `+0xF0` = 0 (Red `moveRandomly = FALSE`). The tactic 8 (KEEP_YOUR_DISTANCE) adjacency check sets `+0x73` (`aiTurningAround`), as in Red.
- Not GO_THE_OTHER_WAY, team leader: a tile room scan for an item entity (type 3); objective 7 (`AI_TAKE_ITEM`), position, target NULL, genID 0 and `+0xF0` = 0, as in Red.
- Not GO_THE_OTHER_WAY, non-leader: the leader if visible (`02183FF0`), objective 1, as in Red.
- Final block: if `(objective − 1) < 2` and the target is not NULL, and the target's genID equals `+0x74`, loop over the target's 4 previous positions (`target_info+0x5E + 4*i`) with `02141AD0` (Red `CanTargetPosition`), then set objective 2 (`AI_CHASE_REMEMBERED_TARGET`), position and `+0xF0` = 0. Otherwise objective 6 (`AI_STAND_STILL`), target NULL and genID 0. This is Red lines 277–296.
- End: `02190854` is Red's `Wander`.

The gate `02184074() == 0` and the monsterBehavior filter appear as `info+0x98 == 0` in Blue. B did not map the exact field. Blue has no separate `unkC`.

**Offsets.** The −8 shift is confirmed (high) for `aiObjective` (`+0x70`), `aiTargetSpawnGenID` (`+0x74`), `aiTarget` (`+0x78`), `aiTargetPos` (`+0x80`/`+0x82`), `aiTargetingEnemy` (`+0x72`), `aiTurningAround` (`+0x73`) and `moveRandomly` (`+0xF0`, Red `0xF8`). [blue-verified for the Blue offsets and behaviour; red-hypothesis for the Red names]

Excluded (not `aiTarget` in Blue):
- `FUN_020319F4` (`020319F4`, hits `02031AD4` and `02031B0C`): `+0x74` is a table of shorts in another struct.
- `FUN_ov_0000__0214E2A4` (`0214E2A4`, hits `0214E4DC` and `0214E534`): writes `param_1+0x74` (a u32) and `param_1+0x70` (bytes) in a struct that is not shown to be the info; not classified.
- `FUN_ov_0001__0216E7B4` (`0216E7B4`, hit `0216E818`): reads `*(param_2+0x70)` as a pointer; the target is unclear; not classified.

**Objective values.** Blue writes 0 (init), 1, 2, 3, 4, 5, 6 and 7. All of Red's enum numbers 1 to 7 appear, and Red's names fit each context. [hypothesis for the names]

**Not verified:**
- ARM9 and the other overlays were not scanned. The hits `ov_0000::0214E2A4` and `ov_0001::0216E7B4` (see Excluded above) stay unclassified.
- Who calls `02175A00`: there is no direct caller.
- The `+0x98` filter against Red's `monsterBehavior`: B did not map the exact field.
- Completeness is medium: a decompiler type cast could hide an accessor.
- Only the numbers of Blue's objective enum are shared with Red's; the names are not confirmed.
- Runtime behaviour: nothing was run; no retail dump is configured.

## Dungeon struct

Pointed to by `gDungeon` (`0x020E0788`). Identical to Red at least up to `0x17A`; after that, fields move earlier.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x002` | `unk2` (floor-end state) | `0x002` | `IsFloorOver` |
| `+0x004` | `unk4` | `0x004` | `IsFloorOver` |
| `+0x005` | `unk5` (skips the turn loop) | `0x005` | `RunDungeon_Async` |
| `+0x00F` | `noActionInProgress` | `0x00F` | `RunLeaderTurn_Async` |
| `+0x00D` | not matched (written by `AddExpPoints`; meaning not known) | — | `ov_0001::02183F78` |
| `+0x0B8`, `+0x0BC` | `unkB8`, `unkBC` (acting entity) | `0x0B8`, `0x0BC` | `RunLeaderTurn_Async` |
| `+0x134`–`+0x17A` | `damageCalcWorkingValues` | same | `CalcDamage` (16 fields at Red's offsets) |
| `+0x5B8` | `unk5C0` | `0x5C0` | `RunDungeon_Async` |
| `+0x638` | start of `unk644` | `0x644` | see below |
| `+0x3360` | `deoxysForm` | `0x3800` | `RunDungeon_Async` |
| `+0x3424` | `numItems` | `0x3904` | `ResetFloor` |
| `+0x34AA` | shop flag (1 when the Kecleon shop pass ran, else 0) | `unk3A0A` | `0x3A0A` | `GenerateFloor` end; the shop pass `0x02149AF8`. Blue `+0x34AA` to Red `0x3A0A`: gap `0x560` |
| `+0x34AC` | stairs room index (floor type 2 only) | not matched | `SpawnNonEnemies` (`ov_0001::0214B650`) writes it |
| `+0x34AE`, `+0x34B0`, `+0x34B4` | `tileset`, `unk3A10`, `fixedRoomNumber` | `0x3A0E`, `0x3A10`, `0x3A14` | `RunDungeon_Async` |
| `+0x34B8` | `tiles[32][56]` (0x14 bytes each, see below) | `0x3A18` (0x18 bytes each) | `sub_80495E4` |
| `+0xC0BC`, `+0xC0BE` | `stairsSpawn.x`, `.y` | `0xE21C` | `ResetFloor` (pmd-red's comment says `0xEA1C`; the layout gives `0xE21C`); written by `SpawnNonEnemies` |
| `+0xC11C` | `unkE27C[8][8]` (fixed-room tiles) | `0xE27C` | `ResetFloor`. Blue's base, `DAT_0214C0A4`, is not read (unverified); the offset is Red's and is not confirmed in Blue |
| `+0xC106` | not matched (`weatherDamageCounter` in the notes) | not matched | weather resolver `021953B4`: decremented, set to 9 when it is 0 |
| `+0xC107` | 8 weather sources, one byte each | not matched | `021953B4`: each moves toward its natural value (`+0xC10F`), the highest is taken |
| `+0xC10F` | 8 natural weather values, one byte each | not matched | `021953B4` |
| `+0xC660` | `tilePointers[32][56]` | `0xE8C0` | `GetTile`, `GetTileMut`, `sub_80495E4` |
| `+0x113B4` | `items[64]` (entity pointers) | `0x1361C` | `SpawnItemEntity` (searches the 64 entries for a free slot) and `0219B700` |
| `+0x114B4` | `traps[64]` (entity pointers) | `0x1371C` | `ResetFloor` clears all 64. No function that writes trap entities is found |
| `+0x11314` | `teamPokemon[4]` (entity pointers) | `0x1357C` | `GetLeader` |
| `+0x11324` | `wildPokemon[16]` (entity pointers) | `0x1358C` | five loops over 16 entries with `EntityIsValid` (`0219F09C`, ...) |
| `+0x11364` | active team list, 20 slots, each checked for a valid entity | not matched | `EnemyEvolution` (`ov_0001::02183BB4`), `sub_8044AB4` (`ov_0001::021406F8`) |
| `+0x15FB0`, `+0x15FB1` | `unk181e8.unk18218`, `unk18219` | `0x18218`, `0x18219` | `RunDungeon_Async` |
| `+0x19DF6` | `unk1BDD4.unk1C05E` | `0x1C05E` | `RunDungeon_Async` |
| `+0x1A2F8`–`+0x1A308` | `floorProperties` fields | `0x1C576`–`0x1C588` | `RunDungeon_Async` ([`dungeon-generation.md`](dungeon-generation.md)) |

The shift grows: −8 at `0x5C0`, −0xC at `unk644`, −0x4A0 at `0x3800`, −0x560 at `0x3A0E`, −0x2160 at `stairsSpawn`, −0x2268 from `traps` on. From the tiles to the tile pointers, the extra shift is exactly 4 bytes per tile (56 × 32 map tiles plus 64 fixed-room tiles).

### `unk644` sub-struct

In Blue it starts at `0x638`. Its first field checked, `unk10`, is at Red's relative offset; every later field checked is 4 bytes earlier, so Blue drops 4 bytes between `+0x11` and `+0x1B` of the sub-struct.

| Blue | Red field | Red | Evidence |
|------|-----------|-----|----------|
| `+0x648` | `unk10` (floor-end reason) | `0x654` | `IsFloorOver` |
| `+0x650` | `fractionalTurn` | `0x660` | `RunFractionalTurn_Async` |
| `+0x656` | not matched (`windTurns` in the notes) | not matched | `UpdateWindTurns` (`ov_0001::02195104`) |
| `+0x65C` | `unk28` | `0x66C` | `RunLeaderTurn_Async` |
| `+0x65E` | `stoleFromKecleon` | `0x66E` | `CalcSpeedStage` |
| `+0x663` | `unk2F` | `0x673` | `RunLeaderTurn_Async` |
| `+0x668` | `missionKind` | `0x678` | `RunDungeon_Async` ([`rng.md`](rng.md)) |
| `+0x669` | not matched | not matched | `DoEndOfTurnEffects` (empty-belly alert counter, capped at 10) |
| `+0x66A` | not matched (`windPhase` in the notes) | not matched | `UpdateWindTurns` (wind phases 0 to 2) |
| `+0x66C` | `prngPreseed23Bit` | `0x67C` | `RunDungeon_Async` |
| `+0x670` | `unk3C` (floor seed) | `0x680` | `RunDungeon_Async` |

## Tile

[blue-verified] by `ResetTile` (`ov_0001::0214C0A8`), which writes every field in Red's order, and by `sub_80495E4`/`GetTileMut` (stride 0x14). `pmd-red`'s `include/structs/map.h` already notes the DS size: the spawn/visibility flags take 2 bytes instead of 6, so every later field is 4 bytes earlier.

| Blue | Red field | Red | Notes |
|------|-----------|-----|-------|
| `+0x00` | `terrainFlags` (u16) | `0x00` | `0x10` = impassable wall (set on the map border by `ResetFloor`); `0x200` is set by `ResolveInvalidSpawns` (`ov_0001::0214C1A8`) |
| `+0x02` | `spawnOrVisibilityFlags` (u16) | `0x04` | bits used by `SpawnNonEnemies` (`0x2` item candidate, `0x4` trap or other candidate), `SpawnEnemies` and `SpawnNonEnemies` (gate on `0x8`, `0x20`, `0x40`, `0x100`), and `ResolveInvalidSpawns` (clears `0x4` when `0x1` or `0x2` is set). The bit names are guessed; the uses are blue-verified |
| `+0x04` | `unk8`: graphics variant, picked with the global RNG; cosmetic ([`dungeon-generation.md`](dungeon-generation.md#emulator-check)) | `0x08` | |
| `+0x05` | `room` (`0xFF` = none) | `0x09` | written by `CreateRoomsAndAnchors` (`ov_0001::0214F7A8`) as a counter from 0; anchor tiles get `0xFE`; `ResetTile` sets `0xFF` |
| `+0x06` | `walkableNeighborFlags[4]` | `0x0A` | one byte per crossable-terrain kind |
| `+0x0A` | `unkE` | `0x0E` | |
| `+0x0C` | `monster` (entity pointer) | `0x10` | |
| `+0x10` | `object` (item or trap entity pointer) | `0x14` | |

**Rooms.** The room data is per tile: the room index is in `+0x05`. A persistent room array in the dungeon struct was not found (medium confidence). The 30-byte cell grid (`x0`/`y0`/`x1`/`y1` at `+0`/`+2`/`+4`/`+6`, room flag at `+0x1C`) is generation scratch passed in by the caller, `GenerateStandardFloor` and its siblings. The meaning of `gNumRoomsAssigned` (`ov_0001::021B45BC`) is taken from older notes and was not rechecked.

Size `0x14` (Red `0x18`). The grid is `tiles[y][x]` at dungeon `+0x34B8`, row stride `0x460`; `GetTile(x, y)` returns `tilePointers[y][x]` (dungeon `+0xC660`, row stride `0xE0`) or, outside 56 × 32, the out-of-bounds tile `gCurTilesetOobTile`.

## Moves, items and inventory

**Move** [blue-verified]: `moveFlags` `+0` (`5` = exists and enabled for the AI), `moveFlags2` `+1`, `id` (u16) `+2`, `PP` `+4`, `ginseng` `+5`; 6 bytes (Red 8). Every field is written by `InitPokemonMove` (`0200B568`), and `GetMovePower` reads `id` and `ginseng`. The info struct holds 4 of them at `+0xF4`; `pmd-red` notes `struggleMoveFlags` at `+0x18` after them.

**Move data** (the `MoveDataEntry` that the accessors read, not the runtime `Move` above) [blue-verified reads; the offsets match Red's `MoveDataEntry` in `include/structs/str_moves.h`]: targeting `u16` for the player at `+0x8` and for the AI at `+0xA` (`FUN_0200B4EC`, read by `ov_0001::02191A30` and the AI resolver `FUN_02192728`); one accuracy byte per slot at `+0xE` to `+0x10` (`FUN_0200B3C4`, read by the hit check); range byte at `+0x19`, display only (`FUN_0200B304`). The entry's size is not determined here.

**Damage struct** (`dmgStruct`, passed to `CalcDamage`) [blue-verified]: the critical-hit flag is at `+0xD`. `CalcDamage` clears it when the damage is 0. Red's name for the field is not checked.

**Item** [blue-verified]: `flags` `+0` (bit `1` = exists, `8` = sticky, `0x10` = set for L+R throwing), `quantity` `+1`, `id` `+2`; 3 bytes, the same as Red's fields. Read by `HasHeldItem` and the inventory functions; `SpawnItem` clears bit `0x10`.

**Team inventory** [blue-verified]: `gTeamInventoryRef` (`0x020DECF8`) points to it. Every field below is written by `InitializeMoneyItems` (`02008588`) or read by a function matched to `pmd-red`, and the offsets agree with `pmd-red`'s Blue notes in `include/structs/str_items.h`.

| Blue | Field | Red | Evidence |
|------|-------|-----|----------|
| `+0x000` | `teamItems[20]` (3-byte `Item`) | `0x000` | `GetItemCountInInventory`, `InitializeMoneyItems` |
| `+0x03C` | `teamStorage[240]` (u16 counts per item id) | `0x050` | `InitializeMoneyItems` (240 entries; ends exactly at `+0x21C`) |
| `+0x21C` | `kecleonShopItems[8]` (2-byte `BulkItem`: `id`, `quantity`) | `0x230` | `CountKecleonShopItems`, `AddKecleonShopItem`, `InitKecleonShopItem` |
| `+0x22C` | `kecleonWareItems[4]` | `0x250` | `CountKecleonWareItems`, `AddKecleonWareItem` |
| `+0x234` | `teamMoney` (s32, clamped to 99,999) | `0x260` | `AddToTeamMoney` |
| `+0x238` | `teamSavings` (s32) | `0x264` | `InitializeMoneyItems` |

Blue's struct is 0x23C bytes (Red 0x268). The difference is packing: Blue stores `Item` in 3 bytes and `BulkItem` in 2, where Red's arrays use a 4-byte stride for both.

**Floor items** [blue-verified]: 64 entity pointers at dungeon `+0x113B4` (Red `0x1361C`). `SpawnItemEntity` (`ov_0001::02141F58`) takes a free slot. It sets the entity type to 3 (`ENTITY_ITEM`), copies the position to `+0x4`/`+0x6`, and sets `entity+0x70` to the 3-byte `Item` slot `gDungeon + 3·i + DAT_02142064`. The base `DAT_02142064` is not read. `SpawnItem` then copies the `Item` into the entity's info, sets `isVisible` and puts the entity in the tile's `object` field.

## Recruited Pokémon

The team roster outside dungeons (Red `Pokemon`). Offsets read from `WritePoke1Bits` (`02003E1C`), which writes each field in turn; they match `pmd-red`'s Blue notes in `include/structs/str_pokemon.h` field for field. Size `0x40` (Red `0x58`).

| Blue | Field | Red |
|------|-------|-----|
| `+0x03` | `level` | `0x03` |
| `+0x04` | `dungeonLocation` | `0x04` |
| `+0x06` | `speciesNum` | `0x08` |
| `+0x08`, `+0x09` | `unkC[2]` (levels) | `0x0C` |
| `+0x0A` | `IQ` (also the learn gate: `FUN_02001844` reads it against 333 for move IDs 238, 239, 272 and 354; [hypothesis]) | `0x14` |
| `+0x0C` | `pokeHP` | `0x16` |
| `+0x0E`–`+0x11` | `offense` (attack, sp. attack, defense, sp. defense) | `0x18` |
| `+0x14` | `currExp` | `0x1C` |
| `+0x18` | `IQSkills` (24 bits) | `0x20` |
| `+0x1B` | `tacticIndex` | `0x24` |
| `+0x1C` | `heldItem` (2-byte `BulkItem`) | `0x28` |
| `+0x1E` | `moves[4]` (6 bytes each) | `0x2C` |
| `+0x36` | `name[10]` | `0x4C` |

## Roster and mail helpers (#170 P1, proposed names)

Static evidence from the investigator comment on [#170](https://github.com/TorratDev/rust-mm/issues/170), read in the Ghidra copy `rescue-arm9-a`. Read-only: nothing is renamed in the project. The names are proposals for the orchestrator, not applied renames.

Tags: **[blue-verified]** for the behaviour, callers and literal-pool values. **[red-hypothesis]** for the names: they are our own, not taken from the repo or `pmd-red`.

Literal-pool values [blue-verified]:

| Literal | Value | Used for |
|---------|-------|----------|
| `DAT_02002FFC` | `0x020DF484` | roster base |
| `DAT_02003000` | `0x19D` (413) | roster record count |
| `DAT_020024E0` | `0x02008C0C` | character-copy routine |
| `DAT_02006420` | `0x0209281B` | base of a 16-byte-record table |
| `DAT_02008C04` | `0x02091C44` | error text used by the decoder |
| `DAT_0200D588` | `0x020DF6EC` | mail-record pointer block |

The roster base and count match [`rng.md`](rng.md) and [#161](https://github.com/TorratDev/rust-mm/issues/161) P2 (records of 0x40 bytes, 413 of them).

**`FUN_02002F88` at `0x02002F88`: proposed `FindFirstActiveRosterRecord`** [blue-verified behaviour; red-hypothesis name]
- Scans the 413-entry roster from `0x020DF484` in steps of `0x40`. Returns the first record whose `u16` at `+0` has bit 0 set and whose byte at `+2` is nonzero. Returns 0 if none matches.
- Callers: `FUN_0200D4DC`, `FUN_0201E020`, `FUN_02052AAC`, `FUN_0206A6DC`, `FUN_02069848`, and others.
- Confidence: high for the behaviour; medium that the array is the 0x40-byte roster (the pointer and count match `rng.md`).

**`FUN_020024D0` at `0x020024D0`, `FUN_020024D0(dst, rec)`: proposed `CopyRosterNameToBuffer`** [blue-verified behaviour; red-hypothesis name]
- Calls `FUN_02008C0C(dst, rec + 0x36, 10)`: copies up to 10 characters through the per-character map `FUN_02008C50` (not read), then NUL-terminates. The `+0x36` field is `name[10]` in the Recruited Pokémon table above.
- Callers: `FUN_0200D4DC`, `FUN_02052AAC`, `FUN_02065844`, `FUN_02057FE8`, `FUN_02038C28`, `FUN_02038D48`, `FUN_02042378`, `FUN_02052560`.
- Confidence: high for the offset and length; medium for the record identity.

**`FUN_02008AF0` at `0x02008AF0`, `FUN_02008AF0(dst, src)`: proposed `DecodeHexEscapes`** [blue-verified behaviour; red-hypothesis name]
- Copies `src` to `dst`. Each `~` followed by two hex digits (`0-9`, `a-f`, `A-F`) becomes one byte, high nibble first. Other bytes are copied unchanged. A bad digit prints the error at `0x02091C44` and gives a 0 nibble. Writes a 0 terminator at the end.
- Callers: `FUN_0200D4DC`, `FUN_0201E020`, `FUN_02004590`, `FUN_02002788`, `FUN_0219C898` (three sites), `FUN_02069848`, `FUN_021461D0` (overlay 0), `FUN_0206C170`.
- Confidence: high for the behaviour (read from the code).

**`FUN_02006410` at `0x02006410`, `FUN_02006410(index)`: proposed `GetTableByteByIndex16`** [blue-verified behaviour; red-hypothesis name]
- Returns the byte at `0x0209281B + index * 0x10`. The table's meaning is not established.
- Callers: `FUN_0200D4DC` (index = the byte at the mail record's `+4`, stored at `+0x2C` of the record) and `RunDungeon_Async` (`0x0206DCEC`, index = the setup struct's `+4`).
- Confidence: high for the behaviour; low for a more specific name. Only `GetTableByteByIndex16` is proposed.

**Mail-record builder `FUN_0200D4DC` at `0x0200D4DC`: context only** [blue-verified structure; not checked against the mail format]
- Writes a flag of 1 at `+0`, copies two parameters to `+4` and `+8`, and seeds the Mersenne Twister with `Rand32Bit`. Stores a random value at `+0x10`, a roster `u16` at `+0xC`, the decoded text at `+0x14`, `sub_8011C34()` at `+0x24` and the table byte at `+0x2C`.
- [#47](https://github.com/TorratDev/rust-mm/issues/47) named this function `sub_809542C`. The structure is consistent with a mail record; this was not checked against the mail format.

Open, not verified (the #170 P1 comment's list, left as it is):
- The per-character map `FUN_02008C50`, which turns each name character into a code.
- The table at `0x0209281B`: what its bytes mean, and which of the two callers it belongs to.
- The roster `+6` byte copied into the mail record's `+0xC`.
- Runtime: no emulator check (no Blue dump configured in this session).
- `FUN_02002F88` returns 0 when nothing matches. Callers that test the result for a nonzero pointer were not all checked.

## Mail record (#170 P3)

The mail record that `FUN_0200D4DC` (`sub_809542C` in [#47](https://github.com/TorratDev/rust-mm/issues/47)) builds. Offsets written by the builder, compared with pmd-red's `include/structs/str_wonder_mail.h` (`unkStruct_203B480`) and `src/code_8094F88.c` (`sub_809542C`). Source: the #170 P3 comment by investigator A; static, Ghidra copy `rescue-arm9-a`, no emulator check.

Tags: **[blue-verified]** for the writes; the Red field names are **[red-hypothesis]**.

| Blue offset | Written by | Red field | Match |
|-------------|-----------|-----------|-------|
| `+0x00` (u8) | `= 1` | `mailType` at `0x0` | offset and value (1 in both) |
| `+0x04`, `+0x08` (u32 each) | copies of `param_1[0]` and `param_1[1]` | `dungeonSeed` (`DungeonMailSeed`, 8 bytes) at `0x4` | size and position |
| `+0x0C` (u16) | roster record `+6` | `clientSpecies` at `0xC`, set from `GetLeaderMon1()->speciesNum` | position; the source is medium (roster `+6` as species is not verified) |
| `+0x10` (u32) | `Random32MersenneTwister()` after `MersenneTwister_InitializeState(Rand32Bit())` | `unk10` at `0x10` | position and call sequence |
| `+0x14` (10 bytes) | roster name through `FUN_020024D0`, then `FUN_02008AF0` (decoded) | `playerName[10]` at `0x14` | position; Red's `sub_809542C` has no decoding step |
| `+0x24` (u32) | `sub_8011C34()` | `unk24` at `0x24` | same call |
| `+0x2C` (u8) | `FUN_02006410(byte at mail +4)` | `rescuesAllowed` (`s8`) at `0x2C`, set from `GetRescuesAllowed(dungeonSeed.location.id)` | **offset matches; value open (below)** |

Not written by `FUN_0200D4DC`: `+0x01` (missionType), `+0x20` (`item` in Red) and `+0x28` (`unk28` in Red). Red sets these elsewhere; the Blue writers were not looked for.

**Open: the byte at `+0x2C` may not be `rescuesAllowed`.**
- `FUN_02006410(index)` returns the byte at `table + index * 0x10`: byte `+0x0` of a 16-byte record. The stride matches pmd-red's `DungeonDataEntry` (`include/dungeon_data.h`, size `0x10`).
- In that struct, byte `+0x0` is `stairDirectionUp`, and `rescuesAllowed` is at `+0x3`. If Blue's table has Red's layout, the mail byte and the byte `RunDungeon_Async` stores at `gDungeon + 0x66B` (from `FUN_02006410(setup + 4)`) are `stairDirectionUp`, not `rescuesAllowed`.
- Red reads `rescuesAllowed` at `gDungeon->unk644.unk37` (`src/run_dungeon.c` 139). Blue's `+0x66B` is a different layout, so this is not a direct comparison.
- **Not verified:** whether Blue's table has Red's layout, and whether Blue's record puts the `+0x3` byte elsewhere. This needs a read of the table or of its users.

## RAM for golden traces

What the V7 dumper ([#55](https://github.com/TorratDev/rust-mm/issues/55)) should read on each turn trigger ([`turns.md`](turns.md)), all through the pointer in `gDungeon`:

| What | Where |
|------|-------|
| Dungeon struct | `*(u32 *)0x020E0788` |
| Turn counter | dungeon `+0x650` |
| Run and floor seeds | dungeon `+0x66C`, `+0x670` |
| Team | 4 entity pointers at dungeon `+0x11314`; for each: `+0x70` → info: `id` `+0x02`, `level` `+0x09`, `HP` `+0x0E`, `maxHPStat` `+0x10`, `types` `+0x56`, statuses `+0x99`–`+0xC4`, `speedStage` `+0xE0`, `belly` `+0x10E` |
| Wild Pokémon | 16 entity pointers at dungeon `+0x11324` |
| Positions | entity `+0x04` (x), `+0x06` (y) |
| Moves, held item | info `+0xF4` (4 × 6 bytes: flags, id `+2`, PP `+4`), info `+0x5A` (3 bytes) |
| Floor items | 64 entity pointers at dungeon `+0x113B4`; each entity's `+0x04` position and `+0x70` → 3-byte `Item` |
| Team inventory | `*(u32 *)0x020DECF8`: 20 × 3-byte items at `+0`, money `+0x234`, savings `+0x238` |
| Floor map | 56 × 32 tiles of 0x14 bytes at dungeon `+0x34B8` (`terrainFlags` `+0`, `room` `+5`, `monster` `+0xC`, `object` `+0x10`); stairs at dungeon `+0xC0BC` |
| RNG | `sPRNGState` `0x020DF704`, `gDungeonRngState` `0x021B3F58` ([`rng.md`](rng.md)) |

## Open questions

- **Entity info size.** `0x1D4` (468), measured in the emulator (#163 P1, runtime). Method: `info = base + k*0x1D4` for the team and wild entries of a dungeon at its start. The array's clear at floor start, its total size `0x2480`, and the per-record field check are not verified (see [placement and writers](#placement-and-writers-163-p2) and the size note above).
- **`aiTarget` inner layout (#164 P2b).** Still open: the ARM9 and the other overlays are not scanned for the `+0x70`, `+0x74` and `+0x78` accessors; the caller of `02175A00` is not found; and the `+0x98` filter in `02191394` against Red's `monsterBehavior` is not mapped (see [aiTarget readers](#aitarget-readers-164)).
- **Ally spawn record (#164 P3b).** The source of `param_4+6` in `02179990`, which becomes info `+0x0C` (IQ), is not traced. The callers `0217F388`, `0217EF24`, `0217A054` and `0217A398` are not checked.
- **IQ gummi cap (#164 P3b).** The cap constant at `0214607C` (used by `02145EC8` to clamp IQ) was not read: the literal pool is not mapped at that address in this program view.
- **IQ writers outside the search (#164 P3b).** Writers that take the info as a parameter, or use non-halfword forms, were not covered by the search that found the IQ writers. Only the 25 functions with both patterns were reviewed.
- **Status write sites (#166 P1, P2).** Found by a search of one call level from the move executor `ov_0001::0215B384` and the move-effect switch `ov_0001::02156824` (314 functions), and of `LDRB`/`STRB` at offsets `0xC9` and `0xCB`. Writers behind a function-pointer table or a second call level were not reached, so the curse, leech-apply and most cringe writers are not established. Blinker cross-eyed (2) and eyedrops (3) were not found by the `LDRB`/`STRB` search, which covers only the immediate offset in the low byte. The `+0xCA = 8` default in `FUN_0205938C` (ARM9 `020593B0`, along with `+0xC9 = 0` and `+0xCB = 0`) is not checked against any reader. The turn roll `FUN_ov_0001__0219CF28` is not named. Cringe writers for paused, infatuated, cowering, taunted and encore are not checked. The readers list stays open. Runtime: no emulator check. Tags: blue-verified for write sites and offsets; red-hypothesis for the names; [hypothesis] for the class names.
- **Trap entities.** The writer of `traps[64]` (`+0x114B4`) is not found. Search the overlay 1 functions called from the floor-generation chain (`GenerateFloor`, `ov_0001::02151E7C`), or dump RAM after a floor load (#17).
- **`DAT_02142064`**, the base of the floor-item slots (dungeon `+0x3·i`). Read its value to name the floor-item storage.
- **Dungeon `+0xC0B8`/`+0xC0BA`** (checked for `-1` next to the stairs; meaning unknown), and the later use of `+0x34AC` (stairs room, floor type 2).
- **Tile spawn bits.** Names for `0x2`, `0x4`, `0x8`, `0x20`, `0x40`, `0x100` and `0x200`. Name them after the trap writer is found.
- **Rooms.** Confirm that no persistent room array exists (medium confidence now), and recheck `gNumRoomsAssigned`.
- Add the remaining trace fields (statuses, moves, held items, floor items, inventory) to `verification/v7-trace/retail_trace.py` and check them by emulator.
