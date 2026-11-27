#!/usr/bin/env python3
"""Use an Escape Orb on the first floor and log the dungeon exit it causes (#641, result code 2).

FORCED: the savestate has an empty team bag and no Escape Orb on the floor (checked with the floor-item
bytes), so this script writes one Escape Orb (item 200 = 0xC8, the Red id; the Blue id is not confirmed)
into bag slot 0 with quantity 1, and sets the bag gate gDungeon+0x64C to 1 as items_bag_probe.py does.
The result is tagged "forced" in the output.

Keys: the bag menu is opened with the same keys as items_bag_probe.py (W, X, DOWN, A, A, then A to use
the first item). The orb is used as the move EscapeOrbAction (0x021623A4, move id 181); the probe logs
that hook. The dungeon exit hooks of friend_areas_probe.py are logged with their frame, r0, and the
result code at gDungeon+0x60 read at each hit.

Usage: escape_orb_probe.py ROM OUT.json [--savestate PATH] [--frames N]
ROM must be a throwaway copy with a name other than pmdb.nds. Keep ROM and OUT outside the repository.
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME  # noqa: E402
from desmume.controls import Keys, keymask  # noqa: E402
from retail_trace import G_DUNGEON, Ram, parse_inputs  # noqa: E402
from items_bag_probe import INV_PTR, BAG_GATE, OPEN_ITEM  # noqa: E402

ESCAPE_ORB = 200            # Red's ITEM_ESCAPE_ORB; the Blue id is not confirmed
ESCAPE_ACTION = 0x021623A4  # EscapeOrbAction (docs/re/move-effects.md, move 181)
EXIT = (0x0206DC20, 0x0206ED24, 0x0206ED3C, 0x0206ED4C, 0x02177FD8, 0x02178068)
RESULT_CODE = 0x60
DEFAULT_STATE = os.path.expanduser("~/rescue-re/saves/states/dungeon1-f1.dst")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--savestate", default=DEFAULT_STATE)
    p.add_argument("--boot", action="store_true",
                   help="boot from boot_to_dungeon.txt instead of the savestate; records 0206DC20's entry r0 "
                        "(the run struct) so the result code can be read at +0x60 of the struct itself")
    p.add_argument("--frames", type=int, default=3000)
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    entry = {}
    if a.boot:
        # 0206DC20's first hit is the run's entry (friend_areas_probe.py): its r0 is the run struct.
        def on_run_entry(addr, size):
            if "r0" not in entry:
                entry["r0"] = int(emu.memory.register_arm9.r0)
        emu.memory.register_exec(0x0206DC20, on_run_entry)
        boot = parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
        for f in range(30000):
            if f in boot:
                emu.input.keypad_update(boot[f])
            emu.cycle(with_joystick=False)
            if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
                break
        else:
            sys.exit("no dungeon reached from boot_to_dungeon.txt")
        emu.input.keypad_update(0)
        for _ in range(60):
            emu.cycle(with_joystick=False)
    else:
        emu.savestate.load_file(os.path.abspath(a.savestate))
        for _ in range(60):
            emu.cycle(with_joystick=False)

    dg = ram.u32(G_DUNGEON)
    inv = ram.u32(INV_PTR)
    if not dg or not inv:
        sys.exit("no dungeon or bag in the savestate")
    before = bytes(ram.bytes(inv, 6)).hex()
    # FORCED: bag slot 0 = (valid, quantity, item id), then the bag gate.
    emu.memory.write(inv, inv + 3, 1, bytes([1, 1, ESCAPE_ORB]))
    emu.memory.write(dg + BAG_GATE, dg + BAG_GATE + 1, 1, bytes([1]))
    res = {"forced": {"bag_slot0_before": before, "bag_slot0_written": [1, 1, ESCAPE_ORB],
                      "bag_gate": 1}, "events": [], "frame": 0}

    frame = {"n": 0}
    events = res["events"]

    def cycle(n=1, keys=0):
        for _ in range(n):
            frame["n"] += 1
            emu.input.keypad_update(keys)
            emu.cycle(with_joystick=False)

    def press(name):
        cycle(4, keymask(getattr(Keys, "KEY_" + name)))
        cycle(20)

    def send(keys):
        for k in keys:
            cycle(200) if k == "W" else press(k)

    def hook(name):
        def cb(addr, size):
            R = emu.memory.register_arm9
            e = {"hook": name, "frame": frame["n"], "r0": int(R.r0), "r5": hex(int(R.r5))}
            # The exit tail (LAB_0206eca0) keeps the run struct in r5; the result code is its s16 at +0x60.
            if name != "EscapeOrbAction" and "r0" in entry:
                # Run struct from 0206DC20's entry r0 (the same struct that the tail reads as param_1).
                e["code"] = ram.s16(entry["r0"] + RESULT_CODE)
                e["entry_r0"] = hex(entry["r0"])
            elif name != "EscapeOrbAction":
                # 0x027E0000 is a mirror of main RAM (4 MB at 0x02000000): map r5 back into it.
                struct = 0x02000000 | (int(R.r5) & 0x3FFFFF)
                e["struct"] = hex(struct)
                e["code_r5"] = ram.s16(struct + RESULT_CODE)
            events.append(e)
        return cb

    emu.memory.register_exec(ESCAPE_ACTION, hook("EscapeOrbAction"))
    for x in EXIT:
        emu.memory.register_exec(x, hook("%08X" % x))

    send(["W", "A", "W", "A"])  # dismiss tutorial boxes, as items_bag_probe.py does
    send(OPEN_ITEM + ["A"])     # open the bag, first item, use
    last = frame["n"]
    while frame["n"] - last < a.frames:
        cycle(120)
        press("A")               # confirm the orb's messages
        if any(e["hook"] == "02178068" for e in events):
            cycle(600)
            break

    fired = {e["hook"] for e in events}
    res["hooks"] = {h: sum(1 for e in events if e["hook"] == h) for h in fired}
    res["not_reached"] = [h for h in ["EscapeOrbAction"] + ["%08X" % x for x in EXIT] if h not in fired]
    res["bag_slot0_after"] = bytes(ram.bytes(inv, 6)).hex()
    res["final_frame"] = frame["n"]
    with open(a.out, "w") as f:
        json.dump(res, f, indent=1)
    print(json.dumps({k: res[k] for k in ("forced", "hooks", "not_reached", "bag_slot0_after")}, indent=1),
          flush=True)
    emu.destroy()


if __name__ == "__main__":
    main()
