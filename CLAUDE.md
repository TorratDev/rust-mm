# Instructions for Claude

## Decompiling and inspecting the ARM9 and overlays

Use **ghidra-cli** to decompile functions, look up cross-references and read notes in the
`rescue-arm9` Ghidra project. Do not drive Ghidra by hand or use other decompilers. The
project contains the retail ARM9 and all 145 overlays at their real addresses, so
decompiling from it gives correct addresses.

- Tool: [ghidra-cli](https://github.com/akiselev/ghidra-cli) (v0.2.2 is the tested version).
- Setup and the commands: [`docs/ghidra-setup.md`](docs/ghidra-setup.md#6-ghidra-cli).
- For read-only questions (what a function does, who refers to an address), search the text
  export in `~/rescue-re/export` first, if its `STAMP` file exists. It is ghidra-cli output
  from the same project: [`docs/ghidra-setup.md`](docs/ghidra-setup.md#7-text-export-for-read-only-questions).
  Start a bridge only for what the export cannot answer.
- Record what you find as evidence in `docs/re/` (addresses, xrefs, and emulator checks marked
  apart from guesses), as described in [`docs/roadmap.md`](docs/roadmap.md).

## Shell helpers for agents

[`docs/agent-api.md`](docs/agent-api.md) lists helpers (`source scripts/agent.sh`) for the export,
GitHub and hc sessions. Chain them in one shell command and print only the result. Create issues
with `scripts/gh-issue.sh`, never bare `gh issue create`, so the parent, blocked-by links,
labels and assignee are set, not only mentioned in the body.
Open PRs with `scripts/gh-pr.sh --closes <n>` (solves the issue) or `--refs <n>` (part of it),
never bare `gh pr create`: it puts the `Closes`/`Refs` line first, assigns `@me`, and rejects
a Refs body that would close the issue.
