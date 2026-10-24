# Status effects

How each status is applied, how long it lasts, what it does each turn, how it ends and is cured, and which RNG calls it makes. Issue: [#51](https://github.com/TorratDev/rust-mm/issues/51), part of Epic #5. Names, addresses and tags are in [`symbols.md`](../symbols.md). The turn loop that calls these checks is in [`turns.md`](turns.md).

**Evidence.** Most rows come from Ghidra decompile and cross-reference queries on the retail ARM9 and overlay 1, from the overlay 1 bytes (file offset = RAM address − `0x02138860`), and from comparison with the pinned `pmd-red` commit `013475aa04f5be3191e5527c186d9bfceae7cae0`. The findings from #189, #190 and #191 are at the end of this page. Their runtime checks used py-desmume on the supported dump (SHA-1 `6f01887356268de39085df01490347bbcc3aa330`); each is marked runtime. Read every "partial", "not verified" and "open" note as a limit on what this page shows.

Tags: **blue-verified** means the role was read in Blue's code. **red-hypothesis** means the name or mapping comes from `pmd-red` and Blue has not matched it yet. **tentative** means our own name, with the role only partly known. Confidence is high, medium or low.

## Status classes

Each class has a status byte and a turns or counter byte. The offsets are in [`structs.md`](structs.md#entity-info). Blue value to Red enum is partial: only the values with a Blue writer or a Blue message case are listed as blue-verified.

| Blue field | Class | Value → Red name | Tag |
|------------|-------|------------------|-----|
| `+0x99` status, `+0x9A` turns | sleep | 1 `STATUS_SLEEP`; 3 `STATUS_NIGHTMARE`; 5 `STATUS_NAPPING`; 4 `STATUS_YAWNING` | 1, 3, 5: blue-verified writes. 4: blue-verified write in `0218ADA8` (medium, see open items), red-hypothesis as a wake path in another finding's catalogue |
| `+0x99` | sleep | 2 `STATUS_SLEEPLESS` | blue-verified writer `0218AC74` (#190 P4); name red-hypothesis |
| `+0x9B` status, `+0x9C` turns | burn | 1 `STATUS_BURN`; 2 `STATUS_POISONED`; 3 `STATUS_BADLY_POISONED`; 4 `STATUS_PARALYSIS` | blue-verified writes for 1 to 4 (4: partial, see below) |
| `+0x9D` | burn | burn damage countdown | blue-verified (read and reset per turn) |
| `+0x9E` | burn | bad-poison damage index | blue-verified (incremented per turn) |
| `+0xA0` status, `+0xA8` turns | frozen | 1 `STATUS_FROZEN`; 3 `STATUS_WRAP`; 4 `STATUS_WRAPPED`; 2 shadow hold, 5 ingrain, 6 petrified, 7 constriction | 1: blue-verified write and message. 2 to 7: message routes blue-verified; names red-hypothesis |
| `+0xAC` status, `+0xAD` counter | cringe | 2 `STATUS_CONFUSED`; 7 `STATUS_INFATUATED`; 1 cringe, 3 paused, 4 cowering, 5 taunted, 6 encore | 2 and 7: blue-verified writes. Others: red-hypothesis message cases. The `+0xAC` offset is inferred from the Own Tempo cure, not from a documented field |
| `+0xAE` | bide | 1 `STATUS_BIDE`; 0xC enraged; flags 7 flying, 8 bouncing, 9 diving, 10 digging in `+0xDC` | blue-verified writes (setter `0218E428`); names red-hypothesis. See [#189 P1](#bide-and-sure-shot-189-p1) |
| `+0xB1` status, `+0xB2` counter | reflect | 2 `STATUS_SAFEGUARD` (gate); 3 `STATUS_LIGHT_SCREEN` (reader); 11, 12, 13 (writers); 14 `STATUS_MIST` (reader); 1 and 4 to 10 writers not found | blue-verified for the listed writes and readers; names red-hypothesis. See [#189 P4](#reflect-class-189-p4) |
| `+0xB3` status, `+0xB6` counter | curse | 1 cursed, 2 decoy, 3 snatch (Red's curse class) | red-hypothesis (strong) |
| `+0xB8` status, `+0xC1` counter | leech seed | — | red-hypothesis (class guessed from the offset) |
| `+0xC4` status, `+0xC5` counter | sure shot | 1 `SURE_SHOT`; 2 `WHIFFER`; 3 `SET_DAMAGE`; 4 `FOCUS_ENERGY` | blue-verified writes; names red-hypothesis. End routine read. See [#189 P1](#bide-and-sure-shot-189-p1) |
| `+0xC7` status, `+0xC8` counter | transform, invisible, mobile | 1 `INVISIBLE`; 2 `TRANSFORMED`; 3 `MOBILE` | blue-verified writes; names red-hypothesis. See [#189 P2](#muzzled-transform-invisible-mobile-blinker-189-p2) |
| `+0xC9` status, `+0xCA` counter | blinker | 1 `BLINKER`; 2 `CROSS_EYED`; 3 `EYEDROPS` | blue-verified writes (#189 P2) |
| `+0xCB` status, `+0xCC` counter | muzzled | 1 muzzled | blue-verified write `0218C1AC` (#189 P2) |
| `+0xD7` | terrified | flag and timer share the byte | blue-verified read in the chain |
| `+0xE0`, `+0xE4` to `+0xE8`, `+0xE9` to `+0xED` | speed stage and counters | — | blue-verified (see [`structs.md`](structs.md#entity-info)) |

## Per-turn order

`TickStatusAndHealthRegen` (`ov_0001::02185C70`) runs the countdowns and the status chain. The chain is in the table below. Each step re-checks that the entity is valid and that the floor is not over before it runs. The order was read from the decompile and matches Red's `TickStatusAndHealthRegen` step for step (red-hypothesis for the names, blue-verified for the order).

**Call sites (four).** The function is called from `02140BA0` (in `sub_8044574_Async`), `021409CC` (in `sub_8044820_Async`), `02140F40` (in `FUN_02140ED0`) and `021410A4` (in `RunLeaderTurn_Async`). **Runtime (#191 P1): the countdown bytes decremented on each call returning to `0x02140BA4` (the call from `02140BA0`) while the counter was above 0, and on no call returning to `0x021409D0` or `0x021410A8`. Calls returning to `0x02140BA4` came every 4 or 5 ticks, not every fifth call. The call from `02140F40` is not in that trace.** So the counters do not tick once per call from each site. Not verified: whether the `0x02140BA4`-only decrement is a gate in `TickStatus` or a property of the call site (need a read of `02185C70` around its countdown calls). Position in the turn matters for #49, see [`turns.md`](turns.md).

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

- **Countdown helper** `0219BF1C` (`DecrementStatusTurns` in [`symbols.md`](../symbols.md)) leaves a byte alone if it is `0x7F` or 0, and decrements a nonzero value otherwise; the class end runs when it reaches 0. This is static (#190 P4) and matches the runtime sentinel (#191 P1). Red's `sub_80838EC` is the same routine (red-hypothesis).
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

- The countdown starts at 0 at application, so the first end of turn deals damage at once. Red `dungeon_turn_effects.c:208` to `210` does the same (red-hypothesis). **blue-verified, high.** The first reset at 0 is not seen in the poison runtime trace (#191 P1).
- The residual-damage reason tags are mapped in [#191 P3](#residual-damage-tags-191-p3): burn `0x208`, poison `0x20A`, curse `0x20C`.

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

- **Write.** `021885A0` writes `0x9B = 4` and the `0x9C` turns. Overlay dispatcher `0215BAF8` calls it when bit 8 of the entity's flags at `+0x144` is set. **blue-verified, medium**.
- **Conflict resolved.** `021885A0` is the setter (#190 P4). It reads the range `0x021AA304` = (1, 2) at runtime. Red's setter (`move_orb_effects_1.c:1311`, `gParalysisTurnRange`) is not matched to `021885A0` (open).
- Expiry is by countdown and by Limber. The speed effect (`CalcSpeedStage` reads `0x9B == 4`) is outside this page.
- The bit meanings of `+0x144` are in [#190 P4](#paralysis-sleepless-the-0x7f-sentinel-and-0x144-190-p4). Their writers are not located.

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

The immobility, movement, faint, damage and petrified readers of `+0xA0` are in [#190 P1](#frozen-and-immobility-readers-190-p1). Wrap damage was not found.

### Frozen: RNG

None found for frozen in the chain (step 4), by the x-ref walk to depth 2. The release helper was not checked for RNG beyond that.

## Cringe class (`+0xAC`, `+0xAD`)

**Write sites belong to #166.** The confusion and infatuation setters are listed here for the class, but their write sites are the subject of #166, so the names may be merged there.

- **Confusion** (`0218E27C`, value 2). **blue-verified, high.** Red's `ConfuseStatusTarget` (`move_orb_effects_2.c:35`).
- **Infatuation** (`0218A9F8`, value 7). **blue-verified, high.** Red's `InfatuateStatusTarget` (`move_orb_effects_1.c:307`).
- **Immunity and cure.** Own Tempo (ability `0x40`, `'@'`) is checked in `02175850`. It cures when `0xAC` is 2 or 7. The setters' immunity checks were not listed in the findings.
- **Per turn.** In the chain (step 5), the end function `0218FA80` runs when the counter reaches 0. It has seven values, each with a message (1 to 7), then clears `+0xAC`, refreshes the display (`0213FF34`), and calls `CalcSpeedStage`. **blue-verified** for the shape. The mapping (1 cringe, 2 confused, 3 paused, 4 cowering, 5 taunted, 6 encore, 7 infatuated) is red-hypothesis, and high confidence on the mapping.
- **RNG.** None found in the chain for this class (step 5 has no call that consumes it).
- **Confusion** (value 2): setter, turn range and readers are in [#190 P3](#confusion-and-cringe-190-p3). Runtime behaviour was not observed. The cringe values 1 and 3 to 6 are only message cases, and their writers are not confirmed.

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
| `021AA304` | paralysis roll | (1, 2) | `gParalysisTurnRange` {1, 2} | runtime value; Red value matches (red-hypothesis) |
| `021AA32C` | sleepless roll | (10, 12) | `gSleeplessTurnRange` {10, 12} | runtime value; read by `0218AC74` |
| `021AA300` | confusion roll | (6, 12) | — | runtime value |
| `021AA31C` | frozen roll | (3, 5) | `gFreezeTurnRange` {3, 5} | runtime value; match |
| `021AA2F0` | poison roll | (127, 127) | — | range read at runtime; not sampled |
| `021AA34C` | bide roll | (3, 5) | `gBideTurnRange` {3, 5} | match (#189 P1) |
| `021AA350` | enraged roll | (4, 10) | `gEnragedTurnRange` {4, 10} | match (#189 P1) |
| `021AA2B0` | muzzled roll | (10, 10) | `gMuzzledTurnRange` {10, 10} | match (#189 P2) |
| `021AA294` | transform roll | (127, 127) | `gTransformedTurnRange` {0x7F, 0x7F} | match (#189 P2) |
| `021AA394` | invisible roll | (10, 20) | `gInvisibleTurnRange` {10, 20} | match (#189 P2) |
| `021AA29C` | mobile roll | (127, 127) | `gMobileTurnRange` {0x7F, 0x7F} | match (#189 P2) |
| `021AA3E4`, `021AA3E8`, `021AA3EC`, `021AA3F0` | HP-tier damage modifiers | 0x19, 0x33, 0x80, 0x100 | — | runtime values (#191 P2) |

Red's `gParalysisTurnRange` ({1, 2}) and `gSleeplessTurnRange` ({10, 12}) are at `021AA304` and `021AA32C` (#190 P4, runtime). The rest of the table comes from the overlay bytes and the runtime reads in the findings (`021AA2F4`, `021AA2C0`, `021AA11C`, `021AA1D0`). Message-table pointers `021B0A58`, `021B0A80`, `021B0C4C` and `021B0C38` point to language-indexed tables. Their strings were not read.

## Findings from #189 to #191

Static findings from Ghidra copies A, B and C, with runtime checks where marked. Each finding keeps its investigator's tags and not-verified items. Blue addresses are overlay 1 unless stated.

### Bide and sure shot (#189 P1)

**Status: partial.**

- **Bide setter** `FUN_ov_0001__0218E428(user, target, status, moveptr, message)` (blue-verified; Red counterpart `SetChargeStatusTarget`, `move_orb_actions_1.c:654`):
  - returns if the target is invalid;
  - if `+0xAE == status` and `status == 0xC` (enraged), shows a message and returns;
  - writes `+0xAE = status`;
  - finds the move slot `i` where `info+0xF4+6*i == moveptr` and writes `+0xB0 = i`;
  - sets `+0x11C = 1`;
  - sets `+0xDC = 1` for statuses 7 and 8, and `+0xDC = 2` for statuses 9 and 10 (Red: FLYING 7, BOUNCING 8, DIVING 9, DIGGING 10; value mapping red-hypothesis, medium-high);
  - for `status == 1` (bide): `+0xAF = roll(0x021AA34C) + 1`, `+0x94 = 0`;
  - for `status == 0xC` (enraged): `+0xAF = roll(0x021AA350) + 1`;
  - calls `FUN_0213FDE0` and the message, then `FUN_0213FF34` (display refresh).
- **Roll pools** (overlay 1 bytes, blue-verified read): `0x021AA34C` = (3, 5), matching Red's `gBideTurnRange` {3, 5}; `0x021AA350` = (4, 10), matching Red's `gEnragedTurnRange` {4, 10}.
- The roll is `FUN_0219CF28` (`CalculateStatusTurns` in Red). Its argument `0` and the adjustment were not compared to Red's `CalculateStatusTurns` (medium).
- **Bide per turn.** `FUN_021861E8` (`DoEndOfTurnEffects_Async`) reads `+0xAE == 1` and `+0xAE == 0xC`. The countdown and the release (Red `dungeon_turn_effects.c:361-395`: bide release fires `TryUseChosenMove` with `MOVE_BIDE_2`; enraged end logs a message) were **not decompiled** in this pass. Not verified.
- **Bide end** `FUN_ov_0001__0218E3E4`. Clears `+0xAE`, `+0x11C` and `+0xDC`, unless `+0xAE` is 1 or 0xC, so bide and enraged keep their state. Red counterpart: `entityInfo->bideClassStatus.status = 0` at `dungeon_turn_effects.c:364` and `:385`. The caller of `FUN_0218E3E4` was not identified. blue-verified for the function body, red-hypothesis for the role.
- **Sure shot setters.** All write `+0xC4` with a constant (Red `SureShotStatusTarget`, `WhifferStatusTarget`, `FixedDamageStatusTarget`, `FocusEnergyStatusTarget`, `move_orb_effects_2.c:357-440`). blue-verified for the writes.
  - `FUN_0218D84C` writes `+0xC4 = 1` (sure shot) and `+0xC5 = param + 1`. If already 1, it shows the message only. Matches Red's guard.
  - `FUN_0218D778` writes `+0xC4 = 2` (whiffer) and `+0xC5 = param + 1`. Matches Red.
  - `FUN_0218D6B4` writes `+0xC4 = 3` (set damage) and `+0xC5 = roll + 1`. Red `gSetDamageTurnRange` {2, 4}. The roll pool was not read (medium).
  - `FUN_0218D5F0` writes `+0xC4 = 4` (focus energy) and `+0xC5 = roll + 1`. Red `gFocusEnergyTurnRange` {2, 4}. The roll pool was not read (medium).
- **Sure shot duration.** Red `gSureShotTurnRange` {10, 12} is passed by the caller (`move_orb_actions_1.c:977`). The Blue caller was **not located**.
- **Sure shot countdown and end.** Step 9 of the chain: if `+0xC4 != 0`, `0219BF1C` runs on `+0xC5`, and at 0 the end `FUN_ov_0001__0218F510` runs (red-hypothesis: `sub_80838EC` then `SendMoveEndMessage`, `dungeon_turn_effects.c:504-507`; order blue-verified from status.md). The end switches on `+0xC4` values 1 to 4 (case 5 is empty), then clears `+0xC4` and refreshes the display (`FUN_0213FF34`). It clears only `+0xC4`, not `+0xC5`, as Red's turns stay. blue-verified.
- **Readers.** `CalcDamage` reads `+0xC4 == 4` (`ov_0001::0217BD94`), matching Red's `FOCUS_ENERGY` branch at `dungeon_damage.c:1320` (medium).

**Not verified**

- The bide per-turn countdown and release in `FUN_021861E8`.
- The caller of `FUN_0218E3E4`.
- The caller of the sure shot setters and the sure shot duration.
- The sure shot roll for `FUN_0218D6B4` and `FUN_0218D5F0`.

### Muzzled, transform, invisible, mobile, blinker (#189 P2)

**Status: partial.** Muzzled, transform and blinker are blue-verified. Invisible and mobile were found by the decimal-offset scan.

- **Muzzled** (`+0xCB` flag, `+0xCC` turns). Setter `FUN_ov_0001__0218C1AC(user, target)`: Safeguard gate `FUN_02180870(user, target, 1)` (returns when set, and shows the Safeguard message); if `+0xCB == 1`, message only; else `+0xCB = 1` and `+0xCC = roll(0x021AA2B0) + 1`. Pool `0x021AA2B0` = (10, 10), matching Red's `gMuzzledTurnRange`. End `FUN_ov_0001__0218F27C` (chain step 12): if `+0xCB == 1`, message; then clears `+0xCB`. Readers of `+0xCB`: `FUN_0215B384`, `FUN_021457B0`, `FUN_021816EC`, `FUN_0218410C`, `FUN_021842D8`, `FUN_02181904`, not analysed (likely the move-block checks).
- **Transform** (`+0xC7` status, `+0xC8` turns). Setter `FUN_ov_0001__0218BFA8(user, target)`, status 2:
  - if already 2, message;
  - picks a species at random: up to 20 tries, `DungeonRandInt(cap)` each, using `FUN_02175BA0`, `FUN_02178C04` and the species list from `FUN_02138A5C`. This is a use of the dungeon RNG;
  - writes the species to `info+2` and `info+4`;
  - writes `+0xC7 = 2` and `+0xC8 = roll(0x021AA294) + 1`;
  - calls `FUN_0217AC10`, `FUN_0213FB88`, `FUN_0217AF58` (sprite change), and a message.
  - Pool `0x021AA294` = (127, 127), matching Red's `gTransformedTurnRange` {0x7F, 0x7F}. A turns value of 0x7F never counts down.
  - End `FUN_ov_0001__0218F3F8` (chain step 10). Case 2 reverts the species (`FUN_02176AFC`, `FUN_02178C04`, `FUN_0217AD64`, `FUN_0217AF58`) and shows a message. Case 1 shows a message and calls `FUN_0219635C(1)`. Case 3 shows a message. It clears `+0xC7`.
- **Invisible** (value 1). Setter `FUN_ov_0001__0218CE68` writes `+0xC7 = 1` and `+0xC8 = roll(0x021AA394) + 1`. Pool `0x021AA394` = (10, 20), matching Red's `gInvisibleTurnRange` {10, 20}. Its end is case 1 of `FUN_0218F3F8`.
- **Mobile** (value 3). Setter `FUN_ov_0001__0218BECC` writes `+0xC7 = 3` and `+0xC8 = roll(0x021AA29C) + 1`. Pool `0x021AA29C` = (127, 127), matching Red's `gMobileTurnRange` {0x7F, 0x7F}.
- Both the invisible and mobile setters first check for the transform (value 2) and call `FUN_0218F3F8` to end it (blue-verified, from the decompile).
- **Blinker** (`+0xC9` status, `+0xCA` turns). Setters write the value, then `+0xCA = roll + 1`, with a message: `FUN_ov_0001__0218BBB4` writes 1 (blinker), `FUN_ov_0001__0218BAC8` writes 2 (cross-eyed), `FUN_ov_0001__0218B9F4` writes 3 (eyedrops). These match Red's `BlinkerClassStatus` values (medium on the roll, which was not read). End `FUN_ov_0001__0218F304` (chain step 11): a message per value 1 to 3, then clears `+0xC9`. If the target is the team leader (`+7 != 0`), it also runs `DungeonRunFrameActions(0x31)`, `FUN_02148B7C` and `FUN_0213C930` (vision refresh, medium).
- **Long toss** (Red `LongTossClassStatus`, values 1 LONG_TOSS, 2 PIERCE). Red setter `LongTossStatusTarget` (`move_orb_effects_2.c:1218`). The Blue field and setter were **not found**. **[NOT VERIFIED: no writer found in overlay 1 at the offsets tried; Blue offset unknown]**
- **Perish song.** Red uses `perishSongTurns` (an `EntityInfo` field, not a class), read in `dungeon_logic.c:489` and `dungeon_misc.c:969`. The Blue field and writes were **not found**. **[NOT VERIFIED: no writer found in overlay 1 at the offsets tried; Blue offset unknown]**

**Not verified**

- Long toss and perish song: no writer at the offsets tried (see above).
- The readers of `+0xCB` are not analysed.
- The blinker rolls were not read (medium).

### Class byte catalogue (#189 P3)

Blue value to Red enum. The Blue writes are blue-verified unless marked.

| Blue field | Class | Value → Red name | Status |
|------------|-------|------------------|--------|
| `+0x99` | sleep | (from status.md) | existing |
| `+0x9B` | burn | (from status.md) | existing |
| `+0xA0` | frozen | (from status.md) | existing |
| `+0xAC` | cringe | (from status.md) | existing |
| `+0xAE` | bide | 1 BIDE; 0xC ENRAGED (writes); 7 FLYING, 8 BOUNCING, 9 DIVING, 10 DIGGING (`+0xDC` flags, red-hypothesis for names) | blue-verified writes; the others go through the same setter |
| `+0xB1` | reflect | 2 SAFEGUARD (gate at `02180870`); 3 LIGHT_SCREEN (reader `CalcDamage`); 11 (setter `FUN_0218E8D0`); 12 CONVERSION2 (setter `FUN_0218E7D0`); 13 VITAL_THROW (setter `FUN_0218E70C`); 14 MIST (reader `FUN_02180798`); 1 REFLECT, 4 to 10 (see #189 P4) | blue-verified for the writes listed; readers red-hypothesis |
| `+0xB3` | curse | 2 DECOY (setter `FUN_0218D384`) | from status.md |
| `+0xC4` | sure shot | 1 SURE_SHOT (`FUN_0218D84C`), 2 WHIFFER (`FUN_0218D778`), 3 SET_DAMAGE (`FUN_0218D6B4`), 4 FOCUS_ENERGY (`FUN_0218D5F0`) | blue-verified writes |
| `+0xC7` | invisible / transform / mobile | 2 TRANSFORMED (`FUN_0218BFA8`); 1 INVISIBLE (`FUN_0218CE68`); 3 MOBILE (`FUN_0218BECC`) | blue-verified writes; roll pools match Red |
| `+0xC9` | blinker | 1 BLINKER, 2 CROSS_EYED, 3 EYEDROPS (`FUN_0218BBB4`, `FUN_0218BAC8`, `FUN_0218B9F4`) | blue-verified writes |
| `+0xCB` | muzzled | 1 muzzled (`FUN_0218C1AC`) | blue-verified |
| Long toss | — | 1 LONG_TOSS, 2 PIERCE | **not found** |
| Perish song | — | not a class in Red | **not found** |

Red values with no Blue writer found: reflect 1 and 4 to 10 (see the next section).

**Status: partial.** Not verified: long toss and perish song (no writer at the offsets tried; Blue offset unknown). A second pass after a decimal-form scan is needed for the full catalogue.

### Reflect class (#189 P4)

**Status: partial.**

- **End switch** `FUN_ov_0001__0218F814` (blue-verified shape): switch on `+0xB1`, cases 0 to 14, each with a message. Case 4 jumps to the message of case 10 (`LAB_ov_0001__0218f8f4`). Case 15 (`0xF`) has none. Then `+0xB1 = 0` and `FUN_0213FF34`. Red `EndReflectClassStatus` shows messages per status; Red's reflect values (`status.h:51-64`) are REFLECT 1, SAFEGUARD 2, LIGHT_SCREEN 3, COUNTER 4, MAGIC_COAT 5, WISH 6, PROTECT 7, MIRROR_COAT 8, ENDURING 9, MINI_COUNTER 10, MIRROR_MOVE 11, CONVERSION2 12, VITAL_THROW 13, MIST 14 (red-hypothesis).

| Value | Red name | Blue setter | Red turn range | Status |
|-------|----------|-------------|----------------|--------|
| 1 | REFLECT | not found | `gReflectTurnRange` {10, 12} | not found |
| 2 | SAFEGUARD | gate `FUN_02180870` (reader); setter not found | `gSafeguardTurnRange` {15, 20} | setter not found |
| 3 | LIGHT_SCREEN | not found; reader in `CalcDamage`: `+0xB1 == 3` with `splitIndex == 1` (special-type halving, matches Red `dungeon_damage.c:1302`) | `gLightScreenTurnRange` {10, 12} | setter not found |
| 4 to 10 | COUNTER … MINI_COUNTER | not found | — | not found |
| 11 | MIRROR_MOVE | `FUN_0218E8D0` | — | setter body not read |
| 12 | CONVERSION2 | `FUN_0218E7D0` (writes 12, roll for `+0xB2`) | — | blue-verified write |
| 13 | VITAL_THROW | `FUN_0218E70C` (writes 13, roll for `+0xB2`) | — | blue-verified write |
| 14 | MIST | not found; reader `FUN_02180798` (`== 0xE`) | `gMistTurnRange` {10, 14} | setter not found |

- The Safeguard gate `FUN_02180870` reads `+0xB1 == 2` (blue-verified). It does not check Mist, as stated above.
- Reader `FUN_02180798` reads `+0xB1 == 0xE` (mist). Its meaning was not read.

**Not verified**

- Writers for reflect 1, 2, 3, 4 to 10 and 14. The decimal scan (`+ 177` for `+0xB1`) found no writes. **[NOT VERIFIED: no writer at a constant offset in overlay 1; a write through a computed offset or from another overlay is not ruled out]**

### Frozen and immobility readers (#190 P1)

Tag: **blue-verified** for the readers and their values (static, decompile and disassembly). The petrified AI and immobility mapping to Red's `CannotAttack`, `sub_8070BC0` and the `ACTION_WALK` switch is red-hypothesis, from pret/pmd-red `src/dungeon_logic.c` and `src/dungeon_action_execution.c` (read-only). Confidence: high for the list of readers and values; medium for the names; wrap damage not found.

`+0xA0` is the frozen-class status byte (`+0xA8` is its turn counter). Values: 1 frozen, 2 shadow hold, 3 wrap, 4 wrapped, 5 ingrain, 6 petrified, 7 constriction.

- **Immobility, monster turn gate.** `ov_0001::0217FD78` and `ov_0001::02180048` (two routines with the same switch on `+0xA0`): 1, 3 or 4 print a message and return 1 (the monster does not act); 6 returns 1 silently; 2 and 5 fall through to move selection.
- **Movement.** `ov_0001::02177700` (move an entity) and `ov_0001::02179874` (warp): 3 or 4 release the wrap link (`02189910`); 2, 5 or 7 end the class (`0218FC18`), in `02177700` only.
- **Faint.** `ov_0001::021778F4` (`HandleFaint_Async`): 3 or 4 releases the link.
- **Damage handler.** `ov_0001::0217D5FC`: at `+0xA0 == 1` (frozen) it prints a message, sets `param_3+0xF = 1` and returns with no damage. It also releases the link on 3 or 4.
- **Petrified (6), AI and turn readers.** `ov_0001::021473A8`: 6 returns 0. `ov_0001::0217B120` (action chooser): 6, paralysis (`+0x9B == 4`) and shadow hold (2) add `DAT_0217B960 & 2` to the animation step (line 242); movement and animation are skipped for 1 and 6 (line 105). `ov_0001::02180994`, `02181530`, `021816EC`, `02191394`, `0218F21C` each test for 6 in a condition. `ov_0001::0218FC18` (frozen end): switch 0 to 8.
- **Tick.** `ov_0001::02185C70` (`TickStatusAndHealthRegen`) counts `+0xA8` and calls `0218FC18` at 0. It does **not** deal damage for wrap or petrified.
- **Wrap damage: not found.** The link `+0x90` has readers in `0x0217D5FC`, `0x02177740`, `0x021779C4`, `0x02179918`, `0x02189950`, `0x021897C0`, `0x02189C08`, `0x02189D08`, `0x02189E60`, `0x0218A0BC`, `0x0218FD14` and `0x0219B118` (inside `0x0219ADFC`). None is a per-turn wrap damage routine on inspection. `0x0219ADFC` is the only one not read; it is the next lead.

**Not verified**

- The callers of `0x0217FD78` and `0x02180048`. Which one runs for which monster (player, AI) is unknown.
- Wrap damage: no per-turn damage routine found. `0x0219ADFC` is not read.
- No runtime check of any reader.

### Wrap link (#190 P2)

**Verdict: the role is verified; the offset against Red is not reconciled.**

- **Writer.** `ov_0001::02189994` sets `+0xA0 = 3` on one entity and `+0xA0 = 4` on the other (decompile lines 50 and 54). It writes the same 32-bit link into `+0x90` of both (lines 59 and 60). The value is `gDungeon+0x3354`. The stores are `0x02189A54` and `0x02189A64`.
- **Release.** `ov_0001::02189910(linkId)` loops over the 20 entity slots. For each entity with `+0x90 == linkId`, it clears `+0xA0` (when it is 3 or 4) and clears `+0x90`. The store is at `0x02189970`.
- **Red's equivalent.** `EntityInfo.unk9C` (`u32` at `0x9C`, `include/structs/dungeon_entity.h`). Written in `src/move_orb_effects_1.c:754-755` for both entities from `gDungeon->unk37F4`. Released by `sub_8076CB4(unk9C)` (`move_orb_effects_1.c:632`, `655`, `685`, `714`, `817`, and `move_orb_effects_5.c:245`). Cleared at `:795-799`.
- **Role match.** Same writer shape (both entities get one link value from a dungeon counter) and same release (clear the link when it matches). Blue-verified for the role; red-hypothesis for the Red side.
- **Offset.** Red's `aiTarget` is at `0x78` and `aiTargetPos` at `0x88` (+0x10). Blue's `aiTargetPos` is at info `+0x80` (writer `0219082C`, in [`structs.md`](structs.md)), so the shift at `0x88` is `-8`. If `-8` continues to `0x9C`, Red's `unk9C` would be Blue `0x94`, not `0x90`. The `0x90` offset implies a shift of `-0xC`, which no field check supports.
- **Blue `+0x94`.** Its accesses are in `0x021546F0`, `0x02156824`, `0x0215CDB4`, `0x02166B10`, `0x0217D5FC`, `0x021861E8`, `0x0218E428` and `0x0219ADFC`. None is identified as the link.

Verdict: Blue `+0x90` is the wrap link (role verified by a writer and a release helper). Its offset relation to Red `unk9C` is **not reconciled**. The simple `-8` shift predicts `0x94`; the `0x90` offset needs a field map of Blue's `0x88` to `0xA0` that is not in the repo. Confidence: high for the role and the writer; low for the offset equivalence.

**Not verified**

- No wrap was observed at runtime, so the writer and the release are static only.
- Blue's `0x88` to `0xA0` field layout.
- Whether Red's `gDungeon->unk37F4` and Blue's `gDungeon+0x3354` count the same thing.

### Confusion and cringe (#190 P3)

**Tag:** blue-verified for the setter, the blocks, the readers and the flag path; red-hypothesis for the names (`ConfuseStatusTarget`, Red's value names). The turn range `(6, 12)` is blue-verified at runtime. Confidence: high for the setter, the blocks and the readers of 2; medium for the behaviour summary; low for the cringe values 1 and 3 to 6.

- **Setter** `ov_0001::0218E27C`, value 2 (`+0xAC = 2`):
  - returns early if the target has held item `0x12` (message when the caller asks for one);
  - blocked by ability `'@'` (Own Tempo), which the code tests with `AbilityIsActive`;
  - if already confused (`+0xAC == 2`): message `021B0D14` and no change;
  - otherwise `+0xAC = 2`, `+0xAD = FUN_0219CF28(entity, 0x021AA300, 0) + 1`, message `021B0D28`, then `0213FDCC` before and `0213FF34` after.
  - Turn range (runtime): `0x021AA300` = **(6, 12)**.
- **Readers of value 2.**
  - `ov_0001::0217FD78` (monster turn): value 2 sets a flag. When the flag is set, the routine prints message `021AE488` instead of choosing a move from the four move slots at `info+0xF4`, then calls `0218E3E4`.
  - `ov_0001::0215A520` and `ov_0001::0215AB34` (target setup): the flag is `value == 2 || value == 4 || info+0xC9 == 1`. It is passed as `param_7` to `0x0215A3A8`, the target-list builder.
  - `0x0215A3A8`: when `param_7 != 0` it skips the range and class filter (`bVar3 = true`), so every entity that passes `0x0215A054` is added to the list.
  - `ov_0001::0215B384`: when the value is neither 2 nor 4 it resets `info+0x138` and `info+0x13A`. The meaning of those fields is not known.
  - `ov_0001::02175850` (Own Tempo cure): when the ability `'@'` is active and the value is 2, it calls `0218FA80` (cringe end).
- **Cringe values.**
  - Writers seen: `0218A9F8` writes 7 (infatuated; the dispatcher bit `0x200` calls it, see the paralysis section below). `0218AB34` compares to 3, `0218D004` to 5, `0218CC14` to 6, `0218E19C` to 4. The constants written are not read, so those four mappings are not confirmed.
  - `0218FA80` ends the class with a switch on values 0 to 8, each with a message.
  - Readers of 3 and 7: `0x02180048` (returns 1 for each, see #190 P1).
- **Confusion behaviour, as far as the code shows.** A confused monster skips move choice on its turn (`0x0217FD78`). Its target list ignores range class (`0x0215A3A8`). It is blocked by Own Tempo and by held item `0x12`. Its counter is `+0xAD`, which `0x0218FA80` clears. Whether the target list ignoring class is the Red "random direction" behaviour is not checked.

**Not verified**

- Confusion at runtime (no confused monster observed).
- Whether `0x0215B384`'s reset of `+0x138` and `+0x13A` is a confusion effect.
- The constants written by `0218AB34`, `0218D004`, `0218CC14` and `0218E19C`.
- The ability names and the item name behind `'@'`, `0x12`, `0x16` and `0x19`.

### Paralysis, sleepless, the 0x7F sentinel and 0x144 (#190 P4)

Tag: **blue-verified** for the setter, the writer of sleepless, the dispatcher bits, the `0x7F` behaviour and the table values. red-hypothesis for Red's names (`gParalysisTurnRange`, `gSleeplessTurnRange`) and for the mapping of the values to statuses. Confidence: high for the setter, the `0x7F` behaviour, the table values and the writer of sleepless; medium for the bit table; low for `0x400`.

- **Paralysis setter** `ov_0001::021885A0(source, target, param)`:
  - immunity check first: `ov_0001::02180870` (same as burn). Returns if it is nonzero;
  - ability `0x16` (Limber; the Blue code uses the char literal `'\x16'`): message, no status;
  - if already paralysed (`+0x9B == 4`): message `021B0CEC`;
  - otherwise `+0x9B = 4`, `+0x9C = FUN_0219CF28(entity, 0x021AA304, 1) + 1`, `+0x9D = 0`, `+0x9E = 0`, message `021B0D00`, `0213FD88`, `CalcSpeedStage`;
  - ability `0x19`: spreads paralysis to entities on the neighbouring tiles (loop over the offsets at `0x021A98B4`).
- **`gParalysisTurnRange` = `0x021AA304`**, runtime value **(1, 2)**. It is the only range the paralysis setter reads.
- **`gSleeplessTurnRange` = `0x021AA32C`**, runtime value **(10, 12)**, read by `ov_0001::0218AC74`, the only reader. The writer is `0218AC74`, which sets `+0x99 = 2` (sleepless), `+0x9A = FUN_0219CF28(entity, 0x021AA32C, 0) + 1`, `+0x132 = +0x133 = 0xFF`, then `0213FA04` and `0217ADF8(8)`. This resolves the earlier "no writer found" note.
- **`+0x144` bits** (`ov_0001::0215BAF8`, the pending-effect dispatcher, a `u16`). Each set bit calls its setter, then the word is cleared:

  | Bit | Call | Effect |
  |-----|------|--------|
  | `0x001` | `02189CBC` | shadow hold (`+0xA0 = 2`) |
  | `0x002` | `02189CBC` | shadow hold (second message) |
  | `0x004` | `02189CBC` | shadow hold (third message) |
  | `0x008` | `021885A0` | paralysis |
  | `0x010` | `021885A0` | paralysis (second message) |
  | `0x020` | `0218A410` | poison (`+0x9B = 2`) |
  | `0x040` | `0218A410` | poison (second message) |
  | `0x080` | `0218B2D0` | sleep, turns from `0x021AA324` = (3, 7) |
  | `0x100` | `0218A710` | burn |
  | `0x200` | `0218A9F8` | infatuated (`+0xAC = 7`) |
  | `0x400` | `0213EC7C`, then `+0xD7` from `0x021AA1FC` | not identified |

- **`0x7F` sentinel.** `ov_0001::0219BF1C` (`DecrementStatusTurns`): if the byte is `0x7F`, it is unchanged; if it is nonzero, it is decremented; a value of 0 is also unchanged. The caller (`TickStatusAndHealthRegen`) checks `== 0` after the call and runs the class end. So `0x7F` never expires, and 0 is never decremented below 0.
- **Other ranges** (runtime): `0x021AA2C0` = (4, 8) (yawning, read by `0218FE64` case 4); `0x021AA2EC` = (127, 127) (burn); `0x021AA2F4` = (127, 127) (bad poison); `0x021AA300` = (6, 12) (confusion). `0x021AA11C` = 8 and `0x021AA1D0` = 999 are damage amounts, not turn ranges (read by `0218FE64` cases 3 and 5).

**Not verified**

- The writers of the `+0x144` bits (which code sets them).
- The bit `0x400` effect and `0x021AA1FC`.
- Paralysis at runtime. The table values are read at runtime; no paralysis was observed.
- The ability names behind `0x16` and `0x19`.
- Whether `0x021AA32C` is the only `(10, 12)` sleepless table. The scan found more `(10, 12)` pairs at `0x021AA330`, `0x021AA334`, `0x021AA340`, `0x021AA35C`, `0x021AA360`, `0x021AA364`, `0x021AA368`, `0x021AA37C`, `0x021AA388`, `0x021AA390` and `0x021AA398`. Only `0x021AA32C` is read by the sleepless writer. The others are read by `0218EC2C` (`0x021AA330`), `0218ECF0` (`0x021AA334`) and `0218ED78` (`0x021AA340`), whose statuses are not identified.

### Durations, poison, burn and countdowns (#191 P1)

Tag: **blue-verified (runtime)** for the durations, the burn sentinel, the countdown caller finding and the constants; **blue-verified (range)** for poison. Confidence: high for the duration table and the countdown caller in this run; the gate behind the caller is not identified.

Durations (runtime: `CalculateStatusTurns` `0219CF28` called with each range pair, 100 samples each):

| Status | Range pair (RAM, read at runtime) | Observed roll | Setter adds | Tag |
|---|---|---|---|---|
| Sleep | `021AA324` = (3, 7) | 3:25, 4:22, 5:28, 6:25 | none seen | blue-verified (runtime) |
| Nightmare | `021AA328` = (4, 8) | 4:23, 5:25, 6:25, 7:27 | n/a | blue-verified (runtime) |
| Napping | `021AA344` = (2, 4) | 2:54, 3:46 | n/a | blue-verified (runtime) |
| Yawn | `021AA2BC` = (2, 2) | 2:100 | n/a | blue-verified (runtime) |
| Burn | `021AA2EC` = (127, 127) | 127:100 | +1 (`0x9C = roll + 1`) | blue-verified (runtime) |
| Bad poison | `021AA2F4` = (127, 127) | 127:100 | +1 | blue-verified (runtime) |
| Poison | `021AA2F0` = (127, 127) | range read only, not sampled | +1 | blue-verified (range) |
| Frozen | `021AA31C` = (3, 5) | 3:50, 4:50 | +1 | blue-verified (runtime) |

The roll is `min + rand % (max - min)`, so `max` is exclusive. Red's ranges (`src/dungeon_config.c`) match the pairs for burn, poison, bad poison, sleep, nightmare, napping, yawn and frozen.

- **Burn sentinel** (runtime, tick trace): the setter writes `roll + 1 = 0x80`. The first countdown tick reduced `0x9C` to `0x7F`, and it then stayed at `0x7F` for 870 ticks. The status did not clear, so burn lasts for the rest of the floor. Blue-verified (runtime).
- **Countdown caller** (runtime): the countdown bytes (`0x9A` sleep, `0x9C` poison and burn) decremented on each call from `0x02140BA4` while the counter was above 0 (14 of 14 measurable calls in the poison trace, 3 of 3 in the sleep trace). The calls from `0x021409D0` and `0x021410A8` did not decrement. The calls from `0x02140BA4` came every 4 or 5 ticks. In the sleep trace the value was 3 after tick 1 and went 3 to 2 to 1 to 0 at ticks 6, 11 and 16. Blue-verified (runtime). This is the finding that corrects the call-site statement above.
- **Per-turn poison damage** (runtime, `eot` events from `021861E8`): the damage counter `0x9D` starts at 0, is reset to 10 after each damage, and decrements once per `021861E8` call. Damage is 4 at the reset (`0x9D` 1 to 10 with HP 21 to 17 at eot n=44). The first damage event (n=7, HP 26 to 22) came with `0x9D` at 9, so the first reset at 0 is not seen in this trace. Two damage events were seen over 60 ticks. The 1-HP steps seen elsewhere in the trace are not explained (probably belly or another source, not checked).
- **Burn damage** (runtime, `tick_burn128`): HP 30 to 3 over 870 ticks, with burn damage 5 steps every 20 decrements.
- **Constants read from RAM**: poison damage `021AA1CC` = 4, countdown `021AA220` = 10; burn damage `021AA1BC` = 5, countdown `021AA224` = 20; bad poison countdown `021AA21C` = 2; nightmare damage `021AA11C` = 8. All match Red (`gPoisonDmgValue`, `gBurnDmgValue` and the countdowns).
- **RNG per tick** (runtime, keyed by return address): `DungeonRandInt` at `0x02186894` (inside `021861E8`) is one call per tick, the Shed Skin roll (blue-verified). `DungeonRandInt` at `0x02193260` (inside `02192D24`) is one or two calls per tick; `02192D24` is called from `02188030` and `02180048`, not from the tick chain (not explained). `DungeonRandInt` at `0x0215A950` (inside `AccuracyCalc`) is caused by the leader's own actions. Global `Rand32Bit` calls are numerous and come from the game loop.

**Not verified**

- Whether the `0x02140BA4`-only decrement is a gate in `TickStatus` or a property of the call site (need a read of `02185C70` around its countdown calls).
- The burn and poison setters' roll is not called through the setter path at runtime; only the roll function was sampled, and the setters' `+1` is from the decompile.
- Poison and bad poison roll samples: the range read is (127, 127) but the roll was not sampled for poison.
- The `0x02193260` RNG site.
- The 1-HP steps.
- Regeneration blocking under poison (step 1 gate) not measured.
- Sleep wake-on-hit: not tested (this is in P1.6 of #51).
- Bad poison damage table at `021AA54C` (30 entries): not sampled at runtime.

### Pool words and the HP-tier modifier (#191 P2)

Tag: **blue-verified (runtime values, decompile of the reader).** Confidence: high for values and the index mapping; medium for the HP thresholds (taken from the integer-division shape in the decompile).

The pool words are not message strings. They are four 24.8 fixed-point damage modifiers, read at runtime from RAM:

| Address | Value (runtime) | As 24.8 | Index |
|---|---|---|---|
| `021AA3E4` | `0x19` | 0.098 | 0 |
| `021AA3E8` | `0x33` | 0.199 | 1 |
| `021AA3EC` | `0x80` | 0.5 | 2 |
| `021AA3F0` | `0x100` | 1.0 | 3 |

- **Reader** `FUN_ov_0001__02160250`, called from `FUN_ov_0001__02156824` (the move-effect dispatcher) at `02159268`. It picks an index from the user's HP fraction: HP ≤ max/4 gives 0; max/4 < HP ≤ max/2 gives 1; max/2 < HP ≤ 3max/4 gives 2; HP > 3max/4 gives 3. It then calls `HandleDamagingMove` (`0215675C`) with the chosen value as the 4th argument, the `modifier` (s24.8) parameter. So this is an HP-scaled damage move (the same shape as Red's HP-scaled moves; the exact move is not identified).
- The `021AA3E4` entry is the table base: the decompile names it `PTR_DAT_ov_0001__021aa3e4_...` (the literal at `021602F8` points to `021AA3E4`). The reads at `021AA3EC` and `021AA3F0` are entries 2 and 3.

**Not verified**

- Which move uses this reader (the caller is the dispatcher `02156824`; the move ID switch was not traced).
- The Red counterpart (not compared; no Red source was fetched for this).
- That the four modifiers are used exactly like the Red move table (the Red values were not compared).

### Residual damage tags (#191 P3)

Tag: **blue-verified** (values and the call shape); **red-hypothesis** (the Red enum names, from the pinned commit). Confidence: high.

The second argument of `0217BCA4` (a wrapper of `HandleDealingDamage_Async`) is Red's `ResidualDamageType`, and the fourth is the Red dungeon exit reason:

| Residual | Blue call (`021861E8`) | Red residual enum | Red exit reason | Blue tag (runtime) | Red value |
|---|---|---|---|---|---|
| Burn | `0217BCA4(e, 5, 1, 0x208)` | `RESIDUAL_DAMAGE_BURN` = 1 | `DUNGEON_EXIT_FAINTED_FROM_BURN` = 520 | `0x208` | 520 (match) |
| Poison | `0217BCA4(e, 4, 3, DAT_021871A0)` | `RESIDUAL_DAMAGE_POISON` = 3 | `DUNGEON_EXIT_FAINTED_FROM_POISON` = 522 | `0x20A` (`021871A0` read at runtime) | 522 (match) |
| Bad poison | `0217BCA4(e, table[i], 3, DAT_021871A0)` | `RESIDUAL_DAMAGE_POISON` = 3 | 522 | `0x20A` | 522 (match) |
| Curse (per turn, `0x20C`) | `0217BCA4(e, i, 7, 0x20C)` | `RESIDUAL_DAMAGE_CURSE` = 7 | `DUNGEON_EXIT_FELLED_BY_CURSE` = 524 | `0x20C` | 524 (match) |

Red's exit reasons are counted from `DUNGEON_EXIT_REASON_START` = 500 in `include/constants/dungeon_exit.h`: the list index gives the value. Burn is list index 20 (520), poison index 22 (522), curse index 24 (524).

**Not verified**

- That the second argument is the residual type in `HandleDealingDamage` itself (the call shape matches Red's `DealDamageToEntity_Async(entity, dmg, residualType, exitReason)`, but the wrapper's parameter names were not read).
- The `0x9B` 4 (paralysis) and other residual tags.

### Burn and frozen setters, line by line (#191 P4)

Tag: **blue-verified** (decompile); **red-hypothesis** (the Red side, from the pinned source). Confidence: high for the order and the checks; medium for the helper names and the release-link field.

**Burn** `FUN_ov_0001__0218A710` against Red `BurnedStatusTarget` (`src/move_orb_effects_1.c:339`):

| Step | Red | Blue | Match |
|---|---|---|---|
| Valid target | `EntityIsValid` | `EntityIsValid` | yes |
| Safeguard | `SafeguardIsActive(pokemon, target, displayMessage)` | `FUN_02180870(param_1, param_2, param_4)` | yes |
| Water Veil | `ABILITY_WATER_VEIL` → message | `AbilityIsActive('B')` (0x42), message if flag | yes |
| Fire type | `MonsterIsType(TYPE_FIRE)` (2) → message | `MonsterIsType(2)` | yes |
| Water-tile check | `SECONDARY` terrain and water type ≠ LAVA → message | `(tile & 3) == 2` and water type ≠ 1 → message | yes (1 = LAVA not read in the enum) |
| Already burned | `status != STATUS_BURN`; else message | `0x9B == 1` → message | yes |
| Set status | `status = STATUS_BURN` | `0x9B = 1` | yes |
| Turns | `CalculateStatusTurns(gBurnTurnRange) + 1` | `0219CF28(021AA2EC) + 1` → `0x9C` | yes (range 127, 127 at runtime) |
| Damage countdown | `damageCountdown = 0` | `0x9D = 0` | yes |
| Bad-poison counter | `badlyPoisonedDamageCount = 0` | `0x9E = 0` | yes |
| Sprite helper | `if (param_3) sub_8041C34(target)` | `if (param_3) FUN_0213FD70(param_2)` | names differ; not compared |
| Message | `gUnknown_80FB548` | `PTR_LAB_021b0ad0` | message IDs not read |
| Icon | `UpdateStatusIconFlags` | `FUN_0213FF34` | yes (name from the `0213FF34` read in #51) |
| Synchronize | only if newly burned, all 8 neighbours, monster type, treat-as-enemy, recurse with flag 1 | same: newly burned only, 8 neighbours, type 1, `FUN_021808E0 == 1`, recurse with 1 | yes; `FUN_021808E0` not identified with Red's treatment helper |
| Extra Blue step | none | `FUN_02181BF0(DAT_0218A9CC, info, 0)` before the sync message | not in Red; not read |

**Frozen** `FUN_ov_0001__02189F68` against Red `FrozenStatusTarget` (`src/move_orb_effects_1.c:598`):

| Step | Red | Blue | Match |
|---|---|---|---|
| Valid target | `EntityIsValid` | `EntityIsValid` | yes |
| Already frozen | `status != STATUS_FROZEN` | `+0xA0 != 1` | yes |
| Safeguard | `SafeguardIsActive` | `FUN_02180870` | yes |
| Magma Armor | `ABILITY_MAGMA_ARMOR` (0x41) → message | `AbilityIsActive('A')` → message | yes |
| Ice type | `MonsterIsType(TYPE_ICE)` (6) → message | `MonsterIsType(6)` | yes |
| Lava tile | `SECONDARY` and water type == LAVA → message | `(tile & 3) == 2` and water type == 1 → message | yes |
| Wrap release | `if status is WRAP or WRAPPED: sub_8076CB4(unk9C)` | `if +0xA0 in {3,4}: FUN_02189910(+0x90)` | yes in structure; link field differs (`unk9C` vs `+0x90`) |
| Pre-set helper | `sub_8041F08(target)` | `FUN_0213F9C0(param_2)` | names differ; not compared |
| Set status | `status = STATUS_FROZEN` (1) | `+0xA0 = 1` | yes |
| Turns | `CalculateStatusTurns(gFreezeTurnRange) + 1` | `FUN_0219CF28(021AA31C) + 1` → `+0xA8` | yes (range 3, 5 at runtime; Red `gFreezeTurnRange` {3, 5}) |
| Countdown reset | `damageCountdown = 0` (Red `+0x9`) | `+0xA9 = 0` | yes, consistent with the Red `+8`/`+9` struct layout |
| Message | `gUnknown_80FB610` | `PTR_LAB_021b0a44` | message IDs not read |
| Icon | `UpdateStatusIconFlags` | `FUN_0213FF34` | yes |

**Not verified**

- The unnamed helpers (`FUN_0213FD70` / `sub_8041C34`, `FUN_0213F9C0` / `sub_8041F08`, `FUN_021808E0`, `FUN_02181BF0`).
- The wrap link field: Blue `+0x90` against Red `unk9C` is not reconciled (the struct offsets in #46 are needed).
- Message IDs (the message table entries were not read).
- The lava value `1` (Blue uses 1; the Red enum value was not read).

## Open questions

- **Paralysis setter.** `021885A0` is the setter (#190 P4). Its match to Red's setter is not checked.
- **Yawning write.** One finding tags `0218ADA8` as writing `0x99 = 4` (blue-verified). Another lists yawning value 4 only as a red-hypothesis wake path. Confirm the write.
- **Sleepless (2).** Writer `0218AC74` (#190 P4). Other `(10, 12)` tables are not identified (see #190 P4, not verified).
- **Yawn-end range.** `0218FE64` case 4 uses `021AA2C0` (4, 8). Its source is unconfirmed. Check Red's `sub_804151C` path.
- **Yawning per-turn function.** `0213F6B0` is not identified.
- **Wake on hit.** `0217D5FC` and `0218F21C` disagree for nightmare and napping (P1.6). A trace at 127 turns is needed.
- **Countdown gate.** Why only the call from `0x02140BA4` decrements (#191 P1).
- **Ability and item IDs.** `FUN_021818D0`, `FUN_021805B0` (IQ and flag checks `0x11`, `0x12`, `0x14`), `FUN_02181904` (negative-status check): not named. Held item `0x14`, `0x15` and the IQ bit are matched by value only.
- **Calls with unknown arguments.** `0218DF38` (heal) argument order, `0217BCA4` (`…, 1, 0x208`) argument list, and `02190100` behaviour (inferred from its body).
- **Burn-end and burn-apply conditions** in `021760B4`: `0x2A & 3 == 2` and the dungeon flag `0x34AE`, unnamed. Callers of `0215C774` and `0214650C` are unknown.
- **Residual-damage reason tags** mapped in [#191 P3](#residual-damage-tags-191-p3).
- **Message strings** are not read. The message-table pointers are listed above.
- **`0x7F` sentinel.** Confirmed at runtime for burn (#191 P1) and read from `0219BF1C` (#190 P4). It is not named.
- **`+0x144` bits** (dispatcher `0215BAF8`). Effects are in #190 P4. Their writers are not located.
- **Mist** (reflect value 14): its check was not found in Blue.
- **Wrap damage** not found. Next lead `0x0219ADFC` (#190 P1).
- **Bide per-turn release, the sure-shot caller and duration, long toss, perish song, curse, leech seed, snatch and decoy.** Not investigated in detail (see the not-verified items in #189). Red write sites are in `reference/pmd-red/src/move_orb_effects_*.c`. Split into two or three PRs by class.
- **Class identity** of `+0xC7` and `+0xC9` resolved by #189 P2 and P3: `+0xC7` is invisible, transform or mobile; `+0xC9` is blinker.
- **Regeneration gate.** Whether `+0x6 == 0` is Red's `!isNotTeamMember` (likely, not confirmed). Ability and item checks in regen (held item `'\n'`, `AbilityIsActive(3)`, weather `4` through `02195C48`) are not mapped to Red's constants.
- **Red line-by-line comparison** of the burn and frozen setters: done in #191 P4.
- **Identity of** `0213EA14` (indirect call with `(e, 0x18, 1)`) and `0213FF34` (sprite or flag refresh, not read).
- **Global RNG.** `0217AF58` (`SetMonsterAnim`) consumes one global `Rand32Bit` step on every call, and has 13 call sites in overlay 1. The curse decoy end and the `+0xC7` case 2 use it through that path. This matters for the golden-trace hashing in #49 (see [`rng.md`](rng.md)).
- **Runtime check.** Partly done: #190 and #191 ran py-desmume traces for the countdowns, durations, poison and burn, and the constants. The wake rule and the other per-turn claims are still static.
- **Wrap link offset.** Blue `+0x90` vs Red `unk9C`, not reconciled.
- **Confusion behaviour** (value 2): setter and readers read (#190 P3). Runtime not observed.
- **Cringe values 1 and 3 to 6** are message cases only (F5.7).
- **Move-effect dispatch table** behind `02156824` was not traced (P1.1).
- **Pool words** mapped to their HP-tier reader (#191 P2). The move that uses them is not identified.
- **Overlay addresses** of `gParalysisTurnRange` and `gSleeplessTurnRange` are `021AA304` and `021AA32C` (#190 P4, runtime values).

## Links

- Symbols: [symbols.md](../symbols.md)
- Turn loop: [turns.md](turns.md)
- Dungeon struct and entity fields: [structs.md](structs.md)
- RNG: [rng.md](rng.md)
- Verification: [verification.md](../verification.md)
