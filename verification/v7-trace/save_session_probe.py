#!/usr/bin/env python3
"""Session-struct and save-branch probe on the supported Blue dump (#197 P7).

Boots the retail ROM with the boot inputs from boot_to_dungeon.txt until the
dungeon global (0x020E0788) is set, or for --frames frames. It logs whether the
alternate save branch is entered: the session struct init and teardown
(FUN_020660E4, FUN_02065F88), the device detect (FUN_0201D638), and the save
reads and writes with their sector numbers. The session struct pointer
(0x020E0490) is recorded with each save call.

Usage:
  save_session_probe.py ROM OUT.jsonl [--frames N]

ROM is your own dump (supported SHA-1 only, see docs/supported-dumps.md). Keep
OUT outside the repository. Never commit a ROM or an output of this script.
"""

import argparse
import hashlib
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

from desmume.emulator import DeSmuME  # noqa: E402
from retail_trace import parse_inputs, Ram  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
BOOT = os.path.join(HERE, "boot_to_dungeon.txt")
G_DUNGEON = 0x020E0788
STRUCT_PTR = 0x020E0490
HOOKS = {
    0x020660E4: "init_session_struct",
    0x02065F88: "teardown_session_struct",
    0x0201D638: "alt_device_detect",
    0x02029E2C: "ReadSaveSector",
    0x02029ECC: "WriteSaveSector",
    0x0202A53C: "WriteSavetoPak",
    0x0202A1AC: "ReadSaveFromPak",
    0x02018DC4: "WriteFlashData",
    0x02018F0C: "ReadFlashData",
}
MAX_FRAMES = 26000


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
    p.add_argument("--frames", type=int, default=MAX_FRAMES)
    a = p.parse_args()

    digest = sha1_of(a.rom)
    if digest not in SUPPORTED_SHA1:
        sys.exit(f"unsupported dump (SHA-1 {digest}); see docs/supported-dumps.md")

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    events = []
    frame = [0]
    counts = {}

    def cb(address, size):
        name = HOOKS[address]
        counts[name] = counts.get(name, 0) + 1
        r = emu.memory.register_arm9
        rec = {"frame": frame[0], "ev": name, "r0": hex(int(r.r0)),
               "r1": hex(int(r.r1)), "r2": hex(int(r.r2))}
        if name in ("ReadSaveSector", "WriteSaveSector"):
            rec["sector"] = ram.u32(int(r.r0))
        if name == "WriteSavetoPak" or name == "ReadSaveFromPak":
            rec["structptr"] = hex(ram.u32(STRUCT_PTR))
        if len(events) < 400:
            events.append(rec)

    for addr in HOOKS:
        emu.memory.register_exec(addr, cb)

    boot = parse_inputs(BOOT)
    dungeon_frame = None
    for f in range(a.frames):
        frame[0] = f
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
            dungeon_frame = f
            break
    summary = {"ev": "summary", "sha1": digest, "dungeon_frame": dungeon_frame,
               "structptr_at_end": hex(ram.u32(STRUCT_PTR)),
               "hook_counts": counts}
    with open(a.out, "w") as o:
        for e in events:
            o.write(json.dumps(e, sort_keys=True) + "\n")
        o.write(json.dumps(summary, sort_keys=True) + "\n")
    print(json.dumps(summary, sort_keys=True), flush=True)
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    main()
