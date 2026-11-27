#!/usr/bin/env python3
"""Bag, drop, pickup and use probe on the supported Blue dump (#465 P1).

Boots to the first dungeon floor (or loads a savestate made by --save-state),
pokes one bag item into slot 0, sets the bag gate gDungeon+0x64C to 1, then
drives the menu: Items -> item -> Place (drop), step off and on to pick it up,
then (--steps use) Items -> Gummi -> eat. After the drop, the pickup and the use it logs the
bag (60 bytes), numItems (s16 gDungeon+0x3424) and floor slot bytes. Exec hooks
log entry to: 02173CCC drop, 021440B0 SpawnItem, 021441D8 pickup, 02143F98
RemoveItemAt, 0217371C use. State is read at hook time, before the function runs.

Menu keys (floor 1 after the tutorial boxes): W A W A W X DOWN A A [DOWN A],
W = 200 idle frames, keys held 4 frames, 20 frame gap. The script sends that
sequence; if the menu differs on your floor, adjust DROP_KEYS and USE_KEYS.

Usage:
  items_bag_probe.py ROM OUT.jsonl [--item ID] [--qty N] [--steps drop|use]
                     [--state FILE] [--save-state FILE]

--steps drop (default) runs drop then pickup with --item 0x20 (Plain Ribbon);
--steps use --item 0x56 (White Gummi) runs the use step. The boot takes minutes; --save-state writes a savestate
60 frames after the dungeon pointer is set, --state loads it. Keep OUT and any
savestate outside the repository. Never commit a ROM, a savestate or an output.
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

from desmume.controls import Keys, keymask  # noqa: E402
from desmume.emulator import DeSmuME  # noqa: E402
from retail_trace import G_DUNGEON, Ram, parse_inputs  # noqa: E402

SUPPORTED_SHA1 = {"6f01887356268de39085df01490347bbcc3aa330"}
BOOT = os.path.join(HERE, "boot_to_dungeon.txt")
INV_PTR = 0x020DECF8
NUM_ITEMS, FLOOR_ITEMS, ITEM_ENTS, BAG_GATE = 0x3424, 0x3364, 0x113B4, 0x64C
HOOKS = {
    0x02173CCC: "drop",
    0x021440B0: "spawn_item",
    0x021441D8: "pickup",
    0x02143F98: "remove_item_at",
    0x0217371C: "use",
}
W = "W"
# Open menu, Items, first item, submenu. The ribbon's submenu is Give, Place,
# Throw, Info, so one DOWN reaches Place (drop). A gummi's first entry is eat.
OPEN_ITEM = [W, "X", "DOWN", "A", "A"]
DROP_KEYS = OPEN_ITEM + ["DOWN", "A"]
USE_KEYS = OPEN_ITEM + ["A"]
STEP_OFF_ON = ["LEFT", "RIGHT"]


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
    p.add_argument("--item", type=lambda s: int(s, 0), default=0x20)
    p.add_argument("--qty", type=int, default=1)
    p.add_argument("--steps", choices=["drop", "use"], default="drop")
    p.add_argument("--state")
    p.add_argument("--save-state")
    a = p.parse_args()

    digest = sha1_of(a.rom)
    if digest not in SUPPORTED_SHA1:
        sys.exit(f"unsupported dump (SHA-1 {digest}); see docs/supported-dumps.md")

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    events, frame = [], [0]

    def dump(name, **extra):
        dg = ram.u32(G_DUNGEON)
        inv = ram.u32(INV_PTR)
        n = ram.s16(dg + NUM_ITEMS) if dg else None
        rec = {"frame": frame[0], "ev": name, "numItems": n}
        if inv:
            rec["bag"] = ram.bytes(inv, 60).hex()
        if dg:
            top = max(0, min(n or 0, 64)) + 1  # one slot past numItems shows stale bytes
            top = min(top, 64)
            rec["floor"] = ram.bytes(dg + FLOOR_ITEMS, 3 * top).hex()
            rec["ents"] = [ram.u32(ram.u32(dg + ITEM_ENTS + 4 * i)) if ram.u32(dg + ITEM_ENTS + 4 * i)
                           else 0 for i in range(top)]
        rec.update(extra)
        events.append(rec)

    def cb(address, size):
        dump("hook_" + HOOKS[address])

    def cycle(n=1):
        for _ in range(n):
            frame[0] += 1
            emu.cycle(with_joystick=False)

    def press(name):
        emu.input.keypad_update(keymask(getattr(Keys, "KEY_" + name)))
        cycle(4)
        emu.input.keypad_update(0)
        cycle(20)

    def send(keys):
        for k in keys:
            cycle(200) if k == W else press(k)

    if a.state:
        if not emu.savestate.load_file(os.path.abspath(a.state)):
            sys.exit("could not load the savestate")
        cycle(2)
    else:
        boot = parse_inputs(BOOT)
        for f in range(30000):
            if f in boot:
                emu.input.keypad_update(boot[f])
            emu.cycle(with_joystick=False)
            if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
                break
        else:
            sys.exit("no dungeon reached")
        emu.input.keypad_update(0)
        cycle(60)
        if a.save_state:
            emu.savestate.save_file(os.path.abspath(a.save_state))

    dg, inv = ram.u32(G_DUNGEON), ram.u32(INV_PTR)
    emu.memory.write(inv, inv + 2, 1, bytes([1, a.qty & 0xFF]))
    emu.memory.write(inv + 2, inv + 3, 1, bytes([a.item & 0xFF]))
    emu.memory.write(dg + BAG_GATE, dg + BAG_GATE + 1, 1, bytes([1]))
    dump("start")

    for addr in HOOKS:
        emu.memory.register_exec(addr, cb)

    send([W, "A", W, "A"])  # dismiss the tutorial boxes
    if a.steps == "drop":
        send(DROP_KEYS)
        cycle(120)
        dump("after_drop")
        send(STEP_OFF_ON)
        cycle(120)
        dump("after_pickup")
    else:
        send(USE_KEYS)
        cycle(120)
        dump("after_use")

    events.append({"ev": "end", "sha1": digest, "item": a.item, "qty": a.qty})
    with open(a.out, "w") as o:
        for e in events:
            o.write(json.dumps(e, sort_keys=True) + "\n")
    for e in events:
        print(e, flush=True)
    emu.destroy()  # libdesmume ends the process here, so it comes last


if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
