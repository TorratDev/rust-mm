#!/usr/bin/env python3
"""OtherRandom16 probe on the supported Blue dump (#169 P1).

Boots with the harness boot inputs (boot_to_dungeon.txt), which pass through ground
mode, and hooks OtherRandom16 (02023B9C): at entry it logs the state (u16 at
020E03F4) and the caller (lr); at 02023BB8 (the instruction after the final ldrh) it
logs the state and r0, the return value. Output is JSON Lines, sorted keys, so two
runs give the same bytes.

With --inject F, a forced call replaces the wait for a natural caller (none is reached on
the boot path): at the first hit of the frame routine (0201F268) at or after frame F,
the registers are saved, then OtherRandom16 is called CALLS times with LR = the hook
address, first from the live state, then with the state forced to 0xFFFF; the registers and the state word are restored. This is a forced call,
not a natural caller.

Usage:
  other_rng.py ROM OUT [--frames N] [--inject F]

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
FRAME = 0x0201F268  # MainLoops_RunFrameActions, hit once per frame
SEEDS = [None, 0xFFFF]  # forced start states; None = the live state
CALLS = 4
REGS = [f"r{i}" for i in range(13)] + ["sp", "lr"]


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=2700)
    p.add_argument("--inject", type=int, default=-1, help="forced-call mode from this frame")
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

    inj = {"done": False, "queue": [], "saved": None, "left": 0, "live": None}

    def on_frame(*_):
        if a.inject < 0 or inj["done"] or frame[0] < a.inject:
            return
        r = emu.memory.register_arm9
        if inj["saved"] is None:
            inj["saved"] = ([getattr(r, n) for n in REGS], r.cpsr, r.r15)
            inj["live"] = state()
            inj["queue"] = list(SEEDS)
            write({"ev": "inject", "f": frame[0], "cpsr": r.cpsr, "pc": r.r15})
        elif inj["left"]:
            write({"ev": "ret", "r0": r.r0, "s": state(), "pc": r.r15})
        if inj["left"] == 0:
            if not inj["queue"]:
                regs, cpsr, pc = inj["saved"]
                for n, v in zip(REGS, regs):
                    setattr(r, n, v)
                r.cpsr = cpsr
                emu.memory.set_next_instruction(FRAME)  # r15 in the hook reads FRAME+8 (pipeline); resume at the hooked instruction
                ram_poke(inj["live"])
                inj["done"] = True
                write({"ev": "restored", "s": state()})
                return
            seed = inj["queue"].pop(0)
            ram_poke(inj["live"] if seed is None else seed)
            inj["left"] = CALLS
            write({"ev": "seed", "s": state()})
        inj["left"] -= 1
        r.lr = FRAME
        emu.memory.set_next_instruction(ENTRY)

    def ram_poke(v):
        emu.memory.write_short(STATE, v)

    emu.memory.register_exec(FRAME, on_frame)
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
