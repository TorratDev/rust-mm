#!/usr/bin/env python3
"""Play a fixed input sequence in DeSmuME and dump RAM per trigger (V7).

Uses py-desmume (libdesmume through ctypes). Loads ROM, optionally loads a
savestate, then runs FRAMES frames while pressing the keys from the input
file. Each time the trigger fires, LEN bytes at ADDR are appended to OUT.

The trigger is either an ARM9 exec address (--trigger, a breakpoint-style
hook on the turn-advance routine once it is known) or, without --trigger,
the end of every frame.

Input file: one "FRAME KEY KEY ..." line per change; keys are held until the
next line. "-" releases all keys. Key names: A B SELECT START RIGHT LEFT UP
DOWN R L X Y. Lines starting with "#" are ignored.

Usage:
  pydesmume_dump.py ROM OUT --frames N --addr HEX --len HEX
                    [--trigger HEX] [--state FILE] [--inputs FILE]
  pydesmume_dump.py ROM --make-state FILE --frames N
"""

import argparse
import ctypes
import hashlib
import sys
import time

from desmume.controls import Keys, keymask
from desmume.emulator import DeSmuME


MAIN_RAM = 0x02000000
MAIN_RAM_SIZE = 0x400000
# Offset of MAIN_MEM in DeSmuME's exported MMU struct (after ARM9 ITCM 32 KiB
# and DTCM 16 KiB). Checked against the per-byte API before use.
MMU_MAIN_MEM_OFFSET = 0xC000


def make_reader(emu: DeSmuME, addr: int, length: int):
    """Return a function that reads LEN bytes at ADDR.

    py-desmume reads one byte per ctypes call, which takes seconds for a 4 MiB
    range. For main RAM, read DeSmuME's backing array directly instead.
    """
    if length == 0:
        return lambda: b""  # timing baseline
    slow = lambda: emu.memory.read(addr, addr + length, 1, False)  # noqa: E731
    if not (MAIN_RAM <= addr and addr + length <= MAIN_RAM + MAIN_RAM_SIZE):
        return slow
    try:
        base = ctypes.addressof(ctypes.c_uint8.in_dll(emu.lib, "MMU"))
    except ValueError:
        print("MMU symbol not exported; using per-byte reads", file=sys.stderr)
        return slow
    ptr = base + MMU_MAIN_MEM_OFFSET + (addr - MAIN_RAM)
    fast = lambda: ctypes.string_at(ptr, length)  # noqa: E731
    probe = max(2, min(length, 0x100))  # py-desmume returns an int for 1 byte
    if ctypes.string_at(ptr, probe) != emu.memory.read(addr, addr + probe, 1, False):
        sys.exit("MMU layout differs from this script's assumption")
    return fast


def parse_inputs(path: str | None) -> dict[int, int]:
    """Map frame -> keymask that takes effect on that frame."""
    changes: dict[int, int] = {}
    if path is None:
        return changes
    with open(path) as f:
        for line in f:
            line = line.split("#", 1)[0].split()
            if not line:
                continue
            mask = 0
            for name in line[1:]:
                if name != "-":
                    mask |= keymask(getattr(Keys, f"KEY_{name.upper()}"))
            changes[int(line[0])] = mask
    return changes


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out", nargs="?")
    p.add_argument("--frames", type=int, required=True)
    p.add_argument("--addr", type=lambda s: int(s, 16))
    p.add_argument("--len", type=lambda s: int(s, 16))
    p.add_argument("--trigger", type=lambda s: int(s, 16))
    p.add_argument("--state", help="savestate to load before playing")
    p.add_argument("--inputs", help="input sequence file")
    p.add_argument("--make-state", help="run FRAMES frames, save a savestate, exit")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(a.rom)

    if a.make_state:
        for _ in range(a.frames):
            emu.cycle(with_joystick=False)
        emu.savestate.save_file(a.make_state)
        emu.destroy()
        return

    if a.out is None or a.addr is None or a.len is None:
        p.error("OUT, --addr and --len are required unless --make-state is given")
    if a.state:
        emu.savestate.load_file(a.state)

    inputs = parse_inputs(a.inputs)
    read = make_reader(emu, a.addr, a.len)
    hits = 0
    out = open(a.out, "wb")

    def dump(*_args) -> None:
        nonlocal hits
        out.write(read())
        hits += 1

    if a.trigger is not None:
        emu.memory.register_exec(a.trigger, dump)

    start = time.monotonic()
    for frame in range(a.frames):
        if frame in inputs:
            emu.input.keypad_update(inputs[frame])
        emu.cycle(with_joystick=False)
        if a.trigger is None:
            dump()
    elapsed = time.monotonic() - start
    out.close()
    emu.destroy()

    with open(a.out, "rb") as f:
        digest = hashlib.sha256(f.read()).hexdigest()
    print(
        f"{a.frames} frames, {hits} dumps, {elapsed:.2f}s "
        f"({a.frames / elapsed:.0f} fps), sha256 {digest}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
