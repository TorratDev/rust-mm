"""Play an input file in DeSmuME and print a sha256 of a RAM range at the given frames.

Hashes only the frames asked for (pydesmume_dump.py writes RAM for every frame). Each
HASH line is also written to OUTDIR/hashes.txt.
"""
import argparse, hashlib, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pydesmume_dump as h
from desmume.emulator import DeSmuME

p = argparse.ArgumentParser()
p.add_argument("rom"); p.add_argument("outdir")
p.add_argument("--inputs")
p.add_argument("--frames", type=int, required=True)
p.add_argument("--hash-at", type=int, action="append", default=[])
p.add_argument("--hash-addr", type=lambda s: int(s, 16), default=0x02000000)
p.add_argument("--hash-len", type=lambda s: int(s, 16), default=0x400000)
a = p.parse_args()
os.makedirs(a.outdir, exist_ok=True)
emu = DeSmuME(); emu.open(a.rom)
inputs = h.parse_inputs(a.inputs)
read = h.make_reader(emu, a.hash_addr, a.hash_len)
t = time.monotonic()
with open(os.path.join(a.outdir, "hashes.txt"), "w") as out:
    for f in range(a.frames):
        if f in inputs: emu.input.keypad_update(inputs[f])
        emu.cycle(with_joystick=False)
        if f in a.hash_at:
            line = f"HASH frame {f} {hashlib.sha256(read()).hexdigest()}"
            print(line, flush=True)
            out.write(line + "\n")
print(f"{a.frames} frames in {time.monotonic()-t:.1f}s", flush=True)
emu.destroy()
