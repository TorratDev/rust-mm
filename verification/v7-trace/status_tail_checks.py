#!/usr/bin/env python3
"""#624 remaining checks: the message IDs, the flinch refusal's return value, and the Enraged message.

Usage: status_tail_checks.py ROM OUT_DIR MODE [FRAMES]

MODE "ids":    print the u32 at 0x021871D8 and 0x021871DC from the savestate (no play).
MODE "flinch": set `+0xAC` = 1 on the leader, play the attack inputs, and print the return value of the
               move gate 02172ADC at each entry with the leader's `+0xAC`.
MODE "flinch0": the same run with the flag left clear (control).
MODE "rage":   set `+0xAE` = 0xC and `+0xAF` = 3 on the leader, play the attack inputs, and print each call
               to the message printer 02154BF0 (r0..r3) with the leader's `+0xAE`.
Output is RAM values; nothing is written to the repo.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_play import Probe, hook_exit, play

if len(sys.argv) < 4:
    sys.exit(__doc__)
pr = Probe(sys.argv[1], sys.argv[2]); R = pr.R
mode = sys.argv[3]
frames = int(sys.argv[4]) if len(sys.argv) > 4 else 2500
L = pr.team[0]; li = pr.info(L)

if mode == "ids":
    print("DAT_021871D8 =", hex(pr.u32(0x021871D8)), " DAT_021871DC =", hex(pr.u32(0x021871DC)))
    sys.exit(0)

if mode in ("flinch", "flinch0"):
    if mode == "flinch":
        pr.w8(li + 0xAC, 1); pr.w8(li + 0xAD, 2)   # flinch0: control, flag left clear

    def gate():
        ac = pr.u8(pr.info(R.r0) + 0xAC) if R.r0 == L else None
        hook_exit(pr, 0x02172ADC, lambda ret: print("gate 02172ADC", "leader" if R.r0 == L else hex(R.r0), "+0xAC", ac, "ret", ret & 0xFF, flush=True))

    pr.hook(0x02172ADC, gate)
    play(pr, frames, lambda f: pr.u8(li + 0xAC), inputs="combat_attack_inputs.txt")

if mode == "rage":
    pr.w8(li + 0xAE, 0xC); pr.w8(li + 0xAF, 3)
    pr.hook(0x02154BF0, lambda: print("message 02154BF0 r0", hex(R.r0), "r1", hex(R.r1), "r2", hex(R.r2), "r3", hex(R.r3),
                                      "+0xAE", pr.u8(li + 0xAE), flush=True))
    play(pr, frames, lambda f: (pr.u8(li + 0xAE), pr.u8(li + 0xAF)), inputs="combat_attack_inputs.txt")
