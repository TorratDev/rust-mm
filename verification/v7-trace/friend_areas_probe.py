#!/usr/bin/env python3
"""Runtime hooks for the friend-area notes (#594, docs/re/progression/p4-friend-areas.md P4).

One boot per run: boot_to_dungeon.txt from a fresh save until the first dungeon floor, a snapshot
of the owned set, then optional turn inputs from that frame. Exec hooks (addresses are the
retail EUR dump, see docs/re/progression/p4-friend-areas.md P4):
  owned set : 02069848 (bulk setter), 02009014 (setter, r0 = area), 02003510 (new hero/partner)
              At the first floor the 58 owned bytes at *020DED04 are read (area 0 = index 0).
  exit      : 0206DC20 (entry: r0, result code at gDungeon+0x60), 0206ED24, 0206ED3C, 0206ED4C,
              02177FD8, 02178068 (entry: r0 = flag)
  gate      : 0217F7EC (entry: r0 = species; record +0x12, +0x16 and the owned byte of that area),
              return sites 0217FB40, 02172168, 0217F274 (r0 = the gate's return value)
  writes    : 020E7B6C+9 and +26 (owned bytes for areas 9 and 26 on the fresh-save path; the
              writer's pc is logged). The base is read again at the floor as *020DED04.

Keys in the input file are "FRAME KEYS" lines, as in retail_trace.py. Nothing is written to the
repository; the JSON goes to OUT. A hook that never fires is reported as not reached, not as a
negative result for the code it guards. The two write watches use their own "write:area9" and
"write:area26" names in not_reached; zero write events mean the writer was not observed, not that
no writer exists.

Usage: friend_areas_probe.py ROM OUT.json [--turn-inputs FILE] [--frames N]
Keep ROM and OUT outside the repository; use a throwaway copy of the ROM (DeSmuME writes
<rom>.dsv next to the ROM name in ~/.config/desmume).
"""
import argparse
import json
import os
import sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME  # noqa: E402
from retail_trace import G_DUNGEON, Ram, parse_inputs  # noqa: E402

