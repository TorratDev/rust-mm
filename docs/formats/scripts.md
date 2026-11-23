# Ground script data

Layout of the ground-mode scripts: where they are stored, the lookup chain, and the command record. Engine behaviour and the opcode table are in [`re/scripts.md`](../re/scripts.md). Issue [#362](https://github.com/TorratDev/rust-mm/issues/362).

Status: **research notes**. No parser exists in `crates/formats`. The layout is read from the ARM9 and overlay code and checked against the retail ROM at runtime. Addresses are ARM9 unless marked `ov_0000::`. Table contents are not recorded.

## Where the data lives

| Kind | Location | Found by | Status |
|------|----------|----------|--------|
| Loader scripts | ARM9 data, a table of 406 entries of 12 bytes, runtime base `0x020DA5D4` | Index into the table: `table + idx × 0xC` (`ov_0000::02147E6C`) | **blue-verified** |
| Bank scripts | Inside ground-bank overlays 10–144 (FAT overlays), loaded at `0x0215D720` | Bank → root table (`0x020DB8E0` for the overlay id, `0x020DB8E4` for the root pointer, both indexed by `bank × 8`) → group → sector → list (`ov_0000::0214D510`) | **blue-verified** |

A bank number is not an overlay id. The runtime tables are filled at load and are not part of the ROM layout.

## Lookup chain (bank scripts)

| Step | Structure | Size | Fields read |
|------|-----------|------|-------------|
| 1 | Bank root | 8 bytes | `u32` group count; `u32` pointer to the groups |
| 2 | Group entry | 8 bytes | `u32` sector count; `u32` pointer to the sectors |
| 3 | Sector record | `0x28` bytes | list count at `+0x20`; list pointer at `+0x24`. Bytes `+0x00`–`+0x1F` not read. |
| 4 | List element | not fixed | `+8`: script pointer. The first list word points to the element. Other fields not read. |

Each lookup asserts that the index is below its count. A sector with a count of zero has no list.

Reference record: `ov_0000::0214D510` returns a 12-byte reference with these fields in this order: script pointer; kind (2 for a loader script, 5 for a bank script that replaces one); group (`u16`); sector (`u8`). Offsets: pointer at `+0x0` (**blue-verified**, runtime 10 of 10); kind at `+0x4` (**blue-verified**, runtime: 2 for loader installs, 5 for bank installs); group at `+0x6` and sector at `+0x8` (**red-hypothesis**: the probe reads them but does not compare them with anything).

## Loader table entry

12 bytes, read by `ov_0000::02147E6C`.

| Offset | Size | Field |
|--------|------|-------|
| `+0x0` | `u32` | script pointer (ARM9) |
| `+0x4` | `u16` | index + 1 |
| `+0x6` | `u16` | kind |
| `+0x8` | `u32` | name pointer |

**blue-verified** (static, `ov_0000::02147E6C`); runtime installs matched the expected pointer (see [`re/scripts.md`](../re/scripts.md#p1--storage-lookup-and-binary-layout)).

## Script command record

A script has no header: its pointer is the first command. Commands are 0x18 bytes each, read in order. The interpreter is `ov_0000::021421CC`.

| Offset | Size | Field | Notes |
|--------|------|-------|-------|
| `+0x0` | `u8` | opcode | `0x00`–`0xF6` dispatch (247 slots) |
| `+0x1` | `u8` | arg | frame counts, compare operators, small values |
| `+0x2` | `s16` | label or variable id | jump target for branches; `-1` in an operand means "read a variable" |
| `+0x4` | `s32` | main operand | its low `s16` is often an id |
| `+0x8`, `+0xC`, `+0x10`, `+0x14` | `s32` | further operands | |

Label `0xF4`: the label id is the `s16` at `+0x2`. Jumps go by label id: `ov_0000::021406A8` searches forward from the script base for a `0xF4` with that id. Opcodes `0xF2`–`0xF6` are no-ops that fall through.

Case record after a switch: opcodes `0xCC` (compare with op 2), `0xCD` (compare with op = `+0x1`) and `0xCE` (compare with a variable id at `+0x4`). The jump target is the `s16` at `+0x2`. Read by `ov_0000::021405B4`. **blue-verified** (static).

Script end: `0xEF` (returns 0). Opcode `0x00` and `0xF6` end a label search.

Variable ids: below `0x400` are global (valid range below `0x53`); `0x400` and up are local to the script's frame. Getter `ARM9 0x02020F74`. **blue-verified** (static). Descriptor layout is in [`formats/save.md`](save.md#2-serialized-layout) under story flags.

## Compression

None is known for the script data. The loader reads bank overlays through the normal FS overlay API, so the bank overlays are presumably stored as ordinary overlays. Not checked against the ROM. **open**.

## Unknown fields

- Sector record `+0x00`–`+0x1F` and list element fields other than `+8`. **lead**: placement lists for actors and objects (see [`re/scripts.md`](../re/scripts.md#p4--actors-camera-and-dialogue)).
- Whether a list holds more than one script (used by `0x1D`, `0xEA`, `0xEB`). **open**.
- Reference record group (`+0x6`) and sector (`+0x8`) offsets. **red-hypothesis** until a probe compares them with the list element and sector they came from.
- Meaning of `0xF6` as the end of a script block. **red-hypothesis**.

## Verification

- Lookup and installs: 10 of 10 matched at runtime on the retail ROM, probe `verification/v7-trace/script_lookup_probe.py` ([`verification.md`](../verification.md#362--script-engine-probes)).
- Record size and dispatch: 228 of 228 dispatches read the record at the cursor, runtime probe `verification/v7-trace/interp_probe.py`.
- No parser or fixture. A future `crates/formats` parser would need a ROM-gated test.
