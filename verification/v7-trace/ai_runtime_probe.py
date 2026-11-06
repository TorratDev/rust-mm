#!/usr/bin/env python3
"""#278 runtime probe (P1-P5). Usage: python ai_runtime_probe.py ROM OUT.json [--turn-frames N]
ROM is a scratch-named copy of pmdb.nds outside the repo; OUT.json is outside the repo too.
Hooks (ARM9 exec): 0x02076B28 (subs pc,lr,#4 ctx restore, target=r14-4), 0x0207D518 (blx r1 list walker),
0x02076328/0x02076340 (blx r2 in 02076280), 0x02185548 (str into candidate array), plus write watches on
0x02135394 (hook cell) and words 0x021B485C..+0x50, 0x021B3F44. P5: info+6 of team/wild at each turn trigger."""
import argparse, hashlib, json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs, Ram, G_DUNGEON, TRIGGER, TEAM, WILD

p = argparse.ArgumentParser(); p.add_argument("rom"); p.add_argument("out")
p.add_argument("--turn-frames", type=int, default=3000); p.add_argument("--max-boot", type=int, default=30000)
a = p.parse_args()
emu = DeSmuME(); emu.open(a.rom); ram = Ram(emu); R = emu.memory.register_arm9
frame = [0]; dungeon_frame = [None]
def inc(d, k): d[k] = d.get(k, 0) + 1
ctx_targets, walker, hook_blx, cand_store, writes, p5 = {}, {}, {}, {}, {}, {}
first = {}
def cb_ctx(addr, size):
    k = hex((int(R.r14) - 4) & 0xFFFFFFFF); inc(ctx_targets, k); first.setdefault("ctx" + k, [frame[0], dungeon_frame[0] is not None])
def cb_walk(addr, size):
    k = hex(int(R.r1)); inc(walker, k); first.setdefault("walk" + k, [frame[0], dungeon_frame[0] is not None])
def mk_blx(name):
    def f(addr, size): inc(hook_blx, name + ":" + hex(int(R.r2)))
    return f
def cb_cand(addr, size):
    inc(cand_store, "r4=%d r0=%s" % (int(R.r4), hex(int(R.r0))))
for ad, f in ((0x02076B28, cb_ctx), (0x0207D518, cb_walk), (0x02076328, mk_blx("hook02135394")),
              (0x02076340, mk_blx("cb021353AC")), (0x02185548, cb_cand)):
    emu.memory.register_exec(ad, f)
pend = set()
WATCH = [0x02135394] + [0x021B485C + 4 * i for i in range(21)] + [0x021B3F44]
def mkw(ad):
    def f(addr, size):
        inc(writes, "%08x pc=%08x dungeon=%s" % (ad, int(R.r15), dungeon_frame[0] is not None)); pend.add(ad)
    return f
for ad in WATCH: emu.memory.register_write(ad, mkw(ad), 4)
vals = []
def flush():
    for ad in sorted(pend): vals.append([frame[0], hex(ad), hex(ram.u32(ad))])
    pend.clear()
def turn_hook(*_):
    d = ram.u32(G_DUNGEON)
    if not d: return
    for grp, base, n in (("team", TEAM, 4), ("wild", WILD, 16)):
        for i in range(n):
            e = ram.u32(d + base + 4 * i)
            if e and ram.u32(e) != 0:
                info = ram.u32(e + 0x70); inc(p5, "%s info+6=%d" % (grp, ram.u8(info + 6)))
emu.memory.register_exec(TRIGGER, turn_hook)
boot = parse_inputs(HERE + "/boot_to_dungeon.txt"); turns = parse_inputs(HERE + "/turn_inputs.txt")
f = 0
while f < a.max_boot + a.turn_frames:
    frame[0] = f
    if dungeon_frame[0] is None:
        if f in boot: emu.input.keypad_update(boot[f])
    else:
        t = f - dungeon_frame[0]
        if t in turns: emu.input.keypad_update(turns[t])
        if t >= a.turn_frames: break
    emu.cycle(with_joystick=False); flush()
    if dungeon_frame[0] is None and f % 10 == 0 and ram.u32(G_DUNGEON) != 0: dungeon_frame[0] = f
    f += 1
def chain(head_lit):
    out = []; headp = ram.u32(head_lit)
    node = ram.u32(headp) if headp else 0
    n = 0
    while node and n < 32:
        out.append([hex(ram.u32(node)), hex(ram.u32(node + 4))]); node = ram.u32(node + 8); n += 1
    return {"literal": hex(head_lit), "head_cell": hex(headp), "nodes_fn_arg": out}
lists = {"pre_sleep_0207d920": chain(0x0207d920), "post_sleep_0207d940": chain(0x0207d940)}
lits = {hex(x): hex(ram.u32(x)) for x in (0x0207d920, 0x0207d940, 0x0201981c, 0x02019820, 0x02019824)}
final = {hex(ad): hex(ram.u32(ad)) for ad in WATCH}
out = {"dungeon_frame": dungeon_frame[0], "last_frame": f, "ctx_restore_targets": ctx_targets, "walker_callbacks": walker,
       "blx_02076280": hook_blx, "cand_store_02185548": cand_store, "writes": writes, "write_values_first60": vals[:60],
       "write_values_total": len(vals), "final_values": final, "first_seen": first, "sleep_lists": lists, "sleep_literals": lits, "p5_info6": p5}
s = json.dumps(out, sort_keys=True, indent=1)
open(a.out, "w").write(s); print(hashlib.sha1(s.encode()).hexdigest(), flush=True)
emu.destroy()
