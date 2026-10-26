#!/usr/bin/env python3
"""Per-frame global RNG probe on the supported Blue dump (#159 P1).

Logs, for every emulated frame, the global LCG state (sPRNGState, 020DF704), the
number of hits on the frame routine (MainLoops_RunFrameActions, 0201F268), the
callers of Rand32Bit (0201198C) and the conditional flag word read at 02018B24.
Output is JSON Lines, one object per frame, with sorted keys, so two runs of the
same input give the same bytes.

Modes:
  windows (default): boot to the first dungeon floor, then play the windows
      idle / A held / idle / Select held / idle (see WINDOWS). With --state, the
      run starts from a save state at the dungeon instead of booting.
  plain: boot inputs only, from power-on, for --frames frames.

Usage:
  rng_per_frame.py ROM OUT [--mode windows|plain] [--frames N] [--state FILE]

ROM is your own dump (supported SHA-1 only, see docs/supported-dumps.md). Nothing
from the dump is written to OUT except the logged state values. Keep OUT out of the
repository. The hash of OUT is printed at the end; see docs/verification.md.
"""

import argparse
import hashlib
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

import retail_trace as rt  # noqa: E402  (addresses and the input parser)
from desmume.controls import Keys, keymask  # noqa: E402
from desmume.emulator import DeSmuME  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
S_PRNG = 0x020DF704  # global LCG state
FRAME_ROUTINE = 0x0201F268  # MainLoops_RunFrameActions
RAND32 = 0x0201198C  # Rand32Bit
FLAG_PTR = 0x02018B24  # literal: pointer to the flag word tested with 0xC2
# Frames after the dungeon starts: (frame, keymask). Idle, A held, idle, Select held, idle.
WINDOWS = [(0, 0), (300, "A"), (600, 0), (900, "SELECT"), (1200, 0)]
WINDOW_LENGTH = 1500


def sha1_of(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--mode", choices=["windows", "plain"], default="windows")
    p.add_argument("--frames", type=int, default=27000, help="total frames (plain: boot frames)")
    p.add_argument("--state", help="save state at the dungeon to start from (windows mode)")
    a = p.parse_args()
    if a.state and a.mode != "windows":
        p.error("--state is only for windows mode")
    if sha1_of(a.rom) not in SUPPORTED_SHA1:
        sys.exit("not a supported dump (see docs/supported-dumps.md)")

    emu = DeSmuME()
    emu.open(a.rom)
    ram = rt.Ram(emu)
    counts = {"hits": 0, "callers": {}}

    def on_frame(*_):
        counts["hits"] += 1

    def on_rand32(*_):
        key = hex(emu.memory.register_arm9.lr)
        counts["callers"][key] = counts["callers"].get(key, 0) + 1

    emu.memory.register_exec(FRAME_ROUTINE, on_frame)
    emu.memory.register_exec(RAND32, on_rand32)

    out = open(a.out, "w", encoding="utf-8", newline="\n")
    digest = hashlib.sha256()

    def write(obj):
        line = json.dumps(obj, sort_keys=True, separators=(",", ":")) + "\n"
        out.write(line)
        digest.update(line.encode("utf-8"))

    def log(frame, phase):
        flag_at = ram.u32(FLAG_PTR)
        write({"f": frame, "ph": phase, "s": ram.u32(S_PRNG), "hits": counts["hits"],
               "r32": counts["callers"],
               "flag": ram.bytes(flag_at, 2).hex() if flag_at else None})
        counts["hits"] = 0
        counts["callers"] = {}

    if a.mode == "plain":
        boot = rt.parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
        for f in range(a.frames):
            if f in boot:
                emu.input.keypad_update(boot[f])
            emu.cycle(with_joystick=False)
            log(f, "boot")
        write({"ev": "end", "frames": a.frames})
    else:
        keys = {"A": keymask(Keys.KEY_A), "SELECT": keymask(Keys.KEY_SELECT)}
        windows = {frame: (keys[name] if isinstance(name, str) else 0) for frame, name in WINDOWS}
        dungeon_at = None
        if a.state:
            emu.savestate.load_file(a.state)
            dungeon_at = 0
            write({"ev": "state", "file": os.path.basename(a.state)})
        else:
            boot = rt.parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
            f = 0
            while True:
                if f in boot:
                    emu.input.keypad_update(boot[f])
                emu.cycle(with_joystick=False)
                if f % 10 == 0 and ram.u32(rt.G_DUNGEON) != 0:
                    dungeon_at = f
                    break
                log(f, "boot")
                f += 1
                if f >= a.frames:
                    sys.exit("no dungeon reached within --frames")
        write({"ev": "dungeon", "f": dungeon_at})
        for rel in range(WINDOW_LENGTH):
            if rel in windows:
                emu.input.keypad_update(windows[rel])
            emu.cycle(with_joystick=False)
            log(rel, "dungeon")
        write({"ev": "end", "frames": WINDOW_LENGTH})

    out.close()
    emu.destroy()  # libdesmume ends the process here, so it comes last
    print(f"sha256 {digest.hexdigest()}", file=sys.stderr)
    print(f"file sha256 {hashlib.sha256(open(a.out, 'rb').read()).hexdigest()}", file=sys.stderr)


if __name__ == "__main__":
    main()
