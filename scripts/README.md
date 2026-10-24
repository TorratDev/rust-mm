# Scripts

## Golden-trace capture (`capture_golden.py`)

Records a golden trace from your own retail dump, in the format of [`docs/golden-traces.md`](../docs/golden-traces.md). It drives the DeSmuME emulator through py-desmume and reuses [`verification/v7-trace`](../verification/v7-trace) for the emulator, the input files and the row reader. After writing, it reads the file back with the `rescue-trace` crate (through [`trace_check`](trace_check)), and it fails if the read-back differs.

### Windows setup

1. Python 3.12 (3.11 and later should also work; the scripts were run on 3.12).
2. Install the emulator bindings: `py -m pip install py-desmume==0.0.9`.
3. Rust, with the toolchain in [`rust-toolchain.toml`](../rust-toolchain.toml). The read-back builds `trace_check` the first time it runs.
4. Your dump, SHA-1 `6f01887356268de39085df01490347bbcc3aa330` (see [`docs/supported-dumps.md`](../docs/supported-dumps.md)). Keep it outside the repository, or in the git-ignored `roms/` folder.
5. Set `SDL_VIDEODRIVER=dummy` and `SDL_AUDIODRIVER=dummy` in the shell, so the emulator runs without a window and without sound.

### Use

Make a save state at the first dungeon floor (once per ROM, about 80 seconds):

```
py scripts\capture_golden.py C:\roms\dump.nds --make-state C:\work\floor1.sav
```

Record a trace from that state (the turn inputs from `verification/v7-trace/turn_inputs.txt`, 3000 frames):

```
py scripts\capture_golden.py C:\roms\dump.nds C:\work\run1.jsonl --state C:\work\floor1.sav --scenario turn_inputs_a30
```

Run it again with another output name and compare the two files. They must be identical. Output and states hold decoded state from your dump, so keep them out of the repository. The state file's `.json` sidecar records the boot inputs that reached the dungeon.

Without `--state`, the boot inputs are replayed from power-on each time. That takes about 80 seconds longer and gives the same trace.

### Self-test

The capture script checks each file it writes: `trace_check` reads it with `rescue_trace::read_trace`, and the script compares the row count, the ROM SHA-1 and the schema version with what it wrote. To run the reader alone on a file:

```
scripts\trace_check\target\debug\trace_check.exe C:\work\run1.jsonl
```
