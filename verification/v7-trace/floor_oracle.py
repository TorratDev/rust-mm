#!/usr/bin/env python3
"""Floor-generation oracle on the supported Blue dump (#48).

Replays BOOT_INPUTS from power-on to the first dungeon floor (as retail_trace.py
does), keeps that point as a temporary savestate, and then, for every requested
case, reloads it, forces the dungeon RNG state when GenerateFloor
(ov_0001::02151E7C) starts, and lets the game generate the floor. At the first
turn (RunFractionalTurn_Async) it records one JSON line:

  rngAtGenerate   dungeon RNG state at GenerateFloor's entry (the forced seed)
  layoutSha256    hash of the 56 x 32 tile grid with each tile's graphics-variant
                  byte (+0x04) zeroed; that byte comes from the global RNG and is
                  cosmetic (docs/re/dungeon-generation.md)
  fieldSha256     per-field hashes of the grid (docs/re/structs.md, Tile)
  layoutFnv1a     FNV-1a 64 over the first 12 bytes of every tile (the 20 without
                  the two entity pointers), +0x04 zeroed, row by row; the same
                  hash as rescue_sim::floor::layout_fnv1a
  stairs, rooms, terrainCounts, rngSteps (dungeon RNG steps until the first turn)
  generateEnd     the same hashes and counts, and rngSteps, at GenerateFloor's
                  return (0x021525B4), where a reimplementation of GenerateFloor ends

A case may also force the global RNG state ("global"), to check that the layout
does not depend on it, and the floor-properties layout byte ("layout", +0x00),
to generate floors of other layouts. The output holds hashes and counts only, never the grid
itself (docs/legal.md), so it can serve as a test oracle for a reimplementation.

Usage:
  floor_oracle.py ROM OUT.jsonl [--seeds S,S,...] [--globals G,G,...]
                  [--layouts L,L,...] [--boot-inputs FILE]
Without --seeds, the floor keeps its natural seed. With --globals and --layouts,
every seed is run once per global value and layout byte.
"""

import argparse
import hashlib
import json
import os
import struct
import sys
import tempfile

from desmume.controls import Keys, keymask
from desmume.emulator import DeSmuME

from retail_trace import G_DUNGEON, G_DUNGEON_RNG, HERE, S_PRNG, TRIGGER, parse_inputs, steps_to

GENERATE_FLOOR = 0x02151E7C
GENERATE_FLOOR_END = 0x021525B4  # after CloseFile, GenerateFloor's last call (floor_stages.py)
TILES, STAIRS = 0x34B8, 0xC0BC
PROPS = 0x1A2F6  # the floor properties GenerateFloor reads (SpawnNonEnemies' r0), in the dungeon struct
TILE_SIZE, TILE_COUNT = 0x14, 56 * 32
FIELDS = {"terrain": (0x0, 2), "spawnOrVisibility": (0x2, 2), "graphicsVariant": (0x4, 1),
          "room": (0x5, 1), "walkable": (0x6, 4), "unkE": (0xA, 2), "monster": (0xC, 4),
          "object": (0x10, 4)}


