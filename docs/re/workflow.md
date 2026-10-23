# Reverse engineering workflow

How a subsystem note in `docs/re/` gets written: where each claim comes from, what evidence it needs, and which status it may carry. Issue: [#57](https://github.com/TorratDev/rust-mm/issues/57), epic [#5](https://github.com/TorratDev/rust-mm/issues/5). Plan: [`roadmap.md`](../roadmap.md) (V9 and Phase 3).

## Tools

- **ghidra-cli** is the tool for decompiling, cross-references and reading notes in the `rescue-arm9` Ghidra project. Setup and commands are in [`ghidra-setup.md`](../ghidra-setup.md#6-ghidra-cli). Use it for every decompile and xref, and do not drive Ghidra by hand or use another decompiler ([`CLAUDE.md`](../../CLAUDE.md)).
- **REA** was the first candidate for this job. It is **optional**. The roadmap's V9 check replaced it with ghidra-cli, so no step below depends on REA. Use it only if someone already has it set up and it adds something ghidra-cli does not.
- **Runtime checks** use the scripts in `verification/v7-trace/` with py-desmume ([`verification.md`](../verification.md#v7--scriptable-ds-emulator-for-ram-and-rng-dumps)).

## Status tags

Every claim in a note has one of three statuses.

- **blue-verified**: read in Blue's ARM9 or overlays, or confirmed by an emulator check on the retail ROM. The evidence must give an address and a routine reference or a trace reference.
- **red-hypothesis**: a fact from `pmd-red` (or from Explorers, which is out of scope) that is not yet checked in Blue. It is a guess until it has Blue evidence.
- **lead**: a pointer from an investigator's findings to something to look into. It is not a fact and has no conclusion attached.

Anything without evidence is not a fact. Open questions go in the note's "Open questions" section, not in the status column.

## Evidence

Each claim names one of these three kinds of evidence:

1. **Blue address.** A RAM or ROM address, written as `ov_NNNN::ADDRESS` for overlay code or as a plain address for the ARM9. Say which dump it came from ([`supported-dumps.md`](../supported-dumps.md)).
2. **Routine reference.** The function name, its address and a one-line paraphrase of what the code does. Do not paste decompiler output or code bytes into the repo ([`ghidra-setup.md`](../ghidra-setup.md#4-record-the-v8-result)).
3. **Trace reference.** The script in `verification/v7-trace/` that checked the claim, plus the output hash and the command. Outputs go to an ignored scratch directory, not into the repo. The note records only the hash and the command.

## Steps

1. **Pick one subsystem and its functions.** Start from the function names in [`symbols.md`](../symbols.md) and the investigators' findings.
2. **Find the code.** Use ghidra-cli to decompile the routine and to list its callers (`x-ref to`), with the commands in [`ghidra-setup.md`](../ghidra-setup.md#6-ghidra-cli). Note each address you use.
3. **Check at runtime when you can.** If a claim is about values in RAM, run a `verification/v7-trace/` script and record the hash and command. A runtime check can move a claim to **blue-verified**.
4. **Compare with Red.** Read `pmd-red` for the logic and cite it by file and function name. Do not copy code or data ([`legal.md`](../legal.md)). A `pmd-red` fact stays **red-hypothesis** until Blue confirms it.
5. **Write the note** from [`note-template.md`](note-template.md). Give each claim its own row, with its evidence and status.
6. **Check the note.** Every claim has a status. Every **blue-verified** row has an address and a routine or trace reference. Every **red-hypothesis** row says what Blue must still show.

## Which template to use

- **Subsystem notes** in `docs/re/` (the Phase 3 notes): copy [`note-template.md`](note-template.md). It holds the claim table, the addresses, the routine references and the trace checks.
- **A single finding** written up as its own page in the Obsidian vault: copy [`templates/re-finding.md`](../templates/re-finding.md), inserted with the Templates plugin ([`CONTRIBUTING.md`](../../CONTRIBUTING.md)). It uses the same three status tags.

## Rules

- **Never commit** ROM data, extracted assets, save files, `reference/` contents or emulator dumps from the retail ROM ([`legal.md`](../legal.md)). Check `git status` and the diff before each commit.
- **Stop the ghidra-cli bridge** when you finish (`ghidra --project rescue-arm9 stop`). It listens on `127.0.0.1` without authentication, and only one process may open the project at a time.
- **Links** are standard Markdown, with no `[[wikilinks]]` and no YAML frontmatter ([`CONTRIBUTING.md`](../../CONTRIBUTING.md)).
