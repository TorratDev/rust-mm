#!/usr/bin/env python3
"""Per-turn dungeon trace on the supported Blue dump (V7, retail run).

Boots the ROM in DeSmuME (py-desmume), replays BOOT_INPUTS from power-on until
the dungeon struct pointer is set (a new game reaches the first dungeon floor),
then plays TURN_INPUTS for --frames frames. Every time RunFractionalTurn_Async
runs, it writes one JSON line with documented state fields (see
docs/re/structs.md and docs/re/rng.md): RNG states, seeds, turn counter, team
and wild Pokemon, and a hash of the floor's tile grid. No raw RAM is written.

At the end it checks the trace against docs/re/rng.md:
  - each RNG state is reachable from the previous one with the LCG
    s = s * 0x5D588B65 + 1;
  - the floor seed equals GenerateDungeonRNGSeed applied to the run preseed,
    and the dungeon RNG state is reachable from InitDungeonRNG(floor seed).
It prints a JSON summary that includes the SHA-256 of the trace.

Input files: one "FRAME KEY KEY ..." line per change, keys held until the next
line, "-" releases all keys. Key names: A B SELECT START RIGHT LEFT UP DOWN R L
X Y. Frames in BOOT_INPUTS count from power-on; frames in TURN_INPUTS count
from the first dungeon frame.

Usage:
  retail_trace.py ROM OUT.jsonl [--boot-inputs FILE] [--turn-inputs FILE]
                  [--frames N] [--max-boot-frames N]

The ROM is the user's own dump and the output stays local; see docs/legal.md.
"""

import argparse
import hashlib
import json
import os
import struct
import sys

from desmume.controls import Keys, keymask
from desmume.emulator import DeSmuME

HERE = os.path.dirname(os.path.abspath(__file__))

# Addresses from docs/re/*.md (supported EUR dump only).
TRIGGER = 0x021411BC           # RunFractionalTurn_Async (overlay 1)
G_DUNGEON = 0x020E0788         # pointer to the dungeon struct
S_PRNG = 0x020DF704            # global LCG state
G_DUNGEON_RNG = 0x021B3F58     # dungeon LCG state (overlay 1)
G_DUNGEON_PRESEED = 0x021B3F5C # dungeon preseed LCG state (overlay 1)
TILES, TILES_LEN = 0x34B8, 56 * 32 * 0x14
TEAM, WILD = 0x11314, 0x11324
MUL, MASK = 0x5D588B65, 0xFFFFFFFF


def parse_inputs(path):
    changes = {}
    with open(path) as f:
        for line in f:
            line = line.split("#", 1)[0].split()
            if not line:
                continue
            mask = 0
            for name in line[1:]:
                if name != "-":
                    mask |= keymask(getattr(Keys, "KEY_" + name.upper()))
            changes[int(line[0])] = mask
    return changes


class Ram:
    """Main RAM reads through py-desmume's per-byte API (works on every OS)."""

    def __init__(self, emu):
        self.emu = emu

    def bytes(self, addr, n):
        if n == 1:
            return bytes([self.emu.memory.read(addr, addr, 1, False)])
        return bytes(self.emu.memory.read(addr, addr + n, 1, False))

    def u8(self, a):
        return self.bytes(a, 1)[0]

    def s16(self, a):
        return struct.unpack("<h", self.bytes(a, 2))[0]

    def u32(self, a):
        return struct.unpack("<I", self.bytes(a, 4))[0]


def entity(ram, ptr):
    if ptr == 0 or ram.u32(ptr) == 0:  # EntityIsValid: type != 0
        return None
    info = ram.u32(ptr + 0x70)
    return {
        "type": ram.u32(ptr),
        "x": ram.s16(ptr + 0x4), "y": ram.s16(ptr + 0x6),
        "species": ram.s16(info + 0x2), "level": ram.u8(info + 0x9),
        "hp": ram.s16(info + 0xE), "maxHp": ram.s16(info + 0x10),
        "speedStage": ram.u32(info + 0xE0),
    }


