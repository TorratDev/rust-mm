#!/usr/bin/env python3
"""#359 L1: how many status ticks a sleep or nightmare counter lasts, and what happens at expiry.
Writes (`+0x99`, `+0x9A`) = (1,3), (3,3), (3,4), (3,3) into the first floor's monsters, then
replays `combat_inputs.txt` for N frames and logs every change of the four values, the damage
calls, the expiry `0218FE64`, and every `DungeonRandInt` with its caller.

Usage: status_nightmare_ticks.py ROM OUT_DIR [FRAMES]   (default 4000)
Read: a counter N expires on the Nth tick; the nightmare monsters lose 8 HP at expiry, the sleeper
none; no dungeon draw comes from the expiry itself."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, parse_args, WILD, HERE

ap = parse_args(__doc__); ap.add_argument("frames", nargs="?", type=int, default=4000)
a = ap.parse_args()
pr = Probe(a.rom, a.out_dir); R = pr.R
inp = pr.h.parse_inputs(HERE + "/combat_inputs.txt")
TAGS = [(1, 3), (3, 3), (3, 4), (3, 3)]
mons = [pr.u32(pr.gd + WILD + 4 * i) for i in range(16)]
mons = [e for e in mons if e and pr.u32(e) == 1 and 0x02000000 <= pr.u32(e + 0x70) < 0x02400000]
tag = {}
for k, e in enumerate(mons):
    tag[e] = TAGS[k % 4]; i = pr.info(e); pr.w8(i + 0x99, tag[e][0]); pr.w8(i + 0x9A, tag[e][1])
def st(e):
    i = pr.info(e); return (pr.u8(i + 0x99), pr.u8(i + 0x9A), pr.s16(i + 0xE))
log = []; fr = [0]
pr.hook(0x0217D5FC, lambda: log.append(("damage", fr[0], tag.get(R.r1), st(R.r1) if R.r1 in tag else None)))
pr.hook(0x0218F21C, lambda: log.append(("wake_check", fr[0], tag.get(R.r1))))
pr.hook(0x0218F1EC, lambda: log.append(("silent_wake", fr[0])))
pr.hook(0x0218FE64, lambda: log.append(("expiry", fr[0], R.r3 & 0xFF, hex(R.r14))))
pr.hook(0x0219D058, lambda: log.append(("DungeonRandInt", fr[0], R.r0, hex(R.r14))))
last = {}
for f in range(a.frames):
    fr[0] = f
    if f in inp: pr.emu.input.keypad_update(inp[f])
    pr.emu.cycle(with_joystick=False)
    if any(st(e) != last.get(e) for e in mons):
        log.append(("chg", f, [(tag[e], st(e)) for e in mons])); last.update({e: st(e) for e in mons})
print("valid monsters", len(mons))
for l in log: print(l)
