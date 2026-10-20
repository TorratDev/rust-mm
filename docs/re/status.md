# Status effects

How each status is applied, how long it lasts, what it does each turn, how it ends and is cured, and which RNG calls it makes. Issue: [#51](https://github.com/TorratDev/rust-mm/issues/51), part of Epic #5. Names, addresses and tags are in [`symbols.md`](../symbols.md). The turn loop that calls these checks is in [`turns.md`](turns.md).

**Evidence: static only.** Every row comes from Ghidra decompile and cross-reference queries on the retail ARM9 and overlay 1, from the overlay 1 bytes (file offset = RAM address − `0x02138860`), and from comparison with the pinned `pmd-red` commit `013475aa04f5be3191e5527c186d9bfceae7cae0`. **No py-desmume run was made**, so no per-turn claim here is verified at runtime. Read every "partial", "not verified" and "open" note as a limit on what this page shows.

Tags: **blue-verified** means the role was read in Blue's code. **red-hypothesis** means the name or mapping comes from `pmd-red` and Blue has not matched it yet. **tentative** means our own name, with the role only partly known. Confidence is high, medium or low.

## Status classes

Each class has a status byte and a turns or counter byte. The offsets are in [`structs.md`](structs.md#entity-info). Blue value to Red enum is partial: only the values with a Blue writer or a Blue message case are listed as blue-verified.

| Blue field | Class | Value → Red name | Tag |
|------------|-------|------------------|-----|
| `+0x99` status, `+0x9A` turns | sleep | 1 `STATUS_SLEEP`; 3 `STATUS_NIGHTMARE`; 5 `STATUS_NAPPING`; 4 `STATUS_YAWNING` | 1, 3, 5: blue-verified writes. 4: blue-verified write in `0218ADA8` (medium, see open items), red-hypothesis as a wake path in C's catalogue |
| `+0x99` | sleep | 2 `STATUS_SLEEPLESS` | red-hypothesis: a message case only; no writer found |
| `+0x9B` status, `+0x9C` turns | burn | 1 `STATUS_BURN`; 2 `STATUS_POISONED`; 3 `STATUS_BADLY_POISONED`; 4 `STATUS_PARALYSIS` | blue-verified writes for 1 to 4 (4: partial, see below) |
| `+0x9D` | burn | burn damage countdown | blue-verified (read and reset per turn) |
| `+0x9E` | burn | bad-poison damage index | blue-verified (incremented per turn) |
| `+0xA0` status, `+0xA8` turns | frozen | 1 `STATUS_FROZEN`; 3 `STATUS_WRAP`; 4 `STATUS_WRAPPED`; 2 shadow hold, 5 ingrain, 6 petrified, 7 constriction | 1: blue-verified write and message. 2 to 7: message routes blue-verified; names red-hypothesis |
| `+0xAC` status, `+0xAD` counter | cringe | 2 `STATUS_CONFUSED`; 7 `STATUS_INFATUATED`; 1 cringe, 3 paused, 4 cowering, 5 taunted, 6 encore | 2 and 7: blue-verified writes. Others: red-hypothesis message cases. The `+0xAC` offset is inferred from the Own Tempo cure, not from a documented field |
| `+0xAE` | bide | — | not investigated |
| `+0xB1` status, `+0xB2` counter | reflect | 2 `STATUS_SAFEGUARD`; 12 `STATUS_CONVERSION2`; 1 to 14 in the end switch | 2: blue-verified gate. 12: red-hypothesis setter. 1 to 14: red-hypothesis mapping of blue-verified cases |
| `+0xB3` status, `+0xB6` counter | curse | 1 cursed, 2 decoy, 3 snatch (Red's curse class) | red-hypothesis (strong) |
| `+0xB8` status, `+0xC1` counter | leech seed | — | red-hypothesis (class guessed from the offset) |
| `+0xC4` status, `+0xC5` counter | sure shot | — | blue-verified read in the chain; end routine not read |
| `+0xC7` status, `+0xC8` counter | unnamed class | — | blue-verified read in the chain; class unclear (transform or invisible) |
| `+0xC9` status, `+0xCA` counter | unnamed class | — | blue-verified read in the chain; blinker is a guess (three values) |
| `+0xCB` status, `+0xCC` counter | muzzled | — | blue-verified read in the chain; class not checked |
| `+0xD7` | terrified | flag and timer share the byte | blue-verified read in the chain |
| `+0xE0`, `+0xE4` to `+0xE8`, `+0xE9` to `+0xED` | speed stage and counters | — | blue-verified (see [`structs.md`](structs.md#entity-info)) |

## Per-turn order

`TickStatusAndHealthRegen` (`ov_0001::02185C70`) runs the countdowns and the status chain. The chain is in the table below. Each step re-checks that the entity is valid and that the floor is not over before it runs. The order was read from the decompile and matches Red's `TickStatusAndHealthRegen` step for step (red-hypothesis for the names, blue-verified for the order).

**Call sites (four, not one).** The function is called from `02140BA0` (in `sub_8044574_Async`), `021409CC` (in `sub_8044820_Async`), `02140F40` (in `FUN_02140ED0`) and `021410A4` (in `RunLeaderTurn_Async`). The counters therefore tick once per call from four places, not only once per gated monster turn. The leader's call is one of the four. Position in the turn matters for #49, see [`turns.md`](turns.md).

| # | Step | Blue flag or timer | Handler | Red counterpart (red-hypothesis) |
|---|------|--------------------|---------|----------------------------------|
| 0 | fixed call with argument `0x50` | — | indirect call | `sub_805229C` → `sub_80526D0(0x50)`; no RNG visible |
| 1 | HP regeneration | gate: entity info `+0x118` is 0, and `+0x9B` is not 2 or 3 | inline | HP heal block |
| 2 | sleep class | `+0x99`, `+0x9A` | `0218FE64(e, e, 1, 1)` | `EndSleepClassStatus(e, e, T, T)` |
| 3 | burn class | `+0x9B`, `+0x9C` | `0218FD80` | `EndBurnClassStatus` |
| 4 | frozen class | `+0xA0`, `+0xA8` | `0218FC18` | `EndFrozenClassStatus` |
| 5 | cringe class | `+0xAC`, `+0xAD` | `0218FA80` | `EndCringeClassStatus` |
| 6 | reflect class | `+0xB1`, `+0xB2` | `0218F814` | `EndReflectClassStatus` |
| 7 | curse class | `+0xB3`, `+0xB6` | `0218F6C8(e, e, 0)` | `EndCurseClassStatus(e, e, 0)` |
| 8 | leech seed | `+0xB8`, `+0xC1` | `0218F610` | `EndLeechSeedClassStatus` |
| 9 | sure shot | `+0xC4`, `+0xC5` | `0218F510` | sure shot end |
| 10 | `+0xC7` class | `+0xC7`, `+0xC8` | `0218F3F8` | invisible or transform (unclear) |
| 11 | `+0xC9` class | `+0xC9`, `+0xCA` | `0218F304` | blinker (eyesight) |
| 12 | muzzled | `+0xCB`, `+0xCC` | `0218F27C` | muzzled |
| 13 | terrified | `+0xD7` (flag and timer share the byte) | inline | `terrifiedTurns` |
| 14 | speed counters | `+0xE9 + i` (down), `+0xE4 + i` (up), `i` = 0 to 4 | inline | speed loop |

- **Countdown helper** `0219BF1C` (`DecrementStatusTurns` in [`symbols.md`](../symbols.md)) decrements a byte unless it is `0x7F`, and then the class end runs when it reaches 0. One finding reports only `0x7F` as the exception. Another also reports `0` as left alone. Not resolved (see open items). Red's `sub_80838EC` is the same routine (red-hypothesis).
- **Speed message:** if a counter in step 14 reaches 0 and `CalcSpeedStage` (`0x02181E58`, no RNG) changes the speed stage, the speed-stage message is logged.
- **RNG reached from the chain.** Depth-limited x-ref walk (depth 2; indirect calls not followed); confidence medium-high. `TickStatusAndHealthRegen` calls no RNG directly. The walk reaches these RNG calls:
  - step 2, case 4 (yawn): `0219CF28` → `DungeonRandRange` `0219D014` → `DungeonRandInt` `0219D058`;
  - step 2, case 3 (nightmare, fourth argument nonzero): `0217BCA4` → `0217E890` (`HandleDealingDamage_Async`) → `DungeonRandInt`;
  - step 7, curse case 2 (decoy end): `0217AF58` → **global** `Rand32Bit` `0201198C` (`& 3` = facing);
  - step 10, `+0xC7` case 2: the same `0217AF58` → `Rand32Bit` path;
  - all other steps: none; step 0: none visible (indirect).

**Per-turn blocks in `021861E8`** (`DoEndOfTurnEffects_Async`, red-hypothesis name). This function runs once per monster at the end of the turn and holds the burn, poison and bad-poison damage, the yawning branch and Shed Skin. Its callers are `02140DD8` (in `sub_8044574_Async`), `02140A84` (in `sub_8044820_Async`), `02187B68` (`DisplayActions_Async`) and `02184DF8` (in `FUN_0218453C`). The per-class sections give the detail.

## Sleep class (`+0x99`, `+0x9A`)

Sleep, nightmare, napping and yawning share one byte. A status can be set only where its setter allows it: see the blocked cases below.

### Sleep: application and immunity

- **Entry points.** Each entry point is a wrapper that rolls the duration and then calls a setter. Wrappers: `0215E07C` (sleep), `0215E03C` (nightmare), `0215E0C0` (yawning), `02161370` (napping). They are reached from the move-effect dispatcher `FUN_02156824`, by an unconditional call (x-ref). The dispatch table was not traced.
- **Roll before the immunity check.** The wrapper rolls first, then the setter runs the immunity check. The RNG is therefore consumed even when the target is immune. Red's `sub_8075BF4` / `SleepStatusTarget` also takes a turn count computed first, so the order matches (red-hypothesis for the Red side). **blue-verified, high.**
- **Setters.** Sleep: `0218B2D0` (with message), then `0218B420` (core). Nightmare: `0218B01C`. Yawning: `0218ADA8`. Napping: `0218AEF0`.
- **Blocks:** sleep is blocked when the status is already 1 or 3 (`0218B420` returns 1 or 2). Nightmare is blocked if already 3, and status 2 gives the sleepless message. Yawning sets only from status 0. Napping sets from 0 or 4.
- **Immunity gate `CannotSleep` (`0218B140`).** Order: Safeguard (`02180870`, only when the gate's third argument is nonzero); IQ skill `0x11` (`IQ_NONSLEEPER`, index 17, checked through `021805B0`); Insomnia (ability `0x36`); Vital Spirit (ability `0x49`); held item `0x15` (`ITEM_INSOMNISCOPE`, value 21). Returns 1 when blocked. **blue-verified, high.** The message texts were not read. The IQ and item IDs were matched to Red's enums by value, not by a Blue use of the names (medium).
- **Early Bird** (ability `0x2F`) halves the sleep turns through `FUN_0208625c(x, 2)` and keeps the result at least 1. It does not halve `0x7F`. **blue-verified, medium** (`0218B420`, `0218AEF0`, `0218B01C`).

### Sleep: duration and roll

| Status | Wrapper | Roll pair (overlay 1) | Red range | Match |
|--------|---------|-----------------------|-----------|-------|
| Sleep | `0215E07C` | `021AA324` = (3, 7) | `gSleepTurnRange` {3, 7} | yes |
| Nightmare | `0215E03C` | `021AA328` = (4, 8) | `gNightmareTurnRange` {4, 8} | yes |
| Yawning | `0215E0C0` | `021AA2BC` = (2, 2) | `gYawnTurnRange` {2, 2} | yes |
| Napping | `02161370` | `021AA344` = (2, 4) | `gNappingTurnRange` {2, 4} | yes |

- The roll itself is `CalculateStatusTurns` (`0219CF28`), which rolls `DungeonRandRange` on a (min, max) pair. For a monster it also adjusts the roll: a check with argument `0x12` halves it, and a check with argument `0x14` caps it at 5 when it is above 4. The checks come from `021805B0`. The meaning of `0x12` and `0x14` (ability or IQ) was not compared with Red's `CalculateStatusTurns` (medium).
- **Yawn-end roll.** `0218FE64` case 4 rolls `021AA2C0` = (4, 8), then sleeps for roll + 1 turns. The value equals Red's nightmare range, but the source is not confirmed. Red's yawn end goes through `sub_80420B8` → `sub_804151C(0x19, 1)`, which is not a sleep-range call (low).

### Sleep: countdown and per-turn

- **Countdown.** In the chain (step 2), if `0x99 != 0` and `0x9A` reaches 0 after `0219BF1C`, the code calls the sleep expiry `0218FE64(entity, entity, 1, 1)`. A turns value of `0x7F` never expires: the sleep lasts until it is woken or cured. **blue-verified, high.**
- **Per-turn yawning.** If `0x99 == 4`, `021861E8` calls `0213F6B0`. Red's `dungeon_turn_effects.c:200` to `202` calls `sub_80420B8`. The name of `0213F6B0` was not confirmed (medium).
- **Shed Skin** (ability `0x21`) runs `DungeonRandInt(100) < 50` (value `021AA090` = 50) and the negative-status check `FUN_02181904`, then calls the cure-all `02190100`. The roll runs once per monster per end of turn, whether or not the monster has a status. **blue-verified, high.**

### Sleep: expiry

`0218FE64` is the expiry switch on `0x99`:

| `0x99` | Action |
|--------|--------|
| 1 sleep | Wake message, clear `0x99`. |
| 2 sleepless | Message, clear. |
| 3 nightmare | Message, then damage 8 (`021AA11C`) through `0217BCA4` when the fourth argument is nonzero (the chain passes 1). Then clear. |
| 4 yawning | Clear `0x99`, roll `021AA2C0` = (4, 8), then `0218B2D0(entity, roll + 1, 1)`. |
| 5 napping | Message, heal 999 (`021AA1D0`) through `0218DF38`, clear `0x99`, then the cure-all `02190100`. |

- The switch is blue-verified (high). The targets are medium: the napping heal's argument order was not checked. `02190100` is treated as the cure-all, inferred from its body (it runs `FUN_02181904` first and calls `0218FE64`, `0218FD80` and other class enders).

### Sleep: wake on hit

Two checks, and they disagree:

- `0217D5FC` (the damage path): if `0x99` is 1, 3 or 5 and `0x9A == 0x7F`, it calls the wake `0218F1EC`. This matches Red's `dungeon_damage.c:279` to `280` (sleep, napping, nightmare). **blue-verified, medium.**
- `0218F21C`: wakes only when `0x99 == 1` and `0x9A == 0x7F`, through `0218FE64(…, 0, 1)`. It is called from `021861E8` and from the move resolver `02156824`. It does **not** cover nightmare or napping, so it disagrees with `0217D5FC`. Needs a trace or a second look before either is relied on (medium).

### Sleep: cures

- **Insomnia** (`0x36`) and **Vital Spirit** (`0x49`) are checked in `02175850` (ability cures). They call the sleep expiry when `FUN_021818D0` is nonzero or the status is yawning (`0x99 == 4`). So yawning is woken by these abilities. `FUN_021818D0` was not identified (medium).
- **Cure-all** `02190100`: Shed Skin and the napping end call it. It clears the negative statuses (inferred, medium).

### Sleep: RNG

- Sleep, nightmare, yawning and napping each consume one `DungeonRandRange` call at application, in the wrapper and before the immunity check (high).
- The yawn-end roll is one more call (high).
- Shed Skin consumes one `DungeonRandInt(100)` per monster per end of turn (high).
- The nightmare damage in the expiry switch reaches `DungeonRandInt` through `0217BCA4` → `0217E890` (`HandleDealingDamage_Async`) (medium-high; see the RNG list under the per-turn order).

## Burn class (`+0x9B`, `+0x9C`, `+0x9D`, `+0x9E`)

Burn, poison, bad poison and paralysis share one status byte. Poison and bad poison come from the same family, so they are covered here as well.

### Burn and poison: application

**Burn** (`0218A710`, status 1). Checks, in order: Safeguard (`02180870`); Water Veil (ability `0x42`, `'B'`), which blocks with a message; `MonsterIsType(entity, 2)` (fire type, Red `TYPE_FIRE` = 2), which blocks; terrain: if the tile is water and the dungeon's water type is not lava (`gDungeonWaterType[tileset] != DUNGEON_WATER_TYPE_LAVA`), it blocks (matches Red `move_orb_effects_1.c:362` to `372`); if already burned (`0x9B == 1`), a message. Otherwise `0x9B = 1`, `0x9C = roll(021AA2EC) + 1`, `0x9D = 0`, `0x9E = 0`. Synchronize (ability `0x19`) spreads the burn to the 8 adjacent monsters when it applies. **blue-verified, high** for the checks and writes. Red match by shape only (medium).

**Poison** (`0218A410`, status 2). Immunity `'E'` (ability `0x45`) check, then a check for status 3 that reports "already badly poisoned". Sets `0x9B = 2`. **blue-verified** for the checks (high).

**Bad poison** (`0218A140`, status 3). Held item `0x14` (Pecha Scarf, `ITEM_PECHA_SCARF` = 20) check; Immunity `'E'`; poison type (8) or steel type (`0x11`) is immune. Then `0x9B = 3`, `0x9C = CalculateStatusTurns(target, range, 1) + 1` with the range at `021AA2F4` = (127, 127), `0x9D = 0`, `0x9E = 0`, then `FUN_0213FD10` and `FUN_0217ADBC(target, 8)`. The checks and writes are blue-verified (high). **The turn range {127, 127} is a red-hypothesis match (medium).** Red counterparts: `PoisonedStatusTarget` (`move_orb_effects_1.c:427`) and `BadlyPoisonedStatusTarget` (`:516`). Red's Pecha Scarf check appears in both (lines 443 and 532).

Burn's duration is `0x7F` (`021AA2EC` = (127, 127)), so burn never expires by countdown. Matches Red's `gBurnTurnRange` = {0x7F, 0x7F} (yes, value match).

Callers of `0218A710` (not decompiled): `0215BAF8`, `021603F0`, `0215D2D0`, `0215D0C4`, `0215D040`, `0215EDFC`, `02161A00`, `02145660`, and the per-turn `021760B4`. `021760B4` calls it only under a condition that was not confirmed.

### Burn and poison: per-turn damage

The burn and poison blocks sit in `021861E8`:

| `0x9B` | Condition and action |
|--------|----------------------|
| 1 burn | If `0x9D != 0`, decrement it. If `0x9D == 0`: reset `0x9D` to 20 (`021AA224`), call the wake-check `0218F21C`, then deal 5 damage (`021AA1BC`) through `0217BCA4(…, 1, 0x208)`. |
| 2 poison | Same pattern: reset 10 (`021AA220`), damage 4 (`021AA1CC`) through `0217BCA4(…, 3, …)`. |
| 3 bad poison | Reset 2 (`021AA21C`). Damage from the 30-entry table `021AA54C` at index `min(0x9E, 0x1D)` (all 30 entries equal 6). Then `0x9E` goes up to `0x1D`. |

- The countdown starts at 0 at application, so the first end of turn deals damage at once. Red `dungeon_turn_effects.c:208` to `210` does the same (red-hypothesis). **blue-verified, high.**
- The residual-damage reason tags (`0x208`, and the `…A0` tag for the others) are not mapped to Red's `RESIDUAL_DAMAGE_BURN` and `RESIDUAL_DAMAGE_POISON`.

### Burn and poison: countdown and expiry

- `TickStatusAndHealthRegen` counts down `0x9C` with `0219BF1C`. At 0, with `0x9B != 0`, it calls the burn-class expiry `0218FD80` (step 3). **blue-verified, high.**
- Burn and bad poison (`0x9C == 0x7F`) never expire by countdown. Paralysis (`0x9C` from `gParalysisTurnRange`, red-hypothesis {1, 2}) does.
- `0218FD80` switches on `0x9B`: 1 burn message; 2 or 3 poison message; 4 paralysis message. It clears `0x9B` and refreshes the monster display (`0213FF34`). **blue-verified, high.**

### Burn and poison: regeneration gate

HP regeneration in step 1 runs only when `0x9B` is neither 2 nor 3. Poison and bad poison block regeneration. **blue-verified, high.** The matching Red behaviour (`dungeon_turn_effects.c:396` on) was not re-checked line by line.

### Burn and poison: cures

| Cure | Path | Effect |
|------|------|--------|
| Limber (`0x16`) | `02175850` | Paralysis (4) → `0218FD80`. |
| Water Veil (`0x42`) | `02175850` | Burn (1) → `0218FD80`. |
| Immunity (`0x45`) | `02175850` | Poison (2) and bad poison (3) → `0218FD80`. |
| Cure-all `02190100` | Shed Skin, napping end | Clears all negative classes. |
| `0215C774` | If `0x9B == 4`, ends paralysis | Caller unknown. |
| `0214650C` | If `0x9B == 1`, ends burn; otherwise a message | Caller unknown (probably an item). |
| `021760B4` (per turn) | Ends burn under a belly or tile condition | Condition not confirmed. |

Cures through `02175850` are blue-verified (medium). The three callers of the cures are unknown.

### Paralysis (`0x9B == 4`): partial

- **Write.** `021885A0` writes `0x9B = 4` and the `0x9C` turns. Overlay dispatcher `0215BAF8` calls it when bit 8 of the entity's flags at `+0x144` is set. **blue-verified, medium** (C's finding).
- **Conflict.** One finding reports that no paralysis setter was located, and that Red's setter (`move_orb_effects_1.c:1311`, `gParalysisTurnRange`) has no Blue match yet. Another reports the write at `021885A0`. The write finding is used here; Red's match to `021885A0` is not checked (open).
- Expiry is by countdown and by Limber. The speed effect (`CalcSpeedStage` reads `0x9B == 4`) is outside this page.
- The meaning of bits 1, 2, 4 and 8 in `+0x144` is not verified.

## Frozen and immobile classes (`+0xA0`, `+0xA8`)

### Frozen: application

Frozen (`02189F68`, status 1). Checks Magma Armor (ability `0x41`, `'A'`), and that `0xA0` is not already 1 and not in 3 to 4. Sets `0xA0 = 1`. **blue-verified, medium** (C, F5.6). The Red match (`FrozenStatusTarget`, `move_orb_effects_1.c:598`) was not compared line by line.

### Frozen: timer and per-turn

- The timer is `+0xA8` (Blue), which matches Red's `frozenClassStatus.turns` at struct offset `+0x8` (red-hypothesis for the Red side). **blue-verified** for the offset.
- In the chain (step 4), when `+0xA0 != 0` and `+0xA8` reaches 0, the frozen end `0218FC18` runs. Nothing else in the chain touches frozen except the countdown.
- Red's `damageCountdown` (`+0x9`) is not touched in the chain.

### Frozen: end handler `0218FC18`

**blue-verified routes; names red-hypothesis.** Routes, by `+0xA0` value:

- 1 (frozen): prints the message, clears `+0xA0`, then calls `0213EA14(e, 0x18, 1)` through an indirect call.
- 2 and 5 (shadow hold and ingrain): share one route with no release call.
- 3 and 4 (wrap and wrapped): share one route, which calls the release helper `02189910` with the entity's `+0x90` value.
- 6 (petrified) and 7 (constriction): each has its own message.
- 0 and 8: no-ops.

Red's `EndFrozenClassStatus` (`src/move_orb_effects_5.c`) has the same shape (red-hypothesis, strong): frozen calls `sub_8042A74`; wrap and wrapped call `sub_8076CB4(unk9C)`.

**Release helper `02189910(linkId)`** (blue-verified, medium). It loops over the 20 entity slots at `gDungeon+0x11364`. For each valid entity whose `+0x90` equals `linkId`, it clears `+0xA0` when that is 3 or 4, clears `+0x90`, and refreshes the sprite with `0213FF34`. Red's counterpart `sub_8076CB4` was not read. The Red wrap link offset (`unk9C`) differs from Blue's `+0x90`, and the two were not reconciled.

### Frozen: immobility and wrap readers

**Not verified.** No reader of `+0xA0` for immobility, wrap damage or the petrified AI was found. The x-ref API does not find struct-offset reads, so this needs a read-only search of the code. Red's `dungeon_logic.c` lines 545 to 633 and `dungeon_action_execution.c` lines 117 to 133 are the Red places to compare against.

### Frozen: RNG

None found for frozen in the chain (step 4), by the x-ref walk to depth 2. The release helper was not checked for RNG beyond that.

## Cringe class (`+0xAC`, `+0xAD`)

**Write sites belong to #166.** The confusion and infatuation setters are listed here for the class, but their write sites are the subject of #166, so the names may be merged there.

- **Confusion** (`0218E27C`, value 2). **blue-verified, high.** Red's `ConfuseStatusTarget` (`move_orb_effects_2.c:35`).
- **Infatuation** (`0218A9F8`, value 7). **blue-verified, high.** Red's `InfatuateStatusTarget` (`move_orb_effects_1.c:307`).
- **Immunity and cure.** Own Tempo (ability `0x40`, `'@'`) is checked in `02175850`. It cures when `0xAC` is 2 or 7. The setters' immunity checks were not listed in the findings.
- **Per turn.** In the chain (step 5), the end function `0218FA80` runs when the counter reaches 0. It has seven values, each with a message (1 to 7), then clears `+0xAC`, refreshes the display (`0213FF34`), and calls `CalcSpeedStage`. **blue-verified** for the shape. The mapping (1 cringe, 2 confused, 3 paused, 4 cowering, 5 taunted, 6 encore, 7 infatuated) is red-hypothesis, and high confidence on the mapping.
- **RNG.** None found in the chain for this class (step 5 has no call that consumes it).
- **Not verified.** The confusion behaviour (value 2) and its readers were not checked. The cringe value 1 and values 3 to 6 are only message cases.

## Reflect class (`+0xB1`, `+0xB2`)

- **Gate** `02180870` (Safeguard): returns 1 when `+0xB1 == 2`. See the infliction checks below.
- **Setter** `0218E7D0` writes 12 (`STATUS_CONVERSION2` in Red), called from `0215E440`. **red-hypothesis.**
- **End switch** `0218F814`: cases 0 to 14, each printing a message, then clears `+0xB1` and refreshes the display. Case 4 shares a block with case 10. The fourteen non-zero values match Red's fourteen reflect-class values. **blue-verified** for the shape. **red-hypothesis** for the mapping. Medium.
- **Mist** (reflect value 14, red-hypothesis) has no check in the Safeguard gate, and its check was not found in Blue. Open.
- **RNG** none in the chain for this class (step 6).

## Infliction checks

| Check | Address | Applies to | Tag |
|-------|---------|------------|-----|
| Safeguard gate (`SafeguardIsActive`, Red `src/dungeon_logic.c:1204`) | `02180870` | Every setter above (29 callers). Returns 1 when `+0xB1 == 2`. With the third argument set, shows the Safeguard message. Does not check Mist. | blue-verified, high |
| Sleep immunity (`CannotSleep`) | `0218B140` | Sleep, nightmare, yawning, napping | blue-verified, high |
| Water Veil | in `0218A710` | Burn | blue-verified (check) |
| Immunity `'E'` and Pecha Scarf | in `0218A410`, `0218A140` | Poison, bad poison | blue-verified (check) |
| Magma Armor `'A'` | in `02189F68` | Frozen | blue-verified, medium |
| Own Tempo `'@'` | in `02175850` (cure) | Cringe class cure | blue-verified, medium |

Paralysis: no immunity check was listed in the findings for `021885A0`. Cringe setters: none listed.

## Pool values (overlay 1)

Each value was read from the overlay 1 file at `addr − 0x02138860` and compared with Red's `dungeon_config.c`.

| Blue address | Used by | Value read | Red symbol | Match |
|--------------|---------|------------|------------|-------|
| `021AA324` | sleep roll | (3, 7) | `gSleepTurnRange` {3, 7} | yes |
| `021AA328` | nightmare roll | (4, 8) | `gNightmareTurnRange` {4, 8} | yes |
| `021AA2BC` | yawning roll | (2, 2) | `gYawnTurnRange` {2, 2} | yes |
| `021AA344` | napping roll | (2, 4) | `gNappingTurnRange` {2, 4} | yes |
| `021AA2C0` | yawn-end roll | (4, 8) | — | source unconfirmed |
| `021AA2EC` | burn roll | (127, 127) | `gBurnTurnRange` {0x7F, 0x7F} | yes |
| `021AA2F4` | bad-poison roll | (127, 127) | — | red-hypothesis match |
| `021AA11C` | nightmare damage | 8 | `gNightmareDmgValue` 8 | yes |
| `021AA1D0` | napping heal | 999 | `gNappingHpHealValue` 999 | yes |
| `021AA224` | burn countdown reset | 20 | `gBurnDmgCountdown` 20 | yes |
| `021AA1BC` | burn damage | 5 | `gBurnDmgValue` 5 | yes |
| `021AA220` | poison countdown reset | 10 | `gPoisonDmgCountdown` 10 | yes |
| `021AA1CC` | poison damage | 4 | `gPoisonDmgValue` 4 | yes |
| `021AA21C` | bad-poison countdown reset | 2 | `gBadPoisonDmgCountdown` 2 | yes |
| `021AA54C` | bad-poison damage table (30 entries) | 6 in every entry read | `gBadPoisonDmgValuesByTurn` | yes |
| `021AA090` | Shed Skin chance (per 100) | 50 | `gShedSkinActivateChance` 50 | yes |

Red's `gParalysisTurnRange` ({1, 2}) and `gSleeplessTurnRange` ({10, 12}) have no located overlay address. The table comes from one finding (overlay bytes) and another (`021AA2F4`, `021AA2C0`, `021AA11C`, `021AA1D0`). Message-table pointers `021B0A58`, `021B0A80`, `021B0C4C` and `021B0C38` point to language-indexed tables. Their strings were not read.

## Open questions

- **Paralysis setter.** One finding reports none; another reports the write at `021885A0`. Confirm which function writes `0x9B = 4` and match it to Red's setter.
- **Yawning write.** One finding tags `0218ADA8` as writing `0x99 = 4` (blue-verified). Another lists yawning value 4 only as a red-hypothesis wake path. Confirm the write.
- **Sleepless (2).** No writer found. No caller of `0218B140` or `0218AEF0` writes 2. Red's `SleeplessStatusTarget` uses {10, 12}. Low confidence.
- **Yawn-end range.** `0218FE64` case 4 uses `021AA2C0` (4, 8). Its source is unconfirmed. Check Red's `sub_804151C` path.
- **Yawning per-turn function.** `0213F6B0` is not identified.
- **Wake on hit.** `0217D5FC` and `0218F21C` disagree for nightmare and napping (P1.6). A trace at 127 turns is needed.
- **Countdown helper.** One finding says `0x7F` only; another says `0` and `0x7F`. Read `0219BF1C` again.
- **Ability and item IDs.** `FUN_021818D0`, `FUN_021805B0` (IQ and flag checks `0x11`, `0x12`, `0x14`), `FUN_02181904` (negative-status check): not named. Held item `0x14`, `0x15` and the IQ bit are matched by value only.
- **Calls with unknown arguments.** `0218DF38` (heal) argument order, `0217BCA4` (`…, 1, 0x208`) argument list, and `02190100` behaviour (inferred from its body).
- **Burn-end and burn-apply conditions** in `021760B4`: `0x2A & 3 == 2` and the dungeon flag `0x34AE`, unnamed. Callers of `0215C774` and `0214650C` are unknown.
- **Residual-damage reason tags** (`0x208` and the `…A0` tag) are not mapped to Red's enum.
- **Message strings** are not read. The message-table pointers are listed above.
- **`0x7F` sentinel.** Read as "never counts down" in every class. It is not named.
- **`+0x144` bits** (dispatcher `0215BAF8`). Their meaning is unknown and needs an owner.
- **Mist** (reflect value 14): its check was not found in Blue.
- **Immobility, wrap damage and petrified AI** readers of `+0xA0` were not found (P3d).
- **Bide, sure shot and the remaining classes.** Bide (`+0xAE`), sure shot end, long toss, muzzled, transform, perish song, invisible, blinker, curse, leech seed, snatch and decoy were not investigated in detail. Red write sites are in `reference/pmd-red/src/move_orb_effects_*.c`. Their Blue counterparts are not located. Split into two or three PRs by class.
- **Class identity** of `+0xC7` (transform or invisible; case 2 changes the sprite to the species sprite) and `+0xC9` (blinker, three values).
- **Regeneration gate.** Whether `+0x6 == 0` is Red's `!isNotTeamMember` (likely, not confirmed). Ability and item checks in regen (held item `'\n'`, `AbilityIsActive(3)`, weather `4` through `02195C48`) are not mapped to Red's constants.
- **Red line-by-line comparison** of the burn and frozen setters has not been done.
- **Identity of** `0213EA14` (indirect call with `(e, 0x18, 1)`) and `0213FF34` (sprite or flag refresh, not read).
- **Global RNG.** `0217AF58` (`SetMonsterAnim`) consumes one global `Rand32Bit` step on every call, and has 13 call sites in overlay 1. The curse decoy end and the `+0xC7` case 2 use it through that path. This matters for the golden-trace hashing in #49 (see [`rng.md`](rng.md)).
- **Runtime check.** No py-desmume trace was run. Every per-turn claim, the wake-rule mismatch and the counter semantics need one before they are blue-verified at runtime.
- **Wrap link offset.** Blue `+0x90` vs Red `unk9C`, not reconciled.
- **Confusion behaviour** (value 2) and its readers, not checked (P3c).
- **Cringe values 1 and 3 to 6** are message cases only (F5.7).
- **Move-effect dispatch table** behind `02156824` was not traced (P1.1).
- **Pool words** `021AA3EC` (`0x80`) and `021AA3F0` (`0x100`) are not mapped (F5.10).
- **Overlay addresses** of `gParalysisTurnRange` and `gSleeplessTurnRange` are not located.

## Links

- Symbols: [symbols.md](../symbols.md)
- Turn loop: [turns.md](turns.md)
- Dungeon struct and entity fields: [structs.md](structs.md)
- RNG: [rng.md](rng.md)
- Verification: [verification.md](../verification.md)
