#!/usr/bin/env python3
"""#359 L1: status setter durations at runtime (24 reseeded samples each).

Usage: status_durations.py ROM OUT_DIR [setter ...]
Calls each setter wrapper on the first floor (partner to leader) and prints, per setter, the
bytes read back, the `CalculateStatusTurns` (pair, flag) seen, and the number of
`DungeonRandInt` draws. ROM is a scratch-named copy of pmdb.nds."""
import collections, json
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, parse_args

ap = parse_args(__doc__); ap.add_argument("which", nargs="*")
a = ap.parse_args()
pr = Probe(a.rom, a.out_dir)
L, P = pr.team; li = pr.info(L)
# name, function, args, watched info offsets
JOBS = [
    ("nightmare_0215E03C", 0x0215E03C, [P, L], [0x99, 0x9A]),
    ("hypnosis_0215E07C", 0x0215E07C, [P, L], [0x99, 0x9A]),
    ("paralysis_021885A0_flag1", 0x021885A0, [P, L, 1], [0x9B, 0x9C]),
    ("burn_0218A710", 0x0218A710, [P, L, 0, 0], [0x9B, 0x9C, 0x9D]),
    ("cringe_021887E0", 0x021887E0, [P, L, 0], [0xAC, 0xAD]),
    ("smokescreen_0215D5BC", 0x0215D5BC, [P, L], [0xC4, 0xC5]),
    ("focus_0218D5F0", 0x0218D5F0, [P, L], [0xC4, 0xC5]),
    ("rage_0215DCC8", 0x0215DCC8, [P, L, 0x02100000], [0xAE, 0xAF]),
    ("leech_02163204", 0x02163204, [P, L], [0xB8, 0xC1]),
    ("speeddown_021882D0", 0x021882D0, [P, L, 1, 0], [0xE9, 0xEA, 0xE0]),
]
for name, addr, args, watch in JOBS:
    if a.which and name not in a.which: continue
    res = collections.Counter(); draws = collections.Counter(); cst = None; fails = 0
    for k in range(24):
        pr.fresh(k)
        ok, _ = pr.call(addr, args)
        if not ok: fails += 1; continue
        res[tuple(pr.u8(li + o) for o in watch)] += 1
        draws[len(pr.rolls)] += 1
        cst = pr.cs_calls
    print(name, json.dumps(dict(watch=[hex(w) for w in watch], results=sorted(res.items()),
                                draws=dict(draws), calc_status_turns=cst, fails=fails)))
