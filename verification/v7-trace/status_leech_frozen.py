#!/usr/bin/env python3
"""#359 Leech Seed frozen skip (`+0xA0 == 1`) at runtime: drains and end-of-turn calls with the flag set.

Usage: status_leech_frozen.py ROM OUT_DIR [FRAMES]

Writes the seed fields into the valid wild monsters as status_leech_seed.py does, and sets `+0xA0` = 1
(frozen) on them. Hooks the drain (0217BCA4) and DoEndOfTurnEffects_Async (021861E8) and prints each
call with the monster's `+0xA0`. The frozen skip holds if no drain is printed for a frozen seeded
monster while `+0xB8` stays 1. Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_play import Probe, wild, on_ptr, play

if len(sys.argv) < 3:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2]); R = pr.R
frames = int(sys.argv[3]) if len(sys.argv) > 3 else 1500
leader = pr.team[0]; li = pr.info(leader)
slot = [pr.u32(pr.gd + 0x11364 + 4 * i) for i in range(20)].index(leader)
mons = wild(pr)
for e in mons:
    i = pr.info(e)
    pr.w8(i + 0xB8, 1); pr.w8(i + 0xC1, 12); pr.w8(i + 0xC2, 0)
    pr.w8(i + 0xC0, slot); pr.w32(i + 0xBC, pr.u32(li + 0x8C))
    pr.w8(i + 0xA0, 1)


def frozen(ent):
    return pr.u8(pr.info(ent) + 0xA0) if on_ptr(ent) and ent in mons else "-"


pr.hook(0x0217BCA4, lambda: print("drain r0", hex(R.r0), "frozen", frozen(R.r0), flush=True))
pr.hook(0x021861E8, lambda: print("eot r0", hex(R.r0), "frozen", frozen(R.r0), flush=True))
play(pr, frames, lambda f: [(pr.u8(pr.info(e) + 0xB8), pr.u8(pr.info(e) + 0xC1)) for e in mons])
