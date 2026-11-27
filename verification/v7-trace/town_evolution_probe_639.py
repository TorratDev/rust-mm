#!/usr/bin/env python3
"""#639 probe: town evolution functions (P6.1) on the Team Base savestate.

Loads a town savestate (boot-town.dst, GROUND_MAP 13, free movement), registers exec hooks on the
town evolution functions and on LogMessageWithTarget, then plays the walk route for FRAMES frames.
Logs one event per hook hit (frame, address, r0..r2) and the GROUND_MAP value every 300 frames.

  0200475C  check (roster record, out, hasItemsGiven)
  0200468C  apply (first half)
  02004590  apply (second half)
  02047D74  evolution menu state machine
  02154B8C  LogMessageWithTarget (r2 = message text pointer; the 0x74D title is printed here)

A hook that never fires means "not reached in this run", not a negative result.
Usage: town_evolution_probe_639.py ROM STATE OUT.json [--frames N] [--inputs PATH]
ROM is a scratch copy under a throwaway name; run with XDG_CONFIG_HOME in a scratch directory.
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
from retail_trace import Ram, parse_inputs  # noqa: E402

GROUND_MAP = 0x0210E670
HOOKS = {0x0200475C: "check", 0x0200468C: "apply_a", 0x02004590: "apply_b",
         0x02047D74: "menu", 0x02154B8C: "logmsg"}
# callers of the check and the menu (xrefs.tsv): the menu entry functions and the item/roster users
CALLERS = {0x02004724: "caller_check_a", 0x0204730C: "caller_check_b", 0x020485D8: "caller_menu",
           0x020473B4: "menu_item_a", 0x020473E8: "menu_item_b", 0x0204746C: "menu_item_c",
           0x020474A8: "menu_item_d"}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("state")
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=3000)
    p.add_argument("--inputs", default=os.path.join(HERE, "ground_walk_inputs.txt"))
    p.add_argument("--explore", action="store_true",
                   help="press buttons in turn (A, START, X, Y, B, directions), 10 frames each every 40")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    emu.savestate.load_file(os.path.abspath(a.state))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    ev = []
    ph = {"f": 0}

    for addr, name in {**HOOKS, **CALLERS}.items():
        def cb(_a, _s, name=name):
            ev.append({"h": name, "f": ph["f"], "r0": int(R.r0), "r1": int(R.r1), "r2": int(R.r2),
                       "lr": hex(int(R.r14) & ~1)})
        emu.memory.register_exec(addr, cb)

    from desmume.controls import Keys, keymask
    walk = parse_inputs(a.inputs)
    cycle = ["A", "START", "X", "A", "Y", "B", "UP", "A", "DOWN", "A", "LEFT", "RIGHT", "START", "B"]
    for f in range(a.frames):
        ph["f"] = f
        if a.explore:
            k = (f // 40) % len(cycle)
            pressed = cycle[k] if f % 40 < 10 else None
            emu.input.keypad_update(keymask(getattr(Keys, "KEY_" + pressed)) if pressed else 0)
        elif f in walk:
            emu.input.keypad_update(walk[f])
        emu.cycle(with_joystick=False)
        if f % 300 == 0:
            ev.append({"h": "map", "f": f, "map": ram.s16(GROUND_MAP)})

    with open(a.out, "w") as fh:
        json.dump(ev, fh, indent=1)
    hits = [e for e in ev if e["h"] in HOOKS.values()]
    print("hook hits", len(hits), [(e["h"], e["f"]) for e in hits][:40])
    print("maps", [(e["f"], e["map"]) for e in ev if e["h"] == "map"])


if __name__ == "__main__":
    import romarg
    romarg.fill()
    main()
