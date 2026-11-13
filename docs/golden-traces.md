# Golden-trace format (schema v2)

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
| `schemaVersion` | integer | `2` for this version. | this page |
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
| `tilesSha256` | string | First 16 lowercase hex digits of SHA-256 over the tile grid (35,840 bytes). Stands in for floor state (see Not dumped). See the note below. | dungeon `+0x34B8`, `tiles[32][56]`, 0x14 bytes each, [`re/structs.md`](re/structs.md#dungeon-struct) and [`re/structs.md`](re/structs.md#tile) |
| `actions` | array of action records | The leader's actions chosen during this turn, in order. Empty when the leader does not act. Required in every row. | leader info, see [Action record](#action-record) |

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

### Action record

Each item of `actions` is one leader action. The game chooses it in `ov_0001::02166B10`, which `ov_0001::02140FAC` (`RunLeaderTurn_Async`) calls. The capture reads the record at `ov_0001::021410D4`, the instruction after that call, from the leader's entity info: team slot 0 (`dungeon+0x11314`), then `entity+0x70`. One leader turn can choose more than one action, so a row can hold several records. The action chooser's behavior is in [`re/turns.md`](re/turns.md) ("For golden traces").

| Key | Type | Meaning | RAM source |
|-----|------|---------|------------|
| `id` | u16 | Raw action id. The schema does not name the ids; the mapping to commands is later work ([#333](https://github.com/TorratDev/rust-mm/issues/333)). | info `+0x42`, [`re/turns.md`](re/turns.md) |
| `direction` | u8, 0 to 7 | The direction the game stored: 0 Down, 1 DownRight, 2 Right, 3 UpRight, 4 Up, 5 UpLeft, 6 Left, 7 DownLeft. It is the game's value, not the held keys. The game can overwrite it during the turn (for example in auto-move), so the raw D-pad does not give it. | info `+0x44`, [`re/io.md`](re/io.md) (Dungeon directions) |
| `param` | u8 | Parameter byte of the action. Its meaning depends on `id` and is not decoded here. | info `+0x46` |
| `slot` | u8 | Move slot the action uses. | info `+0x4C` |

**Matching records to rows.** A row is written when `RunFractionalTurn_Async` starts. The leader's action records of that call come after the row and before the next row, so each record belongs to the last row written. No frame or counter key is needed to match them. A record does not copy `fractionalTurn`, because the game can reset that value during the turn.

**Floor state.** In this version, floor state is `tilesSha256` only. Other floor fields (stairs, traps, items) are not in the row yet.

**Inventory.** Not captured in v1. The row has no `inventory` key, because rows reject unknown keys (see below). The layout of the team inventory is not yet a verified struct in [`re/structs.md`](re/structs.md#moves-items-and-inventory). A later schema version adds it.

### Note on `tilesSha256`

The name says SHA-256, but `retail_trace.py` writes the first 16 hex digits of the digest (`hexdigest()[:16]`), not all 64. Version 1 keeps that value, so existing captures stay valid. The harness compares the 16-digit string as a whole. A 64-digit value would need a new schema version.

## Validation

A reader for version 2 must reject the file if any of these hold:

- the first non-blank line is not a header, or its `schemaVersion` is not `2`;
- `rom.sha1`, `tilesSha256` or `initialSeed.bytes` has the wrong length or is not lower-case hex (for `initialSeed.bytes`, 12 digits);
- `initialSeed.source` is not 8 hex digits;
- `wild` has more than 16 entries;
- an input line has a non-numeric frame, no keys, or an unknown key;
- a row or header has an unknown key, or a field has the wrong type. Unknown keys are an error, not ignored, so a new field cannot slip past an old reader;
- a row has no `actions` key;
- an action record has an unknown key, a field of the wrong type, or a `direction` above 7.

The Rust reader (`rescue_trace::read_trace`) checks all of these. The writer (`write_trace`) checks them before it writes.

## Versioning

- `schemaVersion` is an integer. Any change to the header or row keys is a new version. Readers reject versions they do not know; they do not guess.
- Version 2 readers reject version 1 files. A v1 row has no `actions` key, and v2 requires it. Capture and harness tools move to a new version together.
- Changing what a field means, or its width, is also a new version. The same name with new meaning is not allowed.

## Reproducibility

A capture is reproducible when the same ROM (`rom.sha1`), the same emulator (`emulator.name` and `version`), the same `scenario` and the same input lists give the same rows. Mismatches show up in the header: a file from a different ROM or emulator version is detectable before any row is compared.

Byte-for-byte equality of two files needs one writer. The rows are equal as parsed values when they agree. `retail_trace.py` writes the summary `sha256` of the file, so a repeat run can be checked the same way.

## Status

- The format is written and the reader is in `crates/trace`. `scripts/capture_golden.py` writes the complete trace: header, rows with `actions`, and a read-back check with the crate (see [`scripts/README.md`](../scripts/README.md)). `retail_trace.py` still writes rows only, as it did in V7; its rows are the ones the capture script reuses, and the script adds `actions` to each.
- **The action hook is unverified at runtime.** The hook at `ov_0001::021410D4` was written from the static findings in #367. It has not been run, because no emulator was available when the change was made. Check the first capture by hand before relying on its `actions`.
- `inventory` is out of this version.
- `initialSeed` is read by `capture_golden.py`, from `027FFCF4` in the power-on run, at the first frame where it is set (frame 15 in the retail run), before any input. `retail_trace.py` does not read it.
- The field widths for `type` and `speedStage` follow `retail_trace.py`; they are not confirmed against the struct layout.

## Not dumped

These fields are not in v1 rows. Each is either out of the schema or not documented in `re/` yet, and a capture does not read it:

- Team inventory, and the floor items, traps and stairs (floor state is `tilesSha256` only).
- The meaning of each action `id` and `param`. The ids are recorded raw; the schema names none of them.
- Entity info fields other than `species`, `level`, `hp`, `maxHp` and `speedStage`, including the statuses, moves and types. Their offsets are documented in [`re/structs.md`](re/structs.md#entity-info), but the schema has no key for them yet.
- The Mersenne Twister state (`sStateArray`, `sStateIndex`) and the 16-bit generator in `other_random.c`, whose use is not known yet ([`re/rng.md`](re/rng.md)).
- Save data, sound and graphics. Not part of the game state a trace records.

## Capture scripts

- Windows setup, the save state and the self-test: [`scripts/README.md`](../scripts/README.md).
- The capture script is `scripts/capture_golden.py`. It uses the emulator and the row reader in [`verification/v7-trace`](../verification/v7-trace).

## Leader-only replay (#348)

`rescue-trace-harness` compares every field of a row by default ([`Scope::All`](../apps/trace-harness/src/lib.rs)). A leader-only mode, `Scope::Leader` (`replay_with`), compares `team[0]` only: its position, species, level, HP and max HP. The belly is not in the trace, and the other monsters and the RNG are left out. The clock rules are the same in both modes.

**Why the full match waits for #73.** On the movement trace, every row the replay compares that differs is a monster turn: the wild monsters move and draw RNG each leader turn, and the ally follows and is attacked. The sim does not run those turns yet (enemy AI and wild movement, [#73](https://github.com/TorratDev/rust-mm/issues/73)). The leader itself matches on every compared row. The all-field match stays the gate for #73; the leader-only test is a check on the leader, not a claim that the floor replay matches.

**Floor.** The row has no floor id or tile grid that the sim can compare. The replay uses the floor the scenario gives (`--scenario`, or the test's scenario file), and a move the floor refuses shows as a leader divergence (`team[0].x` or `.y`). The tile hash is not compared (it needs the sim's walkable masks, [#366](https://github.com/TorratDev/rust-mm/issues/366)).

**Gate rows and the start.** A row is compared at the sim's clock, as the replay above describes. Before any command has run, a gate row on another clock is compared rather than reported as a `fractionalTurn` difference: the sim has not run to its first gate yet (the first row of the movement trace is on turn 0, and the first gate is 3).

**The trace-gated test.** `apps/trace-harness/tests/movement_trace.rs` runs the leader-only replay on a local trace. It skips (prints `SKIPPED`, passes) unless `RESCUE_TRACE_MOVEMENT` names the trace. With it set, the test needs:

- `RESCUE_TRACE_MOVEMENT_SCENARIO`: the scenario file for the trace (`{"dungeonId":0,"floorId":0}` for the movement trace).
- `RESCUE_ROM`, or `rom_path` in `rescue.toml`: the trace's own dump (SHA-1 `6f01887356268de39085df01490347bbcc3aa330`, see [`supported-dumps.md`](supported-dumps.md)). The test fails if it is missing or another dump.

It fails on a divergence or a sim gap, and passes with a match over every trace row. Keep the trace out of the repository. Do not commit it or anything derived from it, except the test.

**How to capture the movement trace** (from your own dump, into a folder outside the repository):

```
py scripts\capture_golden.py C:\roms\dump.nds --make-state C:\work\floor1.sav
py scripts\capture_golden.py C:\roms\dump.nds C:\work\movement.jsonl --state C:\work\floor1.sav --turn-inputs verification\v7-trace\movement_inputs.txt --frames 1700
```

Then run the test (PowerShell):

```
$env:RESCUE_ROM = "C:\roms\dump.nds"
$env:RESCUE_TRACE_MOVEMENT = "C:\work\movement.jsonl"
$env:RESCUE_TRACE_MOVEMENT_SCENARIO = "C:\work\movement.scenario.json"
cargo test -p rescue-trace-harness --test movement_trace -- --nocapture
```

The scenario file holds `{"dungeonId":0,"floorId":0}`. With `--nocapture`, a run that compared the trace prints `no leader difference over N trace rows`; a run that skipped prints `SKIPPED`. Check that it did not skip. The full replay (`rescue-trace-harness movement.jsonl --scenario ...`) still stops at the first monster row.

## Links

- Epic and tasks: [#54](https://github.com/TorratDev/rust-mm/issues/54), parent [#5](https://github.com/TorratDev/rust-mm/issues/5).
- Feeds: capture scripts [#55](https://github.com/TorratDev/rust-mm/issues/55), harness [#56](https://github.com/TorratDev/rust-mm/issues/56), replay [#80](https://github.com/TorratDev/rust-mm/issues/80).
- Dungeon layout: [`re/structs.md`](re/structs.md). RNG: [`re/rng.md`](re/rng.md). Symbols: [`symbols.md`](symbols.md).
- Legal: traces hold decoded state from the user's own dump; see [`legal.md`](legal.md). Test fixtures are synthetic.
