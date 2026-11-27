#!/usr/bin/env python3
"""Natural level-up in a dungeon on retail Blue (#570): stat gains, move learning, removal, forget menu.

Boots to the first floor (boot_to_dungeon.txt), then plays the turn inputs (combat_inputs.txt by default,
the script of the documented run; combat_attack_inputs.txt ends near frame 750 and gives no level-up) so the
team earns EXP from normal defeats. No EXP, level or move list is written, except with
--force-nonteam (below). Exec hooks:
  0x021833BC per-level loop    r1 = entity. Entry and return snapshots of info (= entity+0x70).
  0x02182E5C move learning    r1 = entity, r2 = announce, r3 = interactive.
  0x02182A94 random removal   non-team path; the DungeonRandInt(cap) draw inside it is recorded.
  0x0216D77C forget menu      r0 = entity, r1 = 8-slot list (6 bytes each); the return value is recorded.
  0x0219D058 DungeonRandInt   r0 = cap; recorded only inside a removal call.
  0x02183BB4 EXP pass         counted only.
  0x0215439C prompt           counted inside a learn; the probe then presses A until the menu opens.

--force-moves and --force-exp are forced gates (marked in the output): at the species-4 learner's level-loop
entry, the first fills its empty move slots with copies of slot 0 (all four valid), and the second raises its
EXP to the level-7 threshold, so one pass gains levels 6 and 7 and the level-7 learn (move 262) makes five.

Gains are checked against the scratch record dump (LEVELS, written by verification/v7-trace/levels_dump,
which prints parse_level_data records; the output stays outside the repo):
for each level L gained in one call, the record index L-1 (level L) must match: max HP and HP +hp, the four
stat bytes at +0x14..+0x17 +[attack0, attack1, defense0, defense1] (capped at 255; max HP at 999).
The level reached must be the highest L whose EXP threshold is <= the EXP total at +0x18.

--menu-key picks the forget-menu input: "a" (forget the highlighted first move), "b" (press B first: cancel
must not close the menu; then as "a"), "new" (DOWN x4 to the new move in the fifth slot, then as "a"). Forgetting
is A, A (Forget), UP+A (Yes). The menu is driven only when it opens, one step per 120 frames.

--force-nonteam is a forced gate, not a natural path: it writes info+6 = 1 at the level-loop entry and
restores 0 at its return, so 02182E5C takes the non-team branch and 02182A94 is reached. Marked "forced"
in the output.

Usage: cargo run --release --manifest-path verification/v7-trace/levels_dump/Cargo.toml -- ROM > LEVELS
       levelup_probe.py ROM LEVELS OUT.json [--inputs FILE] [--frames N] [--menu-key a|b|new]
                        [--force-nonteam]
ROM must be a throwaway copy with a name other than pmdb.nds (DeSmuME writes <rom>.dsv).
Keep ROM, LEVELS and OUT outside the repository.
"""
import argparse, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
from desmume.emulator import DeSmuME  # noqa: E402
from desmume.controls import Keys, keymask  # noqa: E402
from retail_trace import G_DUNGEON, Ram, parse_inputs  # noqa: E402

LOOP, EXP_PASS, LEARN, REMOVE, FORGET, RAND = 0x021833BC, 0x02183BB4, 0x02182E5C, 0x02182A94, 0x0216D77C, 0x0219D058
U8_CAP, STAT_CAP, HP_CAP = 255, 255, 999


def s32(v):
    return v - (1 << 32) if v >= 1 << 31 else v


