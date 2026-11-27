#!/usr/bin/env python3
"""#624: runtime checks of flinch, paralysis, Focus Energy and the speed stage (status.md).

Usage: status_runtime_checks.py ROM OUT_DIR FRAMES MODE
  MODE = flinch | para | speed | focus. Sets the status fields on every monster (and the leader for
  focus) at frame 0, then logs the hooks and field changes. Output goes to stdout; nothing is written
  to the repo. Run on a scratch-named copy of the ROM (see status_durations.py).
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, WILD, HERE
rom, out, frames, mode = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
pr = Probe(rom, out); R = pr.R
inp = pr.h.parse_inputs(HERE + "/combat_inputs.txt")
mons = [pr.u32(pr.gd + WILD + 4*i) for i in range(16)]
mons = [e for e in mons if e and pr.u32(e) == 1 and 0x02000000 <= pr.u32(e+0x70) < 0x02400000]
L, P = pr.team; li = pr.info(L)
idx = {e: k for k, e in enumerate(mons)}; idx[L] = "L"; idx[P] = "P"
n = lambda e: idx.get(e, hex(e))
print("mode", mode, "mons", len(mons), "E0 before", [pr.u32(pr.info(e)+0xE0) for e in mons], "L", pr.u32(li+0xE0))
for e in mons:
    i = pr.info(e)
    if mode == "flinch": pr.w8(i+0xAC, 1); pr.w8(i+0xAD, 2)
    if mode == "para": pr.w8(i+0x9B, 4); pr.w8(i+0x9C, 4)
    if mode == "speed": pr.w8(i+0xE4, 40); pr.w8(i+0xE9, 3)   # stage left equal: +1 -1
if mode == "focus": pr.w8(li+0xC4, 4); pr.w8(li+0xC5, 40)
FOC = pr.u32(0x0217C7C8); GP = pr.u32(0x0217C79C)
print("lit 0217C7C8", FOC, "gp ptr", hex(GP))
WATCH = [0xAC, 0xAD, 0x9B, 0x9C, 0xC4, 0xC5, 0xE4, 0xE9]
def st(e): i = pr.info(e); return tuple(pr.u8(i+o) for o in WATCH) + (pr.u32(i+0xE0), pr.s16(i+0xE))
fr = [0]; log = []; p6 = [None]
pr.hook(0x02172ADC, lambda: log.append(("useMove_2ADC", fr[0], n(R.r0), hex(R.r1), st(R.r0))))
pr.hook(0x0215B384, lambda: log.append(("exec_B384", fr[0], n(R.r0), hex(R.r14))))
pr.hook(0x0218FA80, lambda: log.append(("endCringe_FA80", fr[0], n(R.r1), st(R.r1), hex(R.r14))))
pr.hook(0x0217E890, lambda: log.append(("hit890", fr[0], n(R.r0), n(R.r1))))
pr.hook(0x02181E58, lambda: log.append(("calcSpeed", fr[0], n(R.r0), hex(R.r14))))
pr.hook(0x021861E8, lambda: log.append(("eot", fr[0], n(R.r0))))
def cd():
    p6[0] = pr.u32(R.r13 + 4); log.append(("calcDmg", fr[0], n(R.r0), n(R.r1), "C4", pr.u8(pr.info(R.r0)+0xC4)))
pr.hook(0x0217BD94, cd)
def ri():
    lr = R.r14
    if 0x0217BD94 <= lr < 0x0217C800:
        log.append(("rand_in_dmg", fr[0], R.r0, hex(lr), "flag168", pr.u8(pr.u32(GP)+0x168) if GP else None,
                    "crit+0xD", pr.u8(p6[0]+0xD) if p6[0] else None))
pr.hook(0x0219D058, ri)
last = {}
for f in range(frames):
    fr[0] = f
    if f in inp: pr.emu.input.keypad_update(inp[f])
    pr.emu.cycle(with_joystick=False)
    cur = {n(e): st(e) for e in mons + [L]}
    if cur != last: log.append(("chg", f, cur)); last = cur
for l in log: print(l)
