#!/usr/bin/env python3
"""Survey the file system of an NDS ROM (Phase 0 check V4).

Lists every file in the ROM's FNT/FAT, plus the ARM9/ARM7 binaries and their
overlays, with path, size, magic bytes, Shannon entropy and a compression guess.
Prints a Markdown report to stdout. The report holds metadata only, no asset data.

Usage: python3 tools/nds_survey.py path/to/rom.nds > docs/file-survey.md
"""

import collections
import math
import struct
import sys

# Magic prefixes -> (format name, tag). Tag is "known" when the format is identified
# from its header alone, "hypothesis" when it is a guess from a naming convention.
MAGICS = [
    (b"SDAT", "Nintendo SDAT sound archive", "known"),
    (b"SSEQ", "Nintendo SSEQ sequence", "known"),
    (b"SBNK", "Nintendo SBNK instrument bank", "known"),
    (b"SWAR", "Nintendo SWAR wave archive", "known"),
    (b"STRM", "Nintendo STRM stream", "known"),
    (b"SWAV", "Nintendo SWAV wave", "known"),
    (b"RIFF", "RIFF container", "known"),
    (b"NARC", "NARC archive", "known"),
    (b"RAHC", "NCGR/character tile data", "known"),
    (b"RLCN", "NCLR palette", "known"),
    (b"RNCN", "NCGR character data", "known"),
    (b"NCLR", "NCLR palette", "known"),
    (b"NCGR", "NCGR character data", "known"),
    (b"NSCR", "NSCR screen data", "known"),
    (b"NCER", "NCER cell data", "known"),
    (b"NANR", "NANR animation data", "known"),
    (b"PKDPX", "PKDPX compressed container", "hypothesis"),
    (b"AT4PX", "AT4PX compressed container", "hypothesis"),
    (b"BMD0", "BMD0 model", "known"),
    (b"BCA0", "BCA0 animation", "known"),
    (b"BTX0", "BTX0 texture", "known"),
]

# Nintendo compression type, from the first byte of a compressed blob's header.
COMPRESSION = {
    0x10: "LZ77 (LZ10)",
    0x11: "LZ77 (LZ11)",
    0x20: "Huffman 4-bit",
    0x28: "Huffman 8-bit",
    0x30: "RLE",
}


def u16(data, off):
    return struct.unpack_from("<H", data, off)[0]


def u32(data, off):
    return struct.unpack_from("<I", data, off)[0]


def entropy(data):
    if not data:
        return 0.0
    counts = collections.Counter(data)
    total = len(data)
    return -sum(c / total * math.log2(c / total) for c in counts.values())


def classify(data):
    head = data[:8]
    for magic, name, tag in MAGICS:
        if head.startswith(magic):
            return name, tag
    if data and data[0] in COMPRESSION and len(data) >= 4:
        size = data[1] | (data[2] << 8) | (data[3] << 16)
        if size:
            return COMPRESSION[data[0]] + " (header guess)", "hypothesis"
    return "", "unknown"


def game_title(rom):
    """Game title and code from the NDS header, e.g. 'TITLE (CODE)'."""
    title = rom[0x00:0x0C].rstrip(b"\0").decode("ascii", "replace")
    code = rom[0x0C:0x10].decode("ascii", "replace")
    return f"{title} ({code})"


def walk_fnt(rom, fnt_off):
    """Return {file_id: path} by walking the File Name Table."""
    names = {}
    dir_stack = [(0xF000, "")]
    seen = set()
    while dir_stack:
        dir_id, prefix = dir_stack.pop()
        if dir_id in seen:
            continue
        seen.add(dir_id)
        entry = fnt_off + (dir_id & 0xFFF) * 8
        sub_off = u32(rom, entry)
        file_id = u16(rom, entry + 4)
        pos = fnt_off + sub_off
        while True:
            length = rom[pos]
            pos += 1
            if length == 0:
                break
            is_dir = bool(length & 0x80)
            name_len = length & 0x7F
            name = rom[pos:pos + name_len].decode("latin-1")
            pos += name_len
            path = f"{prefix}/{name}" if prefix else name
            if is_dir:
                sub_id = u16(rom, pos)
                pos += 2
                dir_stack.append((sub_id, path))
            else:
                names[file_id] = path
                file_id += 1
    return names


def main(rom_path):
    with open(rom_path, "rb") as f:
        rom = f.read()

    fnt_off, fnt_size = u32(rom, 0x40), u32(rom, 0x44)
    fat_off, fat_size = u32(rom, 0x48), u32(rom, 0x4C)
    names = walk_fnt(rom, fnt_off)
    fat_count = fat_size // 8

    rows = []  # (path, size, magic, entropy, format, tag)
    overlay_fids = set()

    def add(path, data, fmt=None, tag=None):
        name, auto_tag = classify(data)
        rows.append((path, len(data), data[:4].hex(" "), entropy(data),
                     fmt or name, tag or auto_tag))

    arm9_off, arm9_size = u32(rom, 0x20), u32(rom, 0x2C)
    arm7_off, arm7_size = u32(rom, 0x30), u32(rom, 0x3C)
    add("[arm9.bin]", rom[arm9_off:arm9_off + arm9_size])
    add("[arm7.bin]", rom[arm7_off:arm7_off + arm7_size])

    for label, table_off_field, table_size_field in (
        ("arm9 overlay", 0x50, 0x54),
        ("arm7 overlay", 0x58, 0x5C),
    ):
        table_off, table_size = u32(rom, table_off_field), u32(rom, table_size_field)
        for i in range(table_size // 32):
            ent = table_off + i * 32
            ov_id = u32(rom, ent)
            ov_file = u32(rom, ent + 24)
            flags = u32(rom, ent + 28)
            if ov_file >= fat_count:
                continue
            overlay_fids.add(ov_file)
            # The overlay's bytes live in the FAT slot for the file id it names.
            start, end = struct.unpack_from("<II", rom, fat_off + ov_file * 8)
            # Bit 24 of the flags word marks a BLZ-compressed overlay.
            if flags & 0x01000000:
                add(f"[{label} {ov_id}]", rom[start:end],
                    "BLZ compressed (overlay flag set)", "hypothesis")
            else:
                add(f"[{label} {ov_id}]", rom[start:end])

    for fid in range(fat_count):
        if fid in overlay_fids:
            continue  # already listed above from the overlay tables
        start, end = struct.unpack_from("<II", rom, fat_off + fid * 8)
        path = names.get(fid, f"[unnamed fat {fid}]")
        add(path, rom[start:end])

    print(f"# {game_title(rom)} file survey")
    print()
    print("Generated by `tools/nds_survey.py`. Metadata only: no asset data.")
    print()
    print(f"- ROM size: {len(rom)} bytes")
    print(f"- FAT entries: {fat_count}; overlays (listed from overlay tables): {len(overlay_fids)}; named files: {len(names)}")
    print("- Entropy is Shannon entropy in bits per byte (max 8.0). Values above ~7.5 suggest compressed or encrypted data.")
    print()
    tags = collections.Counter(r[5] for r in rows)
    print("## Summary")
    print()
    for tag in ("known", "hypothesis", "unknown"):
        print(f"- {tag}: {tags.get(tag, 0)}")
    print(f"- total entries: {len(rows)}")
    print()
    print("## Files")
    print()
    print("| Path | Size | First 4 bytes | Entropy | Format guess | Tag |")
    print("|------|------|---------------|---------|--------------|-----|")
    for path, size, magic, ent, fmt, tag in rows:
        print(f"| `{path}` | {size} | `{magic}` | {ent:.2f} | {fmt} | {tag} |")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
