# Instructions for Claude

## Decompiling and inspecting the ARM9 and overlays

Use **ghidra-cli** to decompile functions, look up cross-references and read notes in the
`rescue-arm9` Ghidra project. Do not drive Ghidra by hand or use other decompilers. The
project contains the retail ARM9 and all 145 overlays at their real addresses, so
decompiling from it gives correct addresses.

- Tool: [ghidra-cli](https://github.com/akiselev/ghidra-cli) (v0.2.2 is the tested version).
- Setup and the commands: [`docs/ghidra-setup.md`](docs/ghidra-setup.md#6-ghidra-cli).
- Record what you find as evidence in `docs/re/` (addresses, xrefs, and emulator checks marked
  apart from guesses), as described in [`docs/roadmap.md`](docs/roadmap.md).
