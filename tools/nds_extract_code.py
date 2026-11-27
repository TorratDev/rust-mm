#!/usr/bin/env python3
"""Extract the ARM9 code of an NDS dump for Ghidra (Phase 0 check V8, issue #42).

Reads the user's own dump, checks it against the supported list, and writes to an
output directory:

    arm9.bin            ARM9 binary, BLZ-decoded if the ROM stores it compressed
    overlay_NNNN.bin    every ARM9 overlay, BLZ-decoded where flag bit 24 is set
    arm7.bin            ARM7 binary (when the ROM has one), laid out the same way
    overlay7_NNNN.bin   every ARM7 overlay (when the ROM has any)
    manifest.json       addresses, sizes, BSS, autoload (ITCM/DTCM) sections, static
                        initialiser ranges, entry point, overlay ids and the ROM SHA-1

`tools/ghidra/RescueNdsImport.java` reads manifest.json and maps everything at its
real address. See docs/ghidra-setup.md.

The output holds game code: never commit it (docs/legal.md). The output directory
must be outside the repository or in a git-ignored folder (default: extracted/code,
which .gitignore covers). The tool refuses anything else.

Python 3.8+, standard library only. Works on Windows and Linux.

Usage:
    python3 tools/nds_extract_code.py ROM [-o OUT_DIR] [--allow-unknown] [--force]

ROM may be omitted when RESCUE_ROM is set.

Format notes (our own words; sources: GBATEK, ndspy docs, see docs/references.md):

* Header: ARM9 ROM offset/entry/RAM address/size at 0x20..0x2C, FAT at 0x48,
  ARM9 overlay table ("y9") at 0x50, ARM7 overlay table at 0x58.
* Overlay table entry, 32 bytes: id, RAM address, RAM size, BSS size, static
  initialiser start, static initialiser end, file id, flags. Flags bits 0-23 hold
  the compressed size, bit 24 marks BLZ compression.
* Nitro "module params": a 36-byte block inside the ARM9 binary holding, in order,
  autoload list start, autoload list end, autoload data start, static BSS start,
  static BSS end, compressed static end, SDK version, and the magic pair
  0xDEC00621 0x2106C0DE (bytes 21 06 C0 DE DE C0 06 21). The ROM usually stores a
  12-byte footer right after the ARM9 binary: 0xDEC00621, then (we believe) the
  offset of the module params in the ARM9 binary, then one more word. We only trust
  the offset when the magic pair is found there.
  We use the footer when present and fall back to searching for the magic pair.
* ARM7 module params: the same first six words, without the magic pair. Header word
  0x74 is the RAM address just past a word holding their address (0x70 is the same
  for the ARM9). We trust them only when the autoload list lies inside arm7.bin.
* If "compressed static end" is non-zero, the ARM9 bytes from its start up to
  that RAM address are one BLZ stream (the SDK's crt0 decodes it in place at boot).
  We decode it, keep any bytes after it as they are, and record the original value
  in the manifest. The field itself is left unchanged in arm9.bin.
* Autoload list: 12-byte entries (destination RAM address, size, BSS size). Their
  data follows the ARM9 static area back to back, starting at "autoload data start".
  The crt0 copies them to ITCM / DTCM / other RAM at boot, so in Ghidra they belong
  at their destination, not where they sit in arm9.bin.
* BLZ ("backwards LZ"): the last 8 bytes are a footer. Byte -5 is the footer
  length (8 plus padding), bytes -8..-6 are the length of the compressed part
  counted from the end, and the last word is how many bytes decoding adds. The
  compressed part is read from its end towards its start: a flag byte (MSB first),
  then for each flag either a literal byte or a two-byte reference (high byte first):
  length = (v >> 12) + 3, distance = (v & 0xFFF) + 3, copying from already written
  output above the write position. Bytes before the compressed part are kept.
"""

import argparse
import hashlib
import json
import os
import struct
import subprocess
import sys

# Keep in sync with docs/supported-dumps.md and SUPPORTED_DUMPS in crates/nds/src/rom.rs.
SUPPORTED_DUMPS = {
    "6f01887356268de39085df01490347bbcc3aa330": ("BlueRescueTeamEur", 33_554_432),
}

