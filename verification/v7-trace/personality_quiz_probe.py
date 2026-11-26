#!/usr/bin/env python3
"""#575 probe: personality quiz to roster hand-off on the supported Blue dump.

Golden trace. Hooks: 0206C590 (question draw; logs the tracker counters), 0206C394 (nature pick;
logs on entry and, via a one-shot at the return address, the tracker after), 02003510 (roster
writer: species, isHero; on return dumps every roster slot at the base read from *(*020036E0), and
also reads *020DDF84 to check that it holds the same base), 02001AF4 (move filter
arguments), 0201E17C (tracker to hand-off block copy), 0201E020 (hand-off reader).
Output is JSON Lines; the starter and partner are read from the tracker (+8, +0xA) and the
hand-off block 020DF920 (+4, +6) when 0201E020 is entered.
Usage: python personality_quiz_probe.py ROM OUT FRAMES INPUTS
ROM is a scratch-named copy of pmdb.nds and OUT a .jsonl file, both outside the repo.
INPUTS is verification/v7-trace/boot_to_dungeon.txt. Reaches the nature pick by about frame 3743
and the roster writes by about frame 10743 (run with FRAMES 10800).
"""
import json, os, sys
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs

TRKP, HAND, ROSTER_PTR, ROSTER_COUNT, ROSTER_ALT = 0x020E077C, 0x020DF920, 0x020036E0, 0x020036E4, 0x020DDF84
DRAW, NATURE, ROSTER, COPY, READ, MOVES = 0x0206C590, 0x0206C394, 0x02003510, 0x0201E17C, 0x0201E020, 0x02001AF4

def main():
    rom, out_path, frames, inp = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    emu = DeSmuME(); emu.open(rom)
    boot = parse_inputs(inp); st = {"f": 0}
    out = open(out_path, "w", encoding="utf-8", newline="\n")
    def rd(a, n): return bytes(emu.memory.read(a, a + n, 1, False))
    def u16(a): return int.from_bytes(rd(a, 2), "little")
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    def ev(name, **kw): out.write(json.dumps({"ev": name, "f": st["f"], **kw}, sort_keys=True) + "\n")
    def tp(): return int.from_bytes(rd(TRKP, 4), "little")  # 020E077C holds a pointer to the 200-byte tracker
    def trk():
        T = tp()
        return {"tracker": hex(T), "count": u16(T + 0x40), "q": u16(T + 0x44), "nature": u16(T + 0x48),
                "starter": u16(T + 8), "partner": u16(T + 0xA), "gender": u16(T + 0xB8)}
    def once_at_return(fn):
        # DeSmuME has no unregister call, so the return-address hook is armed per call and fires once
        lr = reg("r14") & ~1; fired = [False]
        def after(*_):
            if not fired[0]: fired[0] = True; fn()
        emu.memory.register_exec(lr, after)
    def on_draw(*_): ev("draw", lr=hex(reg("r14")), **trk())
    def on_nature(*_):
        ev("nature_in", totals=[int.from_bytes(rd(tp() + 0x4C + 4 * i, 4), "little") for i in range(13)], **trk())
        once_at_return(lambda: ev("nature_out", **trk()))
    def on_roster(*_):
        ev("roster", species=reg("r0"), is_hero=reg("r1"), name_ptr=hex(reg("r2")), lr=hex(reg("r14")))
        def dump():
            # 02003510 reads *(*0x020036E0) as the record base and 0x020036E4 as the slot count (no cap)
            base = int.from_bytes(rd(int.from_bytes(rd(ROSTER_PTR, 4), "little"), 4), "little")
            alt = int.from_bytes(rd(ROSTER_ALT, 4), "little")  # *020DDF84, checked against base
            n = int.from_bytes(rd(ROSTER_COUNT, 4), "little")  # all slots, no cap (413 expected)
            recs = [rd(base + 0x40 * i, 0x40).hex() for i in range(n)] if base else []
            live = [(i, int.from_bytes(bytes.fromhex(r)[6:8], "little"), bytes.fromhex(r)[2]) for i, r in enumerate(recs)
                    if int(r[:2], 16) & 1]  # flags bit 0 set: (slot, species, isHero)
            ev("roster_out", base=hex(base), alt_020DDF84=hex(alt), count=n, live=live, records=recs)  # raw hex: scratch only
        once_at_return(dump)
    def on_moves(*_): ev("moves", species=reg("r1"), level=reg("r2"), cap=reg("r3"))
    def on_copy(*_): ev("copy", lr=hex(reg("r14")), **trk())
    def on_read(*_): ev("read", lr=hex(reg("r14")), hand_starter=u16(HAND + 4), hand_partner=u16(HAND + 6), **trk())
    for ad, cb in [(DRAW, on_draw), (NATURE, on_nature), (ROSTER, on_roster), (MOVES, on_moves),
                   (COPY, on_copy), (READ, on_read)]:
        emu.memory.register_exec(ad, cb)
    for f in range(frames):
        st["f"] = f
        if f in boot: emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
    out.close(); emu.destroy()
if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
