#!/usr/bin/env python3
"""IQ persistence at dungeon exit (#589): party+0x06 -> roster+0x0A, party+0x42..0x44 -> roster+0x18..0x1A.

Boots to the first floor (boot_to_dungeon.txt), writes distinct sentinels into the leader's party
record (u16 +0x06, bytes +0x42..0x44), then watches the exit chain 02178068 -> 02001E24 -> 02001D24.
At 02001E24 entry r0 = party idx, r1 = party record; at 02001D24 entry r0 = roster record
(*0x020DDF84 + idx*0x40), r1 = party record. The roster record is read after 02001D24 returns (hook on lr).
Party base: *0x020DDF84 + 0x6740 (stride 0x54); roster base: *0x020DDF84.

--force-call writes r0 = 0 and jumps to 02178068 at the first turn-loop trigger, because a natural
floor exit is not driven. The chain below 02178068 then runs for real.

Usage: iq_persist_probe.py ROM OUT.json [--force-call] [--frames N]
Output: pass/fail and the compared values on stdout; full detail in OUT.json.
Keep ROM and OUT outside the repository. Never commit a ROM or an output.
"""
import argparse, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME  # noqa: E402
from desmume.controls import Keys, keymask  # noqa: E402
from retail_trace import G_DUNGEON, TRIGGER, Ram, parse_inputs  # noqa: E402

PARTY_OFF, STRIDE, ROSTER_CELL = 0x6740, 0x54, 0x020DDF84
ENTRY, E24, COPY = 0x02178068, 0x02001E24, 0x02001D24
IQ, SK = 0x1234 & 0x3FF, (0x11, 0x22, 0x33)  # distinct sentinels (IQ fits the 10-bit save field)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom"); p.add_argument("out")
    p.add_argument("--force-call", action="store_true")
    p.add_argument("--frames", type=int, default=6000)
    a = p.parse_args()
    emu = DeSmuME(); emu.open(os.path.abspath(a.rom)); ram = Ram(emu); R = emu.memory.register_arm9
    boot = parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
    for f in range(30000):
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
            break
    else:
        sys.exit("no dungeon reached")
    emu.input.keypad_update(0)
    for _ in range(60):
        emu.cycle(with_joystick=False)
    party = ram.u32(ROSTER_CELL) + PARTY_OFF
    w = lambda ad, b: emu.memory.write(ad, ad + len(b), 1, bytes(b))
    w(party + 6, [IQ & 0xFF, IQ >> 8]); w(party + 0x42, SK)
    res = {"party": hex(party), "party_bit0": ram.u8(party) & 1, "roster_idx": ram.s16(party + 8),
           "sentinel_iq": IQ, "sentinel_sk": list(SK), "entry_hit": False, "e24_hits": [], "hits": []}
    state, last_e24 = {}, {}

    def on_e24(addr, size):
        last_e24.update(idx=int(R.r0), party=int(R.r1))
        res["e24_hits"].append({"r0_idx": last_e24["idx"], "r1_party": hex(last_e24["party"]),
                                "r1_is_leader": last_e24["party"] == party})

    def on_copy(addr, size):
        dst, src = int(R.r0), int(R.r1)
        # the 02001D24 hit counts only when preceded by 02001E24 on the same party record
        via_e24 = bool(last_e24) and last_e24["party"] == src
        last_e24.clear()
        state.update(dst=dst, src=src, lr=int(R.r14) & ~1)
        res["hits"].append({"dst": hex(dst), "src": hex(src), "src_is_leader": src == party, "via_e24": via_e24,
                            "src_iq": ram.s16(src + 6), "src_sk": list(ram.bytes(src + 0x42, 3))})
        emu.memory.register_exec(state["lr"], on_ret)

    def on_ret(addr, size):
        d = state["dst"]
        res["hits"][-1].update(dst_iq=ram.s16(d + 0x0A), dst_sk=list(ram.bytes(d + 0x18, 3)))

    def on_entry(addr, size):
        res["entry_hit"] = True

    emu.memory.register_exec(ENTRY, on_entry)
    emu.memory.register_exec(E24, on_e24)
    emu.memory.register_exec(COPY, on_copy)
    if a.force_call:
        def force(addr, size):
            res["trigger_fired"] = res.get("trigger_fired", 0) + 1
            if res["trigger_fired"] == 1:
                R.r0 = 0; emu.memory.set_next_instruction(ENTRY)  # shortcut: leaves the turn loop unreturned, probe only
        emu.memory.register_exec(TRIGGER, force)

    def press(mask):
        emu.input.keypad_update(mask)
        for _ in range(4): emu.cycle(with_joystick=False)
        emu.input.keypad_update(0)
        for _ in range(20): emu.cycle(with_joystick=False)

    for k in ["A", "A", "A", "LEFT", "RIGHT", "LEFT", "RIGHT"]:  # dismiss tutorial boxes, then take turns
        for _ in range(200): emu.cycle(with_joystick=False)
        press(keymask(getattr(Keys, "KEY_" + k)))
    for _ in range(a.frames):
        emu.cycle(with_joystick=False)

    ok = any(h["via_e24"] and h["src_is_leader"] and h.get("dst_iq") == IQ and h.get("dst_sk") == list(SK)
             for h in res["hits"])
    res["confirmed"] = ok
    with open(a.out, "w") as f:
        json.dump(res, f, indent=1)  # full detail stays in the scratch file
    compared = [{k: h.get(k) for k in ("via_e24", "src_iq", "dst_iq", "src_sk", "dst_sk")} for h in res["hits"]]
    print(json.dumps({"pass": ok, "forced_call": a.force_call, "e24_hits": len(res["e24_hits"]),
                      "entry_hit": res["entry_hit"], "compared": compared}), flush=True)
    emu.destroy()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
