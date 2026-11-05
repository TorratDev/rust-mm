#!/usr/bin/env python3
"""D-pad repeat timing probe on the supported Blue dump (#175 P5).

Boots with no input, then holds one D-pad direction (default Down) from frame --press for
--hold frames, and logs once per frame the current key struct at 020F2790 (held +0,
pressed +2, repeat +4, flags +6, D-pad group +8, counter +0xC), written by UpdateKeyState
(FUN_02017B20). The last line lists the frames, counted from the press, where pressed and
where repeat are nonzero. Output is JSON Lines, sorted keys, so two runs give the same bytes.

Usage:
  dpad_repeat.py ROM OUT [--dir Down|Up|Left|Right] [--press N] [--hold N]

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

import retail_trace as rt  # noqa: E402  (Ram)
from desmume.controls import Keys, keymask  # noqa: E402
from desmume.emulator import DeSmuME  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
KEY_STRUCT = 0x020F2790  # current key state: u16 held, pressed, repeat, flags, group; int at +0xC


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--dir", choices=["Down", "Up", "Left", "Right"], default="Down")
    p.add_argument("--press", type=int, default=800)
    p.add_argument("--hold", type=int, default=200)
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
    key = keymask(getattr(Keys, "KEY_" + a.dir.upper()))
    out = open(a.out, "w", encoding="utf-8", newline="\n")
    digest = hashlib.sha256()
    pressed, repeat = [], []

    def write(obj):
        line = json.dumps(obj, sort_keys=True, separators=(",", ":")) + "\n"
        out.write(line)
        digest.update(line.encode("utf-8"))

    for f in range(a.press + a.hold + 20):
        if f == a.press:
            emu.input.keypad_update(key)
        if f == a.press + a.hold:
            emu.input.keypad_update(0)
        emu.cycle(with_joystick=False)
        if f >= a.press:
            held, prs, rep, flags, grp = struct.unpack("<5H", ram.bytes(KEY_STRUCT, 10))
            cnt = struct.unpack("<i", ram.bytes(KEY_STRUCT + 0xC, 4))[0]
            rel = f - a.press
            write({"rel": rel, "held": held, "pressed": prs, "repeat": rep, "flags": flags,
                   "group": grp, "counter": cnt})
            if prs & 0xF0:
                pressed.append(rel)
            if rep & 0xF0:
                repeat.append(rel)
    write({"ev": "summary", "dir": a.dir, "pressed_at": pressed, "repeat_at": repeat})
    out.close()
    print(f"pressed at {pressed}\nrepeat at {repeat}", file=sys.stderr)
    print(f"sha256 {digest.hexdigest()}", file=sys.stderr)
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    main()
