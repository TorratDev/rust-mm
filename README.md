# rescue: a native Rust port of Blue Rescue Team

`rescue` is a project to rebuild **Pokémon Mystery Dungeon: Blue Rescue Team** (Nintendo DS, EUR) as a native Windows and Linux program written in Rust, with SDL3 for the platform layer.

It does not emulate the DS, and it does not run the original ARM code. The approach is:

1. **Reverse engineer** the original ARM9 binary and its overlays (with Ghidra, driven by `ghidra-cli`) to learn the data formats, the execution model and the game rules.
2. **Parse** the game's own files from the user's dump (`formats`, `data`, `nds`).
3. **Reimplement** the rules in a deterministic, headless simulation (`sim`), and check the results against traces from the original game.
4. **Draw and play** the result with a platform layer and a renderer (`platform`, `render`, `apps/game`).

The project never ships game data. You supply your own dump of a cartridge you own.

> **Status: early.** Formats, data tables, the asset viewer and the reverse-engineering groundwork are in place. The playable game is a placeholder, and the dungeon simulation is still being ported. See [Project status](#project-status).

## Contents

- [Legal](#legal)
- [Project status](#project-status)
- [Getting started](#getting-started)
- [Workspace layout](#workspace-layout)
- [Crates and apps](#crates-and-apps)
- [Game data we can read](#game-data-we-can-read)
- [Reverse engineering](#reverse-engineering)
- [Verification](#verification)
- [Blue and Red](#blue-and-red)
- [Engineering rules](#engineering-rules)
- [Contributing](#contributing)
- [Documentation map](#documentation-map)
- [Open items](#open-items)

## Legal

- **You supply the dump.** The repository never contains a ROM, a patch against one, or a link to one. Extracted assets, screenshots of game graphics, save files and ROM-derived tables are never committed. The full policy is in [`docs/legal.md`](docs/legal.md).
- **Dumps are identified by SHA-1.** Tools accept only the dumps on the supported list and refuse anything else with a message that prints the computed hash. Supported dumps are listed in [`docs/supported-dumps.md`](docs/supported-dumps.md).
- **Neutral naming.** Crates, binaries and packages use the `rescue` prefix. "Pokémon" and Nintendo trademarks do not appear in names. The game's title appears only in documentation, to say which game a fact describes.
- **Red is read-only.** The `pret/pmd-red` decompilation (Red Rescue Team, GBA) has no license file. It is used as a reference for logic only; its code and data are never copied. It is fetched into the ignored `reference/` folder with [`tools/fetch_pmd_red.sh`](tools/fetch_pmd_red.sh).

The supported dump:

| ID (`rescue_nds::rom::RomId`) | Game | Platform | Region | Size (bytes) | SHA-1 |
|-------------------------------|------|----------|--------|--------------|-------|
| `BlueRescueTeamEur` | Blue Rescue Team | NDS | EUR | 33,554,432 | `6f01887356268de39085df01490347bbcc3aa330` |

Check your dump with `sha1sum your-dump.nds`.

## Project status

Phases follow [`docs/roadmap.md`](docs/roadmap.md).

| Phase | Milestone | State |
|-------|-----------|-------|
| 0. Verify assumptions | Every assumption confirmed or replaced | Done. V2 partly (cargo-nextest and cargo-fuzz runs pending); V9 replaced (REA was rejected, ghidra-cli used instead) |
| 1. Research and setup | Workspace, lints, CI, ROM identification, a window that opens | Done for the foundation: workspace, ROM identification, SDL3 hello-world |
| 2. Extract and understand the data | Container, formats, data tables, asset viewer, index | Largely done. All formats below are verified against the EUR dump except where noted |
| 3. Reverse engineer the game | Execution model, RNG, turns, combat, floor generation, save | In progress. RNG, turn loop, combat entry points, save layout and most overlay code are mapped |
| 4. Native engine | Tilemap and sprite rendering, screen layout, input, audio | Not started (`render` and `audio` are empty; the platform layer opens a window) |
| 5. Gameplay slices | Boot, dungeon, movement, combat, AI, items, ... | Not started |
| 6. Validate against the original | Golden traces replayed through `sim` | Golden-trace capture works in the emulator scripts; the harness is not written yet (`apps/trace-harness` is a placeholder) |
| 7. Polish and ship | Settings, profiling, release binaries | Not started |

Current priorities, from the roadmap: the asset viewer, one playable dungeon, then a complete gameplay loop.

## Getting started

### Prerequisites

- **Rust 1.97.0**, pinned in [`rust-toolchain.toml`](rust-toolchain.toml). Components `rustfmt` and `clippy` are installed by rustup automatically.
- **A C toolchain and CMake.** SDL3 is built from source (`sdl3` with the `build-from-source-static` feature), so no SDL binary is needed.
- **Linux:** the system packages that the SDL and GL build needs (CMake configure fails without them).
- **Windows:** MSVC. CI builds both `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`.
- **Optional:** `cargo-nextest` for the test runner, `cargo-fuzz` (Linux, with the nightly toolchain in `fuzz/`) for the fuzz targets.

### Build and run

```bash
cargo build --workspace
cargo run -p rescue-game -- --frames 120    # placeholder game; --frames stops after N ticks
```

The placeholder game opens a window with two 256×192 DS screens. It logs keyboard and gamepad input and shows the held DS buttons on the bottom screen. `RESCUE_FRAMES` does the same as `--frames`.

### Point the tools at your dump

Tools that need game data read the dump from one of these, in order:

1. the `RESCUE_ROM` environment variable, or
2. `rom_path = "..."` in a `rescue.toml` file in the working directory (git-ignored). Use single quotes for Windows paths: `rom_path = 'C:\roms\my-dump.nds'`.

```bash
export RESCUE_ROM=~/roms/my-dump.nds
cargo run -p rescue-asset-viewer              # browse the dump
cargo run -p rescue-asset-viewer -- index build -o rescue-index.tsv
cargo run -p rescue-asset-viewer -- index search kao025
```

Exports from the viewer go to `extracted/png/`, which is git-ignored and for local use only.

### Tests

```bash
cargo test --workspace                       # or: cargo nextest run --workspace
RESCUE_ROM=~/roms/my-dump.nds cargo nextest run --workspace --no-capture
RESCUE_REQUIRE_ROM=1 RESCUE_ROM=~/roms/my-dump.nds cargo nextest run --workspace
```

- Without a dump, ROM-gated tests print `SKIPPED <test>: no ROM configured` and pass. This is what CI sees.
- `RESCUE_REQUIRE_ROM=1` turns a missing dump into a failure, so a local run cannot skip silently.
- Fixtures are generated at test time into `target/rom-fixtures/<sha1>/`, never committed.
- Snapshot tests (`insta`) store hashes, sizes and decoded metadata, not asset bytes.

Writing ROM-gated tests is described in [`docs/rom-tests.md`](docs/rom-tests.md).

### Checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
sh scripts/check-deps.sh          # dependency rule: only platform and apps may use sdl3
sh scripts/check-determinism.sh   # no floats, HashMap or clocks in sim and data
```

### Fuzzing

The fuzz targets live in `fuzz/` and use their own pinned nightly toolchain. cargo-fuzz runs only on Unix-like systems (x86-64 and AArch64), not Windows.

```bash
cd fuzz
cargo fuzz run formats_gfx
```

Targets: `data_tables`, `formats_assets`, `formats_compress`, `formats_containers`, `formats_gfx`, `nds_blz`, `nds_container`.

## Workspace layout

```
rescue/
├── Cargo.toml              workspace (members: crates/*, apps/*, tests)
├── crates/
│   ├── nds/                NDS container: header, FNT/FAT, ARM9/ARM7, overlays, BLZ
│   ├── formats/            Blue file formats: archives, SIR0, compression, graphics, text, maps
│   ├── data/               typed game data: species, moves, items, dungeon floors (no SDL)
│   ├── sim/                deterministic, headless simulation: RNG, floor generation (no SDL, no I/O)
│   ├── save/               save serialization (placeholder)
│   ├── audio/              sound sequencer and synthesizer (placeholder, later phase)
│   ├── render/             tilemaps, sprites, palettes, screen composition (placeholder)
│   └── platform/          SDL3 window, input, audio output, timing
├── apps/
│   ├── game/               main executable, builds the binary `rescue` (placeholder game)
│   ├── asset-viewer/       egui viewer and resource index, builds `rescue-asset-viewer`
│   └── trace-harness/      golden-trace replay and diff, builds `rescue-trace-harness` (placeholder)
├── tests/                  integration tests (`rescue-tests`), ROM-gated and synthetic
├── fuzz/                   cargo-fuzz targets (nightly, Linux)
├── tools/                  Python and Ghidra tools for the dump (extraction, survey, Ghidra scripts)
├── scripts/                CI checks: dependency rule, determinism, progress treemaps
├── verification/           standalone Phase 0 probes and the V7 emulator trace scripts
├── rescue-re/              Ghidra import inputs and project copies (derived from the dump; see the note below)
└── docs/                   specs, RE notes, roadmap, verification log (an Obsidian vault)
```

Directory names are short; package names carry the `rescue-` prefix (`crates/sim` is `rescue-sim`, `apps/game` builds `rescue`).

> **Note on tracked game data.** The repository currently tracks two ROM archives at the root (`Pokemon_MMist_Azul(Esp).rar` and `Pokemon Mundo Misterioso Equipo de Rescate Rojo.zip`), and `rescue-re/` contains extracted ARM9 and overlay binaries and Ghidra project copies. [`docs/legal.md`](docs/legal.md) forbids committing any of this. These files are listed here as they are in the tree; the maintainer should decide how to remove them from history.

**Dependency rules.** `sim` and `data` depend on nothing platform-related. Only `platform` and `apps/*` may depend on SDL3. `scripts/check-deps.sh` enforces this in CI.

## Crates and apps

| Crate / app | Package | What it does | State |
|-------------|---------|--------------|-------|
| `crates/nds` | `rescue-nds` | Reads the cartridge header (CRC-16 checked), FNT/FAT file system, ARM9/ARM7 binaries, overlay tables, BLZ decompression, and a memory map as JSON for Ghidra. Identifies dumps by SHA-1 (`rom::RomId`, `SUPPORTED_DUMPS`) | Verified on the EUR dump |
| `crates/formats` | `rescue-formats` | Parsers for Blue's formats: pack archives, SIR0, AT4PX/PKDPX (PX), Nitro LZ10/LZ11/Huffman/RLE, 4bpp/8bpp tiles, RGBX palettes, portraits, sprites, effects, tilesets, ground maps, text and fonts | Verified (see [Game data we can read](#game-data-we-can-read)) |
| `crates/data` | `rescue-data` | Typed species, moves, items, learnsets, level growth, dungeon floors, type chart | Verified; equal to Red's values field by field |
| `crates/sim` | `rescue-sim` | Deterministic simulation. Ported RNG (global LCG, dungeon LCG, preseed) and floor generation in progress | RNG bit-exact; floor generator being ported step by step |
| `crates/platform` | `rescue-platform` | SDL3 window, event loop, keyboard and gamepad input mapped to DS buttons, fixed-tick loop, two 256×192 offscreen screens composited at an integer scale | Works (hello-world level) |
| `crates/save` | `rescue-save` | Save serialization | Empty (layout known, see [`docs/formats/save.md`](docs/formats/save.md)) |
| `crates/render` | `rescue-render` | Tilemaps, sprites, palettes, animation | Empty |
| `crates/audio` | `rescue-audio` | Sound sequencer and synthesizer | Empty (later phase; SDAT confirmed) |
| `apps/game` | `rescue-game` (bin `rescue`) | Main executable | Placeholder |
| `apps/asset-viewer` | `rescue-asset-viewer` | egui viewer for every resource kind, plus `index build` and `index search` | Working |
| `apps/trace-harness` | `rescue-trace-harness` | Golden-trace replay and state diff | Placeholder |
| `tests` | `rescue-tests` | Cross-crate tests, the `rom_test!` macro and the ROM gate | Working |

The `sim` and `data` crates use integer and fixed-point math only. Their `lib.rs` and `clippy.toml` deny `f32`, `f64`, `HashMap`, `HashSet`, clocks and environment access, and `scripts/check-determinism.sh` checks it in CI.

## Game data we can read

Everything below is parsed from the EUR dump and checked against it. Extracted data is never committed.

### Container and file system

- **Dump layout.** ARM9 binary, ARM7 binary, 145 ARM9 overlays, and 24 named files in one flat directory. The file-survey tool lists 171 entries, tagged known, hypothesis or unknown (1 known, 155 hypothesis, 15 unknown). See [`docs/file-survey.md`](docs/file-survey.md).
- **Pack archives** (`dungeon`, `effect`, `ground`, `monster`, `ornament`, `sample`, `system`, `titlemenu`, `.sbin`). A table of 16-byte entries (8-byte name, offset, size). All 8 parse, with 154 AT4PX and 1,715 SIR0 sub-files. See [`docs/formats/archive.md`](docs/formats/archive.md).
- **SIR0** wrapper with a relocation pointer list in big-endian base-128 varints.
- **Sound.** `sound.sbin` is a Nintendo SDAT container with 98 SSEQ, 83 SBNK, 4 SWAR and 1 SSAR sub-files. Stream count is open.

### Compression

| Codec | Used by Blue? | State |
|-------|---------------|-------|
| BLZ ("backwards LZ") | Yes: all 145 overlays (flag bit 24); whether `arm9.bin` is BLZ-compressed is decided from the dump | Decoded; overlays 0, 1 and 7 hold the code analysed in Ghidra |
| PX: AT4PX | Yes: portraits, sprites, tilesets, title and halt screens, level growth | Verified: all 154 AT4PX sub-files decode to their stored size |
| PX: PKDPX | Not in this dump | Implemented from the same rule |
| Nitro LZ10, LZ11, Huffman, RLE | Not found in this dump | Unit-tested with hand-built vectors only |
| Ground BPC tiles | Yes | Decoded with a custom byte-command scheme (see below) |

`message_*.str` files are **not** compressed. Their first word is an offset table.

### Graphics

- Tiles: 4bpp (32 bytes, low nibble on the left) and 8bpp. Tilemap entries: tile bits 0–9, flips at bits 10–11, palette bank at bits 12–15.
- Palettes: Blue stores **RGBX** (4 bytes per colour: `r, g, b, x`), not the BGR555 the hardware uses. Colour 0 is transparent for sprites and tiles.
- Verified against Blue: 4bpp tiles, tilemap entries and RGBX palettes (portraits, sprites and tilesets render correctly).

### Data tables (`crates/data`)

| Table | Where | Count and layout | Verified |
|-------|-------|------------------|----------|
| Species | `system.sbin` `monspara` | 424 × 0x48-byte entries | All numeric fields and names equal Red's |
| Moves | `system.sbin` `wazapara` | 413 × 0x24-byte entries; learnsets (8 bytes per species) | Numeric fields equal Red's; move-table layout compared in part |
| Items | `system.sbin` `itempara` | 240 × 0x20-byte entries | Numeric fields equal Red's |
| Level growth | `system.sbin` `lvmpNNN` | 421 files, 100 × 12-byte `LevelData` each | All 421 equal Red's |
| Dungeon floors | `dungeon.sbin` `mapparam` | 64 dungeons, 1,831 floors, 1,764 floor-property records, 839 monster lists, 148 trap lists, 178 item lists | All tables equal Red's (a header-pointer miscount was fixed: 183 → 178) |
| Type chart | ARM9 overlay 1, `0x72688` | 18 × 18 `s16`, values 0–3 | Equal to Red's `gTypeEffectivenessChart` |
| Translations | `wazapar{f,g,i,s}`, `itempar{f,g,i,s}` | Same tables with translated text | Blue only |

Move streams: a move ID is one byte, or two when bit 7 of the first is set. Level-up streams add a level byte after each ID; 0 ends a stream.

### Text and fonts

- **Message tables** `message_e`, `_f`, `_g`, `_i`, `_s` (English, French, German, Italian, Spanish). 3,336 strings each. `.str` and `.bin` agree string for string.
- **Encoding.** Latin-1 for single bytes; `0x0A` is a line break. `0x81`–`0x9F` start two-byte game glyphs (button icons, symbols).
- **Control codes** typed as `text::Token`: colour (`#C`, `#R`, `#c`, `#r`), page and wait (`#P`, `#W`), cursor (`#=`, `#y`), and variables (`$m` monster, `$i` item, `$d`/`$n` numbers, `$t` team name, `$h` friend area). Several codes are still unidentified.
- **Fonts.** `kanji_a` and `kanji_b` are variable-width 12×12 dialogue fonts (473 glyphs in `kanji_a`). `font*`, `hpNfont*` and `fontsp` are fixed 8×8 fonts.

### Sprites, portraits, effects, tilesets, maps

- **Creature sprites** (`monster.sbin` `axNNN`): 423 files, all parse and render. Each has frames of OAM-style pieces, images, per-frame reference points and animation groups per direction. Direction order: 0 south (facing the viewer), then clockwise. Every group is documented in [`docs/formats/sprites.md`](docs/formats/sprites.md).
- **Portraits** (`kaoNNN`): 73 files. 13 emotion slots, 40×40 AT4PX images with a 16-colour palette per slot.
- **Effects** (`effect.sbin`): 139 `efob` effect objects and 8 `efbg` backgrounds. Frame meanings are open.
- **Dungeon tilesets** (`dungeon.sbin` `bNN*`): 76 tilesets, each with 8×8 font tiles, 24×24 metatiles, palettes, an autotile table and palette animation. Autotile bit order is a hypothesis.
- **Ground maps** (`ground.sbin`): BPL palettes, BPC tiles and chunks (a custom compression), BPA animated tiles, BMA chunk maps with data and collision layers. Maps render; BPA frame info and layer priority are open. 14 maps use no tiles of their own.
- **Halt screens** (`nitro_haltscreen*.sbin`) decode as 8bpp-indexed images; their pixel layout is not pinned down.

## Reverse engineering

The original ARM9 code is the ground truth. Notes live in [`docs/re/`](docs/re/) and every named address is listed in [`docs/symbols.md`](docs/symbols.md). Each finding is tagged:

- **blue-verified**: the role was read in Blue's code, or confirmed by an emulator trace.
- **red-hypothesis**: taken from `pmd-red`, not yet checked in Blue.
- **sdk-hypothesis**: a NitroSDK function, recognised by its behaviour.
- **tentative**: our own name, role only partly known.

### Toolchain

- **Extraction.** [`tools/nds_extract_code.py`](tools/nds_extract_code.py) checks the dump's SHA-1, writes `arm9.bin`, every overlay (BLZ-decoded when flagged) and a `manifest.json` memory map. It reads only the dump you point it at and writes to a git-ignored folder.
- **Import.** [`tools/ghidra/import.sh`](tools/ghidra/import.sh) (or `import.ps1`) runs headless Ghidra and maps the ARM9 and every overlay at their real addresses (`RescueNdsImport.java`). The result is 154 blocks: 145 overlays plus the ARM9 segments.
- **Analysis scripts** in `tools/ghidra/`:
  - `RescueOverlayDiscover.java` finds code the first analysis missed in overlays 0, 1 and 7.
  - `RescueApplySymbols.java` applies the rows of `docs/symbols.md` to the project and checks them.
  - `RescueRngLabels.java` names the RNG functions after checking their constants.
  - `RescueCallers.java`, `RescueNdsReport.java` answer caller and summary queries.
- **Querying.** [ghidra-cli](https://github.com/akiselev/ghidra-cli) v0.2.2 runs Ghidra as a resident bridge for function search, xrefs, decompilation (ARM and Thumb), comments and tags. Setup: [`docs/ghidra-setup.md`](docs/ghidra-setup.md).
- **Emulator.** `verification/v7-trace/` drives `py-desmume` to boot the retail dump, play a fixed input sequence and dump the dungeon state at every fractional turn. Output is byte-identical across runs.

### Memory map and execution model

- The ARM9 binary runs the boot code, `GameLoop_Async` (`0201F2E4`), the title loop, the save code and the mode dispatch.
- **Overlays.** Only three of the 145 overlays hold code. Overlays 0 (ground mode, towns and menus) and 1 (dungeon: RNG, combat, AI, floor generation) share RAM at `0x02138860`, so only one is resident at a time. Overlay 7 is a title-menu mode that Red does not have (`RunOverlay7Mode`). Overlays 2–6 are dungeon text tables for each language at `0x021B4900`, chosen by `GetLanguage()`. The other overlays are data: pointer tables, text and records.
- **Title menu.** Choices 2, 3 (new game), 4 and 5 (overlay 7). Choice 5 is the only one with no Red counterpart.
- **Per-frame work.** Every wait loop calls `MainLoops_RunFrameActions` (`0201F268`). Whether the global RNG advances per frame is still open; Red does it, and Blue's path does not reach the RNG within four call levels.

### Random number generators

Blue has three generators. All three match Red's constants and behaviour ([`docs/re/rng.md`](docs/re/rng.md)).

| Generator | State | Step | Seeded by |
|-----------|-------|------|-----------|
| Global LCG | `sPRNGState`, `020DF704` | `s = s * 0x5D588B65 + 1`, output `s >> 16` | `SeedRng` at boot from the **console MAC address** (Red uses constants); `SetRNGSeed` from the save |
| Dungeon LCG | `gDungeonRngState`, `ov_0001::021B3F58` | same step | `InitDungeonRNG(floor seed)` once per floor |
| Dungeon preseed LCG | `gDungeonRngPreseedState`, `ov_0001::021B3F5C` | same step, two per call | `SetDungeonRNGPreseed23Bit` once per dungeon run |
| Mersenne Twister | `sStateArray`, `020DED24` | MT19937 | `MersenneTwister_InitializeState`; its two callers are not identified yet |

The seed chain was checked on the retail dump by emulator: the dungeon RNG state is reachable from the previous one with the LCG step in every consecutive pair, and the first floor's seed equals `GenerateDungeonRNGSeed` applied to the run preseed.

Key routines:

- `DungeonRandInt` `ov_0001::0219D058`, `DungeonRandRange` `ov_0001::0219D014`, `DungeonRandOutcome` `ov_0001::0219CFCC`.
- `Rand16Bit` `020119A4`, `RandInt` `02011968`, `RandRange` `02011924`.
- Floor seed: `GenerateDungeonRNGSeed` `ov_0001::0219D0C4` returns `(hi16 of step 2) << 16 | (hi16 of step 1)`, masked to 24 bits, with bit 0 set.

### Dungeon state

- `gDungeon` (`0x020E0788`) holds the dungeon struct pointer.
- Fields used by the golden traces: `fractionalTurn` at `+0x650`, mission kind at `+0x668`, run preseed at `+0x66C`, floor seed at `+0x670`. Blue's `unk644` sub-struct starts at `0x638`; its fields from `+0x14` on sit 4 bytes earlier than in Red. Full map: [`docs/re/structs.md`](docs/re/structs.md).
- Entity info pointer is `entity+0x70`; `EntityInfo.types` is at `+0x56` (Red: `+0x5C`).

### Turn loop

[`docs/re/turns.md`](docs/re/turns.md). `RunDungeon_Async` (`0206DC20`) loops `RunFractionalTurn_Async` (`ov_0001::021411BC`) until `IsFloorOver` (`ov_0001::0214067C`). One fractional turn spawns monsters, runs the leader's turn, runs the other monsters' turns, and advances `fractionalTurn`, which cycles through 0–23. The leader acts when `gSpeedTurns[CalcSpeedStage(leader)][fractionalTurn]` allows it. This is the V7 trigger.

### Combat

[`docs/re/combat.md`](docs/re/combat.md). `CalcDamage` (`ov_0001::0217BD94`) matches Red's `CalcDamage` by constants (damage cap 999, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`), by argument count and by RNG call order. Four tables match Red by value: the 18×18 type chart, the 4×4 effectiveness combination table, and the attack and defence stat-stage multipliers. Wonder Guard is ability `0x35`.

### Floor generation

[`docs/re/dungeon-generation.md`](docs/re/dungeon-generation.md). `GenerateFloor` (`ov_0001::02151E7C`) picks a layout from `layout & 0xF` (large, small, one room with monster house, outer ring, crossroads, two rooms, line, cross, beetle, outer rooms, medium), then runs the room, hallway, maze, shop, monster-house, imperfection, secondary-structure and spawn steps. The Rust port in `crates/sim` is being checked step by step against `verification/v7-trace/floor_stages.py` (RNG state and arguments at 26 checkpoints). Its status is tracked in the roadmap's Phase 3 section; the open work is: the rest of the driver, the `ResetFloor` body, and the fixed-room placements, which read game data and stay behind the `FloorSteps` trait.

### Save format

[`docs/formats/save.md`](docs/formats/save.md). Backup media is 2 Mbit (256 KiB) serial flash in 4 KiB sectors (`CARD_IdentifyBackup(0x1202)`). The main save is a `0x57D8`-byte block written twice (sectors 0–5 and 6–11), with a word-sum checksum, the magic `0x06061320`, a Blue-only language field, and eight bit-packed sections in the same order and widths as Red's `src/save.c`. Not yet checked against a real save. The quicksave sector is open.

## Verification

The Phase 0 log in [`docs/verification.md`](docs/verification.md) records each check with its date, evidence and outcome.

| ID | Assumption | Outcome |
|----|------------|---------|
| V1 | `sdl3` builds and links with renderer, gamepad and audio APIs | Confirmed (Linux and Windows MSVC; gamepad on real hardware not yet tried) |
| V2 | egui, insta, proptest, cargo-nextest and cargo-fuzz on stable, edition 2024 | Partial: egui, insta and proptest confirmed; nextest and fuzz runs pending |
| V3 | Blue's audio uses SDAT | Confirmed |
| V4 | Non-audio formats identified, save media identified | Confirmed |
| V5 | `pmd-red` covers dungeon, combat, AI and RNG | Confirmed, with naming gaps |
| V6 | Blue reuses Red's logic | Confirmed: 4 tables by value; RNG step and damage calc constants match |
| V7 | A DS emulator can be scripted to dump RAM and RNG per turn | Confirmed on the retail dump; byte-identical over 3 runs |
| V8 | Ghidra loads ARM9 and overlays at their real addresses | Confirmed: 145 overlays mapped; cross-references resolve |
| V9 | REA supports the Ghidra workflow on ARM DS | Replaced by ghidra-cli |

## Blue and Red

Red Rescue Team (GBA, `pmd-red`) is a useful map, because Blue reuses much of its logic. Blue is still the ground truth. The differences are recorded in [`docs/blue-vs-red.md`](docs/blue-vs-red.md).

| Area | Same as Red | Different in Blue |
|------|-------------|-------------------|
| RNG | LCG step and constants; dungeon wrappers; floor seed | Global LCG seeded from the console MAC; MT19937 users unknown |
| Execution | `GameLoop_Async` call order | Language selection; title-menu choice 5 (overlay 7); a 4 KiB memory check before the save |
| Combat | Damage formula constants, RNG order, four tables | Entity info is 6 bytes earlier |
| Save | Eight sections, bit-packed serializer, field widths | 2 Mbit flash; block 4 bytes longer; language field; different magic |
| Data | Species, move, item, level growth and floor values (numeric fields) | Translated text tables (`wazapar*`, `itempar*`); some species bytes Red does not have (`0x0A`–`0x0B`, `0x46`–`0x47`) |
| Graphics and audio | Red's GBA formats are not used | Nintendo DS formats: RGBX palettes, AT4PX, SDAT audio; 256×192 dual screen with touch |

Red-derived facts stay hypotheses until Blue's ARM9, its overlays or an emulator trace confirms them.

## Engineering rules

From [`docs/roadmap.md`](docs/roadmap.md#engineering-principles):

- **Deterministic simulation.** `rescue-sim` uses integer and fixed-point maths only, one seeded RNG owned by the sim, and no `HashMap` iteration in logic. It advances purely from input commands, so it runs in CI without a window.
- **Parsers never panic.** Formats are parsed from `&[u8]` with explicit bounds checks that return `Result`. The fuzz targets and `tests/tests/parsers_no_panic.rs` exercise this.
- **Golden traces.** Emulator runs dump entity and RNG state per turn. Traces are language-neutral fixtures with documented fields only, never raw RAM images.
- **Layering.** Reverse-engineering notes, extraction, the simulation and the platform layer stay independent and testable on their own.
- **Lints.** `unsafe_op_in_unsafe_fn` and `unused_must_use` are denied; `clippy::all`, `dbg_macro`, `todo`, `unimplemented` and `undocumented_unsafe_blocks` warn. The data, formats, nds, sim and save crates use `#![forbid(unsafe_code)]`; the platform layer is the only place that may need `unsafe` (for SDL).
- **Formatting.** `rustfmt.toml`: edition 2024, max width 100.

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) first. In short:

- Plans start from [`docs/templates/plan.md`](docs/templates/plan.md) and RE findings from [`docs/templates/re-finding.md`](docs/templates/re-finding.md).
- Every claim gets evidence: an address, a file, a trace or a command. Mark facts as verified in Blue or as hypotheses.
- Before committing, check `git status` and the diff for anything `.gitignore` might miss. If game data gets committed, tell the maintainer rather than deleting it in a follow-up commit.
- `docs/` is an [Obsidian](https://obsidian.md) vault. Open the `docs/` folder as the vault. Write links as standard Markdown, not `[[wikilinks]]`, so they read on GitHub too.

## Documentation map

Start at [`docs/index.md`](docs/index.md).

| Topic | Where |
|-------|-------|
| Roadmap and phases | [`docs/roadmap.md`](docs/roadmap.md) |
| Verification log (V1–V9) | [`docs/verification.md`](docs/verification.md) |
| Legal policy | [`docs/legal.md`](docs/legal.md) |
| Supported dumps | [`docs/supported-dumps.md`](docs/supported-dumps.md) |
| ROM-gated tests | [`docs/rom-tests.md`](docs/rom-tests.md) |
| Ghidra setup and ghidra-cli | [`docs/ghidra-setup.md`](docs/ghidra-setup.md) |
| Symbol table | [`docs/symbols.md`](docs/symbols.md) |
| Format specs | [`docs/formats/README.md`](docs/formats/README.md) |
| File survey | [`docs/file-survey.md`](docs/file-survey.md) |
| RE notes | [`docs/re/`](docs/re/) |
| Blue vs Red deltas | [`docs/blue-vs-red.md`](docs/blue-vs-red.md) |
| External references and trust | [`docs/references.md`](docs/references.md) |
| Plans | [`docs/plans/README.md`](docs/plans/README.md) |
| Progress treemaps | [`docs/progress/README.md`](docs/progress/README.md) |

Progress maps are regenerated by `python scripts/progress_treemap.py` (code size) and `python scripts/port_coverage_treemap.py` (Blue function coverage: 178 functions at v0).

## Open items

Collected from the roadmap and the format specs:

- **Reverse engineering:** name the three end-of-turn functions; sort the 176 `DungeonRandInt` call sites by subsystem; identify the two Mersenne Twister users; find the 16-bit generator from Red's `other_random.c` (multiplier 109, increment 1021); confirm per-frame RNG behaviour by emulator; name the 19 `MainLoops_RunFrameActions` calls; locate the quicksave sector; map overlays 8 and 9; decide what title-menu choice 5 does.
- **Floor generation:** finish the driver and the `ResetFloor` body; decode the fixed-room placements; port the enemy spawns and the stairs marker.
- **Formats:** ground collision and data layers, BPA frame info and layer priority; sprite group meanings in the table above, checked against more species; autotile neighbour bit order (E and W) and the 48 trailing bytes of `cex`; background frame meanings in `efbg`; `ornament.sbin`, `titlemenu.sbin`, `sample.sbin` and the halt-screen layout; text codes `#1`–`#9` and `$v`, and which code changes the portrait; unnamed bytes in species, move and item entries; dungeon names and floor counts.
- **Save:** check every section against a real save (local only); name the fields of the remaining sections.
- **Audio:** stream count from the SDAT `INFO` block.
- **Engine:** rendering, screen layout, input, audio and the trace harness are not written yet.
- **Verification:** run `cargo-nextest` on both OSes and `cargo-fuzz` on Linux (V2); try gamepad input on real hardware (V1).
