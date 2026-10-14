#!/usr/bin/env python3
"""Build a small synthetic NDS ROM for testing tools/nds_extract_code.py and the Ghidra
import script. Everything here is written from scratch: no retail code or data.

The ROM has:

* an ARM9 binary at 0x02000000 with a few real ARM instructions, a Nitro module
  params block, a static BSS range and two autoload sections (ITCM code at
  0x01FF8000, a DTCM section at 0x027E0000), optionally BLZ-compressed like a
  retail ARM9 ("compressed static end" set);
* three ARM9 overlays: 0 and 1 share the RAM range at 0x02100000 (1 is
  BLZ-compressed), 2 sits at 0x02110000 and is BLZ-compressed. Overlay code calls
  back into the ARM9 and loads pointers into its own data, so cross-references can
  be checked in Ghidra.

Usage: python3 tools/tests/synth_nds.py OUT.nds [--compress-arm9]
(Write OUT outside the repository or into an ignored folder; *.nds is git-ignored.)
"""

import struct
import sys

ARM9_RAM = 0x02000000
OV_RAM_A = 0x02100000
OV_RAM_B = 0x02110000
ITCM = 0x01FF8000
DTCM = 0x027E0000
ARM9_FUNC = ARM9_RAM + 0x20  # "bx lr" target called from the entry and the overlays
SINIT_OFF = 0x18  # overlay static initialiser table: one pointer

NITROCODE = 0xDEC00621  # bytes 21 06 C0 DE
NITROCODE_REV = 0x2106C0DE


# ------------------------------------------------------------------ BLZ encoder


def blz_compress(data, raw_prefix=0):
    """BLZ-encode data[raw_prefix:], keeping data[:raw_prefix] stored as is.

    Greedy encoder, written for tests: correctness over ratio. The decoder reads the
    compressed part from its end, so we work on the reversed data, where BLZ is plain
    LZ with distances 3..4098 and lengths 3..18.
    """
    prefix, body = bytes(data[:raw_prefix]), bytes(data[raw_prefix:])
    r = body[::-1]
    out = bytearray()  # the compressed stream, in reversed order
    i = 0
    n = len(r)
    while i < n:
        flag_pos = len(out)
        out.append(0)
        flags = 0
        for bit in range(8):
            if i >= n:
                break
            best_len, best_d = 0, 0
            for d in range(3, min(4098, i) + 1):
                ln = 0
                while ln < 18 and i + ln < n and r[i + ln] == r[i + ln - d]:
                    ln += 1
                if ln > best_len:
                    best_len, best_d = ln, d
                    if ln == 18:
                        break
            if best_len >= 3:
                v = ((best_len - 3) << 12) | (best_d - 3)
                out += bytes([v >> 8, v & 0xFF])
                flags |= 0x80 >> bit
                i += best_len
            else:
                out.append(r[i])
                i += 1
        out[flag_pos] = flags
    comp = bytes(out[::-1])
    pad = (-(len(prefix) + len(comp))) % 4
    header_len = 8 + pad
    comp_len = len(comp) + header_len
    blob = prefix + comp + b"\xff" * pad
    total = len(blob) + 8
    extra = len(data) - total
    if extra <= 0:
        raise ValueError("data does not compress; pick more compressible test data")
    return blob + struct.pack("<II", (header_len << 24) | comp_len, extra)


# --------------------------------------------------------------- ARM9 / overlays


def words(*ws):
    return struct.pack(f"<{len(ws)}I", *ws)


def bl(frm, to):
    """ARM BL instruction at address `frm` to `to`."""
    off = ((to - frm - 8) >> 2) & 0xFFFFFF
    return 0xEB000000 | off


