#!/usr/bin/env python3
"""#359 P2: Bubble's speed-down setter (`021882D0`) at runtime: the cached stage `+0xE0`.

Usage: status_effects.py ROM OUT_DIR

For 0..3 up counters (`+0xE4` on, set to 9) on the team leader, and k = 0..2 reseeded samples per
count, it calls `021882D0(seeder, leader, 1, 0)` from the turn gate and prints `+0xE0` and the
speed counters before and after, plus the RNG rolls. With the leader's base stage 1, `+0xE0` after
the call should equal the number of up counters (base 1 + ups, minus 1 from the new down counter).
Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe

if len(sys.argv) < 3:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2])
L, P = pr.team
li = pr.info(L)


def speed(i):
    return (pr.u8(i + 0xE0), [pr.u8(i + 0xE4 + j) for j in range(5)], [pr.u8(i + 0xE9 + j) for j in range(5)])


ok_all = 0; n = 0
for ups in range(4):
    for k in range(3):
        pr.fresh(k)
        for j in range(ups):
            pr.w8(li + 0xE4 + j, 9)
        before = speed(li)
        ok, _ = pr.call(0x021882D0, [P, L, 1, 0])
        after = speed(li)
        n += 1
        ok_all += int(ok and after[0] == ups)
        print("ups", ups, "k", k, "finished", ok, "before", before, "after", after, "rolls", pr.rolls)
print("E0 == ups in", ok_all, "of", n, "samples")
