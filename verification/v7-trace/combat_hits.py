"""Capture CalcDamage hits on retail Blue with py-desmume (#185).

Boots the ROM and plays boot_to_dungeon.txt, then plays turn_inputs.txt on the first floor.
An entry hook on CalcDamage (ov_0001::0217BD94) records the arguments, the attacker and
target info structs (entity+0x70), gDungeonRngState and the return address. Return hooks at
the four call sites record the dmg_struct and the gDungeon working values. One JSON line per
hit goes to OUT/hits.jsonl. At the first hit the lookup tables are read from RAM into
OUT/tables.json (overlay 1 is resident there during a dungeon).

Nothing from the ROM is written into the repository: OUT must be outside it.

Usage:
  combat_hits.py ROM OUT [--frames N] [--inputs FILE]
"""

import argparse
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pydesmume_dump as h  # noqa: E402
from desmume.emulator import DeSmuME  # noqa: E402

CALC_DAMAGE = 0x0217BD94
GDUNGEON_PTR = 0x0217C79C  # literal holding &gDungeon, as CalcDamage loads it
G_DUNGEON = 0x020E0788  # pointer to the dungeon struct (retail_trace.py)
RNG_STATE = 0x021B3F58  # gDungeonRngState
RET_SITES = [0x021567C8, 0x02161B50, 0x02161D5C, 0x0215672C]  # BL sites, from the x-ref
INFO_LEN = 0x140
GD_LO, GD_HI = 0x134, 0x16C  # damageCalcWorkingValues
# Lookup tables in overlay 1: type chart 18x18 s16, effectiveness 4x4 s32, stat stages 21 x s24.8
TABLES = {
    "typeChart": (0x021AAEE8, 18 * 18 * 2),
    "effectiveness": (0x021AA588, 4 * 4 * 4),
    "atkStageMul": (0x021AA660, 21 * 4),
    "defStageMul": (0x021AA6B4, 21 * 4),
    "physicalByType": (0x02091E78, 18),  # 1 = physical; the table the pointer at 0x02008C70 holds
}


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=9000, help="turn frames after the dungeon starts")
    p.add_argument("--inputs", default=os.path.join(HERE, "combat_inputs.txt"), help="turn input file")
    a = p.parse_args()
    os.makedirs(a.out, exist_ok=True)

    emu = DeSmuME()
    emu.open(a.rom)
    regs = emu.memory.register_arm9
    mem = emu.memory

    def rd(addr: int, n: int) -> bytes:
        if n == 1:  # py-desmume returns an int for a 1-byte read
            return bytes([mem.read(addr, addr, 1, False)])
        return bytes(mem.read(addr, addr + n, 1, False))

    def rd32(addr: int) -> int:
        return int.from_bytes(rd(addr, 4), "little")

    frame_no = [0]
    pending = None
    tables_done = [False]
    out = open(os.path.join(a.out, "hits.jsonl"), "w")
    count = {"entry": 0, "hit": 0}

    def on_entry(*_):
        nonlocal pending
        sp = regs.r13
        attacker, target = regs.r0, regs.r1
        a_info = rd32(attacker + 0x70)
        t_info = rd32(target + 0x70)
        pending = {
            "frame": frame_no[0],
            "lr": regs.r14,
            "attacker": attacker,
            "target": target,
            "moveType": regs.r2 & 0xFF,
            "movePower": regs.r3,
            "critChance": rd32(sp),
            "dmgStruct": rd32(sp + 4),
            "arg8": rd32(sp + 8),
            "moveId": int.from_bytes(rd(sp + 0xC, 2), "little"),
            "arg10": rd(sp + 0x10, 1)[0],
            "rng_before": rd(RNG_STATE, 4).hex(),
            "a_info": rd(a_info, INFO_LEN).hex(),
            "t_info": rd(t_info, INFO_LEN).hex(),
        }
        count["entry"] += 1
        if not tables_done[0]:
            tables_done[0] = True
            tables = {name: rd(addr, n).hex() for name, (addr, n) in TABLES.items()}
            with open(os.path.join(a.out, "tables.json"), "w") as f:
                json.dump(tables, f)

    def make_exit(site: int):
        def on_exit(*_):
            nonlocal pending
            if pending is None or pending["lr"] != site:
                return
            rec, pending = pending, None
            gd = rd32(rd32(GDUNGEON_PTR))
            rec["site"] = hex(site)
            rec["rng_after"] = rd(RNG_STATE, 4).hex()
            rec["dmg_struct"] = rd(rec.pop("dmgStruct"), 16).hex()
            rec["gDungeon_0x134_0x16C"] = rd(gd + GD_LO, GD_HI - GD_LO).hex()
            out.write(json.dumps(rec) + "\n")
            out.flush()
            count["hit"] += 1

        return on_exit

    emu.memory.register_exec(CALC_DAMAGE, on_entry)
    for site in RET_SITES:
        emu.memory.register_exec(site + 4, make_exit(site + 4))

    # Boot until the dungeon struct exists, the same as retail_trace.py.
    boot = h.parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
    start = time.monotonic()
    for f in range(30000):
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and rd32(G_DUNGEON) != 0:
            print(f"dungeon frame {f} ({time.monotonic() - start:.1f}s)", flush=True)
            break
    else:
        sys.exit("no dungeon reached")

    turns = h.parse_inputs(a.inputs)
    for f in range(a.frames):
        frame_no[0] = f
        if f in turns:
            emu.input.keypad_update(turns[f])
        emu.cycle(with_joystick=False)
    out.close()
    print(f"{a.frames} turn frames: {count['entry']} entries, {count['hit']} hits", flush=True)
    emu.destroy()


if __name__ == "__main__":
    main()
