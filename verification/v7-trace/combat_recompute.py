"""Recompute each captured CalcDamage hit from docs/re/combat.md (#185).

Reads OUT/hits.jsonl and OUT/tables.json written by combat_hits.py and recomputes every
hit with the formula in docs/re/combat.md (steps 1 to 17), in 48.16 fixed point as the
code does. Each hit is compared with the working values CalcDamage stored in gDungeon
(0x134..0x16C) and with the damage it wrote to dmg_struct.

Not modelled, because the capture does not record them: held items, abilities, status
effects, weather and the type-effectiveness combination (sub_806E100). The type multiplier
is an input (`--type-mult`, default 1.0). The script also reports the range of type
multipliers that every hit allows, so the default can be checked against it.

Exit status is 1 if any hit does not match.

Usage:
  combat_recompute.py OUT [--type-mult 1.0]
"""

import argparse
import json
import os
import struct
import sys
from fractions import Fraction

ONE = 1 << 16  # 48.16 fixed point
RNG_MUL, RNG_ADD = 0x5D588B65, 1
CRIT_MUL = Fraction(3, 2)  # gUnknown_8106F14, as documented in combat.md step 14
VARIANCE_BASE = 0xE000  # step 16
DEF_STAGE_OFFSET = 0x20  # defensive stages: assumed to follow the offensive ones; all 14 hits are at stage 10
INFO_ATK, INFO_DEF = 0x14, 0x16
INFO_OFF_MUL, INFO_DEF_MUL = 0x2C, 0x34
INFO_OFF_STAGE = 0x1C
INFO_LEVEL, INFO_NOT_TEAM = 0x09, 0x06
GD_BASE = 0x134  # damageCalcWorkingValues
GD_RAW0, GD_DEF, GD_R6 = 0x140, 0x142, 0x14A  # s16
GD_POST, GD_PRE, GD_ARG8 = 0x150, 0x154, 0x15C  # s32


def rng_step(state: int) -> tuple[int, int]:
    """One dungeon LCG step; returns (new state, 16-bit output)."""
    state = (state * RNG_MUL + RNG_ADD) & 0xFFFFFFFF
    return state, state >> 16


def rand_int(state: int, cap: int) -> tuple[int, int]:
    """DungeonRandInt(cap) from rng.md: ((rand16 & 0xFFFF) * cap) >> 16."""
    state, r = rng_step(state)
    return state, (r * cap) >> 16


def mul(a: int, b: int) -> int:
    """48.16 multiply, rounding half up on bit 15 (combat.md, Rounding)."""
    p = a * b
    return (p >> 16) + ((p >> 15) & 1)


def sdiv(a: int, b: int) -> int:
    """48.16 divide, truncating toward zero."""
    q = (abs(a) << 16) // abs(b)
    return q if (a < 0) == (b < 0) else -q


def to_s32(v: int) -> int:
    """FP48_16_ToS32: the integer part, plus 1 when bit 15 is set."""
    return (v >> 16) + ((v >> 15) & 1)


def s24_8_mul(a: int, b: int) -> int:
    """24.8 multiply, rounding half up on bit 7."""
    p = a * b
    return (p >> 8) + ((p >> 7) & 1)


def f248_to_int(v: int) -> int:
    """F248ToInt: truncates toward zero."""
    return v // 256 if v >= 0 else -((-v) // 256)


def s16(b: bytes, off: int) -> int:
    return struct.unpack_from("<h", b, off)[0]


def s32(b: bytes, off: int) -> int:
    return struct.unpack_from("<i", b, off)[0]


def clamp_stage(v: int) -> int:
    return min(max(v, 0), 20)


