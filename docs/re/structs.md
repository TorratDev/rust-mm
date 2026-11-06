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
| `+0x06` | `isNotTeamMember` | `0x06` | 0 | `CalcSpeedStage`, `CalcDamage`: selects the level path, `ldrb r0,[r5,#6]` at `0x0217c270` in `ov_0001::0217BD94` (#185, P3, P6). Writers (#275 P3, static): `FUN_ov_0001__021422dc` stores 0 (team; its only caller is `02179990`, the ally spawn, which passes flag 1 to `021794cc`); `FUN_ov_0001__02142120` stores 1 (non-team; only caller `02179cfc`, flag 0 to `021794cc`). `02179874` only reads it. The `u16` store at `+6` in `021794cc` is on the entity (`pos.y`, `0xFFFE`), not on the info. Runtime values of 0 and 1 not run, see #278. [static; high for the overlay 1 writers; medium that no store outside it exists] |
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
| `+0xAC`, `+0xAD` | cringe status, counter (*red-hypothesis* names; the class is guessed from the shift). One setter per value 1 to 7 writes `+0xAC` = value and `+0xAD` = turns + 1 (blue-verified writes; see [`status.md`](status.md), cringe class): 1 `FUN_ov_0001__021887E0`, 2 `FUN_ov_0001__0218E27C`, 3 `FUN_ov_0001__0218AB34`, 4 `FUN_ov_0001__0218E19C`, 5 `FUN_ov_0001__0218D004`, 6 `FUN_ov_0001__0218CC14`, 7 `FUN_ov_0001__0218A9F8`. Cleared by `FUN_ov_0001__0218FA80` and `FUN_ov_0001__0217930C` | `0xBC`, `0xBD` | −0x10 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) only |
| `+0xAE` | `bideClassStatus.status` | `0xC0` | −0x12 | `CalcDamage` |
| `+0xB1` | `reflectClassStatus.status` | `0xC4` | −0x13 | `CalcDamage`, `TickStatusAndHealthRegen` |
| `+0xB2` | `reflectClassStatus` counter | `0xC5` | −0x13 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) |
| `+0xB3`, `+0xB6`, `+0xB7` | curse status, counter, and the byte after it (*red-hypothesis* names; the class is guessed from the shift). Set by the curse, decoy and snatch setters `FUN_ov_0001__0218D218` (1), `FUN_ov_0001__0218D384` (2) and `FUN_ov_0001__0218D0E0` (3) (blue-verified writes; `+0xB4` and `+0xB5` are the decoy applier flag and tracker). Cleared by `FUN_ov_0001__0218F6C8` and `FUN_ov_0001__0217930C` (see [`status.md`](status.md), curse class) | `0xC8`, `0xCB` | −0x15 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`), and `RunMonsterAI` (`ov_0001::02188030`, decoy test, blue-verified read); `ov_0001::02181904` (values 1 and 2) and `ov_0001::02183FF0` (leader, value 2) also read it (#275 P3, static) |
| `+0xB4` | decoy applier flag: the applier's `info+6` (0 team, 1 wild) | — | — | Writer `FUN_ov_0001__0218D384` (the setter that also sets `+0xB3` to 2) stores the applier's `info+6`. Reader: `RunMonsterAI` (`ov_0001::02188030`, decoy step) compares it with 0 (#275 P3, static). Runtime values not run, see #278 |
| `+0xB8`, `+0xC1` | leech seed status, counter (*red-hypothesis* names; the class is guessed from the shift). Set to 1 (leech seed, `FUN_ov_0001__0218DAE4`) or 2 (destiny bond, `FUN_ov_0001__0218D908`), with `+0xC1` = turn roll + 1 (blue-verified writes; see [`status.md`](status.md), leech section). Cleared by `FUN_ov_0001__0218DA68`, `FUN_ov_0001__0218F610` (`+0xB8 = 0`, then `+0xC0 = 0xFF`) and `FUN_ov_0001__0217930C` | `0xD0`, `0xD9` | −0x18 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`), and `ov_0001::0218F610` (switch on `+0xB8`, messages for 1 and 2) |
| `+0xC4` | `sureShotClassStatus.status` | `0xDC` | −0x18 | `CalcDamage` |
| `+0xC5` | `sureShotClassStatus` counter | `0xDD` | −0x18 | `TickStatusAndHealthRegen` (`ov_0001::02185C70`) |
| `+0xC7` | mobility status checked against 3 (STATUS_MOBILE); Red `invisibleClassStatus.status` is inferred from the value, not checked | not matched | — | `CanMoveInDirection` (`ov_0001::0218141C`) |
| `+0xC9`, `+0xCA` | blinker class, status and turns (1 `BLINKER`, 2 `CROSS_EYED`, 3 `EYEDROPS`; #189 P2; names red-hypothesis; blue-verified write sites). The old "invisible" name was wrong: invisible is `+0xC7` (`0218CE68`). Setters `ov_0001::0218BBB4` (status 1), `ov_0001::0218BAC8` (status 2), `ov_0001::0218B9F4` (status 3); turns = the turn roll + 1 in each | not matched | — | `FUN_ov_0001__0218F304` (`ov_0001::0218F3BC`, switch and clear), `FUN_ov_0001__0217930C` (reset, with `+0xCB`..`+0xCC`) |
| `+0xCB`, `+0xCC` | muzzled flag, 0 or 1 only (#189 P2; name red-hypothesis; Safeguard-gated setter, pool `0x021AA2B0` = (10, 10) = Red `gMuzzledTurnRange`). Setter `ov_0001::0218C1AC` (status 1), turns = the turn roll + 1; no writer of 2 or 3 (#166 P2b) | not matched | — | `FUN_ov_0001__0218F27C` (`ov_0001::0218F2EC`, read and clear), `FUN_ov_0001__0217930C` (reset, with `+0xC9`..`+0xCA`) |
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

**`+0xCA = 8` is not a status default (#166 P2b).** `FUN_0205938C` (ARM9 `020593B0`) is a menu-state init. It stores `+0xC8 = 1`, `+0xC9 = 0`, `+0xCA = 8` and `+0xCB = 0` into a menu object, not into an entity info. `+0xCA = 8` is the cursor column that `FUN_020591E4` reads when it draws the list. The matching offsets are a coincidence.

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
| `+0x3364` | `floorItems[64]`, 3-byte `Item` slots (red-hypothesis name) | `0x3804` (4-byte slots, `unk3804`) | `SpawnItemEntity` (`ov_0001::02141F58`), through `DAT_02142064` = `0x3364` |
| `+0x3424` | `numItems` | `0x3904` | `ResetFloor` |
| `+0x34AA` | shop flag (1 when the Kecleon shop pass ran, else 0) | `unk3A0A` | `0x3A0A` | `GenerateFloor` end; the shop pass `0x02149AF8`. Blue `+0x34AA` to Red `0x3A0A`: gap `0x560` |
| `+0x34AC` | `monsterHouseRoom`, the monster-house room index (`0xFF` when none; red-hypothesis name) | `0x3A0C` | `ov_0001::02151E7C` sets `0xFF` at floor start. Written by `0214D190` (monster-house selection) and by `0214B650`: on floor type 2 the stairs room becomes the monster-house room. Read by `0219037C`, `021826A0` and `021946FC` (which clears it to 0). Shift `−0x560`, the same as `+0x34A8` |
| `+0x34AE`, `+0x34B0`, `+0x34B4` | `tileset`, `unk3A10`, `fixedRoomNumber` | `0x3A0E`, `0x3A10`, `0x3A14` | `RunDungeon_Async` |
| `+0x34B8` | `tiles[32][56]` (0x14 bytes each, see below) | `0x3A18` (0x18 bytes each) | `sub_80495E4` |
| `+0xC0B8`, `+0xC0BA` | `playerSpawn.x`, `.y` (leader start, `-1` when unset; red-hypothesis name) | `0xEA18` | pair A. Reset to `-1` at each try of `ov_0001::02151E7C` (`ResetFloor` clears only pair B). Written by `SpawnNonEnemies` step 6 and `FUN_ov_0001__02152B20`. Shift `−0x2960`, the same as pair B |
| `+0xC0BC`, `+0xC0BE` | `stairsSpawn.x`, `.y` (`-1` when unset; red-hypothesis name) | `0xEA1C` | pair B. Cleared by `ResetFloor`; written by `SpawnNonEnemies` step 1 and `FUN_ov_0001__02152B20`. Shift `−0x2960`. Red's value is from pmd-red's `str_dungeon.h` (checked) |
| `+0xC11C` | `unkE27C[8][8]` (fixed-room tiles) | `0xE27C` | `ResetFloor`. Blue's base, `DAT_0214C0A4`, is not read (unverified); the offset is Red's and is not confirmed in Blue |
| `+0xC106` | not matched (`weatherDamageCounter` in the notes) | not matched | weather resolver `021953B4`: decremented, set to 9 when it is 0 |
| `+0xC107` | 8 weather sources, one byte each | not matched | `021953B4`: each moves toward its natural value (`+0xC10F`), the highest is taken |
| `+0xC10F` | 8 natural weather values, one byte each | not matched | `021953B4` |
| `+0xC660` | `tilePointers[32][56]` | `0xE8C0` | `GetTile`, `GetTileMut`, `sub_80495E4` |
| `+0xE5E4` | junction-tile counts per room, `s16[32]` | not matched | `FUN_ov_0001__0214D82C` zeroes and fills them; read by `FUN_ov_0001__02190B80` |
| `+0xE624` | junction-tile lists per room: 32 entries of `(s16 x, s16 y)` in a 0x80-byte block per room | not matched | `FUN_ov_0001__0214D82C` writes; read by `FUN_ov_0001__02190854` and `FUN_ov_0001__02190B80` |
| `+0x113B4` | `items[64]` (entity pointers) | `0x1361C` | `SpawnItemEntity` (searches the 64 entries for a free slot) and `0219B700` |
| `+0x114B4` | `traps[64]` (entity pointers) | `0x1371C` | `ResetFloor` clears all 64. Filled by `FUN_ov_0001__021428F8`; trap entities are claimed by `FUN_ov_0001__02142068`. See the Traps note below |
| `+0x11314` | `teamPokemon[4]` (entity pointers) | `0x1357C` | `GetLeader` |
| `+0x11324` | `wildPokemon[16]` (entity pointers) | `0x1358C` | five loops over 16 entries with `EntityIsValid` (`0219F09C`, ...) |
| `+0x11364` | active team list, 20 slots, each checked for a valid entity | not matched | `EnemyEvolution` (`ov_0001::02183BB4`), `sub_8044AB4` (`ov_0001::021406F8`) |
| `+0x15FB0`, `+0x15FB1` | `unk181e8.unk18218`, `unk18219` | `0x18218`, `0x18219` | `RunDungeon_Async` |
| `+0x19DF6` | `unk1BDD4.unk1C05E` | `0x1C05E` | `RunDungeon_Async` |
| `+0x1A2F8`–`+0x1A308` | `floorProperties` fields | `0x1C576`–`0x1C588` | `RunDungeon_Async` ([`dungeon-generation.md`](dungeon-generation.md)) |

The shift grows: −8 at `0x5C0`, −0xC at `unk644`, −0x4A0 at `0x3800`, −0x560 at `0x3A0E`, −0x2160 at `unkE27C`, −0x2960 at `playerSpawn` and `stairsSpawn`, −0x2268 from `traps` on. From the tiles to the tile pointers, the extra shift is exactly 4 bytes per tile (56 × 32 map tiles plus 64 fixed-room tiles).

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
| `+0x02` | `spawnOrVisibilityFlags` (u16) | `0x04` | spawn word, see [tile bits](#tile-bits-165-p2): used by `SpawnNonEnemies` (`0x1` stairs, `0x2` item, `0x4` trap), `SpawnEnemies` (`0x8`), and `ResolveInvalidSpawns` (clears `0x4` when `0x1` or `0x2` is set). `SpawnNonEnemies` also gates on terrain bits `0x8`, `0x20`, `0x40` and `0x100` (the terrain word at `+0x00`) |
| `+0x04` | `unk8`: graphics variant, picked with the global RNG; cosmetic ([`dungeon-generation.md`](dungeon-generation.md#emulator-check)) | `0x08` | |
| `+0x05` | `room` (`0xFF` = none) | `0x09` | written by `CreateRoomsAndAnchors` (`ov_0001::0214F7A8`) as a counter from 0; anchor tiles get `0xFE`; `ResetTile` sets `0xFF` |
| `+0x06` | `walkableNeighborFlags[4]` | `0x0A` | one byte per crossable-terrain kind |
| `+0x0A` | `unkE` | `0x0E` | |
| `+0x0C` | `monster` (entity pointer) | `0x10` | |
| `+0x10` | `object` (item or trap entity pointer) | `0x14` | |

### Tile bits (#165 P2)

A tile has two 16-bit flag words, and the same bit numbers mean different things in each. The terrain word is `+0x00`; the spawn word is `+0x02`. Every note must say which word a bit is in. Names are **[red-hypothesis]** from `pmd-red`'s `include/structs/map.h`. The uses are **[blue-verified]** (static Ghidra, decompiles of `ov_0001`).

Terrain word (`+0x00`):

| Bit | Name (red-hypothesis) | Blue evidence |
|-----|-----------------------|---------------|
| `0x4` | `TERRAIN_CORNER_CUTTABLE` | `StairsAlwaysReachable` |
| `0x8` | `TERRAIN_NATURAL_JUNCTION` | `SpawnNonEnemies` "plain" test; `FUN_ov_0001__021983F8` refuses it for a trap |
| `0x10` | `TERRAIN_IMPASSABLE_WALL` | `ResetFloor` border; `ResolveInvalidSpawns` |
| `0x20` | `TERRAIN_SHOP` | "plain" test; `FUN_ov_0001__021983F8` refuses it |
| `0x40` | `TERRAIN_IN_MONSTER_HOUSE` | `SpawnNonEnemies` steps 2 and 4; set on the stairs room when floor type 2 |
| `0x100` | `TERRAIN_UNBREAKABLE` | "plain" test; `ResolveInvalidSpawns` |
| `0x200` | `TERRAIN_STAIRS` | set by `ResolveInvalidSpawns`; `FUN_ov_0001__021983F8` refuses it |

Spawn word (`+0x02`):

| Bit | Name (red-hypothesis) | Blue evidence |
|-----|-----------------------|---------------|
| `0x1` | `SPAWN_STAIRS` | `SpawnNonEnemies` step 1 |
| `0x2` | `SPAWN_ITEM` | `SpawnNonEnemies` steps 2 to 4 |
| `0x4` | `SPAWN_TRAP` | `SpawnNonEnemies` step 5; `FUN_ov_0001__02198824` reads it to place traps |
| `0x8` | `SPAWN_MONSTER` | `SpawnEnemies` blocked-tile bit |
| `0x10` | `SPAWN_SPECIAL_TILE` | stairs candidate exclusion (role by Red only) |
| `0x20` | `SPAWN_SECONDARY_STRUCTURE` (Red `unk5`) | gate in the item chance table (role by Red only) |
| `0x40` | `SPAWN_WARP_TILE` (Red `unk6`) | `FUN_ov_0001__02198824` forces trap id 6 (the warp trap in Red's enum) |

`0x100` and `0x200` exist only in the terrain word; `0x2`, `0x4`, `0x8`, `0x20` and `0x40` exist in both words with different meanings. Blue-verified roles: spawn `0x4` and `0x40` (read by `02198824`) and terrain `0x200`; the other roles rest on the uses above or on Red. Trap ids 6 (warp) and `0x11` (wonder) come from Red's enum and are not checked in Blue's trap table.

**Rooms.** The room data is per tile: the room index is in `+0x05`. No room-bounds array is in the dungeon struct (medium confidence; see open questions). The 30-byte cell grid (`x0`/`y0`/`x1`/`y1` at `+0`/`+2`/`+4`/`+6`, room flag at `+0x1C`) is generation scratch passed in by the caller, `GenerateStandardFloor` and its siblings. `gNumRoomsAssigned` (`ov_0001::021B45BC`) is a counter local to `AssignRooms` (`ov_0001::0214FB4C`): its six references are all inside it. It counts the cells that the call marks as having a room, and it is read only inside that call. **[blue-verified, high]**

The dungeon struct does keep per-room junction tiles, in `+0xE5E4` and `+0xE624`. `FUN_ov_0001__0214D82C` fills them from the tiles with terrain bit `0x8`. It is called as the last step of `FinalizeJunctions` (`0214DB0C`) and from `FUN_ov_0001__021946FC` (`02194A00`). The AI reads them in `FUN_ov_0001__02190854` and `FUN_ov_0001__02190B80`. **[blue-verified for the layout and the readers; medium for completeness]**

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

**Floor items** [blue-verified]: 64 entity pointers at dungeon `+0x113B4` (Red `0x1361C`). `SpawnItemEntity` (`ov_0001::02141F58`) takes a free slot. It sets the entity type to 3 (`ENTITY_ITEM`), copies the position to `+0x4`/`+0x6`, and sets `entity+0x70` to the 3-byte `Item` slot `gDungeon + 3·i + DAT_02142064`. The base `DAT_02142064` is `0x3364` (blue-verified; read from the overlay), so the array is dungeon `+0x3364` to `+0x3423`: 64 slots of 3 bytes, with the info of item entity `i` at `gDungeon + 0x3364 + 3·i`. `SpawnItem` then copies the `Item` into the entity's info, sets `isVisible` and puts the entity in the tile's `object` field.

**Traps** [blue-verified for the writers and flows; red-hypothesis for the names]: `traps[64]` (`+0x114B4`) holds entity pointers. `FUN_ov_0001__021428F8` fills the 64 pointers, each to a 0x74-byte entity of type 0 (`*(gDungeon + i*4 + 0x114B4) = i*0x74 + gDungeon + DAT_02142A98`). It also initialises the team, enemy and item arrays. It has no direct caller. `FUN_ov_0001__02142068` (red-hypothesis name `SpawnTrap`) claims the first invalid entity: type 2 (trap), `+0x70` = the 2-byte trap record at `gDungeon + DAT_0214211C + i*2` (id, flags), and the position at `+4` and `+6`. Its callers:
- `FUN_ov_0001__02198824` (red-hypothesis name `SpawnInitialTraps`), after generation: for each tile with spawn bit `0x4`, id 6 if the tile has spawn bit `0x40`, else a weighted draw `FUN_02138C68` from the floor's trap-weight table. Id `0x11` gets flags 2 and `isVisible`. Its caller is not found (see open questions).
- `FUN_ov_0001__02152FAC` (called from `FUN_ov_0001__02152B20`): fixed-room traps. The id is the fixed-room table byte `+0xB`, used when it is not `0x14`, and `+0x20` is table byte `+0xD`.
- `FUN_ov_0001__021983F8` (runtime placement; called from `02198330` and `0215FEEC`): places a given trap id on a tile, reusing an existing trap entity if there is one. It refuses a tile with terrain `0x200`, `0x8` or `0x20`, a terrain type other than 1, or room `0xFF`.
- `02199B40` (suspend-save restore).

Trap placement is not in `GenerateFloor` (`ov_0001::02151E7C`) itself. `GenerateFloor` only marks tiles with spawn bit `0x4`, and for fixed rooms it reaches `FUN_ov_0001__02142068` through `02152B20` and `02152FAC`. The random traps come from `02198824`, which runs after `GenerateFloor`.

## Recruited Pokémon

The team roster outside dungeons (Red `Pokemon`). Offsets read from `WritePoke1Bits` (`02003E1C`), which writes each field in turn; they match `pmd-red`'s Blue notes in `include/structs/str_pokemon.h` field for field. Size `0x40` (Red `0x58`).

| Blue | Field | Red |
|------|-------|-----|
| `+0x03` | `level` | `0x03` |
| `+0x04` | `dungeonLocation` | `0x04` |
| `+0x06` | `speciesNum` (the species id, #170 P3b; [blue-verified], see [Roster record +6](#roster-record-6-is-the-species-id-170-p3b)) | `0x08` |
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

### Roster move layout (#239 P2c)

The 6-byte move in a roster record (`+0x1E`, four of them) is not the dungeon `Move`. Save widths (`WritePoke1MovesBits` at `0x020094D8`, mirrored by the reader `FUN_020094A0`): `+0` 4 bits (`moveFlags`), `+2` 9 bits (move id), `+4` 7 bits (the ginseng value). The save writes neither `+1` nor `+5`. [blue-verified]

- `FUN_0200978C` converts a roster move to a dungeon `Move` (callers `FUN_02001E40`, `ov_0001::02178828`, `ov_0001::0217A754`). For each of the four moves: if `src[0] & 1` is 0, `dst[0] = 0`. Otherwise `dst[0] = src[0]`, `dst[1] = 0`, `dst[2..3] = src[2..3]` (the id), `dst[4]` = the move's max PP from the move table (`id × 0x24 + 0xC`), and `dst[5] = src[4]` (the ginseng). So PP is derived at load and not stored, and roster `+4` becomes dungeon `+5`. [blue-verified]
- Roster `+1` (`moveFlags2`) is ignored on the way in (`dst[1] = 0`). At a new game it is `0x02` in the first two moves of both in-use records (one state, runtime), so the save drops a nonzero value. The code that writes it was not found. [blue-verified reads; runtime for one state]
- Roster `+5` is not read by the converter and is unused in this format. [blue-verified]
- The dungeon `Move` (info `+0xF4`) keeps all six bytes; the quick-save serializer `ov_0001::0219A444` writes all six. [blue-verified]
- Which code sets the ginseng of a roster move after a dungeon is **[open]**: no info-to-roster copy of moves was found. [hypothesis]

### Roster record +6 is the species id (#170 P3b)

Verdict: **[blue-verified]**, high. Roster `+6` (s16) is the species id (Red `Pokemon.speciesNum`, Red `0x08`). The mail record's `+0xC` (`clientSpecies`) therefore copies a species id. The roster is 413 records of `0x40` bytes. Its base is not a fixed address: it is read from the cell at `0x020DDF84`, whose boot-time value is `0x020E0F8C` (in `arm9_bss`) **[blue-verified]** for the cell and its readers, static only for the value (literal reads; runtime read not yet run). Record `+0` bit 0 is "in use".

Writers (static, Ghidra copy `rescue-arm9-c`; names are **[hypothesis]**):
- `FUN_020033A8` (proposed `InitRosterRecord`): builds a fresh record. `+6` takes the species argument verbatim, `+0` is set to 1 and `+3` (level) to 1. Every other derived field is a lookup keyed by the species (for example `FUN_020022C0`, `FUN_0200224C`, `FUN_0200222C`, `FUN_020016B4`, `FUN_020026D0`).
- `FUN_02003130` (proposed `AddRosterRecord`): inserts the built record. It reads the species at `+6`, indexes the species table (stride `0x48`, base `*DAT_02003210`) for the key at `+0x16`, finds a free slot whose `FUN_02009230(slot)` equals that key, copies the 0x40 bytes, clears bit `0x4000` of `+0`, and calls `FUN_020115BC(species)` (not decompiled).
- `FUN_0200303C` is `FUN_020033A8` followed by `FUN_02003130`. `FUN_02003080(species)` finds the free slot without writing.

Serialization: `WritePoke1Bits` (`02003E1C`) writes `+6` as 9 bits (0 to 511), which holds the species table size `0x1A8` (424). The read side (`ReadPoke1Bits`) was not decompiled; [hypothesis] it mirrors the write. `FUN_02003AD0` writes a different struct (`+8` and `+0xA` are 16 bits each, `+0xC` is 9 bits), so it is not this record; not examined.

Readers (all use `+6` as a species index):
- `FUN_02002838`: for active and team records, `FUN_020023E0(species)`, which is the per-species cost byte at `+9` of the `0x48` species entry. It sums these and tests `< 7` (a team limit of at most 4 members).
- `FUN_0200D4DC` (mail): `mail+0xC = *(s16 *)(FUN_02002F88() + 6)`.
- `FUN_0206A6DC` (mission species filter, #168 P3b): compares the mission species with `FUN_02002F88()+6` and with `FUN_02002F0C()+6` (the partner, the record whose `+4` is `'A'`), after a `FUN_020010E8` round trip.
- `FUN_02002EA4` and `FUN_02002F0C` find records by `+4 == '@'` and `+4 == 'A'` (Red `dungeonLocation`, `+0x04`).

The value is an index into the species table, not a roster id. `FUN_020010E8` folds the Unown form ids `0xCA` to `0xE2` into `0xC9` (201), so `+6` is the species or form id.

**Not verified:** the read side of the save (`ReadPoke1Bits`) and the load path that sets `+6`; `FUN_020115BC`, `FUN_02009230` and the table at `*DAT_02003214`; writers outside the constructor (evolution or form change writing `+6` in place), since only the `0x02002000` to `0x02003F00` helpers were scanned; runtime (no emulator check).

## Dungeon data table (#170 P1b)

The table at `0x02092818` holds one `0x10`-byte record per dungeon, Red's `DungeonDataEntry` (`include/dungeon_data.h`, read only). The layout matches Red field by field on records 0 and 1 **[blue-verified]**. That record 0 starts at `0x02092818` is **medium**: the bytes before it belong to another table, and the start was fixed by the alignment of the `1000` field. Records continue in `0x10` steps (the rows dumped to `0x02092A7C` follow the pattern).

| Offset | Red field | Records 0 and 1 |
|--------|-----------|-----------------|
| `+0` | `stairDirectionUp` | 0, 0 |
| `+1` | `enemiesEvolveWhenKOed` | 0, 0 |
| `+2` | `recruitingEnabled` | 0, 1 |
| `+3` | `rescuesAllowed` (s8) | 0xFF, 0xFF |
| `+4` | `maxItemsAllowed` | 0x14, 0x14 |
| `+5` | `maxPartyMembers` | 3, 3 |
| `+6` | `levelResetTo1` | 0, 0 |
| `+7` | `keepMoney` | 1, 1 |
| `+8` | `leaderCanSwitch` | 0, 0 |
| `+9` | `hasCheckpoint` | 0, 0 |
| `+0xA` | `enterWithoutGameSave` | 1, 1 |
| `+0xB` | `HMMask` | 0, 0 |
| `+0xC` | `turnLimit` (s16) | 0x03E8 (1000), 0x03E8 |
| `+0xE` | `randomMovementChance` (s16) | 0, 0 |

`FUN_02006410(index)` is `GetRescuesAllowed` (proposed; red-hypothesis name). It returns byte `+3` of record `index`, that is `0x0209281B + index * 0x10`. Both callers use this one global table: the mail builder `FUN_0200D4DC` (index from mail `+4`) and `RunDungeon_Async` (`0x0206DCEC`, index from the setup struct's `+4`). Records beyond 1 are not mapped to dungeon ids or names.

## Roster and mail helpers (#170 P1, proposed names)

Static evidence from the investigator comment on [#170](https://github.com/TorratDev/rust-mm/issues/170), read in the Ghidra copy `rescue-arm9-a`. Read-only: nothing is renamed in the project. The names are proposals for the orchestrator, not applied renames.

Tags: **[blue-verified]** for the behaviour, callers and literal-pool values. **[red-hypothesis]** for the names: they are our own, not taken from the repo or `pmd-red`.

Literal-pool values [blue-verified]:

| Literal | Value | Used for |
|---------|-------|----------|
| `DAT_02002FFC` | `0x020DDF84` | address of the cell that holds the roster base (not the roster); the cell is `0x020DDF84` and its boot value `0x020E0F8C` is stored by `FUN_02003794` (literal at `0x02003810`, called unconditionally from boot init `FUN_0201F2E4`) and by `FUN_0202A778` via `FUN_02003788` (literal at `0x02003790`) **[blue-verified]** for the stores, static only for the value |
| `DAT_02003000` | `0x19D` (413) | roster record count |
| `DAT_020024E0` | `0x02008C0C` | character-copy routine |
| `DAT_02006420` | `0x0209281B` | `rescuesAllowed` of dungeon record 0 (table base `0x02092818`, stride `0x10`; see [Dungeon data table](#dungeon-data-table-170-p1b)) |
| `DAT_02008C04` | `0x02091C44` | error text used by the decoder |
| `DAT_0200D588` | `0x020DF6EC` | mail-record pointer block |

The record size and count match [`rng.md`](rng.md) and [#161](https://github.com/TorratDev/rust-mm/issues/161) P2 (records of 0x40 bytes, 413 of them). The base is read through the cell `0x020DDF84` (see the table row above), not stored at a fixed address.

**`FUN_02002F88` at `0x02002F88`: proposed `FindFirstActiveRosterRecord`** [blue-verified behaviour; red-hypothesis name]
- Loads `*DAT_02002FFC` (the cell `0x020DDF84`, boot value `0x020E0F8C`) and scans the 413-entry roster from that base in steps of `0x40`. Returns the first record whose `u16` at `+0` has bit 0 set and whose byte at `+2` is nonzero. Returns 0 if none matches.
- Callers: 38 sites in 33 functions (ARM9, overlays 0 and 7). Examples: `FUN_0200D4DC`, `FUN_0201E020`, `FUN_02052AAC`, `FUN_0206A6DC`, `FUN_02069848`.
- Handling of the 0 return (#170 P1b; blue-verified sites, medium classification by eye):
  - Test for 0: `FUN_0201E020`, `FUN_02067AE0`, `FUN_0206964C`, `FUN_ov_0007__02139054`, `FUN_ov_0007__02139A98`, `FUN_ov_0007__02139F80`; `FUN_ov_0000__02140860` at 2 of its 3 sites.
  - Assert, then use: `FUN_ov_0000__021548EC` (`hero index error`).
  - Compare only: `FUN_0203C9D8`, `FUN_ov_0000__02154D14`.
  - Store the pointer without dereferencing: `FUN_0203CB30`, `FUN_02047D74`, `FUN_0204873C` (a later reader may dereference).
  - Dereference with no check: `FUN_0200D4DC`, `FUN_02028628`, `FUN_02029734`, `FUN_0206A6DC`, `FUN_0204A40C`, `FUN_0204BDB8`, `FUN_0204F76C`, `FUN_0205F598`, `FUN_02042378`, `FUN_02052AAC`, `FUN_02057FE8`, `FUN_0203A244`, `FUN_02058A9C`, and the first site of `FUN_ov_0000__02140860`. `FUN_ov_0000__02153884` returns the pointer as is for some cases.
  - So about 13 functions test or assert, and about 15 dereference unchecked. That a hero record always exists in play is a hypothesis: it was not checked at game start or on the title screen.
- Confidence: high for the behaviour; medium that the array is the 0x40-byte roster (the pointer and count match `rng.md`).

**`FUN_020024D0` at `0x020024D0`, `FUN_020024D0(dst, rec)`: proposed `CopyRosterNameToBuffer`** [blue-verified behaviour; red-hypothesis name]
- Calls `FUN_02008C0C(dst, rec + 0x36, 10)`: copies up to 10 characters. `FUN_02008C0C` copies each byte through `FUN_02008C50`, which is an identity stub (`bx lr`), so the bytes are copied unchanged [blue-verified]; then NUL-terminates. The `+0x36` field is `name[10]` in the Recruited Pokémon table above.
- Callers: `FUN_0200D4DC`, `FUN_02052AAC`, `FUN_02065844`, `FUN_02057FE8`, `FUN_02038C28`, `FUN_02038D48`, `FUN_02042378`, `FUN_02052560`.
- Confidence: high for the offset and length; medium for the record identity.

**`FUN_02008AF0` at `0x02008AF0`, `FUN_02008AF0(dst, src)`: proposed `DecodeHexEscapes`** [blue-verified behaviour; red-hypothesis name]
- Copies `src` to `dst`. Each `~` followed by two hex digits (`0-9`, `a-f`, `A-F`) becomes one byte, high nibble first. Other bytes are copied unchanged. A bad digit prints the error at `0x02091C44` and gives a 0 nibble. Writes a 0 terminator at the end.
- Callers: `FUN_0200D4DC`, `FUN_0201E020`, `FUN_02004590`, `FUN_02002788`, `FUN_0219C898` (three sites), `FUN_02069848`, `FUN_021461D0` (overlay 0), `FUN_0206C170`.
- Confidence: high for the behaviour (read from the code).

**`FUN_02006410` at `0x02006410`, `FUN_02006410(index)`: proposed `GetRescuesAllowed`** [blue-verified behaviour; red-hypothesis name]
- Returns `rescuesAllowed` of dungeon `index`: the byte at `0x0209281B + index * 0x10` (see [Dungeon data table](#dungeon-data-table-170-p1b)).
- Callers: `FUN_0200D4DC` (index = the byte at the mail record's `+4`, stored at `+0x2C` of the record) and `RunDungeon_Async` (`0x0206DCEC`, index = the setup struct's `+4`).
- Confidence: high for the behaviour; medium for the table start (record 0).

**Mail-record builder `FUN_0200D4DC` at `0x0200D4DC`: context only** [blue-verified structure; not checked against the mail format]
- Writes a flag of 1 at `+0`, copies two parameters to `+4` and `+8`, and seeds the Mersenne Twister with `Rand32Bit`. Stores a random value at `+0x10`, a roster `u16` at `+0xC`, the decoded text at `+0x14`, `sub_8011C34()` at `+0x24` and the table byte at `+0x2C`.
- [#47](https://github.com/TorratDev/rust-mm/issues/47) named this function `sub_809542C`. The structure is consistent with a mail record; this was not checked against the mail format.

Open, not verified (after #170 P1b and P3b):
- The table beyond record 1 is not mapped to dungeon ids or names; record 0's start is medium.
- Which roster index the hero always has, and whether a hero record exists at game start or on the title screen (not checked).
- `FUN_02002F88` returns 0 when nothing matches; about 15 callers dereference without a check (see the list above).
- Runtime: no emulator check (no Blue dump configured in this session).

## Mail record (#170 P3)

The mail record that `FUN_0200D4DC` (`sub_809542C` in [#47](https://github.com/TorratDev/rust-mm/issues/47)) builds. Offsets written by the builder, compared with pmd-red's `include/structs/str_wonder_mail.h` (`unkStruct_203B480`) and `src/code_8094F88.c` (`sub_809542C`). Source: the #170 P3 comment by investigator A; static, Ghidra copy `rescue-arm9-a`, no emulator check.

Tags: **[blue-verified]** for the writes; the Red field names are **[red-hypothesis]**.

| Blue offset | Written by | Red field | Match |
|-------------|-----------|-----------|-------|
| `+0x00` (u8) | `= 1` | `mailType` at `0x0` | offset and value (1 in both) |
| `+0x04`, `+0x08` (u32 each) | copies of `param_1[0]` and `param_1[1]` | `dungeonSeed` (`DungeonMailSeed`, 8 bytes) at `0x4` | size and position |
| `+0x0C` (u16) | roster record `+6` | `clientSpecies` at `0xC`, set from `GetLeaderMon1()->speciesNum` | position; the source is high: roster `+6` is the species id ([Roster record +6](#roster-record-6-is-the-species-id-170-p3b)) |
| `+0x10` (u32) | `Random32MersenneTwister()` after `MersenneTwister_InitializeState(Rand32Bit())` | `unk10` at `0x10` | position and call sequence |
| `+0x14` (10 bytes) | roster name through `FUN_020024D0`, then `FUN_02008AF0` (decoded) | `playerName[10]` at `0x14` | position; Red's `sub_809542C` has no decoding step |
| `+0x24` (u32) | `sub_8011C34()` | `unk24` at `0x24` | same call |
| `+0x2C` (u8) | `FUN_02006410(byte at mail +4)` | `rescuesAllowed` (`s8`) at `0x2C`, set from `GetRescuesAllowed(dungeonSeed.location.id)` | offset matches; value is `rescuesAllowed` (dungeon table field `+3`, see [Dungeon data table](#dungeon-data-table-170-p1b)) |

Not written by `FUN_0200D4DC`: `+0x01` (missionType), `+0x20` (`item` in Red) and `+0x28` (`unk28` in Red). Red sets these elsewhere; the Blue writers were not looked for.

**Resolved (#170 P1b): the byte at `+0x2C` is `rescuesAllowed`.** `FUN_02006410` returns byte `+3` of the dungeon data record, and in Blue's table that byte is `rescuesAllowed` (the layout matches Red's `DungeonDataEntry` field by field; the record 0 start is medium). See [Dungeon data table](#dungeon-data-table-170-p1b).
- Red reads `rescuesAllowed` at `gDungeon->unk644.unk37` (`src/run_dungeon.c` 139). Blue's byte that `RunDungeon_Async` stores at `gDungeon + 0x66B` was not compared with that field, so the `+0x66B` layout is still unchecked.

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
- **Status write sites (#166 P1b, P1c, P2b), what is still open.** Writers reached only through a function-pointer table were not followed. The move wrappers `021612D0`, `0216365C` and `02163070` are not named after their moves. The callers of the leech setters `02163210` and `021635C0` are not traced. The ARM9 write `*(gDungeon+0xB3) = 0` at `0206CD9C` is a Dungeon field, not classified. Whether `FUN_ov_0001__02190100` (the end-routine caller at `0219019C`) ends the curse on faint or despawn is not verified. The third argument of the turn roll at each call site is not read: the muzzled setter `0218C1AC` shows two arguments in the decompile. Muzzled `+0xCB` is searched in ov_0000, ov_0001 and the ARM9 only, not in the other overlays. The message text of each cringe setter is not decompiled. Runtime: no emulator check. Tags: blue-verified for write sites and offsets; red-hypothesis for the names; [hypothesis] for the class names.
- **Trap writer caller.** `FUN_ov_0001__02198824` (random trap placement after `GenerateFloor`) has no caller found: no x-ref, no branch encoding and no pointer literal in ov_0001 or ov_0000. It may be called through a function-pointer table or from another overlay.
- **Floor-item array.** The reader of `numItems` (Blue `+0x3424`, Red `0x3904`) is not found. Only `SpawnItemEntity` references the `0x3364` literal; other writers through the array are not ruled out.
- **`+0x34AC` readers.** The meaning of the write of 0 in `021946FC` and of the two thresholds in `021826A0` is not verified.
- **Tile spawn bits 0x10 and 0x20.** Their roles are by Red's names only. Trap ids 6 (warp) and `0x11` (wonder) come from Red's enum and are not checked in Blue's trap table.
- **Rooms.** No room-bounds array was found. Still open: the global `DAT_0214FD78`, which `AssignRooms` writes after its count (not resolved); whether other overlays or ov_0000 read `+0xE5E4` or `+0xE624` (the search covered ov_0001 only); and the values of `DAT_02190B7C` and `DAT_02190FE4`, since the pool is unmapped in this view, so the `0xE624` base is inferred from `0214D82C`.
- Add the remaining trace fields (statuses, moves, held items, floor items, inventory) to `verification/v7-trace/retail_trace.py` and check them by emulator.
