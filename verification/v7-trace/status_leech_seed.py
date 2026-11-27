#!/usr/bin/env python3
"""#359 U1: Leech Seed per-turn drain at runtime (natural play, no direct call).

Usage: status_leech_seed.py ROM OUT_DIR [FRAMES] [ooze]

Writes the seed fields that the setter writes (`+0xB8` = 1, `+0xC1`, `+0xC2` = 0, `+0xC0` = leader
slot, `+0xBC` = leader `info+0x8C`) into the valid wild monsters, sets the leader's HP to 3, then
plays the scripted inputs. It hooks the drain (`0217BCA4`), the seeder heal (`0218DF38`), the
countdown end (`0218F610`), `DungeonRandInt` (`0219D058`) and `DoEndOfTurnEffects_Async`
(`021861E8`), and prints each event with the monster's seed bytes. `ooze` forces the target's
ability byte to 0x3A, which takes the seeder branch instead of the heal.

A direct call to `021861E8` does not return under the probe (the function is async), so the
drain is seen only in natural play. Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, WILD, HERE

import romarg; romarg.fill()
if len(sys.argv) < 3:
    sys.exit(__doc__)
rom, out = sys.argv[1], sys.argv[2]
frames = int(sys.argv[3]) if len(sys.argv) > 3 else 3000
ooze = len(sys.argv) > 4 and sys.argv[4] == "ooze"

pr = Probe(rom, out); R = pr.R
inp = pr.h.parse_inputs(HERE + "/combat_inputs.txt")
leader = pr.team[0]; li = pr.info(leader)
slot = [pr.u32(pr.gd + 0x11364 + 4 * i) for i in range(20)].index(leader)
pr.w8(li + 0x0E, 3); pr.w8(li + 0x0F, 0)                     # leader HP = 3
mons = [pr.u32(pr.gd + WILD + 4 * i) for i in range(16)]
mons = [e for e in mons if e and pr.u32(e) == 1 and 0x02000000 <= pr.u32(e + 0x70) < 0x02400000]
for e in mons:
    i = pr.info(e)
    pr.w8(i + 0xB8, 1); pr.w8(i + 0xC1, 12); pr.w8(i + 0xC2, 0)
    pr.w8(i + 0xC0, slot); pr.w32(i + 0xBC, pr.u32(li + 0x8C))
    if ooze:
        pr.w8(i + 0x58, 0x3A)
print("leader slot", slot, "monsters", len(mons))

fr = [0]; log = []
idx = {e: k for k, e in enumerate(mons)}


def st(e):
    i = pr.info(e)
    return (pr.u8(i + 0xB8), pr.u8(i + 0xC1), pr.u8(i + 0xC2), pr.s16(i + 0xE))


pr.hook(0x021861E8, lambda: log.append(("eot", fr[0], idx.get(R.r0, hex(R.r0)),
                                        st(R.r0) if R.r0 in idx else None, "leaderHP", pr.s16(li + 0xE))))
pr.hook(0x0217BCA4, lambda: log.append(("dmg", fr[0], idx.get(R.r0, hex(R.r0)), R.r1, R.r2, hex(R.r14))))
pr.hook(0x0218DF38, lambda: log.append(("heal", fr[0], hex(R.r1), R.r2, R.r3, hex(R.r14))))
pr.hook(0x0218F610, lambda: log.append(("end_F610", fr[0], idx.get(R.r0, hex(R.r0)), hex(R.r14))))
pr.hook(0x0219D058, lambda: log.append(("rand", fr[0], R.r0, hex(R.r14))))

last = {}
for f in range(frames):
    fr[0] = f
    if f in inp:
        pr.emu.input.keypad_update(inp[f])
    pr.emu.cycle(with_joystick=False)
    cur = {e: st(e) for e in mons}
    cur["L"] = pr.s16(li + 0xE)
    if cur != last:
        log.append(("chg", f, [cur[e] for e in mons], "L", cur["L"]))
        last = cur
for entry in log:
    print(entry)
