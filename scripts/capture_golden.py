#!/usr/bin/env python3
"""Record a golden trace (schema v1, docs/golden-traces.md) from the retail ROM.

Plays a scripted input sequence in DeSmuME (py-desmume) and writes one JSON Lines
row each time RunFractionalTurn_Async runs. The emulator and the row reader are the
ones in verification/v7-trace (pydesmume_dump.py, retail_trace.py); this script adds
the header, the boot seed and the self-test.

Two steps:

  1. Make a save state at the first dungeon floor, once per ROM:
       capture_golden.py ROM --make-state STATE.sav

  2. Record a trace, replaying the turn inputs from the save state:
       capture_golden.py ROM OUT.jsonl --state STATE.sav --scenario NAME

Without --state the boot inputs are replayed from power-on instead (about 80 s).

The header inputs.boot is the input list that reached the dungeon (from the state's
sidecar file, or the boot inputs file). The boot seed is the 6 bytes at 027FFCF4,
read before any input is applied (see docs/re/rng.md).

Only the supported dump is accepted (docs/supported-dumps.md). The output holds
decoded state from the user's own dump, so keep it out of the repository.

After writing, the file is read back with the rescue-trace crate (scripts/trace_check)
and compared with what was written. A mismatch exits with status 1.

Usage:
  capture_golden.py ROM --make-state STATE [--boot-inputs FILE]
  capture_golden.py ROM OUT [--state STATE] [--scenario NAME] [--turn-inputs FILE]
                    [--frames N]
"""

import argparse
import hashlib
import importlib.metadata
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
V7 = os.path.join(REPO, "verification", "v7-trace")
CHECKER = os.path.join(HERE, "trace_check")
sys.path.insert(0, V7)
import retail_trace as rt  # noqa: E402  (addresses, RAM reader, row snapshot, input parser)
from desmume.emulator import DeSmuME  # noqa: E402

SCHEMA_VERSION = 1
SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330": "BlueRescueTeamEur"}
SEED_SOURCE = 0x027FFCF4  # 6 bytes, the console MAC address copied at boot (docs/re/rng.md)
SEED_LEN = 6
EMULATOR_NAME = "DeSmuME (py-desmume)"
LIBDESMUME_VERSION = "0.9.12"  # the version libdesmume prints at startup ("DeSmuME 0.9.12")


