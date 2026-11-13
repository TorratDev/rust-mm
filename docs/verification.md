# Phase 0 verification log

Single log for the Phase 0 checks (V1–V9). Each check records its assumption, the date it was run, the evidence, the outcome, and any fallback adopted. Phase gates are defined in [`roadmap.md`](roadmap.md#phase-0--verify-assumptions).

## Status

| ID | Assumption (short) | Blocks | Status |
|----|--------------------|--------|--------|
| V1 | `sdl3` builds, links, exposes renderer / gamepad / audio stream | Phase 1 | **Confirmed** — Linux and Windows MSVC build and headless run pass in CI; gamepad input on real hardware not exercised |
| V2 | egui/eframe, insta, proptest, cargo-nextest, cargo-fuzz work on stable, edition 2024 | Phase 1 | Partial — egui/eframe build and insta/proptest tests pass on Linux and Windows in CI; cargo-nextest not run; cargo-fuzz nightly requirement confirmed from docs |
| V3 | Blue's audio uses the Nintendo SDAT container | Phase 4 audio | **Confirmed** |
| V4 | Non-audio formats identified; save media identified | Phase 2 | **Confirmed** — file survey complete (171 entries tagged); save media is 2 Mbit (256 KiB) flash, save layout mapped |
| V5 | `pmd-red` decomp covers dungeon, combat, AI, RNG | Phase 3 | **Confirmed, with naming gaps** |
| V6 | Blue reuses Red's data tables and constants | Phase 3 | **Confirmed** — 4 tables shared by value (type chart, effectiveness combination, attack and defense stat stages); 2 routine constants match (RNG step, damage calc) |
| V7 | DS emulator can be scripted to dump RAM/RNG per turn | Phase 3 trace harness | **Confirmed** — retail Blue on Windows: replay from power-on to the first dungeon floor, one dump per turn on `RunFractionalTurn_Async`, byte-identical over 3 runs; RNG formulas and seed chain checked against the dumps. synthetic-ROM CI job passes on Linux and Windows |
| V8 | Ghidra loads ARM9 and overlays at real addresses | Phase 3 | **Confirmed** — retail ARM9 and all 145 overlays mapped (Ghidra 12.1.4); type chart referenced from 4 places in overlay 1; the 142 overlays without code are data |
| V9 | REA supports the planned Ghidra workflow on ARM DS binaries | Phase 3 | **Replaced** — REA 6.5.0 rejects ARM targets on Windows and cannot open the V8 project; ghidra-cli 0.2.2 on the V8 project covers function search, xrefs, decompilation (ARM and Thumb) and notes |

Outcome values: **confirmed**, **partial**, **replaced** (fallback adopted), **blocked**, **pending**.

## Dumps used

Dumps are identified by SHA-1 and are not committed. The roadmap's legal hygiene rule applies. The list the tools accept is [`supported-dumps.md`](supported-dumps.md).

| Dump | Size | SHA-1 | Used for |
|------|------|-------|----------|
| Pokémon Mystery Dungeon – Rescue Team Blue (EUR, NDS), `pmdb.nds` | 33,554,432 bytes | `6f01887356268de39085df01490347bbcc3aa330` | V3, V4, V6 |
| Pokémon Mystery Dungeon – Red Rescue Team (GBA), `rs-pmdr.gba` | 33,554,432 bytes | `afee3b060dd5fd4a68afb1b003456aef3a2af073` | V6 control only |

The `pmd-red` decomp expects **Red Rescue Team 1.0 (US)**, SHA-1 `9f4cfc5b5f4859d17169a485462e977c7aac2b89`. The Red dump above does **not** match that base, so it is not the decomp's base ROM. Its matches below are by value, not by build.

## Entry template

Copy this section for each check.

```markdown
## Vn — <assumption>

- **Assumption:**
- **Date:** YYYY-MM-DD
- **Environment:** OS, rustc / tool versions, hardware or emulator as relevant
- **Evidence:** commands run, CI run links, addresses, file paths, output excerpts
- **Outcome:** confirmed | partial | replaced | blocked | pending
- **Fallback adopted:** none | <fallback from the roadmap> (link to decision)
- **Notes:**
```

---

## V1 — `sdl3` crate builds and links, with renderer, gamepad and audio-stream APIs

- **Assumption:** the `sdl3` crate builds and links on Windows MSVC and Linux (static or bundled dynamic), and exposes SDL3's 2D renderer, gamepad and audio-stream APIs.
- **Date:** 2026-10-13
- **Environment:** Linux (Ubuntu 24.04, gcc 13.3, cmake 3.28.3), rustc 1.97.0 stable, `sdl3` 0.20.0 with `sdl3-sys` 0.7.2+SDL-3.4.18, feature `build-from-source-static`. CI: `ubuntu-latest` and `windows-latest` (MSVC), rustc 1.99.0 stable.
- **Evidence:**
  - Probe: [`verification/sdl3-probe`](../verification/sdl3-probe). Feature `build-from-source-static` builds SDL from the source bundled in the `sdl3-src` crate, so no network download of SDL is needed.
  - Linux build needs the packages that the CI workflow `.github/workflows/verify.yml` listed (CMake configure fails without them). That workflow file was removed in `320b68d` and is not in the repository, so the list is no longer here.
  - Local run: window, 256×192 texture target, `with_texture_canvas` + `copy` + `present`, gamepad subsystem with `GamepadButtonDown` events, and an audio playback stream with an `AudioCallback`. `cargo build` succeeds. Run with `SDL_VIDEO_DRIVER=offscreen SDL_AUDIO_DRIVER=dummy`: 120 frames, exit 0, "gamepads attached: 0". No gamepad was attached, so gamepad input was not exercised.
  - CI run [38444628595](https://github.com/TorratDev/rust-mm/actions/runs/38444628595): `cargo build` and `cargo run` (headless, exit 0) pass on both `ubuntu-latest` and `windows-latest`.
- **Outcome:** confirmed. Static bundled build links and runs on Linux and Windows MSVC.
- **Fallback adopted:** none.
- **Notes:** Gamepad input on real hardware is still to be exercised (no pad attached in CI).

---

## V2 — egui/eframe, insta, proptest, cargo-nextest, cargo-fuzz on stable Rust, edition 2024

- **Assumption:** `egui`/`eframe`, `insta`, `proptest`, `cargo-nextest` and `cargo-fuzz` work on stable Rust, edition 2024, on both OSes. Any nightly requirement is confined to `fuzz/`.
- **Date:** 2026-10-13
- **Environment:** Linux, rustc 1.97.0 stable, edition 2024. CI: `ubuntu-latest` and `windows-latest`, rustc 1.99.0 stable.
- **Evidence:**

  | Crate | Version checked | Declared `rust-version` | Result on Linux |
  |-------|-----------------|-------------------------|-----------------|
  | egui | 0.36.2 | 1.95 | builds (dependency only, not exercised) |
  | eframe | 0.36.2 | 1.95 | builds (dependency only, not exercised) |
  | insta | 1.49.0 | 1.66.0 | snapshot test passes |
  | proptest | 1.12.0 | 1.88 | property test passes (2 passed) |
  | cargo-nextest | 0.9.148 | 1.91 | not run |
  | cargo-fuzz | 0.13.2 | unknown | not run |

  - Probe: [`verification/v2-probe`](../verification/v2-probe), versions pinned. `egui`/`eframe` are declared but unused, so the probe shows they compile and link, not that a window opens.
  - CI run [38444628595](https://github.com/TorratDev/rust-mm/actions/runs/38444628595): `cargo test` passes on `ubuntu-latest` and `windows-latest` (2 passed).
  - `cargo-fuzz` README: needs a nightly compiler (unstable `-Z` sanitizer flags), works only on x86-64 and AArch64, and **only on Unix-like systems, not Windows**.
- **Outcome:** partial. egui/eframe, insta and proptest confirmed on stable on Linux and Windows. The cargo-fuzz nightly requirement is confirmed from its documentation. cargo-nextest and cargo-fuzz runs are pending.
- **Fallback adopted:** none. Consequence: the fuzz job runs on Linux only, with its own nightly toolchain, and is excluded from the Windows matrix.
- **Notes:** Still to do: `cargo nextest` on both OSes and `cargo fuzz` on Linux.

---

## V3 — Blue's audio uses the Nintendo SDAT container

- **Assumption:** Blue's audio uses the standard Nintendo SDAT container (SSEQ/SBNK/SWAR/STRM).
- **Date:** 2026-10-13
- **Environment:** Blue dump above, parsed in Python 3 (scratch script, not committed).
- **Evidence:**
  - `sound.sbin` (2,253,504 bytes) starts with `SDAT`, byte order `0xFEFF`, and a header `file_size` equal to the file length.
  - Blocks: `SYMB` at `0x40` (21,264 bytes), `INFO` at `0x5350` (4,512 bytes), `FAT` at `0x64F0` (2,988 bytes), `FILE` at `0x709C` (2,224,676 bytes). The `SYMB` block is present, so symbol names are available.
  - FAT: 186 entries, all in bounds. By magic: **98 SSEQ**, **83 SBNK**, **4 SWAR**, **1 SSAR**. No `STRM` magic appears as a FAT entry, so stream count is not established here.
  - Survey row: `sound.sbin` tagged *known*, Nintendo SDAT sound archive ([`file-survey.md`](file-survey.md)).
- **Outcome:** confirmed. The SDAT is located and its FAT and SYMB are parsed. Stream count is still open (the roadmap asks for streams too).
- **Fallback adopted:** none.
- **Notes:** Check the `INFO` block for stream entries before closing Phase 4 audio.

---

## V4 — Non-audio formats and save layout

- **Assumption:** Blue's non-audio formats (compression, graphics, palettes, animation, text encoding, data tables, save layout) are unknown and must be identified.
- **Date:** 2026-10-13
- **Environment:** Blue dump above. Survey tool [`tools/nds_survey.py`](../tools/nds_survey.py).
- **Evidence:**
  - Output: [`docs/file-survey.md`](file-survey.md), metadata only.
  - Totals: 171 entries = `arm9.bin`, `arm7.bin`, 145 overlays (ARM9 and ARM7 tables), and 24 named files. The FNT has one directory (root), so all 24 named files are flat.
  - Tags: 1 *known* (`sound.sbin`, SDAT), 155 *hypothesis*, 15 *unknown*.
  - Overlay flag bit 24 marks BLZ compression. Many overlays have it set, and their raw bytes have entropy around 7.2–7.5. Their contents are not decompressed in the survey.
  - "Header guess" tags (LZ10, Huffman, RLE) are a weak check on the first byte. Treat them as leads, not findings. The `message_*.str` files all start with `20 34 00 00`, which is a plausible LZ/RLE-style header but is unverified.
  - **Save media (2026-10-14, Ghidra):** the boot code calls NitroSDK's `CARD_IdentifyBackup` with `0x1202`, which is `CARD_BACKUP_TYPE_FLASH_2MBITS`: 2 Mbit (256 KiB) serial flash, used in 4 KiB sectors. The main save is a `0x57D8`-byte block written twice (sectors 0–5 and 6–11), with a word-sum checksum and Red's eight sections plus a Blue-only language field. Details: [`formats/save.md`](formats/save.md).
- **Outcome:** confirmed. The survey covers 100% of entries, each tagged, and the save media type and size are identified.
- **Fallback adopted:** N/A (discovery task). Unknown formats become Phase 2/3 work items.
- **Notes:** BLZ overlays are now decompressed by `tools/nds_extract_code.py` (V8). The save media has not been checked on a real cartridge save yet.

---

## V5 — `pmd-red` decomp coverage

- **Assumption:** the pret `pmd-red` decomp has enough of the dungeon, combat, AI and RNG code decompiled and named to be useful.
- **Date:** 2026-10-13
- **Environment:** `pret/pmd-red` shallow clone (read only), counted with `grep`/`find` in the clone.
- **Evidence:**

  | Subsystem | C files | C lines | Files with `NONMATCHING` | Notes |
  |-----------|---------|---------|--------------------------|-------|
  | Dungeon (`src/dungeon_*.c`) | 93 | 62,355 | 4 | Largest subsystem |
  | Combat (damage, moves, move orbs, AI attack) | 15 | 13,959 | 1 | |
  | AI (`dungeon_ai*`, `*_ai_*`) | 6 | 2,987 | 0 | |
  | RNG (`*random*`, `*rng*`) | 4 | 291 | 0 | `Random32MersenneTwister` in `random_mersenne_twister.c` |

  - Whole tree: 696 C files, 254,672 lines. Remaining `.s` files: `arm_funcs.s`, `m4a_1.s`, `rom_header.s`, `crt0.s` (1,903 lines). None is game logic.
  - Naming: 2,128 unique unnamed identifiers (`sub_`/`FUN_`/`unk_` with an address) are referenced from 277 C files. Most are in source files, not data files.
  - The repo README does not publish a percent-decompiled figure. The numbers above are proxies, not a verified percentage.
- **Outcome:** confirmed, with naming gaps. Dungeon, AI and RNG are substantially decompiled. Combat is decompiled, but its callers depend on 2,128 unnamed identifiers.
- **Fallback adopted:** none.
- **Notes:** Correction (2026-10-14): the RNG row lists only the Mersenne Twister, but Red's main generators are LCGs (`src/random.c`, `src/dungeon_random.c`). Blue has the same three generators ([`re/rng.md`](re/rng.md)). The roadmap's fallback ("lower Red's role") is not triggered. Naming gaps mean V6 comparisons may need Ghidra analysis of Blue for the call targets.

---

## V6 — Blue reuses Red's logic

- **Assumption:** Blue reuses Red's logic closely enough that Red is a useful map.
- **Date:** 2026-10-13
- **Environment:** Blue dump and Red dump above, scratch BLZ decoder (not committed). Tables taken from `pmd-red` source: `gTypeEffectivenessChart` (`src/dungeon_config.c`), `gTypeGummiIQBoost` and `gUnknown_810A808` (`src/dungeon_data.c`). Each is 18×18 `s16`.
- **Evidence:**

  | Table | Red dump (control) | Blue dump |
  |-------|--------------------|-----------|
  | `gTypeEffectivenessChart` (18×18, `s16`) | exact at `0x1B7870` | **exact** in `arm9 overlay 1` (BLZ-decoded) at `0x72688` |
  | `gTypeGummiIQBoost` (18×18, `s16`) | exact at `0x2725E8` | not found (as `s16` or `u8`), raw or BLZ-decoded |
  | `gUnknown_810A808` (18×18, `s16`) | exact at `0x272870` | not found (as `s16` or `u8`), raw or BLZ-decoded |

  - The Red control confirms the table extraction. The Blue hit is a full 648-byte match after BLZ decoding.
  - Searches covered ARM9 binary, all 145 overlays (BLZ-decoded where flagged), and all named files.
  - **Routine constant 1, RNG step (2026-10-14, Ghidra):** Blue's global LCG (`Rand16Bit`, `020119A4`) and dungeon LCG (`DungeonRand16Bit`, `ov_0001::0219D07C`) both step `s * 0x5D588B65 + 1` and return `s >> 16`, like Red's `src/random.c` and `src/dungeon_random.c`. The seeding constant `0xD305` (54021) in `SeedRng` and the floor-seed mask `& 0xFFFFFF | 1` also match. The multiplier is in the Red dump too (file offsets `0x6994`, `0x890D0`, `0x89100`, `0x9AF08`). Differences: Blue seeds the global LCG from the console MAC address. Details: [`re/rng.md`](re/rng.md), [`blue-vs-red.md`](blue-vs-red.md).
  - **Routine constant 2, damage calc (2026-10-14, Ghidra):** Blue's `CalcDamage` (`ov_0001::0217BD94`, found from the type chart's readers) has the constants of Red's `CalcDamage` (`src/dungeon_damage.c`): damage cap `999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`, and the same three dungeon RNG calls in the same order (`DungeonRandInt(100)`, `DungeonRandInt(100)`, `DungeonRandInt(0x4000)`). Its 9 arguments match too. Details: [`re/combat.md`](re/combat.md).
  - **Damage constants against Red (2026-10-20, static):** P1 and P2 of #50 compared the damage formula's constants and the 24.8 and 48.16 rounding helpers with Red's `src/dungeon_damage.c` and `src/math.c`: no change found. The comparison with observed hits is still open. Details: [`re/combat.md`](re/combat.md#damage-formula).
  - **More shared tables (2026-10-14, Ghidra):** the pointers in `CalcDamage` and `sub_806F370_Async` lead to three more tables in overlay 1 with the same values as Red's `src/dungeon_config.c`, compared entry by entry: `gEffectivenessChart` (4 × 4 `s32`, `ov_0001::021AA588`), `gAtkStatStageMultipliers` and `gDefStatStageMultipliers` (21 × s24.8 each, `ov_0001::021AA660` and `021AA6B4`).
  - **Damage hits against the formula (2026-10-24, #185, runtime capture):** 14 `CalcDamage` hits from one scripted run. **14 of 14 match: 0 mismatches.** The formula is in [`re/combat.md`](re/combat.md#damage-formula).
    - Capture: [`combat_hits.py`](../verification/v7-trace/combat_hits.py) with [`combat_inputs.txt`](../verification/v7-trace/combat_inputs.txt), 9,000 turn frames from the first dungeon frame (24,960), retail `BlueRescueTeamEur` dump. Command: `SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy python verification/v7-trace/combat_hits.py <ROM> <OUT> --frames 9000`. Output goes to an ignored scratch folder, outside the repo.
    - Recompute: [`combat_recompute.py`](../verification/v7-trace/combat_recompute.py) `<OUT>`. It replays each hit from the captured arguments, the attacker and target info structs, and the RNG state. It checks the three RNG draws, `gDungeon` `+0x140` (power + attack), `+0x142` (defence), `+0x14A` (`r6`), `+0x154` (pre), `+0x150` (post), the `dmg_struct` damage and the crit flag.
    - SHA-256 of `hits.jsonl`: `c2af877c1464d7b15d774966f550ffb7e7a47bdae483e96dd5262cad47bc53e2`. Of `tables.json` (the RAM tables read at the first hit): `fcd5b09f6cdb022743afd3f7bf8d0a8b3dc62320ad486d612a5f3ff6073149e4`. Of `combat_inputs.txt`: `0b7d2c30a2b55763432da3b2ddb27a9ecd43182bdf402b57e48071ba8ea375e6`.
    - Repeatability: two runs of the same command gave the same `hits.jsonl`. SHA-256, run 1: `c2af877c1464d7b15d774966f550ffb7e7a47bdae483e96dd5262cad47bc53e2`. SHA-256, run 2 (2026-10-24): `c2af877c1464d7b15d774966f550ffb7e7a47bdae483e96dd5262cad47bc53e2`. Both recompute with 0 mismatches.
    - Output of the recompute (Atk and Def are the stats after the stage and multiplier step; `r6` is the level-or-attack term as an integer; the other columns are observed/recomputed):

    | # | Frame | Move | Type | Split | Atk | Def | r6 | Pre obs/calc | Post obs/calc | Dmg obs/calc | Checks |
    |---|---|---|---|---|---|---|---|---|---|---|---|
    | 1 | 1437 | 154 | 1 | 0 | 11 | 9 | 1 | 4/4 | 4/4 | 4/4 | match |
    | 2 | 1528 | 355 | 0 | 0 | 11 | 5 | 3 | 14/14 | 7/7 | 7/7 | match |
    | 3 | 1591 | 355 | 0 | 0 | 5 | 9 | 1 | 3/3 | 1/1 | 1/1 | match |
    | 4 | 1650 | 355 | 0 | 0 | 11 | 5 | 3 | 14/14 | 7/7 | 8/8 | match |
    | 5 | 3051 | 260 | 1 | 0 | 19 | 5 | 3 | 16/16 | 16/16 | 18/18 | match |
    | 6 | 4035 | 355 | 0 | 0 | 4 | 9 | 1 | 3/3 | 2/2 | 2/2 | match |
    | 7 | 4088 | 355 | 0 | 0 | 11 | 4 | 3 | 15/15 | 8/8 | 8/8 | match |
    | 8 | 4157 | 355 | 0 | 0 | 12 | 4 | 3 | 16/16 | 8/8 | 9/9 | match |
    | 9 | 4798 | 355 | 0 | 0 | 11 | 5 | 3 | 14/14 | 7/7 | 6/6 | match |
    | 10 | 4887 | 355 | 0 | 0 | 12 | 5 | 3 | 14/14 | 7/7 | 8/8 | match |
    | 11 | 6558 | 355 | 0 | 0 | 11 | 4 | 3 | 15/15 | 8/8 | 8/8 | match |
    | 12 | 6645 | 355 | 0 | 0 | 12 | 4 | 3 | 16/16 | 8/8 | 7/7 | match |
    | 13 | 7438 | 355 | 0 | 0 | 11 | 4 | 3 | 15/15 | 8/8 | 8/8 | match |
    | 14 | 7527 | 355 | 0 | 0 | 12 | 4 | 3 | 16/16 | 8/8 | 8/8 | match |

    - **Type multiplier.** The type-effectiveness step (`sub_806E100`) is not modelled. It is fixed at 1.0. The 14 hits allow only the range [0.9868, 1.0025), which contains 1.0, so the hits show no type effect. The chart is not read by the script; its values are not written here ([`legal.md`](legal.md)).
    - **Correction to step 10.** The level-or-attack term must be fixed point, as in the code's 48.16 arithmetic (`level × 0xAAAA`, and `attack / 3` as a 48.16 divide). The truncated integer `r6` (`level × 2 / 3`, `attack / 3`) gives 14 of 14 mismatches with the same inputs.
    - **Checked in the captured structs, not modelled:** no held item in any of the 14 hits. Abilities present (IDs `0x10`, `0x1E`, `0x46`, `0x4A`, `0x4C`) are not in the ability table above, and none is Battle Armor or Shell Armor. Defence stage is 10 (neutral) in all 14 at the assumed offset (`+0x20`, see [`re/structs.md`](re/structs.md)). Status effects and weather were not checked.
    - **Not exercised by these hits:** a critical hit (no roll passed its odds, and the crit flag is 0 in all 14). The stage table is used with an attacker at stage 9 (frame 4035), which matches. `arg8` takes two values, `0x100` (moves 154 and 260) and `0x80` (move 355), so its 1.0 = `0x100` scale is checked at both.
    - **Ordering not distinguished.** `pre` (`+0x154`) is written as the base before the type and crit multipliers. With type 1.0 and no crit the hits cannot tell this apart from the other order.
    - **Result:** **confirmed for these 14 hits.** The fixed-point formula in [`re/combat.md`](re/combat.md#damage-formula) reproduces each captured hit: its working values, the damage the call wrote, and the crit flag. The issue's condition, that the formula matches observed hits, holds for this set. Crit, held items, abilities, status and type effects are still unchecked (see the list above).
    - **Fallback:** none.
    - **Notes:** A first capture (2026-10-23, 14 hits from a scratch script not in the repo) could not be recomputed because the formula was blocked by missing values; those values are now read from the dump or derived from the captured fields. The earlier record is replaced by this entry.
- **Outcome:** **confirmed.** 4 tables are shared by value (type chart, effectiveness combination, attack and defense stat-stage multipliers), meeting the "3+ shared tables" criterion, and both routine constants match (RNG step, damage calc). `gTypeGummiIQBoost` and `gUnknown_810A808` are still not found by value.
- **Fallback adopted:** none. Red stays the map for Phase 3.
- **Notes:** Still open: where the two Gummi tables went in Blue (moved, re-laid out, or replaced). The Red dump is not the decomp's base ROM (see above).

---

## V7 — Scriptable DS emulator for RAM and RNG dumps

- **Assumption:** a DS emulator can be scripted to dump ARM9 memory and RNG state per turn (DeSmuME Lua, BizHawk + melonDS core, or melonDS GDB stub).
- **Date:** 2026-10-13
- **Environment:** Linux (Ubuntu 24.04), Python 3.13. `desmume-cli` 0.9.11 from apt. `py-desmume` 0.0.9 from PyPI (bundles libdesmume 0.9.12; wheels for Linux, Windows and macOS). CI: `ubuntu-latest` and `windows-latest`, job `v7` in `.github/workflows/verify.yml` (the workflow file was removed in `320b68d`; the link was broken, see #161).
- **Evidence:**
  - Retail ROM not available in this session, so the check uses a synthetic ROM built by [`make_test_rom.py`](../verification/v7-trace/make_test_rom.py) (source [`test_rom.s`](../verification/v7-trace/test_rom.s), no retail code). Each frame its ARM9 code bumps a counter, mixes `KEYINPUT` into an LCG (stand-in for the game RNG) and stores both at `0x02100000`. The instruction at `0x02000044` runs once per frame, right after the update, standing in for the turn-advance routine.
  - **Candidates tried:**

    | Candidate | Result on Linux |
    |-----------|-----------------|
    | `desmume-cli` 0.9.11 (apt) + Lua | No Lua in this build (links SDL 1.2 only, no Lua strings). |
    | `desmume-cli` 0.9.11 ARM9 GDB stub (`--arm9gdb=PORT`, `--cpu-mode=0`) | Works: [`gdb_dump.py`](../verification/v7-trace/gdb_dump.py) speaks GDB RSP, sets `Z0` at the trigger, reads memory with `m`, steps over the breakpoint. 120 consecutive frames dumped, counter 1..120 with no gaps. Slow: about 4 hits/s. No scripted input: `--play-movie` and `--record-movie` are parsed but ignored (strace shows the file is never opened). |
    | `py-desmume` 0.0.9 | Works, chosen. Python API over libdesmume: savestate save/load, `keypad_update` per frame, `register_exec` hooks, memory reads. Headless with `SDL_VIDEODRIVER=dummy`. |
    | BizHawk + melonDS core | Not tried. |

  - **Script:** [`pydesmume_dump.py`](../verification/v7-trace/pydesmume_dump.py) loads the ROM and a savestate, plays an [input file](../verification/v7-trace/inputs.txt) (`FRAME KEY...` lines), and appends `LEN` bytes at `ADDR` to a file on each exec-trigger hit or at the end of each frame. [`run_v7.sh`](../verification/v7-trace/run_v7.sh) runs the whole check.
  - **Repeatability** (`run_v7.sh`, savestate taken at frame 30, then 600 frames):
    - 16-byte state block on the exec trigger, 3 runs: 600 dumps each, all sha256 `ec2531b0…bc45a`. Frame counter 31..630, no gaps. `KEYINPUT` changes land on the scripted frames (A at +10, A+Right at +20, release at +40, Start at +50).
    - All 4 MiB of main RAM at the end of every frame, 60 frames, 3 runs: all sha256 `01086505…19ca6`.
    - Negative control, different inputs: sha256 `c13cf7e6…78574`, so inputs reach the game state.
  - **Speed** (unthrottled, interpreter, shared container, so noisy): ~70–90 frames/s with no dump or a 16-byte dump; ~20–50 frames/s dumping 4 MiB per frame. py-desmume reads one byte per ctypes call (seconds per 4 MiB), so the script reads main RAM straight from libdesmume's exported `MMU` struct (`MAIN_MEM` at offset `0xC000`) after checking it against the per-byte API.
  - **Retail run (2026-10-14, Windows 11, Python 3.12, py-desmume 0.0.9 / libdesmume 0.9.12, interpreter, supported dump):** [`retail_trace.py`](../verification/v7-trace/retail_trace.py) boots the ROM with no save file and no savestate, replays [`boot_to_dungeon.txt`](../verification/v7-trace/boot_to_dungeon.txt) (1,678 input changes: intro, new game, the personality quiz answered with the first option each time, default partner, one-letter player name) and reaches the first dungeon floor at frame 24,960, when the `gDungeon` pointer becomes non-zero. It then plays [`turn_inputs.txt`](../verification/v7-trace/turn_inputs.txt) (A every 30 frames) for 3,000 frames and hooks `RunFractionalTurn_Async` (`0x021411BC`).
    - 256 hits; `fractionalTurn` takes every value 0–23. Each hit writes one JSON line with documented fields only: RNG states, seeds, turn counter, team and wild Pokémon (species, position, level, HP, speed stage) and a hash of the tile grid.
    - 3 runs from power-on (and runs from savestates taken along the way): identical output, sha256 `1dfa3860…0555f3`.
    - Checks against [`re/rng.md`](re/rng.md), done by the script: in all 255 consecutive pairs, the dungeon RNG state is reachable from the previous one with `s * 0x5D588B65 + 1` (0–19 steps per fractional turn) and so is the global one (up to 672 steps). The first floor seed equals `GenerateDungeonRNGSeed` applied once to `(runSeed | 1) & 0xFFFFFF`, the preseed state after it matches, and the dungeon RNG state is 1,812 steps after `InitDungeonRNG(floorSeed)` (floor generation).
    - Speed: about 40 s for the whole run (boot replay plus trace) on a desktop PC.
    - The same replay drives [`floor_oracle.py`](../verification/v7-trace/floor_oracle.py), which forces the dungeon RNG at `GenerateFloor` and shows that the floor depends only on that state (results in [`re/dungeon-generation.md`](re/dungeon-generation.md#emulator-check)).
- **Outcome:** confirmed. A scripted DeSmuME run dumps the dungeon state once per turn, byte-identically across runs, on the retail ROM. The synthetic-ROM job (`run_v7.sh`, which fails on any dump mismatch) passes on `ubuntu-latest` and `windows-latest` (CI run [38763275793](https://github.com/TorratDev/rust-mm/actions/runs/38763275793), 2026-10-15).
- **Fallback adopted:** none. The melonDS GDB stub stays the fallback; the DeSmuME GDB stub shows that route works but is slow and has no input playback.
- **Notes:**
  - Setup quirks: libdesmume needs a video driver even headless (`SDL_VIDEODRIVER=dummy`); on Linux it needs `libGL` and `glib` from the system. The apt `desmume-cli` defaults to the JIT; the GDB stub run used `--cpu-mode=0`. py-desmume ran the interpreter.
  - Save files: the retail run starts from power-on with no save file (libdesmume keeps the backup in RAM), so it needs neither a savestate nor a save. The emulator's built-in firmware has the fixed MAC address `00:09:BF:12:34:56`, which seeds the global RNG ([`re/rng.md`](re/rng.md)); a real console would give a different run seed.
  - On Windows, libdesmume does not export its `MMU` symbol, so `retail_trace.py` reads RAM through py-desmume's per-byte API, and `DeSmuME.destroy()` ends the Python process: write all output before calling it.
  - Per-turn trigger for the retail run ([`re/turns.md`](re/turns.md)): the entry of `RunFractionalTurn_Async`, `0x021411BC`, counted only once overlay 1 is loaded (overlays 0 and 7 share that RAM). Dump `fractionalTurn` at `dungeon+0x650` with the RNG state; the dungeon struct pointer is in `gDungeon` at `0x020E0788`.
  - RNG state to dump on the retail run ([`re/rng.md`](re/rng.md)): `sPRNGState` at `0x020DF704` and `gDungeonRngState` at `0x021B3F58` (overlay 1 loaded), plus the dungeon struct fields `+0x66C` (run preseed) and `+0x670` (floor seed). Both generators advance during a dungeon. The global LCG is seeded at boot from the console MAC address in the firmware, so keep the emulator firmware fixed between runs.

---

## V8 — Ghidra loads ARM9 and overlays at real addresses

- **Assumption:** Ghidra can load the ARM9 binary and its overlays at their real addresses, through an existing NDS loader or a custom script.
- **Date:** 2026-10-14 (tooling, synthetic ROM; retail run on Windows).
- **Environment:** Linux sandbox, Ghidra 11.4.3 (`ghidra_11.4.3_PUBLIC_20251203.zip`; the GitHub release download now works from the session, the earlier HTTP 403 is gone), OpenJDK 21.0.12, Python 3.13. No retail dump in the session.
- **Evidence:**
  - Took the "If false" route (a script of our own); no NDS loader extension tried yet. Setup and usage: [`ghidra-setup.md`](ghidra-setup.md).
  - [`tools/nds_extract_code.py`](../tools/nds_extract_code.py) checks the dump's SHA-1 and writes `arm9.bin` (BLZ-decoded if compressed), every ARM9 overlay (BLZ-decoded when flag bit 24 is set) and a `manifest.json` memory map (module params, static BSS, autoload sections, overlay RAM ranges, BSS, static initialisers). Unit tests on synthetic ROMs in [`tools/tests`](../tools/tests); its BLZ decoder agrees with `ndspy` in both directions.
  - [`tools/ghidra/RescueNdsImport.java`](../tools/ghidra/RescueNdsImport.java) maps ARM9 static + BSS, the autoload sections at their destinations, and each overlay as its own Ghidra overlay block (plus its BSS in the same overlay space), with language `ARM:LE:32:v5t`. Headless wrappers: [`import.sh`](../tools/ghidra/import.sh), [`import.ps1`](../tools/ghidra/import.ps1).
  - Synthetic end-to-end run (`tools/tests/synth_nds.py --compress-arm9`, then `import.sh ... --check ov_0000+0x14 ov_0001+0x14 ov_0002+0x14 arm9+0x20`): 12 blocks. Two overlays share `0x02100000` as `ov_0000::02100000` and `ov_0001::02100000`. Calls from all three overlays to `0x02000020` resolve to the ARM9 function. Each overlay's literal-pool pointer resolves inside its own overlay space (`from ov_0001::02100008 DATA in ov_0001_start`).
  - **Retail run** (Windows 11, Ghidra 12.1.4, JDK 25.0.4, headless import wrapper (log `rescue-arm9-import.log`), supported dump): `RESCUE: mapped arm9 + 145 overlays, 154 blocks`, ROM SHA-1 `6f01887356268de39085df01490347bbcc3aa330`, no overlap warning. Auto-analysis took 146 s. `RescueNdsReport.java` (run through ghidra-cli, see [`ghidra-setup.md`](ghidra-setup.md#6-ghidra-cli)) counts 5,236 functions: 2,988 in `arm9`, 1,741 in `ov_0001`, 365 in `ov_0000`, 95 in `ov_0007`, 1 in `ov_0002`, and **none in the other 140 overlays**. Auto-analysis only followed code reachable from entry points it knew (the ARM9 entry and the overlays' static initialisers).
  - Cross-references resolve inside overlays (for example 175 calls to `DungeonRandInt` within `ov_0001`) and from overlays into the ARM9. Calls from the ARM9 into an overlay resolve to the `ram` address, as expected; [`RescueCallers.java`](../tools/ghidra/RescueCallers.java) collects those too.
  - **Only 3 overlays hold code.** A byte census of every decoded overlay (share of 32-bit words with the ARM "always" condition, count of `stmdb sp!, {..., lr}` prologues, count of Thumb `push {..., lr}`) puts overlays 0, 1 and 7 at 74–83 % ARM words with 83–1,484 prologues. The other 142 have under 3 % ARM words, no ARM prologues and almost no Thumb prologues. Their contents are pointer tables followed by text or records: data, not unanalysed code. Overlays 2–6 load at the same address (`0x021B4900`) with similar sizes, likely one text table per EUR language. The single "function" in `ov_0002`, and the bad instructions at `ov_0002::021b4908` and `ov_0004::021be5ec`, come from static-initialiser entries in these data overlays being disassembled.
  - **Code discovery** ([`RescueOverlayDiscover.java`](../tools/ghidra/RescueOverlayDiscover.java), run on overlays 0, 1 and 7): 112 more functions from ARM prologues and from code following function returns (`ov_0000` 365 → 446, `ov_0001` 1,741 → 1,759, `ov_0007` 95 → 108), with no new bad instructions. Sampled new functions are real code, probably reached only through function pointers. It also typed 3,621 literal-pool words as pointers (842 / 2,731 / 48): analysis had left them as `undefined4`, so no reference reached their targets. Total after re-analysis: 5,392 functions (the ARM9 also gained 44, 2,988 → 3,032, from references the new code exposed). A second run changes nothing.
  - **Type-chart spot check:** before the pool step, `references to ov_0001::021aaee8 (ov_0001+0x72688): 0`. After it: **4**, from the literal pools `ov_0001::0217bb1c`, `0217bc9c`, `0217ce7c` and `0217d5c8`. They are read by `FUN_ov_0001__0217ba94`, `0217bb94`, `0217cb3c` and `0217ce88`, which index the table as `s16` with a row stride of `0x24` (18 × 2 bytes), the attack type selecting the row and a defender type byte (entity `+0x56`) the column. The chart is labelled `gTypeEffectivenessChart`.
- **Outcome:** **confirmed.** ARM9 and all 145 overlays are mapped at their real addresses. Cross-references resolve within overlays, from overlays into the ARM9, and from code to the type chart. The 142 overlays without functions are data.
- **Fallback adopted:** none (the script is the roadmap fallback).
- **Notes:** After importing, run `RescueOverlayDiscover.java -- all ov_0000 ov_0001 ov_0007` ([`ghidra-setup.md`](ghidra-setup.md#2c-find-the-rest-of-the-overlay-code)). The GUI path uses the same scripts but has not been exercised. Calls from ARM9 code into overlay ranges cannot resolve statically (Ghidra cannot know which overlay is loaded); that is expected.

---

## V9 — REA workflow on ARM DS binaries

- **Assumption:** REA supports the planned workflow with Ghidra on ARM DS binaries: function search, xrefs, decompiled-routine summaries and evidence notes.
- **Date:** 2026-10-13 (README review); 2026-10-15 (runs).
- **Environment:** Windows 11. REA `rea-agents` 6.5.0 (npm, run through `npx`), Node.js from `C:\Program Files\nodejs`. Ghidra 12.1.4, JDK 25.0.4. [ghidra-cli](https://github.com/akiselev/ghidra-cli) 0.2.2. The V8 project (`rescue-arm9`: retail ARM9 + 145 overlays, after `RescueOverlayDiscover`, `RescueRngLabels` and `RescueApplySymbols`).
- **Evidence:**
  - **REA.** `rea doctor` finds Ghidra 12.1.4 and its headless launcher, and needs `JAVA_HOME` on a JDK 21 or newer (JDK 17 is rejected). With that set, `rea function overlay_0001.bin 0x0217ba94 --target-format raw-binary --architecture arm --entry-point 0x02138860 --provider ghidra` (decoded overlay 1 at its RAM address) fails before Ghidra starts: `target_format_unsupported`, "Windows Ghidra P0 accepts native PE and explicitly selected raw 6502 targets only." Hopper is macOS-only, and IDA needs an IDA install. REA's native commands take a binary path and import it themselves; nothing in REA opens an existing `.gpr` project. So even on Linux, where raw ARM is accepted, REA would see one raw image at one base address, without the overlay blocks, the autoloads or the symbols V8 applies, and without xrefs between overlays and the ARM9.
  - **ghidra-cli on the V8 project** (one bridge, queries in under a second each):

    | Capability | Command | Result |
    |------------|---------|--------|
    | Function search | `find function DungeonRand`, `find function RunFractionalTurn` | 5 matches in `ov_0001` (`DungeonRandInt` at `ov_0001::0219d058`, ...); `RunFractionalTurn_Async` at `ov_0001::021411bc` |
    | Xrefs to data | `x-ref to ov_0001::021aaee8` (type chart) | the 4 literal-pool references found in V8 |
    | Callers | `function x-refs ov_0001::0217ba94` | 3 callers in `ov_0001` |
    | Callees | `disasm ov_0001::0217ba94` | `bl 0x02180644` (`MonsterIsType`); `function calls` lists callers, not callees, so use `disasm` or `x-ref from` |
    | Xrefs into the ARM9 | `x-ref to 0x020df704` (`sPRNGState`) | reads and writes from `SetRNGSeed` and `GetRNGState`; overlay → ARM9 calls as in V8 |
    | Decompile, ARM | `decompile ov_0001::0217ba94` | readable C: checks `MonsterIsType(pokemon, 14)`, then reads `gTypeEffectivenessChart` as `s16` at `type * 0x24 + defenderType * 2` for both defender types |
    | Decompile, Thumb | `disasm 02000678`, `decompile 02000678` | `svc 3; bx lr` and `software_interrupt(3)` |
    | Evidence notes | `comment set/get/list`, `tag add/get/delete` | plate comment and function tag written, read back, listed as JSON, then removed |

  - **Thumb share.** A scratch script counting functions with `TMode = 1` finds 2 Thumb functions, both in the ARM9 (`0x02000246`, `0x02000678`). All of `ov_0000` (446), `ov_0001` (1,759) and `ov_0007` (108) is ARM, as are 3,076 ARM9 functions.
- **Outcome:** **replaced.** REA does not support this workflow: on Windows its Ghidra provider rejects ARM targets, and on any OS it imports a binary itself instead of opening the overlay-mapped project. ghidra-cli on the V8 project supports all four capabilities, on ARM and Thumb code.
- **Fallback adopted:** ghidra-cli plus Ghidra scripts ([`tools/ghidra`](../tools/ghidra)), as in [`ghidra-setup.md`](ghidra-setup.md#6-ghidra-cli). Evidence notes live in the repo as `docs/re/*.md` (addresses, xrefs, and emulator checks marked separately from guesses, as in [`re/rng.md`](re/rng.md)); Ghidra comments and tags mirror them in the project and can be exported with `comment list --json`.
- **Notes:** Look at REA again if a release adds ARM raw images on Windows or opening an existing Ghidra project. Its Linux path was not run.

---

## #42 — Ghidra project acceptance

Checks the epic #5 tasks of [#42](https://github.com/TorratDev/rust-mm/issues/42) against `origin/main` (`b14fa59`). Not a V-check, but it records what the shared project does and does not provide.

- **Assumption:** the shared `rescue-arm9` project holds the ARM9 as `ARM:LE:32:v5t`, all 145 overlays at their real addresses, the labelled I/O registers and SDK functions, and only scripts are committed.
- **Date:** 2026-10-19 (first attempt the same day; revision after the base project was found missing).
- **Environment:** Windows 11, Ghidra 12.1.4 (`C:\Users\Troll\Downloads\ghidra_12.1.4_PUBLIC_20260921`), Eclipse Temurin JDK 25.0.4, ghidra-cli 0.2.2 (`C:\Users\Troll\tools\ghidra-cli\ghidra.exe`). The ghidra-cli config's `ghidra_project_dir` points at the main checkout, so every command passes `--projects-dir` instead of changing the config.
- **Project checked:** `rescue-arm9-import-a.log` records project `rescue-arm9` (log lines 41–44 and 409). `rescue-arm9-a`, `-b` and `-c` are copies with identical logs (same MD5 and mtime). The checked copy is `rescue-arm9-a` in `C:\Users\Troll\Documents\github\rust-mm\rescue-re\ghidra` (`rescue-arm9-a.gpr`, 2026-10-14 14:43, `Save succeeded` and `Import succeeded`, `RESCUE: mapped arm9 + 145 overlays, 154 blocks`, ROM SHA-1 `6f01887356268de39085df01490347bbcc3aa330`).
- **Base project `rescue-arm9` is missing.** `C:\Users\Troll\rescue-re\ghidra\rescue-arm9.gpr` is 0 bytes and `rescue-arm9.rep/idata/00/` is absent, so no program is saved there. It must be rebuilt with [`tools/ghidra/import.ps1`](../tools/ghidra/import.ps1) (`-CodeDir`, `-ProjectDir`, `-Check ov_0001+0x72688`; [`ghidra-setup.md`](ghidra-setup.md#2a-build-the-project-headless-recommended)). The 13:35 out-of-memory import was in `C:\Users\Troll\rescue-re\ghidra`. The 14:43 import that succeeded was in `rust-mm\rescue-re\ghidra` and is no longer there.
- **Evidence (live, read only).** Each command was run through the bridge on `rescue-arm9-a`, program `arm9.bin`. Stored under the scratchpad, not in the repo:
  - `ghidra --projects-dir 'C:\Users\Troll\Documents\github\rust-mm\rescue-re\ghidra' --project rescue-arm9-a --program arm9.bin start` → `Bridge is running` (after about 2 minutes).
  - `... summary` → `"executable_format":"Raw Binary","function_count":5394,"image_base":"02000000","language":"ARM/little/32/v5t","max_address":"ov_0144::0215d89f","min_address":"01ff8000"`.
  - `... memory map` → 6 RAM blocks (`autoload_0_01FF8000`, `arm9`, `arm9_bss`, `autoload_1_027E0000`, `io` at `04000000`, `io_ipc` at `04100000`) and 145 overlay blocks named `ov_NNNN` (the count of names matching `"name":"ov_NNNN"`, without `_bss`), each with its `_bss` block.
  - `... symbol list --limit 0 --filter 'name~REG_'` → 78 labels, all `source: IMPORTED`, from `REG_DISPCNT` at `04000000` (matches the 78 entries in `RescueNdsImport.java`).
  - `... x-ref to 'ov_0001::021aaee8'` (`ov_0001` base `02138860` + `0x72688`) → 4 `DATA` references, from `ov_0001::0217bb1c`, `0217bc9c`, `0217ce7c` and `0217d5c8`. Same as [V8](#v8--ghidra-loads-arm9-and-overlays-at-real-addresses).
  - `... symbol get FS_LoadOverlayInfo` → `0207bed8`, `USER_DEFINED`; `FS_StartOverlay` → `0207bc9c`, `USER_DEFINED`.
  - `symbols.md` against the project's full symbol list (`symbol list --limit 0`, 6.4 MB, read into the scratchpad): 218 data rows; 198 present by name. **20 are not applied:** still `FUN_*` in the project. They are `GetDefaultMonsterAnimId`, `SetDefaultMonsterAnim`, `SetMonsterAnim`, `GetMonsterSpriteHandle`, `SetMonsterAnimAndDir`, `SetMonsterAnimAndDirIfChanged`, `PlayMoveAttackerAnim`, `PlayMoveAnim`, `WaitForOneShotAnimEnd`, `UpdateMonsterSprite`, `SetAnimForced` (all in `ov_0001`), and `GetMoveAnimId` (`02023E9C`), `IsOneShotAnim` (`02002438`), `GetMoveEntry` (`02025674`), `GetMoveEntry2` (`02025688`), `MoveHasTypeFourAt` (`020241C0`), `MoveHasTypeFour` (`02024238`), `GetMoveSfxId` (`020242D4`), `InitSpriteState` (`02012848`), `GetPokemonOverworldPalette` (`020020A0`). Most come from [`symbols.md`](symbols.md) rows added after the project was imported (for example `ab93ba2`, #133). `RescueApplySymbols.java` was not run, because this check is read only.
  - `git ls-files | grep -iE '\.(nds|gpr|gba)$|\.rep/'` → nothing. `git ls-files rescue-re` → 147 ROM-derived files (see task 4).
- **Side effect on `rescue-arm9-a`.** Read-only queries still touched the project. The bridge rewrote `rescue-arm9-a.rep/idata/~index.{dat,bak}`, `versioned/~index.{dat,bak}` and `idata/00/~00000000.db/db.39.gbf` (15:30, when the bridge stopped; the earlier `db.38.gbf` is from 2026-10-18). No `save` was run. I did not restore the files, and I did not check their content before and after.
- **Heap.** The 2026-10-14 13:35 import (`rescue-arm9-import.log`) hit `OutOfMemoryError` in analysis and saved nothing. The 14:43 import (project `rescue-arm9`, copied later to `rescue-arm9-a`) used the same script and finished with no out-of-memory error (`Post-analysis succeeded`, `Save succeeded`, zero `ERROR` lines). Neither log records the heap size, so the cause is not confirmed. Documented in [`ghidra-setup.md`](ghidra-setup.md#2a-build-the-project-headless-recommended) as a setting to try: `GHIDRA_MAXMEM`.
- **Task outcomes** (live results on `rescue-arm9-a`; base project missing):

  | # | Task | Outcome | Why |
  |---|------|---------|-----|
  | 1 | ARM9 loaded as ARMv5TE ARM/Thumb | **met** (on `rescue-arm9-a`) | `summary` gives `ARM/little/32/v5t`. Thumb coverage is in V9. The base project still has to be rebuilt. |
  | 2 | 145 overlays mapped at real addresses | **met** (on `rescue-arm9-a`) | 145 overlay blocks, mapped at their `ov_0000::02138860`-style addresses. The import log records `RescueNdsImport.java` output, but not which wrapper ran. The base project is missing. |
  | 3 | `REG_*` and SDK functions labelled | **partial** | 78 `REG_*` labels are present. `FS_LoadOverlayInfo` and `FS_StartOverlay` are labelled. 20 of 218 `symbols.md` names are not applied. |
  | 4 | Only scripts committed | **gap** (kept) | No `.gpr`, `.rep`, `.nds` or `.gba` is tracked. `rescue-re/code/` tracks `arm9.bin`, `manifest.json` and `overlay_0000.bin` … `overlay_0144.bin`, all ROM-derived, added in `84e97e3`. They are not in `.gitignore`. They stay in place until the maintainer decides. |
  | 5 | Symbol naming and sharing agreed | **partial** | `symbols.md` names `RescueApplySymbols.java`. The table is 20 rows ahead of the project. "Agreed" is a decision this check cannot verify. |

  The type-chart xref check (`ov_0001::021aaee8`) is **met**: 4 references, as in V8.
- **Fallback adopted:** none.
- **Notes:**
  - **To close #42:** rebuild `rescue-arm9` with `import.ps1` (heap set with `GHIDRA_MAXMEM` if it runs out of memory). Rerun `RescueNdsReport.java`, then `RescueApplySymbols.java`, and check the 20 missing names.
  - **Gap for the maintainer:** `rescue-re/code/` (147 files) is tracked on `origin/main`. Per [`CONTRIBUTING.md`](../CONTRIBUTING.md), they are reported here and not removed. Confirm they are ROM-derived, then remove them from the index and add `rescue-re/code/` to `.gitignore`. They stay in history unless history is rewritten.

---

## P5 — Per-frame RNG advance (#44)

- **Assumption:** the global LCG (`sPRNGState`, `020DF704`) advances once per frame, as Red's `WaitForNextFrameAndAdvanceRNG` does. This is the check that [`re/execution-model.md`](re/execution-model.md#rng-per-frame) cited as #17; the static note it replaces said the opposite.
- **Date:** not recorded in the investigation findings.
- **Environment:** py-desmume (DeSmuME). ROM `pmdb.nds` in the repository root, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (the supported dump), not committed. Addresses checked in Ghidra copy `rescue-arm9-b`.
- **Command:** not recorded in the investigation. The script `rng_per_frame.py` is a scratch file in the scratchpad and is not committed. It hooks `MainLoops_RunFrameActions` (`0201F268`) and `Rand16Bit` (`020119A4`), and checks each interval of `sPRNGState` against the steps counted. Inputs: [`verification/v7-trace/boot_to_dungeon.txt`](../verification/v7-trace/boot_to_dungeon.txt), with extra A and Select windows appended after frame 2600 (scratch copy).
- **Evidence:**
  - The frame routine fires once per emulated frame from emulated frame 2418. Frame 2419 fires twice.
  - 122 intervals: 117 show exactly 4 LCG steps. The others show 2, 8, 36 and 56.
  - State chain: 0 mismatches across all intervals.
  - Sprite routine `FUN_02012648`: 0 calls in every interval, so it does not explain the 4 steps.
  - A held: 7 intervals, with 2, 4, 8 and 56 steps. Too few and mixed, so no A effect is shown.
  - Select: no frame-routine hits after emulated frame 2508, so there is no Select data.
  - The mode during the run (title or ground) is not identified.
  - Repeatability: see the repeat run below.
  - **Repeat run (#159 P1, 2026-10-26).** [`verification/v7-trace/rng_per_frame.py`](../verification/v7-trace/rng_per_frame.py) replaces the scratch script. It was run twice from the same save state at the first dungeon floor, with the windows idle / A held / idle / Select held / idle (1500 frames). Command: `python verification/v7-trace/rng_per_frame.py <ROM> <OUT> --state <STATE>`, with `SDL_VIDEODRIVER=dummy` and `SDL_AUDIODRIVER=dummy`. `<STATE>` is made once with [`scripts/capture_golden.py`](../scripts/capture_golden.py) `--make-state`. Both runs gave identical files: sha256 of each `ab4b7e223e14bc4025dc7e44cd6e2b5cf6c04242031596146b8081a317f60837`. Per-frame steps in that output: 4 steps in 1287 intervals, 6 in 159, 2 in 26, 8 in 21, and 12, 14, 16 and 28 once each; 2 intervals match no count up to 50 steps. The frame routine has 0 hits in the dungeon, as noted above. These are not the boot-run hashes earlier in this entry: the boot-only and window hashes from the scratch script were not reproduced by this script.
- **Boot-phase A and Select (#159 P4, 2026-11-04).** Command: `python verification/v7-trace/rng_per_frame.py <ROM> <OUT> --mode boot-hold --hold none|A|SELECT` (2600 frames from power-on; the held button is added over frames 2418–2561, where the frame routine runs). Runs: `--hold none` four times, all `62e333fe61e150e89c02cd930d04d0dcb1553ac8df04820c52c2957d738ab183`; `--hold A` twice, both `a5a93d52c938bf21026d7e8a8a6f75db6c56ee597eaa2aeb541c2e011dc8cc3e`; `--hold SELECT` twice, both `5d6472f5e2752d18c38684db85aa36efdf0052c0c9926eada81942b4f149f269`. In the hold window (144 frames) the frame routine has 123 hits on 122 frames with every setting. Steps per interval in the window (count: intervals): none 2: 2, 4: 142; A 2: 28, 4: 116; SELECT 2: 9, 4: 135. Before the window the histogram is the same in all three runs. With A held, the frame routine also hits on frames 2562–2599 (38 hits, one per frame); with none or SELECT it does not.
- **Outcome:** **measured (medium confidence); repeated from a save state (#159 P1), not on a second OS or in CI:** the global LCG advances 4 steps per emulated frame, with no button held. The earlier static note is superseded. Not established: the caller slot of `FUN_02018ACC` (the handler that makes the steps, see [`re/rng.md`](re/rng.md#per-frame-steps-159)). The repeat run (two runs from one save state, both `ab4b7e223e14bc4025dc7e44cd6e2b5cf6c04242031596146b8081a317f60837`) covers the dungeon windows only. **A and Select in the boot frame-loop phase (#159 P4):** measured (medium confidence, one dump, one emulator): holding A changes the step count inside the window (2-step intervals 2 → 28) and the frame routine keeps running for 38 frames after release; holding Select changes the window's step count (2 → 9) and nothing after it. Both runs of each setting gave the same hash. Not established: which handler reads A or Select, and whether the 38 extra frames come from the game's phase or from the input path.
- **Two-step intervals (#159 P3, runtime, repeated).** The 2-step intervals are real, not a hook artifact. Two runs of the 3000-frame plain probe from power-on were byte-identical (scratch `rng_dungeon.py`, not committed; sha256 `4758e33e74ab1c5e423a3b786291d2ba253a336b9f8dd0999332f7ea82720c65`; command not recorded in the findings). Of the 1174 two-step frames, 1172 lie outside the frame-routine window counted from frame 21. The double frame-routine hit at frame 2419 advances 4 steps, like its neighbours. The committed script does not reproduce these hashes (see Repeat run above). Details: [`re/rng.md`](re/rng.md#per-frame-steps-159).
- **Fallback adopted:** none.
- **Notes:** Next: hook `Rand32Bit` (`0201198C`) and its callers on the frame path, rerun the same script.

---

## #197 — Save load and session runs

- **Assumption:** the save-load path, the state machine that follows it, and the session struct pointer behave on a running game as the static decompiles in [`formats/save.md`](formats/save.md) say.
- **Date:** 2026-11-04 (blank-card run). The other cases come from the investigation findings; their dates are not recorded.
- **Environment:** Windows 11, Python 3.12.10, py-desmume (DeSmuME). ROM `pmdb.nds` at the repository root, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (the supported dump, not committed). Addresses checked in Ghidra copies `rescue-arm9-a`, `-b` and `-c`.
- **Evidence:**
  - **Blank card (run, 2026-11-04):** `python verification/v7-trace/save_load_probe.py <ROM> <OUT> --card none --frames 3000`. Output sha256 `e9ebbe3279a5b88b408c092687725e6fc1ae1605732bc6a82eeefc88b95843bf` (scratch, not committed). Both copies return status 2; the boot read result is 2; the state goes 2 → 4 → 6 at frame 2473 with no message. Globals at the end: size `0x40000`, unit `0x1000`, session pointer `0x0`, `unk418` `-1`, alternate-device flag `0x0`.
  - **Injected zeroed copy A (findings run, `zero_reads`):** zeros written at the `ReadSaveSector` return. Copy A status 0 and magic 0, so `ReadSaveFromPak` returns 4, and copy B is not read. Fault injection, not a real card.
  - **Forced status 1 (findings run, `force1`):** r0 set to 1 on both copies. State 5 from frame 2459; 1,543 more calls to frame 4000, with no exit. Fault injection.
  - **Zeroed card file import:** `backup.import_file` did not reach the game in this build (DeSmuME logged "Will operate in RAM"). Not a result.
  - **Boot to dungeon (findings run, `save_session_probe.py`, 24,960 frames):** `FUN_020660E4` and `FUN_02065F88` never fired; the session pointer was 0 at the dungeon. The device detect `FUN_0201D638` ran once at frame 52 and found no device (flag `0x020F38A0` = 0). Quick-save sectors: reads at 0, 6 and 31, and erase writes at 31 (frames 2460 and 2563); `WriteSavetoPak` did not fire. Command: `python verification/v7-trace/save_session_probe.py <ROM> <OUT>`. The script was compiled in this entry but not re-run.
  - Fault injection overwrites a return code or a buffer. It shows what the code does with a given input, not what a real card produces.
- **Outcome:** partial. Confirmed at runtime: the blank-card load path, the size and unit globals, the state-5 stall given status 1, and the absence of a device on the boot path. Open: valid-save cases, the zeroed real-card behaviour, the real trigger of status 1, and the session pointer in other scenes.
- **Fallback adopted:** none.
- **Notes:** The two probes are the repository copies of the findings' `p1_probe.py` and `p7_probe.py`, with paths made relative to the script. They take the ROM as an argument and refuse any SHA-1 other than the supported one. Both pass `python -I -m py_compile`. They must run without `-I`, which hides the user-installed `desmume` module. Still open, as listed in [`formats/save.md`](formats/save.md#4-open-runtime-checks): P1.5, P2c, P3.7, P5 unlock meanings, P6.4, P7.3, P7.5, P7.7, P8, and X.1.

## #239 — Save format: remaining findings

- **Assumption:** the remaining #239 save-format findings match the code paths in [`formats/save.md`](formats/save.md): the session-pointer path and its device, the read status codes, the blank card, the marker sector, the quick-save payload, the unlock flags, and the roster move layout.
- **Date:** 2026-11-06 for the write-up. The findings do not record the dates of the runs.
- **Environment:** Linux, DeSmuME through py-desmume, with the SDL dummy video and audio drivers. Investigator A used a Python 3.12 venv with py-desmume; investigator B used py-desmume 0.0.9 in a scratch venv. ROM: the supported dump, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Static checks in the Ghidra copies `rescue-arm9-a` and `rescue-arm9-b`.
- **Evidence:**
  - **Session pointer, boot to dungeon (runtime, 24,960 frames):** `python verification/v7-trace/save_session_probe.py <ROM> <OUT>`. Output SHA-1 `b8d94c372f478ad5d80f47f5f0b9fd6598fb4ec4`. `FUN_020660E4` and `FUN_02065F88` never fired; the session pointer `0x020E0490` was 0 at every save call and at the end. Hook counts: `ReadFlashData` 6, `ReadSaveFromPak` 2, `ReadSaveSector` 5, `WriteFlashData` 2 (both at sector `0x1F`), `FUN_0201D638` 1 (frame 52). [blue-verified runtime]
  - **Forced session pointer (scratch script `force_session.py`, not committed; script SHA-1 `3e967cd81eeb4610ebe51aaa25dade91abb2c306`; `python force_session.py <ROM> <OUT> N control|force`):** at the first `ReadSaveFromPak` entry (frame 53) a zeroed 0x100-byte block is stored in `0x020E0490`. Control (1,500 frames, SHA-1 `9cc3f8dc0b0296a5ccfd4517ed24c9c789a87cf7`): the normal read, with `ReadFlashData` called. Forced (1,500 frames, SHA-1 `d78947b71bbc790e80723664b4f06c5ae66cf05c`): `ReadSaveSector` for copies A and B returns 1 each (`0x0202A1E0`, `0x0202A1F8`), `FUN_0201D638` is called, and there is no `ReadFlashData`. A 3,000-frame forced run (SHA-1 `57b99cfb5dc0f15d627ddbc50182990368f7c972`) reaches the same status-1 path again at frame 2449, then reports `ARM9: Undefined instruction: 0xE7FFDEFF PC=0x02000000`. The cause was not found; the stand-in struct is incomplete, so this is likely a forcing artefact. [blue-verified runtime; the stand-in is not a real session struct]
  - **Blank card, second run (scratch wrapper `p15/wrap.py`, which imports a scratch copy of a local DeSmuME card before the run; the card's save area is blank):** `save_load_probe.py` on the ROM copy for 3,000 frames. Output SHA-1 `a81c7e27044542f4b04313c47fbd9adf42586969`, the same for the variants A-ok, A-bad and B-bad, as expected for a blank image. Both copies return status 2 and the read buffers start with `0xFF`. [blue-verified runtime; blank card only]
  - **Marker-sector buffer (runtime, boot to dungeon):** hook on `FUN_02018DC4` with r0 = `0x1F` at frames 2460 and 2563. Buffer SHA-1 prefixes `5a552514` and `5b8f9c3b` (full hashes not in the finding). Each buffer has 4 bytes of `0xFF` and leftover heap data after them, and the two differ. [blue-verified runtime]
  - **Quick-save payload (runtime, forced call, one state):** at frame 25,285 (new game, floor 1) a forced call of `ov_0001::0219B81C` with a 0x4800-byte scratch buffer; the stream byte count at `0219BC44` is 17,363. The per-monster writer `0219ADFC` is entered 20 times, 264 bytes apart. Scratch probe `qs3.py` (SHA-1 `03a88222a95cd0c57fa9757215b89e8cfcde0b03`), command `python qs3.py <ROM> <OUT> 1`, output SHA-1 `e687f9f9b53dcc9db194b52008495dac5643b663`, identical in the investigator run and two reviewer re-runs (count 17363 each). [blue-verified runtime]
  - **Roster records (runtime, new game, first dungeon turn):** 413 records of 0x40 bytes, 2 in use, both level 5; the first two moves of both carry `+1 = 0x02`; `+4` and `+5` are 0. Scratch probe `ro.py` (SHA-1 `4f880c3253bd39d3f6b651ed5f43c6f955622d4d`). [blue-verified reads; runtime for one state]
  - **Process note:** the first runs used the ROM named `pmdb.nds`, and DeSmuME's automatic backup overwrote an existing `.dsv` of that name in the user's DeSmuME save folder. The file was restored from a copy and checked. Later runs use a renamed copy of the ROM, which writes a separate `.dsv`.
  - **Static only (Ghidra copies, no runtime run):** friend-area bits, the unlock flags of `FUN_0201064C`, the 4B98 record 0, the mail section widths, the converter `FUN_0200978C`, and the device identity (`FUN_0201D638`, `FUN_0208126C`, `FUN_02074688`). Addresses are in [`formats/save.md`](formats/save.md#2-serialized-layout) and [`re/structs.md`](re/structs.md#roster-move-layout-239-p2c).
- **Outcome:** partial. Confirmed: the call chain to the session pointer, and the pointer staying 0 on the boot to a dungeon; the device identity (static); the status-1 path with a forced pointer; the blank-card load; the marker sector erase without zeroing; the quick-save size in one state; and the roster move reads. Blocked: the valid-save cases (P1.5), which move to follow-up [#260](https://github.com/TorratDev/rust-mm/issues/260).
- **Fallback adopted:** none.
- **Notes:** The open items are listed in [`formats/save.md`](formats/save.md#4-open-runtime-checks): the valid-save cases ([#260](https://github.com/TorratDev/rust-mm/issues/260)); a real flash read failure; the scene that reaches the alternate branch; the device in play (no cartridge was present in any run); the quick-save size in other states; the writers of roster `+1` and the ginseng; the level-0 creator; the unlock meanings; and the field names in P8. `save_load_probe.py` and `save_session_probe.py` are in `verification/v7-trace/`. `force_session.py` and the other scratch scripts are not committed.

## #160 P2 — Overlay 0 residency at the scene runner

- **Assumption:** overlay 0 (the ground code, `GroundStation_UnloadBank` at `0214FCA4`) is resident at every scene-runner entry, whatever the scene id. The static part is in [`re/execution-model.md`](re/execution-model.md#ground-chain-and-overlay-0-160).
- **Date:** 2026-11-04
- **Environment:** Windows 11, Python 3.12.10, py-desmume (DeSmuME), `SDL_VIDEODRIVER=dummy`, `SDL_AUDIODRIVER=dummy`. ROM `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (supported, read in place, not copied).
- **Evidence:**
  - Command: `python verification/v7-trace/overlay0_residency.py <ROM> <OUT> 2700` (the boot inputs from [`boot_to_dungeon.txt`](../verification/v7-trace/boot_to_dungeon.txt)). Run without `-I`. `<OUT>` is outside the repository.
  - Output sha256: `5e74aa824ca60d0989806dd4cf0a81d727a99918cfad8238e2b3fee4198e31c4`.
  - Runner entries (scene id, bytes at `0x0214FCA4`): frame 74, `0xF`, `10402de944109fe50000e0e3f040d1e1`; frame 614, `0x10`, same bytes; frame 2575, `0`, same bytes.
  - Overlay 0 loads at frame 45. `SetBank` events at frames 76, 616, 1133, 1270, 1789 and 2579 (banks 235, 6, 5, 234, 229, 228), all with the same bytes. The variable at `0x0215D704` (value read before each call) reads -1 at frames 76, 616 and 2579, and 6, 5 and 234 at frames 1133, 1270 and 1789.
  - Not reached in this run: dungeon entry (`0201DB70`) and the overlay reload after it, the overlay-7 path, and scene ids 1 to `0xC`. The run was 2700 frames, so the dungeon was not entered.
- **Outcome:** partial. Confirmed at runtime for scene ids `0xF`, `0x10` and `0`, and after five bank loads: overlay 0 code is intact at the runner entries. Static answer (residency does not depend on the id, with the boot, dungeon and overlay-7 unload/reload pairs and bank loads above overlay 0's BSS) is in the RE doc. Open: ids 1 to `0xC` at runtime, the reload after a dungeon, the overlay-7 path, the load-failure path, and the text overlay unloaded at `0201DBE8` (P2e).
- **Fallback adopted:** none.
- **Notes:** The probe is the repository copy of the scratch `p2_residency.py`, with the harness path made relative to the script and a SHA-1 check added. It also runs `python -I -m py_compile` clean. The scratch run logs are not committed. Ids 1 to `0xC` need inputs that reach them.

---

## #160 P4 — Text overlay residency across a dungeon run

- **Assumption:** `FS_UnloadOverlay` on the language text overlay (id 2 to 6, called at `0201DBE8`) leaves the overlay's bytes in RAM. So the text tables are still resident at `RunDungeon_Async` entry and during the dungeon. The static reading is in [`re/execution-model.md`](re/execution-model.md#into-a-dungeon-and-back).
- **Date:** not recorded in the investigator's run. This entry is written 2026-11-06.
- **Environment:** py-desmume 0.0.9 in a scratch venv, `SDL_VIDEODRIVER=dummy`, SDL dummy audio. Supported dump `pmdb.nds`, SHA-1 starting `6f018873`, read in place and not copied.
- **Evidence:**
  - Method: a scratch probe (`probe.py`) replayed [`boot_to_dungeon.txt`](../verification/v7-trace/boot_to_dungeon.txt) for 25000 frames. It hooks `FS_UnloadOverlay` at `0x0207BB28` (entry), `LoadAndRunDungeon_Async` at `0x0201DB70`, `RunDungeon_Async` at `0x0206DC20` and `FS_StartOverlay` at `0x0207BC9C`. At each hook it reads the first 0x1000 bytes at `0x021B4900` and hashes them. The probe is a scratch script, not a repository file. It is not committed, so no command here re-runs it from the repo; the output was kept outside the repository.
  - Frame 24939 (entry to `LoadAndRunDungeon_Async`): the bytes at `0x021B4900` are zero.
  - Frame 24941: `FS_UnloadOverlay(0, 2)` (the English text overlay) runs and finds the text bytes already loaded. The first 16 bytes equal the first 16 bytes of the English overlay file, compared offline.
  - Frame 24952 (`RunDungeon_Async` entry, after the unload): the 0x1000-byte hash is identical to the hash before the unload. Hash prefix `85ae4f3c3aa1`.
  - Frame 25000 (end of the run): the hash is unchanged. This comes from two reviewer re-runs of the probe, which were identical: hash prefix `85ae4f3c3aa1` at frames 24941 to 24952 and at the end of the 25000-frame run. The investigator's own output ended at frame 24952.
- **Outcome:** confirmed for overlay 2 (English) on this dump. The text bytes stay resident from the unload through `RunDungeon_Async` entry and the end of the run. The unload itself only drops registered callbacks in the overlay's range (static, `FUN_0207BB98`). **Confidence:** high that the bytes stay after the unload; medium for residency over the whole dungeon. **Not verified:** other languages (only id 2 was seen); any write to `0x021B4900` between the entry and end checks; the reload after the dungeon; the post-dungeon `FS_UnloadOverlay` of overlay 1 (not hashed).
- **Fallback adopted:** none.
- **Notes:** The probe ran against 25000 frames of the boot replay. The dungeon run stopped 48 frames in, so the dungeon's later frames were not sampled.

---

## #258 — Scene entry scripts

- **Assumption:** the per-frame script-context steppers, the two-stage entry-script selection, and the absence of opcode 9 with map 3 or 4 in the walked scripts, match the decompiles in [`re/execution-model.md`](re/execution-model.md#entry-script-of-a-scene-258-p1).
- **Date:** 2026-11-06 for the write-up. The findings do not record the dates of the runs.
- **Environment:** Linux, py-desmume with the SDL dummy video driver. `XDG_CONFIG_HOME` pointed at a scratch folder, so DeSmuME's configuration and save files stayed outside the user's folder. Input: `verification/v7-trace/boot_to_dungeon.txt`. ROM: scratch copies of the supported dump; the dump's SHA-1 is not recorded in the finding. Static work in the Ghidra copy `rescue-arm9-b` (read-only).
- **Evidence:** all probes are scratch scripts, not committed. Command form: `python <script> <scratch ROM> <out> <frames>`.
  - **`p258.py`, 20,000 frames** (hooks on the three step functions, `FUN_021461D0`, the scene runner and the bank calls): script SHA-1 `8c7c91a006e3520ff2d862b6f86f631286e3548f`, output SHA-1 `cfac7b9317f2653582b891b5dc8f2293f2021ad3`. [blue-verified runtime]
  - **`p258d.py`, 80 frames** (hooks on `FUN_02147C48`, `02147C18`, `02147EDC`, `02148024`, `021480E4` and `0214D804`): script SHA-1 `dc317cc4a9cab4f2f8b0189c1d8f36f9554761c3`, output SHA-1 `5fc44ca40cacee94d7e15645d2cdc984c976fbd7`. [blue-verified runtime]
  - **`p258e.py`, 3,200 frames** (hooks on `FUN_0214D498`, the scene runner, `FUN_0214D420` and the master-context `FUN_02147C48`): script SHA-1 `f88026b35fc2679737f84d5d8a17b537d0572679`, output SHA-1 `00da04274aa4e5e477ec9830456de9e53faba39a`. This is the run that shows the two stages. [blue-verified runtime]
  - **`p258c.py`, 90 frames** (write watch on the master context): script SHA-1 `b6760cbfada048debd9c10a38302b6448c06ed3d`, output SHA-1 `7a51188df82f48340e08cc51c6e545bb6cd0b1bc`. [blue-verified runtime]
  - **Static walk, `walk258.py` and `count258.py`:** script SHA-1 `9fee9c8ed2ec0e80f058be20cfa65eaeba12ed8c` and `7e4e179af0d72059d41e4ea0d6c4097206f567f3`. The counts are in [`re/execution-model.md`](re/execution-model.md#opcode-9-with-map-3-or-4-258-p2); no output hash is recorded in the finding. [blue-verified, bounded]
- **Outcome:** partial. Confirmed at runtime: the three steppers fire at runner frame 79, one master context and the lives, objects and effects contexts; the two-stage entry for scene ids `0xF`, `0x10` and `0`; the later kind-5 installs at frames 1136, 1270 and 1789. Bounded static result: no opcode 9 with map 3 or 4 in 592 walked scripts. Not confirmed: the scene-id mapping for ids 1 to `0xE`.
- **Fallback adopted:** none.
- **Notes:** No scene-to-bank or scene-to-script table is committed, and no script bytes are recorded, per the #160 decision. The logs and the scratch ROM copies stay in the scratchpad outside the repository.

## #161 P3 — Overlay 7 under fault injection

- **Assumption:** overlay 7 (entry `ov_0007::02139F80`, reached from title choice 5 through `RunOverlay7Mode`) shows a screen, and B leaves it for the title menu. This is checked by forcing the title result to 5, which is fault injection and not a game path. The real title route to choice 5 needs a save and is not tested here.
- **Date:** 2026-11-04
- **Environment:** Windows 11, Python 3.12.10, py-desmume (DeSmuME). ROM `pmdb.nds` at the repository root, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (the supported dump, not committed). Ghidra copy `rescue-arm9-b` for the static route.
- **Evidence:**
  - Command (the first run): `python verification/v7-trace/overlay7_inject.py <ROM> <OUTDIR> --frames 3000 --force-at 2476 --press B:120 --shots 2600,2800,3000`. The forced title result is written at `0x0201F6A0` (r0 and r8 set to 5) on the first title return at frame 2476. `<OUTDIR>` is outside the repository. The log's sha256 is `b042bce936c895e82ff0ea0a318c7dd1d322e0a5eade243bd79cb548c98ddf71`. Two runs gave a byte-identical `log.txt`; the earlier hash `8bb73829…` is obsolete, because that log held wall-clock `secs=`, which now goes to stdout only.
  - Log of that run: `TITLE first return frame=2476`, `INJECT frame=2476`, `OV7 entry reached frame=2509` (33 frames after the injection, once), `PRESS B at frame 2629`, `RELEASE at frame 2634`, and `done ... ov7_hits=1 title_hits=8`. The overlay 7 entry is hit once.
  - Screenshot at frame 2600 (before B): a grid of about 100 species sprites on a blue background, one pink sprite in the lower centre, and the message box "Please close the DS system. To end this adventure, press B".
  - Repeat with `--frames 4500 --shots 3000,3200,3600,4000,4500` (log sha256 `fe594da3cab51f37dae72eacabae62b9b69a89214da61e2003db2d68212d9d0f`, identical in two runs): the title return hook counts 8 by frame 2740, and the screens show the title logo with story text (frame 2740), then text on a coloured background (frames 3200 to 4500). The New Game / Adventure Log menu is **not** captured in this repeat. The earlier investigator run captured that menu at frame 4200 with a different input file; that screenshot is in scratch and is not reproduced here.
  - Control (no injection, investigator B): the fresh-boot title shows New Game and Adventure Log only, and choice 5 is not offered.
  - The probe was compiled with `python -I -m py_compile`. It must be run without `-I`, which hides the user-installed `desmume` module.
- **Outcome:** partial. Confirmed under injection: the overlay 7 entry is reached once, and the sprite-grid screen with its message is shown. B leaves overlay 7, the title function runs again, and the game continues to the title logo and story text. The return to the New Game / Adventure Log menu is not reproduced in this run and is open. The meaning of the screen and the real route to choice 5 are open.
- **Fallback adopted:** none.
- **Notes:** The probe is the repository copy of the investigator's `ov7.py`, with the paths made relative to the script. It refuses any SHA-1 other than the supported one. Also fixed in this entry: the broken `.github/workflows/verify.yml` links (the file was removed in `320b68d`). The open items are in [`re/execution-model.md`](re/execution-model.md#open-questions).

---

## #169 P1 — OtherRandom16 emulator vector (not reached)

- **Assumption:** the 16-bit generator (`OtherRandom16`, `02023B9C`, state u16 at `020E03F4`) is called in ground mode, and the boot path of `boot_to_dungeon.txt` passes through ground code that uses it.
- **Date:** 2026-11-05
- **Environment:** Windows 11, py-desmume (DeSmuME 0.9.12), `SDL_VIDEODRIVER=dummy`. ROM `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (checked by the probe, supported, not committed).
- **Evidence:**
  - Command: `python verification/v7-trace/other_rng.py <ROM> <OUT> --frames N`, with N = 2700, 12000 and 27000. `<OUT>` is outside the repository. Hooks: entry `02023B9C` (logs state and `lr`) and `02023BB8` (logs state and `r0`).
  - Result: zero calls in all three runs; the log holds only the `end` event. Output sha256 `ed1d57e743858ee2ded7b6ddc3d054e4f5b9ed63d5ab7593c922730d8366066c` (2700 frames, identical in two runs), `845ef0f325707c85982eccccdec67fea0e6a4e7ef373ea55de3c6ebd705cb1b1` (12000), `67440d99fd8833c106f4adee338f55930401cb65d787d3b14a97e14e602aa0a4` (27000).
- **Injection (follow-up):** `python verification/v7-trace/other_rng.py <ROM> <OUT> --frames 2700 --inject 2500`. At the first hit of the frame routine (`0201F268`) at or after frame 2500 the probe saves the registers, calls `OtherRandom16` four times with LR = `0201F268` (from the live state, then from `0xFFFF`), reads `r0` and the state, and restores the registers and the state word. Two runs gave a byte-identical log, sha256 `46863b159f39469eb2b839667bbc5adcedff4ea944c0e771aeb9e52172d02fac` (SHA-1 check as above). Live state `0x348C`: returns 25497, 27682, 3703, 11432; from `0xFFFF`: 912, 34893, 3270, 29771. `r0` equals the new state, top 16 bits 0; `state' = state * 109 + 1021` (16 bits). Test: `other_rng_matches_forced_calls_on_the_retail_rom`. Natural callers are still not reached.
- **Outcome (natural callers):** not reached. The entry/exit hooks are proven to fire by the injection run. Needs input that reaches a caller (for example the `FUN_02140860` event check or an actor script in ground mode).
- **Fallback adopted:** none.

---

## #175 P5 — D-pad repeat timing

- **Assumption:** `UpdateKeyState` (`FUN_02017B20`) is called once per frame and its D-pad repeat counter starts at 1, fires at 48 and resets to 43 (static reading in [`re/io.md`](re/io.md#d-pad-repeat)).
- **Date:** 2026-11-05
- **Environment:** Windows 11, py-desmume (DeSmuME 0.9.12), `SDL_VIDEODRIVER=dummy`. ROM `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (checked by the probe, supported, not committed).
- **Evidence:**
  - Command: `python verification/v7-trace/dpad_repeat.py <ROM> <OUT>` (defaults: Down held from frame 800 for 200 frames, no other input). `<OUT>` is outside the repository. The probe logs the key struct at `0x020F2790` (held, pressed, repeat, flags, group, counter) every frame.
  - Output sha256 `a2467ac72b44c0b2a1b8c673887786032b146d5b63d42cc0adb3c3e26fcf6c02`, identical in two runs.
  - Frames counted from the press: pressed at 0; repeat nonzero at 0, 47, 52, 57, ... 197 (every 5 frames). The counter reads 1 at frame 0, 47 at frame 46, then 43 at frame 47 (the fire).
- **Outcome:** confirmed for Down: fires at the press, again after 47 frames (counter 48, reset to 43), then every 5 frames. One call per frame holds. Up, Left and Right were not run.
- **Fallback adopted:** none.

---

## #278 P1–P5 — AI runtime probe

- **Assumption:** the indirect calls and tables left open by #194 and #275 can be answered by one dungeon run: `02076AEC` call 2, `020731AC`, the `0207D500` lists, hook cell `0x02135394` and the table at `0x021B485C`, and `info+6` for team and wild monsters.
- **Date:** 2026-11-06
- **Environment:** Linux, py-desmume (DeSmuME 0.9.12), `SDL_VIDEODRIVER=dummy`, `SDL_AUDIODRIVER=dummy`, `XDG_CONFIG_HOME` set to a fresh scratch directory, a scratch-named copy of the ROM. ROM `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed).
- **Evidence:**
  - Command: `XDG_CONFIG_HOME=<fresh dir> SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy ~/venvs/desmume/bin/python -I verification/v7-trace/ai_runtime_probe.py <scratch ROM copy> <OUT.json> --turn-frames 3000`. `<OUT.json>` is outside the repository. It uses `boot_to_dungeon.txt` and `turn_inputs.txt`; the dungeon is reached at frame 24960 and the run ends at frame 27960.
  - Output SHA-1 `87f0b773319bc7cdc5513aaf530bc6b0503f03b0`, identical in two runs by investigator B and in two reruns by the executor (four runs in all, no difference).
  - Hooks: exec `0x02076B28`, `0x0207D518`, `0x02076328`, `0x02076340`, `0x02185548`; write watches on `0x02135394`, the words `0x021B485C`–`0x021B48AC` and `0x021B3F44`; P5 reads `info+6` at each turn trigger.
- **Outcome:** answers P1–P5 in [`re/ai.md`](re/ai.md) and [`re/structs.md`](re/structs.md). Limits: one new-game first floor and 3000 frames of "press A"; no item-throwing AI and no sleep or lid event occurred, so the entry values at `0x021B485C` and the walker's execution are unobserved.
- **Fallback adopted:** none.

---

## #267 P1 — Scene-entry probe

- **Assumption:** the scene runner `0x0206F7EC` picks a loader-script index per scene id and calls `FUN_0214D498` with it; the #258 mapping for ids `0xF`, `0x10` and `0` holds on a repeat, and the runner's selection for the other ids can be run by forcing the id.
- **Date:** 2026-11-06
- **Environment:** Linux, py-desmume (DeSmuME 0.9.12), `SDL_VIDEODRIVER=dummy`, `SDL_AUDIODRIVER=dummy`, a fresh `XDG_CONFIG_HOME` per run, a scratch-named ROM copy. ROM `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Input `verification/v7-trace/boot_to_dungeon.txt`, SHA-1 `b3f969cec1bb5aeb790c50e62727b5cdc5686d61`.
- **Evidence:**
  - Base command: `XDG_CONFIG_HOME=<fresh dir> SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy ~/venvs/desmume/bin/python -I verification/v7-trace/scene_entry_probe.py <scratch ROM copy> <OUT.jsonl> 3200 verification/v7-trace/boot_to_dungeon.txt`. Output SHA-1 `87393ba12893f5d586c13ee7811cc547e913f2ad`, identical in two runs.
  - Forced-id command: the same with `300` frames and `--runner-sid ID` after the input path. The id overwrites `r0` at the first runner entry, so it is **forced, not a natural transition**. Output SHA-1 per id (run 1 = run 2 for all 17, by investigator C); the executor reran ids 5 and 14 twice and reproduced them:
    id 1 `d0a04962c9f316653f4926f967c455aa16942e30`; id 2 `c3a2c32d25c4951fb5c9a0a531f552b3edb18591`; id 3 `8829d6f3e9c77088287e4af75ac08ad91fe9898e`; id 4 `e5a380d9c36209bb48b3115f6eed75320462bde5`; id 5 `6e995b6dcec0776e821881a576353901db6b93fb`; id 6 `4e6f042a1d22aa6eb381a3762773741a6e08c03c`; id 7 `88f015bf8ebaa8a9645942cda8435336c7a652d2`; id 8 `a55b1378a091569f315c005e0b13810c036608f0`; id 9 `a42bf4bdd9e152a20b643cd9db5e40d0c77138e6`; id 10 `9c0d850ad9eee593110779203a3b9eb1b68e0ed9`; id 11 `b024d7132993707405451eff668409e3fccfd732`; id 12 `b23420432633489d21a2888816e4e19926054dea`; id 13 `e60876e80916236f88b27dbca9bdbd78f183a71f`; id 14 `61fe92e1236f27cf082adb9d5a74b6083a86a047`; id 15 `db9c1dbdf7e63d33f64f2fb1c6c5f6e929c9de5b`; id 16 `d6cc5fbf987d5cb12c63d6471e88cbe991ab9755`; id 17 `5bfa4c8aa5a4f2d55cfe9867a93ff22d3f931215`.
- **Outcome:** results in [`re/execution-model.md`](re/execution-model.md#runtime-repeat-and-forced-scene-ids-267). The repeat confirms scenes `0`, `0xF`, `0x10` and adds an index `0x72` call at frame 2380; the forced ids give the runner's selection for ids 1 to `0x11`; the loader table has 406 entries, not the 56 the first walk used. Natural reach of ids 1 to `0xE` is not achieved: it needs game progress or a save.
- **Fallback adopted:** none.

---

## #270 P1, P3 — Message display and RunMonsterAI RNG probes

- **Assumption:** the message display `02154BF0` advances the global LCG (static chain to `Rand32Bit`, #193 P2 / P6-f), and the per-call RNG shape and weighted-pick rule of `RunMonsterAI` from #193 (row 17, P1, P3) hold on a rebuilt probe.
- **Date:** 2026-11-07
- **Environment:** Windows 11, py-desmume (DeSmuME 0.9.12), `SDL_VIDEODRIVER=dummy`, `SDL_AUDIODRIVER=dummy`, a fresh `XDG_CONFIG_HOME` per run, a scratch-named ROM copy. ROM `pmdb.nds`, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed). Inputs `boot_to_dungeon.txt` and `combat_inputs.txt` (P1) or `turn_inputs.txt` (P3). Run without `-I`: isolated mode hides the user-site `desmume` package on this machine.
- **Evidence:**
  - P1 command: `XDG_CONFIG_HOME=<fresh dir> python verification/v7-trace/msg_lcg_probe.py <scratch ROM copy> <OUT.json> 36000 10 0x4a`. Forced, not natural: it pokes `info+0xD9` and `info+0x40` from the 10th `RunMonsterAI` entry. Output file SHA-1 `63548bbbd4c87b7030466b4ee33fc4d55e806e0f`, identical in two runs (equals the investigator's). Dungeon frame 24960; 623 `02154BF0` entries and returns, 207 with `02154910` entered, 0 with a changed `sPRNGState`, 0 LCG writes inside.
  - P3 observe: `... python verification/v7-trace/ai_rng_probe.py <scratch ROM copy> <OUT.json>` (4700 frames after the dungeon frame). Output file SHA-1 `209a25d8254e0357941bbf67ae58e5087351e9a8`, identical in two runs. 263 calls, draws 247/15/1.
  - P3 force: the same with `--force`. Output file SHA-1 `f5f39c8575a1c7ee323bb5a85c57f5c5ce681dbd`, identical in two runs. 255 picks, 255 match `cumsum[i] >= roll`, 221 match `roll < cumsum[i]`.
  - The file hashes are of the files as Windows writes them (CRLF line ends in the two multi-line outputs). With line ends normalised to LF, as on Linux, the `ai_rng_probe.py` outputs are observe `a488f88c0fd4766134292cad111ff5cf2f72992f` (the hash the script prints) and force `a54fa8a4ac225b3129487f542cb924e70ac64b37`; `msg_lcg_probe.py`'s is `92e7c68c954e97e4f80df47f0630d0b1048f5388`.
- **Outcome:** P6-f and row 17 of [`re/ai.md`](re/ai.md) are runtime-refuted for the forced step-4 path; [`re/rng.md`](re/rng.md) matches. The #193 P1/P3 counts are replaced by this probe's; they differ from the lost scratch script's 303-call run, which cannot be reproduced. The `>=` pick rule is reproduced (255/255). Limits: forced preconditions and one string for P1; one floor, behaviour 0 only for P3.
- **Fallback adopted:** none.

---

## #260 — Valid-save load cases

- **Assumption:** with real save data, `ReadSaveFromPak` takes copy A when it is valid (status 0, copy B skipped), and falls back to copy B when copy A fails its checksum (status 2). Both cases reach the same state path. These are the valid-save rows left open in [`formats/save.md`](formats/save.md#3-corrupt-or-blank-detection).
- **Date:** 2026-11-07. The findings do not record run dates; the image's file name carries this date.
- **Environment:** py-desmume (DeSmuME) through `save_load_probe.py`, with a fresh scratch `XDG_CONFIG_HOME` for each case. OS and package versions are not recorded in the findings. ROM: the supported dump, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (not committed; the probe checks it).
- **Evidence:**
  - Image: the maintainer's save, `.dsv` SHA-1 `1fff5c79f4e3344e59ba8b8f9b8fce12a0b8704b` (262,266 bytes). Raw save SHA-1 `435117ec7cd8a8d1876cb5eb7454f8421032ff8a` (the first `0x40000` bytes). The image is maintainer-held and was not committed. Its SHA-1 was unchanged after case A; case B used a modified copy.
  - Case A (copy A valid) and case B (copy A bad by one byte, `0x500` changed `0x00` → `0x01`; copy B unchanged): `python verification/v7-trace/save_load_probe.py <ROM> <OUT> --frames 3000`. Setup and the case table are in [`formats/save.md`](formats/save.md#3-corrupt-or-blank-detection).
  - Frames and statuses are in the save.md table. Per-frame logs are maintainer-held. Probe output SHA-1s, identical across two reruns under a different ROM file name: case A `c52077c218e3073c5c179c7dd6eb0d5f3db9cd15`; case B `0343520f28057834f96c6276c3e25bc5635960a7`. Case B modified image (copy A byte `0x500` flipped `00` → `01`) SHA-1 `30312bc7aba1c239c741dc6ac1bff66690669578`.
- **Outcome:** confirmed on this image. Copy A valid: status 0, copy B skipped, result 0. Copy A corrupt with copy B valid: copy A status 2, copy B status 0, result 0. Both cases follow the state path 1 → 2 → 6. Confidence: high. [blue-verified runtime]. Not run: both copies bad with real data; the scene reached after Continue (needs a Continue input script, see #267).
- **Fallback adopted:** none.
- **Notes:** The story-point values in save.md section 3 are raw values whose meaning is not decoded. Use a throwaway ROM file name: DeSmuME overwrites a same-named `.dsv` in the user's save folder ([#239 notes](#239--save-format-remaining-findings)).

---

## #267 P4 — Natural reach of the scene ids

- **Assumption:** the scene ids that reach the runner in normal play (1, 2, 5, 9, 10 and 12) can be reached from the maintainer's post-Thunderwave save with real input, and the P3 static map (callers, conditions) holds at runtime. Id 11 has no natural route (static reading).
- **Date:** 2026-11-07 for the maintainer's live run. Investigator B's runs have no date in the findings.
- **Environment:** py-desmume (DeSmuME). Investigator B's full run used a driver with `SDL_VIDEODRIVER=dummy` and `XDG_CONFIG_HOME` set to a scratch folder inside the driver. The maintainer's live run used a hooked interactive window with SDL keyboard state. ROM copy: SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (live run). Investigator B's ROM copy hash is not recorded.
- **Evidence:**
  - Save: the maintainer's save after Thunderwave Cave and the dream (20:04:59). `.dsv` SHA-1 `02260adef6284e8dde8a6bcda497dfdb5b804bd9`. Maintainer-held; investigator B copied it and never wrote it back.
  - Investigator B, full input run (21,929 frames), run twice with identical output. Command: `SDL_VIDEODRIVER=dummy python play.py --out <DIR> --plan plan_full.txt`. Output `keys.txt` SHA-1 `6046fde1f2e83db628796e60eb4d5ecba600372e`; output `events.jsonl` SHA-1 `3c11f0c2fe6f1aab701973a9480a335c97ca3d78`. The plan file's SHA-1 is not recorded.
  - Maintainer live run, run 1 (single run, accepted by the maintainer as evidence): output `keys.txt` SHA-1 `7ac69678814bd91156a3d93022a01ea4ce51702d`; output `events.jsonl` SHA-1 `f94ead6757a87792be2afab3456a69429ab387e0`. Ids 5, 9 and 10 with indices `0x70`, `0x167` and `0x168`.
  - Replay of the live run's key log (input `keys.txt`, SHA-1 `7ac69678814bd91156a3d93022a01ea4ce51702d`; `--replay <run1 keys.txt> --frames 75086`, SDL dummy driver). Output `events.jsonl` SHA-1 `b5fb6f919a8c7550fc5069390316c7075eef57a6` in two identical runs.
  - **Replay mismatch:** the replay does not match run 1. It diverges at frame 2167, just after Continue, and reaches only ids 0xF, 0x10, 0x11 and 1. The likely cause is touch input the key log does not hold (a click that took window focus is a DS touchscreen tap). Not verified.
  - Investigator B's savestate runs (the Give Up result hook and the quicksave runs) have no hashes recorded in the findings.
  - Driver scripts, saves and logs are maintainer-held and not committed.
- **Outcome:** confirmed by B's full run for ids 1, 2, 5 and 12, run twice with identical output. Confirmed by the single live run for ids 5, 9 and 10. Id 12's result -1 (Give Up) is confirmed in a savestate run. A quicksave writes result 3 with no runner call, and Continue from a quicksave reaches dungeon start with no runner call. Id 11: not reached in normal play (static, medium). The replay of the live run does not reproduce it. Confidence: high for 1, 2, 5 and 12; medium for 9 and 10 (one run, results not logged). [blue-verified runtime]
- **Fallback adopted:** none.
- **Notes:** The index for id 10 (`0x168`) differs from the forced P1b value (`0x167`), so the P1b values for ids 9 to `0xC` depend on the game state. The dungeon-result hook was not in the live run's script, so the results for ids 9 and 10 are not logged. Full results and the id table are in [`re/execution-model.md`](re/execution-model.md#runtime-repeat-and-forced-scene-ids-267).

---

## #148 — Layout case 3 (outer ring) floor: whole-floor match

- **Assumption:** the Rust port of `GenerateOuterRingFloor` (`floor::place_outer_ring_rooms`, `PortedSteps::outer_ring`) produces the same floor as the retail ROM for layout 3: the same number of RNG steps from `GenerateFloor`'s entry to its return, the same tiles, stairs, room count and terrain counts.
- **Date:** 2026-11-12 for the Rust test. The emulator run is the one recorded in [`re/outer-ring-floor.md`](re/outer-ring-floor.md#emulator-check-floor-oracle), date 2026-10-18 in that note; it was not repeated.
- **Environment:** Rust 1.97.0 on Linux 7.0.0. Emulator run from the note: py-desmume 0.0.9, supported dump SHA-1 `6f01887356268de39085df01490347bbcc3aa330`, `floor_oracle.py` SHA-1 `5515cabe4adadb664fd862227ace80e9c0d87acf`. `RESCUE_ROM` was unset in this session, so no emulator run was made here.
- **Evidence:**
  - Oracle values (seed `0x12345678`, layout 3, entry `rngAtGenerate` 305419896): 1683 RNG steps to the return, layout FNV-1a 64 `c32e3bac3f2c7b7b`, stairs (32, 20), 8 rooms, terrain type 0: 1357, type 1: 435.
  - Row added to `ORACLE_FLOORS` in `crates/sim/src/floor.rs`; `cargo test -p rescue-sim whole_floors_match_the_floor_oracle` passes.
  - `cargo test -p rescue-sim`: 93 tests passed before the row was added (the row adds no test).
  - Per-step checkpoints: `verification/v7-trace/floor_stages.py` has no option to force layout 3, and no ROM was configured, so no checkpoint run was made.
- **Outcome:** confirmed for the whole floor on seed `0x12345678`. Pending: the per-step RNG checkpoints (the 4 Range / 2 Int order inside the cell loop, and the callees' draw counts for layout 3), which need `floor_stages.py` extended and a ROM.
- **Fallback adopted:** none.
- **Notes:** The oracle fixture gives totals only. The other seeds for layout 3 were not run. Scripts and the ROM are not committed.

---

## #339 — Layout cases 4, 6 and 7 (crossroads, line, cross): static port check

- **Assumption:** the Rust ports of `GenerateCrossroadsFloor`, `GenerateLineFloor` and `GenerateCrossFloor` (`floor::place_crossroads_rooms`, `PortedSteps::crossroads`, `line`, `cross`) match the grids, bounds, invalid cells, hand-set links, RNG draw counts and step order recorded in [`re/layout-cases-4-6-7.md`](re/layout-cases-4-6-7.md).
- **Date:** 2026-11-12.
- **Environment:** rustc 1.97.0 on Linux 7.0.0. No ROM (`RESCUE_ROM` unset, no `rescue.toml` in the worktree), no emulator run, no Ghidra copy. The static inputs are the note's ghidra-cli decompiles, which are not committed.
- **Evidence:**
  - Read-only comparison of the code with the note: `CROSSROADS_XB`/`YB` (`0,11,22,33,44,56` / `1,9,16,23,31`), `LINE_XB`/`YB` (`0,11,22,33,44,56` / `4,15`), `CROSS_XB`/`YB` (`11,22,33,44` / `2,11,20,30`); crossroads corners and anchor range; room and anchor draw formulas; 18 South/North and 16 East/West link bytes for case 4; the 8 cross links; hallway flag 1; secondary-terrain flag set after 4 and 6, not after 7. No mismatch found.
  - `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass.
  - `cargo test -p rescue-sim`: 93 passed, 0 failed. Covers `crossroads_roles_and_links` (10 rooms, 6 anchors, 34 link bytes), `crossroads_makes_48_draws`, `cross_grid_has_five_rooms_and_eight_links`, `crossroads_line_and_cross_layouts_run_without_panicking`, and `sets_secondary_flag` for 6 and 7.
  - `whole_floors_match_the_floor_oracle`: no row added. The note records no whole-floor oracle values (step count, layout FNV-1a, stairs, rooms, terrain) for layouts 4, 6 or 7 on any seed. Its per-stage step counts (for example case 6, seed `0x12345678`: std3 129, std4 25, std5 33, std6 14, std11 39) are not oracle totals.
- **Outcome:** statically confirmed for layouts 4, 6 and 7. Whole-floor oracle match: not verified for any of the three. Layouts lacking a recorded oracle run: **4 (crossroads), 6 (line) and 7 (cross)**, on all seeds. The note's open questions list the runs needed: `floor_oracle.py` on seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D` for layouts 4, 6 and 7. Confidence in the static port: medium-high. [blue-verified static]
- **Fallback adopted:** none.
- **Notes:** Issue #339 stays open (PR uses `Refs #339`). Closing it needs the three oracle runs and their rows in `ORACLE_FLOORS`. Earlier notes said layouts 6 and 7 were run on one seed; those runs were per-stage only, not whole-floor.

---

## #339 — Whole-floor oracle rows for layouts 4, 6 and 7

- **Assumption:** the Rust ports of cases 4, 6 and 7 generate the same floors as the game for the forced seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D`, checked against `floor_oracle.py` totals and hashes (RNG steps, layout FNV-1a, stairs, rooms, terrain).
- **Date:** 2026-11-12.
- **Environment:** rustc 1.97.0 on Linux 7.0.0. Oracle values from the `floor_oracle.py` run recorded in [`re/layout-cases-4-6-7.md`](re/layout-cases-4-6-7.md#emulator-check-floor-oracle); probe ROM SHA-1 `6f01887356268de39085df01490347bbcc3aa330`. This session did not run the emulator (no ROM here). The values are hashes and counts only.
- **Evidence:**
  - Nine rows added to `ORACLE_FLOORS` in `crates/sim/src/floor.rs`.
  - The first run of `whole_floors_match_the_floor_oracle` failed on layout 4, entry `0x12345678`, tile hash. The port's value `0x2f5338a16f6fb9d7` matched the raw oracle output, so the oracle row was a transcription error: the row read `0x2f5338a16f9fb6d7`. The row is corrected to `0x2f53_38a1_6f6f_b9d7`. The port was not changed.
  - `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: pass. `cargo test -p rescue-sim`: 93 passed, 0 failed, including `whole_floors_match_the_floor_oracle`.
- **Outcome:** all nine rows pass.
- **Notes:** the three oracle runs and their nine rows are in place and pass, so this PR closes #339 (PR uses `Closes #339`).

---

## #340 — Whole-floor oracle rows for layouts 5, 9 and 10

- **Assumption:** the Rust ports of cases 5 (two rooms with a monster house), 9 (beetle) and 10 (outer rooms) generate the same floors as the game for the forced seeds `0x12345678`, `0xCAFEBABF` and `0x0BADF00D`, checked against `floor_oracle.py` totals and hashes (RNG steps, layout FNV-1a, stairs, rooms, terrain).
- **Date:** 2026-11-12.
- **Environment:** rustc 1.97.0 on Linux 7.0.0. Oracle values from the `floor_oracle.py` run recorded in [`re/layout-cases-5-9.md`](re/layout-cases-5-9.md#emulator-check-floor-oracle) and [`re/layout-case-10.md`](re/layout-case-10.md#emulator-check-floor-oracle). Command: `floor_oracle.py ROM OUT.jsonl --seeds 0x12345678,0xCAFEBABF,0x0BADF00D --layouts 5,9,10`, run with py-desmume (DeSmuME 0.9.12). Probe ROM SHA-1 `6f01887356268de39085df01490347bbcc3aa330`. The values are hashes and counts only; the raw output is not committed.
- **Evidence:**
  - Nine rows added to `ORACLE_FLOORS` in `crates/sim/src/floor.rs`, copied from the generated row file by command.
  - `whole_floors_match_the_floor_oracle` passes with all nine rows, with no row value changed.
  - `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass. `cargo test -p rescue-sim`: 93 passed, 0 failed.
- **Outcome:** all nine rows pass, so the PR closes #340 and #341 (`Closes #340`, `Closes #341`).
- **Notes:** the port was not changed. The emulator run was made outside this worktree (its raw output stays in the scratch area); this session did not run the emulator, and checked the rows against the port with the test.
- **Fallback adopted:** none.

---

## #69 — Slice 1 boot and menus: state on main

- **Assumption:** the boot flow in `apps/game` (ROM check, splash, title, main menu, dungeon entry and back) runs headless as its scene graph describes, and the menu matches the fresh-boot evidence: New Game and Adventure Log only, with Continue added only when a save exists ([`re/execution-model.md`](re/execution-model.md#start-up-and-the-title-menu), choice table; [#161 P3](#161-p3--overlay-7-under-fault-injection)).
- **Date:** 2026-11-12.
- **Environment:** Rust 1.97.0 on Linux 7.0.0. `RESCUE_ROM` unset and no `rescue.toml`, so no emulator run and no asset-dependent check was made. `cargo-nextest` is not installed, so `cargo test` was used.
- **Evidence:**
  - `cargo fmt --all --check`: clean.
  - `cargo clippy --workspace --all-targets -- -D warnings`: no issues.
  - `cargo test -p rescue-game`: 23 passed. No ROM-gated test exists in `apps/game`, so nothing was skipped.
  - The headless tests cover: the splash ends after its ticks or on a press; the title waits for a press; the menu cursor wraps; the two entries open their placeholders and B returns; a held button counts once; a refused ROM quits on a press; boot to menu, menu to dungeon and back, with the floor built on entry and dropped on leaving (`app.rs`); the fixed seed builds the same floor twice and draws each visible cell in its terrain colour (`dungeon.rs`).
  - **Mismatch with the sub-issue text:** [#69](https://github.com/TorratDev/rust-mm/issues/69) and the brief name the menu as New game / Continue / Options. The evidence above gives New Game and Adventure Log on a fresh boot, Continue only with a save, and no Options entry. The code follows the evidence. Nothing was added for Options.
- **Outcome:** the placeholder flow, scene machine and headless tests are on `main` (#379, #381). The done-when is not met: the title and menu are not built from original art and text, and no ROM was configured to load them. That part is the follow-up [#380](https://github.com/TorratDev/rust-mm/issues/380). The PR for this entry is `Refs #69`.
- **Fallback adopted:** none.
- **Notes:** The menu entries in the code are placeholders (blocks, no text), as `draw.rs` says. The menu wording and Options question need a decision before the follow-up: either the evidence is extended to show an Options entry, or the sub-issue text is corrected.

---

## #342 — Staged fixed rooms (P1) and the number-4 rectangle (P2): static port check

- **Assumption:** the Rust ports of the staged branch of `02149E80` (`floor::stamp_fixed_room`, entry flags bit 0: the 12 retail entries 50-52, 54-60, 62, 63) and of the number-4 rectangle in `0214A0D4` (`floor::place_fixed_room`, x 2..=4, y 5..=16) match the findings for #342 (investigator A) on the room rectangle, the 8 x 8 code buffer, the scratch copy keeping the room byte, the placeholder (terrain `0x0110`, tile `+0xA = 0xE`) and the rectangle's terrain and type.
- **Date:** 2026-11-12.
- **Environment:** rustc 1.97.0 on Linux 7.0.0. No ROM (`RESCUE_ROM` unset), no emulator run, no Ghidra copy in this session. The static and ROM-data inputs are investigator A's findings (`~/rescue-re/scratch/findings/342-a.md`); the flag set was read from the retail `fixedmap` by that investigator and not re-read here.
- **Evidence:**
  - New unit tests in `crates/sim/src/floor.rs`: `number_4_rectangle_is_applied_with_or_without_the_tileset_flag` (rectangle type 0 and terrain `0x10`; outside tiles keep type 3 under the code-7 flag; number 3 gets no rectangle; no panic with the flag) and `staged_room_is_a_placeholder` (rectangle `[3, 3, 10, 9]`, buffer entries at `dx * 8 + dy`, room bytes kept, one table monster `0x1A5` recorded at (3, 3) with spawn 0, stairs still written, placeholder exactly on the tiles 2 inside on every side).
  - Spawn gating, from a review decompile of `02152B20` [blue-verified static]: spawn 0 suppresses only items (the id goes to `+0x67B`) and the table trap. A table monster (`+8 != 0`) and the code-`0x45` trap are recorded whatever the spawn argument is. `fixed_tile_table_path` checks the monster with spawn 0 and no item; `fixed_tile_setter_codes` checks the code-`0x45` trap with spawn 0 and spawn 1. Rooms 0-49 and the unflagged rooms use spawn 1 and are unchanged.
  - `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass. `cargo test -p rescue-sim`: 95 passed, 0 failed (93 before, plus the two new tests; the count is unchanged by the test edits here).
  - The stamp test that expected a panic for staged rooms is replaced by the staged-room test.
- **Outcome:** statically and unit-tested only. No emulator run covers P1 or P2: a forced staged room and a forced fixed room 4 with the tileset flag have not been run against the retail dump. The oracle rows are unchanged and do not run here (no ROM). Confidence: high for the flag set and the rectangle (decompile and ROM data, per the findings); medium for the scratch start state and the placeholder's field set, which are marked hypothesis in [`re/fixed-rooms.md`](re/fixed-rooms.md#staged-rooms-p1).
- **Fallback adopted:** none.
- **Notes:** P3 (the spawn draw table per setter) and P4 (`0214961C`, `+0xC65C`, cell `+0x1B` are dead stores, not ported) are recorded in [`re/fixed-rooms.md`](re/fixed-rooms.md#spawn-rng-draws-per-setter-p3). The reveal `0215277C` is not ported. Issue #342 stays open (PR uses `Refs #342`); the forcing needed for P5 is separate.

---

## #342 — Retail whole-floor check of forced fixed rooms 1 to 66 (P5)

- **Assumption:** the Rust port of `GenerateFloor` with `ProcessFixedRoom` (`crates/sim`, `PortedSteps` with the `fixedmap` file and the `021AC208` table from the ROM) leaves the same stairs, room count, terrain counts, tiles (`layoutFnv1a`) and dungeon RNG state at `GenerateFloor`'s return as the retail game, when the fixed-room number is forced at entry (dungeon `+0x34B4`, layout byte 1, natural floor properties). This is the check the port was missing for #342 (the earlier notes said no emulator run covered P1 or P2).
- **Date:** 2026-11-13.
- **Environment:** rustc 1.97.0 on Linux 7.0.0. ROM: the supported dump `pmdb.nds` at the repo root (SHA-1 `6f01887356268de39085df01490347bbcc3aa330`), `RESCUE_ROM` set to it and `RESCUE_REQUIRE_ROM=1`. Oracle values from the probe run in [`re/fixed-rooms.md`](re/fixed-rooms.md#retail-whole-floor-check-p5) (`~/rescue-re/scratch/342b/fx.py`, six processes, seeds `0x12345678`, `0xCAFEBABF`, `0x0BADF00D`; the raw output is not committed). No Ghidra copy in this session.
- **Evidence:**
  - New file `tests/tests/fixed_rooms_rom.rs` (`fixed_rooms_match_the_retail_floors`, a `rom_test!`). Its 100 rows were copied from the generated row file by `cat`. No change to `crates/sim` was needed: every item the test uses is already `pub`.
  - Command: `RESCUE_REQUIRE_ROM=1 RESCUE_ROM=<dump> cargo test -p rescue-tests --test fixed_rooms_rom`. **Result: FAILED**, 49 mismatches in 100 rows. Without a ROM the test prints `SKIPPED fixed_rooms_match_the_retail_floors: no ROM configured ...` and passes.
  - Stairs, rooms and terrain counts: all 100 rows match.
  - Dungeon RNG steps: numbers 1 to 49 except 6 differ on all seeds (the port takes 2 steps; the oracle takes 3 to 6). Number 6 matches (2 steps). Numbers 50 to 66 match on all three seeds.
  - Tile hash: number 1 on seed `0x0BADF00D` differs (port `0x6473e7f0b4b5b711`, oracle `0xb2d08f29d84e66a9`). All other rows match.
  - Number 4 (the number-4 rectangle, P2): **does not pass**. Tile hash, stairs, rooms and terrain counts match; the RNG steps differ (port 2, oracle 3).
  - `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass.
  - `cargo test -p rescue-sim`: 95 passed, 0 failed (the port is unchanged, so the count is the same as before this PR)
- **Outcome:** the whole-floor check does not pass. The port's fixed-room tiles and stairs match the retail floors on 99 of 100 rows (the exception is the tile hash of number 1 on one seed). The RNG steps differ on 48 of the numbers 1 to 49, which is consistent with the item, trap and monster draws of the fixed-room setters being unported (not isolated; see the notes in [`re/fixed-rooms.md`](re/fixed-rooms.md#retail-whole-floor-check-p5)). The rows were not changed. The test stays red until those draws and the number-1 tile difference are explained; the PR uses `Refs #342`.
- **Fallback adopted:** none.
- **Notes:** the test reads the tile table from the ARM9 overlay that holds `021AC208`, and `fixedmap` from `dungeon.sbin`. The tileset (`+0x34AE`) is left at 0, as in the whole-floor test of `crates/sim`, because the natural value was not recorded by the probe.

### Re-run after P6 (code `0x44`) and P7 (monster draw), 2026-11-13
- **Result: PASSES.** `RESCUE_REQUIRE_ROM=1 RESCUE_ROM=<dump> cargo test -p rescue-tests --test fixed_rooms_rom`: 1 passed, 0 mismatches in 100 rows. Before the change: 49 mismatches (RNG state on numbers 1-49, and number 1's tile hash on seed `0x0BADF00D`). The rows were not changed.
- **Environment:** Windows 11 Pro (10.0.26200), rustc 1.97.0, the supported dump `pmdb.nds` (SHA-1 `6f01887356268de39085df01490347bbcc3aa330`).
- Fixes: code `0x44` clears tile `+2` bit 1 only (`02152B20`, [`re/fixed-rooms.md`](re/fixed-rooms.md#code-0x44-and-the-monster-draw-p6-p7)); the monster setter draws one `DungeonRandInt(8)` per table monster (`02153018`, LR `021909C4`), in `PortedSteps::place_fixed_room`.
- `cargo test -p rescue-sim`: 96 passed, 0 failed (one new unit test, `fixed_tile_setter_code_44_clears_bit_1_only`).
- `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass. `scripts/check-determinism.sh`: pass.
- Outcome: the whole-floor check passes on all 100 rows. The P7 draw is a shortcut: the setter's global-RNG draws stay unported; rooms 50-66 have no table monster, so no draw happens there. The test does not check the spawn lists, so the draw is checked only through the dungeon RNG state.

---

## #110 — Whole-floor golden coverage: layout variants and fixed rooms

- **Assumption:** every `Layout` and `LayoutPlan` variant has a whole-floor row for the natural seed and for each of the three forced seeds (`0x12345678`, `0xCAFEBABF`, `0x0BADF00D`) that `cargo test` checks, and the fixed rooms are checked by the ROM-gated test `tests/tests/fixed_rooms_rom.rs`.
- **Date:** 2026-11-13.
- **Environment:** Windows 11 Pro (10.0.26200), rustc 1.97.0, Python 3.12.10 with py-desmume (system Python), dump SHA-1 `6f01887356268de39085df01490347bbcc3aa330`.
- **Evidence:**
  - Oracle runs (`verification/v7-trace/floor_oracle.py`, outputs kept outside the repo): `--layouts 3,4,5,6,7,9,10` (natural seed; 7 cases, output SHA-1 `024756e54b9432979ec3f996b483c19cae1150f6`) and `--seeds 0xCAFEBABF,0x0BADF00D --layouts 3` (2 cases, output SHA-1 `1163534e0b4a2d6987c8f7c7f743fbcb45614933`). Both ran with `layoutIndependentOfGlobalRng: true`.
  - Nine new rows in `ORACLE_FLOORS` (39 to 48): the natural seed for layouts 3, 4, 5, 6, 7, 9 and 10, and layout 3 for seeds `0xCAFEBABF` and `0x0BADF00D`. All nine match `generate_floor`; no mismatch. Before this, layouts 3 to 7, 9 and 10 had no natural-seed row, and layout 3 had only seed `0x12345678`.
  - Audit of `whole_floors_match_the_floor_oracle` (`crates/sim/src/floor.rs`): after the new rows, every layout byte 0 to 11 has four rows (natural seed and the three forced seeds). The coverage table is in [`re/dungeon-generation.md`](re/dungeon-generation.md#whole-floor-golden-test).
  - New test `whole_floor_rows_cover_every_layout_variant` (`crates/sim/src/floor.rs`): maps each variant to its layout byte through `plan_layout`, and checks a row exists for each of the four entries. Its `variant` match is exhaustive, so a new variant fails to compile until it is named.
  - The whole-floor test's doc now points to `fixed_rooms_rom.rs`; the module doc of `crates/sim/src/floor_gen.rs` no longer says the fixed rooms are not ported.
  - `cargo test -p rescue-sim`: 97 passed, 0 failed. `RESCUE_ROM=<dump> cargo test -p rescue-tests --test fixed_rooms_rom`: 1 passed, with the dump (the body ran).
  - `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check-determinism.sh`: pass.
- **Outcome:** the sim side covers the natural seed and the three forced seeds on every layout byte 0 to 11, and all rows match. Still not covered by a row: layout bytes 12 to 15, which take the standard generator as byte 0 does (checked by reading `GenerateFloor`'s `switch`, not by an oracle run). The fixed-room check is ROM-gated and passed on the supported dump.
- **Fallback adopted:** none.
- **Notes:** the nine values are hashes and counts from the `generateEnd` fields; the runs and their commands are in the coverage section of `re/dungeon-generation.md`.

---

## #397 — `fixedmap` parser in `crates/formats` (P1-P3)

- **Assumption:** the parser in `crates/formats/src/fixedmap.rs` reads the retail `fixedmap` as the game does (entry table at the SIR0 content pointer, 140 file-relative offsets, entry = width, height, flags and a stream bounded by the next entry, the run-length decoder), and the sim's switch to it changes no output on the retail file.
- **Date:** 2026-11-13.
- **Environment:** rustc 1.97.0 on Windows 11 Pro (10.0.26200); nightly-2026-10-01 for the fuzz target's type check. Dump SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (the main checkout's `pmdb.nds`). The static inputs are investigator A's findings for #397 (P1-P3, comment of 2026-11-13).
- **Evidence:**
  - Unit tests in `crates/formats/src/fixedmap.rs` (4 tests, hand-made bytes): lookup and stream bounds, width 0 accepted, the decoder's runs and escapes, an escape without its literal, and rejections (table past the file, entry in the header, entry overlapping the next header, entry 139 at the table, short stream, run past the last code).
  - ROM test `fixedmap_entries_decode_to_their_size` (`tests/tests/fixedmap_rom.rs`): all 140 entries; widths and heights 5 to 17, flags 0 or 1, exactly `width * height` codes per entry, and entry 139 followed by two `0xAA` bytes before the table.
  - `RESCUE_ROM=<dump> cargo test -p rescue-tests --test fixed_rooms_rom --test fixedmap_rom`: 2 passed, 0 skipped. `fixed_rooms_match_the_retail_floors` still checks 100 of 100 rows.
  - `cargo test --workspace`: 460 passed, 0 failed. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check-deps.sh`, `scripts/check-determinism.sh`: pass.
  - Fuzz: `cargo +nightly-2026-10-01 check --manifest-path fuzz/Cargo.toml --bin formats_fixedmap`: pass.
- **Outcome:** verified against the retail dump: all 140 entries parse and decode as the spec says; the whole-floor check runs through the new parser with no change in its 100 rows. Not run: the fuzz target under libFuzzer (`cargo-fuzz` is not installed here; CI runs it). The game's behaviour on a malformed file is not run.
- **Fallback adopted:** the entry count is fixed at 140 (the game stores none); the rejection list is a design choice, marked as such in [`formats/fixedmap.md`](formats/fixedmap.md).
- **Notes:** the sim keeps `GenState::fixed_map` as raw bytes and parses it at each use. Behaviour changes only for a malformed file: a file that fails the parse now gives no room (size 0) or the existing "no entry" panic, where the old lookup read whatever the bad entry held. The sim's unit tests for the old lookup moved to `fixedmap.rs`, so `cargo test -p rescue-sim` runs 2 fewer tests.

---

## #400 — Dungeon scene: floor properties in the sim and the floor layer (P1, P2)

- **Assumption:** `properties_for` and `properties_from_record`, moved from the trace harness into `rescue_sim`, read the same values; and a background layer built from tileset 14's tiles and the sim's autotile cells on the natural seed draws the first floor with the tileset's own tiles.
- **Date:** 2026-11-13.
- **Environment:** rustc 1.97.0, Windows 11 Pro (10.0.26200). Dump SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (the main checkout's `pmdb.nds`). Static and runtime inputs: investigator a's P1 and investigator b's P2 on #400.
- **Evidence:**
  - Moved to `crates/sim/src/floor_gen.rs`: `properties_from_record` and `properties_for(map, dungeon, floor)`, which returns a new `NoFloor` error. The two tests moved with them (`record_tests`). The harness's `start_properties` calls the sim function and maps `NoFloor` to its own `FloorError::NoFloor`. `rescue-sim` gained the `rescue-data` dependency (`scripts/check-deps.sh`: pass).
  - New `rescue_render::bg::floor_layer(&Tileset, cells)`: the 56x32 cells become a 168x96 `BgLayer`, each cell drawn as its metatile (3x3 tiles) with the tileset's own palettes. Unit tests `floor_layer_draws_each_cell_as_its_metatile` and `floor_layer_refuses_what_the_tileset_lacks` (short cell grid, metatile past the list, tile past the tile data, palette bank past the palettes).
  - ROM test `floor_layer_builds_for_the_natural_first_floor` (`crates/render/src/rom_tests.rs`): dungeon 0 floor 1 through `properties_for`; tileset number from record byte `+0x02` (asserted 14); the sim generates the floor from `DungeonRng::from_state(0xB280_4D01)`; autotile with variant draws fixed at 0; the layer is 168x96 and draws some nonzero tiles. Run as `RESCUE_ROM=<main checkout>/pmdb.nds RESCUE_REQUIRE_ROM=1 cargo test -p rescue-render floor_layer_builds`: 1 passed, not skipped.
  - `cargo test -p rescue-sim -p rescue-render -p rescue-trace-harness`: sim 97 passed, render 124 passed, harness 39 + 6 passed. `cargo test --workspace`: pass.
  - `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check-deps.sh`, `scripts/check-determinism.sh`: pass.
- **Outcome:** P1 and P2 as the investigators describe them: the floor's properties come from the ROM's record for dungeon 0 floor 1, and tileset 14 builds a layer for the natural-seed floor. Not run: the layer is not compared with a runtime frame, and no pixels are checked.
- **Fallback adopted:** variant draws are fixed at 0 (`shortcut:` comment in `rom_tests.rs`); the retail variants need the global RNG (P3), which the scene wiring will add. Palette animation is not applied (static palette).
- **Notes:** `crates/render/Cargo.toml` gained a dev-dependency on `rescue-sim` for the sim-floor test; the task's file list did not name it. The ROM test repeats the harness's generation call (about six lines), because the harness's `generate` is app-local and was not in the move.
