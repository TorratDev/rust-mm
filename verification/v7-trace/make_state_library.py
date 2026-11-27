#!/usr/bin/env python3
"""Build the shared savestate library (#643, #648) for the probes.

  dungeon1-f1.dst  first dungeon, floor 1: evolution_probe.py (boot_to_dungeon.txt, 60 frames, save)
  boot-town.dst    Pokemon Team Base (GROUND_MAP 13), free movement: ground_script_log.py with the first
                   0x40000 bytes of RESCUE_SAVE imported, ground_walk_inputs.txt, saved at frame 3400.
                   RESCUE_SAVE defaults to ~/rescue-re/saves/pmdb-maintainer-20261107-2004-after-dream.dsv;
                   skipped when that file is missing.

Each probe runs on a scratch copy of the ROM under a throwaway name and a scratch XDG_CONFIG_HOME, so
~/.config/desmume is not touched. Existing files are kept.

Usage: RESCUE_ROM=/path/pmdb.nds RESCUE_PY=/path/python make_state_library.py [STATES_DIR]
STATES_DIR defaults to ~/rescue-re/saves/states. Nothing is written into the repository.
"""
import hashlib, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SAVE = os.path.expanduser("~/rescue-re/saves/pmdb-maintainer-20261107-2004-after-dream.dsv")


def sha1(path):
    with open(path, "rb") as f:
        return hashlib.sha1(f.read()).hexdigest()


def dungeon1_f1(work, rom, made):
    return ["evolution_probe.py", rom, os.path.join(work, "run.json"), "--savestate", made]


def boot_town(work, rom, made):
    save = os.environ.get("RESCUE_SAVE", SAVE)
    if not os.path.isfile(save):
        return None
    raw = os.path.join(work, "save.raw")
    with open(save, "rb") as f, open(raw, "wb") as o:
        o.write(f.read(0x40000))  # the .dsv is the 256 KiB card image plus DeSmuME's footer
    return ["ground_script_log.py", rom, os.path.join(work, "run.log"), "3500",
            os.path.join(HERE, "ground_walk_inputs.txt"), "--save", raw, "--savestate-at", "3400", made]


STATES = [("dungeon1-f1.dst", dungeon1_f1), ("boot-town.dst", boot_town)]


def main():
    rom, py = os.environ.get("RESCUE_ROM", ""), os.environ.get("RESCUE_PY", "")
    if not os.path.isfile(rom):
        sys.exit("RESCUE_ROM must name the retail .nds dump")
    if not os.path.isfile(py):
        sys.exit("RESCUE_PY must name the py-desmume python")
    out = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/rescue-re/saves/states"))
    os.makedirs(out, exist_ok=True)
    for name, build in STATES:
        dst = os.path.join(out, name)
        if os.path.exists(dst):
            print(f"kept existing {dst} sha1 {sha1(dst)}")
            continue
        with tempfile.TemporaryDirectory(prefix="rescue-states-") as work:
            scratch_rom = os.path.join(work, "state-maker.nds")  # throwaway name: DeSmuME backs up <rom>.dsv
            shutil.copy(rom, scratch_rom)
            made = os.path.join(work, name)
            cmd = build(work, scratch_rom, made)
            if cmd is None:
                print(f"skipped {name}: RESCUE_SAVE not found")
                continue
            env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(work, "xdg"),
                       SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
            subprocess.run([py, os.path.join(HERE, cmd[0])] + cmd[1:], env=env, check=True)
            if not os.path.isfile(made):
                sys.exit(f"{cmd[0]} did not write {name}")
            shutil.move(made, dst)
        print(f"wrote {dst} sha1 {sha1(dst)}")


if __name__ == "__main__":
    main()
