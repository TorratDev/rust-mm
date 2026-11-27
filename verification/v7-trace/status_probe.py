"""Shared helpers for the status probes of #359 (status_*.py). Not run on its own.

Loads a scratch-named copy of the retail ROM, builds (once) a savestate on the first dungeon
floor, and calls a game function from the turn gate with set registers and stack arguments.
Probe-only patches (message printers return at once) are written to emulator RAM, never to files.
The ROM and the savestate stay outside the repo; the probes print values read from RAM only."""
import os, sys, hashlib, json

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy"); os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

G_DUNGEON = 0x020E0788          # pointer to gDungeon
TEAM, WILD = 0x11314, 0x11324   # gDungeon offsets: 4 team slots, 16 wild slots
RNG = 0x021B3F58                # gDungeonRngState
GATE = 0x02142AE4               # natural turn-gate call site, used to host the calls
SENT = 0x0218F168               # return sentinel for a called function
MSG_PRINTERS = (0x02154B8C, 0x02154BF0)
BX_LR = 0xE12FFF1E


def setup_env(out_dir):
    """Keep DeSmuME's card backup (.dsv) out of ~/.config/desmume."""
    os.makedirs(out_dir, exist_ok=True)
    os.environ["XDG_CONFIG_HOME"] = os.path.join(out_dir, "xdg")


def sha256_text(s):
    return hashlib.sha256(s.encode()).hexdigest()


class Probe:
    def __init__(self, rom, out_dir):
        setup_env(out_dir)
        from desmume.emulator import DeSmuME
        import pydesmume_dump as h
        self.h = h
        self.emu = DeSmuME(); self.emu.open(rom)
        self.mem = self.emu.memory; self.R = self.mem.register_arm9
        # The shared library state (#643) when it exists; else build one in out_dir.
        lib = os.environ.get("RESCUE_STATE", os.path.expanduser("~/rescue-re/saves/states/dungeon1-f1.dst"))
        self.state_path = lib if os.path.exists(lib) else os.path.join(out_dir, "dungeon1.dst")
        if not os.path.exists(self.state_path):
            self._make_state()
        self.emu.savestate.load_file(self.state_path)
        self.gd = self.u32(G_DUNGEON)
        self.team = [self.u32(self.gd + TEAM + 4 * i) for i in range(2)]
        self.rolls = []; self.cs_calls = []; self.done = False; self.active = False; self.pending = None
        self.hook(0x0219D058, lambda: self.rolls.append(self.R.r0))          # DungeonRandInt
        self.hook(0x0219CF28, self._on_cst)                                  # CalculateStatusTurns
        self.mem.register_exec(GATE, self._on_gate)
        self.mem.register_exec(SENT, self._on_sent)

    # memory
    def rd(self, a, n):
        return bytes(self.mem.read(a, a + n, 1, False))
    def u8(self, a): return self.rd(a, 1)[0]
    def u32(self, a): return int.from_bytes(self.rd(a, 4), "little")
    def s16(self, a): return int.from_bytes(self.rd(a, 2), "little", signed=True)
    def w8(self, a, v): self.mem.write_byte(a, v)
    def w32(self, a, v): self.mem.write_long(a, v & 0xFFFFFFFF)

    def hook(self, addr, fn):
        """Run fn() when the CPU is about to execute addr (once per hit)."""
        self.mem.register_exec(addr, lambda *_: fn() if self.R.r15 == addr + 8 else None)

    def info(self, ent): return self.u32(ent + 0x70)

    def _make_state(self):
        boot = self.h.parse_inputs(HERE + "/boot_to_dungeon.txt")
        for f in range(30000):
            if f in boot: self.emu.input.keypad_update(boot[f])
            self.emu.cycle(with_joystick=False)
            if f % 10 == 0 and self.u32(G_DUNGEON) != 0:
                for _ in range(120): self.emu.cycle(with_joystick=False)
                self.emu.savestate.save_file(self.state_path)
                return
        raise SystemExit("never reached a dungeon")

    def fresh(self, k):
        """Reload the floor, silence the message printers, reseed the dungeon RNG from k."""
        self.emu.savestate.load_file(self.state_path)
        for a in MSG_PRINTERS: self.w32(a, BX_LR)
        self.w32(RNG, 0x9E3779B1 * (k + 1) + 12345)
        self.active = False; self.done = False; self.pending = None

    def _on_cst(self):
        p = self.R.r1
        self.cs_calls.append(((self.s16(p), self.s16(p + 2)), self.R.r2 & 0xFF))

    def _on_gate(self, *_):
        if self.pending and not self.active and self.R.r15 == GATE + 8:
            addr, args = self.pending; self.pending = None; self.active = True
            self.rolls = []; self.cs_calls = []
            for i, a in enumerate(args[:4]): setattr(self.R, f"r{i}", a & 0xFFFFFFFF)
            if len(args) > 4:                       # stack arguments, as the ARM ABI passes them
                sp = (self.R.r13 - 32) & ~7
                for j, a in enumerate(args[4:]): self.w32(sp + 4 * j, a)
                self.R.r13 = sp
            self.R.r14 = SENT; self.R.r15 = addr; self.mem.set_next_instruction(addr)

    def _on_sent(self, *_):
        if self.active:
            self.active = False; self.done = True
            self.R.r15 = GATE; self.mem.set_next_instruction(GATE)   # park until the next reload

    def call(self, addr, args, max_frames=900):
        """Run addr(args) once, from the turn gate. Returns (finished, frames used)."""
        self.done = False; self.pending = (addr, args); n = 0
        while not self.done and n < max_frames:
            self.emu.cycle(with_joystick=False); n += 1
        return self.done, n


def parse_args(doc):
    import argparse
    p = argparse.ArgumentParser(description=doc)
    p.add_argument("rom", help="scratch-named copy of pmdb.nds, outside the repo")
    p.add_argument("out_dir", help="scratch directory (savestate, DeSmuME config); outside the repo")
    return p
