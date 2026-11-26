#!/usr/bin/env python3
"""#362 P4 probe: actors, camera and dialogue in the GroundScript engine (ov0).
Usage: python actor_probe.py ROM OUT FRAMES INPUTS [retail_trace dir]
Prints counts and booleans only (no pointers, no table values)."""
import json, os, sys, collections
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, sys.argv[5] if len(sys.argv) > 5 else os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs
DISP, STEP, LERP = 0x02142344, 0x021461D0, 0x02021660
CAM_POOLS = [0x02140234, 0x02140254, 0x02140274, 0x021402A0, 0x021402C0, 0x021402EC,
             0x0214030C, 0x02140338, 0x02140354, 0x0214037C, 0x021403B0, 0x02140420]
NAMED = {0x02155314: "lives_select", 0x02154220: "lives_add", 0x02156980: "object_select",
         0x021579AC: "effect_select", 0x02158118: "event_select", 0x0213C560: "msg_talk",
         0x0213D754: "msg_render", 0x0202DF1C: "dollar_expander", 0x02140218: "cam_get",
         0x0214033C: "cam_attach", 0x02140358: "cam_fix", 0x02140380: "cam_0x97", 0x02157CC8: "mark_lookup"}
def main():
    rom, outp, frames, inp = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    emu = DeSmuME(); emu.open(rom); boot = parse_inputs(inp)
    c = collections.Counter(); st = dict(ctx=None, cam=None, last=None)
    def w32(a): return int.from_bytes(bytes(emu.memory.read(a, a + 4, 1, False)), "little")
    def w16(a): return int.from_bytes(bytes(emu.memory.read(a, a + 2, 1, False)), "little")
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    def on_step(*_): st["ctx"] = reg("r0")
    def on_disp(*_):
        op = reg("r2"); ctx = st["ctx"]
        if 0x32 <= op <= 0x38 and ctx:
            rec = w32(ctx + 0x40); p = w32(rec + 0xC)
            b = bytes(emu.memory.read(p, p + 64, 1, False)); s = b.split(b"\0")[0]
            ok = len(s) > 0 and all(32 <= x < 127 or x in (10, 13) or x >= 0x80 for x in s)
            c[f"msg_op_{op:#x}_text_{'ok' if ok else 'bad'}"] += 1
            # the ov0 helper 021421A0 adds a per-language u16 offset from rec+0x10
            c["msg_rec_ptr_in_ov_or_ram"] += 0x02000000 <= p < 0x02400000
        if 0x71 <= op <= 0x88: c["move_family_dispatch"] += 1
        if op == 0x54: c["anim_set_dispatch"] += 1
    def on_lerp(*_):
        ctx = st["ctx"]; lr = reg("r14")
        if ctx and 0x021461D0 <= lr < 0x02147990:
            op = w32(ctx + 0x44) & 0xFF
            c["lerp_in_step_op_in_71_88" if 0x71 <= op <= 0x88 else f"lerp_in_step_other_{op:#x}"] += 1
    def named(name):
        def cb(*_):
            c[name] += 1
            if name == "cam_attach" and st["cam"] is None:
                pools = {w32(a) for a in CAM_POOLS}
                c["cam_pools_one_struct" if len(pools) == 1 else "cam_pools_differ"] += 1
                st["cam"] = w32(0x02140354)
            if name == "msg_talk": c[f"msg_talk_kind_{reg('r0') if reg('r0') < 4 else 'var'}"] += 1
        return cb
    emu.memory.register_exec(DISP, on_disp); emu.memory.register_exec(STEP, on_step)
    emu.memory.register_exec(LERP, on_lerp)
    for a, n in NAMED.items(): emu.memory.register_exec(a, named(n))
    modes = collections.Counter()
    for f in range(frames):
        if f in boot: emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        cam = st["cam"]
        if cam:
            m = w32(cam + 8); pos = (w32(cam + 0x3C), w32(cam + 0x40))
            modes[m] += 1
            if st["last"] is not None and pos != st["last"]: c["cam_pos_changed_frames"] += 1
            st["last"] = pos
    with open(outp, "w", encoding="utf-8", newline="\n") as out:
        out.write(json.dumps({"counts": c, "cam_mode_frames": {str(k): v for k, v in modes.items()}}, sort_keys=True) + "\n")
    emu.destroy()
if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
