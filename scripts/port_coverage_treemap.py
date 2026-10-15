#!/usr/bin/env python3
"""Render Blue function port coverage to docs/progress/port-coverage.svg.

Units are the functions in docs/symbols.md (rows whose Type is a C prototype).
A function counts as ported when its address appears in crates/sim/src, the
same way the port docs cite Blue addresses (`0214FD7C` or `ov_0001::0214FD7C`).
Groups are the ARM9 binary and each overlay. Cells are equal weight: function
byte sizes need the Ghidra project, which is not in the repo.
Run: python scripts/port_coverage_treemap.py
"""
import html
import re
from pathlib import Path

from progress_treemap import squarify, truncate

ROOT = Path(__file__).resolve().parent.parent
SYMBOLS = ROOT / "docs" / "symbols.md"
SIM_SRC = ROOT / "crates" / "sim" / "src"
OUT = ROOT / "docs" / "progress" / "port-coverage.svg"

W, H = 1200, 760
HEAD = 70
FOOT = 34
PAD = 3
TITLE_H = 22

ROW = re.compile(r"^\| (?:(ov_\d{4})::)?([0-9A-F]{8}) \| ([^|]+?) \| ([^|]+?) \| ([^|]+?) \|")
ADDR = re.compile(r"(?:ov_\d{4}::)?\b(0[0-9A-F]{7})\b")


def load_functions():
    funcs = []
    for line in SYMBOLS.read_text(encoding="utf-8").splitlines():
        m = ROW.match(line)
        if not m:
            continue
        overlay, addr, name, ctype, tag = m.groups()
        if "(" not in ctype:
            continue
        funcs.append({"group": overlay or "ARM9", "addr": addr, "name": name.strip(), "tag": tag.strip()})
    return funcs


def ported_addresses():
    text = " ".join(p.read_text(encoding="utf-8", errors="replace") for p in SIM_SRC.glob("*.rs"))
    return set(ADDR.findall(text))


def main():
    funcs = load_functions()
    ported = ported_addresses()
    for f in funcs:
        f["ported"] = f["addr"] in ported
    done = sum(f["ported"] for f in funcs)
    total = len(funcs)
    pct = done / total * 100 if total else 0

    groups = {}
    for f in funcs:
        groups.setdefault(f["group"], []).append(f)
    order = sorted(groups, key=lambda g: -len(groups[g]))

    body_h = H - HEAD - FOOT
    areas = [(len(groups[g]) / total * W * body_h, g) for g in order]
    rects = squarify(areas, 0, HEAD, W, body_h)

    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
        'font-family="Segoe UI, Helvetica, Arial, sans-serif">',
        f'<rect width="{W}" height="{H}" fill="#0d1117"/>',
        '<text x="20" y="34" fill="#e6edf3" font-size="24" font-weight="600">Blue functions ported</text>',
        f'<text x="{W - 20}" y="34" fill="#e6edf3" font-size="24" font-weight="600" text-anchor="end">{pct:.0f}%</text>',
        f'<text x="20" y="58" fill="#8b949e" font-size="13">{done} of {total} functions in docs/symbols.md '
        f'have a crates/sim reference · {len(groups)} groups (ARM9 + overlays)</text>',
    ]

    for g, (x, y, w, h) in rects:
        x0, y0, w0, h0 = x + PAD, y + PAD, w - 2 * PAD, h - 2 * PAD
        gdone = sum(f["ported"] for f in groups[g])
        svg.append(f'<rect x="{x0:.1f}" y="{y0:.1f}" width="{w0:.1f}" height="{h0:.1f}" fill="#161b22" stroke="#30363d"/>')
        if h0 > TITLE_H + 10 and w0 > 80:
            label = f"{g}  ·  {gdone}/{len(groups[g])}"
            svg.append(
                f'<text x="{x0 + 8:.1f}" y="{y0 + 16:.1f}" fill="#c9d1d9" font-size="13" font-weight="600">'
                f'{html.escape(truncate(label, w0 - 16, 13))}</text>'
            )
        ix, iy, iw, ih = x0 + PAD, y0 + TITLE_H, w0 - 2 * PAD, h0 - TITLE_H - PAD
        if iw <= 0 or ih <= 0:
            continue
        members = groups[g]
        cells = squarify([(iw * ih / len(members), f) for f in members], ix, iy, iw, ih)
        for f, (cx, cy, cw, ch) in cells:
            cx, cy, cw, ch = cx + PAD / 2, cy + PAD / 2, cw - PAD, ch - PAD
            color = "#3fb950" if f["ported"] else "#484f58"
            status = "ported" if f["ported"] else "not ported"
            svg.append(
                f'<g><title>{html.escape(f["group"])}::{f["addr"]} {html.escape(f["name"])} — {status} '
                f'({html.escape(f["tag"])})</title>'
                f'<rect x="{cx:.1f}" y="{cy:.1f}" width="{cw:.1f}" height="{ch:.1f}" rx="3" fill="{color}" fill-opacity="0.85"/>'
            )
            if cw > 50 and ch > 18:
                fill = "#0d1117" if f["ported"] else "#c9d1d9"
                svg.append(
                    f'<text x="{cx + 5:.1f}" y="{cy + 14:.1f}" fill="{fill}" font-size="11">'
                    f'{html.escape(truncate(f["name"], cw - 10))}</text>'
                )
            svg.append("</g>")

    svg.append(
        f'<text x="20" y="{H - 12}" fill="#8b949e" font-size="12">'
        "Green = referenced in crates/sim. Grey = not yet. Cells are equal weight (byte sizes need Ghidra).</text>"
    )
    svg.append("</svg>")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(svg) + "\n", encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)}: {done}/{total} ported ({pct:.0f}%)")


if __name__ == "__main__":
    main()