def build_arm9():
    """Uncompressed ARM9 image and its layout facts."""
    a = bytearray(0x830)
    # Entry: ldr r0,=data; bl ARM9_FUNC; b .
    a[0x00:0x14] = words(0xE59F0008, bl(ARM9_RAM + 4, ARM9_FUNC), 0xEAFFFFFE, 0xE1A00000,
                         ARM9_RAM + 0x300)
    a[0x20:0x24] = words(0xE12FFF1E)  # ARM9_FUNC: bx lr
    autoload_list = ARM9_RAM + 0x200
    autoload_start = ARM9_RAM + 0x800
    bss_start, bss_end = ARM9_RAM + 0x800, ARM9_RAM + 0x1000
    mp_off = 0x100
    a[mp_off:mp_off + 0x24] = words(autoload_list, autoload_list + 24, autoload_start,
                                    bss_start, bss_end, 0, 0x05028000,
                                    NITROCODE, NITROCODE_REV)
    a[0x200:0x218] = words(ITCM, 0x20, 0, DTCM, 0x10, 0x400)
    # Static data: compressible pattern.
    for k in range(0x300, 0x800):
        a[k] = (k // 16) & 0xFF
    # ITCM code: mov r0,#1; bx lr; then padding.
    a[0x800:0x808] = words(0xE3A00001, 0xE12FFF1E)
    a[0x820:0x830] = b"DTCMDATA" * 2
    layout = {"mp_off": mp_off, "autoload_start": autoload_start, "bss": (bss_start, bss_end)}
    return bytes(a), layout


def build_overlay(ram, tag, size):
    """Overlay image: a function at +0 that calls ARM9_FUNC and loads a pointer to its
    own data at +0x14, and a one-entry static initialiser table at +0x18 pointing at it."""
    o = bytearray(size)
    # push {lr}; bl ARM9_FUNC; ldr r0,=ram+0x14; pop {pc}; .word ram+0x14
    o[0:0x14] = words(0xE52DE004, bl(ram + 4, ARM9_FUNC), 0xE59F0000, 0xE49DF004, ram + 0x14)
    for k in range(0x14, size):
        o[k] = (tag * 7 + k // 8) & 0xFF
    o[SINIT_OFF:SINIT_OFF + 4] = words(ram)
    return bytes(o)


def build(compress_arm9=False):
    """Return (rom bytes, facts dict) for a synthetic ROM."""
    arm9_plain, lay = build_arm9()
    if compress_arm9:
        # The module params sit in the raw (uncompressed) prefix, so "compressed static
        # end" can be patched after encoding without touching the compressed stream.
        # The ROM's copy and the decoded image then both hold the same value.
        arm9_rom = bytearray(blz_compress(arm9_plain, raw_prefix=0x300))
        cse = ARM9_RAM + len(arm9_rom)
        arm9_plain = bytearray(arm9_plain)
        for img in (arm9_rom, arm9_plain):
            struct.pack_into("<I", img, lay["mp_off"] + 0x14, cse)
        arm9_rom, arm9_plain = bytes(arm9_rom), bytes(arm9_plain)
    else:
        arm9_rom = arm9_plain

    ov_plain = [build_overlay(OV_RAM_A, 0, 0x200), build_overlay(OV_RAM_A, 1, 0x400),
                build_overlay(OV_RAM_B, 2, 0x300)]
    ov_stored = [ov_plain[0], blz_compress(ov_plain[1]), blz_compress(ov_plain[2], raw_prefix=0x10)]
    ov_meta = [  # id, ram, bss, compressed
        (0, OV_RAM_A, 0x40, False), (1, OV_RAM_A, 0x80, True), (2, OV_RAM_B, 0, True)]

    arm9_off = 0x1000
    footer = words(NITROCODE, lay["mp_off"], 0)
    pos = arm9_off + len(arm9_rom) + len(footer)
    pos = (pos + 0x1FF) & ~0x1FF
    ov_table_off = pos
    pos += 32 * len(ov_meta)
    fat_off = (pos + 0x1FF) & ~0x1FF
    pos = fat_off + 8 * len(ov_stored)
    file_offs = []
    for blob in ov_stored:
        pos = (pos + 0x1FF) & ~0x1FF
        file_offs.append(pos)
        pos += len(blob)
    total = (pos + 0xFFF) & ~0xFFF

    rom = bytearray(total)
    rom[0x00:0x0C] = b"RESCUESYNTH\0"
    rom[0x0C:0x10] = b"####"
    struct.pack_into("<4I", rom, 0x20, arm9_off, ARM9_RAM, ARM9_RAM, len(arm9_rom))
    struct.pack_into("<2I", rom, 0x48, fat_off, 8 * len(ov_stored))
    struct.pack_into("<2I", rom, 0x50, ov_table_off, 32 * len(ov_meta))
    struct.pack_into("<I", rom, 0x80, total)
    rom[arm9_off:arm9_off + len(arm9_rom)] = arm9_rom
    rom[arm9_off + len(arm9_rom):arm9_off + len(arm9_rom) + 12] = footer
    for i, ((ov_id, ram, bss, comp), plain, blob) in enumerate(zip(ov_meta, ov_plain, ov_stored)):
        flags = (len(blob) | (1 << 24)) if comp else 0
        struct.pack_into("<8I", rom, ov_table_off + 32 * i, ov_id, ram, len(plain), bss,
                         ram + SINIT_OFF, ram + SINIT_OFF + 4, i, flags)
        struct.pack_into("<2I", rom, fat_off + 8 * i, file_offs[i], file_offs[i] + len(blob))
        rom[file_offs[i]:file_offs[i] + len(blob)] = blob

    facts = {"arm9": arm9_plain, "overlays": ov_plain, "ov_meta": ov_meta, "layout": lay}
    return bytes(rom), facts


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 1:
        sys.exit(__doc__)
    rom, _ = build(compress_arm9="--compress-arm9" in sys.argv)
    with open(args[0], "wb") as f:
        f.write(rom)
