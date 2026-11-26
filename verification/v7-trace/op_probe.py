#!/usr/bin/env python3
"""#362 P2 probe: GroundScript interpreter FUN_ov_0000__021421CC.
Hooks: loop top 0x02142304 (r6 = ctx+0x24), dispatch 0x02142344 (r2 = opcode),
FUN_02020F74 entry (r0 = var base, r1 = var id) while inside the interpreter.
Checks: PC advance 0x18, r2 == byte at fetched record, opcode <= 0xF6,
local-var base == ctx+0x74, frame waits (ctx+0x4E) count down once per frame.
Usage: python interp_probe.py ROM OUT FRAMES INPUTS [retail_trace dir]
"""
import json, os, sys, collections
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, sys.argv[5] if len(sys.argv) > 5 else os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs
TOP, DISP, GETVAR, ENTRY = 0x02142304, 0x02142344, 0x02020F74, 0x021421CC
def main():
    rom, outp, frames, inp = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    emu = DeSmuME(); emu.open(rom); boot = parse_inputs(inp)
    st = dict(f=0, ctx=None, pc=None, inside=0)
    c = collections.Counter(); ops = collections.Counter(); wait = {}; first = {}
    def w32(a): return int.from_bytes(bytes(emu.memory.read(a, a + 4, 1, False)), "little")
    def w16(a): return int.from_bytes(bytes(emu.memory.read(a, a + 2, 1, False)), "little")
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    def on_entry(*_):
        c["entries"] += 1; st["ctx"] = reg("r0"); st["inside"] = 1; st["pc"] = None
        # frame-wait bookkeeping: entry means step state 3 reached for this ctx
    def on_top(*_):
        ctx = reg("r6") - 0x24
        c["ctx_ok" if ctx == st["ctx"] else "ctx_bad"] += 1
        pc = w32(ctx + 0x28)
        if st["pc"] is not None:
            c["pc_step_0x18" if pc == st["pc"] + 0x18 else "pc_jump"] += 1
        st["pc"] = pc
    def on_disp(*_):
        ctx = st["ctx"]; op = reg("r2"); rec = w32(ctx + 0x40)
        c["op_le_f6" if op <= 0xF6 else "op_gt_f6"] += 1
        c["op_matches_rec" if op == (w32(rec) & 0xFF) and op == (w32(ctx + 0x44) & 0xFF) else "op_mismatch"] += 1
        c["rec_is_pc_minus_0x18" if rec == w32(ctx + 0x28) - 0x18 else "rec_other"] += 1
        ops[op] += 1; first.setdefault(op, st["f"])
    def on_getvar(*_):
        if not st["inside"] or st["ctx"] is None: return
        lr = reg("r14")
        if not (0x021421CC <= lr < 0x021461D0): return
        base, vid = reg("r0"), reg("r1")
        if base == 0: c["getvar_base0_id_lt_0x400" if vid < 0x400 else "getvar_base0_local"] += 1
        else: c["getvar_base_ctx74" if base == st["ctx"] + 0x74 else "getvar_base_other"] += 1
        if base and vid >= 0x400: c["getvar_local_id_ge_0x400"] += 1
    for a, cb in [(ENTRY, on_entry), (TOP, on_top), (DISP, on_disp), (GETVAR, on_getvar)]:
        emu.memory.register_exec(a, cb)
    out = open(outp, "w", encoding="utf-8", newline="\n")
    for f in range(frames):
        st["f"] = f; st["inside"] = 0
        if f in boot: emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        ctx = st["ctx"]
        if ctx:
            run, n, op = w16(ctx + 0x26), w16(ctx + 0x4E), w32(ctx + 0x44) & 0xFF
            prev = wait.get(ctx)
            if prev and run == 2 and prev[0] == 2 and prev[2] == op and prev[1] < 0x8000 and n < 0x8000 and prev[1] > 0:
                c["wait_dec_by_1" if n == prev[1] - 1 else "wait_dec_other"] += 1
            wait[ctx] = (run, n, op)
    out.write(json.dumps({"checks": c, "opcodes_seen": len(ops), "max_op": hex(max(ops)) if ops else None,
                          "dispatches": sum(ops.values()), "ops": {hex(k): [v, first[k]] for k, v in sorted(ops.items())}}, sort_keys=True) + "\n")
    out.close(); emu.destroy()
if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