def recompute(hit: dict, tables: dict, type_mult: Fraction) -> dict:
    a = bytes.fromhex(hit["a_info"])
    t = bytes.fromhex(hit["t_info"])
    gd = bytes.fromhex(hit["gDungeon_0x134_0x16C"])
    dmg = bytes.fromhex(hit["dmg_struct"])
    atk_stage_mul = [struct.unpack_from("<i", bytes.fromhex(tables["atkStageMul"]), 4 * i)[0] for i in range(21)]
    def_stage_mul = [struct.unpack_from("<i", bytes.fromhex(tables["defStageMul"]), 4 * i)[0] for i in range(21)]
    physical = bytes.fromhex(tables["physicalByType"])

    # Step 1 to 3: the three RNG draws, in order.
    state = int.from_bytes(bytes.fromhex(hit["rng_before"]), "little")
    state, roll_move = rand_int(state, 100)
    state, roll_crit = rand_int(state, 100)
    state, roll_var = rand_int(state, 0x4000)
    rng_ok = state == int.from_bytes(bytes.fromhex(hit["rng_after"]), "little")

    # Step 5 to 6: base power and defence, with stage and multiplier, from the captured structs.
    split = 0 if physical[hit["moveType"]] == 1 else 1
    atk_stage = clamp_stage(s16(a, INFO_OFF_STAGE + 2 * split))
    def_stage = clamp_stage(s16(t, DEF_STAGE_OFFSET + 2 * split))
    atk_raw = a[INFO_ATK + split]
    def_raw = t[INFO_DEF + split]
    stat = s24_8_mul(s24_8_mul((atk_raw + hit["movePower"]) << 8, atk_stage_mul[atk_stage]), s32(a, INFO_OFF_MUL + 4 * split))
    atk_stat = f248_to_int(stat)
    stat = s24_8_mul(s24_8_mul(def_raw << 8, def_stage_mul[def_stage]), s32(t, INFO_DEF_MUL + 4 * split))
    def_stat = f248_to_int(stat)

    # Step 10: the level-or-attack term, in fixed point (not the truncated integer).
    level = a[INFO_LEVEL]
    if a[INFO_NOT_TEAM] == 0:
        r6_int = level * 2 // 3
        term = mul(level * ONE, 0xAAAA)
    else:
        r6_int = atk_raw // 3
        term = sdiv(atk_raw * ONE, 3 * ONE)

    # Steps 4 to 12: base damage x, then x*x*0xCCC + (2x - defence + 10), clamped to [1, 999].
    x = sdiv((atk_stat - def_stat) * ONE, 8 * ONE) + term
    base = mul(mul(x, x), 0xCCC) + mul(x, 2 * ONE) - def_stat * ONE + 10 * ONE
    base = min(max(base, ONE), 999 * ONE)

    # Step 14: critical hit. Odds are 0 for a non-team attacker. It scales the multiplier.
    odds = 0 if a[INFO_NOT_TEAM] else hit["critChance"]
    crit = roll_crit < odds
    mult = int(type_mult * ONE)
    if crit:
        mult = mul(mult, int(CRIT_MUL * ONE))

    # Step 15: the multiplier, then the arg8 scale. Step 16: variance. Step 17: integer conversion.
    pre = to_s32(base)  # stored before the multiplier is applied (Red: unk154)
    value = mul(base, mult)
    arg8 = hit["arg8"] << 8  # s24.8 to 48.16
    post_fp = mul(value, arg8)
    post = to_s32(post_fp)
    var = VARIANCE_BASE + roll_var
    dmg_pred = to_s32(mul(post_fp, var))

    checks = {
        "rng": rng_ok,
        "raw0": s16(gd, GD_RAW0 - GD_BASE) == atk_raw + hit["movePower"],
        "def": s16(gd, GD_DEF - GD_BASE) == def_raw,
        "r6": s16(gd, GD_R6 - GD_BASE) == r6_int,
        "pre": s32(gd, GD_PRE - GD_BASE) == pre,
        "post": s32(gd, GD_POST - GD_BASE) == post,
        "dmg": struct.unpack_from("<i", dmg, 0)[0] == dmg_pred,
        "crit": bool(dmg[0xD]) == crit,
    }
    return {
        "checks": checks,
        "pre": pre, "post": post, "dmg": dmg_pred,
        "pre_obs": s32(gd, GD_PRE - GD_BASE), "post_obs": s32(gd, GD_POST - GD_BASE),
        "dmg_obs": struct.unpack_from("<i", dmg, 0)[0],
        "base": base, "split": split, "atk": atk_stat, "def": def_stat, "r6": r6_int, "crit": crit,
        "atk_stage": atk_stage, "def_stage": def_stage,
        "arg8": hit["arg8"], "base_fp": base, "value_fp": value, "post_fp": post_fp, "var": var,
    }


def type_mult_range(res: dict) -> tuple[Fraction, Fraction]:
    """Type multipliers that reproduce this hit's observed post and dmg (rounding bounds)."""
    lo, hi = Fraction(0), Fraction(10)
    k = Fraction(res["base_fp"], ONE) * Fraction(res["arg8"], 256) * (Fraction(3, 2) if res["crit"] else 1)
    if k == 0:
        return lo, hi
    for obs, scale in ((res["post_obs"], 1), (res["dmg_obs"], Fraction(res["var"], ONE))):
        lo = max(lo, (obs - Fraction(1, 2)) / (k * scale))
        hi = min(hi, (obs + Fraction(1, 2)) / (k * scale))
    return lo, hi


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("out")
    p.add_argument("--type-mult", default="1.0", help="type multiplier input (sub_806E100 not modelled)")
    a = p.parse_args()
    hits = [json.loads(line) for line in open(os.path.join(a.out, "hits.jsonl"))]
    with open(os.path.join(a.out, "tables.json")) as f:
        tables = json.load(f)
    mult = Fraction(a.type_mult)

    bad = 0
    lo_all, hi_all = Fraction(0), Fraction(10)
    print("| # | Frame | Move | Type | Split | Atk | Def | r6 | Pre obs/calc | Post obs/calc | Dmg obs/calc | Checks |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for i, hit in enumerate(hits, 1):
        res = recompute(hit, tables, mult)
        failed = [k for k, ok in res["checks"].items() if not ok]
        bad += bool(failed)
        lo, hi = type_mult_range(res)
        lo_all, hi_all = max(lo_all, lo), min(hi_all, hi)
        status = "match" if not failed else "fail: " + ",".join(failed)
        print(
            f"| {i} | {hit['frame']} | {hit['moveId']} | {hit['moveType']} | {res['split']} | {res['atk']} | {res['def']} | {res['r6']} "
            f"| {res['pre_obs']}/{res['pre']} | {res['post_obs']}/{res['post']} | {res['dmg_obs']}/{res['dmg']} | {status} |"
        )
    print(f"\nhits: {len(hits)}, mismatched: {bad}")
    print(f"type multiplier allowed by all hits: [{float(lo_all):.4f}, {float(hi_all):.4f}) (input {float(mult):.4f})")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
