#!/usr/bin/env python3
"""#359 L3: does chain step 10 case 1 (`0218F3F8` -> `0219635C(1)`, the Kecleon shop check) draw
RNG? Calls `0218F3F8(0, leader)` with `+0xC7 = 1`, four reseeded samples per case, with hooks on the
dungeon and global RNG entries and on the shop helpers.

Usage: status_shop_check.py ROM OUT_DIR
Cases: the floor as it is (`gDungeon+0x684 == 0`, no shop), `+0x688` forced to 0 and 1, forced
shop states (debt, no debt, credit), and a control with `+0xC7 = 0`. A sample that waits on a
payment prompt does not finish within 900 frames and prints done False. The shopkeeper-adjacent
`021938C8` branch is not staged."""
import collections
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, parse_args, RNG

a = parse_args(__doc__).parse_args()
pr = Probe(a.rom, a.out_dir)
L, _ = pr.team; li = pr.info(L); GD = pr.gd
seen = []
for ad, nm in {0x0219D058: "DungeonRandInt", 0x0219D014: "DungeonRandRange", 0x02011968: "RandInt",
               0x0201198C: "Rand32Bit", 0x02196084: "02196084", 0x02195E18: "02195E18",
               0x02195C88: "02195C88", 0x021938C8: "021938C8", 0x0219635C: "0219635C",
               0x0217AF58: "0217AF58"}.items():
    pr.hook(ad, (lambda nm: lambda: seen.append(nm))(nm))

def mk(c7, f688=None, s684=None, s67c=None, s680=None):
    def f():
        pr.w8(li + 0xC7, c7); pr.w8(li + 0xC8, 1)
        if f688 is not None: pr.w8(GD + 0x688, f688)
        if s684 is not None: pr.w32(GD + 0x684, s684); pr.w32(GD + 0x67C, s67c); pr.w32(GD + 0x680, s680)
    return f

CASES = [("natural", mk(1)), ("+688=0", mk(1, 0)), ("+688=1", mk(1, 1)),
         ("control +C7=0", mk(0)), ("shop debt", mk(1, None, 100, 0, 0)),
         ("shop no debt", mk(1, None, 100, 100, 0)), ("shop credit", mk(1, None, 100, 100, 50)),
         ("control +C7=0 debt", mk(0, None, 100, 0, 0))]
for name, setup in CASES:
    for k in range(4):
        pr.fresh(k); seen.clear(); setup()
        r0 = pr.u32(RNG)
        ok, _ = pr.call(0x0218F3F8, [0, L])
        print(name, k, "done", ok, "rng_changed", pr.u32(RNG) != r0, dict(sorted(collections.Counter(seen).items())))
