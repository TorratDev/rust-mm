#!/usr/bin/env python3
"""Overlay-7 probe on the supported Blue dump (#161 P3). FAULT INJECTION, not a game path.

Boots the ROM with the harness boot inputs (boot_to_dungeon.txt). With --force-at N,
the title-menu result is forced to 5 (choice 5) on the first title return at or after
frame N, by writing r0 and r8 at 0x0201F6A0 (the instruction after the call to
FUN_020628BC in GameLoop_Async). That sends GameLoop_Async to RunOverlay7Mode, which
enters overlay 7 at ov_0007::02139F80. The entry is counted, and an optional key is
pressed --press KEY:DELAY frames after the entry (B returns to the title menu).
Screenshots are written to OUTDIR.

Reaching choice 5 by the real title menu is NOT tested here: it needs a save.

Usage:
  overlay7_inject.py ROM OUTDIR [--frames N] [--force-at N] [--inputs FILE]
                     [--press KEY:DELAY] [--shots LIST]

ROM is your own dump (supported SHA-1 only, see docs/supported-dumps.md). OUTDIR gets
the log and screenshots: keep it outside the repository. Never commit a ROM, a
screenshot, a save or an output of this script.
"""

import argparse
import hashlib
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

import pydesmume_dump as h  # noqa: E402  (parse_inputs: the harness input format)
from desmume.emulator import DeSmuME  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
BOOT = os.path.join(HERE, "boot_to_dungeon.txt")
REG_RET = 0x0201F6A0  # mov r8, r0 after BL FUN_020628BC (GameLoop_Async)
OV7_ENTRY = 0x02139F80  # ov_0007::02139F80, called by RunOverlay7Mode
KEYS = {"A": 0x0001, "B": 0x0002}


def sha1_of(path):
    hsh = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            hsh.update(chunk)
    return hsh.hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("outdir")
    p.add_argument("--frames", type=int, default=1500)
    p.add_argument("--force-at", type=int, default=-1,
                   help="force the title result to 5 from this frame on; -1 = no injection")
    p.add_argument("--inputs", default=BOOT, help="harness input file (default: boot_to_dungeon.txt)")
    p.add_argument("--press", default="", help="KEY:DELAY, e.g. B:120 (KEY is A or B)")
    p.add_argument("--shots", default="300,900,1200,1400,1500")
    a = p.parse_args()

    digest = sha1_of(a.rom)
    if digest not in SUPPORTED_SHA1:
        sys.exit(f"unsupported dump (SHA-1 {digest}); see docs/supported-dumps.md")

    os.makedirs(a.outdir, exist_ok=True)
    shots = {int(x) for x in a.shots.split(",") if x}
    state = {"frame": 0, "forced": 0, "hits": 0, "ov7_hits": 0, "ov7_first": None}
    log = open(os.path.join(a.outdir, "log.txt"), "w")

    def log_line(s):
        print(s, flush=True)
        log.write(s + "\n")
        log.flush()

    log_line(f"sha1 {digest}")
    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))

    def on_title_return(*_):
        state["hits"] += 1
        if state["hits"] == 1:
            log_line(f"TITLE first return frame={state['frame']}")
        if a.force_at >= 0 and state["frame"] >= a.force_at and not state["forced"]:
            r = emu.memory.register_arm9
            r.r0 = 5
            r.r8 = 5
            state["forced"] = state["frame"]
            log_line(f"INJECT frame={state['frame']} title result forced to 5 (r0, r8) at {REG_RET:#x}")

    def on_ov7(*_):
        state["ov7_hits"] += 1
        if state["ov7_first"] is None:
            state["ov7_first"] = state["frame"]
            log_line(f"OV7 entry reached frame={state['frame']}")

    emu.memory.register_exec(REG_RET, on_title_return)
    emu.memory.register_exec(OV7_ENTRY, on_ov7)

    press_key, press_delay = (None, 0)
    if a.press:
        key, delay = a.press.split(":")
        press_key, press_delay = key, int(delay)

    t = time.monotonic()
    inputs = h.parse_inputs(a.inputs)
    for f in range(a.frames):
        state["frame"] = f + 1
        if f in inputs:
            emu.input.keypad_update(inputs[f])
        if press_key and state["ov7_first"] is not None:
            if f == state["ov7_first"] + press_delay:
                emu.input.keypad_update(KEYS[press_key])
                log_line(f"PRESS {press_key} at frame {f}")
            if f == state["ov7_first"] + press_delay + 5:
                emu.input.keypad_update(0)
                log_line(f"RELEASE at frame {f}")
        emu.cycle(with_joystick=False)
        if (f + 1) in shots:
            emu.screenshot().save(os.path.join(a.outdir, f"frame{f + 1:05d}.png"))
            log_line(f"shot frame={f + 1} title_hits={state['hits']} ov7_hits={state['ov7_hits']}")
    log_line(f"done frames={a.frames} forced={state['forced']} "
             f"ov7_first={state['ov7_first']} ov7_hits={state['ov7_hits']} title_hits={state['hits']}")
    # Wall-clock time goes to stdout only, so log.txt is identical between runs.
    print(f"secs={time.monotonic() - t:.1f}", flush=True)
    log.close()
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    main()
