#!/usr/bin/env python3
"""#639 interactive playthrough: opens a DeSmuME window on the user's display with a save and lets the
user play with the keyboard (default DeSmuME keys). Logs GROUND_MAP changes to stdout, so the path to
Luminous Cave can be read from the log. Quit by closing the window.
Usage: play_639.py ROM SAV OUTDIR
Saves OUTDIR/cave.dst the first time GROUND_MAP is 8 (Luminous Cave), and OUTDIR/manual-N.dst on F5.
ROM is a scratch copy under a throwaway name; run with XDG_CONFIG_HOME in a scratch directory.
"""
import ctypes, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from desmume.emulator import DeSmuME  # noqa: E402
from desmume.controls import Keys, keymask  # noqa: E402
from retail_trace import Ram  # noqa: E402

GROUND_MAP = 0x0210E670
# D-pad remap: physical I/J/K/L (X11 keysyms) drive Up/Left/Down/Right. The DeSmuME window's default
# arrow keys do not reach the emulator in this build, so the keys are read from the X server directly.
REMAP = {"i": Keys.KEY_UP, "j": Keys.KEY_LEFT, "k": Keys.KEY_DOWN, "l": Keys.KEY_RIGHT}


class PhysicalKeys:
    """Polls the X server for the pressed physical keys (XQueryKeymap), so focus does not matter."""

    def __init__(self, display=":0"):
        self.x11 = ctypes.cdll.LoadLibrary("libX11.so.6")
        self.x11.XOpenDisplay.restype = ctypes.c_void_p
        self.x11.XStringToKeysym.restype = ctypes.c_ulong
        self.x11.XKeysymToKeycode.restype = ctypes.c_ubyte
        self.dpy = self.x11.XOpenDisplay(display.encode())
        self.codes = {}
        for name, key in REMAP.items():
            sym = self.x11.XStringToKeysym(name.encode())
            self.codes[self.x11.XKeysymToKeycode(ctypes.c_void_p(self.dpy), sym)] = key

    def pressed(self):
        buf = (ctypes.c_char * 32)()
        self.x11.XQueryKeymap(ctypes.c_void_p(self.dpy), buf)
        out = []
        for code, key in self.codes.items():
            if (ord(buf[code // 8]) >> (code % 8)) & 1:
                out.append(key)
        return out

    def f5(self):
        """True while F5 is held; a press saves a manual savestate."""
        buf = (ctypes.c_char * 32)()
        self.x11.XQueryKeymap(ctypes.c_void_p(self.dpy), buf)
        code = self.x11.XKeysymToKeycode(ctypes.c_void_p(self.dpy), self.x11.XStringToKeysym(b"F5"))
        return bool((ord(buf[code // 8]) >> (code % 8)) & 1)


CAVE_MAP = 8  # MAP_LUMINOUS_CAVE in pret/pmd-red (include/constants/ground_map.h)


def main():
    rom, sav, outdir = sys.argv[1], sys.argv[2], sys.argv[3]
    os.makedirs(outdir, exist_ok=True)
    emu = DeSmuME(); emu.open(os.path.abspath(rom))
    emu.backup.import_file(os.path.abspath(sav))
    ram = Ram(emu)
    win = emu.create_sdl_window(auto_pause=False)
    phys = PhysicalKeys()
    last, f, t0 = None, 0, time.time()
    cave_saved, manual, f5_was_down = False, 0, False
    print("playing; close the window to stop", flush=True)
    while not win.has_quit():
        win.process_input()
        for key in phys.pressed():
            emu.input.keypad_add_key(keymask(key))
        emu.cycle(with_joystick=True)
        win.draw()
        f += 1
        m = ram.s16(GROUND_MAP)
        if m != last:
            print("frame", f, "map", m, flush=True)
            last = m
            if m == CAVE_MAP and not cave_saved:
                emu.savestate.save_file(os.path.join(outdir, "cave.dst"))
                cave_saved = True
                print("saved cave.dst", flush=True)
        if phys.f5() and not f5_was_down:
            manual += 1
            emu.savestate.save_file(os.path.join(outdir, "manual-%d.dst" % manual))
            print("saved manual-%d.dst at frame %d map %d" % (manual, f, m), flush=True)
        f5_was_down = phys.f5()
        dt = t0 + f / 60.0 - time.time()
        if dt > 0:
            time.sleep(dt)
    print("closed at frame", f, "last map", last, flush=True)
    win.destroy()


if __name__ == "__main__":
    main()
