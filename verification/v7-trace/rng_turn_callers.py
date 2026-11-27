#!/usr/bin/env python3
"""#351 U2: global-LCG steps per caller between two RunFractionalTurn_Async rows.

Boots to the first floor like retail_trace.py, plays its TURN_INPUTS, and for each
pair of trigger hits (021411BC, gDungeon != 0) writes one JSON line: frame delta,
sPRNGState step count (steps_to), steps counted from Rand32Bit hits (2 per call),
frame-routine hits (02018ACC) and the steps per outer caller lr. Rand32Bit lr inside
RandInt/RandRange is resolved to their own caller lr.
Usage: rng_turn_callers.py ROM OUT.jsonl [--frames N]
"""
import argparse, hashlib, json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
import retail_trace as rt
from desmume.emulator import DeSmuME

RAND32, RANDINT, RANDRANGE = 0x0201198C, 0x02011968, 0x02011924
SEED_A, SEED_B = 0x020118EC, 0x020119D8       # sPRNGState writers other than the step
FRAME_FN = 0x02018ACC
RANDINT_END, RANDRANGE_END = 0x0201198C, 0x02011968

def main():
    p = argparse.ArgumentParser(); p.add_argument("rom"); p.add_argument("out")
    p.add_argument("--frames", type=int, default=3000); a = p.parse_args()
    emu = DeSmuME(); emu.open(a.rom); ram = rt.Ram(emu)
    boot = rt.parse_inputs(os.path.join(os.path.dirname(rt.__file__), "boot_to_dungeon.txt"))
    for f in range(30000):
        if f in boot: emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(rt.G_DUNGEON) != 0: break
    else: sys.exit("no dungeon")
    turns = rt.parse_inputs(os.path.join(os.path.dirname(rt.__file__), "turn_inputs.txt"))
    st = {"ri": 0, "rr": 0, "callers": {}, "r32": 0, "frame_fn": 0, "seeds": 0}
    rows, frame, prev = [], [0], [None]
    lr = lambda: emu.memory.register_arm9.lr
    def on_rr(*_): st["rr"] = lr()
    def on_ri(*_):
        l = lr(); st["ri"] = st["rr"] if RANDRANGE <= l < RANDRANGE_END else l
    def on_r32(*_):
        if ram.u32(rt.G_DUNGEON) == 0: return
        l = lr(); c = st["ri"] if RANDINT <= l < RANDINT_END else l
        k = "%08X" % c; st["callers"][k] = st["callers"].get(k, 0) + 2; st["r32"] += 1
    def on_frame(*_):
        if ram.u32(rt.G_DUNGEON): st["frame_fn"] += 1
    def on_seed(*_):
        if ram.u32(rt.G_DUNGEON): st["seeds"] += 1
    def on_turn(*_):
        if ram.u32(rt.G_DUNGEON) == 0: return
        s, d = ram.u32(rt.S_PRNG), ram.u32(rt.G_DUNGEON_RNG)
        if prev[0] is not None:
            pf, ps, pd = prev[0]
            rows.append({"frame": frame[0], "dFrame": frame[0] - pf,
                         "lcgSteps": rt.steps_to(ps, s, 100000), "countedSteps": 2 * st["r32"],
                         "frameFnHits": st["frame_fn"], "reseeds": st["seeds"],
                         "dungeonSteps": rt.steps_to(pd, d, 100000),
                         "callers": dict(sorted(st["callers"].items()))})
        prev[0] = (frame[0], s, d)
        st.update(callers={}, r32=0, frame_fn=0, seeds=0)
    for addr, fn in ((RAND32, on_r32), (RANDINT, on_ri), (RANDRANGE, on_rr), (FRAME_FN, on_frame),
                     (SEED_A, on_seed), (SEED_B, on_seed), (rt.TRIGGER, on_turn)):
        emu.memory.register_exec(addr, fn)
    for f in range(a.frames):
        frame[0] = f
        if f in turns: emu.input.keypad_update(turns[f])
        emu.cycle(with_joystick=False)
    with open(a.out, "w", newline="\n") as o:
        for r in rows: o.write(json.dumps(r, sort_keys=True, separators=(",", ":")) + "\n")
    tot = {}
    for r in rows:
        for k, v in r["callers"].items(): tot[k] = tot.get(k, 0) + v
    summ = {"pairs": len(rows), "lcgEqCounted": sum(r["lcgSteps"] == r["countedSteps"] for r in rows),
            "sha256": hashlib.sha256(open(a.out, "rb").read()).hexdigest(), "totals": tot}
    print(json.dumps(summ), file=sys.stderr, flush=True)
    emu.destroy()

if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
