# Save format

Blue's main save: the backup media, how the data is laid out on it, how it is checked, and what is still open. Issue: [#53](https://github.com/TorratDev/rust-mm/issues/53). V4 in [`verification.md`](../verification.md#v4--non-audio-formats-and-save-layout). Function names and addresses are in [`symbols.md`](../symbols.md).

These are research notes. Nothing here has been checked against a real save file; real saves are user data and are never committed ([`legal.md`](../legal.md)).

## Tags

- **[blue-verified]**: a fact tied to an address in Blue's ARM9. Only facts with an address get this tag.
- **[lead]**: a name, layout or reading taken from `pmd-red` (`src/save.c`, `src/data_serializer.c`) or from an investigator's notes that is not yet confirmed in Blue code (it may cite an address). Not a fact.
- **[open]**: not known. Needs a Blue address or a runtime check.

`pmd-red` is cited by function name only; no code is copied ([`legal.md`](../legal.md)).

## 1. Media, size, slots, checksum

**Media and sector size**

- **[blue-verified]** `InitFlash` (`020190CC`) identifies the backup chip by calling `CARD_IdentifyBackup(0x1202)` (`0207E598`). The literal `0x00001202` is read from `0x02019124`.
- **[blue-verified]** `0x1202` decodes as a 2 Mbit (256 KiB) flash chip. The low byte `0x02` selects the flash family (`FUN_0207E3A8`, `0x0207E3A8`, branch `param_1 == 2`). The size is `1 << ((type >> 8) & 0xFF)` = `0x40000` (same function, `0x40000` branch). The SDK size is stored at `+0x18` of the backup struct and read by `FUN_0207E6E0` (`0x0207E6E0`).
- **[open]** The size global `0x020F38B4` holds the SDK size at runtime. Its value was not read (not readable statically).
- **[blue-verified]** `InitFlash` stores the sector unit `0x1000` in a global (`*DAT_0201912C = 0x1000`, `0x020F38B8`). The global's runtime value was not read.
- **[blue-verified]** Reads and writes go through `ReadFlashData` (`02018F0C`) and `WriteFlashData` (`02018DC4`) in 4 KiB sectors, up to 4 attempts each (source: `docs/symbols.md`, lines 184–185).

**Main save block and slots**

- **[blue-verified]** `WriteSavetoPak` (`0202A53C`) builds one block of `0x57D8` bytes and writes it twice in a row with `WriteSaveSector`, which advances the sector after each copy. Each copy takes 6 sectors (`0x6000` bytes).
- **[blue-verified]** `ReadSaveFromPak` (`0202A1AC`) reads copy A first; if that read fails, it reads copy B.
- **[blue-verified]** Sector map: copy A is sectors 0–5 (offset `0x0000`), copy B is sectors 6–11 (offset `0x6000`). Sector counts come from `(size + 0xFFF) / 0x1000` (`0x02029F6C`), with two `WriteSaveSector` calls in `WriteSavetoPak` (`0x0202A53C`).
- **[blue-verified]** The main block is `0x57D8` bytes (`0x0202A410`, `0x0202A73C`). All five `WriteSavetoPak` call sites start at sector 0, so there is no save slot index; the layout is fixed.
- **[lead]** Sectors 12–15 and 21–30 are unused. No code addresses them (x-ref search of all sector writers only; medium confidence).
- **[blue-verified]** Quicksave (suspended dungeon): sector `0x10`, 0x4800 bytes, 5 sectors (`ReadQuickSave` `0x0202AFD4`, write state case 1 `0x0202B140`; size from `FUN_0206DB78`, `0x0206DB78`). Validity: `IsQuickSaveValid` (`0x0202AFC0`) reads byte `+0x0C` of the struct at `*DAT_0202AFD0`. Read state: `PrepareQuickSaveRead` (`0x0202B0B8`) builds a 16-byte struct from `MemoryAlloc(0x10, 5)`: buffer at `+4`, size at `+8`, `+0x0C` = 0, `+0` = 1.
- **[blue-verified]** Status marker at sector `0x1F`, 0x800 bytes (writer `0x0202A4A0`; readers `0x0202A0A4`, `0x0202A11C`).
- **[lead]** Marker values: `0xF1207` success, `0xF1208` invalid, `0xF1209` none (`0x0201E3C0`, `0x0202B310`, `0x0202B324`). Semantics not verified.
- **[lead]** Sector `0x10` is not erased on load; the load path (`0x0201E204`) writes only the marker.
- **[open]** Alternate read branch when the flag at `0x02029EC8` is nonzero (`0x02029E2C`, `0x0201D638`, `0x0201D730`). Not analysed.
- **Red cross-check** (`pret/pmd-red` `src/save.c`): marker sector `0x1F` and default `0xF1209` match. Red's quicksave sector was not checked.
- No local real save was available for a check. Runtime values were not read.

