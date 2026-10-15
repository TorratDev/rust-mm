#!/usr/bin/env python3
"""Stage-by-stage floor-generation oracle on the supported Blue dump (#48).

Replays BOOT_INPUTS from power-on to the first dungeon floor (as
retail_trace.py does) and records, at the entry of every floor-generation step
(GenerateFloor, its layout generators, the 13 steps of GenerateStandardFloor,
the spawn steps) and at the first turn, the dungeon RNG state, the arguments
r0-r3, the first two stack words (the fifth and sixth arguments of functions
that take them), and the number of dungeon RNG steps since the previous entry.
A reimplementation must use the same number of RNG steps between each pair of
checkpoints. With --seed, the dungeon RNG is forced to that state when
GenerateFloor starts (see floor_oracle.py).

Usage:
  floor_stages.py ROM OUT.jsonl [--seed S] [--force-chance 8=P,9=P,10=P] [--maze-gate] [--boot-inputs FILE]
Output: one JSON line per checkpoint; only states, counts and arguments.
"""

import argparse
import json
import os
import struct
import sys

from desmume.controls import Keys, keymask
from desmume.emulator import DeSmuME

from retail_trace import G_DUNGEON, G_DUNGEON_RNG, HERE, TRIGGER, parse_inputs, steps_to

