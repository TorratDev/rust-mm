#!/usr/bin/env python3
"""Overlay 0 residency at the scene runner on the supported Blue dump (#160 P2).

At every scene-runner entry (0x0206F7EC, ARM9) it reads the 16 bytes at 0x0214FCA4.
Overlay 0 has 10402de9 44109fe5 there (GroundStation_UnloadBank); overlay 1 or 7 occupy
the same address and differ, so intact bytes mean overlay 0 is resident. It also logs
the overlay-0 load and unload entries, the bank loads (SetBank) and the dungeon entry,
each with its frame number. Output is JSON Lines, one object per event.

Usage:
  python overlay0_residency.py ROM OUT FRAMES

ROM is your own dump (supported SHA-1 only, see docs/supported-dumps.md). Keep OUT out of
the repository. Run without -I: the desmume package is then imported from the user site.
"""

import hashlib
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

from desmume.emulator import DeSmuME  # noqa: E402
from retail_trace import parse_inputs  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
RUNNER = 0x0206F7EC  # scene runner (ARM9)
LOAD_G, UNLOAD_G = 0x0201DCEC, 0x0201DCB4  # NDS_LoadOverlay_GroundMain / NDS_UnloadOverlay_GroundMain
DUNGEON_ENTRY, RUN_DUNGEON = 0x0201DB70, 0x0206DC20  # LoadAndRunDungeon_Async / RunDungeon_Async
PROBE_ADDR, PROBE_LEN = 0x0214FCA4, 16  # GroundStation_UnloadBank bytes (overlay 0)
SETBANK = 0x0214FC50  # GroundStation_SetBank; overlay 0 address, valid only before the dungeon
CUR_BANK_PTR_LIT = 0x0214FCF4  # literal: address of the current-bank short


def sha1_of(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__.split("\n\nUsage:")[1].split("\n\n")[0].strip())
    rom, out_path, frames = sys.argv[1], sys.argv[2], int(sys.argv[3])
    if sha1_of(rom) not in SUPPORTED_SHA1:
        sys.exit("not a supported dump (see docs/supported-dumps.md)")

    emu = DeSmuME()
    emu.open(rom)
    boot = parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
    state = {"f": 0}
    out = open(out_path, "w", encoding="utf-8", newline="\n")

    def ram(addr, n):
        return bytes(emu.memory.read(addr, addr + n, 1, False)).hex()

    def ev(name, **kw):
        out.write(json.dumps({"ev": name, "f": state["f"], **kw}, sort_keys=True) + "\n")

    def reg(name):
        return getattr(emu.memory.register_arm9, name)

    def on_runner(*_):
        ev("runner", sid=reg("r0") & 0xFFFFFFFF, ov0_bytes=ram(PROBE_ADDR, PROBE_LEN))

    def on_load_g(*_):
        ev("load_ov0")

    def on_unload_g(*_):
        ev("unload_ov0")

    def on_setbank(*_):
        p = int.from_bytes(bytes(emu.memory.read(CUR_BANK_PTR_LIT, CUR_BANK_PTR_LIT + 4, 1, False)),
                           "little")
        cur = int.from_bytes(bytes(emu.memory.read(p, p + 2, 1, False)), "little", signed=True)
        ev("setbank", bank=reg("r0") & 0xFFFFFFFF, ov0_bytes=ram(PROBE_ADDR, PROBE_LEN),
           cur_var_addr=hex(p), cur_bank=cur)

    def on_dungeon(*_):
        ev("dungeon_entry")

    def on_run_dungeon(*_):
        ev("run_dungeon", ov0_bytes=ram(PROBE_ADDR, PROBE_LEN))

    for addr, cb in [(RUNNER, on_runner), (LOAD_G, on_load_g), (UNLOAD_G, on_unload_g),
                     (DUNGEON_ENTRY, on_dungeon), (RUN_DUNGEON, on_run_dungeon),
                     (SETBANK, on_setbank)]:
        emu.memory.register_exec(addr, cb)

    for f in range(frames):
        state["f"] = f
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
    out.close()
    print(f"done {frames} frames", file=sys.stderr)
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    main()
