#!/usr/bin/env python3
"""#359 flinch at runtime: the refusal at 02172ADC (`info+0xAC == 1`) and the clear (0218FA80).

Usage: status_flinch.py ROM OUT_DIR [FRAMES]

Sets `+0xAC` = 1, `+0xAD` = 2 on the team leader (the move gate 02172ADC is reached by the leader), then plays the scripted inputs. Logs each
entry to the move gate 02172ADC (r0 and its `+0xAC`), each clear 0218FA80, and each change of the
monsters' flinch bytes. Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_play import Probe, on_ptr, play

if len(sys.argv) < 3:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2]); R = pr.R
frames = int(sys.argv[3]) if len(sys.argv) > 3 else 1500
L = pr.team[0]; mons = [L]
pr.w8(pr.info(L) + 0xAC, 1); pr.w8(pr.info(L) + 0xAD, 2)


def ac(ent):
    return pr.u8(pr.info(ent) + 0xAC) if on_ptr(ent) else None


pr.hook(0x02172ADC, lambda: print("gate 02172ADC r0", hex(R.r0), "+0xAC", ac(R.r0), flush=True))
pr.hook(0x0218FA80, lambda: print("clear 0218FA80 r0", hex(R.r0), flush=True))
play(pr, frames, lambda f: [(ac(e), pr.u8(pr.info(e) + 0xAD)) for e in mons], inputs="combat_attack_inputs.txt")