STEPS = {
    0x02151E7C: "GenerateFloor",
    0x0214BDBC: "ResetFloor",
    0x02150958: "ProcessFixedRoom",
    0x02151BF4: "GenerateStandardFloor",
    0x0214FEB0: "std1_GridBounds",
    0x0214FD7C: "std2_InitGrid",
    0x0214FB4C: "std3",
    0x0214F7A8: "std4",
    0x0214F6A0: "std5",
    0x0214E824: "std6",
    0x0214DB54: "std7",
    0x0214CF2C: "std8",
    0x0214D448: "std9",
    0x0214D190: "std10",
    0x0214FF30: "std11",
    0x0214E3DC: "std12",
    0x0214F6F0: "std13",
    0x021506B0: "GenerateOneRoomMonsterHouseFloor",
    0x0215174C: "GenerateOuterRingFloor",
    0x021512B4: "GenerateCrossroadsFloor",
    0x021504F4: "GenerateTwoRoomsWithMonsterHouseFloor",
    0x02151160: "GenerateLineFloor",
    0x02150FC0: "GenerateCrossFloor",
    0x02150DFC: "GenerateBeetleFloor",
    0x02150A10: "GenerateOuterRoomsFloor",
    0x02149A70: "ResetInnerBoundaryTileRows",
    0x0214C0E0: "EnsureImpassableTilesAreWalls",
    0x0214D900: "FinalizeJunctions",
    0x0214AAF8: "GenerateSecondaryTerrainFormations",
    0x0214B650: "SpawnNonEnemies",
    0x0214B338: "SpawnEnemies",
    0x0214C1A8: "ResolveInvalidSpawns",
    0x0214A760: "StairsAlwaysReachable",
    TRIGGER: "FirstTurn",
}


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--seed", type=lambda s: int(s, 0))
    p.add_argument("--force-chance", default="",
                   help="STEP=PERCENT,... (STEP 8, 9 or 10): at that step's entry, replace its chance "
                        "argument (r3), so the optional rooms fire on floors whose properties give 0")
    p.add_argument("--maze-gate", action="store_true",
                   help="write -1 to dungeon +0x34B6 at std8's entry (the attempt index, which is "
                        "never negative in a normal run, gates the maze room)")
    p.add_argument("--boot-inputs", default=os.path.join(HERE, "boot_to_dungeon.txt"))
    p.add_argument("--max-boot-frames", type=int, default=30000)
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(a.rom)
    regs = emu.memory.register_arm9
    u32 = lambda addr: struct.unpack("<I", bytes(emu.memory.read(addr, addr + 4, 1, False)))[0]  # noqa: E731

    forced = {int(k): int(v) for k, v in (kv.split("=") for kv in a.force_chance.split(",") if kv)}
    boot = parse_inputs(a.boot_inputs)
    for f in range(a.max_boot_frames):
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and u32(G_DUNGEON) != 0:
            break
    else:
        print("no dungeon reached; check the ROM and the boot inputs", file=sys.stderr)
        emu.destroy()

    log, done = [], []

    def tiles_fnv1a():
        # FNV-1a 64 over terrainFlags (u16 LE) and room (u8) of the 56 x 32
        # tiles, row by row. Same hash as floor::tiles_fnv1a in Rust.
        base = u32(G_DUNGEON) + 0x34B8
        h = 0xCBF29CE484222325
        for y in range(32):
            row = bytes(emu.memory.read(base + y * 0x460, base + (y + 1) * 0x460, 1, False))
            for x in range(56):
                t = row[x * 0x14:(x + 1) * 0x14]
                for b in (t[0], t[1], t[5]):
                    h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
        return f"{h:016x}"

    def make(name):
        def hook(*_):
            if done:
                return
            if name == "GenerateFloor" and a.seed is not None:
                emu.memory.write_long(G_DUNGEON_RNG, a.seed)
            if name == "FirstTurn":
                if not any(e["step"] == "GenerateFloor" for e in log):
                    return
                done.append(True)
            sp = regs.r13 & 0xFFFFFFFF
            entry = {"step": name, "rng": u32(G_DUNGEON_RNG),
                     "args": [regs.r0, regs.r1, regs.r2, regs.r3],
                     "stackArgs": [u32(sp), u32(sp + 4)]}
            step_no = int(name[3:]) if name[:3] == "std" and name[3:].isdigit() else None
            if step_no in forced:
                regs.r3 = forced[step_no]
                entry["args"][3] = forced[step_no]
            if name == "std8" and a.maze_gate:
                emu.memory.write_short(u32(G_DUNGEON) + 0x34B6, 0xFFFF)
            if name in ("std7", "std8", "std9", "std10", "std11"):
                # Tile hash at the entry of steps 7 to 11, to check the hallways (step 6), step 7
                # and the optional rooms (steps 8 to 10).
                entry["tilesFnv1a"] = tiles_fnv1a()
            if name in ("std8", "std9", "std10", "std11"):
                dun = u32(G_DUNGEON)
                # Dungeon +0x34B6 (i16: gate of std8), flags at 021B4590 (maze), 021B4594 (monster
                # house present, read by std9), 021B458C (shop), the pair at 021B45C8, the rect at
                # 021B45CC, and the room bounds at dungeon +0xC0E0.
                entry["state"] = {
                    "attempt34B6": struct.unpack("<h", bytes(emu.memory.read(dun + 0x34B6, dun + 0x34B8, 1, False)))[0],
                    "flags": [bytes(emu.memory.read(x, x + 1, 1, False))[0] for x in (0x21B4590, 0x21B4594, 0x21B458C)],
                    "pair": list(struct.unpack("<2H", bytes(emu.memory.read(0x21B45C8, 0x21B45CC, 1, False)))),
                    "rect": list(struct.unpack("<4i", bytes(emu.memory.read(0x21B45CC, 0x21B45DC, 1, False)))),
                    "houseRoom": bytes(emu.memory.read(dun + 0x34AC, dun + 0x34AD, 1, False))[0],
                    "bounds": list(struct.unpack("<4i", bytes(emu.memory.read(dun + 0xC0E0, dun + 0xC0F0, 1, False)))),
                }
            log.append(entry)
        return hook

    for addr, name in STEPS.items():
        emu.memory.register_exec(addr, make(name))
    a_key = keymask(Keys.KEY_A)
    for f in range(3000):
        emu.input.keypad_update(a_key if f % 30 < 4 else 0)
        emu.cycle(with_joystick=False)
        if done:
            break

    with open(a.out, "w") as out:
        prev = None
        for entry in log:
            entry["stepsSincePrev"] = None if prev is None else steps_to(prev, entry["rng"])
            prev = entry["rng"]
            out.write(json.dumps(entry) + "\n")
    print(json.dumps({"checkpoints": len(log),
                      "totalSteps": sum(e["stepsSincePrev"] or 0 for e in log)}),
          file=sys.stderr, flush=True)
    emu.destroy()  # libdesmume ends the process here


if __name__ == "__main__":
    main()
