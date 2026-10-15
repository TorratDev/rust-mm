#!/usr/bin/env python3
"""Render a code-size treemap of the repo to docs/progress/treemap.svg.

Each package (crates/*, apps/*, tools/*, ...) is a block sized by its non-blank
source lines; each source file is a cell inside it. Nothing is hand-maintained.
Run: python scripts/progress_treemap.py
"""
import html
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "progress" / "treemap.svg"
SCAN = ["crates", "apps", "tools", "verification", "tests", "fuzz", "rescue-re"]
EXTS = {".rs", ".py"}
SKIP_DIRS = {"target", "node_modules", "__pycache__"}

W, H = 1200, 760
HEAD = 70
FOOT = 34
PAD = 3
TITLE_H = 22

COLORS = {
    "crates": "#3fb950",
    "apps": "#58a6ff",
    "tools": "#d29922",
    "verification": "#a371f7",
    "tests": "#8b949e",
    "fuzz": "#8b949e",
    "rescue-re": "#8b949e",
}


def squarify(items, x, y, w, h):
    """items: list of (area, payload) sorted by area desc, areas sum to w*h.
    Returns list of (payload, (x, y, w, h)) using the squarified algorithm."""
    out = []
    items = list(items)

    def worst(row, side):
        s = sum(a for a, _ in row)
        return max(max(side * side * a / (s * s), (s * s) / (side * side * a)) for a, _ in row)

    while items:
        side = min(w, h)
        row = [items.pop(0)]
        while items and worst(row + [items[0]], side) <= worst(row, side):
            row.append(items.pop(0))
        s = sum(a for a, _ in row)
        if w >= h:
            rw = s / h
            yy = y
            for a, p in row:
                rh = a / rw
                out.append((p, (x, yy, rw, rh)))
                yy += rh
            x += rw
            w -= rw
        else:
            rh = s / w
            xx = x
            for a, p in row:
                rw = a / rh
                out.append((p, (xx, y, rw, rh)))
                xx += rw
            y += rh
            h -= rh
    return out


def truncate(text, width_px, size=11):
    max_chars = int(width_px / (size * 0.56))
    return text if len(text) <= max_chars else text[: max(0, max_chars - 1)] + "…"


def collect():
    """Return {package: [(rel_path, loc), ...]} for source files under SCAN dirs."""
    packages = {}
    for top in SCAN:
        base = ROOT / top
        if not base.is_dir():
            continue
        for dirpath, dirnames, filenames in os.walk(base):
            dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
            for name in filenames:
                path = Path(dirpath) / name
                if path.suffix not in EXTS:
                    continue
                rel = path.relative_to(ROOT).as_posix()
                loc = sum(1 for line in path.read_text(encoding="utf-8", errors="replace").splitlines() if line.strip())
                package = "/".join(rel.split("/")[:2]) if rel.count("/") >= 2 else top
                packages.setdefault(package, []).append((rel, loc))
    return packages


def main():
    packages = collect()
    files = [(rel, loc, pkg) for pkg, items in packages.items() for rel, loc in items if loc > 0]
    total_loc = sum(loc for _, loc, _ in files)

    body_h = H - HEAD - FOOT
    pkg_loc = {pkg: sum(loc for _, loc in items) for pkg, items in packages.items()}
    pkg_order = sorted((p for p in pkg_loc if pkg_loc[p] > 0), key=lambda p: -pkg_loc[p])
    pkg_areas = [(pkg_loc[p] / total_loc * W * body_h, p) for p in pkg_order]
    pkg_rects = squarify(pkg_areas, 0, HEAD, W, body_h)

    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
        'font-family="Segoe UI, Helvetica, Arial, sans-serif">',
        f'<rect width="{W}" height="{H}" fill="#0d1117"/>',
        '<text x="20" y="34" fill="#e6edf3" font-size="24" font-weight="600">Rust port · code map</text>',
        f'<text x="{W - 20}" y="34" fill="#e6edf3" font-size="24" font-weight="600" text-anchor="end">'
        f'{total_loc:,} lines</text>',
        f'<text x="20" y="58" fill="#8b949e" font-size="13">{len(files)} source files · '
        f'{len(pkg_order)} packages · non-blank .rs and .py lines</text>',
    ]

    for pkg, (x, y, w, h) in pkg_rects:
        x0, y0, w0, h0 = x + PAD, y + PAD, w - 2 * PAD, h - 2 * PAD
        top = pkg.split("/")[0]
        color = COLORS.get(top, "#8b949e")
        svg.append(f'<rect x="{x0:.1f}" y="{y0:.1f}" width="{w0:.1f}" height="{h0:.1f}" fill="#161b22" stroke="#30363d"/>')
        if h0 > TITLE_H + 10 and w0 > 80:
            label = f"{pkg}  ·  {pkg_loc[pkg]:,}"
            svg.append(
                f'<text x="{x0 + 8:.1f}" y="{y0 + 16:.1f}" fill="#c9d1d9" font-size="13" font-weight="600">'
                f'{html.escape(truncate(label, w0 - 16, 13))}</text>'
            )
        ix, iy, iw, ih = x0 + PAD, y0 + TITLE_H, w0 - 2 * PAD, h0 - TITLE_H - PAD
        if iw <= 0 or ih <= 0:
            continue
        pfiles = sorted(((rel, loc) for rel, loc in packages[pkg] if loc > 0), key=lambda f: -f[1])
        if not pfiles:
            continue
        pl = pkg_loc[pkg]
        cells = squarify([(loc / pl * iw * ih, (rel, loc)) for rel, loc in pfiles], ix, iy, iw, ih)
        for (rel, loc), (cx, cy, cw, ch) in cells:
            cx, cy, cw, ch = cx + PAD / 2, cy + PAD / 2, cw - PAD, ch - PAD
            name = rel.split("/")[-1]
            svg.append(
                f'<g><title>{html.escape(rel)} — {loc:,} lines</title>'
                f'<rect x="{cx:.1f}" y="{cy:.1f}" width="{cw:.1f}" height="{ch:.1f}" rx="3" fill="{color}" fill-opacity="0.8"/>'
            )
            if cw > 50 and ch > 18:
                svg.append(
                    f'<text x="{cx + 5:.1f}" y="{cy + 14:.1f}" fill="#0d1117" font-size="11">'
                    f'{html.escape(truncate(name, cw - 10))}</text>'
                )
            svg.append("</g>")

    legend_y = H - 12
    svg.append(
        f'<text x="20" y="{legend_y}" fill="#8b949e" font-size="12">'
        "Block = package, cell = source file, area = non-blank lines. Hover for path and count.</text>"
    )
    svg.append("</svg>")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(svg) + "\n", encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(files)} files, {total_loc:,} lines")


if __name__ == "__main__":
    main()
