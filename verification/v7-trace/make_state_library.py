#!/usr/bin/env python3
"""Build the shared savestate library (#643, #648) for the status probes.

Makes dungeon1-f1.dst (first dungeon, floor 1: boot_to_dungeon.txt, 60 frames, save) with
evolution_probe.py. The probe runs on a scratch copy of the ROM under a throwaway name and a
scratch XDG_CONFIG_HOME, so ~/.config/desmume is not touched. An existing file is kept.

Usage: RESCUE_ROM=/path/pmdb.nds RESCUE_PY=/path/python make_state_library.py [STATES_DIR]
STATES_DIR defaults to ~/rescue-re/saves/states. Nothing is written into the repository.
Not covered: boot-town.dst needs a maintainer save and ground_script_log.py (see the README there).
"""
import hashlib, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def sha1(path):
    with open(path, "rb") as f:
        return hashlib.sha1(f.read()).hexdigest()


def main():
    rom, py = os.environ.get("RESCUE_ROM", ""), os.environ.get("RESCUE_PY", "")
    if not os.path.isfile(rom):
        sys.exit("RESCUE_ROM must name the retail .nds dump")
    if not os.path.isfile(py):
        sys.exit("RESCUE_PY must name the py-desmume python")
    out = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/rescue-re/saves/states"))
    dst = os.path.join(out, "dungeon1-f1.dst")
    os.makedirs(out, exist_ok=True)
    if os.path.exists(dst):
        print(f"kept existing {dst} sha1 {sha1(dst)}")
        return
    with tempfile.TemporaryDirectory(prefix="rescue-states-") as work:
        scratch_rom = os.path.join(work, "state-maker.nds")  # throwaway name: DeSmuME backs up <rom>.dsv
        shutil.copy(rom, scratch_rom)
        made = os.path.join(work, "dungeon1-f1.dst")
        env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(work, "xdg"),
                   SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
        subprocess.run([py, os.path.join(HERE, "evolution_probe.py"), scratch_rom,
                        os.path.join(work, "run.json"), "--savestate", made], env=env, check=True)
        if not os.path.isfile(made):
            sys.exit("evolution_probe.py did not write the savestate")
        os.makedirs(out, exist_ok=True)
        shutil.move(made, dst)
    print(f"wrote {dst} sha1 {sha1(dst)}")


if __name__ == "__main__":
    main()
