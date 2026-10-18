"""Scratch driver: play inputs, save states, screenshots and RAM hashes at frames."""
import argparse, hashlib, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pydesmume_dump as h
from desmume.emulator import DeSmuME

p = argparse.ArgumentParser()
p.add_argument("rom"); p.add_argument("outdir")
p.add_argument("--inputs"); p.add_argument("--state-in"); p.add_argument("--state-out")
p.add_argument("--state-at", type=int, action="append", default=[])
p.add_argument("--frames", type=int, required=True)
p.add_argument("--shot-at", type=int, action="append", default=[])
p.add_argument("--hash-at", type=int, action="append", default=[])
p.add_argument("--hash-addr", type=lambda s: int(s, 16), default=0x02000000)
p.add_argument("--hash-len", type=lambda s: int(s, 16), default=0x400000)
a = p.parse_args()
os.makedirs(a.outdir, exist_ok=True)
emu = DeSmuME(); emu.open(a.rom)
if a.state_in: emu.savestate.load_file(a.state_in)
inputs = h.parse_inputs(a.inputs)
read = h.make_reader(emu, a.hash_addr, a.hash_len)
t = time.monotonic()
for f in range(a.frames):
    if f in inputs: emu.input.keypad_update(inputs[f])
    emu.cycle(with_joystick=False)
    if f in a.state_at:
        emu.savestate.save_file(os.path.join(a.outdir, f"state_{f}.dst"))
    if f in a.shot_at:
        emu.screenshot().save(os.path.join(a.outdir, f"shot_{f}.png"))
    if f in a.hash_at:
        print(f"HASH frame {f} {hashlib.sha256(read()).hexdigest()}", flush=True)
if a.state_out: emu.savestate.save_file(a.state_out)
print(f"{a.frames} frames in {time.monotonic()-t:.1f}s", flush=True)
emu.destroy()
