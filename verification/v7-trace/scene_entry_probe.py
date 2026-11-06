#!/usr/bin/env python3
"""#267 P1 probe: ground entry-script selection hooks on the supported Blue dump.

Hooks: scene runner 0x0206F7EC (r0 = scene id), FUN_0214D498 (r0 = index, r1),
FUN_0214D420 (bank, group, sector, flag), FUN_02147C48 (r0 = reference pointer; logs the
first 4 words of the reference struct: kind etc.). Output is JSON Lines.
Usage: python scene_entry_probe.py ROM OUT FRAMES INPUTS [--runner-sid ID]
ROM is a scratch-named copy of pmdb.nds and OUT a .jsonl file, both outside the repo;
INPUTS is verification/v7-trace/boot_to_dungeon.txt.
--runner-sid ID overwrites r0 (the scene id argument) at the FIRST scene-runner entry. This is a
forced input, not a natural transition; the event "forced_runner_sid" marks it. The runner's own
code then selects the index, so the FUN_0214D498 argument is the real selection for that id.
"""
import json, os, sys
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs

RUNNER, IDX, BANK, INST = 0x0206F7EC, 0x0214D498, 0x0214D420, 0x02147C48
TABLE, SCENE_ID = 0x020DA5D4, 0x02133CEC

def main():
    rom, out_path, frames, inp = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    force = int(sys.argv[6], 0) if len(sys.argv) > 6 and sys.argv[5] == "--runner-sid" else None
    done = [False]
    emu = DeSmuME(); emu.open(rom)
    boot = parse_inputs(inp)
    st = {"f": 0}
    out = open(out_path, "w", encoding="utf-8", newline="\n")
    def rd(addr, n): return bytes(emu.memory.read(addr, addr + n, 1, False))
    def w32(addr): return int.from_bytes(rd(addr, 4), "little")
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    def ev(name, **kw): out.write(json.dumps({"ev": name, "f": st["f"], **kw}, sort_keys=True) + "\n")
    def on_runner(*_):
        if force is not None and not done[0]:
            ev("forced_runner_sid", was=reg("r0"), now=force)
            emu.memory.register_arm9.r0 = force
            done[0] = True
        ev("runner", sid=reg("r0"), lr=hex(reg("r14")))
    def on_idx(*_):
        i = reg("r0"); s = i - (1 << 32) if i >= 1 << 31 else i
        e = {}
        if 0 <= s < 0x200: e = {"entry_ptr": hex(w32(TABLE + 12 * s)), "entry_w1": hex(w32(TABLE + 12 * s + 4)), "entry_w2": hex(w32(TABLE + 12 * s + 8))}
        ev("idx", index=s, r1=reg("r1"), lr=hex(reg("r14")), **e)
    def on_bank(*_): ev("bank", bank=reg("r0"), group=reg("r1"), sector=reg("r2"), flag=reg("r3"), lr=hex(reg("r14")))
    def on_inst(*_):
        p = reg("r0")
        ev("install", r0=hex(p), r1=hex(reg("r1")), lr=hex(reg("r14")), cur_script=hex(w32(0x0215D1A8)), words=[hex(w32(p + 4 * k)) for k in range(4)] if 0x02000000 <= p < 0x02400000 else None)
    for ad, cb in [(RUNNER, on_runner), (IDX, on_idx), (BANK, on_bank), (INST, on_inst)]:
        emu.memory.register_exec(ad, cb)
    for f in range(frames):
        st["f"] = f
        if f in boot: emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
    out.close(); emu.destroy()
if __name__ == "__main__": main()
