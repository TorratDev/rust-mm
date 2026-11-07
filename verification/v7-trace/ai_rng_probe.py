#!/usr/bin/env python3
"""#193 P1/P3 runtime probe: per-RunMonsterAI RNG counts and weighted-pick check (Blue, EUR).
Usage: ai_rng_probe.py ROM OUT.json [--force] [--turn-frames 4700]
ROM = scratch-named copy of the supported dump, outside the repo; OUT outside the repo.
Observe mode: hooks RunMonsterAI entry (02188030) and credits each RNG entry hit to the active call.
--force: at 0x02193260 (r0 = DungeonRandInt(total) result, 'mov r6,r0') replaces the roll with a scheduled
boundary value and records the index chosen (r4 at 0x021932D0) against both rules. Weights: 5 entries of 8 bytes
at sp+0x24 (byte 0 = flag, word +4 = weight). Run with XDG_CONFIG_HOME in a scratch folder."""
import argparse, hashlib, json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
REPO_V7 = HERE
sys.path.insert(0, REPO_V7)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs, Ram, G_DUNGEON, G_DUNGEON_RNG, S_PRNG, steps_to

p = argparse.ArgumentParser(); p.add_argument("rom"); p.add_argument("out")
p.add_argument("--force", action="store_true"); p.add_argument("--turn-frames", type=int, default=4700)
p.add_argument("--max-boot", type=int, default=30000)
a = p.parse_args()
emu = DeSmuME(); emu.open(a.rom); ram = Ram(emu); R = emu.memory.register_arm9

RNG = {0x0219D058: "DungeonRandInt", 0x0219D07C: "DungeonRand16Bit", 0x0219D014: "DungeonRandRange",
       0x0219CFCC: "DungeonRandOutcome", 0x0219CFF0: "DungeonRandOutcome_2", 0x0201198C: "Rand32Bit"}
frame = [0]; active = [None]; calls = []; closers = set(); cap_log = []; picks = []; fcount = [0]

def close(*_):
    c = active[0]
    if c is None or int(R.r13) < c["sp"]: return
    c["lcg_end"] = ram.u32(G_DUNGEON_RNG); c["glcg_end"] = ram.u32(S_PRNG)
    c["act_end"] = ram.u32(ram.u32(c["ent"] + 0x70) + 0x42) if False else None
    calls.append(c); active[0] = None
enters = [0]
def on_enter(addr, size):
    enters[0] += 1
    ent = int(R.r0); lr = int(R.r14) & ~1
    info = ram.u32(ent + 0x70)
    active[0] = {"sp": int(R.r13), "frame": frame[0], "beh": ram.u8(info + 0x98), "p6": ram.u8(info + 6),
                 "act_start": ram.u32(info + 0x42) & 0xFFFF, "lcg_start": ram.u32(G_DUNGEON_RNG),
                 "glcg_start": ram.u32(S_PRNG), "hits": {}, "caps": [], "ent": ent}
    if lr not in closers: closers.add(lr); emu.memory.register_exec(lr, close)
def mk(name, ad):
    def f(addr, size):
        c = active[0]
        if c is None: return
        c["hits"][name] = c["hits"].get(name, 0) + 1
        if name == "DungeonRandInt": c["caps"].append(int(R.r0))
    return f
emu.memory.register_exec(0x02188030, on_enter)
for ad, n in RNG.items(): emu.memory.register_exec(ad, mk(n, ad))

def entries():
    sp = int(R.r13)
    return [(ram.u8(sp + 0x24 + 8 * i), ram.u32(sp + 0x24 + 8 * i + 4)) for i in range(5)]
def cb_roll(addr, size):
    if active[0] is None: return
    es = entries(); cum = []; t = 0
    for fl, w in es:
        if fl and w: t += w
        cum.append(t if (fl and w) else None)
    tot = int(R.r5)  # total passed to DungeonRandInt (r5 at 02193258)
    cands = sorted({min(max(x, 0), tot - 1) for c in cum if c is not None for x in (c - 1, c, c + 1)} | {0, tot - 1})
    roll = cands[fcount[0] % len(cands)]; fcount[0] += 1
    R.r0 = roll
    picks.append({"entries": es, "total": tot, "roll": roll, "cum": cum})
