#!/usr/bin/env python3
"""#359 Speed lowered at runtime: the stage restore at expiry (Tick 0218F610, step 14).

Usage: status_speed_restore.py ROM OUT_DIR [FRAMES]

Writes the state the Bubble setter leaves (stage 0, counter 7; see status_effects.py), then plays the
scripted inputs. Logs each change of the leader's cached stage `+0xE0` and its down counters
`+0xE9` to `+0xED`. The restore is seen when the counters reach 0 and `+0xE0` returns to the base.
Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_play import Probe, play

if len(sys.argv) < 3:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2])
frames = int(sys.argv[3]) if len(sys.argv) > 3 else 1500
L = pr.team[0]
li = pr.info(L)
# State after the setter (status_effects.py: stage 0, first counter 7); written directly, because
# returning from pr.call and then cycling the game crashed the ARM9 (undefined instruction).
pr.w8(li + 0xE0, 0); pr.w8(li + 0xE9, 7)
print("start E0", pr.u8(li + 0xE0), "counters", [pr.u8(li + 0xE9 + j) for j in range(5)], flush=True)
play(pr, frames, lambda f: (pr.u8(li + 0xE0), [pr.u8(li + 0xE9 + j) for j in range(5)]))
