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
| V7 | DS emulator can be scripted to dump RAM/RNG per turn | Phase 3 trace harness | Partial — py-desmume script with savestate, fixed inputs and exec trigger gives byte-identical dumps over 3 runs on Linux, on a synthetic test ROM; retail Blue run and Windows CI result pending |
| V8 | Ghidra loads ARM9 and overlays at real addresses | Phase 3 | **Confirmed** — retail ARM9 and all 145 overlays mapped (Ghidra 12.1.4); type chart referenced from 4 places in overlay 1; the 142 overlays without code are data |
| V9 | REA supports the planned Ghidra workflow on ARM DS binaries | Phase 3 | Pending — depends on V8 for the run; README reviewed |

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
  - Linux build needs the packages listed in [`.github/workflows/verify.yml`](../.github/workflows/verify.yml) (CMake configure fails without them).
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
  - **More shared tables (2026-10-14, Ghidra):** the pointers in `CalcDamage` and `sub_806F370_Async` lead to three more tables in overlay 1 with the same values as Red's `src/dungeon_config.c`, compared entry by entry: `gEffectivenessChart` (4 × 4 `s32`, `ov_0001::021AA588`), `gAtkStatStageMultipliers` and `gDefStatStageMultipliers` (21 × s24.8 each, `ov_0001::021AA660` and `021AA6B4`).
- **Outcome:** **confirmed.** 4 tables are shared by value (type chart, effectiveness combination, attack and defense stat-stage multipliers), meeting the "3+ shared tables" criterion, and both routine constants match (RNG step, damage calc). `gTypeGummiIQBoost` and `gUnknown_810A808` are still not found by value.
- **Fallback adopted:** none. Red stays the map for Phase 3.
- **Notes:** Still open: where the two Gummi tables went in Blue (moved, re-laid out, or replaced). The Red dump is not the decomp's base ROM (see above).

---

## V7 — Scriptable DS emulator for RAM and RNG dumps

- **Assumption:** a DS emulator can be scripted to dump ARM9 memory and RNG state per turn (DeSmuME Lua, BizHawk + melonDS core, or melonDS GDB stub).
- **Date:** 2026-10-13
- **Environment:** Linux (Ubuntu 24.04), Python 3.13. `desmume-cli` 0.9.11 from apt. `py-desmume` 0.0.9 from PyPI (bundles libdesmume 0.9.12; wheels for Linux, Windows and macOS). CI: `ubuntu-latest` and `windows-latest`, job `v7` in [`.github/workflows/verify.yml`](../.github/workflows/verify.yml).
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
- **Outcome:** partial. Scripted dumps on a frame or exec trigger are byte-identical across runs on Linux. Not yet done: the same run on the retail Blue ROM, and the Windows result from CI.
- **Fallback adopted:** none. The melonDS GDB stub stays the fallback; the DeSmuME GDB stub shows that route works but is slow and has no input playback.
- **Notes:**
  - Setup quirks: libdesmume needs a video driver even headless (`SDL_VIDEODRIVER=dummy`); on Linux it needs `libGL` and `glib` from the system. The apt `desmume-cli` defaults to the JIT; the GDB stub run used `--cpu-mode=0`. py-desmume ran the interpreter.
  - Save files: the retail run should start from a savestate taken in a known spot, with the backup (save) file fixed too.
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
- **Date:** 2026-10-13
- **Environment:** [`morluto/rea`](https://github.com/morluto/rea) shallow clone (read only). REA is an MCP server with Node.js/npm. Its native analysis uses an existing Hopper, Ghidra or IDA install.
- **Evidence:** README reviewed. Native analysis is driven by an external Hopper, Ghidra or IDA install. Setup is `npx rea-agents setup`.
- **Outcome:** pending. The README confirms the Ghidra dependency, so the run waits on V8.
- **Fallback adopted:** none.
- **Notes:** Still to do: install and check the REA–Ghidra bridge once V8 is unblocked; run it on a small ARM9 region and record supported features and limits. Setup outline: [`ghidra-setup.md`](ghidra-setup.md#5-rea-v9-on-top-of-this-project).
