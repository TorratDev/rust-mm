#!/usr/bin/env python3
"""Read 32-bit words at a RAM address of the ARM9 or an overlay, from the extracted code.

Usage:
    python3 tools/ov_read.py ov_NNNN::ADDRESS [COUNT]
    python3 tools/ov_read.py arm9::ADDRESS [COUNT]
    python3 tools/ov_read.py ADDRESS [COUNT]          (only if one space holds ADDRESS)
    options: --code DIR   extracted code (default: $RESCUE_CODE or extracted/code)

ADDRESS is a RAM address as Ghidra prints it (0214C0A4, 0x0214C0A4). Words are read
little-endian, the way the ARM9 reads them. COUNT defaults to 1.

This reads the files tools/nds_extract_code.py wrote from your own dump, and changes
nothing. Its output is game data: do not commit it (docs/legal.md). Standard library only.
"""

import argparse
import json
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(HERE)
DEFAULT_CODE = os.path.join(REPO_ROOT, "extracted", "code")
MANIFEST_FORMAT = "rescue-nds-code-manifest"  # tools/nds_extract_code.py
ADDR_RE = re.compile(r"^(?:([A-Za-z0-9_]+)::)?(?:0x)?([0-9A-Fa-f]{1,8})$")


class ReadError(Exception):
    pass


def load_spaces(code_dir):
    """Every RAM range that has bytes in an extracted file: (label, start, size, file, offset)."""
    try:
        with open(os.path.join(code_dir, "manifest.json"), encoding="utf-8") as f:
            manifest = json.load(f)
    except OSError as e:
        raise ReadError(f"cannot read manifest.json in {code_dir}: {e}; run tools/nds_extract_code.py first")
    if manifest.get("format") != MANIFEST_FORMAT:
        raise ReadError(f"{code_dir}/manifest.json is not a {MANIFEST_FORMAT} file")
    a9 = manifest["arm9"]
    spaces = [("arm9", int(a9["ram_address"], 16), a9["size"], a9["file"], 0)]
    for al in a9.get("autoloads", []):
        spaces.append((f"arm9-autoload-{al['index']}", int(al["ram_address"], 16), al["size"],
                       a9["file"], al["file_offset"]))
    for ov in manifest["overlays"]:
        label = f"ov_{ov['id']:04d}"
        spaces.append((label, int(ov["ram_address"], 16), ov["size"], ov["file"], 0))
    return code_dir, spaces


def read_words(code_dir, spaces, spec, count):
    m = ADDR_RE.match(spec.strip())
    if not m or count < 1:
        raise ReadError(f"bad address or count: {spec!r} {count}")
    prefix, hexpart = m.group(1), m.group(2)
    addr = int(hexpart, 16)
    if prefix is not None:
        cands = [s for s in spaces if s[0] == prefix]
        if not cands:
            raise ReadError(f"no space named {prefix}; use ov_NNNN, arm9 or arm9-autoload-N")
    else:
        cands = [s for s in spaces if s[1] <= addr < s[1] + s[2]]
        if len(cands) > 1:
            names = ", ".join(s[0] for s in cands)
            raise ReadError(f"{hexpart} lies in several spaces ({names}); name the one you mean "
                            f"as ov_NNNN::{hexpart} or arm9::{hexpart}")
        if not cands:
            raise ReadError(f"{hexpart} is in no extracted space (BSS, or outside the program)")
    label, start, size, fname, foff = cands[0]
    if addr < start:
        raise ReadError(f"{label}::{addr:08X} is below {label}'s start {start:08X}")
    if addr + 4 * count > start + size:
        raise ReadError(f"{count} word(s) at {label}::{addr:08X} run past the end of {label} "
                        f"(0x{start:08X}..0x{start + size:08X}); the rest is BSS or not in the file")
    with open(os.path.join(code_dir, fname), "rb") as f:
        f.seek(foff + addr - start)
        data = f.read(4 * count)
    if len(data) != 4 * count:
        raise ReadError(f"{fname} is shorter than its manifest says")
    return [(label, addr + 4 * i, struct.unpack_from("<I", data, 4 * i)[0]) for i in range(count)]


def main(argv=None):
    ap = argparse.ArgumentParser(description="Read words from the extracted ARM9 and overlays.")
    ap.add_argument("address", help="ov_NNNN::ADDRESS, arm9::ADDRESS or ADDRESS")
    ap.add_argument("count", nargs="?", type=int, default=1)
    ap.add_argument("--code", default=os.environ.get("RESCUE_CODE", DEFAULT_CODE))
    args = ap.parse_args(argv)
    try:
        code_dir, spaces = load_spaces(args.code)
        for label, addr, word in read_words(code_dir, spaces, args.address, args.count):
            print(f"{label}::{addr:08X} = 0x{word:08X}")
    except ReadError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
