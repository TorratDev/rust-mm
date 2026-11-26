# Job boards, mail, rescue missions, rewards and rank

**Date:** 2026-11-26
**Dump:** Pokémon Mystery Dungeon – Rescue Team Blue (EUR), `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Static evidence comes from the ghidra-cli text export of the `rescue-arm9` project (STAMP 2026-11-14, 5284 functions, 0 failed) and from 32-bit pool words of the extracted `arm9.bin`.
**Issue:** [#526](https://github.com/TorratDev/rust-mm/issues/526) (batch [#645](https://github.com/TorratDev/rust-mm/issues/645), parent [#76](https://github.com/TorratDev/rust-mm/issues/76))
**Workflow:** [workflow.md](workflow.md)

Names that match `pmd-red` (`code_80958E8.c`, `dungeon_jobs.c`, `mission_reward.c`, `code_803C1B4.c`, `rescue_team_info.c`) are given where Blue's code does the same thing; the Blue address and its code are the evidence, the name alone is **red-hypothesis**. No decompiler output, ROM bytes or table values are in this note: tables are named by address and size only.

## Status by point

| Point | Status | What is open |
|-------|--------|--------------|
| P1 records, board, generator | **blue-verified** (record, call order, draw order) | The candidate-list builders' tables; the five reward helpers of step 7 |
| P2 accept, placement, completion | **blue-verified** (static) | The take step (type 5 → 6) in the UI; a dungeon run with a mission target |
| P3 rewards and rank | **blue-verified** | The difficulty lookup `FUN_0200588C`; rank names |
| P4 mail | **blue-verified** for the arrays and the type-7 path | The writer of mail kind 7; kind names |
| P5 runtime | **blue-verified** with forced calls | A natural board generation and accept through the Pelipper UI |

## The job store

The mail and job store is a struct whose pointer cell is `0x020DF6F8`, set by `LoadMailInfo` (`0x0200FCF4`) and used by `SaveMailInfo` (`0x0200DB80`) and `RestoreMailInfo` (`0x0200DD5C`). Offsets: mailbox, 4 records at `+0x00`; Pelipper board, 8 records at `+0x50`; accepted jobs, 8 records at `+0xF0`; news-received flags, 56 bytes at `+0x2F0`; "new mail pending" flag at `+0x328`. The layout matches Red's `unkStruct_203B490`. **blue-verified**.

## P1 — Job records, the board and its generator

| Claim | Evidence | Status |
|-------|----------|--------|
| A job record is 0x14 bytes: `+0` mail type, `+1` mission type, `+2` story-board index (1–3, else 0; the validator rejects values above 9), `+4` dungeon id, `+5` floor, `+8` u32 seed (24 bits), `+0xC` client species (s16), `+0xE` target species (s16), `+0x10` target item, `+0x11` reward kind, `+0x12` reward item, `+0x13` friend-area reward. An empty slot has type 0, dungeon 99, floor 0, reward kind 5 (`ResetJobSlot`). | Writers `0x0200F768`, reader `0x0200F9A8`, `0x0206A428`; runtime: 5 board records read from RAM | **blue-verified** |
| Mail types: 0 empty; 1 PKMN-news mail (mailbox only, dungeon `0x62`, floor = news index); 2–4 story jobs on the board; 5 generated or accepted, not taken; 6 taken for the next dungeon; 7 cleared with 8 and 9 (no writer found); 8 target done inside the dungeon; 9 dungeon left after that, ready for the reward. `ov_0001::0219D8E8` writes 6 → 8 and `ov_0001::0219D8B0` writes 8 → 9. | Readers and writers named | **blue-verified** for 0, 1, 5, 6, 8, 9; 7 **lead** |
| Mission types (same values in the generator and the dungeon code): 0 rescue the client, 1 rescue a target, 2 escort, 3 find an item, 4 deliver an item. These differ from Red's generator names. | `ov_0001::0219D980`, `0x0200E45C`, `0x0200F9A8`, `ov_0001::0219D5B4` | **blue-verified** |
| Slot functions (Red names): ShiftJobSlotsDown `0x0200E2A8`, SortJobSlots `0x0200E1DC` (key: dungeon, then floor), ResetJobSlot `0x0200E350`, IsJobSlotEmpty `0x0200E550`, GetJobSlotInfo `0x0200E57C`, AcceptJob `0x0200E598` (copy into the first empty slot; 0 on success, 1 when all 8 are full), CountJobsinDungeon `0x0200E694`, GetNumAcceptedJobs `0x0200E6EC`, IsMailinJobSlot `0x0200E7C0`, ShiftPelipperJobsDown `0x0200EAD4`, SortPelipperJobs `0x0200EBA0`, ResetPelipperBoardSlot `0x0200EC48`, GetPelipperBoardSlotInfo `0x0200ECC0`, GenerateMailJobInfo `0x0200F768`, GenerateMailJobDungeonInfo `0x0200F5C4`, ValidateWonderMail `0x0200F9A8`. | Decompiles; names matched by code | **blue-verified** (roles); names **red-hypothesis** |

### Board generator `GeneratePelipperJobs` (`0x0200E8BC`)

1. `RandRange(4, 8)` (`0x02011924`) gives `range`; all 8 board types are cleared.
2. Three story jobs, each gated by `CheckQuest` (`0x0201006C`) on its own quest bit. A set quest writes a fixed record (mail types 2, 3, 4; mission type escort; a fixed dungeon, floor and species pair) at the next slot, with one `Rand32Bit() & 0xFFFFFF` draw for the seed.
3. For `i` from the filled count to `range`: `GenerateMailJobInfo(slot)`; stop at the first failure; on success the reward kind is `RandRange(0, 4)`. So the board holds up to `range + 1` jobs with reward kinds 0–3.
4. `SortPelipperJobs`, then `ShiftPelipperJobsDown`.

**blue-verified** (static and runtime, P5).

### Job generator `GenerateMailJobInfo` (`0x0200F768`), in draw order

1. `GenerateMailJobDungeonInfo`: `FUN_020703B8` lists the open dungeons (two fallback dungeons when empty); `RandInt(n)` picks one; `RandRange(floors/2, floors)` picks a floor; it probes forward (wrapping inside the upper half) until the pair is free of the mailbox, board and job slots (`0x0200F568`; an escort job blocks its whole dungeon) and passes `FUN_02006364`; else it moves to the next dungeon with another `RandRange`. Failure returns 0.
2. Mail type 5; mission type from an 8-entry table at `0x02091D5C` indexed by `RandInt(8)`; an escort becomes a rescue when the rescue rank is 0.
3. Story index 0; seed `Rand32Bit() & 0xFFFFFF`.
4. Client and target: `RandInt(n)` twice over the species `1..0x1A7` accepted by `FUN_0206A6DC` (fixed fallbacks when none). For mission types other than 1 and 2 the target is the client.
5. Target item: `FUN_0200F3E4(dungeon)` picks an allowed item with `RandInt(m)`. With no item the job becomes a rescue of the client and `FUN_0206A384` picks one; a deliver job in a dungeon where `FUN_02006434` is 0 also becomes a rescue.
6. Reward item: `FUN_0206A384` (Red `sub_803C37C`), repeated until it differs from the target item; it draws `RandInt(9999)` and `RandInt(65535)` pairs and chooses by the difficulty from `FUN_0206A694`. Friend-area field 0.
7. One more `Rand32Bit()` for mission types 1–4, whose masked bits may call one of five helpers (`FUN_0206A2E0`, `FUN_0206A228`, `FUN_0206A170`, `FUN_0206A0B0`, `FUN_02069FF0`) that rewrite the record. Their bodies were not read.

**blue-verified** for the call and draw order (static and runtime); the candidate tables and step 7's helpers **lead**.

### Mailbox generator (`0x0200EED4`, Red `sub_80961D8`)

Called from `ov_0000::021409F8` (script special 4). It returns false when the mailbox holds 4 records or the pending flag `+0x328` is 0 (and clears it). It then picks one PKMN-news index whose received flag is 0, gated by scenario and story checks (`FUN_020205B0`, `FUN_0201F994`) and the rank. A news pick writes a type-1 record. Otherwise it draws `RandRange(n, 4)` more jobs through `GenerateMailJobInfo` with reward kinds `RandRange(5, 10)`; kind 9 gets a friend area or falls back to 5. **blue-verified** (static; not run).

### When the board is regenerated

`GeneratePelipperJobs` is called from `FUN_0206F6B8` (script special 3, which also sets the pending-mail flag through `FUN_0200F158`), from `LoadAndRunQuickSaveDungeon_Async` (`0x0201E2BC`, the branch with no quicksave) and from the quest setter `FUN_0200FFD4` when one of the three story quests is set. Loading a save and arriving at the Team Base does not regenerate it: the board comes from the save. **blue-verified** (call sites; runtime for the load case).

## P2 — Accepting a job, placing the target, completion

| Claim | Evidence | Status |
|-------|----------|--------|
| `AcceptJob` copies the record unchanged (type stays 5). Taking it for the next dungeon sets type 6; the writer is in the job-list UI (`0x0205xxxx`) and was not located. | `0x0200E598`; forced call (P5) | copy **blue-verified**; the take step **lead** |
| At a new floor `RunDungeon_Async` stores setup byte `+0xD` at dungeon `+0x668` (0 normal, 1 rescue by mail, 2 a floor with accepted jobs), clears `+0x678` (target species) and `+0x67A` (target item) and calls `ov_0001::0219D980` (Red `sub_80842F0`). That function clears `+0x667` (escort flag), stops unless `+0x668` is 2, and scans the 8 job slots for type 6 on this dungeon and floor: deliver sets `+0x67A` and `+0x678`; rescue and escort set `+0x678` (escort also `+0x667`); find-item sets nothing. Each match sets dungeon `+9`. | `RunDungeon_Async` (call at `0x0206E2F8`), `ov_0001::0219D980` | **blue-verified** (static) |
| Placement: with `+0x678` set, `ov_0001::0214B338` marks one more spawn tile, `ov_0001::0214D190` makes no monster house, and the floor-start spawn `ov_0001::02179E2C` (Red `SpawnWildMonsOnFloor`) spawns the target species at level 1 with the mission flag on the first spawn tile it finds from a random start (`DungeonRandInt(0x38)`, `DungeonRandInt(0x20)`). If it never spawns, dungeon `+0xA` is set. | Functions named | **blue-verified** (static) |
| A find-item job has no placement code: no reader of the job slots touches the item list, so the item must appear naturally (the generator only picks items allowed in that dungeon). | Reader list of `GetJobSlotInfo` (6 callers, all read) | **red-hypothesis** for "must appear naturally" |
| Completion: talking to the mission monster (`ov_0001::02172FAC`) calls `0219D5B4` (rescue and deliver; deliver removes the item from the bag) or `0219D2F0` (escort). Both call `0219D8E8(species)`, which turns matching type-6 jobs into type 8, remove the monster and offer to leave (dungeon `+4` = 1, `+0x11` = 2). At the end of the floor `RunDungeon_Async` calls `0219D8B0` (8 → 9) for results 2 and 1, not 4; defeats leave type 8. `FUN_0200E3FC` (clear types 7–9, shift, sort) is called from `RunGameMode_Async` at 5 sites. A find-item job is done when `0x0200E45C` sees the item in the bag at the thank-you screen (`FUN_02055AD8`). | Functions named | **blue-verified** (static); the meaning of dungeon `+9` and `+0xA` **lead** |

## P3 — Rewards and rank

| Claim | Evidence | Status |
|-------|----------|--------|
| `FUN_0206A428(job, out)` (Red `sub_803C21C`) builds the reward. Its rank is `min(difficulty / 2, 6)`, where the difficulty is `FUN_0200588C` plus 2 for an escort, capped at 15 (`FUN_0206A694`). Board kinds 0–3: money (rank + 1) × 100 (kinds 0, 1), the reward item (1, 2, 3), plus one random different item (3). Mailbox kinds 5–8 double the money and add up to two random items; kind 9 gives the friend area. Kinds 4 and 10 and up give nothing; the validator rejects them. | `FUN_0206A428`, `FUN_0206A66C`, `FUN_0206A694`, `ValidateWonderMail` | **blue-verified** |
| Rank points per job come from a 16-entry table at `0x020B7810` indexed by the difficulty (`FUN_0206A63C`). | `FUN_0206A63C` | **blue-verified** (values not recorded) |
| The reward state machine `FUN_02057994` (Red `MR_InitStateDialogue`) adds the money, the friend area, the items and the rank points (`AddToTeamRankPts`), and shows a rank-up message when the rank changes. The thank-you screen `FUN_02055AD8` and the mail preview `FUN_0206BC28` call the builder. | `FUN_02057994` | **blue-verified** |
| Rank: team data at `0x020DED00`, rank points s32 at `+0xC`, team name (10 bytes) at `+0`. `GetRescueTeamRank` (`0x0200881C`) returns the first index 0–5 whose threshold (a 7-entry table at `0x02091C18`) is above the points, else 6 (the top). `AddToTeamRankPts` (`0x02008860`) adds with a cap; `GetPtsToNextRank` `0x02008894`; `GetTeamRankPts` `0x020088D4`. Script specials `ov_0000::02140860` cases `0x2D` (points to the next rank) and `0x2E` (add enough points to rank up; used by the story). The thresholds match Red's `sRescueRankMaxPoints`. | Functions named | **blue-verified** (values not recorded) |

## P4 — Mail

Two stores, easy to confuse:

| Claim | Evidence | Status |
|-------|----------|--------|
| Wonder Mail / rescue mail: 32 records of 0x30 bytes (save section in [../formats/save.md](../formats/save.md)); byte 0 kind, `+0x10` a random value, `+0x20` item, `+0x2D` flag. Accessors: first index of a kind `0x0200D58C` (GetFirstIndexofMailType), by kind and value `0x0200D5CC`, last of kind 1 `0x0200D620`, count `0x0200D658`, count of a kind `0x0200D690`, exists `0x0200D6D8`, get `0x0200D7E0`, delete `0x0200D79C` (DeleteMailAtIndex), copy into slot 1 `0x0200D7F8`, add from index 2 `0x0200D844`, reset `0x0200D8CC`. | Decompiles | **blue-verified** |
| Kind names after `pmd-red` (`constants/wonder_mail.h`): 1 SOS, 2 SOS-1, 4 A-OK, 5 thank-you, 6 completed, 7 OKD. Blue's uses agree: `FUN_02066D14` and `FUN_020651A4` test kinds 2, 4, 6 and 1 against the value at `+0x10` (the save nonce); `FUN_0205F598` counts kinds 1, 5, 4. | Callers named | **red-hypothesis** for the names |
| Kind 7 on Continue: `GameLoop_Async` (`0x0201F2E4`), after title choice 2, looks up the first kind-7 mail. None: `RunGameMode_Async(1)`; one: delete it, then `RunGameMode_Async(2)`. So a kind-7 mail is consumed once, on Continue, and selects mode 2. | `GameLoop_Async`; investigator run: the lookup ran at frame 1562 of the Continue boot | **blue-verified**; the writer of kind 7 **lead** |
| The job store's mailbox (4 records) holds PKMN news (type 1) and mailbox jobs (types 5–9). It is filled by the mailbox generator (P1), read by the mailbox UI (`FUN_02054F08`, `FUN_02056068`, `FUN_02056CDC`), reset through script specials `0xF` and `0x8`; the news-received flags are set by `FUN_0200E198` and read by `FUN_0200E180`. | Functions named | **blue-verified** |

## P5 — Runtime check

The investigator run for this issue (probes not committed) loaded a maintainer save at the Pokémon Team Base:

1. Loading the save and entering the base called neither generator over 4300 frames; the board in RAM came from the save (5 jobs, records as in P1). **blue-verified**.
2. **Forced call** (tagged as forced: a stub called `GeneratePelipperJobs` from a frame hook; the RNG draws are genuine, the transition is not): 6 calls to `GenerateMailJobInfo`, 5 jobs kept (the sixth failed in the dungeon search), 146 RNG draws in the P1 order (caller addresses `0x0200E8CC`, `0x0200F5F8`, `0x0200F638`, `0x0200F794`, `0x0200F7C8`, `0x0200F824`, `0x0200F83C`, `0x0200F86C`, `0x0200F8D4`, `0x0200F914`/`0x0200F964`, `0x0200EAA4`); the board ended sorted by dungeon and floor. Output SHA-256 prefix `4f531f9741192c7f`. **blue-verified** (forced input).
3. **Forced accept:** `AcceptJob(board slot 0)` returned 0 and job slot 0 became a copy with type 5; the board kept its 5 jobs (the UI removes the board slot itself). **blue-verified** (forced input).

The batch recording (run B in [ground-mode.md](ground-mode.md#trace-checks)) reached the base from the same save with the scene runner called with id 1 at frame 1562, as in step 1; it did not open the board.

**Still to show at runtime:** a natural board generation (script special 3 or a dungeon start), the take step through the Pelipper UI, a floor with a mission target, the thank-you scene.

## Links

- [story-flags.md](story-flags.md) (CheckStoryFlag, area lists)
- [../formats/save.md](../formats/save.md) (mail and job sections)
- [dungeon-generation.md](dungeon-generation.md) (spawn tiles, monster house)
- [rng.md](rng.md)
