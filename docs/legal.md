# Legal hygiene policy

This project reimplements a commercial Nintendo DS game. The repository must never contain the game, anything taken from it, or its trademarks in project names. These rules apply to every commit, issue attachment, CI artifact and release.

## Project name

The project, crate, binary and package prefix is **`rescue`**. It contains no "Pokémon" and no Nintendo, Game Freak, Creatures or Spike Chunsoft trademark.

- Crates: `rescue-nds`, `rescue-formats`, `rescue-data`, `rescue-sim`, `rescue-save`, `rescue-audio`, `rescue-render`, `rescue-platform`.
- Binaries: `rescue` (game), `rescue-asset-viewer`, `rescue-trace-harness`.
- Release archives and installers use the same prefix.

The game's title may appear in documentation and source comments only to say which game a fact describes (for example "Blue Rescue Team (NDS)"). It never appears in crate, binary, package, window-title or release-file names.

## Rules

1. **Users supply their own dump.** The project never distributes a ROM, a patch against one, or a link to one. Users supply a dump of a cartridge they own, made by themselves.
2. **Dumps are identified by SHA-1.** The tools accept only dumps whose SHA-1 is on the supported list, and refuse anything else with a clear message (implementation: #21). Supported dumps are listed in [`verification.md`](verification.md#dumps-used) and, later, in the release docs.
3. **Extracted assets are never committed.** No sprites, portraits, tiles, maps, text, music, sound samples, scripts or data tables decoded from the ROM. This includes screenshots or crops of game graphics, hex dumps longer than a few bytes, and generated files that embed ROM data (for example a Rust `const` table copied from the ARM9 binary).
4. **Fixtures are generated at test time.** Tests that need game data read it from the user's dump, located through configuration, and skip cleanly when no dump is configured (#27). Snapshot tests store hashes, sizes or decoded metadata, not decoded asset bytes.
5. **Release binaries contain no ROM data.** The game binary loads everything from the user's dump at runtime. No asset is embedded with `include_bytes!` or similar. CI artifacts follow the same rule.
6. **Save files and savestates stay local.** Game saves (`.sav`, `.dsv`) and emulator savestates contain game RAM and must not be committed. Golden traces may be committed only when they hold documented state fields (positions, HP, RNG state, turn counters) and not raw RAM images.

## What may be committed

- Original code, written by contributors.
- Format descriptions in our own words: field offsets, sizes, magic values, algorithms.
- File names, sizes, offsets, hashes and short magic values from the dump (as in [`file-survey.md`](file-survey.md)).
- Addresses and names of functions and variables found during reverse engineering.
- Synthetic test data built from scratch (as in `verification/v7-trace`).

## Third-party references

- The pret [`pmd-red`](https://github.com/pret/pmd-red) decompilation has no license file. Use it as a read-only reference: read it, cite file names and symbols, but do not copy its code or data into this repository. Fetch it with [`tools/fetch_pmd_red.sh`](../tools/fetch_pmd_red.sh), which places it in the ignored `reference/` directory. See [`references.md`](references.md).
- Leaked or prototype builds are not used, linked or cited as evidence.

## Enforced by `.gitignore`

`.gitignore` blocks ROMs (`*.nds`, `*.gba`, `*.srl`, `roms/`), extracted output folders (`extracted/`, `extract/`, `dump/`, `dumps/`, `out/`, `assets/extracted/`), saves (`*.sav`, `*.dsv`), emulator savestates (`*.dst`, `*.ds0`–`*.ds9`, `*.mln`, `*.State`) and the local `reference/` checkout. These rules are a safety net, not a substitute for reviewing what a commit adds.

## If something slips through

Do not just delete it in a new commit: the data stays in history. Tell the maintainer, who removes it from history and force-pushes before anything else lands.
