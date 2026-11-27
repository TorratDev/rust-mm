#!/usr/bin/env python3
"""#359 U2: Enraged (`+0xAE` = 0xC) at runtime: the stage raise on hit and the end-of-turn expiry.

Usage: status_rage.py ROM OUT_DIR FRAMES AF

Puts `+0xAE` = 0xC and `+0xAF` = AF on every valid wild monster, then plays the scripted inputs
(`combat_inputs.txt`). It hooks the stage raise (`02189234`), the hit handler (`0217E890`) and
`DoEndOfTurnEffects_Async` (`021861E8`), and prints each event with the monsters' `+0xAE`,
`+0xAF`, the four offensive stages (`+0x1C`, stride 2) and the leader's HP.
The literal at `0217EEF0` gives the stat index passed to `02189234`; it is printed at start.
Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, WILD, HERE

import romarg; romarg.fill()
if len(sys.argv) < 5:
    sys.exit(__doc__)
rom, out, frames, af = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])

pr = Probe(rom, out); R = pr.R
inp = pr.h.parse_inputs(HERE + "/combat_inputs.txt")
p = pr.u32(0x0217EEF0)
print("stat pool 0217EEF0 ->", hex(p), "value", pr.u32(p))
mons = [pr.u32(pr.gd + WILD + 4 * i) for i in range(16)]
mons = [e for e in mons if e and pr.u32(e) == 1 and 0x02000000 <= pr.u32(e + 0x70) < 0x02400000]
idx = {e: k for k, e in enumerate(mons)}
idx[pr.team[0]] = "L"; idx[pr.team[1]] = "P"
for e in mons:
    i = pr.info(e)
    pr.w8(i + 0xAE, 0xC); pr.w8(i + 0xAF, af)


def st(e):
    i = pr.info(e)
    return (pr.u8(i + 0xAE), pr.u8(i + 0xAF), [pr.s16(i + 0x1C + 2 * k) for k in range(4)], pr.s16(i + 0xE))


fr = [0]; log = []
pr.hook(0x02189234, lambda: log.append(("raise", fr[0], idx.get(R.r0, hex(R.r0)), idx.get(R.r1, hex(R.r1)),
                                        R.r2, R.r3, hex(R.r14))))
pr.hook(0x0217E890, lambda: log.append(("hit890", fr[0], idx.get(R.r0, hex(R.r0)), idx.get(R.r1, hex(R.r1)))))
pr.hook(0x021861E8, lambda: log.append(("eot", fr[0], idx.get(R.r0, hex(R.r0)))))

last = {}
for f in range(frames):
    fr[0] = f
    if f in inp:
        pr.emu.input.keypad_update(inp[f])
    pr.emu.cycle(with_joystick=False)
    cur = {idx[e]: st(e) for e in mons}
    if cur != last:
        log.append(("chg", f, cur))
        last = cur
for entry in log:
    print(entry)
