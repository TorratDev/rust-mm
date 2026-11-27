#!/usr/bin/env python3
"""Natural leader defeat on the first floor: the run's result code -1 (#641).

Nothing is forced. The probe boots the normal route (boot_to_dungeon.txt), then presses A every --a-every
frames so the leader keeps attacking, and does nothing else. It logs the leader's HP every --log-every frames
(team slot 0 entity at dungeon +0x11364, info at entity +0x70, HP s16 at info +0xE, max HP at info +0x10),
and the run's exit hooks with the result code read at the run struct's +0x60. The run struct is the entry r0
of 0206DC20 (the first hit on the boot route, as in friend_areas_probe.py and escape_orb_probe.py).

Usage: defeat_probe.py ROM OUT.json [--frames N] [--a-every N] [--log-every N]
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

TEAM0 = 0x11364
EXIT = (0x0206ED24, 0x0206ED3C, 0x0206ED4C, 0x02177FD8, 0x02178068)
RUN = 0x0206DC20
RESULT_CODE = 0x60


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=60000)
    p.add_argument("--a-every", type=int, default=30)
    p.add_argument("--log-every", type=int, default=300)
    p.add_argument("--force-hp", type=int, default=None,
                   help="FORCED: set the leader's HP (info+0xE, s16) after boot, so a monster hit can KO it")
    p.add_argument("--force-rescues", type=int, default=None,
                   help="FORCED: write this distress-call count at gDungeon+0x66B (Blue unk644.unk37, inferred "
                        "from Red's layout, not checked) after boot. Red: a count above 0 offers the distress-call "
                        "prompt on a leader faint; yes sets the abandon flag, and the next exit is code -2.")
    a = p.parse_args()
    FAINT = 0x021778F4  # HandleFaint_Async (docs/re/combat.md)
    RESCUES = 0x66B

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    entry = {}
    events = []
    frame = {"n": 0}

    def on_run(addr, size):
        if "r0" not in entry:
            entry["r0"] = int(R.r0)
        events.append({"hook": "%08X" % RUN, "frame": frame["n"], "entry": True})

    def hook(name):
        def cb(addr, size):
            e = {"hook": name, "frame": frame["n"], "r0": int(R.r0)}
            if "r0" in entry:
                e["code"] = ram.s16(entry["r0"] + RESULT_CODE)
            events.append(e)
        return cb

    emu.memory.register_exec(RUN, on_run)
    for x in EXIT:
        emu.memory.register_exec(x, hook("%08X" % x))

    def on_faint(addr, size):
        dg = ram.u32(G_DUNGEON)
        events.append({"hook": "%08X" % FAINT, "frame": frame["n"], "r0": hex(int(R.r0)),
                       "r1": hex(int(R.r1)),  # exit reason (Red's DungeonExitReason values for the non-KO cases)
                       "unk37": (lambda v: v - 256 if v >= 128 else v)(ram.u8(dg + RESCUES)) if dg else None,
                       "missionKind": ram.u8(dg + 0x668) if dg else None})

    def on_ask(addr, size):
        # AskYesNo(portrait, msg, flag): the distress-call prompt is the one from HandleFaint_Async.
        events.append({"hook": "AskYesNo", "frame": frame["n"], "msg": int(R.r1)})

    emu.memory.register_exec(FAINT, on_faint)
    emu.memory.register_exec(0x02153E80, on_ask)

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

    d = ram.u32(G_DUNGEON)
    leader = ram.u32(d + TEAM0)
    info = ram.u32(leader + 0x70) if leader else 0
    if a.force_hp is not None and info:
        # FORCED: leader HP (s16 at info+0xE) set to --force-hp after boot, so a monster hit ends the run by KO.
        emu.memory.write_byte(info + 0xE, a.force_hp & 0xFF)
        emu.memory.write_byte(info + 0xF, (a.force_hp >> 8) & 0xFF)
    if a.force_rescues is not None:
        # FORCED: the distress-call count (see --force-rescues). Written as one signed byte.
        emu.memory.write_byte(d + RESCUES, a.force_rescues & 0xFF)
        res_forced = {"offset": hex(RESCUES), "value": a.force_rescues}
    else:
        res_forced = None
    log = []
    res = {"run_entry_r0": hex(entry["r0"]) if "r0" in entry else None, "log": log, "forced": res_forced}
    last_code = {"v": None}
    for f in range(a.frames):
        frame["n"] = f
        keys = keymask(Keys.KEY_A) if f % a.a_every < 4 else 0
        emu.input.keypad_update(keys)
        emu.cycle(with_joystick=False)
        if f % a.log_every == 0 and info:
            log.append({"frame": f, "hp": ram.s16(info + 0xE), "maxhp": ram.s16(info + 0x10),
                        "level": ram.u8(info + 9), "dungeon": ram.u32(G_DUNGEON) != 0})
        if events and any(e.get("hook") in ("0206ED24", "0206ED3C", "0206ED4C", "02177FD8", "02178068")
                          for e in events) and f % a.log_every == 0:
            break
        if ram.u32(G_DUNGEON) == 0 and f > 0:
            # gDungeon freed: the run ended.
            break
    emu.input.keypad_update(0)
    res["events"] = events
    res["final_frame"] = frame["n"]
    res["hook_counts"] = {h: sum(1 for e in events if e.get("hook") == h)
                          for h in {e.get("hook") for e in events if e.get("hook")}}
    with open(a.out, "w") as f:
        json.dump(res, f, indent=1)
    print(json.dumps({"run_entry_r0": res["run_entry_r0"], "final_frame": res["final_frame"],
                      "hook_counts": res["hook_counts"], "last_log": log[-1] if log else None},
                     indent=1), flush=True)
    emu.destroy()


if __name__ == "__main__":
    main()
