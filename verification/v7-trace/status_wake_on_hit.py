#!/usr/bin/env python3
"""#359 L2: does a hit wake a sleeper? Calls the per-hit damage function `0217D5FC` on the
leader with its `+0x99`/`+0x9A` set, three reseeded samples per case.

Usage: status_wake_on_hit.py ROM OUT_DIR
The damage struct is synthetic (damage 5, 6, 2, type 0), so this checks the wake rules, not a
real attack's damage. Prints per case: status after, HP lost, dungeon draws, the wake functions
reached (`0218F21C` check, `0218F1EC` silent wake, `0218FE64` expiry) and RNG entries seen."""
import collections
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, parse_args

a = parse_args(__doc__).parse_args()
pr = Probe(a.rom, a.out_dir)
L, P = pr.team; li = pr.info(L)
STRUCT = 0x023E0000
events = []; rng_hits = []
for nm, ad in (("wake_check_0218F21C", 0x0218F21C), ("silent_wake_0218F1EC", 0x0218F1EC), ("expiry_0218FE64", 0x0218FE64)):
    pr.hook(ad, (lambda nm: lambda: events.append(nm))(nm))
for nm, ad in (("RandInt", 0x02011968), ("RandRange", 0x02011924), ("Rand32", 0x0201198C),
               ("DungeonRandInt", 0x0219D058), ("DungeonRandRange", 0x0219D014)):
    pr.hook(ad, (lambda nm: lambda: rng_hits.append(nm))(nm))
CASES = [("awake", 0, 0), ("sleep_finite", 1, 4), ("sleep_7F", 1, 0x7F), ("nightmare_finite", 3, 4),
         ("nightmare_7F", 3, 0x7F), ("napping_finite", 5, 4), ("napping_7F", 5, 0x7F)]
for name, st, turns in CASES:
    res = collections.Counter()
    for k in range(3):
        pr.fresh(k); events.clear(); rng_hits.clear()
        pr.w8(li + 0x99, st); pr.w8(li + 0x9A, turns)
        hp0 = pr.s16(li + 0xE)
        for j, w in enumerate([5, 6, 2, 0, 0]): pr.w32(STRUCT + 4 * j, w)
        ok, _ = pr.call(0x0217D5FC, [P, L, STRUCT, 0, 0, 0, 0])
        res[(ok, pr.u8(li + 0x99), pr.u8(li + 0x9A), hp0 - pr.s16(li + 0xE), len(pr.rolls),
             tuple(events), tuple(sorted(set(n for n in rng_hits if n.startswith("Dungeon")))))] += 1
    print(name, sorted(res.items()))