def file_sha1(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_input_lines(path):
    """Input lines in the header format: 'FRAME KEY ...', no comments, no blank lines."""
    lines = []
    with open(path, encoding="utf-8") as f:
        for raw in f:
            words = raw.split("#", 1)[0].split()
            if words:
                lines.append(" ".join(words))
    return lines


def first_input_frame(lines):
    return min(int(line.split()[0]) for line in lines)


def read_seed(emu, before_frame):
    """Run frames with no input until the boot seed is set; it must be set before any input."""
    ram = rt.Ram(emu)
    for f in range(before_frame):
        emu.cycle(with_joystick=False)
        seed = bytes(ram.u8(SEED_SOURCE + i) for i in range(SEED_LEN))
        if seed != bytes(SEED_LEN):
            return seed, f
    sys.exit(f"boot seed at {SEED_SOURCE:08X} stayed 0 before the first input frame")


def boot_to_dungeon(emu, boot_inputs, start_frame, max_frames):
    """Replay boot inputs from power-on (absolute frame numbers) until the dungeon exists."""
    ram = rt.Ram(emu)
    for f in range(start_frame, max_frames):
        if f in boot_inputs:
            emu.input.keypad_update(boot_inputs[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(rt.G_DUNGEON) != 0:
            return f
    sys.exit("no dungeon reached; check the ROM and the boot inputs")


def emulator_version():
    return f"py-desmume {importlib.metadata.version('py-desmume')} (libdesmume {LIBDESMUME_VERSION})"


def dumps(obj):
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def make_state(args, sha1):
    boot_path = args.boot_inputs
    boot_lines = read_input_lines(boot_path)
    boot_map = rt.parse_inputs(boot_path)
    emu = DeSmuME()
    emu.open(args.rom)
    seed, seed_frame = read_seed(emu, first_input_frame(boot_lines))
    dungeon_frame = boot_to_dungeon(emu, boot_map, seed_frame + 1, args.max_boot_frames)
    emu.savestate.save_file(args.make_state)
    with open(args.make_state + ".json", "w", encoding="utf-8") as f:
        json.dump({"rom": {"sha1": sha1}, "bootInputs": boot_lines,
                   "initialSeed": seed.hex(), "dungeonFrame": dungeon_frame}, f, indent=1)
    print(f"state saved at dungeon frame {dungeon_frame}: {args.make_state}", file=sys.stderr)
    emu.destroy()


def record(args, sha1):
    if args.state:
        with open(args.state + ".json", encoding="utf-8") as f:
            side = json.load(f)
        if side["rom"]["sha1"] != sha1:
            sys.exit("the state was made from a different ROM")
        boot_lines = side["bootInputs"]
    else:
        boot_lines = read_input_lines(args.boot_inputs)
    turn_lines = read_input_lines(args.turn_inputs)
    turns = rt.parse_inputs(args.turn_inputs)

    emu = DeSmuME()
    emu.open(args.rom)
    seed, seed_frame = read_seed(emu, first_input_frame(boot_lines))
    if args.state:
        emu.savestate.load_file(args.state)
    else:
        boot_to_dungeon(emu, rt.parse_inputs(args.boot_inputs), seed_frame + 1, args.max_boot_frames)

    ram = rt.Ram(emu)
    rows, tiles, frame = [], {}, [0]

    def hook(*_):
        dungeon = ram.u32(rt.G_DUNGEON)
        if dungeon == 0:
            return
        row = rt.snapshot(ram, dungeon)
        if row["floorSeed"] != tiles.get("seed"):
            tiles["seed"] = row["floorSeed"]
            tiles["hash"] = hashlib.sha256(ram.bytes(dungeon + rt.TILES, rt.TILES_LEN)).hexdigest()[:16]
        row["tilesSha256"] = tiles["hash"]
        row["frame"] = frame[0]
        rows.append(row)

    emu.memory.register_exec(rt.TRIGGER, hook)
    for f in range(args.frames):
        frame[0] = f
        if f in turns:
            emu.input.keypad_update(turns[f])
        emu.cycle(with_joystick=False)
    if not rows:
        sys.exit("no RunFractionalTurn_Async call was seen; check the turn inputs")

    header = {
        "schemaVersion": SCHEMA_VERSION,
        "rom": {"sha1": sha1},
        "emulator": {"name": EMULATOR_NAME, "version": emulator_version()},
        "scenario": args.scenario,
        "initialSeed": {"source": f"{SEED_SOURCE:08X}", "bytes": seed.hex()},
        "inputs": {"boot": boot_lines, "turn": turn_lines},
    }
    with open(args.out, "w", encoding="utf-8", newline="\n") as out:
        out.write(dumps(header) + "\n")
        for row in rows:
            out.write(dumps(row) + "\n")

    check_with_crate(args.out, sha1, len(rows))
    print(f"{len(rows)} rows, sha256 {file_sha1(args.out)}", file=sys.stderr)
    emu.destroy()  # libdesmume ends the process here, so it comes last


def check_with_crate(path, sha1, rows):
    """Read the file back with rescue-trace and compare with what was written."""
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--release", "--manifest-path",
         os.path.join(CHECKER, "Cargo.toml"), "--", path],
        cwd=REPO, capture_output=True, text=True,
    )
    if result.returncode != 0:
        sys.exit(f"rescue-trace rejected the file:\n{result.stderr}")
    summary = json.loads(result.stdout)
    if summary["rows"] != rows or summary["romSha1"] != sha1 or summary["schemaVersion"] != SCHEMA_VERSION:
        sys.exit(f"read-back mismatch: wrote {rows} rows for {sha1}, read {summary}")


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out", nargs="?")
    p.add_argument("--make-state", metavar="STATE", help="save a state at the first dungeon floor and exit")
    p.add_argument("--state", help="save state to start from (made with --make-state)")
    p.add_argument("--scenario", default="turn_inputs", help="scenario ID written to the header")
    p.add_argument("--boot-inputs", default=os.path.join(V7, "boot_to_dungeon.txt"))
    p.add_argument("--turn-inputs", default=os.path.join(V7, "turn_inputs.txt"))
    p.add_argument("--frames", type=int, default=3000, help="turn frames from the start of the run")
    p.add_argument("--max-boot-frames", type=int, default=30000)
    args = p.parse_args()

    sha1 = file_sha1(args.rom)
    if sha1 not in SUPPORTED_SHA1:
        sys.exit(f"not a supported dump (SHA-1 {sha1}); see docs/supported-dumps.md")
    if args.make_state:
        make_state(args, sha1)
        return
    if args.out is None:
        p.error("OUT is required unless --make-state is given")
    record(args, sha1)


if __name__ == "__main__":
    main()
