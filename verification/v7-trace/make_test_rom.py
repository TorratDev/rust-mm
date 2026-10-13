#!/usr/bin/env python3
"""Build a tiny synthetic NDS ROM for Phase 0 check V7.

The ARM9 program (source: test_rom.s) waits for VBlank once per frame, then
updates a small "game state" block at 0x02100000:

    +0  u32  frame counter
    +4  u32  LCG state, mixed with KEYINPUT each frame (stand-in for the RNG)
    +8  u16  KEYINPUT as read this frame

TICK_DONE runs once per frame, right after the update, so a breakpoint there
sees the state for that frame. That stands in for the per-turn trigger the real game
will need. The ARM7 spins forever. No retail code or data is included.

Usage: python3 make_test_rom.py out.nds
"""

import struct
import sys

ARM9_ADDR = 0x02000000
ARM7_ADDR = 0x02380000
TICK_DONE = ARM9_ADDR + 0x44
STATE_ADDR = 0x02100000

# Assembled from test_rom.s with
#   llvm-mc -triple=armv5te-none-eabi -filetype=obj test_rom.s
# and kept as words so no ARM toolchain is needed to build the ROM.
ARM9_CODE = [
    0xE59F4050, 0xE3A05621, 0xE3A06000, 0xE59F7048,
    0xE59F8048, 0xE59F9048, 0xE59FA048, 0xE1D400B0,
    0xE35000C0, 0x1AFFFFFC, 0xE2866001, 0xE1DA10B0,
    0xE0277001, 0xE0279798, 0xE5856000, 0xE5857004,
    0xE1C510B8, 0xE1A00000, 0xE1D400B0, 0xE35000C0,
    0x0AFFFFFC, 0xEAFFFFF0, 0x04000006, 0x01234567,
    0x41C64E6D, 0x00003039, 0x04000130,
]
ARM7_CODE = [0xEAFFFFFE]  # b .


def crc16(data: bytes) -> int:
    """CRC-16/MODBUS, as used for the NDS header checksum."""
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def build() -> bytes:
    arm9 = struct.pack(f"<{len(ARM9_CODE)}I", *ARM9_CODE)
    arm7 = struct.pack(f"<{len(ARM7_CODE)}I", *ARM7_CODE)
    arm9_off, arm7_off = 0x4000, 0x8000
    total = 0x10000

    h = bytearray(0x4000)
    h[0x00:0x0C] = b"V7TRACETEST\0"
    h[0x0C:0x10] = b"####"
    h[0x10:0x12] = b"00"
    h[0x14] = 0  # capacity: 128 KiB
    struct.pack_into("<4I", h, 0x20, arm9_off, ARM9_ADDR, ARM9_ADDR, len(arm9))
    struct.pack_into("<4I", h, 0x30, arm7_off, ARM7_ADDR, ARM7_ADDR, len(arm7))
    struct.pack_into("<2I", h, 0x80, total, 0x4000)
    struct.pack_into("<H", h, 0x15E, crc16(h[:0x15E]))

    rom = bytearray(total)
    rom[: len(h)] = h
    rom[arm9_off : arm9_off + len(arm9)] = arm9
    rom[arm7_off : arm7_off + len(arm7)] = arm7
    return bytes(rom)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    with open(sys.argv[1], "wb") as f:
        f.write(build())