**Checksum**

- **[blue-verified]** The header's first 4 bytes (word 0) hold a checksum: the sum of the other 32-bit words in the block. `CalculateChecksum` (`02029FD4`) writes it; `ValidateChecksum` (`02029F98`) checks it.
- **[open]** `docs/symbols.md` (line 190) describes `ValidateChecksum` as returning true when word 0 *differs* from the sum. That reads as inverted, so the return sense must be checked before relying on it (see section 3).
- **[blue-verified]** Word 0 is the 32-bit sum of words 1..N−1, with no complement (`CalculateChecksum`, `0x02029FD4`). A checksum mismatch makes `ReadSaveSector` (`0x02029E2C`) return status 2 (`ValidateChecksum`, `0x02029F98`).

## 2. Serialized layout

**Header** (layout from `pmd-red`; the Status column says which offsets are tied to Blue addresses)

| Offset | Size | Field | Status | Notes |
|--------|------|-------|--------|-------|
| `0x000` | 4 | checksum | [blue-verified] | Word 0; see section 1 (`02029FD4`, `02029F98`) |
| `0x004` | `0x400` | global script variables | [blue-verified] | `SaveGlobalScriptVars` `0201F884` / `RestoreGlobalScriptVars` `0201F81C`; raw copy in Red |
| `0x404` | `0x10` | game internal name | [blue-verified] | `GetGameInternalName` `0202A000`, copied by `WriteSavetoPak` (`0202A53C`) |
| `0x414` | 4 | magic | [lead] | Blue `0x06061320` (Red `0x05071412`); mismatch fails the load |
| `0x418` | 4 | `unk418` | [blue-verified] | Accessors `sub_8011C34` `0202A060` (written by `WriteSavetoPak`) and `sub_8011C40` `0202A050` (read by `ReadSaveFromPak`). Meaning unknown |
| `0x41C` | 4 | `unk41C` | [blue-verified] | Accessor `sub_8011C28` `0202A070` (read by `ReadSaveFromPak`). Meaning unknown |
| `0x420` | 4 | global RNG state | [lead] | `GetRNGState` / `SetRNGSeed`; see [`re/rng.md`](../re/rng.md) |
| `0x424` | 4 | language | [blue-verified] | Written by `WriteSavetoPak` (`0202A53C`). Reader side `SetLanguage` `0202A010` is tentative. `-1` when `unk41C` is 0 is [lead] |
| `0x428`–`0x448` | 4 each | section results | [lead] | One value per section; compared on load |
| `0x44C` | | section data | [lead] | Starts here |

**Sections** (offsets from `0x44C`; order and sizes from the save/restore pairs in `symbols.md`, except where marked [lead])

| Offset | Size | Contents | Restore (read) / save (write), with addresses | Status |
|--------|------|----------|-----------------------------------------------|--------|
| `0x0000` | `0x4650` | recruited Pokémon | `RestoreRecruitedPokemon` `02003F2C` / `SaveRecruitedPokemon` `0200409C` | [blue-verified] pair; size is [lead] (not in `symbols.md`) |
| `0x4650` | `0x258` | `Poke2` records (150 × 4) | `RestorePoke2s` `020038AC` / `SavePoke2s` `02003AD0` | [blue-verified] |
| `0x48A8` | `0x1D8` | team inventory | `RestoreTeamInventory` `020068AC` / `SaveTeamInventory` `020069C8` | [blue-verified] |
| `0x4A80` | `0x10` | rescue team info | `ReadRescueTeamInfo` `020086CC` / `SaveRescueTeamInfo` `02008758` | [blue-verified] |
| `0x4A90` | `0x8` | friend areas | `ReadSavedFriendAreas` `02008CB0` / `SaveFriendAreas` `02008D30` | [blue-verified] |
| `0x4A98` | `0x100` | adventure data | `RestoreAdventureData` `02010EFC` / `SaveAdventureData` `02010F4C` | [blue-verified] |
| `0x4B98` | `0x594` | unnamed in Red | `sub_80954CC` `0200D2B8` / `sub_8095624` `0200D0CC` | [blue-verified] |
| `0x512C` | `0x221` | mail | `RestoreMailInfo` `0200DD5C` / `SaveMailInfo` `0200DB80` | [blue-verified] |

The sections end at `0x534D`, inside the `0x57D8` block. The restore function for each section must return the value stored in its header slot.

**Bit packing** (`pmd-red` `src/data_serializer.c`)

- **[lead]** Every section except the script variables is bit-packed. `WriteBits` copies the low `n` bits of a value, least significant bit first, filling each byte from bit 0. `ReadBits` is the reverse.
- **[lead]** A section's header result is the bit position (0–7) inside its last byte when the section ends. On load, the reader must end at the same position.
- **[lead]** Multi-byte values are written low byte first.