def load_levels(path):
    """Species -> list of (exp, hp, a0, a1, d0, d1) per record, plus the learnset line if present."""
    table, learnset = {}, None
    with open(path) as f:
        for line in f:
            d = json.loads(line)
            if "learnset" in d:
                learnset = d["learnset"]
            else:
                table[d["species"]] = [tuple(r) for r in d["records"]]
    return table, learnset


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rom"); p.add_argument("levels"); p.add_argument("out")
    p.add_argument("--inputs", default=os.path.join(HERE, "combat_inputs.txt"))
    p.add_argument("--frames", type=int, default=12000)
    p.add_argument("--menu-key", choices=["a", "b", "new"], default="a")
    p.add_argument("--force-nonteam", action="store_true")
    p.add_argument("--force-moves", action="store_true")
    p.add_argument("--force-exp", action="store_true")
    a = p.parse_args()
    levels, learnset = load_levels(a.levels)

    emu = DeSmuME(); emu.open(os.path.abspath(a.rom)); ram = Ram(emu); R = emu.memory.register_arm9
    boot = parse_inputs(os.path.join(HERE, "boot_to_dungeon.txt"))
    for f in range(30000):
        if f in boot:
            emu.input.keypad_update(boot[f])
        emu.cycle(with_joystick=False)
        if f % 10 == 0 and ram.u32(G_DUNGEON) != 0:
            break
    else:
        sys.exit("no dungeon reached")
    emu.input.keypad_update(0)
    for _ in range(60):
        emu.cycle(with_joystick=False)

    def snap(info):
        return {"species": ram.s16(info + 2), "level": ram.u8(info + 9), "exp": s32(ram.u32(info + 0x18)),
                "pending_exp": s32(ram.u32(info + 0x1C8)), "maxhp": ram.s16(info + 0x10),
                "hp": ram.s16(info + 0xE), "stats": list(ram.bytes(info + 0x14, 4)),
                "team": ram.u8(info + 6), "moves": [list(ram.bytes(info + 0xF4 + 6 * i, 6)) for i in range(4)]}
    # info holds 4 move slots (+0xF4..+0x10B); +0x10C on is another field, so never count slots 4..7 as moves.
    # The 8-entry lists are the learn's stack copy (r1 of 0216D77C and 02182A94), read from that pointer.

    def valid(moves):
        return [m for m in moves if m[0] & 1]

    res = {"forced_nonteam": a.force_nonteam, "menu_key": a.menu_key, "loop_calls": [], "learn_calls": [],
           "remove_calls": [], "forget_calls": [], "exp_pass": 0, "turns_frames": a.frames}
    st = {"loop": None, "remove": None, "forget": None, "learn": None}
    state = {"menu_open": False, "menu_frames": 0, "remove_window": False}

    def on_loop(addr, size):
        entity = int(R.r1)
        info = ram.u32(entity + 0x70)
        if not info:
            return
        empty = [info + 0xF4 + 6 * i for i in range(4) if not ram.u8(info + 0xF4 + 6 * i) & 1]
        if a.force_moves and ram.s16(info + 2) == 4 and empty:
            # forced gate: fill the empty move slots with copies of slot 0, so the learn's copy holds 5
            src = bytes(ram.bytes(info + 0xF4, 6))
            for s in empty:
                emu.memory.write(s, s + 6, 1, src)
            res["forced_moves_calls"] = res.get("forced_moves_calls", 0) + 1
        if a.force_exp and ram.s16(info + 2) == 4 and ram.u8(info + 9) < 7 and "forced_exp_calls" not in res:
            # forced gate: raise EXP to the level-7 threshold so one pass gains levels 6 and 7 (species 4 learns move 262 at 7)
            thr = levels[4][6][0]
            emu.memory.write(info + 0x18, info + 0x1C, 1, int(thr).to_bytes(4, "little"))
            res["forced_exp_calls"] = 1
        call = {"entity": hex(entity), "pre": snap(info)}
        if a.force_nonteam:
            w8 = ram.u8(info + 6)
            emu.memory.write(info + 6, info + 7, 1, bytes([1]))
            call["forced_team_flag_from"] = w8
        st["loop"] = (call, info)
        res["loop_calls"].append(call)
        emu.memory.register_exec(int(R.r14) & ~1, on_loop_ret)

    def on_loop_ret(addr, size):
        if not st["loop"]:
            return
        call, info = st["loop"]
        st["loop"] = None
        if a.force_nonteam:
            emu.memory.write(info + 6, info + 7, 1, bytes([call.pop("forced_team_flag_from")]))
        call["post"] = snap(info)
        call["returned"] = int(R.r0)

    def on_learn(addr, size):
        entity = int(R.r1)
        info = ram.u32(entity + 0x70)
        call = {"entity": hex(entity), "announce": int(R.r2), "interactive": int(R.r3),
                "pre_valid": len(valid([list(ram.bytes(info + 0xF4 + 6 * i, 6)) for i in range(4)])) if info else -1}
        call.update({"draws": [], "prompt": 0, "msg": 0})
        res["learn_calls"].append(call)
        st["learn"] = (call, info)
        state["learn_win"] = call
        emu.memory.register_exec(int(R.r14) & ~1, on_learn_ret)

    def on_learn_ret(addr, size):
        state["learn_win"] = None

    def on_prompt(addr, size):  # 0215439C: the prompt the menu branch shows before 0216D77C (announce set)
        if state.get("learn_win") is not None:
            state["learn_win"]["prompt"] += 1
            state["prompt_open"], state["prompt_frames"] = True, 0  # it waits for input: press A until the menu opens

    def on_msg(addr, size):  # 0213F50C(entity, 0x138): the learned message
        if state.get("learn_win") is not None:
            state["learn_win"]["msg"] += 1

    def on_remove(addr, size):  # 02182A94(entity, list): list is the learn's 8-entry stack copy
        lst = int(R.r1)
        call = {"entity": hex(int(R.r0)), "list": hex(lst), "pre": [list(ram.bytes(lst + 6 * i, 6)) for i in range(8)],
                "draws": []}
        call["pre_valid"] = len(valid(call["pre"]))
        res["remove_calls"].append(call)
        st["remove"] = (call, lst)
        state["remove_window"] = True
        emu.memory.register_exec(int(R.r14) & ~1, on_remove_ret)

    def on_remove_ret(addr, size):
        state["remove_window"] = False
        if st["remove"]:
            call, lst = st["remove"]
            call["returned"] = int(R.r0)
            call["post"] = [list(ram.bytes(lst + 6 * i, 6)) for i in range(8)]
            # the list is compacted after the removal, so map the drawn index to the valid entries instead
            pre_v, post_v = valid(call["pre"]), valid(call["post"])
            drawn = call["draws"][0].get("drawn") if call["draws"] else None
            if drawn is not None and drawn < len(pre_v):
                call["removed_index"], call["removed_entry"] = drawn, pre_v[drawn]
                call["removed_matches"] = pre_v[:drawn] + pre_v[drawn + 1:] == post_v
                call["removed_new"] = bool(pre_v[drawn][1] & 4)  # byte 1 bit 2: the move just learned
        st["remove"] = None

    def on_rand(addr, size):  # DungeonRandInt(cap): record the cap and, at its return, the drawn index
        tgt = st["remove"][0] if state["remove_window"] and st["remove"] else state.get("learn_win")
        if tgt is None:
            return
        d = {"cap": int(R.r0)}
        tgt["draws"].append(d)
        emu.memory.register_exec(int(R.r14) & ~1, lambda ad, sz, d=d: d.setdefault("drawn", int(R.r0)))

    def on_forget(addr, size):
        entity = int(R.r0)
        lst = int(R.r1)
        pre = [list(ram.bytes(lst + 6 * i, 6)) for i in range(8)]
        call = {"entity": hex(entity), "r2": int(R.r2), "r3": int(R.r3), "pre": pre}
        res["forget_calls"].append(call)
        st["forget"] = (call, lst)
        state["menu_open"] = True
        state["menu_frames"] = 0
        state["prompt_open"] = False
        emu.memory.register_exec(int(R.r14) & ~1, on_forget_ret)

    def on_forget_ret(addr, size):
        state["menu_open"] = False
        if st["forget"]:
            call, lst = st["forget"]
            call["returned"] = int(R.r0)
            call["post"] = [list(ram.bytes(lst + 6 * i, 6)) for i in range(8)]
        st["forget"] = None

    def on_exp_pass(addr, size):
        res["exp_pass"] += 1

    emu.memory.register_exec(LOOP, on_loop)
    emu.memory.register_exec(LEARN, on_learn)
    emu.memory.register_exec(REMOVE, on_remove)
    emu.memory.register_exec(FORGET, on_forget)
    emu.memory.register_exec(RAND, on_rand)
    emu.memory.register_exec(0x0215439C, on_prompt)
    # call-site counters (ghidra-cli): 021830D0 bl prompt, 021830E8 bl 0216D77C, 021830F8 bl 02182A94
    res["entry_hits"] = {}
    for ad in (0x021830D0, 0x021830E8, 0x021830F8):
        def _hit(addr, size, ad=ad):
            res["entry_hits"][hex(ad)] = res["entry_hits"].get(hex(ad), 0) + 1
        emu.memory.register_exec(ad, _hit)
    emu.memory.register_exec(0x0213F50C, on_msg)
    emu.memory.register_exec(EXP_PASS, on_exp_pass)

    def press(keys):
        for k in keys:
            emu.input.keypad_update(keymask(getattr(Keys, "KEY_" + k)))
            for _ in range(4): emu.cycle(with_joystick=False)
            emu.input.keypad_update(0)
            for _ in range(20): emu.cycle(with_joystick=False)

    # One step per 120-frame tick. On screen (#642): A on a move opens Forget/Delink/Info, A on Forget asks
    # "will be forgotten. Is that OK?", and A there picks No (back to the list), so UP selects Yes first.
    confirm = [["A"], ["A"], ["UP", "A"]]
    menu_seq = {"a": confirm, "b": [["B"]] + confirm, "new": [["DOWN"] * 4] + confirm}[a.menu_key]
    turns = parse_inputs(a.inputs)
    for f in range(a.frames):
        if state.get("prompt_open") and not state["menu_open"]:
            state["prompt_frames"] += 1
            if state["prompt_frames"] % 60 == 1:
                press(["A"])
            if state["prompt_frames"] > 1800:  # the prompt never closed: stop pressing
                state["prompt_open"] = False
            emu.cycle(with_joystick=False)
            continue
        if state["menu_open"]:
            state["menu_frames"] += 1
            if state["menu_frames"] % 120 == 1:
                press(menu_seq[(state["menu_frames"] // 120) % len(menu_seq)])
            if state["menu_frames"] > 3000:  # menu never returned: stop driving it
                state["menu_open"] = False
            emu.cycle(with_joystick=False)
            continue
        if f in turns:
            emu.input.keypad_update(turns[f])
        emu.cycle(with_joystick=False)

    # Cross-check: the learnset line is the species the hero is, from the record dump (no table values stored).
    res["learnset_levels"] = sorted({lv for _, lv in learnset}) if learnset else []
    species_seen = sorted({c["pre"]["species"] for c in res["loop_calls"]})
    checks = []
    for c in res["loop_calls"]:
        pre, post = c["pre"], c.get("post")
        if not post or post["level"] <= pre["level"]:
            continue
        recs = levels.get(pre["species"])
        gained = list(range(pre["level"] + 1, post["level"] + 1))
        if recs is None or post["level"] > len(recs):  # no record dump for this species: flag, do not compare
            c["check"] = {"gained": gained, "error": "no record", "match": False}
            checks.append(False)
            continue
        rows = [{"level": L, "record_index": L - 1, "exp_threshold": recs[L - 1][0]} for L in gained]
        # Per call: sums over the gained records (exact for one level per call).
        sums = [sum(recs[L - 1][k] for L in gained) for k in range(6)]
        hp_gain, gains = sums[1], sums[2:6]
        d_max = post["maxhp"] - pre["maxhp"]
        exp_max = min(pre["maxhp"] + hp_gain, HP_CAP) - pre["maxhp"]
        d_hp = post["hp"] - pre["hp"]
        exp_hp = min(pre["hp"] + hp_gain, post["maxhp"]) - pre["hp"]
        stat_exp = [min(pre["stats"][i] + gains[i], STAT_CAP) - pre["stats"][i] for i in range(4)]
        stat_got = [post["stats"][i] - pre["stats"][i] for i in range(4)]
        # the level reached is the highest L with threshold <= EXP total (threshold = record L-1's EXP)
        want_level = max([pre["level"]] + [L for L in range(1, min(100, len(recs)) + 1)
                                           if recs[L - 1][0] <= pre["exp"]])
        c["check"] = {
            "gained": gained, "records": rows, "level_expected": want_level, "level_got": post["level"],
            "maxhp_delta": [d_max, exp_max], "hp_delta": [d_hp, exp_hp],
            "stat_delta": [stat_got, stat_exp],
            "match": (d_max == exp_max and d_hp == exp_hp and stat_got == stat_exp and want_level == post["level"]),
        }
        checks.append(c["check"]["match"])
    summary = {"loop_calls": len(res["loop_calls"]), "level_gaining_calls": len(checks),
               "all_match": bool(checks) and all(checks), "learn_calls": len(res["learn_calls"]),
               "remove_calls": len(res["remove_calls"]),
               "remove_draws": [c.get("draws") for c in res["remove_calls"]],
               "forget_calls": len(res["forget_calls"]),
               "forget_returns": [c.get("returned") for c in res["forget_calls"]],
               "exp_pass": res["exp_pass"], "species_seen": species_seen}
    res["summary"] = summary
    with open(a.out, "w") as f:
        json.dump(res, f, indent=1)  # full detail stays in the scratch file, never in the repo
    print(json.dumps(summary), flush=True)
    emu.destroy()


if __name__ == "__main__":
    import romarg; romarg.fill()
    main()
