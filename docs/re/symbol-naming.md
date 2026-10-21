# Symbol naming and sharing

**Date:** 2026-10-18
**Status:** blue-verified
**Subsystem:** symbols (naming rules, #42)

The naming rules are in [`symbols.md`](../symbols.md). This note records the state the rules were written against. No Ghidra project and no code changed in #42.

## Addresses
| Address | Name | Source |
|---------|------|--------|
| 02029CF4 | NDS_DebugPrintf | `symbols.md` row, type `-`; Ghidra has a function here (open, see below) |
| ov_0001::021AAEE8 | gTypeEffectivenessChart | `symbols.md` row; no Ghidra symbol |
| ov_0001::021AA588 | gEffectivenessChart | `symbols.md` row; no Ghidra symbol |

## Evidence
- Overlay source for the Ghidra import: `manifest.json`, written by `tools/nds_extract_code.py` and read by `tools/ghidra/RescueNdsImport.java` (the docstring of `tools/nds_extract_code.py` lists its fields). `memory_map_json` in `crates/nds/src/binary.rs:103` is not the source. Its overlay entries carry `id`, `ram_addr`, `ram_size`, `bss_size`, `file_id` and `compressed`, with no ROM offset and no file size.
- `docs/symbols.md` has 218 rows: 198 function rows and 20 others. Checked by counting the table in the repo.
- The `sub_` rows: 14 (7 in overlay 1, 7 in the ARM9 binary). Their names come from `pmd-red` addresses. Checked by listing the table.
- Ghidra state (investigators' findings, not re-run here): the 198 function rows have no names applied in any copy. `rescue-arm9`, `-a`, `-b` and `-c` each hold 5,284 functions, and none has the names from the table.
- Ghidra state: 2 data rows have no Ghidra symbol: `gTypeEffectivenessChart` and `gEffectivenessChart`.
- Symbol count: two counts differ by one, 58,799 and 58,800 (investigators' findings). The cause is not explained.
- ghidra-cli: the V6 type-chart read fails on the `ov_` address form. This is an open ghidra-cli issue; it was not fixed here.

## Red comparison
The `sub_` names are Red's addresses. Blue's functions at those addresses are not yet checked, so the names stay as they are. [red-hypothesis]

## Open questions
- `NDS_DebugPrintf` (`02029CF4`): the row stays as it is, typed `-`. Ghidra has a function at that address, so the row is not yet complete.
- The 14 `sub_` rows are unverified until Blue's functions are checked.
- The 58,799 vs 58,800 symbol-count difference is unexplained.
- Applying the table to Ghidra (`RescueApplySymbols.java`) is out of scope for #42.
- ghidra-cli does not read the `ov_` address form for the V6 type-chart read. Fixing address parsing is out of scope for #42.
- Prose still writes some ARM9 addresses as `FUN_` names (for example `FUN_02028628` in [`formats/sprites.md`](../formats/sprites.md) and `FUN_02002288` in [`data-tables.md`](data-tables.md)). They are not yet in the canonical form.
- The four RNG functions in [`glossary.md`](../glossary.md) have no `symbols.md` row yet.

## Links
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
- Glossary: [glossary.md](../glossary.md)