OWNED_PTR = 0x020DED04       # *ptr = 58 owned bytes, index = area id
SPECIES_FLAG_PTR = 0x020022A0  # reader 02002288: *(*ptr) = species base, stride 0x48, +0x12 flag
SPECIES_AREA_PTR = 0x020022F4  # reader 020022DC: *(*ptr) = species base (its own literal), +0x16 area
REC, FRIEND, AREA = 0x48, 0x12, 0x16
N_AREAS = 58
SPECIES_COUNT = 0x1A8        # 424 records, static loop bound; the table is in VRAM, not main RAM
FRESH_OWNED_BASE = 0x020E7B6C  # fresh-save base observed at the first floor (*020DED04)
MAIN_RAM = (0x02000000, 0x02400000)
VRAM = (0x06000000, 0x06800000)  # ARM9 VRAM; the monspara copy starts at 0x06608000
SETTERS = (0x02069848, 0x02009014, 0x02003510)
EXIT = (0x0206DC20, 0x0206ED24, 0x0206ED3C, 0x0206ED4C, 0x02177FD8, 0x02178068)
GATE = 0x0217F7EC
GATE_RET = (0x0217FB40, 0x02172168, 0x0217F274)
RESULT_CODE = 0x60           # s16 at gDungeon+0x60 (exit code, as in exit_probe.py)
WRITE_WATCHES = ((9, "area9"), (26, "area26"))  # offsets from FRESH_OWNED_BASE


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--turn-inputs", default=os.path.join(HERE, "combat_attack_inputs.txt"))
    p.add_argument("--frames", type=int, default=1500)
    p.add_argument("--force-kind", type=int, default=0,
                   help="FORCED gate: write gDungeon+0x11 = KIND at turn frame --force-frame (0 = off)")
    p.add_argument("--force-frame", type=int, default=0)
    p.add_argument("--force-over", type=int, default=0,
                   help="FORCED: post-loop 0219C868 returns this (nonzero gives code -1 or -2)")
    p.add_argument("--force-plus6", type=int, default=-1,
                   help="FORCED: write gDungeon+6 = this at --force-frame (-1 = leave it)")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    boot = parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
    events = []
    writes = []
    phase = {"floor": False, "frame": 0}
    last_gate = {}

    def on_owned_write(label):
        def cb(addr, size):
            writes.append({"hook": "write:" + label, "frame": phase["frame"],
                           "floor": phase["floor"], "addr": hex(int(addr)),
                           "pc": hex(int(R.r15) & ~1), "value": ram.u8(int(addr))})
        return cb

    for off, label in WRITE_WATCHES:
        emu.memory.register_write(FRESH_OWNED_BASE + off, on_owned_write(label), 1)

    def hook(name, fn=None):
        def cb(addr, size):
            e = {"hook": name, "frame": phase["frame"], "floor": phase["floor"],
                 "r0": int(R.r0), "r1": int(R.r1), "lr": hex(int(R.r14) & ~1)}
            if fn:
                fn(e)
            events.append(e)
        return cb

    def on_gate(e):
        sp = e["r0"]
        f12 = ram.u8(ram.u32(ram.u32(SPECIES_FLAG_PTR)) + sp * REC + FRIEND)
        f16 = ram.u8(ram.u32(ram.u32(SPECIES_AREA_PTR)) + sp * REC + AREA)
        owned = ram.u8(ram.u32(OWNED_PTR) + f16) if f16 < N_AREAS and ram.u32(OWNED_PTR) else None
        e.update(species=sp, plus12=f12, plus16=f16, owned_byte=owned)
        last_gate.clear()
        last_gate.update(e)

    def on_ret(name):
        def fn(e):
            e["gate_species"] = last_gate.get("species")
        return fn

    setup = {}

    def on_exit(e):
        d = ram.u32(G_DUNGEON)
        if d and e["hook"] == "0206DC20":
            e["code"] = ram.s16(d + RESULT_CODE)
        if e["hook"] == "0206DC20":
            setup["p"] = e["r0"]  # RunDungeon_Async(setup): its exit code is at setup+0x60
        elif setup.get("p"):
            e["setup_code"] = ram.s16(setup["p"] + 0x60)

    for s in SETTERS:
        emu.memory.register_exec(s, hook("%08X" % s))
    for x in EXIT:
        emu.memory.register_exec(x, hook("%08X" % x, on_exit))
    emu.memory.register_exec(GATE, hook("%08X" % GATE, on_gate))
    for r in GATE_RET:
        emu.memory.register_exec(r, hook("%08X" % r, on_ret("%08X" % r)))

    # FORCED floor end: IsFloorOver (0214067C) is called at 0206E6A4 in the turn do-while, return
    # site 0206E6A8. The first return after --force-frame is set to 1, so the do-while ends and
    # gDungeon+0x11 (--force-kind) picks the exit path. Every other return is the game's own.
    forced_end = {}

    def on_floor_over_ret(addr, size):
        if a.force_kind and phase.get("turns") and not forced_end:
            forced_end.update(frame=phase["frame"], r0_natural=int(R.r0) & 0xFFFFFFFF)
            R.r0 = 1
    emu.memory.register_exec(0x0206E6A8, on_floor_over_ret)

    # FORCED exit test: 0219C868 (0206EB6C, return 0206EB70) is the post-loop "game over" test. A
    # nonzero return with gDungeon+6 == 0 gives code -1 (02177FD8, then 02178068(0)); with +6 != 0,
    # code -2 (02178068(0)). --force-over sets that return; --force-plus6 writes gDungeon+6.
    over_done = {}

    def on_868_ret(addr, size):
        if a.force_over and phase.get("turns") and not over_done:
            over_done.update(frame=phase["frame"], r0_natural=int(R.r0) & 0xFFFFFFFF)
            R.r0 = a.force_over
    emu.memory.register_exec(0x0206EB70, on_868_ret)

    for f in range(30000):
        phase["frame"] = f
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
            break
    else:
        sys.exit("no dungeon floor reached from boot_to_dungeon.txt")
    emu.input.keypad_update(0)
    for _ in range(60):
        emu.cycle(with_joystick=False)

    phase["floor"] = True
    base = ram.u32(OWNED_PTR)
    owned = list(ram.bytes(base, N_AREAS)) if base else []
    flag_base = ram.u32(ram.u32(SPECIES_FLAG_PTR))
    area_base = ram.u32(ram.u32(SPECIES_AREA_PTR))
    in_window = lambda v, w: w[0] <= v < w[1]
    # 0x06609AD0 is the monspara first record in VRAM (0x06608000 + 0x1AD0), so a main-RAM-only
    # check would wrongly reject the correct base. Read the full 424-record table, not past it.
    friend_flags = {s: ram.u8(flag_base + s * REC + FRIEND) for s in range(SPECIES_COUNT)}
    friend_hist = Counter(friend_flags.values())
    nonzero = [s for s, v in friend_flags.items() if v]
    snapshot = {"frame": phase["frame"], "owned_ptr": hex(base), "area0": owned[0] if owned else None,
                "owned_areas": [i for i, v in enumerate(owned) if v],
                "literal_flag": hex(ram.u32(SPECIES_FLAG_PTR)), "literal_area": hex(ram.u32(SPECIES_AREA_PTR)),
                "cell_020DDF80": ram.bytes(0x020DDF80, 4).hex(), "roster_cell_020DDF84": hex(ram.u32(0x020DDF84)),
                "species_base": hex(flag_base), "area_base": hex(area_base),
                "flag_base_in_main_ram": in_window(flag_base, MAIN_RAM),
                "flag_base_in_vram": in_window(flag_base, VRAM),
                "flag_base_mapped": in_window(flag_base, MAIN_RAM) or in_window(flag_base, VRAM),
                "area_base_in_main_ram": in_window(area_base, MAIN_RAM),
                "area_base_in_vram": in_window(area_base, VRAM),
                "area_base_mapped": in_window(area_base, MAIN_RAM) or in_window(area_base, VRAM),
                "friend_flag_values": dict(friend_hist),
                "friend_flag_nonzero": nonzero,
                "friend_flag_nonzero_count": len(nonzero)}

    turns = parse_inputs(a.turn_inputs) if a.turn_inputs else {}
    t0 = phase["frame"]
    forced = None
    phase["turns"] = True
    for k in range(a.frames):
        phase["frame"] = t0 + k
        if k in turns:
            emu.input.keypad_update(turns[k])
        if a.force_kind and k == a.force_frame:
            g = ram.u32(G_DUNGEON)
            emu.memory.write(g + 0x11, g + 0x12, 1, bytes([a.force_kind & 0xFF]))
            forced = {"kind": a.force_kind, "frame": t0 + k, "gDungeon": hex(g),
                      "byte_0x11_after_write": ram.u8(g + 0x11)}
            if a.force_plus6 >= 0:
                emu.memory.write(g + 6, g + 7, 1, bytes([a.force_plus6 & 0xFF]))
                forced["plus6_after_write"] = ram.u8(g + 6)
        emu.cycle(with_joystick=False)
    emu.input.keypad_update(0)
    if forced is not None:
        forced["floor_end"] = forced_end or None
        forced["post_loop_868"] = over_done or None

    fired_hooks = {e["hook"] for e in events}
    fired_writes = {w["hook"] for w in writes}
    not_reached = ["%08X" % x for x in SETTERS + EXIT + (GATE,) + GATE_RET
                   if "%08X" % x not in fired_hooks]
    not_reached += ["write:" + label for _, label in WRITE_WATCHES
                    if "write:" + label not in fired_writes]
    summary = {"hooks": dict(Counter(e["hook"] for e in events)),
               "write_hooks": dict(Counter(w["hook"] for w in writes)),
               "owned_write_events": len(writes),
               "not_reached": not_reached}
    res = {"snapshot": snapshot, "summary": summary, "events": events, "owned_writes": writes,
           "forced": forced}
    with open(a.out, "w") as f:
        json.dump(res, f, indent=1)
    print(json.dumps({"snapshot": {k: snapshot[k] for k in ("owned_ptr", "area0", "owned_areas",
                                                             "literal_flag", "literal_area",
                                                             "species_base", "flag_base_in_main_ram",
                                                             "flag_base_in_vram", "flag_base_mapped",
                                                             "area_base", "area_base_mapped",
                                                             "friend_flag_nonzero_count",
                                                             "friend_flag_nonzero")},
                      "friend_flag_value_counts": snapshot["friend_flag_values"],
                      "summary": summary}, indent=1), flush=True)
    emu.destroy()


if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