**Field widths** (an investigator compared each section's `WriteBits` widths in Blue's code against the `pmd-red` functions; the result is reported as matching, but the comparison is not final and no Blue address is attached to the list)

- **[lead]** `Poke2` records, friend areas, mail and rescue records, mail info: reported as the same widths in both.
- **[lead]** Script variables: a raw 0x400-byte copy in both. The copy is confirmed in Blue (see below); the validation target is open.
- **Recruited Pokémon, rescue team info, adventure data, team inventory:** confirmed from Blue's code. See the subsections below.

**Recruited Pokémon records** (`SaveRecruitedPokemon` `0200409C`, `RestoreRecruitedPokemon` `02003F2C`)

- **[blue-verified]** A record is 0x40 bytes in RAM; the save is a bit stream, not byte fields (stride `0x40` in the writer loop, `0200409C`).
- **[blue-verified]** Global pointer to the record array: `020DDF84`. Record count 413 (0x19D): `02004094`, `0200421C`. Team block offset `0x6890`: `02004098`, `02004220`.
- **[blue-verified]** Field widths in writer order (`WritePoke1Bits` `02003E1C`; helpers `0200617C`, `0200684C`, `020094D8`, `02009550`, `0200388C`). Decompiled per helper. Level helper calls `WriteBits` through a pointer at `0x02003898` (value `0x0200C0F4`):
  - `+3` (7 bits); `+6` (9); `+4` and `+5` (7 each, dungeon location); `+8` (7); `+9` (7); `+10` (10); `+12` (10); `+14` (8); `+15` (8); `+16` (8); `+17` (8); `+20` (24); `+24` (24); `+27` (4); `+28` (8) and `+29` (7), the held item; moves at `+30` (4 × 20 bits, stride 6); `+54` (80).
- **[lead]** Record is 323 bits. Arithmetic from the widths above (7+9+14+7+7+10+10+8+8+8+8+24+24+4+15+80+80).
- **[blue-verified]** The reader mirrors the writer for every field: `FUN_02003CD8`, called from `RestoreRecruitedPokemon` (`02003F2C`).
- **[open]** Read helpers not decompiled: dungeon location `FUN_02006140` (`0x02006140`), level `FUN_0200389C` (`0x0200389C`), moves `FUN_020094A0` (`0x020094A0`). Held item `ReadHeldItemBits` (`0x0200687C`) is decompiled but not re-read here.
- **[open]** Meaning of the record fields (species, level, item, moves). Names are inferred only from offsets and widths.
- **[blue-verified]** Team block: 4 entries, each a 1-bit flag then `WritePoke1Bits`; 4 × 324 bits (`SaveRecruitedPokemon`, second loop).
- **[blue-verified] Flag asymmetry:** the writer stores `flags & 1`; the reader sets `3` or `0`. Writer loop at `+0x6890`; reader second loop.
- **[blue-verified]** Team-index list: six 16-bit entries, then one 16-bit leader index. The writer sets bit 2 on each listed record and byte `+2` = 1 on the leader (`SaveRecruitedPokemon` tail; `RestoreRecruitedPokemon` tail).
- **[blue-verified]** Valid bit: on load it is rebuilt from byte `+3` being nonzero (`FUN_02003CD8`, first lines); the writer zeroes byte `+3` for invalid records. A valid record with byte `+3` = 0 would not round-trip. The effect on real data is **[open]**.
- **[lead]** Stream size: 413 × 323 + 4 × 324 + 112 = 134,807 bits = 16,851 bytes of the 0x4650 (18,000) section. Derived arithmetic.
- **[lead]** The section's header result is 7 (bit index after the last write). Not checked against a save.

**Team inventory** (`SaveTeamInventory` `020069C8`, `RestoreTeamInventory` `020068AC`)

- **[blue-verified]** 20 item slots × 23 bits, slot bytes 8, 7, 8 (`WriteItemSlotBits` `020067CC`); symmetric with `ReadItemSlotBits` (`0200680C`).
- **[blue-verified]** Storage: 240 counts × 10 bits, from the ushort at `+0x3C`.
- **[blue-verified]** Kecleon shop: 8 entries × 15 bits (`WriteHeldItemBits` 8 + 7), from `+0x21C`.
- **[blue-verified]** Kecleon wares: 4 entries × 15 bits, from `+0x22C`.
- **[blue-verified]** Money: 24 bits at `+0x234`; savings: 24 bits at `+0x238`.
- **[lead]** Total: 3,088 bits (386 bytes) of the `0x1D8`-byte section (arithmetic from the widths).
- **[lead]** "Bank" = the 240-entry storage block. No other bank in the save path; name search only.

**Rescue team info and adventure data** (`SaveRescueTeamInfo` `02008758`, `SaveAdventureData` `02010F4C`)