def cb_pick(addr, size):
    if picks and "got" not in picks[-1]: picks[-1]["got"] = int(R.r4)
if a.force:
    emu.memory.register_exec(0x02193260, cb_roll); emu.memory.register_exec(0x021932D0, cb_pick)

boot = parse_inputs(REPO_V7 + "/boot_to_dungeon.txt"); turns = parse_inputs(REPO_V7 + "/turn_inputs.txt")
dfr = None; f = 0
while f < a.max_boot + a.turn_frames:
    frame[0] = f
    if dfr is None:
        if f in boot: emu.input.keypad_update(boot[f])
    else:
        t = f - dfr
        if t in turns: emu.input.keypad_update(turns[t])
        if t >= a.turn_frames: break
    emu.cycle(with_joystick=False)
    if dfr is None and f % 10 == 0 and ram.u32(G_DUNGEON) != 0: dfr = f
    f += 1

def summarize():
    s = {"calls": len(calls), "beh": {}, "p6": {}, "draws_per_call": {}, "lcg_steps_ok": 0, "lcg_steps_bad": 0,
         "global_lcg_changed": 0, "other_hits": {}, "first_cap_is_total": None, "extra_caps": {}, "act_changes": {}}
    for c in calls:
        s["beh"][c["beh"]] = s["beh"].get(c["beh"], 0) + 1; s["p6"][c["p6"]] = s["p6"].get(c["p6"], 0) + 1
        n = c["hits"].get("DungeonRandInt", 0); s["draws_per_call"][n] = s["draws_per_call"].get(n, 0) + 1
        st = steps_to(c["lcg_start"], c["lcg_end"], 100)
        if st == c["hits"].get("DungeonRand16Bit", 0): s["lcg_steps_ok"] += 1
        else: s["lcg_steps_bad"] += 1
        if c["glcg_start"] != c["glcg_end"]: s["global_lcg_changed"] += 1
        for k, v in c["hits"].items():
            if k not in ("DungeonRandInt", "DungeonRand16Bit"): s["other_hits"][k] = s["other_hits"].get(k, 0) + v
        if c["hits"].get("DungeonRand16Bit", 0) != n: s["other_hits"]["16bit!=int"] = s["other_hits"].get("16bit!=int", 0) + 1
        key = ",".join(map(str, c["caps"][1:])); 
        if key: s["extra_caps"][key] = s["extra_caps"].get(key, 0) + 1
    # strip nondeterminism-free fields only; everything above is deterministic
    return s
out = {"dungeon_frame": dfr, "last_frame": f, "force": a.force, "enters": enters[0], "summary": summarize()}
if a.force:
    ok_new = ok_old = n = 0; bad = []
    for pk in picks:
        if "got" not in pk: continue
        n += 1
        exp_new = next((i for i, c in enumerate(pk["cum"]) if c is not None and c >= pk["roll"]), None)
        exp_old = next((i for i, c in enumerate(pk["cum"]) if c is not None and pk["roll"] < c), None)
        ok_new += pk["got"] == exp_new; ok_old += pk["got"] == exp_old
        if pk["got"] != exp_new: bad.append(pk)
    layouts = sorted({tuple(tuple(e) for e in pk["entries"]) for pk in picks})
    out["force_summary"] = {"picks": n, "match_ge_rule": ok_new, "match_lt_rule": ok_old, "layouts": len(layouts),
                            "mismatches_first5": bad[:5], "layout_list": [list(map(list, l)) for l in layouts]}
s = json.dumps(out, sort_keys=True, indent=1)
open(a.out, "w").write(s); print(hashlib.sha1(s.encode()).hexdigest(), flush=True)
emu.destroy()
