# Project roadmap: Pokémon Mystery Dungeon — Blue Rescue Team Native

**Goal:** Build a native Windows and Linux version of Blue Rescue Team in **Rust** with SDL3, without emulating the Nintendo DS or executing its original ARM code.

Approach: reverse engineer the original game, understand its data and mechanics, and reimplement them in a portable engine. Use REA and Ghidra to speed up investigation, not to generate the whole port automatically.

## Language decision
- **Rust (stable, edition 2024)** for engine, simulation and tools. Chosen over C++ for cargo, memory-safe binary parsers, and easy enforcement of a pure simulation crate.
- Go rejected: SDL needs cgo (painful cross-compiles/Windows builds); GC pauses can cause frame stutter.
- Decided up front to avoid a later rewrite. The language-independent assets (specs, glossary, golden traces) stay as data anyway.

## Naming decision
- **Project, crate, binary and package prefix: `rescue`** (decided 2026-10-13, #20). No "Pokémon" or Nintendo trademarks in any name.
- Crates: `rescue-nds`, `rescue-formats`, `rescue-data`, `rescue-sim`, `rescue-save`, `rescue-audio`, `rescue-render`, `rescue-platform`.
- Binaries: `rescue` (game), `rescue-asset-viewer`, `rescue-trace-harness`.
- Replaces the earlier `pmd-*` working names. `pmd-red` below is pret's repository name, not ours.

## Reference sources
- **Ground truth: Blue Rescue Team (NDS) only.** Its ARM9 binary, overlays, filesystem and observed runtime behavior are authoritative.
- **Explorers of Time/Darkness/Sky: out of scope.** Their tooling and formats (pmdsky-debug, SkyTemple, ppmdu) don't apply unless a format is verified identical in Blue.
- **Red Rescue Team (GBA, pret `pmd-red` decomp): comparison reference for gameplay logic only.** Same game design, different platform — use it for hypotheses about mechanics (dungeon generation, damage, AI, RNG, struct layouts, data tables).
  - Not applicable: graphics, audio, filesystem/compression, save format, screen layout, memory map. Blue is a DS rebuild (ARM9 + overlays, different compiler, 256×192 dual screen, touch).
  - Anchor matching on shared data tables and constant literals, not code shape (agbcc Thumb vs. Metrowerks ARM/Thumb).
  - Any Red-derived fact is a hypothesis until confirmed in Blue's ARM9 or by an emulator trace. Tag glossary entries `[red-hypothesis]` / `[blue-verified]`.
  - Expect Blue-specific deltas (bug fixes, rebalanced data, DS-only features). Document each one found.

## Engineering principles
- **Deterministic, headless simulation.** `rescue-sim` has no SDL/platform dependency (enforced by its `Cargo.toml`). Integer/fixed-point math only — no `f32`/`f64` in game logic. A single seeded RNG owned by the sim. No `HashMap` iteration in logic paths (use `Vec`, `BTreeMap` or `IndexMap`). The sim advances purely from input commands, so it runs in CI without a window.
- **Golden traces.** Scripted emulator runs dump RAM entity state and RNG state per turn (tooling per V7). Traces are language-neutral fixtures (JSON or a documented binary format): replay the same inputs in `rescue-sim` and diff state turn by turn.
- **Audio deferred.** Placeholder audio until the first playable dungeon. Native playback of Blue's sound format (expected SDAT — see V3) is its own crate.
- **Rendering.** Each DS screen renders into a 256×192 offscreen target via SDL3's 2D renderer; the window composites and scales by integer factors. GPU API only if a later need justifies it.
- **Parsers.** Binary formats parsed from `&[u8]` with explicit bounds checks returning `Result` — never panic on malformed input. Fuzz the ROM/format parsers (`cargo-fuzz`).
- **Legal hygiene.** Identify the user's ROM by SHA-1 and refuse unknown dumps. Keep "Pokémon" out of repo, crate, binary and package names. Never commit extracted assets — test fixtures are generated at test time from the user's dump (tests skip cleanly when no ROM is configured). Full policy: [`docs/legal.md`](legal.md).
- **Separation.** Reverse engineering notes, asset extraction, simulation and platform layer stay independent and individually testable.

## Toolchain
- Rust stable via `rustup`; `rustfmt`, `clippy` (deny warnings in CI), `cargo-nextest`, `cargo-fuzz`.
- `sdl3` crate (SDL3 bindings; see V1).
- Asset viewer UI: `egui`/`eframe` (see V2).
- Tests: `insta` (snapshot tests for decoded formats), `proptest` (property tests for sim invariants).
- Ghidra (see V8), REA (see V9), scriptable DS emulator (see V7) — observation and tracing only.
- CI: GitHub Actions matrix — `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`.

## Workspace layout
```
rescue/
├── Cargo.toml              # workspace
├── crates/
│   ├── nds/                # NDS container: header, FNT/FAT, ARM9/ARM7, overlays
│   ├── formats/            # Blue-specific formats: compression, graphics, text, data tables
│   ├── data/               # Typed game data: species, moves, items, dungeons (no SDL)
│   ├── sim/                # Dungeon + game simulation: turns, combat, AI, RNG (no SDL, no I/O)
│   ├── save/               # Save serialization
│   ├── audio/              # Sound sequencer/synth (later)
│   ├── render/             # Tilemaps, sprites, palettes, animation, screen composition
│   └── platform/           # SDL3: window, input, audio output, timing
├── apps/
│   ├── game/               # Main executable
│   ├── asset-viewer/       # egui tool
│   └── trace-harness/      # Trace replay + state diff CLI; emulator scripts in scripts/
├── tests/                  # Integration tests, golden-trace replays
├── fuzz/
└── docs/                   # Format specs, glossary, Blue-vs-Red deltas, verification log
```
Directory names are short; package names carry the prefix (`crates/sim` is `rescue-sim`, `apps/game` builds `rescue`, `apps/asset-viewer` builds `rescue-asset-viewer`, `apps/trace-harness` builds `rescue-trace-harness`).
Dependency rule: `sim` and `data` depend on nothing platform-related; only `platform` and `apps/*` depend on SDL3.

## Phase 0 — Verify assumptions
Milestone: every assumption the plan relies on is confirmed or replaced. Record each result (date, evidence, outcome) in [`docs/verification.md`](verification.md). A check gates the phase listed in "Blocks"; it doesn't have to finish before Phase 1 starts.

| ID | Assumption | How to check | Done when | If false | Blocks |
|----|------------|--------------|-----------|----------|--------|
| V1 | `sdl3` crate builds and links on Windows MSVC and Linux (static or bundled dynamic), with SDL3's 2D renderer, gamepad and audio-stream APIs exposed | Hello-window app (window + render target + gamepad + audio callback) in the CI matrix; check current crate version, feature flags and SDL3 sourcing | Both CI targets produce a running binary; feature flags documented | Use `sdl3-sys` directly, ship SDL3 as a dynamic lib, or switch platform layer to `winit` + `pixels`/`wgpu` + `cpal` + `gilrs` | Phase 1 |
| V2 | `egui`/`eframe` and the other chosen crates (`insta`, `proptest`, `cargo-nextest`, `cargo-fuzz`) work on stable Rust, edition 2024, on both OSes | Add them to the skeleton workspace; build and run in CI (`cargo-fuzz` needs nightly — confirm and isolate) | Clean CI build; any nightly requirement isolated to `fuzz/` | Swap the crate or pin an older edition for that crate | Phase 1 |
| V3 | Blue's audio uses the standard Nintendo SDAT container (SSEQ/SBNK/SWAR/STRM) | Scan the ROM filesystem for `.sdat` files and `SDAT` magic; parse header and count sequences/banks | SDAT located and its FAT/SYMB parsed, or alternative format identified | Reverse engineer the custom sound format; re-scope the `audio` crate | Phase 4 audio |
| V4 | Blue's non-audio formats (compression, graphics, palettes, animation, text encoding, data tables, save layout) are unknown and must be identified | File survey tool: list every file with size, magic bytes, entropy, compression guess; classify known/unknown | `docs/file-survey.md` covers 100% of files, each tagged known/hypothesis/unknown; save media type and size identified | N/A — discovery task; unknowns become Phase 2/3 work items | Phase 2 |
| V5 | `pmd-red` decomp has enough of the dungeon, combat, AI and RNG code decompiled and named to be useful | Read repo README/progress; inspect source tree for dungeon generation, damage calc, AI, RNG; note % decompiled and naming quality | Coverage noted per subsystem in `docs/verification.md` | Lower Red's role; rely on Ghidra analysis of Blue directly for weak subsystems | Phase 3 |
| V6 | Blue reuses Red's logic closely enough that Red is a useful map | Locate 3+ shared data tables (e.g. type chart, move data, species base stats) in Blue's ARM9/files by value; compare constants in 2+ routines (damage calc, RNG step) | Matches/mismatches documented with addresses | Treat Red as design reference only; budget more RE time for Phase 3 | Phase 3 |
| V7 | A DS emulator can be scripted to dump ARM9 memory and RNG state per turn (DeSmuME Lua, BizHawk + melonDS core, or melonDS GDB stub) | Write a script that hooks a frame or address, reads a known RAM range, and writes it to a file | One working script dumps a stable RAM region per turn to disk | Use melonDS GDB stub + external script, or patch an open-source emulator for tracing | Phase 3 trace harness |
| V8 | Ghidra can load the ARM9 binary and overlays at their real addresses (existing NDS loader plugin or a custom script) | Try available NDS loader extensions for the current Ghidra version; otherwise script overlay mapping from the overlay table | ARM9 + all overlays mapped; cross-references into overlays resolve | Write a Ghidra script that creates overlay memory blocks from the `nds` crate's output | Phase 3 |
| V9 | REA supports the planned workflow (function search, xrefs, decompiled-routine summaries, evidence notes) with Ghidra on ARM DS binaries | Run REA on a small ARM9 region; check its Ghidra integration, ARM support and how it records evidence | Supported features listed; limits noted | Use Ghidra scripting + manual notes for the unsupported parts | Phase 3 |

## Phase 1 — Research and project setup
Milestone: establish the technical foundation.
- Define the target: native x86-64 Windows and Linux builds, with no DS hardware emulator or ARM instruction translation.
- Obtain a legally acquired game dump for local analysis; record its SHA-1.
- Choose a neutral name (`rescue`) and write the legal-hygiene policy ([`docs/legal.md`](legal.md)).
- Research existing Blue/DS reverse-engineering work and DS file-format documentation ([`docs/references.md`](references.md)); clone `pmd-red` as the logic reference (`tools/fetch_pmd_red.sh`).
- Set up the cargo workspace, lints, CI matrix for Windows and Linux.
- Close V1 and V2.
- Working Windows and Linux builds that open a window and handle input.

## Phase 2 — Extract and understand the game data
Milestone: inspect the game's resources independently of the original console.
- `nds` crate: header, filesystem (FNT/FAT), ARM9/ARM7 binaries, overlay table.
- Close V3 and V4 (file survey).
- Identify graphics, palettes, sprites, animations, tilesets, maps, text, sound, game data.
- `formats` crate: parsers for the formats Blue actually uses; fuzzed and snapshot-tested.
- Asset viewer (egui): sprites, portraits, dungeon tiles, animations.
- Document file structures, compression, resource IDs, unknown fields in `docs/`.
- Searchable index of extracted resources.

## Phase 3 — Reverse engineer the original game
Milestone: understand the execution model and major subsystems.
- Close V5, V6, V7, V8 and V9 before relying on them.
- Load ARM9 into Ghidra with the correct ARM architecture; load overlays at their real addresses.
- Identify main loop, input, scene transitions, rendering, resource loading.
- Investigate representation of dungeon tiles, entities, moves, items, stats.
- Trace turn scheduler, movement, combat, status effects, enemy AI — using pmd-red as a map, verifying in Blue.
- Investigate save serialization and RNG behavior (priority: prerequisite for Phase 6).
- Build the golden-trace harness early (emulator scripts + `trace-harness` diff tool).
- Use REA for function search, xrefs, summaries, evidence-backed notes.
- Glossary of structures/functions, separating verified findings from hypotheses.
- Reverse engineer each subsystem when its implementation becomes necessary.

### Phase 3 status (2026-10-15)
Where the Ghidra work on Blue stands, so any session can pick up from here. Details live in [`docs/re/`](re/), [`docs/formats/save.md`](formats/save.md), [`docs/symbols.md`](symbols.md) and [`docs/verification.md`](verification.md).

Done:
- V4 (save media and format), V6 (4 shared tables, 2 routine constants), V7 (scripted retail trace, `verification/v7-trace/`), V8 (all overlays mapped).
- RNG (#47): all three generators, seeding, dungeon seed chain; bit-exact Rust port in `rescue-sim` (`crates/sim/src/rng.rs`).
- Execution model and overlays (#44), dungeon and entity structs (#46), turn loop and V7 trigger (#49), floor-generation entry and RNG order (#48), combat (#50), save format (#53).
- Tooling: ghidra-cli workflow, `tools/ghidra/Rescue*.java`, `RescueApplySymbols.java` driven by `symbols.md`.

Partly done:
- V9: REA not tested yet. #43 glossary still to write.
- RNG leftovers: what uses the Mersenne Twister (callers `0200D4DC`, `0206C6E4`); the 16-bit generator from `pmd-red`'s `other_random.c` (multiplier 109, increment 1021) not found yet.
- Turn loop: name the three end-of-turn functions ([`re/turns.md`](re/turns.md)).
- #53: locate the quicksave sector.

Not started:
- #45 input and resource loading (`REG_KEYINPUT` `0x04000130`, file-system open calls).

Rust floor generator (`crates/sim/src/floor.rs`), checked step by step against `verification/v7-trace/floor_stages.py` (RNG state and arguments at 26 checkpoints):
- Done: map, reset, step 1 (grid bounds).
- Next: step 2 `InitGrid` (`ov_0001::0214FD7C`), step 3 (`0214FB4C`, 129 RNG calls), then steps 4-13 of `GenerateStandardFloor`, the cleanup and spawn passes, and the other layout generators. The plan is in [`re/dungeon-generation.md`](re/dungeon-generation.md), "Reimplementation, step by step".


## Phase 4 — Build the native engine
Milestone: render original game assets in your own application.
- `render`: tilemaps, sprites, palettes, animation; per-screen 256×192 targets with integer scaling.
- DS screen layout incl. lower-screen info and menus.
- `platform`: keyboard, controller, configurable input; frame timing.
- Placeholder audio; real playback (format per V3) after the first playable dungeon.
- PC-friendly window layout, resizing, alternative screen arrangements.

## Phase 5 — Gameplay in vertical slices
Milestone: make a small part of the game playable. In order, testing each:
1. Boot and menus
2. Dungeon rendering
3. Turn-based movement (grid, collision, stairs, turn advance)
4. Combat (moves, damage, range, accuracy, defeat)
5. Enemy AI
6. Items and inventory
7. Pokémon systems (stats, levels, XP, recruitment)
8. Story and missions
9. Saving and loading
10. Full campaign

Prioritize one complete, correct dungeon over many partial systems. Audio playback lands after slice 5.

## Phase 6 — Validate behavior against the original
- Controlled scenarios in the original (Blue) game: starting state, actions, seeds, expected outcomes.
- Replay golden traces through the headless `sim`; diff state per turn.
- Compare movement, combat, status, AI, inventory, map generation.
- Property tests for sim invariants; differential testing vs. observed behavior.
- Investigate discrepancies with REA/Ghidra instead of guessing.
- Save/load cycles, unusual inputs, edge cases.
- For randomness, compare distributions or use reproducible seeds.

## Phase 7 — Polish and ship
- Test on clean Windows/Linux installs.
- Config, controller mappings, resolution, audio settings.
- Profiling; logging (`tracing`), crash diagnostics, corrupted-save recovery.
- Document supported dumps (by SHA-1) and limitations.
- Ship binaries without copyrighted assets or ROM data; neutral project naming; reproducible builds; setup guide.

## Priorities
1. Asset viewer
2. One playable dungeon
3. A complete gameplay loop (inventory, progression, saving, missions)
