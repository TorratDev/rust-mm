# Project home

Start here. This folder is an [Obsidian](https://obsidian.md) vault: open `docs/` as the vault. The same notes read normally on GitHub, so links use standard Markdown, not `[[wikilinks]]`.

## Plan and status
- [Roadmap](roadmap.md): goal, phases, verification gates, current Phase 3 status.
- [Verification log](verification.md): one entry per assumption (V1–V9), with evidence.
- [Plans](plans/README.md): one note per plan or decision that is too big for the roadmap.

## Reverse engineering (Blue)
- [Symbols](symbols.md): Blue function table, the universe for the coverage map.
- [Execution model](re/execution-model.md), [turns](re/turns.md), [combat](re/combat.md), [RNG](re/rng.md), [structs](re/structs.md), [dungeon generation](re/dungeon-generation.md), [ground BPC](re/ground-bpc.md).
- [Ghidra setup](ghidra-setup.md) and [references](references.md).
- [Blue vs Red](blue-vs-red.md): where the Red decomp is a hypothesis and where it is verified.

## File formats
- [Format index](formats/README.md): archive, compression, data, effects, graphics, maps, portraits, save, sprites, text, tilesets, NDS container.
- [File survey](file-survey.md): every file in the dump, tagged known, hypothesis or unknown.

## Progress
- [Progress maps](progress/README.md): generated code-size and port-coverage treemaps.

## Setup and policy
- [Legal](legal.md), [supported dumps](supported-dumps.md), [ROM tests](rom-tests.md).
- [Contributing](../CONTRIBUTING.md) and its evidence rules.

## Templates
- [Plan](templates/plan.md) and [RE finding](templates/re-finding.md). In Obsidian, use the Templates plugin to insert them.
