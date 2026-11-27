#!/usr/bin/env python3
"""#359 paralysis at runtime: the -1 in CalcSpeedStage (02181E58, `+0x9B == 4`) and the refusal (02172ADC).

Usage: status_paralysis.py ROM OUT_DIR [FRAMES]

Sets `+0x9B` = 4, `+0x9C` = 2 on the team leader, then plays the scripted inputs. Logs the
speed-stage return value (with the entity's `+0x9B`), each gate entry 02172ADC, and each change of the
monsters' paralysis byte. Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_play import Probe, on_ptr, play

if len(sys.argv) < 3:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2]); R = pr.R
frames = int(sys.argv[3]) if len(sys.argv) > 3 else 1500
L = pr.team[0]; mons = [L]
pr.w8(pr.info(L) + 0x9B, 4); pr.w8(pr.info(L) + 0x9C, 2)


def p9b(ent):
    return pr.u8(pr.info(ent) + 0x9B) if on_ptr(ent) else None


def speed_entry():
    ent, lr = R.r0, R.r14

    def exit_cb(*_):
        if R.r15 == lr + 8:
            print("speed_stage ret", R.r0 & 0xFF, "entity", hex(ent), "+0x9B", p9b(ent), flush=True)
    pr.mem.register_exec(lr, exit_cb)


pr.hook(0x02181E58, speed_entry)
pr.hook(0x02172ADC, lambda: print("gate 02172ADC r0", hex(R.r0), "+0x9B", p9b(R.r0), flush=True))
play(pr, frames, lambda f: [p9b(e) for e in mons], inputs="combat_attack_inputs.txt")
