#!/usr/bin/env python3
"""#362 P1 probe: recompute each installed script pointer from the bank and loader tables.
At FUN_02147C48 entry (installs a reference on a context): if lr is in FUN_0214D420, walk
bank table (literal 0x0214FC48 -> root[bank*8]) -> group[g] {count, sectors} -> sector[s]
(0x28 B; +0x20 count, +0x24 list) -> *list -> +8 and compare with reference word 0.
If lr is in FUN_0214D498, read the loader table (literal 0x02147ED4) entry index*12 word 0.
Usage: python script_lookup_probe.py ROM OUT FRAMES INPUTS
"""
import json, os, sys
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, sys.argv[5] if len(sys.argv) > 5 else os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs

INST, BANKLOAD, IDXLOAD = 0x02147C48, 0x0214D420, 0x0214D498
def main():
    rom, out_path, frames, inp = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    emu = DeSmuME(); emu.open(rom); boot = parse_inputs(inp); st = {"f": 0, "args": None, "idx": None}
    out = open(out_path, "w", encoding="utf-8", newline="\n")
    def w32(a): return int.from_bytes(bytes(emu.memory.read(a, a + 4, 1, False)), "little")
    def w16(a): return int.from_bytes(bytes(emu.memory.read(a, a + 2, 1, False)), "little")
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    def ev(name, **kw): out.write(json.dumps({"ev": name, "f": st["f"], **kw}, sort_keys=True) + "\n")
    def on_bank(*_): st["args"] = (reg("r0"), reg("r1"), reg("r2"), reg("r3"))
    def on_idx(*_): st["idx"] = reg("r0")
    def on_inst(*_):
        lr, ref = reg("r14"), reg("r2")
        got, kind, grp, sec = w32(ref), w16(ref + 4), w16(ref + 6), w32(ref + 8) & 0xFF
        op0 = w32(got) & 0xFF
        if BANKLOAD <= lr < IDXLOAD and st["args"]:
            b, g, s, fl = st["args"]
            ovl_tab, root_tab = w32(0x0214FD90), w32(0x0214FC48)
            root = w32(root_tab + 8 * b); grp_e = w32(root + 4) + 8 * g
            sec_e = w32(grp_e + 4) + 0x28 * s; lst = w32(sec_e + 0x24)
            want = w32(w32(lst) + 8)
            ev("bank_install", bank=b, group=g, sector=s, flag=fl, overlay=w32(ovl_tab + 8 * b),
               ovl_tab=hex(ovl_tab), root_tab=hex(root_tab), groups=w32(root), sectors=w32(grp_e),
               list_count=w32(sec_e + 0x20), got=hex(got), want=hex(want), match=got == want,
               kind=kind, ref_group=grp, ref_sector=sec, first_op=hex(op0), in_bank_area=got >= 0x0215D720)
        elif IDXLOAD <= lr < IDXLOAD + 0x60 and st["idx"] is not None:
            tab = w32(0x02147ED4); i = st["idx"]; want = w32(tab + 12 * i)
            ev("loader_install", index=i, tab=hex(tab), got=hex(got), want=hex(want), match=got == want,
               kind=kind, first_op=hex(op0), in_arm9=got < 0x02138860)
    for ad, cb in [(INST, on_inst), (BANKLOAD, on_bank), (IDXLOAD, on_idx)]:
        emu.memory.register_exec(ad, cb)
    for f in range(frames):
        st["f"] = f
        if f in boot: emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
    out.close(); emu.destroy()
if __name__ == "__main__": main()
