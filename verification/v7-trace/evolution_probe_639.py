#!/usr/bin/env python3
"""Runtime checks for #639 on the dungeon-evolution notes (docs/re/progression/p6-evolution.md P6.2).

Same boot and forced setup as evolution_probe.py (info+0x11B = 1, gDungeon+0xD = 1 on every entity),
plus:
  --trace-apply     exec hook on every word of apply 02182900..02182AA0; each apply call logs the
                    words it executed, so the path taken (and where it leaves) is visible
  --read-sprites    at the evolution pass entry, read gDungeon+0x158DC+species*4 for every entity
                    species (the loaded-sprite pointer the apply path tests)
  --force-316 N     when the candidate list of a species contains 316, DungeonRandInt returns the index
                    of 316 instead (the pick is made by the test, not by the game's draw)
  --force-317       force the return of 021389A0 for the 316 spawn check (argument 0x13D = 317)
  --inject-for S --inject-target T
                    when the candidate list of species S is built, its first entry is replaced by T
                    (the draw then picks T; the list is the game's, only the first entry changes)

Usage: evolution_probe_639.py ROM OUT.json [--frames N] [--savestate PATH] [--force-returns]
       [--trace-apply] [--read-sprites] [--force-316 SPECIES] [--force-317]
       [--inject-for S --inject-target T]
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

ENTITY_ARRAY = 0x11364
SPRITE_POINTERS = 0x158DC
INFO_SPECIES, INFO_TARGET, INFO_LEVEL = 2, 0x4, 0x9
INFO_HP, INFO_MAX_HP, INFO_EXP_TOTAL = 0xE, 0x10, 0x18
INFO_FORCE, INFO_PENDING_EXP = 0x11B, 0x1C8
DUNGEON_FLAG, DUNGEON_MODE = 0xD, 0x10
APPLY_START, APPLY_END = 0x02182900, 0x02182AA0
BOOT_LIMIT = 30000


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=480)
    p.add_argument("--savestate", default=None)
    p.add_argument("--force-returns", action="store_true")
    p.add_argument("--trace-apply", action="store_true")
    p.add_argument("--read-sprites", action="store_true")
    p.add_argument("--force-316", type=int, default=None)
    p.add_argument("--force-317", action="store_true")
    p.add_argument("--inject-for", type=int, default=None)
    p.add_argument("--inject-target", type=int, default=None)
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    mem = emu.memory
    ev = []
    ph = {"f": 0}
    apply_log = {"on": False, "words": []}
    cand_buf = {"addr": 0}

    def ents():
        d = ram.u32(G_DUNGEON)
        out = []
        for i in range(20):
            e = ram.u32(d + ENTITY_ARRAY + i * 4)
            if e and ram.u32(e) != 0:
                out.append((i, ram.u32(e + 0x70)))
        return out

    def log(name, **kw):
        kw.update({"h": name, "f": ph["f"]})
        ev.append(kw)

    def on_main(addr, size):
        d = ram.u32(G_DUNGEON)
        rec = {"flag_D": ram.u8(d + DUNGEON_FLAG), "mode_10": ram.u8(d + DUNGEON_MODE),
               "bytes_63C": list(ram.bytes(d + 0x63C, 4)),
               "ents": []}
        for i, inf in ents():
            sp = ram.s16(inf + INFO_SPECIES)
            e = {"slot": i, "species": sp, "level": ram.u8(inf + INFO_LEVEL),
                 "hp": ram.s16(inf + INFO_HP), "exp": ram.u32(inf + INFO_EXP_TOTAL)}
            if a.read_sprites:
                e["sprite_ptr"] = hex(ram.u32(d + SPRITE_POINTERS + sp * 4)) if 0 <= sp < 0x1A8 else None
            rec["ents"].append(e)
        log("main", **rec)

    def on_cand_entry(addr, size):
        cand_buf["addr"] = int(R.r1)
        cand_buf["species"] = int(R.r0)
        log("cand", species=int(R.r0))

    def on_cand_ret(addr, size):
        n = min(int(R.r0), 8)
        buf = cand_buf["addr"]
        vals = [ram.s16(buf + 2 * k) for k in range(n)]
        if a.inject_for is not None and cand_buf.get("species") == a.inject_for and n:
            t = a.inject_target
            mem.write_byte(buf, t & 0xFF)
            mem.write_byte(buf + 1, (t >> 8) & 0xFF)
            vals[0] = t
            log("inject", species=a.inject_for, target=t)
        log("cand_ret", count=int(R.r0), values=vals)
        cand_buf["vals"] = vals

    def on_rand(addr, size):
        # return site of the DungeonRandInt call in the evolution pass
        vals = cand_buf.get("vals", [])
        r = int(R.r0)
        if a.force_316 is not None and a.force_316 in vals:
            R.r0 = vals.index(a.force_316)
            log("rand", drawn=r, forced_to=int(R.r0), candidates=vals)
        else:
            log("rand", drawn=r, candidates=vals)

    def on_apply_entry(addr, size):
        inf = ram.u32(int(R.r1) + 0x70)
        apply_log["on"], apply_log["info"] = True, inf
        log("apply", species_arg=int(R.r2), info=hex(inf),
            before={"species": ram.s16(inf + INFO_SPECIES), "target": ram.s16(inf + INFO_TARGET),
                    "hp": ram.s16(inf + INFO_HP), "exp": ram.u32(inf + INFO_EXP_TOTAL),
                    "level": ram.u8(inf + INFO_LEVEL)})
        if a.read_sprites:
            d = ram.u32(G_DUNGEON)
            ev[-1]["sprite_ptr_target"] = hex(ram.u32(d + SPRITE_POINTERS + int(R.r2) * 4))
        apply_log["ev"] = ev[-1]
        ev[-1]["path"] = []

    def on_apply_word(addr, size):
        if apply_log["on"] and len(apply_log["ev"]["path"]) < 2000:
            apply_log["ev"]["path"].append([ph["f"], hex(addr)])

    def on_join(addr, size):
        # common join 02182A74: snapshot after apply
        if apply_log["on"]:
            apply_log["on"] = False
            inf = apply_log["info"]
            apply_log["ev"]["after"] = {"species": ram.s16(inf + INFO_SPECIES),
                                        "target": ram.s16(inf + INFO_TARGET),
                                        "hp": ram.s16(inf + INFO_HP), "exp": ram.u32(inf + INFO_EXP_TOTAL),
                                        "level": ram.u8(inf + INFO_LEVEL)}
        log("join", at=hex(addr))

    mem.register_exec(0x02183BB4, on_main)
    mem.register_exec(0x02154B8C, lambda addr, size: log("logmsg", r2=hex(int(R.r2)), lr=hex(int(R.r14) & ~1)))
    mem.register_exec(0x02001738, on_cand_entry)
    mem.register_exec(0x02183DB8, on_cand_ret)
    mem.register_exec(0x02183DC4, on_rand)
    mem.register_exec(0x02182900, on_apply_entry)
    mem.register_exec(0x02182A74, on_join)
    if a.trace_apply:
        # 02182A74 is the join hook above; registering a word hook there would replace it
        for w in range(APPLY_START, APPLY_END, 4):
            if w not in (0x02182900, 0x02182A74):
                mem.register_exec(w, on_apply_word)

    if a.force_317:
        def on_317_call(addr, size):
            if int(R.r0) != 0x13D:
                return
            ret = int(R.r14) & ~1
            log("021389a0_317", lr=hex(ret))

            def ret_hook(addr2, size2):
                log("021389a0_317_ret", r0=int(R.r0))
                R.r0 = 1
            mem.register_exec(ret, ret_hook)
        mem.register_exec(0x021389A0, on_317_call)

    def ret_force(name):
        def fn(addr, size):
            log(name, r0=int(R.r0))
            if a.force_returns:
                R.r0 = 1
        return fn

    mem.register_exec(0x02183DE0, ret_force("inlist_ret"))
    mem.register_exec(0x02183DF8, ret_force("tbl_ret"))
    mem.register_exec(0x0218293C, ret_force("tbl_in_apply_ret"))

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

    d = ram.u32(G_DUNGEON)
    for _, inf in ents():
        mem.write_byte(inf + INFO_FORCE, 1)
    mem.write_byte(d + DUNGEON_FLAG, 1)

    turns = parse_inputs(os.path.join(HERE, "combat_attack_inputs.txt"))
    for k in range(a.frames):
        ph["f"] = k + 1
        if k in turns:
            emu.input.keypad_update(turns[k])
        emu.cycle(with_joystick=False)

    with open(a.out, "w") as fh:
        json.dump(ev, fh, indent=1)
    print("events", len(ev))
    print("END", [(i, ram.s16(inf + INFO_SPECIES), ram.s16(inf + INFO_TARGET), ram.u8(inf + INFO_LEVEL),
                   ram.u8(inf + INFO_FORCE)) for i, inf in ents()])


if __name__ == "__main__":
    import romarg
    romarg.fill()
    main()