def describe(grid, rd_u32_stairs):
    tiles = [grid[i * TILE_SIZE:(i + 1) * TILE_SIZE] for i in range(TILE_COUNT)]
    layout = b"".join(t[:4] + b"\0" + t[5:] for t in tiles)
    terrain = [struct.unpack_from("<H", t)[0] & 0x3 for t in tiles]
    fnv = 0xCBF29CE484222325
    for t in tiles:
        for b in t[:4] + b"\0" + t[5:12]:
            fnv = ((fnv ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return {
        "layoutSha256": hashlib.sha256(layout).hexdigest()[:16],
        "layoutFnv1a": f"{fnv:016x}",
        "fieldSha256": {name: hashlib.sha256(b"".join(t[o:o + n] for t in tiles)).hexdigest()[:8]
                        for name, (o, n) in FIELDS.items()},
        "terrainCounts": {str(k): terrain.count(k) for k in sorted(set(terrain))},
        "rooms": len({t[5] for t in tiles} - {0xFF}),
        "stairs": list(rd_u32_stairs),
    }


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--seeds", default="")
    p.add_argument("--globals", default="")
    p.add_argument("--layouts", default="")
    p.add_argument("--boot-inputs", default=os.path.join(HERE, "boot_to_dungeon.txt"))
    p.add_argument("--max-boot-frames", type=int, default=30000)
    a = p.parse_args()
    seeds = [int(x, 0) for x in a.seeds.split(",") if x] or [None]
    globals_ = [int(x, 0) for x in a.globals.split(",") if x] or [None]
    layouts = [int(x, 0) for x in a.layouts.split(",") if x] or [None]

    emu = DeSmuME()
    emu.open(a.rom)
    rd = lambda addr, n: bytes(emu.memory.read(addr, addr + n, 1, False))  # noqa: E731
    u32 = lambda addr: struct.unpack("<I", rd(addr, 4))[0]  # noqa: E731

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
    fd, start = tempfile.mkstemp(suffix=".dst")  # local only, deleted below
    os.close(fd)
    emu.savestate.save_file(start)

    a_key = keymask(Keys.KEY_A)
    rows = []
    cases = [(s, g, lay) for s in seeds for g in globals_ for lay in layouts]
    for seed, glob, lay in cases:
        emu.savestate.load_file(start)
        row = {"seed": seed, "global": glob}

        def on_generate(*_):
            if seed is not None:
                emu.memory.write_long(G_DUNGEON_RNG, seed)
            if glob is not None:
                emu.memory.write_long(S_PRNG, glob)
            props = u32(G_DUNGEON) + PROPS
            if lay is not None:
                emu.memory.write_byte(props, lay)
            row["layout"] = rd(props, 1)[0]
            row["rngAtGenerate"] = u32(G_DUNGEON_RNG)

        def on_generate_end(*_):
            if "rngAtGenerate" not in row or "generateEnd" in row:
                return
            d = u32(G_DUNGEON)
            end = describe(rd(d + TILES, TILE_COUNT * TILE_SIZE),
                           struct.unpack("<hh", rd(d + STAIRS, 4)))
            end["rngSteps"] = steps_to(row["rngAtGenerate"], u32(G_DUNGEON_RNG))
            row["generateEnd"] = end

        def on_turn(*_):
            if "layoutSha256" in row or "rngAtGenerate" not in row:
                return
            d = u32(G_DUNGEON)
            row.update(describe(rd(d + TILES, TILE_COUNT * TILE_SIZE),
                                struct.unpack("<hh", rd(d + STAIRS, 4))))
            row["rngSteps"] = steps_to(row["rngAtGenerate"], u32(G_DUNGEON_RNG))

        emu.memory.register_exec(GENERATE_FLOOR, on_generate)
        emu.memory.register_exec(GENERATE_FLOOR_END, on_generate_end)
        emu.memory.register_exec(TRIGGER, on_turn)
        for f in range(3000):
            emu.input.keypad_update(a_key if f % 30 < 4 else 0)
            emu.cycle(with_joystick=False)
            if "layoutSha256" in row:
                break
        emu.memory.register_exec(GENERATE_FLOOR, None)
        emu.memory.register_exec(GENERATE_FLOOR_END, None)
        emu.memory.register_exec(TRIGGER, None)
        rows.append(row)

    os.remove(start)
    with open(a.out, "w") as out:
        for row in rows:
            out.write(json.dumps(row, sort_keys=True) + "\n")
    by_seed = {}
    for row in rows:
        by_seed.setdefault((row["seed"], row.get("layout")), set()).add(row.get("layoutSha256"))
    print(json.dumps({
        "cases": len(rows),
        "layoutIndependentOfGlobalRng": all(len(v) == 1 for v in by_seed.values()),
        "distinctLayouts": len({row.get("layoutSha256") for row in rows}),
    }), file=sys.stderr, flush=True)
    emu.destroy()  # libdesmume ends the process here


if __name__ == "__main__":
    main()