ROM_ENV = "RESCUE_ROM"
MANIFEST_FORMAT = "rescue-nds-code-manifest"
MANIFEST_VERSION = 1
GHIDRA_LANGUAGE = "ARM:LE:32:v5t"  # ARM946E-S is ARMv5TE; Ghidra's closest id is v5t

# Nitro magic pair, read as little-endian words: bytes 21 06 C0 DE, then DE C0 06 21.
NITROCODE = 0xDEC00621
NITROCODE_REV = 0x2106C0DE
MODULE_PARAMS_SIZE = 0x24
BLZ_FLAG = 1 << 24
SECURE_AREA_DECRYPTED = b"\xff\xde\xff\xe7\xff\xde\xff\xe7"

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_OUT = os.path.join(REPO_ROOT, "extracted", "code")


class ExtractError(Exception):
    """A problem with the input that stops extraction. Printed without a traceback."""


# --------------------------------------------------------------------------- helpers


def u32(data, off, what="value"):
    if off < 0 or off + 4 > len(data):
        raise ExtractError(f"{what}: offset 0x{off:X} is outside the data (size 0x{len(data):X})")
    return struct.unpack_from("<I", data, off)[0]


def hx(v):
    return f"0x{v:08X}"


def sha1(data):
    return hashlib.sha1(data).hexdigest()


def slice_checked(data, start, size, what):
    if start < 0 or size < 0 or start + size > len(data):
        raise ExtractError(
            f"{what}: range 0x{start:X}+0x{size:X} is outside the data (size 0x{len(data):X})")
    return data[start:start + size]


# ------------------------------------------------------------------------------ BLZ


def blz_decompress(data, what="BLZ data"):
    """Decode a BLZ ("backwards LZ") blob. Returns bytes. Raises ExtractError."""
    n = len(data)
    if n < 8:
        raise ExtractError(f"{what}: {n} bytes, too short for a BLZ footer")
    word = u32(data, n - 8, what)
    extra = u32(data, n - 4, what)
    header_len = word >> 24
    comp_len = word & 0xFFFFFF
    if extra == 0:
        return bytes(data)  # stored uncompressed
    if header_len < 8 or header_len > comp_len or comp_len > n:
        raise ExtractError(
            f"{what}: bad BLZ footer (footer length {header_len}, compressed length "
            f"{comp_len}, file size {n})")
    end = n - comp_len  # bytes [0, end) are stored raw
    src = n - header_len
    out = bytearray(n + extra)
    out[:n] = data
    dst = len(out)
    while src > end:
        src -= 1
        flags = data[src]
        for _ in range(8):
            if src <= end:
                break
            if flags & 0x80:
                if src - 2 < end:
                    raise ExtractError(f"{what}: reference runs past the start of the stream")
                v = (data[src - 1] << 8) | data[src - 2]
                src -= 2
                length = (v >> 12) + 3
                disp = (v & 0xFFF) + 3
                if dst - length < end or dst + disp - 1 >= len(out):
                    raise ExtractError(f"{what}: reference out of bounds (corrupt BLZ data)")
                for _ in range(length):
                    dst -= 1
                    out[dst] = out[dst + disp]
            else:
                if dst - 1 < end:
                    raise ExtractError(f"{what}: output underflow (corrupt BLZ data)")
                src -= 1
                dst -= 1
                out[dst] = data[src]
            flags = (flags << 1) & 0xFF
    if dst != end:
        raise ExtractError(
            f"{what}: decoded size mismatch (stopped 0x{dst - end:X} bytes short); "
            "corrupt BLZ data")
    return bytes(out)


# ---------------------------------------------------------------------- ROM checks


def identify(rom, allow_unknown):
    digest = sha1(rom)
    known = SUPPORTED_DUMPS.get(digest)
    if known and known[1] == len(rom):
        return {"sha1": digest, "size": len(rom), "supported": True, "id": known[0]}
    supported = ", ".join(f"{i} ({s:,} bytes, SHA-1 {h})" for h, (i, s) in SUPPORTED_DUMPS.items())
    msg = (f"this file is not a supported dump: {len(rom):,} bytes, SHA-1 {digest}. "
           f"Supported: {supported}. See docs/supported-dumps.md.")
    if not allow_unknown:
        raise ExtractError(msg + " (Use --allow-unknown only for synthetic test ROMs.)")
    print("WARNING: --allow-unknown: " + msg, file=sys.stderr)
    print("WARNING: output is labelled UNSUPPORTED/SYNTHETIC in manifest.json.", file=sys.stderr)
    return {"sha1": digest, "size": len(rom), "supported": False, "id": None,
            "note": "UNSUPPORTED/SYNTHETIC: extracted with --allow-unknown, not a supported dump"}


