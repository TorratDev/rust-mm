"""Capture CalcDamage hits on retail Blue with py-desmume (#185).

Boots the ROM and plays boot_to_dungeon.txt, then plays turn_inputs.txt on the first floor.
An entry hook on CalcDamage (ov_0001::0217BD94) records the arguments, the attacker and
target info structs (entity+0x70), gDungeonRngState and the return address. Return hooks at
the four call sites record the dmg_struct and the gDungeon working values. One JSON line per
hit goes to OUT/hits.jsonl. With a leader flag set (--focus, --flinch,
--paralysis), each leader entry to the move gate (02172ADC) and to the attack it calls
(0215B384) goes to OUT/gate.jsonl with the flinch and paralysis bytes (#624). Each change of the
leader's entity, position, HP bytes +0x0E..+0x13 or paralysis bytes goes to OUT/watch.jsonl. Every call to
the Attack / Sp. Atk lowerer (02189534) goes to OUT/gate.jsonl as fn 'lower'. At the first hit the lookup tables are read from RAM into
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
MOVE_GATE = 0x02172ADC  # flinch / paralysis gate (#624); void, the refusal is the branch it takes
LOWER_OFFENSE = 0x02189534  # lowers the target's Attack (stat 0) or Sp. Atk stage
GATE_ATTACK = 0x0215B384  # the attack call the gate makes only when not refused
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
    p.add_argument("--paralysis", action="store_true", help="set the team leader's +0x9B = 4, +0x9C = 9 (paralysis, #624)")
    p.add_argument("--flinch", action="store_true", help="set the team leader's +0xAC = 1, +0xAD = 9 (flinch, #624)")
    p.add_argument("--focus", choices=["none", "on", "off"], default="none",
                   help="set the team leader's +0xC4/+0xC5 (Focus Energy) at the first dungeon frame (#624)")
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
            tables["focusOdds_0217C7C8"] = rd32(0x0217C7C8)  # the literal itself is the odds (#624)
            tables["critMul_0217C7F4"] = rd(rd32(0x0217C7F4), 8).hex()  # literal points at the 48.16 crit factor
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

    leader = [0]  # leader entity, set after the team fills in
    gate_out = open(os.path.join(a.out, "gate.jsonl"), "w")

    def on_gate(name: str):
        def hook(*_):
            if leader[0] and regs.r0 == leader[0]:
                info = rd32(leader[0] + 0x70)
                gate_out.write(json.dumps({"frame": frame_no[0], "fn": name, "lr": regs.r14,
                                           "ac": rd(info + 0xAC, 1)[0], "ad": rd(info + 0xAD, 1)[0],
                                           "9b": rd(info + 0x9B, 1)[0], "9c": rd(info + 0x9C, 1)[0]}) + "\n")
        return hook

    def on_lower(*_):  # every call, any entity: who lowers whose Attack or Sp. Atk (#624)
        gate_out.write(json.dumps({"frame": frame_no[0], "fn": "lower", "lr": regs.r14, "user": regs.r0,
                                   "target": regs.r1, "stat": regs.r2, "amount": regs.r3,
                                   "t_1c": rd(rd32(regs.r1 + 0x70) + 0x1C, 4).hex()}) + "\n")

    emu.memory.register_exec(LOWER_OFFENSE, on_lower)
    emu.memory.register_exec(MOVE_GATE, on_gate("gate"))
    emu.memory.register_exec(GATE_ATTACK, on_gate("attack"))
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

    if a.focus != "none" or a.flinch or a.paralysis:
        for _ in range(120):  # let the team fill in after the dungeon struct appears (as status_probe does)
            emu.cycle(with_joystick=False)
        gd = rd32(G_DUNGEON)
        leader_info = rd32(rd32(gd + 0x11314) + 0x70)
        if leader_info < 0x02000000:
            sys.exit(f"leader info pointer not valid yet: {leader_info:#x}")
        if a.focus != "none":
            v = 4 if a.focus == "on" else 0
            mem.write_byte(leader_info + 0xC4, v)
            mem.write_byte(leader_info + 0xC5, v)
            print(f"leader info {leader_info:#x} +0xC4 set to {v}", flush=True)
        if a.paralysis:
            mem.write_byte(leader_info + 0x9B, 4)
            mem.write_byte(leader_info + 0x9C, 9)
            print(f"leader info {leader_info:#x} +0x9B set to 4", flush=True)
        if a.flinch:
            mem.write_byte(leader_info + 0xAC, 1)
            mem.write_byte(leader_info + 0xAD, 9)   # long window: the first attack comes near frame 1530
            print(f"leader info {leader_info:#x} +0xAC set to 1", flush=True)

    gd = rd32(G_DUNGEON)
    leader[0] = rd32(gd + 0x11314) if a.focus != "none" or a.flinch or a.paralysis else 0

    turns = h.parse_inputs(a.inputs)
    watch_out = open(os.path.join(a.out, "watch.jsonl"), "w")  # leader state changes, per frame (#624)
    last = None
    for f in range(a.frames):
        frame_no[0] = f
        if f in turns:
            emu.input.keypad_update(turns[f])
        emu.cycle(with_joystick=False)
        if leader[0]:
            ent = rd32(rd32(G_DUNGEON) + 0x11314)
            info = rd32(ent + 0x70) if ent >= 0x02000000 else 0
            st = {"ent": ent, "pos": rd(ent + 4, 4).hex() if info else "",
                  "hp_0e_14": rd(info + 0x0E, 6).hex() if info else "",
                  "9b": rd(info + 0x9B, 1)[0] if info else -1, "9c": rd(info + 0x9C, 1)[0] if info else -1}
            if st != last:
                watch_out.write(json.dumps({"frame": f, **st}) + "\n")
                last = st
    watch_out.close()
    out.close()
    gate_out.close()
    print(f"{a.frames} turn frames: {count['entry']} entries, {count['hit']} hits", flush=True)
    emu.destroy()


if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
