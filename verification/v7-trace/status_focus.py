#!/usr/bin/env python3
"""#359 Focus Energy at runtime: CalcDamage (0217BD94) with the attacker's `+0xC4 == 4`.

Usage: status_focus.py ROM OUT_DIR [FRAMES] [clear]

Sets `+0xC4` = 4, `+0xC5` = 4 on the team leader (or 0 with `clear`), then plays the scripted inputs.
Logs each CalcDamage call where r0 is the leader, with its `+0xC4` at entry and the return value.
The crit flag is not read directly: compare the returns with Focus Energy against the `clear` run.
Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_play import Probe, wild, hook_exit, play

if len(sys.argv) < 3:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2]); R = pr.R
frames = int(sys.argv[3]) if len(sys.argv) > 3 else 1500
clear = len(sys.argv) > 4 and sys.argv[4] == "clear"
L = pr.team[0]; li = pr.info(L)
# Place the leader one tile left and one down of the first wild monster, as the golden trace starts
# (leader (20,25), monster (21,24), attack up-right). Position is entity +0x04 (x) and +0x06 (y).
mons = wild(pr)
if mons:
    mx, my = pr.s16(mons[0] + 4), pr.s16(mons[0] + 6)
    lx, ly = mx - 1, my + 1
    for off, v in ((4, lx), (6, ly)):
        pr.w8(L + off, v & 0xFF); pr.w8(L + off + 1, (v >> 8) & 0xFF)
    print("monster at", (mx, my), "leader moved to", (lx, ly), flush=True)
pr.w8(li + 0xC4, 0 if clear else 4); pr.w8(li + 0xC5, 0 if clear else 4)


def on_entry():
    # Every CalcDamage call: attacker pointer, the leader's +0xC4 at entry, and the return value.
    c4 = pr.u8(li + 0xC4); att = R.r0
    hook_exit(pr, 0x0217BD94, lambda ret: print("calc_dmg attacker", hex(att), "leader +0xC4", c4, "ret", ret & 0xFFFFFFFF, flush=True))


pr.hook(0x0217BD94, on_entry)
play(pr, frames, lambda f: pr.u8(li + 0xC4), inputs="combat_attack_inputs.txt")