def check_out_dir(out_dir, force):
    """Refuse output inside the repository unless git ignores it (docs/legal.md)."""
    out_abs = os.path.abspath(out_dir)
    root = os.path.abspath(REPO_ROOT)
    try:
        inside = os.path.commonpath([out_abs, root]) == root
    except ValueError:  # different drives on Windows
        inside = False
    if not inside:
        return
    rel = os.path.relpath(out_abs, root)
    try:
        # A trailing probe file name tests the directory as git would see its contents.
        probe = rel.replace(os.sep, "/") + "/arm9.bin"
        res = subprocess.run(["git", "-C", root, "check-ignore", "-q", probe],
                             capture_output=True)
        ignored = res.returncode == 0
    except OSError:
        ignored = False
    if not ignored and not force:
        raise ExtractError(
            f"output directory {out_abs} is inside the repository and not git-ignored. "
            "Extracted code must never be committed (docs/legal.md). Use a directory "
            "outside the repository, or the default extracted/code.")


# ------------------------------------------------------------------------- parsing


def find_module_params(rom, arm9_off, arm9_size, arm9, ram):
    """Return (offset in arm9, source) of the module params block, or (None, reason).

    Tries, in order: the post-ARM9 footer, the header word at 0x70 (the RAM address
    just past a word holding the module params address, as ndspy reads it), and a
    search for the magic pair. Each candidate must have the magic pair at +0x1C.
    """
    def valid(off):
        return (0 <= off and off % 4 == 0 and off + MODULE_PARAMS_SIZE <= len(arm9)
                and u32(arm9, off + 0x1C) == NITROCODE and u32(arm9, off + 0x20) == NITROCODE_REV)

    footer_off = arm9_off + arm9_size
    if footer_off + 12 <= len(rom) and u32(rom, footer_off) == NITROCODE:
        off = u32(rom, footer_off + 4)
        if valid(off):
            return off, "footer"
    hook = u32(rom, 0x70)
    if ram + 4 <= hook <= ram + len(arm9):
        off = u32(arm9, hook - ram - 4) - ram
        if valid(off):
            return off, "header 0x70"
    needle = struct.pack("<II", NITROCODE, NITROCODE_REV)
    hits = []
    pos = arm9.find(needle)
    while pos != -1:
        if pos % 4 == 0 and pos >= 0x1C:
            hits.append(pos - 0x1C)
        pos = arm9.find(needle, pos + 1)
    if len(hits) == 1:
        return hits[0], "magic search"
    if not hits:
        return None, "no nitrocode footer and no module params magic in ARM9"
    return None, f"{len(hits)} module params candidates; ambiguous"


def parse_module_params(arm9, off, ram):
    names = ("autoload_list_start", "autoload_list_end", "autoload_start",
             "static_bss_start", "static_bss_end", "compressed_static_end", "sdk_version")
    vals = struct.unpack_from("<7I", arm9, off)
    mp = dict(zip(names, vals))
    mp["ram_address"] = ram + off
    mp["arm9_offset"] = off
    return mp


def parse_autoloads(arm9, mp, ram, what="ARM9"):
    lst, lend, data_start = mp["autoload_list_start"], mp["autoload_list_end"], mp["autoload_start"]
    if lst == lend:
        return []
    if not (ram <= lst <= lend <= ram + len(arm9)) or (lend - lst) % 12:
        raise ExtractError(f"autoload list {hx(lst)}..{hx(lend)} is not inside the {what} binary")
    if not ram <= data_start <= ram + len(arm9):
        raise ExtractError(f"autoload data start {hx(data_start)} is not inside the {what} binary")
    out = []
    file_off = data_start - ram
    for i, ent in enumerate(range(lst - ram, lend - ram, 12)):
        dest, size, bss = struct.unpack_from("<3I", arm9, ent)
        if file_off + size > len(arm9):
            raise ExtractError(f"autoload {i} ({hx(dest)}, 0x{size:X} bytes) runs past the {what} binary")
        out.append({"index": i, "ram_address": hx(dest), "file_offset": file_off, "size": size,
                    "bss_address": hx(dest + size), "bss_size": bss,
                    "sha1": sha1(arm9[file_off:file_off + size])})
        file_off += size
    return out