- **[blue-verified]** Rescue team info widths: 0x58 bits from the struct at `*DAT_020087E8`, 0x20 bits at `+0x0C`, and a 1-bit flag at `+0x10` from `FUN_0201F994(0)`. Meanings **[open]**.
- **[lead]** Rescue team info is 121 bits in total (arithmetic from the widths).
- **[lead]** pmd-red: `learnedMoves` is 13 x 32 bits; not yet matched to a Blue sub-block.
- **[blue-verified]** Adventure sub-block widths: 15 bits (`FUN_0200C870`, `0x0200C870`), 32 bits (`FUN_0200CD64`, `0x0200CD64`), 1,463 bits (`FUN_02010520`, `0x02010520`), and `FUN_020112C8` (`0x020112C8`). Meaning of the flag arrays **[open]**.
- **[lead]** `FUN_020112C8` is 532 bits (arithmetic from the widths): 1 + 424 + 64 + 31 + 12 one-bit flags, indexed by ID through `FUN_0201151C`, `FUN_02011570`, `FUN_02011470`.
- **[lead]** Adventure data is 2,042 bits in a 2,048-bit section (arithmetic from the widths).
- **[lead]** `FUN_0201064C` (`0x0201064C`), called from `FUN_02010520`, runs before the adventure write and ORs bits into arrays at offsets `0x18` and `0x88` (struct `DAT_02010ACC`), plus unlock calls through `FUN_02010DFC` (IDs `0x0E–0x1F` and 7). The save has a side effect. Whether `DAT_02010ACC` is the struct that is serialized (`DAT_02010648`) is not verified.

**Story flags** (`SaveGlobalScriptVars` `0201F884`, `RestoreGlobalScriptVars` `0201F81C`)

- **[blue-verified]** The 0x400-byte script-variable block is copied raw (`thunk_FUN_02021F5C`).
- **[open]** Validation: a short at `+10` of a struct from `FUN_02021108`, compared with an int through `local_c`; a mismatch calls `FUN_02029D8C`.

## 3. Corrupt or blank detection

**Load result** (`ReadSaveFromPak`; values from `pmd-red`'s `ReadSaveStatus`, Blue's codes not yet tied to addresses)

| Value | Meaning | Status |
|-------|---------|--------|
| 0 | valid | [lead] |
| 1 | read failed (then copy B is tried) | [blue-verified] for the copy-B fallback at `0202A1AC`; the code value is [lead] |
| 2 | checksum error | [lead] |
| 3 | a section's restore result does not match the header | [lead] |
| 4 | wrong magic, or the script variables were rejected | [lead] |

**Blank and zeroed cards**

- **[lead]** Erased flash fails the checksum, so there is no separate blank check (`ReadSaveSector` `0x02029E2C`, `CalculateChecksum` `0x02029FD4`). Derived from the code: the sum of erased words is not `0xFFFFFFFF` for N ≠ 2. Not run.
- **[open]** How a zeroed card is handled. An all-zero block has checksum word 0 equal to the sum (0), so whether it passes depends on the return sense of `ValidateChecksum` (section 1). The magic check is the likely catch. Not verified.
- **[open]** Whether a failed copy A followed by a failed copy B is reported as a read failure, and what the game does then.

**Sector read** (`ReadSaveSector` `0x02029E2C`)

- **[blue-verified]** Status 0 is valid, 1 is read failed, and 2 is checksum error (a mismatch, via `ValidateChecksum` `0x02029F98`).
- The `ReadSaveFromPak` table above stays [lead] until its codes are tied to addresses.

## 4. Open runtime checks

These need a run on Blue with `py-desmume` and the scripts in `verification/v7-trace/`. Outputs go to an ignored scratch directory; only the hash and the command are recorded in notes. None of these has been run for this file.

- **[open]** Load states: what the game does with no save, copy A valid, copy A bad and copy B valid, and both bad.
- **[open]** The state-5 stall during load. Cause not known.
- **[open]** Blank card and zeroed card cases (see section 3).
- **[open]** Friend areas, mail, `unk418` and `unk41C` are not covered by any check yet.
- **[open]** `FUN_0201064C`: which struct it writes, to confirm the target layout.
- **[open]** The alternate read branch when the flag at `0x02029EC8` is nonzero (`0x02029E2C`, `0x0201D638`, `0x0201D730`).
- **[open]** Read the runtime values of the size global `0x020F38B4` and the sector global `0x020F38B8` on a running game. Neither is readable statically.
- **[open]** Check the whole layout on a real save file (local only, never committed).

## Links

- Symbols: [`symbols.md`](../symbols.md)
- Verification: [`verification.md`](../verification.md)
- RNG state: [`re/rng.md`](../re/rng.md)
- Legal: [`legal.md`](../legal.md)
