#!/usr/bin/env python3
"""Walk the leader to the first floor's stairs by D-pad input, then press A and log the floor exit (#641).

Starts from a savestate of dungeon floor 1 (dungeon1-f1.dst, see docs/re/progression/p4-friend-areas.md),
reads the floor grid (dungeon +0x34B8, 56 x 32 tiles of 0x14 bytes) and the stairs pair B (+0xC0BC) from RAM,
and plans a 4-way BFS path over open tiles (terrain type 1 without the impassable-wall bit 0x10).
The leader is team slot 0 (dungeon +0x11364, entity pointer; position at entity +4 / +6, s16).

Closed loop: for each step, press one D-pad key for --hold frames, then wait until the leader's position
changes to the expected tile. A step that does not move the leader within --wait frames stops the walk
and is recorded as blocked (no replanning). A position that moves the wrong way is recorded as a mismatch
and stops the walk, so the direction mapping is checked on every step.

On arrival at the stairs, A is pressed every --a-every frames for --after frames. The exit hooks of
friend_areas_probe.py (0206DC20 entry and exit, 02178068, 0206ED24, 0206ED3C, 0206ED4C, 02177FD8) are
recorded with their frame and r0; 0206DC20's result code is read at dungeon +0x60 on each hit.

Nothing here is forced: the leader is moved only by keys. A walk that never reaches the stairs or an exit
that never fires is reported as such, with the position where it stopped.

Usage: stairs_walk_probe.py ROM OUT.json [--savestate PATH] [--hold N] [--wait N] [--a-every N] [--after N]
ROM must be a throwaway copy with a name other than pmdb.nds (DeSmuME writes <rom>.dsv). Keep ROM and OUT
outside the repository.
"""
import argparse
import json
import os
import sys
from collections import deque

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME  # noqa: E402
from desmume.controls import Keys, keymask  # noqa: E402
from retail_trace import G_DUNGEON, Ram  # noqa: E402

TILES = 0x34B8          # tiles[32][56], row stride 0x460, tile 0x14 bytes
ROW, TILE = 0x460, 0x14
STAIRS = 0xC0BC         # stairs pair B (x, y), s16
TEAM0 = 0x11364         # team slot 0 entity pointer (levelup_probe.py uses this list)
ENT_X, ENT_Y = 4, 6
IMPASSABLE = 0x10
EXIT = (0x0206DC20, 0x0206ED24, 0x0206ED3C, 0x0206ED4C, 0x02177FD8, 0x02178068)
RESULT_CODE = 0x60
DIRS = {(0, -1): "UP", (0, 1): "DOWN", (-1, 0): "LEFT", (1, 0): "RIGHT"}
DEFAULT_STATE = os.path.expanduser("~/rescue-re/saves/states/dungeon1-f1.dst")


