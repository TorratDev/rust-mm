"""Shared helpers for the natural-play status probes of #624 (not run on its own)."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from status_probe import Probe, WILD, HERE  # noqa: F401 (re-exported)


def wild(pr):
    """Valid wild monsters (entity type 1 with a plausible info pointer)."""
    mons = [pr.u32(pr.gd + WILD + 4 * i) for i in range(16)]
    return [e for e in mons if e and pr.u32(e) == 1 and 0x02000000 <= pr.u32(e + 0x70) < 0x02400000]


def on_ptr(ent):
    return 0x02000000 <= ent < 0x02400000


def hook_exit(pr, addr, fn):
    """Call fn(return value) when the function at addr returns; set up on each entry."""
    R = pr.R

    def entry():
        lr = R.r14
        pr.mem.register_exec(lr, lambda *_: fn(R.r0) if R.r15 == lr + 8 else None)
    pr.hook(addr, entry)


def play(pr, frames, watch, inputs="combat_inputs.txt"):
    """Play the scripted inputs and print each change of watch(); nothing is written to the repo."""
    inp = pr.h.parse_inputs(HERE + "/" + inputs)
    last = None
    for f in range(frames):
        if f in inp:
            pr.emu.input.keypad_update(inp[f])
        pr.emu.cycle(with_joystick=False)
        cur = watch(f)
        if cur != last:
            print("f", f, cur, flush=True)
            last = cur
