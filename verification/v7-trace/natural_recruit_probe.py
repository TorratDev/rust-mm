#!/usr/bin/env python3
"""Natural (unforced) recruit probe (#569, docs/re/progression/p3-recruiting.md P3).

Starts from a savestate taken in a dungeon whose record byte +2 (recruitingEnabled, read by
02006400 from the dungeon id at gDungeon+0x63C) is nonzero, then plays the input file and logs
the recruit path as the game runs it. It forces nothing: no memory write, no gate, table or roll
override. Reads only.

Hooks (ARM9 addresses, retail EUR dump). Entry hooks log r0..r2; return values come from a
return hook on the link register taken at entry. Gate hooks only log inside 0217F9B8 (in_recruit).
  0217F9B8 recruit entry (r0 recruiter, r1 target), return = result
  02006400 gate 1, recruitingEnabled for the dungeon id, return = byte
  0217F7EC gate 4, species gate, return = result
  020817E0 gate 5, adjacency helper (called on dx and dy), return = |d|
  02141C08 gate 7, reach gate (r0 target, r1 recruiter), return = result
  021432E8 held-item search (r0 recruiter, r1 item id); logged for item id 0x2E, return = pointer
  0219D058 DungeonRandInt (r0 cap); logged only inside 0217F9B8 with cap 1000, return = roll
  020021B0 species recruit rate (r0 species), return = rate (per mille)
  0217F388 join (r0 recruiter, r1 target), return = join result
  021778F4 HandleFaint_Async (r0 target, r1 reason, r2 killer), logged inside the join
  02179990 spawn of the joined ally, return = result, logged inside the join
  02153E80 prompt, return = answer, logged inside the join

rate_total is rebuilt from its parts in the summary line: species rate (020021B0 return), the
held-item term (s16 at 021AA1EC, added when 021432E8 finds 0x2E) and the recruiter level term
(s16 at 021AA96C + 2*level). The recruiter level is info+9 with info = entity+0x70 (entity+0x70 is a
pointer to the info struct). The summary's success test is rate_total > roll.

Refuses to run when recruitingEnabled is 0 in the savestate's dungeon, because a natural recruit
cannot happen there. --allow-disabled runs it anyway as a negative control (gate 1 is expected to
fail there with no forced input).

Usage: natural_recruit_probe.py ROM STATE OUT.jsonl [--inputs FILE] [--frames N] [--allow-disabled]
ROM is a throwaway copy of pmdb.nds, STATE a savestate, OUT a JSON Lines file. All three stay
outside the repository, and XDG_CONFIG_HOME should point at a scratch directory (DeSmuME writes
<rom>.dsv next to the ROM name in ~/.config/desmume).
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME  # noqa: E402
from retail_trace import G_DUNGEON, Ram, parse_inputs  # noqa: E402

REC_TABLE = 0x0209281A      # dungeon record byte +2 is at 0x02092818 + id*16 + 2
REC_STRIDE = 0x10
ID_OFS = 0x63C              # gDungeon+0x63C is the dungeon id
FIXED_ROOM_OFS = 0x34B4     # gDungeon+0x34B4, s16
INFO_PTR_OFS = 0x70         # entity+0x70 is the info pointer
INFO_LEVEL_OFS = 0x09       # info+9 is the level
INFO_SPECIES_OFS = 0x02     # info+2 is the species
HELD_ITEM_ID = 0x2E
HELD_ITEM_TERM = 0x021AA1EC  # s16 added when the recruiter holds 0x2E
LEVEL_TABLE = 0x021AA96C    # s16 per recruiter level
ROLL_CAP = 1000

RECRUIT, GATE_ENABLED, GATE_SPECIES = 0x0217F9B8, 0x02006400, 0x0217F7EC
GATE_ADJ, GATE_REACH, HELD_ITEM = 0x020817E0, 0x02141C08, 0x021432E8
ROLL, SPECIES_RATE = 0x0219D058, 0x020021B0
JOIN, FAINT, SPAWN, PROMPT = 0x0217F388, 0x021778F4, 0x02179990, 0x02153E80


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("state")
    p.add_argument("out")
    p.add_argument("--inputs", default=os.path.join(HERE, "combat_attack_inputs.txt"))
    p.add_argument("--frames", type=int, default=3000)
    p.add_argument("--allow-disabled", action="store_true")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    R = emu.memory.register_arm9
    try:
        emu.savestate.load_file(os.path.abspath(a.state))
    except RuntimeError:
        sys.exit("could not load the savestate")
    for _ in range(3):
        emu.cycle(with_joystick=False)

    d = ram.u32(G_DUNGEON)
    if not d:
        sys.exit("no dungeon floor in this savestate")
    dungeon_id = ram.u8(d + ID_OFS)
    enabled = ram.u8(REC_TABLE + dungeon_id * REC_STRIDE)
    if not enabled and not a.allow_disabled:
        sys.exit("recruitingEnabled is 0 in dungeon id %d: no natural recruit can happen here "
                 "(use --allow-disabled for a negative control)" % dungeon_id)

    events = []
    st = {"frame": 0, "in_recruit": False, "in_join": False, "rate": None, "level": None,
          "level_term": None, "item_held": None, "roll": None}
    pending = {}  # return address -> list of (name, on_return); one callback per address

    def dungeon_on():
        return ram.u32(G_DUNGEON) != 0

    def log(name, **kw):
        if dungeon_on():
            events.append(dict(hook=name, frame=st["frame"], **kw))

    def make_ret(lr):
        def cb(addr, size):
            q = pending.get(lr)
            if not q:
                return
            name, on_ret = q.pop()
            r = int(R.r0)
            log(name + ":ret", r0=r)
            if on_ret:
                on_ret(r)
        return cb

    def want_return(lr, name, on_ret=None):
        if lr not in pending:
            pending[lr] = []
            emu.memory.register_exec(lr, make_ret(lr))
        pending[lr].append((name, on_ret))

    def hook(addr, name, in_recruit=False, filt=None, on_ret=None, in_join=False):
        def cb(a_, s_):
            if not dungeon_on():
                return
            if in_recruit and not st["in_recruit"]:
                return
            if in_join and not st["in_join"]:
                return
            if filt and not filt():
                return
            lr = int(R.r14) & ~1
            log(name, r0=int(R.r0), r1=int(R.r1), r2=int(R.r2), lr=hex(lr))
            want_return(lr, name, on_ret)
        emu.memory.register_exec(addr, cb)

    def recruit_entry(addr, size):
        if not dungeon_on() or st["in_recruit"]:
            return
        rec, tgt = int(R.r0), int(R.r1)
        info_r, info_t = ram.u32(rec + INFO_PTR_OFS), ram.u32(tgt + INFO_PTR_OFS)
        lvl = ram.u8(info_r + INFO_LEVEL_OFS)
        st.update(in_recruit=True, rate=None, item_held=None, roll=None, level=lvl,
                  level_term=ram.s16(LEVEL_TABLE + 2 * lvl))
        log("%08X" % RECRUIT, r0=rec, r1=tgt, species=ram.s16(info_t + INFO_SPECIES_OFS),
            recruiter_level=lvl, fixed_room=ram.s16(d + FIXED_ROOM_OFS),
            x64D=ram.u8(d + 0x64D), x64E=ram.u8(d + 0x64E), x649=ram.u8(d + 0x649),
            lr=hex(int(R.r14) & ~1))

        def on_ret(r):
            rate, held = st["rate"], st["item_held"]
            item_term = ram.s16(HELD_ITEM_TERM) if held else 0
            total = (rate or 0) + item_term + (st["level_term"] or 0)
            log("RECRUIT:summary", result=r, species_rate=rate, item_held=held,
                item_term=item_term, level_term=st["level_term"], rate_total=total,
                roll=st["roll"], roll_passed=(st["roll"] is not None and total > st["roll"]))
            st.update(in_recruit=False)
        want_return(int(R.r14) & ~1, "%08X" % RECRUIT, on_ret)

    def roll_entry(addr, size):
        if st["in_recruit"] and int(R.r0) == ROLL_CAP:
            want_return(int(R.r14) & ~1, "%08X" % ROLL,
                        lambda r: st.update(roll=r))
            log("%08X" % ROLL, cap=int(R.r0))

    def item_filt():
        return int(R.r1) == HELD_ITEM_ID

    def item_ret(r):
        st["item_held"] = bool(r)

    def rate_ret(r):
        st["rate"] = r

    def join_entry(addr, size):
        if not dungeon_on():
            return
        st["in_join"] = True
        lr = int(R.r14) & ~1
        log("%08X" % JOIN, r0=int(R.r0), r1=int(R.r1), lr=hex(lr))
        want_return(lr, "%08X" % JOIN, lambda r: st.update(in_join=False))

    emu.memory.register_exec(RECRUIT, recruit_entry)
    emu.memory.register_exec(JOIN, join_entry)
    emu.memory.register_exec(ROLL, roll_entry)
    hook(GATE_ENABLED, "%08X" % GATE_ENABLED, in_recruit=True)
    hook(GATE_SPECIES, "%08X" % GATE_SPECIES, in_recruit=True)
    hook(GATE_ADJ, "%08X" % GATE_ADJ, in_recruit=True)
    hook(GATE_REACH, "%08X" % GATE_REACH, in_recruit=True)
    hook(HELD_ITEM, "%08X" % HELD_ITEM, in_recruit=True, filt=item_filt, on_ret=item_ret)
    hook(SPECIES_RATE, "%08X" % SPECIES_RATE, in_recruit=True, on_ret=rate_ret)
    hook(FAINT, "%08X" % FAINT, in_join=True)
    hook(SPAWN, "%08X" % SPAWN, in_join=True)
    hook(PROMPT, "%08X" % PROMPT, in_join=True)

    turns = parse_inputs(a.inputs) if a.inputs else {}
    for k in range(a.frames):
        st["frame"] = k
        if k in turns:
            emu.input.keypad_update(turns[k])
        emu.cycle(with_joystick=False)
    emu.input.keypad_update(0)

    header = {"dungeon_id": dungeon_id, "recruiting_enabled_nonzero": bool(enabled),
              "frames": a.frames, "allow_disabled": a.allow_disabled}
    with open(a.out, "w") as f:
        f.write(json.dumps({"header": header}) + "\n")
        for e in events:
            f.write(json.dumps(e) + "\n")
    counts = {}
    for e in events:
        counts[e["hook"]] = counts.get(e["hook"], 0) + 1
    print(json.dumps({"header": header, "event_counts": counts}, indent=1), flush=True)
    emu.destroy()


if __name__ == "__main__":
    main()