def bfs(open_tile, start, goal):
    prev = {start: None}
    q = deque([start])
    while q:
        cur = q.popleft()
        if cur == goal:
            break
        for dx, dy in DIRS:
            nxt = (cur[0] + dx, cur[1] + dy)
            if nxt in prev or not (0 <= nxt[0] < 56 and 0 <= nxt[1] < 32):
                continue
            if not open_tile(nxt) and nxt != goal:
                continue
            prev[nxt] = cur
            q.append(nxt)
    if goal not in prev:
        return None
    path, cur = [], goal
    while prev[cur] is not None:
        path.append((cur[0] - prev[cur][0], cur[1] - prev[cur][1]))
        cur = prev[cur]
    return list(reversed(path))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom")
    p.add_argument("out")
    p.add_argument("--savestate", default=DEFAULT_STATE)
    p.add_argument("--hold", type=int, default=6, help="frames each D-pad press is held")
    p.add_argument("--wait", type=int, default=600, help="frames to wait for the leader to move")
    p.add_argument("--retries", type=int, default=4, help="presses of the same key before a step is blocked")
    p.add_argument("--a-every", type=int, default=60)
    p.add_argument("--after", type=int, default=900, help="frames to keep pressing A after arrival")
    a = p.parse_args()

    emu = DeSmuME()
    emu.open(os.path.abspath(a.rom))
    ram = Ram(emu)
    emu.savestate.load_file(os.path.abspath(a.savestate))
    for _ in range(60):
        emu.cycle(with_joystick=False)

    d = ram.u32(G_DUNGEON)
    if not d:
        sys.exit("no dungeon in the savestate")
    leader = ram.u32(d + TEAM0)
    if not leader:
        sys.exit("no leader entity in team slot 0")

    def pos():
        return ram.s16(leader + ENT_X), ram.s16(leader + ENT_Y)

    def open_tile(xy):
        base = d + TILES + xy[1] * ROW + xy[0] * TILE
        terrain = int.from_bytes(ram.bytes(base, 2), "little")
        occupied = ram.u32(base + 0xC) != 0  # a monster on the tile: moving into it attacks instead
        return (terrain & 0x3) == 1 and not (terrain & IMPASSABLE) and not occupied

    stairs = (ram.s16(d + STAIRS), ram.s16(d + STAIRS + 2))
    start = pos()
    res = {"start": start, "stairs": stairs, "start_is_expected_20_25": start == (20, 25),
           "steps": [], "blocked": None, "mismatch": None, "arrived": False, "after_frames": 0}
    path = bfs(open_tile, start, stairs)
    if path is None:
        res["no_path"] = True
        with open(a.out, "w") as f:
            json.dump(res, f, indent=1)
        sys.exit("no open path from the leader to the stairs on this floor")
    res["path_length"] = len(path)

    events = []
    frame = {"n": 0}

    def hook(name):
        def cb(addr, size):
            e = {"hook": name, "frame": frame["n"], "r0": int(emu.memory.register_arm9.r0),
                 "pos": pos()}
            if name == "0206DC20":
                e["code"] = ram.s16(ram.u32(G_DUNGEON) + RESULT_CODE) if ram.u32(G_DUNGEON) else None
            events.append(e)
        return cb

    for x in EXIT:
        emu.memory.register_exec(x, hook("%08X" % x))

    def run(n, keys=0):
        for _ in range(n):
            frame["n"] += 1
            emu.input.keypad_update(keys)
            emu.cycle(with_joystick=False)

    def step_to(delta, expect):
        # A press can be refused while a partner next to the path is still moving: retry the same key
        # up to --retries times before the step is reported as blocked.
        key = DIRS[delta]
        before = pos()
        attempts = 0
        for attempts in range(1, a.retries + 1):
            emu.input.keypad_update(keymask(getattr(Keys, "KEY_" + key)))
            for _ in range(a.hold):
                frame["n"] += 1
                emu.cycle(with_joystick=False)
            emu.input.keypad_update(0)
            for _ in range(a.wait):
                frame["n"] += 1
                emu.cycle(with_joystick=False)
                if pos() != before:
                    break
            if pos() != before:
                break
        got = pos()
        ok = got == expect
        res["steps"].append({"key": key, "from": before, "to": got, "expected": expect, "ok": ok,
                             "attempts": attempts, "frame": frame["n"]})
        return ok, before

    # Floor entry: D-pad presses before the first A are not taken (checked by hand on this savestate:
    # no move in 900 frames without A, then UP moves after one A). movement_inputs.txt starts the same way.
    run(900)
    run(a.hold, keymask(Keys.KEY_A))
    run(200)

    # Replan before every step, so a monster that moved onto the path is avoided. Stop after 400 steps.
    cur = start
    for i in range(400):
        if cur == stairs:
            break
        path = bfs(open_tile, cur, stairs)
        if path is None:
            res["blocked"] = {"step": i, "at": cur, "key": None, "reason": "no open path from here"}
            break
        delta = path[0]
        expect = (cur[0] + delta[0], cur[1] + delta[1])
        ok, before = step_to(delta, expect)
        if not ok:
            if pos() == before:
                res["blocked"] = {"step": i, "at": before, "key": DIRS[delta]}
            else:
                res["mismatch"] = {"step": i, "at": before, "got": pos(), "key": DIRS[delta]}
            break
        cur = expect
        run(30)  # let the turn finish before the next press

    if res["blocked"] is None and res["mismatch"] is None and pos() == stairs:
        res["arrived"] = True
        t = 0
        while t < a.after:
            emu.input.keypad_update(keymask(Keys.KEY_A))
            for _ in range(a.hold):
                frame["n"] += 1
                emu.cycle(with_joystick=False)
            emu.input.keypad_update(0)
            for _ in range(a.a_every - a.hold):
                frame["n"] += 1
                emu.cycle(with_joystick=False)
            t += a.a_every
        res["after_frames"] = t

    fired = {e["hook"] for e in events}
    res["hooks"] = {h: sum(1 for e in events if e["hook"] == h) for h in fired}
    res["not_reached"] = ["%08X" % x for x in EXIT if "%08X" % x not in fired]
    res["events"] = events
    res["final_pos"] = pos()
    with open(a.out, "w") as f:
        json.dump(res, f, indent=1)
    print(json.dumps({k: res[k] for k in ("start", "stairs", "path_length", "blocked", "mismatch",
                                          "arrived", "final_pos", "hooks", "not_reached")}, indent=1),
          flush=True)
    emu.destroy()


if __name__ == "__main__":
    main()
