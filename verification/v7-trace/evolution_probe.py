#!/usr/bin/env python3
"""Runtime hooks for the dungeon-evolution notes (#573, docs/re/progression/p6-evolution.md P6.2).

FORCED run, not a natural one: with --force, info+0x11B is set to 1 on every team/wild entity and
gDungeon+0xD to 1 at the first floor, so 02183BB4 takes its evolution branch. Natural runs never set
+0x11B for team members (P6.2). Boot: boot_to_dungeon.txt to the first floor, 60 frames, then the
savestate (created on the first run, loaded afterwards if --savestate exists). Then
combat_attack_inputs.txt for --frames frames.

Exec hooks (retail EUR dump addresses, docs/re/progression/p6-evolution.md P6.2):
  02183BB4  evolution pass entry (main; records gDungeon+0xD, +0x10 and each slot's species)
  02001738  candidate list (r0 = species, r1 = buffer, r2 = 0, r3 = 0)
  0219D058  DungeonRandInt (r0 = candidate count)
  02183DE0  return of 021389A0 (floor species list; --force-returns sets r0 = 1)
  02183DF8  return of 02178C04 inside 02183BB4 (--force-returns sets r0 = 1)
  02182900  apply (entry: r1 = entity; reads info+2, +4, +0x18, +9)
  0218293C  return of 02178C04 inside apply (--force-returns sets r0 = 1)

A hook that never fires means "not reached", not a negative result. Nothing is written to the
repository; the JSON goes to OUT. Keep the ROM and OUT outside the repository, and use a throwaway
copy of the ROM name (DeSmuME writes <rom>.dsv next to the ROM name in ~/.config/desmume).

Usage: evolution_probe.py ROM OUT.json [--force] [--force-returns] [--frames N] [--savestate PATH]
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
from retail_trace import G_DUNGEON, Ram, parse_inputs  # noqa: E402

ENTITY_ARRAY = 0x11364  # gDungeon + 0x11364, 20 slots of entity pointers
INFO_SPECIES, INFO_TARGET, INFO_LEVEL = 2, 0x4, 0x9  # info+2 species, +4 target, +9 level
INFO_FORCE, INFO_EXP = 0x11B, 0x1C8
DUNGEON_FLAG, DUNGEON_MODE = 0xD, 0x10
BOOT_LIMIT = 30000


def entities(ram):
    d = ram.u32(G_DUNGEON)
    out = []
    for i in range(20):
        e = ram.u32(d + ENTITY_ARRAY + i * 4)
        if e:
            out.append((i, e, ram.u32(e + 0x70)))  # e+0x70 = info pointer
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--force", action="store_true", help="set info+0x11B and gDungeon+0xD")
    p.add_argument("--force-returns", action="store_true",
                   help="force the 021389A0 and 02178C04 returns to 1 (lets apply run)")
    p.add_argument("--frames", type=int, default=480)
    p.add_argument("--savestate", default=None, help="load if present, else save here")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    mem = emu.memory
    ev = []
    ph = {"f": 0}

    def rec(name, fn=None):
        def cb(addr, size):
            x = {"h": name, "f": ph["f"], "r0": int(R.r0), "r1": int(R.r1), "r2": int(R.r2),
                 "lr": hex(int(R.r14) & ~1)}
            if fn:
                fn(x)
            ev.append(x)
        return cb

    def on_main(x):
        d = ram.u32(G_DUNGEON)
        x["flag_D"] = ram.u8(d + DUNGEON_FLAG)
        x["mode_10"] = ram.u8(d + DUNGEON_MODE)
        x["ents"] = [(i, ram.s16(inf + INFO_SPECIES), ram.u8(inf + INFO_LEVEL),
                      ram.u8(inf + INFO_FORCE), ram.u32(inf + INFO_EXP))
                     for i, _, inf in entities(ram)]

    def on_apply(x):
        inf = ram.u32(x["r1"] + 0x70)
        x["before"] = (ram.s16(inf + 2), ram.s16(inf + 4), ram.u32(inf + 0x18), ram.u8(inf + 9))

    for addr, name, fn in [(0x02183BB4, "main", on_main), (0x02001738, "cand", None),
                           (0x0219D058, "rand", None), (0x02182900, "apply", on_apply),
                           (0x02178C04, "tbl", None)]:
        mem.register_exec(addr, rec(name, fn))

    def ret_forced(name):
        def fn(x):
            x[name] = int(R.r0)
            if a.force_returns:
                R.r0 = 1
                x["forced"] = 1
        return fn

    mem.register_exec(0x02183DE0, rec("inlist_ret", ret_forced("ret")))
    mem.register_exec(0x02183DF8, rec("tbl_ret", ret_forced("tblret")))
    mem.register_exec(0x0218293C, rec("tbl_in_apply_ret", ret_forced("tbl2")))

    if a.savestate and os.path.exists(a.savestate):
        emu.savestate.load_file(a.savestate)
    else:
        boot = parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
        for f in range(BOOT_LIMIT):
            ph["f"] = f
            if f in boot:
                emu.input.keypad_update(boot[f])
            emu.cycle(with_joystick=False)
            if f % 10 == 0 and ram.u32(G_DUNGEON):
                break
        emu.input.keypad_update(0)
        for _ in range(60):
            emu.cycle(with_joystick=False)
        if a.savestate:
            emu.savestate.save_file(a.savestate)

    print("floor entities (slot, species, level):",
          [(i, ram.s16(inf + INFO_SPECIES), ram.u8(inf + INFO_LEVEL)) for i, _, inf in entities(ram)])
    if a.force:
        d = ram.u32(G_DUNGEON)
        for _, _, inf in entities(ram):
            mem.write_byte(inf + INFO_FORCE, 1)
        mem.write_byte(d + DUNGEON_FLAG, 1)

    turns = parse_inputs(os.path.join(HERE, "combat_attack_inputs.txt"))
    for k in range(a.frames):
        ph["f"] += 1
        if k in turns:
            emu.input.keypad_update(turns[k])
        emu.cycle(with_joystick=False)

    with open(a.out, "w") as fh:
        json.dump(ev, fh, indent=1)
    print("events", len(ev), [(e["h"], e["f"]) for e in ev][:40])
    print("END", [(i, ram.s16(inf + INFO_SPECIES), ram.s16(inf + INFO_TARGET), ram.u8(inf + INFO_LEVEL),
                   ram.u8(inf + INFO_FORCE)) for i, _, inf in entities(ram)])


if __name__ == "__main__":
    main()
