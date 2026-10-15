# Data tables: type chart

Issue: [#37](https://github.com/TorratDev/rust-mm/issues/37), partial. Code: `crates/data/src/typechart.rs`.

**Verification status:** the type chart is verified in Blue (V6/V8). It is read from the decoded `arm9 overlay 1` at `0x72688`, and the ROM-gated test `tests/tests/nds_rom.rs` checks every cell against the dump. Other tables are not located yet.

## Type chart

- 18 × 18 cells, `s16`, little-endian. Row = attacking type, column = defending type; 36 bytes per row (`0x24`).
- Values in this dump: 0, 1, 2 and 3 (the first row is all 2). The meaning of each value is not yet written down. Check the reader in `ov_0001::0217ba94` before naming them.
- Parsed by `data::typechart::parse_type_chart`, which takes the bytes from the table's offset.

## Not yet located

- Species base stats, learnsets, moves, items, dungeon and floor data, spawn lists and recruit rates. Red's `pmd-red` names these tables, but Blue's copies have not been found by value.
- `gTypeGummiIQBoost` and `gUnknown_810A808`: not found in Blue (see `docs/verification.md`, V6).
