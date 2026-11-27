#!/usr/bin/env python3
"""#639 probe: town evolution in Luminous Cave (docs/re/progression/p6-evolution.md P6.1).

Loads a savestate made in Luminous Cave (map 8), then plays seeded random menu inputs (A, B, directions,
Start) while hooks log the town evolution functions:

  0200475C  check (r0 = roster record, r1 = out; flags at out+4 are read at its return)
  0200468C  apply (r0 = record, r1 = new species)
  02004590  apply, second half
  02003130  roster write (r0 = the new record)
  02047D74  menu state machine, 020485D8 its caller

Record layout read: +0x03 level, +0x06 species (u16), +0x0C HP (s16), +0x14 EXP (u32).
Options (the test, not the game): --level N writes N into the record's level byte at the first check;
--species N writes N into the record's species (+6) at the first check, to start from a later stage;
--give-item N writes item id N into out[0] when the menu calls the check with hasItemsGiven (r2 != 0);
--force-item makes the item-count helper 02007AF8 return 0 (an item "owned") so item stages pass.

Usage: town_menu_drive_639.py ROM STATE FRAMES SEED [--level N] [--species N] [--give-item N] [--force-item]
ROM is a scratch copy under a throwaway name; run with XDG_CONFIG_HOME in a scratch directory.
The savestate is made outside the repository from a user save.
"""
import argparse
import os
import random
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.controls import Keys, keymask  # noqa: E402
from desmume.emulator import DeSmuME  # noqa: E402
from retail_trace import Ram  # noqa: E402

CHECK, APPLY, APPLY_INNER, WRITE_ROSTER = 0x0200475C, 0x0200468C, 0x02004590, 0x02003130
MENU, CALLER, ITEM_COUNT = 0x02047D74, 0x020485D8, 0x02007AF8
KEYS = ["A", "A", "B", "UP", "DOWN", "LEFT", "RIGHT", "START"]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("state")
    p.add_argument("frames", type=int)
    p.add_argument("seed", type=int)
    p.add_argument("--level", type=int, default=None)
    p.add_argument("--species", type=int, default=None)
    p.add_argument("--give-item", type=int, default=None)
    p.add_argument("--force-item", action="store_true")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    ph = {"f": 0, "level_set": False}
    ev = []

    def rec_fields(rec):
        return {"level": ram.u8(rec + 3), "species": struct.unpack("<H", ram.bytes(rec + 6, 2))[0],
                "hp": struct.unpack("<h", ram.bytes(rec + 0xC, 2))[0],
                "exp": struct.unpack("<I", ram.bytes(rec + 0x14, 4))[0]}

    def on_check(_a, _s):
        rec, out, lr = int(R.r0), int(R.r1), int(R.r14) & ~1
        if a.level is not None and not ph["level_set"]:
            emu.memory.write_byte(rec + 3, a.level)
            ph["level_set"] = True
        if a.species is not None and not ph.get("species_set"):
            emu.memory.write_byte(rec + 6, a.species & 0xFF)
            emu.memory.write_byte(rec + 7, a.species >> 8)
            ph["species_set"] = True
        if a.give_item is not None and int(R.r2) != 0:
            emu.memory.write_byte(out, a.give_item)  # the menu's hasItemsGiven path reads out[0], out[1]
        before = rec_fields(rec)

        def on_ret(_a2, _s2):
            ev.append({"h": "check", "f": ph["f"], **before,
                       "flags": struct.unpack("<H", ram.bytes(out + 4, 2))[0]})
        emu.memory.register_exec(lr, on_ret)

    def on_apply(_a, _s):
        ev.append({"h": "apply", "f": ph["f"], "new_species": int(R.r1), **rec_fields(int(R.r0))})

    def on_write(_a, _s):
        ev.append({"h": "write_roster", "f": ph["f"], **rec_fields(int(R.r0))})

    def on_item(_a, _s):
        lr = int(R.r14) & ~1

        def on_ret(_a2, _s2):
            ev.append({"h": "item_count", "f": ph["f"], "item": None, "r0": int(R.r0)})
            R.r0 = 0
        emu.memory.register_exec(lr, on_ret)

    emu.memory.register_exec(CHECK, on_check)
    emu.memory.register_exec(APPLY, on_apply)
    emu.memory.register_exec(WRITE_ROSTER, on_write)
    emu.memory.register_exec(MENU, lambda _a, _s: ev.append({"h": "menu", "f": ph["f"]}))
    emu.memory.register_exec(CALLER, lambda _a, _s: ev.append({"h": "caller_menu", "f": ph["f"]}))
    if a.force_item:
        emu.memory.register_exec(ITEM_COUNT, on_item)

    emu.savestate.load_file(os.path.abspath(a.state))
    rng = random.Random(a.seed)
    f = 0
    while f < a.frames:
        key, hold = rng.choice(KEYS), rng.randint(4, 15)
        for _ in range(hold):
            ph["f"] = f
            emu.input.keypad_update(keymask(getattr(Keys, "KEY_" + key)))
            emu.cycle(with_joystick=False)
            f += 1
    emu.input.keypad_update(0)
    for e in ev:
        if e["h"] in ("check", "apply", "write_roster"):
            print("EVENT", e, flush=True)
    print("COUNTS", {h: sum(1 for e in ev if e["h"] == h) for h in sorted({e["h"] for e in ev})}, flush=True)


if __name__ == "__main__":
    import romarg
    romarg.fill()
    main()
