#!/usr/bin/env python3
"""OtherRandom16 probe on the supported Blue dump (#169 P1).

Boots with the harness boot inputs (boot_to_dungeon.txt), which pass through ground
mode, and hooks OtherRandom16 (02023B9C): at entry it logs the state (u16 at
020E03F4) and the caller (lr); at 02023BB8 (the instruction after the final ldrh) it
logs the state and r0, the return value. Output is JSON Lines, sorted keys, so two
runs give the same bytes.

Usage:
  other_rng.py ROM OUT [--frames N]

ROM is your own dump (supported SHA-1 only, see docs/supported-dumps.md). Keep OUT
outside the repository. The sha256 of OUT is printed; see docs/verification.md.
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

import retail_trace as rt  # noqa: E402  (Ram and the input parser)
from desmume.emulator import DeSmuME  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
ENTRY = 0x02023B9C  # OtherRandom16
EXIT = 0x02023BB8  # after `ldrh` at 02023BB4: r0 is the return value
STATE = 0x020E03F4  # gOtherRngState (u16)


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=2700)
    a = p.parse_args()
    h = hashlib.sha1()
    with open(a.rom, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    if h.hexdigest() not in SUPPORTED_SHA1:
        sys.exit("not a supported dump (see docs/supported-dumps.md)")

    emu = DeSmuME()
    emu.open(a.rom)
    ram = rt.Ram(emu)
    frame = [0]
    out = open(a.out, "w", encoding="utf-8", newline="\n")
    digest = hashlib.sha256()

    def write(obj):
        line = json.dumps(obj, sort_keys=True, separators=(",", ":")) + "\n"
        out.write(line)
        digest.update(line.encode("utf-8"))

    def state():
        return struct.unpack("<H", ram.bytes(STATE, 2))[0]

    def on_entry(*_):
        write({"ev": "in", "f": frame[0], "lr": hex(emu.memory.register_arm9.lr), "s": state()})

    def on_exit(*_):
        write({"ev": "out", "f": frame[0], "r0": emu.memory.register_arm9.r0, "s": state()})

    emu.memory.register_exec(ENTRY, on_entry)
    emu.memory.register_exec(EXIT, on_exit)
    boot = rt.parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
    for f in range(a.frames):
        frame[0] = f
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
    write({"ev": "end", "frames": a.frames})
    out.close()
    print(f"sha256 {digest.hexdigest()}", file=sys.stderr)
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    main()