def find_arm7_params(rom, arm7, ram):
    """Offset in arm7 of the ARM7 module params, or None (see the format notes)."""
    hook = u32(rom, 0x74)
    if not ram + 4 <= hook <= ram + len(arm7):
        return None
    off = u32(arm7, hook - ram - 4) - ram
    if not (0 <= off and off % 4 == 0 and off + 28 <= len(arm7)):
        return None
    lst, lend, data_start = struct.unpack_from("<3I", arm7, off)
    end = ram + len(arm7)
    if not (ram <= lst <= lend <= end and (lend - lst) % 12 == 0 and ram <= data_start <= end):
        return None
    return off


def lay_out(raw, ram, mp, what):
    """BLZ-decode the static part if needed; return (image, compressed, autoloads,
    static size, static BSS or None)."""
    data, compressed = raw, False
    if mp and mp["compressed_static_end"]:
        cse = mp["compressed_static_end"]
        if not ram < cse <= ram + len(raw):
            raise ExtractError(f"compressed static end {hx(cse)} is outside the {what} binary")
        split = cse - ram
        data = blz_decompress(raw[:split], f"{what} static (BLZ)") + raw[split:]
        compressed = True
    autoloads = parse_autoloads(data, mp, ram, what) if mp else []
    static_size = mp["autoload_start"] - ram if mp and autoloads else len(data)
    bss = None
    if mp and mp["static_bss_end"] > mp["static_bss_start"]:
        bss = {"ram_address": hx(mp["static_bss_start"]),
               "size": mp["static_bss_end"] - mp["static_bss_start"]}
    return data, compressed, autoloads, static_size, bss


