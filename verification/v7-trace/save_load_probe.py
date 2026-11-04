#!/usr/bin/env python3
"""Save-load probe on the supported Blue dump (#197 P1).

Boots the retail ROM with a chosen backup image, replays the boot inputs from
boot_to_dungeon.txt, and logs save-load events from exec hooks:
  - ReadSaveFromPak return (GameLoop_Async call and the FUN_0202AA14 call)
  - ReadSaveSector return for copy A and copy B (0x02029E2C)
  - FUN_0202AA14 entry: state object state and mode
  - the message box (FUN_0202F280) and FUN_0201EB88 entries
  - ReadFlashData entry and return (0x02018F0C)

Fault injection (not a real card): --force-read N overwrites the return code of
copy A and copy B reads (r0) with N; --zero-reads replaces the bytes of a 0x57D8
save read with zeros. --card zero imports an all-zero 256 KiB backup file, which
DeSmuME did not pick up in the Windows build used for #197 (see docs/verification.md).

Modes: --card none (default, no save file, blank card as DeSmuME RAM 0xFF) or zero.

Usage:
  save_load_probe.py ROM OUT.jsonl [--card none|zero] [--frames N]
                     [--force-read N] [--zero-reads]

ROM is your own dump (supported SHA-1 only, see docs/supported-dumps.md). OUT and
the zero-card file are written next to OUT: keep OUT outside the repository. Never
commit a ROM, a save image or an output of this script.
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
OBJ_PTR = 0x020E0494  # state object pointer (FUN_0202AB6C / FUN_0202AA14)
HOOKS = {
    0x0201F3AC: "gameloop_readsave_ret",
    0x0202AA6C: "aa14_readsave_ret",
    0x0202A1E0: "readsave_copyA_ret",
    0x0202A1F8: "readsave_copyB_ret",
    0x0202AA14: "aa14_entry",
    0x0202F280: "msgbox_F280_entry",
    0x0201EB88: "eb88_entry",
    0x02018F0C: "rfd_entry",
    0x02029E64: "rfd_from_readsave_ret",
    0x0201F380: "rfd_from_gameloop_ret",
}
RFD_DEST = [None]
RFD_SIZE = [None]


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
    p.add_argument("--card", choices=["none", "zero"], default="none")
    p.add_argument("--frames", type=int, default=3000)
    p.add_argument("--force-read", type=int, default=None)
    p.add_argument("--zero-reads", action="store_true")
    a = p.parse_args()

    digest = sha1_of(a.rom)
    if digest not in SUPPORTED_SHA1:
        sys.exit(f"unsupported dump (SHA-1 {digest}); see docs/supported-dumps.md")

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    if a.card == "zero":
        zpath = os.path.join(os.path.dirname(os.path.abspath(a.out)), "zero_256k.sav")
        with open(zpath, "wb") as f:
            f.write(b"\x00" * 0x40000)
        print("import zero card before frame 0:", emu.backup.import_file(zpath), flush=True)

    events = []
    frame = [0]
    last_state = [None]
    aa14_calls = [0]

    def ev(name, extra):
        rec = {"frame": frame[0], "ev": name}
        rec.update(extra)
        events.append(rec)

    def cb(address, size):
        name = HOOKS[address]
        r = emu.memory.register_arm9
        if name in ("readsave_copyA_ret", "readsave_copyB_ret") and a.force_read is not None:
            # Fault injection: overwrite ReadSaveSector's return code (r0).
            ev(name + "_orig", {"r0": int(r.r0)})
            r.r0 = a.force_read
        if name.endswith("_ret") and not name.startswith("rfd_from_"):
            ev(name, {"r0": int(r.r0)})
        elif name == "aa14_entry":
            obj = ram.u32(OBJ_PTR)
            st = ram.u32(obj) if obj else None
            mode = ram.u32(obj + 4) if obj else None
            aa14_calls[0] += 1
            if (st, mode) != last_state[0]:
                last_state[0] = (st, mode)
                ev(name, {"obj": hex(obj), "state": st, "mode": mode})
        elif name == "rfd_entry":
            RFD_DEST[0] = int(r.r1)
            RFD_SIZE[0] = int(r.r2)
            ev(name, {"sector": int(r.r0), "dest": hex(int(r.r1)), "size": hex(int(r.r2))})
        elif name.startswith("rfd_from_"):
            head = ram.bytes(RFD_DEST[0], 8).hex() if RFD_DEST[0] else None
            ev(name, {"r0": int(r.r0), "head8": head})
            if a.zero_reads and name == "rfd_from_readsave_ret" and RFD_SIZE[0] == 0x57D8:
                # Fault injection: the card returns zeros for a save copy.
                emu.memory.write(RFD_DEST[0], RFD_DEST[0] + RFD_SIZE[0], 1, bytes(RFD_SIZE[0]))
                ev("zeroed_copy", {"dest": hex(RFD_DEST[0]), "len": hex(RFD_SIZE[0])})
        elif name == "msgbox_F280_entry":
            ev(name, {"r0": hex(int(r.r0)), "r3": hex(int(r.r3))})
        else:
            ev(name, {"r0": hex(int(r.r0))})

    for addr in HOOKS:
        emu.memory.register_exec(addr, cb)

    boot = parse_inputs(BOOT)
    for f in range(a.frames):
        frame[0] = f
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)

    obj = ram.u32(OBJ_PTR)
    events.append({"frame": a.frames, "ev": "end",
                   "sha1": digest,
                   "aa14_calls": aa14_calls[0],
                   "final_state": ram.u32(obj) if obj else None,
                   "final_mode": ram.u32(obj + 4) if obj else None,
                   "g_size_20F38B4": hex(ram.u32(0x020F38B4)),
                   "g_unit_20F38B8": hex(ram.u32(0x020F38B8)),
                   "g_structptr_20E0490": hex(ram.u32(0x020E0490)),
                   "g_unk41C_20E048C": hex(ram.u32(0x020E048C)),
                   "g_altdev_20F38A0": hex(ram.u32(0x020F38A0)),
                   "g_altflag_20DF8C0": hex(ram.u32(0x020DF8C0)),
                   "g_unk418_02132D98": hex(ram.u32(0x02132D98))})
    with open(a.out, "w") as o:
        for e in events:
            o.write(json.dumps(e, sort_keys=True) + "\n")
    print("events:", len(events), flush=True)
    for e in events[:60]:
        print(e, flush=True)
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    main()
