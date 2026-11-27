#!/usr/bin/env python3
"""#362 P5/P6 probe: scene variables and the scene runner FUN_0206F7EC.
Usage: python scene_probe.py ROM OUT FRAMES INPUTS [retail_trace dir]"""
import json, os, sys, collections
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, sys.argv[5] if len(sys.argv) > 5 else os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs
IDS = [0xE, 0xF, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x17, 0x19, 0x26, 0x33]
RUNNER, ENTER, SET, GET, SETI = 0x0206F7EC, 0x0214D498, 0x02020B48, 0x02020F74, 0x020208E8
def main():
    rom, outp, frames, inp = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    emu = DeSmuME(); emu.open(rom); boot = parse_inputs(inp)
    def rd(a, n): return bytes(emu.memory.read(a, a + n, 1, False))
    def w32(a): return int.from_bytes(rd(a, 4), "little")
    def s32(a): v = w32(a); return v - (1 << 32) if v >> 31 else v
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    ev = []; f = [0]; getters = collections.Counter()
    def cells(): return dict(state=s32(w32(0x0207035C)), scene=s32(w32(0x02070334)), w358=s32(w32(0x02070358)))
    def on_runner(*_): ev.append(dict(f=f[0], ev="runner", r0=reg("r0")))
    def on_enter(*_): ev.append(dict(f=f[0], ev="enter_0214D498", r0=reg("r0") if reg("r0") < 0x80000000 else -1, **cells()))
    def on_set(*_):
        if reg("r0") == 0 and reg("r1") in IDS:
            v = reg("r2"); ev.append(dict(f=f[0], ev="set", id=hex(reg("r1")), v=v - (1 << 32) if v >> 31 else v, lr=hex(reg("r14"))))
    def on_get(*_):
        if reg("r0") == 0 and reg("r1") in IDS: getters[f"{reg('r1'):#x}@{reg('r14'):#x}"] += 1
    for a, cb in ((RUNNER, on_runner), (ENTER, on_enter), (SET, on_set), (GET, on_get)): emu.memory.register_exec(a, cb)
    desc = None
    for f[0] in range(frames):
        if f[0] in boot: emu.input.keypad_update(boot[f[0]])
        emu.cycle(with_joystick=False)
        if desc is None and f[0] > 60:
            desc = {}
            for i in IDS:
                d = 0x02098060 + i * 0x10; t = rd(d, 16)
                np = int.from_bytes(t[12:16], "little")
                name = rd(np, 32).split(b"\0")[0].decode("ascii", "replace") if 0x02000000 <= np < 0x02400000 else "?"
                desc[hex(i)] = dict(type=int.from_bytes(t[0:2], "little"), off=int.from_bytes(t[4:6], "little"),
                                    count=int.from_bytes(t[8:10], "little"), name=name)
    with open(outp, "w", encoding="utf-8", newline="\n") as out:
        out.write(json.dumps({"desc": desc, "getters": getters}, sort_keys=True) + "\n")
        for e in ev: out.write(json.dumps(e) + "\n")
    emu.destroy()
if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
