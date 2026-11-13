# Fixed-room file: `fixedmap`

Issue: [#397](https://github.com/TorratDev/rust-mm/issues/397), parent [#342](https://github.com/TorratDev/rust-mm/issues/342). Code: `crates/formats/src/fixedmap.rs`. Consumer: `crates/sim` (`floor_gen.rs`, `PortedSteps`; `floor.rs`, `place_fixed_room` and `place_fixed_room_large`). Tests: unit tests in `fixedmap.rs`; `tests/tests/fixedmap_rom.rs` (`fixedmap_entries_decode_to_their_size`, ROM-gated); the whole-floor check `tests/tests/fixed_rooms_rom.rs` reads the same file. Fuzz: `fuzz/fuzz_targets/formats_fixedmap.rs`. Process notes: [`re/fixed-rooms.md`](../re/fixed-rooms.md).

**Verification status:** verified against the Blue EUR dump (2026-11-13): all 140 entries parse and each decodes exactly `width * height` codes, ending at the next entry (entry 139: at the table, with two `0xAA` bytes before it). The 100-row whole-floor check passes with the parser feeding the sim.

**Tags.** `[blue-verified]`: checked against the ROM data or the decompile of the Blue ARM9 and its overlays. `[red-hypothesis]`: not checked in Blue. Nothing here comes from pmd-red. The reject list is a design choice, marked `[design choice]`: the game checks none of it.

## Where it lives

`fixedmap` is an entry of the `dungeon.sbin` pack archive (see [`archive.md`](archive.md)), 7552 bytes in the EUR dump. It is a SIR0 file ([`archive.md`](archive.md#sir0)): the header's content pointer (`+4`) is the entry table, and the pointer list relocates the header's own words and the 140 table words. [blue-verified] (`0x1AB8` table, `0x1CF0` pointer list, file length `0x1D80`.)

The game opens it with `OpenFileAndGetFileDataPtr("fixedmap")` (`0201A54C`), which reads it, relocates it (`0201A188`, `0201A0B8`) and stores the relocated content pointer as the table base in the file handle's `+0xC`. `ProcessFixedRoom` (`02150958`) and `0214A0D4` read entry `n` through that base. [blue-verified] static (investigator findings for #397, P1).

## Table

| Offset | Size | Field |
|--------|------|-------|
| content pointer (`+4` of the SIR0 header) | 4 × 140 | `u32` offsets from the file start, one per entry; entry `n` is word `n` |
| after the table | 2 | `0xAA` padding in retail (the pointer list follows) |

The count is not stored. Retail has 140 words, and the word after them is padding, so the table length is the entry count. [blue-verified] Words are strictly increasing; the first is `0x10` (right after the SIR0 header), the last is `0x1A9B`. [blue-verified]

## Entry

| Offset from the entry | Size | Field |
|-----------------------|------|-------|
| `+0` | 1 | width (tiles) |
| `+1` | 1 | height (tiles) |
| `+2` | 1 | flags. Bit 0 selects the staged branch of the stamp for rooms 50 and up (`02149E80`). Retail values: 0 or 1. |
| `+3` | to the next entry | tile stream |

There is no length field and no terminator. The entry's stream ends where the next entry starts; entry 139's stream ends at the content pointer. [blue-verified] The consumer pulls exactly `width * height` codes, row by row.

### Tile stream

| Byte | Meaning |
|------|---------|
| `0x0E` | escape: the next byte is a literal tile code (one tile) |
| any other `b` | tile code `b >> 4`, repeated `(b & 0xF) + 1` times |

The escape is read only when no run is pending, so a literal is one tile. Codes `0x10` and above can only be written with the escape, since a run's code is a nibble. [blue-verified] static: decoder `0214A348`. The sim reads code 0 past the end of a stream; retail never reaches it.

Retail data, [blue-verified] (ROM data, decoded by investigator A; the ROM test asserts the first three):

- widths and heights are 5 to 17, so no retail entry has a zero size (`ProcessFixedRoom` treats a zero size as a one-room monster house; no retail entry takes that branch);
- flags are 0 or 1 (1 on 12 entries: 50-52, 54-60, 62, 63);
- decoding exactly `width * height` codes ends at the next entry's start for entries 0 to 138, with nothing left over; entry 139 is followed by two `0xAA` bytes, then the table;
- the largest tile code is `0x90`.

## Checks on parse

`FixedMap::parse` checks the whole table once, so [`FixedMap::entry`](../../crates/formats/src/fixedmap.rs) returns only entries that passed. Every rejection below is a design choice (`[design choice]`); none is a game check, and the game's behaviour on such files is not run ([red-hypothesis]).

- The content pointer and the 140 table words must lie inside the file. `[design choice]`
- An entry must start at `0x10` or later (the end of the SIR0 header). `[design choice]`
- An entry's 3-byte header must fit before the next entry's start (entry 139: before the content pointer). This also makes the offsets non-decreasing. `[design choice]`
- A stream must give `width * height` codes before it runs out. An escape with no literal byte counts as running out. `[design choice]`
- A run must not pass the last code (a pending repeat after the last code). `[design choice]`

Accepted on purpose: width or height 0 (returned as is, with the stream it has; the sim decides what a zero size means); bytes after the last code (entry 139's `0xAA` padding is one).

`entry(number)` takes an `i16`; a negative or out-of-range number gives `None`.

## Unknown fields

- The game's behaviour on a malformed `fixedmap` (no retail file is malformed) is not run. [red-hypothesis]
- The writer of the file handle's `+0xC` field (dungeon `+0x11304` holds the handle) is not traced; the table base is taken from the relocation code, not from a runtime read. [red-hypothesis]