def snapshot(ram, dungeon):
    return {
        "fractionalTurn": ram.s16(dungeon + 0x650),
        "runSeed": ram.u32(dungeon + 0x66C),
        "floorSeed": ram.u32(dungeon + 0x670),
        "sPRNGState": ram.u32(S_PRNG),
        "dungeonRng": ram.u32(G_DUNGEON_RNG),
        "dungeonPreseed": ram.u32(G_DUNGEON_PRESEED),
        "team": [entity(ram, ram.u32(dungeon + TEAM + 4 * i)) for i in range(4)],
        "wild": [e for e in (entity(ram, ram.u32(dungeon + WILD + 4 * i))
                             for i in range(16)) if e],
    }


def step(s):
    return (s * MUL + 1) & MASK


def steps_to(a, b, limit=200000):
    for k in range(limit + 1):
        if a == b:
            return k
        a = step(a)
    return None


def generate_floor_seed(preseed):
    """GenerateDungeonRNGSeed (docs/re/rng.md): returns (floor seed, new state)."""
    s1 = step(preseed)
    s2 = step(s1)
    return ((((s2 & 0xFFFF0000) | (s1 >> 16)) & 0xFFFFFF) | 1), s2


def check(rows):
    gaps = {"global": [], "dungeon": []}
    for prev, cur in zip(rows, rows[1:]):
        gaps["global"].append(steps_to(prev["sPRNGState"], cur["sPRNGState"]))
        if prev["floorSeed"] == cur["floorSeed"]:
            gaps["dungeon"].append(steps_to(prev["dungeonRng"], cur["dungeonRng"]))
    summary = {
        name: {"pairs": len(g), "unreachable": g.count(None),
               "maxSteps": max((k for k in g if k is not None), default=None)}
        for name, g in gaps.items()
    }
    first = rows[0]
    pre = (first["runSeed"] | 1) & 0xFFFFFF  # SetDungeonRNGPreseed23Bit
    call = None
    for n in range(1, 100):
        seed, pre = generate_floor_seed(pre)
        if seed == first["floorSeed"]:
            call = n
            break
    summary["seedChain"] = {
        "floorSeedFromRunSeedCall": call,
        "preseedStateMatches": call is not None and pre == first["dungeonPreseed"],
        "dungeonRngStepsFromFloorSeed": steps_to(first["floorSeed"] | 1, first["dungeonRng"]),
    }
    return summary


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--boot-inputs", default=os.path.join(HERE, "boot_to_dungeon.txt"))
    p.add_argument("--turn-inputs", default=os.path.join(HERE, "turn_inputs.txt"))
    p.add_argument("--frames", type=int, default=3000)
    p.add_argument("--max-boot-frames", type=int, default=30000)
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(a.rom)
    ram = Ram(emu)

    boot = parse_inputs(a.boot_inputs)
    dungeon_frame = None
    for f in range(a.max_boot_frames):
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
            dungeon_frame = f
            break
    if dungeon_frame is None:
        print("no dungeon reached; check the ROM and the boot inputs", file=sys.stderr)
        emu.destroy()

    turns = parse_inputs(a.turn_inputs)
    rows, tiles, frame = [], {}, [0]

    def hook(*_):
        dungeon = ram.u32(G_DUNGEON)
        if dungeon == 0:
            return
        row = snapshot(ram, dungeon)
        if row["floorSeed"] != tiles.get("seed"):
            tiles["seed"] = row["floorSeed"]
            tiles["hash"] = hashlib.sha256(ram.bytes(dungeon + TILES, TILES_LEN)).hexdigest()[:16]
        row["tilesSha256"] = tiles["hash"]
        row["frame"] = frame[0]
        rows.append(row)

    emu.memory.register_exec(TRIGGER, hook)
    for f in range(a.frames):
        frame[0] = f
        if f in turns:
            emu.input.keypad_update(turns[f])
        emu.cycle(with_joystick=False)

    with open(a.out, "w") as out:
        for row in rows:
            out.write(json.dumps(row, sort_keys=True) + "\n")
    with open(a.out, "rb") as f:
        digest = hashlib.sha256(f.read()).hexdigest()
    summary = {"dungeonFrame": dungeon_frame, "hits": len(rows), "sha256": digest,
               "turnValues": sorted({r["fractionalTurn"] for r in rows})}
    if rows:
        summary.update(check(rows))
    print("\n" + json.dumps(summary), file=sys.stderr, flush=True)
    emu.destroy()  # libdesmume ends the process here


if __name__ == "__main__":
    main()
