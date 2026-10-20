# Containers: `.sbin` pack archives and SIR0

Issues: [#31](https://github.com/TorratDev/rust-mm/issues/31), [#41](https://github.com/TorratDev/rust-mm/issues/41). Code: `crates/formats/src/archive.rs`, `crates/formats/src/sir0.rs`. Tests: unit tests in those files; `tests/tests/formats_rom.rs` (`archives_and_wrappers_parse`). Survey entries: every `*.sbin` except `sound.sbin` in [`file-survey.md`](../file-survey.md).

**Verification status:** verified against the Blue EUR dump (2026-10-16): all 8 pack archives parse; their 154 AT4PX and 1715 SIR0 sub-files parse.

## Pack archive (`dungeon`, `effect`, `ground`, `monster`, `ornament`, `sample`, `system`, `titlemenu` `.sbin`)

No magic. A table of 16-byte entries at offset 0, then the sub-files.

| Offset | Size | Field |
|--------|------|-------|
| `0x0` | 8 | name, NUL-padded Latin-1 (`kao001`, `b00pal`, ...) |
| `0x8` | 4 | offset of the sub-file in the archive |
| `0xC` | 4 | size |

The table ends at an entry whose name starts with NUL, or where the first sub-file starts. The survey's "first 4 bytes" for these files are the first entry's name (`ax00`, `b00c`, `font`, ...).

Resource IDs are the names: see the format specs for the per-archive naming ([`portraits.md`](portraits.md), [`sprites.md`](sprites.md), [`tilesets.md`](tilesets.md), [`maps.md`](maps.md), [`text.md`](text.md), [`data.md`](data.md)).

## Archive index

Blue reads the TOC of every pack archive at boot, into one global table (`LoadArchiveTocs`, called from `0x02000CE8`; [`re/io.md`](../re/io.md#archive-index-and-toc)). Each archive has an index:

| Index | Archive |
|-------|---------|
| 1 | `sample` |
| 2 | `system` |
| 3 | `titlemenu` |
| 4 | `dungeon` |
| 5 | `monster` |
| 6 | `effect` |
| 7 | `ground` |
| 8 | `ornament` |
| 9 | `sound` |

Index 0 is unused. `sound.sbin` (index 9) is loaded by this same boot reader, so it is not only a survey entry. This page does not describe its contents. The use of the `/sound.sbin` string is unresolved ([`re/io.md`](../re/io.md#open-questions)).

## SIR0

| Offset | Size | Field |
|--------|------|-------|
| `0x0` | 4 | magic `SIR0` |
| `0x4` | 4 | offset of the content header ("data pointer") |
| `0x8` | 4 | offset of the pointer list |
| `0xC` | 4 | 0 |

Pointers inside the file are offsets from the file start; the game relocates them at load time. The pointer list is a run of big-endian base-128 varints (bit 7 = more bytes follow), each the distance from the previous pointer's position; a 0 byte ends it. The first two entries are always the header's own pointers at `0x4` and `0x8`. Padding between parts is `0xAA`.

## Unknown fields

- None in the containers. Archive sub-files with no magic (raw palettes, ground files) are described in their own specs.
