#!/usr/bin/env python3
"""#645 probe: one recording of the ground script engine for #522, #524, #525, #526, #527.

Logs, one JSON line per event:
  op     every interpreter dispatch (ov_0000::02142340, r2 = opcode, ctx = r6 - 0x24):
         context, owner kind (ctx+8), record address and the six record words
  set / set0 / scen   variable writes (020208E8, 02020B48) and SetScenario (020206AC)
  runner / loader / bank   scene runner 0206F7EC, loader index 0214D498, bank load 0214D420
  talk / stepon / install  ground interaction: A-probe 02151B04, step-on 02153D00,
         script installer 02147C48
  map    GROUND_MAP (var 0x11, s16 at 0x0210E670) when it changes
  box    the player's box (lives block at *0x0215D1DC, life 0: +0x144..+0x150, facing +0x142)
         when it changes
Usage: ground_script_log.py ROM OUT FRAMES INPUTS [--save DSV] [--savestate-at F DST]
ROM is a scratch copy of the supported dump (throwaway name: DeSmuME backs up its .dsv).
Run with XDG_CONFIG_HOME set to a scratch directory.
"""
import argparse, json, os, sys
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs

DISPATCH = 0x02142340
LIVES_PP, MAP_VAR = 0x0215D1DC, 0x0210E670
HOOKS = {0x020208E8: "set", 0x02020B48: "set0", 0x020206AC: "scen", 0x0206F7EC: "runner",
         0x0214D498: "loader", 0x0214D420: "bank", 0x02151B04: "talk", 0x02153D00: "stepon",
         0x02147C48: "install"}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom"); p.add_argument("out"); p.add_argument("frames", type=int); p.add_argument("inputs")
    p.add_argument("--save", help="raw 256 KiB save to import before boot")
    p.add_argument("--savestate-at", nargs=2, metavar=("FRAME", "DST"))
    a = p.parse_args()
    emu = DeSmuME(); emu.open(os.path.abspath(a.rom))
    if a.save:
        assert emu.backup.import_file(os.path.abspath(a.save)), "save import failed"
    boot = parse_inputs(a.inputs)
    out = open(a.out, "w", encoding="utf-8", newline="\n")
    f = [0]

    def rd(addr, n): return bytes(emu.memory.read(addr, addr + n, 1, False))
    def u32(addr): return int.from_bytes(rd(addr, 4), "little")
    def s16(addr): return int.from_bytes(rd(addr, 2), "little", signed=True)
    def reg(n): return getattr(emu.memory.register_arm9, n) & 0xFFFFFFFF
    def sg(x): return x - (1 << 32) if x >= 1 << 31 else x
    def ev(name, **k): out.write(json.dumps({"f": f[0], "ev": name, **k}) + "\n")

    def on_dispatch(*_):
        ctx = reg("r6") - 0x24; rec = u32(ctx + 0x40)
        ev("op", op=reg("r2"), ctx=hex(ctx), kind=s16(ctx + 8), rec=hex(rec),
           w=[hex(u32(rec + 4 * i)) for i in range(6)])
    emu.memory.register_exec(DISPATCH, on_dispatch)

    def hook(addr, name):
        def cb(*_):
            r = [reg("r%d" % i) for i in range(4)]
            ev(name, r=[hex(x) for x in r], lr=hex(reg("r14")), **({"val": sg(r[3])} if name == "set" else
                                                                    {"val": sg(r[2])} if name == "set0" else {}))
        emu.memory.register_exec(addr, cb)
    for addr, name in HOOKS.items():
        hook(addr, name)

    last = {"map": None, "box": None}
    for fr in range(a.frames):
        f[0] = fr
        if fr in boot:
            emu.input.keypad_update(boot[fr])
        emu.cycle(with_joystick=False)
        m = s16(MAP_VAR)
        if m != last["map"]:
            ev("map", map=m); last["map"] = m
        blk = u32(LIVES_PP)  # the cell holds the lives block; life 0 is the player
        if 0x02000000 <= blk < 0x02400000 and s16(blk + 2) != -1:
            box = [sg(u32(blk + o)) for o in (0x144, 0x148, 0x14C, 0x150)] + [rd(blk + 0x142, 1)[0]]
            if box != last["box"]:
                ev("box", box=box); last["box"] = box
        if a.savestate_at and fr == int(a.savestate_at[0]):
            emu.savestate.save_file(a.savestate_at[1]); ev("savestate", path=os.path.basename(a.savestate_at[1]))
    out.close(); emu.destroy()


if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
