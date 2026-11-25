# Progression addresses

One entry per line, merged with `merge=union` (see [workflow](../workflow.md#shared-lists)): add a new line, do not reflow or reorder existing ones.

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
| `ov_0001::02178068` | dungeon-end commit: party to roster (IQ copy, lines 57 to 67), recruit list (lines 98 to 103) | blue-verified (static) |
| `ov_0001::02179274`, `ov_0001::02179128` | four-slot team loop (called from `0206DC20` at line 477); party record writeback from the entity (IQ `info+0xC` to party `+0x06`) | blue-verified (static) |
| `02001D24`, `02001E24` | party to roster copy (roster `+0x0A` = party `+0x06`); `02001E24` is the tail jump that passes roster record `idx*0x40` | blue-verified (static) |
| `02006444` | dungeon record `+6` reader (`0x0209281E + id*0x10`): entry rule and exit copy selector (nonzero: held item only, `02001CD8`); callers `0206DF84`, `021788C4`, `021781C0`, `02177C44`, `02177E68` | blue-verified (reader, table, call sites) |
| `0200457C`, `02092FC4` | skill index to name message id (u16 table) | blue-verified (static) |
| `ov_0001::02175E60` | IQ skill unlock message (old list against new list) | blue-verified (static) |
| `0203A87C` | IQ menu draw (skill name, enabled mask test) | blue-verified (static) |
| `02008EC4`, `02009000`, `02009014`, `02003130` | friend-area count, owned test, owned set, roster insert | blue-verified (static) |
| `02002288`, `020022DC` | species friend flag (`+0x12`) and area id (`+0x16`) readers | blue-verified (static) |
| `02009110`, `02009120`, `02009230` | area table readers: field 3 (`u32` at `+4`), field 2 (byte at `+2`), roster index to area | blue-verified (static) |
| `02069848`, `*020DED04` | bulk roster/area init and owned table; runtime at the first floor of a fresh save: only areas 9 and 26 read 1, area 0 reads 0, and `02069848` did not run on that path | blue-verified (static init; runtime owned bytes); new-game identity of init red-hypothesis |
| `020699B4`, `020628BC` | init sub-state step (sub-state 6 calls `02069848`) and dispatcher state `0x10` | blue-verified (static) |
| `0206DC20` | floor exit; calls `02178068` with a flag from the result code | blue-verified (static) |
| `ov_0001::0217F7EC` | recruit-allowed check; friend-area test | blue-verified (static) |
| `ov_0001::02145EC8` | gummi IQ gain | blue-verified (static) |
| `02004464`, `02004250`, `0200429C`, `020042F8`, `0200439C` | IQ unlock test, bit test, default mask, set with group exclusion, toggle | blue-verified (static) |
| `ov_0001::021804CC` | rebuild active skill bits | blue-verified (static) |
| `0200475C`, `0200468C`, `02004590`, `02047D74` | town evolution check, apply, record copy, menu | blue-verified (static) |
| `ov_0001::02182900`, `02001738` | dungeon evolution apply, candidate list | blue-verified (static) |
| `0206C590`, `0206C394`, `0206C978`, `0206C6E4` | quiz question draw, nature pick, partner filter, state machine | blue-verified (static) |
| `02006400`, `02006454` | dungeon record `+2` and `+9` readers | blue-verified (reader and callers) |
| `ov_0001::0217855C` | add a member to a free team slot (writes `0x55AA` via `02001E40`) | blue-verified (static) |
| `ov_0001::0217A054` | collect `0x55AA` team slots, flip to `0x5AA5`, spawn | blue-verified (static) |
| `02006130` | dungeon row byte (`0x02092819 + id*0x10`, record `+1`); the wild-evolution trigger's test | blue-verified (static; literal `02006134`) |
| `0206DE4C`, `0206DE64` | `gDungeon+0x63C` dungeon-id writers (`0206DE64` forces floor 1) | blue-verified (static) |
| `ov_0001::0217E7BC` | only direct nonzero `info+0x11B` store (wild/non-team branch of `0217D5FC`); also sets `gDungeon+0xD` | blue-verified (static) |
| `ov_0001::02178C04`, `ov_0001::02178B9C` | loaded-sprite pointer read (`gDungeon+0x158DC+species*4`) and per-species `ax%03d` fill | blue-verified (static) |
| `ov_0001::02178CB0`, `ov_0001::02178C4C` | per-floor sprite-pointer loads (floor list, team, special records) and live-entity reload | blue-verified (static) |
| `ov_0001::02182A74` | common join after dungeon-evolution apply (the 316 branch rejoins here) | blue-verified (static) |
