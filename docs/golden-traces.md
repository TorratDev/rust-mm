# Golden-trace format (schema v1)

A golden trace is the game's state, read from a scripted retail run, once per turn. `pmd-sim` replays the same inputs, and the harness diffs the two turn by turn. The capture scripts (#55) write these files, and `trace-harness` (#56) reads them. This page is the one format both agree on. The Rust types and the reader are in [`crates/trace`](../crates/trace) (crate `rescue-trace`).

Traces are language-neutral fixtures: the capture side may be Lua or Python, the replay side is Rust. The format is JSON Lines, and it is versioned because the RAM fields will grow as [#46](https://github.com/TorratDev/rust-mm/issues/46) (dungeon and entity structs) and [#47](https://github.com/TorratDev/rust-mm/issues/47) (RNG) add them.

## File layout

- A trace is UTF-8 text in JSON Lines: one JSON object per line, `\n` line endings.
- **Line 1 is the header.** It has `schemaVersion`, and a reader that does not know that version stops.
- **Lines 2 onward are turn rows**, in capture order. Blank lines are ignored.
- No raw RAM is stored. Every value is a decoded field, so a file can be shared and diffed without a dump.

### Header

| Key | Type | Meaning | Source |
|-----|------|---------|--------|
| `schemaVersion` | integer | `1` for this version. | this page |
| `rom.sha1` | string | SHA-1 of the ROM file, 40 lowercase hex digits. The trace is valid only for this dump. | [`supported-dumps.md`](supported-dumps.md) |
| `emulator.name` | string | Capture emulator, for example `DeSmuME (py-desmume)`. | the capture script |
| `emulator.version` | string | Emulator version, exactly as it reports itself. | the capture script |
| `scenario` | string | Scenario ID. The input lists below belong to it. | free text, chosen by the author |
| `initialSeed.source` | string | RAM address the boot seed is copied from, 8 hex digits: `027FFCF4`. | [`re/rng.md`](re/rng.md) (Seeding: `SeedRng` at boot, MAC-derived, fixed per console) |
| `initialSeed.bytes` | string | The 6 bytes at `source`, 12 hex digits, read at boot before any input. | [`re/rng.md`](re/rng.md) |
| `inputs.boot` | array of strings | Input lines from power-on until the dungeon is reached. | the input files format below |
| `inputs.turn` | array of strings | Input lines from the first dungeon frame. | the input files format below |

**Input lines** use the format of the input files in [`verification/v7-trace/`](../verification/v7-trace/): `FRAME KEY KEY ...`, where `FRAME` is an integer and the keys are held from that frame until the next line. A lone `-` after the frame releases all keys. Key names: `A B SELECT START RIGHT LEFT UP DOWN R L X Y`, upper case. Comments (`#`) are not allowed in the header. The reader checks each line.

### Turn row

| Key | Type | Meaning | RAM source |
|-----|------|---------|------------|
| `frame` | integer | Frame counter of the capture run. Not RAM. | the capture script |
| `fractionalTurn` | s16 | Turn fraction. | dungeon `+0x650` (`fractionalTurn`), [`re/structs.md`](re/structs.md#dungeon-struct) |
| `runSeed` | u32 | Run preseed (the 24-bit value at `+0x66C`). The name comes from `retail_trace.py`. | dungeon `+0x66C` (`prngPreseed23Bit`), [`re/structs.md`](re/structs.md#dungeon-struct); [`re/rng.md`](re/rng.md) (seeding a dungeon run) |
| `floorSeed` | u32 | Seed for the current floor. | dungeon `+0x670` (`unk3C`, floor seed), [`re/structs.md`](re/structs.md#dungeon-struct); [`re/rng.md`](re/rng.md) |
| `sPRNGState` | u32 | Global LCG state. | `020DF704`, [`re/rng.md`](re/rng.md) (global LCG) |
| `dungeonRng` | u32 | Dungeon LCG state. | `ov_0001::021B3F58`, [`re/rng.md`](re/rng.md) (dungeon LCG) |
| `dungeonPreseed` | u32 | Preseed LCG state. | `ov_0001::021B3F5C`, [`re/rng.md`](re/rng.md) (preseed LCG) |
| `team` | array of 4, each an entity or `null` | Team slots. `null` means the pointer is 0 or the entity is not valid. | dungeon `+0x11314` (4 entity pointers), [`re/structs.md`](re/structs.md#dungeon-struct) |
| `wild` | array of entities | Set wild slots only (up to 16). Empty slots are left out. | dungeon `+0x11324` (16 entity pointers), [`re/structs.md`](re/structs.md#dungeon-struct) |
| `tilesSha256` | string | First 16 lowercase hex digits of SHA-256 over the tile grid (35,840 bytes). Stands in for floor state in v1. See the note below. | dungeon `+0x34B8`, `tiles[32][56]`, 0x14 bytes each, [`re/structs.md`](re/structs.md#dungeon-struct) and [`re/structs.md`](re/structs.md#tile) |

`gDungeon` (`020E0788`, [`re/structs.md`](re/structs.md#dungeon-struct)) is the dungeon pointer that every dungeon offset above is relative to. The trigger that writes a row is `RunFractionalTurn_Async`, at `ov_0001::021411BC` (`TRIGGER` in [`verification/v7-trace/retail_trace.py`](../verification/v7-trace/retail_trace.py); V7 in [`verification.md`](verification.md#v7--scriptable-ds-emulator-for-ram-and-rng-dumps)).

**Entity** (each team and wild slot). The pointer is the entity; `info` is the entity info it points to (`entity+0x70`):

| Key | Type | Meaning | RAM source |
|-----|------|---------|------------|
| `type` | u32 | Entity type; 0 means no entity. | entity `+0x00`, `EntityIsValid`, [`re/structs.md`](re/structs.md#entity) |
| `x`, `y` | s16 | Position. | entity `+0x04`, `+0x06`, [`re/structs.md`](re/structs.md#entity) |
| `species` | s16 | Species ID. | info `+0x02` (`id`), [`re/structs.md`](re/structs.md#entity-info) |
| `level` | u8 | Level. | info `+0x09`, [`re/structs.md`](re/structs.md#entity-info) |
| `hp` | s16 | Current HP. | info `+0x0E`, [`re/structs.md`](re/structs.md#entity-info) |
| `maxHp` | s16 | Max HP stat. | info `+0x10`, [`re/structs.md`](re/structs.md#entity-info) |
| `speedStage` | u32 | Speed stage. The width is what `retail_trace.py` reads; `re/structs.md` gives no width. | info `+0xE0`, [`re/structs.md`](re/structs.md#entity-info) |

**Floor state.** In v1, floor state is `tilesSha256` only. Other floor fields (stairs, traps, items) are not in the row yet.

**Inventory.** Not captured in v1. The row has no `inventory` key, because rows reject unknown keys (see below). The layout of the team inventory is not yet a verified struct in [`re/structs.md`](re/structs.md#moves-items-and-inventory). A later schema version adds it.

### Note on `tilesSha256`

The name says SHA-256, but `retail_trace.py` writes the first 16 hex digits of the digest (`hexdigest()[:16]`), not all 64. Version 1 keeps that value, so existing captures stay valid. The harness compares the 16-digit string as a whole. A 64-digit value would need a new schema version.

## Validation

A reader for version 1 must reject the file if any of these hold:

- the first non-blank line is not a header, or its `schemaVersion` is not `1`;
- `rom.sha1`, `tilesSha256` or `initialSeed.bytes` has the wrong length or is not lower-case hex (for `initialSeed.bytes`, 12 digits);
- `initialSeed.source` is not 8 hex digits;
- an input line has a non-numeric frame, no keys, or an unknown key;
- a row or header has an unknown key, or a field has the wrong type. Unknown keys are an error, not ignored, so a new field cannot slip past an old reader.

The Rust reader (`rescue_trace::read_trace`) checks all of these. The writer (`write_trace`) checks them before it writes.

## Versioning

- `schemaVersion` is an integer. Any change to the header or row keys is a new version. Readers reject versions they do not know; they do not guess.
- Version 1 readers do not accept version 2 files, even if the new keys look harmless. Capture and harness tools move to a new version together.
- Changing what a field means, or its width, is also a new version. The same name with new meaning is not allowed.

## Reproducibility

A capture is reproducible when the same ROM (`rom.sha1`), the same emulator (`emulator.name` and `version`), the same `scenario` and the same input lists give the same rows. Mismatches show up in the header: a file from a different ROM or emulator version is detectable before any row is compared.

Byte-for-byte equality of two files needs one writer. The rows are equal as parsed values when they agree. `retail_trace.py` writes the summary `sha256` of the file, so a repeat run can be checked the same way.

## Status

- The format is written and the reader is in `crates/trace`. The capture scripts do not write the header yet: `retail_trace.py` still writes rows only. Its output will not read as a complete trace until #55 adds the header. Its rows do read: `rescue_trace` accepts a row exactly as the script emits it (tested).
- `inventory` is out of v1.
- `initialSeed` is written by the capture script at boot. Its value is not captured yet in any existing trace.
- The field widths for `type` and `speedStage` follow `retail_trace.py`; they are not confirmed against the struct layout.

## Links

- Epic and tasks: [#54](https://github.com/TorratDev/rust-mm/issues/54), parent [#5](https://github.com/TorratDev/rust-mm/issues/5).
- Feeds: capture scripts [#55](https://github.com/TorratDev/rust-mm/issues/55), harness [#56](https://github.com/TorratDev/rust-mm/issues/56), replay [#80](https://github.com/TorratDev/rust-mm/issues/80).
- Dungeon layout: [`re/structs.md`](re/structs.md). RNG: [`re/rng.md`](re/rng.md). Symbols: [`symbols.md`](symbols.md).
- Legal: traces hold decoded state from the user's own dump; see [`legal.md`](legal.md). Test fixtures are synthetic.