def parse_overlays(rom, table_off, table_size, fat_off, fat_count, cpu, prefix, files, warnings):
    """Overlay table entries of one CPU; adds the decoded files to `files`."""
    if table_size % 32 or (table_size and table_off + table_size > len(rom)):
        raise ExtractError(f"{cpu} overlay table at 0x{table_off:X}+0x{table_size:X} is malformed "
                           "or outside the ROM")
    overlays = []
    seen_ids = set()
    for i in range(table_size // 32):
        ent = table_off + i * 32
        ov_id, ov_ram, ov_size, ov_bss, si_start, si_end, fid, flags = \
            struct.unpack_from("<8I", rom, ent)
        what = f"{cpu} overlay {ov_id} (table entry {i})"
        if ov_id in seen_ids:
            raise ExtractError(f"{what}: duplicate overlay id")
        seen_ids.add(ov_id)
        if fid >= fat_count:
            raise ExtractError(f"{what}: file id {fid} is outside the FAT ({fat_count} entries)")
        start, end = struct.unpack_from("<II", rom, fat_off + fid * 8)
        if not start <= end <= len(rom):
            raise ExtractError(f"{what}: FAT range 0x{start:X}..0x{end:X} is outside the ROM")
        raw = rom[start:end]
        is_comp = bool(flags & BLZ_FLAG)
        if is_comp:
            csize = flags & 0xFFFFFF
            if csize and csize != len(raw):
                warnings.append(f"{what}: flags give compressed size {csize}, FAT gives {len(raw)}; "
                                "using the FAT size")
            data = blz_decompress(raw, what)
        else:
            data = raw
        if len(data) != ov_size:
            warnings.append(f"{what}: decoded size {len(data)} differs from the table's RAM size "
                            f"{ov_size}; mapping the decoded size")
        name = f"{prefix}_{ov_id:04d}.bin"
        files[name] = data
        overlays.append({
            "id": ov_id, "file": name, "sha1": sha1(data), "size": len(data),
            "file_id": fid, "rom_offset": start, "rom_size": len(raw),
            "ram_address": hx(ov_ram), "ram_size": ov_size,
            "bss_address": hx(ov_ram + len(data)), "bss_size": ov_bss,
            "static_init": {"start": hx(si_start), "end": hx(si_end)},
            "flags": hx(flags), "compressed": is_comp,
        })
    return overlays


def extract(rom, allow_unknown=False):
    """Parse a ROM. Returns (files {name: bytes}, manifest dict, warnings list)."""
    info = identify(rom, allow_unknown)
    if len(rom) < 0x200:
        raise ExtractError(f"file is {len(rom)} bytes, too small for an NDS header")
    warnings = []

    title = rom[0:12].rstrip(b"\0").decode("ascii", "replace")
    code = rom[12:16].decode("ascii", "replace")
    arm9_off, entry, ram, arm9_size = struct.unpack_from("<4I", rom, 0x20)
    fat_off, fat_size = struct.unpack_from("<2I", rom, 0x48)
    ov9_off, ov9_size = struct.unpack_from("<2I", rom, 0x50)
    ov7_off, ov7_size = struct.unpack_from("<2I", rom, 0x58)

    arm9_raw = slice_checked(rom, arm9_off, arm9_size, "ARM9 binary")
    if not ram <= entry < ram + max(arm9_size, 1) and not 0x01FF8000 <= entry < 0x02000000:
        warnings.append(f"ARM9 entry {hx(entry)} is outside the ARM9 load range")
    if arm9_off == 0x4000 and arm9_raw[:8] != SECURE_AREA_DECRYPTED and arm9_raw[:8] != b"\0" * 8:
        warnings.append("ARM9 secure area (first 2 KiB) does not start with the decrypted marker; "
                        "it may be encrypted, so code in its first 0x800 bytes may be garbage")

    mp_off, mp_src = find_module_params(rom, arm9_off, arm9_size, arm9_raw, ram)
    mp = None
    if mp_off is None:
        warnings.append(f"module params not found ({mp_src}); mapping the whole ARM9 as static, "
                        "no BSS or autoload sections")
    else:
        mp = parse_module_params(arm9_raw, mp_off, ram)
        mp["found_by"] = mp_src
    arm9, compressed, autoloads, static_size, bss = lay_out(arm9_raw, ram, mp, "ARM9")
    # The module params block lies before the compressed part, so re-read it from the
    # decoded image to make sure it still checks out.
    if mp and u32(arm9, mp_off + 0x1C) != NITROCODE:
        raise ExtractError("module params magic missing after ARM9 decompression")

    files = {"arm9.bin": arm9}
    # ARM7 binary (header 0x30..0x3C), laid out like the ARM9: static part, BSS and
    # autoload sections from its module params (format notes).
    arm7_off, arm7_entry, arm7_ram, arm7_size = struct.unpack_from("<4I", rom, 0x30)
    arm7_raw = slice_checked(rom, arm7_off, arm7_size, "ARM7 binary")
    # No ARM7 binary (size 0) means no arm7.bin and "arm7": null in the manifest.
    arm7_entry_info = None
    if arm7_size:
        mp7_off = find_arm7_params(rom, arm7_raw, arm7_ram)
        mp7 = None
        if mp7_off is None:
            warnings.append("ARM7 module params not found (header 0x74); mapping the whole ARM7 "
                            "as static, no BSS or autoload sections")
        else:
            mp7 = parse_module_params(arm7_raw, mp7_off, arm7_ram)
            del mp7["sdk_version"], mp7["arm9_offset"]  # ARM7 params end after six words
            mp7["arm7_offset"] = mp7_off
            mp7["found_by"] = "header 0x74"
        arm7, compressed7, autoloads7, static7, bss7 = lay_out(arm7_raw, arm7_ram, mp7, "ARM7")
        files["arm7.bin"] = arm7
        arm7_entry_info = {
            "file": "arm7.bin", "sha1": sha1(arm7), "size": len(arm7),
            "rom_offset": arm7_off, "rom_size": arm7_size,
            "ram_address": hx(arm7_ram), "entry": hx(arm7_entry),
            "compressed": compressed7,
            "static": {"ram_address": hx(arm7_ram), "file_offset": 0, "size": static7},
            "bss": bss7, "autoloads": autoloads7,
            "module_params": None if mp7 is None else {
                k: (hx(v) if k not in ("arm7_offset", "found_by") else v) for k, v in mp7.items()},
        }
    arm9_entry = {
        "file": "arm9.bin", "sha1": sha1(arm9), "size": len(arm9),
        "rom_offset": arm9_off, "rom_size": arm9_size,
        "ram_address": hx(ram), "entry": hx(entry),
        "compressed": compressed,
        "static": {"ram_address": hx(ram), "file_offset": 0, "size": static_size},
        "bss": bss, "autoloads": autoloads,
        "module_params": None if mp is None else {
            k: (hx(v) if k not in ("arm9_offset", "found_by") else v) for k, v in mp.items()},
    }

    # FAT and ARM9 overlay table.
    if fat_size % 8 or fat_off + fat_size > len(rom):
        raise ExtractError(f"FAT at 0x{fat_off:X}+0x{fat_size:X} is malformed or outside the ROM")
    fat_count = fat_size // 8
    overlays = parse_overlays(rom, ov9_off, ov9_size, fat_off, fat_count, "ARM9", "overlay",
                              files, warnings)
    arm7_overlays = parse_overlays(rom, ov7_off, ov7_size, fat_off, fat_count, "ARM7", "overlay7",
                                   files, warnings)

    manifest = {
        "format": MANIFEST_FORMAT, "version": MANIFEST_VERSION,
        "generator": "tools/nds_extract_code.py",
        "warning": "Contains addresses of game code; the .bin files next to it are game code. "
                   "Never commit them (docs/legal.md).",
        "rom": dict(info, title=title, game_code=code),
        "processor": {"ghidra_language": GHIDRA_LANGUAGE, "compiler_spec": "default",
                      "cpu": "ARM946E-S (ARMv5TE)"},
        "arm9": arm9_entry,
        "arm7": arm7_entry_info,
        "overlays": overlays,
        "arm7_overlays": arm7_overlays,
        "arm7_overlay_count": ov7_size // 32,
        "warnings": warnings,
    }
    return files, manifest, warnings


# ---------------------------------------------------------------------------- main


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Extract ARM9 + overlays of a supported NDS dump for Ghidra (see docs/ghidra-setup.md).")
    ap.add_argument("rom", nargs="?", help=f"path to your dump (default: ${ROM_ENV})")
    ap.add_argument("-o", "--out", default=DEFAULT_OUT,
                    help="output directory (default: extracted/code in the repo, git-ignored)")
    ap.add_argument("--allow-unknown", action="store_true",
                    help="accept a ROM that is not on the supported list. ONLY for synthetic "
                         "test ROMs; the manifest is labelled UNSUPPORTED/SYNTHETIC")
    ap.add_argument("--force", action="store_true",
                    help="write into a non-ignored directory inside the repository (not recommended)")
    args = ap.parse_args(argv)

    rom_path = args.rom or os.environ.get(ROM_ENV)
    try:
        if not rom_path:
            raise ExtractError(f"no ROM given and ${ROM_ENV} is not set")
        try:
            with open(rom_path, "rb") as f:
                rom = f.read()
        except OSError as e:
            raise ExtractError(f"cannot read ROM {rom_path}: {e}") from None
        check_out_dir(args.out, args.force)
        files, manifest, warnings = extract(rom, args.allow_unknown)
        os.makedirs(args.out, exist_ok=True)
        for name, data in files.items():
            with open(os.path.join(args.out, name), "wb") as f:
                f.write(data)
        with open(os.path.join(args.out, "manifest.json"), "w", encoding="utf-8", newline="\n") as f:
            json.dump(manifest, f, indent=2)
            f.write("\n")
    except ExtractError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    for w in warnings:
        print(f"warning: {w}", file=sys.stderr)
    a9 = manifest["arm9"]
    print(f"ROM {manifest['rom']['sha1']} ({'supported: ' + manifest['rom']['id'] if manifest['rom']['supported'] else 'UNSUPPORTED/SYNTHETIC'})")
    print(f"arm9.bin: {a9['size']} bytes at {a9['ram_address']}, entry {a9['entry']}, "
          f"{'BLZ-decoded' if a9['compressed'] else 'not compressed'}, "
          f"{len(a9['autoloads'])} autoload section(s)")
    n_comp = sum(o["compressed"] for o in manifest["overlays"])
    print(f"overlays: {len(manifest['overlays'])} ({n_comp} BLZ-decoded)")
    a7 = manifest["arm7"]
    if a7:
        print(f"arm7.bin: {a7['size']} bytes at {a7['ram_address']}, entry {a7['entry']}, "
              f"{len(a7['autoloads'])} autoload section(s), "
              f"{len(manifest['arm7_overlays'])} ARM7 overlay(s)")
    print(f"wrote {len(files) + 1} files to {os.path.abspath(args.out)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
