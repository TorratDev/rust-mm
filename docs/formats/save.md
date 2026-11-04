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
- **[blue-verified]** `InitFlash` stores the SDK size in the global `0x020F38B4` (from `FUN_0207E6E0`). Runtime value at the title screen: `0x40000` (#197 P7.1, `save_load_probe.py` run with no save).
- **[blue-verified]** `InitFlash` stores the sector unit `0x1000` in the global `0x020F38B8` (`*DAT_0201912C = 0x1000`). Runtime value `0x1000` (same run).
- **[blue-verified]** Reads and writes go through `ReadFlashData` (`02018F0C`) and `WriteFlashData` (`02018DC4`) in 4 KiB sectors, up to 4 attempts each (source: `docs/symbols.md`, lines 184–185).

**Main save block and slots**

- **[blue-verified]** `WriteSavetoPak` (`0202A53C`) builds one block of `0x57D8` bytes and writes it twice in a row with `WriteSaveSector`, which advances the sector after each copy. Each copy takes 6 sectors (`0x6000` bytes).
- **[blue-verified]** `ReadSaveFromPak` (`0202A1AC`) reads copy A first. Copy A status 0: copy B is skipped and the sector counter advances by 6. Status 1 (read failed) or 2 (checksum mismatch): copy B is read. If the status is still 0 and the magic at `+0x414` is not `0x06061320`, the result is 4. A copy A with a bad magic does **not** fall back to copy B (#197 P1).
- **[blue-verified]** Sector map: copy A is sectors 0–5 (offset `0x0000`), copy B is sectors 6–11 (offset `0x6000`). Sector counts come from `(size + 0xFFF) / 0x1000` (`0x02029F6C`), with two `WriteSaveSector` calls in `WriteSavetoPak` (`0x0202A53C`).
- **[blue-verified]** The main block is `0x57D8` bytes (`0x0202A410`, `0x0202A73C`). All five `WriteSavetoPak` call sites start at sector 0, so there is no save slot index; the layout is fixed.
- **[blue-verified]** Sectors 12–15 and 21–30 are not used by any decompiled save routine, and sectors 32–63 are not referenced either. This is from the decompiles; Ghidra's x-ref list omitted `ReadQuickSave` and `FUN_0202B140` as callers, so it is not a full-program proof (#197 P7.6).
- **[blue-verified]** Quicksave (suspended dungeon): sectors 16–20 (`0x10` to `0x14`), 0x4800 bytes starting at sector `0x10` (`local_10 = 0x10`, 5 sectors) (`ReadQuickSave` `0x0202AFD4`, write state case 1 `0x0202B140`; size from `FUN_0206DB78`, `0x0206DB78`). Validity: `IsQuickSaveValid` (`0x0202AFC0`) reads byte `+0x0C` of the struct at `*DAT_0202AFD0`. Read state: `PrepareQuickSaveRead` (`0x0202B0B8`) builds a 16-byte struct from `MemoryAlloc(0x10, 5)`: buffer at `+4`, size at `+8`, `+0x0C` = 0, `+0` = 1.
- **[blue-verified]** Status marker at sector `0x1F`, 0x800 bytes (writer `0x0202A4A0`; readers `0x0202A0A4`, `0x0202A11C`).
- **[blue-verified]** The marker erase `FUN_0202A444` writes sector `0x1F` from an 0x800 buffer in which only the first 4 bytes are set to `0xFF` (`thunk_FUN_02021F8C(src, 0xFF, 4)`). The rest is whatever `MemoryAlloc` returns. Whether that matters depends on whether `MemoryAlloc` zeroes memory **[open]** (#197 X.1). It runs at frames 2460 and 2563 in the boot-to-dungeon probe.
- **[lead]** Marker values: `0xF1207` success, `0xF1208` invalid, `0xF1209` none (`0x0201E3C0`, `0x0202B310`, `0x0202B324`). Semantics not verified.
- **[lead]** Sector `0x10` is not erased on load; the load path (`0x0201E204`) writes only the marker.
- **[blue-verified]** The alternate branch is gated by the word at `0x020E0490`, the **session struct pointer** (not a debug flag). The literal at `0x02029EC8` (in `ReadSaveSector`) and the one at `0x02029F68` (in `WriteSaveSector`) both point at that cell. `FUN_020660E4` allocates the session struct and calls `FUN_0202A778`, which stores the pointer. `FUN_02065F88` clears it. In the boot-to-dungeon run (new game, 24,960 frames) neither fired and the pointer stayed 0. Which scene code calls `FUN_02065CFC` (through `FUN_02065E8C` and `FUN_02065EEC`) is **[open]** (#197 P7.3, P7.7). When the pointer is set, `FUN_0201D638` checks the device, and with no device `ReadSaveSector` returns status 1. Otherwise the read goes through `FUN_0201D730` and `FUN_02074780` to the bus window at `0x0A000000`, with bank switching for 0x20000-byte chips. Reading that window as GBA-slot save memory is a hypothesis, and the device identity is **[open]** (#197 P7.5).
- **Red cross-check** (`pret/pmd-red` `src/save.c`): marker sector `0x1F` and default `0xF1209` match. Red's quicksave sector was not checked.
- No local real save was available for a check. Runtime values from the #197 probes (an empty card and injected cases) are in sections 3 and 4.

**Quick-save write and buffer (#197 P3; static decompile, no runtime run)**

- **[blue-verified]** `LoadAndRunQuickSaveDungeon_Async` (`0x0201E204`) allocates the quick-save buffer as `setup+0x58`, with size `0x4800` from `FUN_0206DB78` (`0x0206DB78`). A second buffer at `setup+0x5C` has the size of the literal at `0x0206DB8C` (`0x0001AC5C`).
- **[blue-verified]** At dungeon exit `RunDungeon_Async` (`0x0206DC20`) passes the buffer to the overlay-1 serializer `ov_0001::0219B81C`, with size `0x4800`. On a fresh start (`setup+6 == 0`) byte 0 is set to 0 first. On resume (`setup+6 != 0`), `ov_0001::0219A268` restores it before the dungeon starts.
- **[blue-verified]** The stream starts with a 4-byte zero word, then nine sub-blocks, written by `0219B7E0`, `0219AB38`, `0219AA14`, `0219A9C0`, `0219B700`, `0219B5F8`, `0219B47C`, `0219BC34` and the routine behind `PTR_DAT_ov_0001__021AE3B0`. The restore discards the 4 bytes and calls the same nine readers. Sub-block sizes are not measured.
- **[blue-verified]** The write starts only in `LoadAndRunQuickSaveDungeon_Async` (call sites `0x0201E348`, `0x0201E35C`), and only when `setup+0x60` is `3` or `-2`. `FUN_0202B32C` (`0x0202B32C`) allocates a 0x14-byte state (`MemoryAlloc(0x14, 5)`) with the buffer at `+4`, the size at `+8`, and a flag at `+0xC` that is set when `setup+0x60 == -2`.
- **[blue-verified]** State machine `FUN_0202B140` (`0x0202B140`). State 1 calls `FUN_0202B5DC` (stops BGM and the icon) and `WriteSaveSector(sector 0x10, buf, 0x4800)`, stores the result at `+0x10`, and returns 1, so the caller frees both buffers. State 2 calls `WriteSavetoPak` only when `+0x10 == 0`, then shows a message. State 5 returns 3 if the result is nonzero, else 2.
- **[blue-verified] Correction:** the `+0xC` flag does **not** decide whether the main save is written. It selects only the message text.
- **[blue-verified] Risk:** the byte writer `ov_0001::0219BCA0` and the byte reader `ov_0001::0219BC48` increment the count but never compare it with the end pointer set by `0219BCEC` and `0219BC88`. A payload over `0x4800` bytes would write past the buffer. Whether the real payload fits is **[open]** (#197 P3.7; needs a runtime byte count at a dungeon exit).
- **[open]** The meaning of the setup values `1`, `2` and `4`, and of dungeon `+3` and `+6`.

**Quick-save and the global RNG state** (investigator C on [#171](https://github.com/TorratDev/rust-mm/issues/171), P2; static Ghidra only, no runtime run)

- **[lead]** Answer: a quick-save resume does not set the global LCG `sPRNGState` (`0x020DF704`). The quick-save data does not carry it, so after a resume the global LCG keeps whatever value the session had before the load. High confidence for the code path; the runtime effect is not run.
- **[blue-verified]** Writers of `0x020DF704` are `SetRNGSeed` (`0x020118F8`, `0x02011904`), `Rand16Bit` (`0x020119BC`, `0x020119C4`) and `SeedRng` (`0x02011A0C`). In the save code, `SetRNGSeed` is called only from `ReadSaveFromPak` (`0x0202A250`), the main-save reader.
- **[blue-verified]** `LoadAndRunQuickSaveDungeon_Async` (`0x0201E204`, branch `setup+6 != 0`) calls `PrepareQuickSaveRead`, a loop of `ReadQuickSave` with `MainLoops_RunFrameActions`, `IsQuickSaveValid`, `FinishQuickSaveRead`, `FUN_0202A4A0` and `LoadAndRunDungeon_Async`. None of these calls `SetRNGSeed`, `ReadSaveFromPak` or a `Rand*` function, and none writes `0x020DF704` directly.
- **[blue-verified]** `FUN_0200D4DC` (`0x0200D4DC`) is the one place the branch touches the global LCG. It runs only when `setup+0x60 == -2`. It calls `Rand32Bit()` (two LCG steps, `0x0201198C`), seeds the Mersenne Twister, and reads `unk418` through `sub_8011C34` (`0x0202A060`). See the `unk418` accessors above.
- **[blue-verified]** `setup+0x60` is set to `-2` in `RunDungeon_Async` (`0x0206DC20`), on the branch where dungeon `+6 != 0` and `0x0219C868` returns nonzero. It is set to `-1` when dungeon `+6 == 0` (#197 P3.4). So the `FUN_0200D4DC` draw happens on that resume branch.
- **[blue-verified]** The quick-save write (`FUN_0202B140`, state 1, `0x0202B1xx`) calls `FUN_0202B5DC` and `WriteSaveSector` only. It does not call `GetRNGState`. `WriteSavetoPak` (state 2) is the only caller of `GetRNGState` on this path, so the global state is saved only in the main save's `+0x420`.
- **[blue-verified]** `FUN_0202B57C`, `FUN_0202B5DC`, `FUN_0202EDAC`, `FUN_0202F280` and `FUN_0206DB78` contain no RNG accesses.
- **[lead]** Per-frame draws keep moving the global LCG during the read loop. `MainLoops_RunFrameActions` runs each frame, and the frame callback `FUN_02018ACC` (see #175 P3 and #44) draws from it. So the global state changes during the read even though the read does not set it.
- **[open]** Whether the dungeon LCG (`gDungeonRngState`, `ov_0001::021B3F58`) is in the quick-save sector. The sector routines above show no RNG accesses, but the sector contents were not decoded.
- **[open]** The resume condition in `RunDungeon_Async` for the dungeon preseed. #47 (`re/rng.md`) says the preseed is drawn once per run and not on resume; not re-checked here.

**Checksum**

- **[blue-verified]** The header's first 4 bytes (word 0) hold a checksum: the sum of the other 32-bit words in the block. `CalculateChecksum` (`02029FD4`) writes it; `ValidateChecksum` (`02029F98`) checks it.
- **[blue-verified]** `ValidateChecksum` (`0x02029F98`) returns true when word 0 differs from the sum (`return *in != sum`). `docs/symbols.md` says the same, so the return sense is settled; status 2 is a mismatch (#197 P1).
- **[blue-verified]** Word 0 is the 32-bit sum of words 1..N−1, with no complement (`CalculateChecksum`, `0x02029FD4`). A checksum mismatch makes `ReadSaveSector` (`0x02029E2C`) return status 2 (`ValidateChecksum`, `0x02029F98`).

## 2. Serialized layout

**Header** (layout from `pmd-red`; the Status column says which offsets are tied to Blue addresses)

| Offset | Size | Field | Status | Notes |
|--------|------|-------|--------|-------|
| `0x000` | 4 | checksum | [blue-verified] | Word 0; see section 1 (`02029FD4`, `02029F98`) |
| `0x004` | `0x400` | global script variables | [blue-verified] | `SaveGlobalScriptVars` `0201F884` / `RestoreGlobalScriptVars` `0201F81C`; raw copy in Red |
| `0x404` | `0x10` | game internal name | [blue-verified] | `GetGameInternalName` `0202A000`, copied by `WriteSavetoPak` (`0202A53C`) |
| `0x414` | 4 | magic | [lead] | Blue `0x06061320` (Red `0x05071412`); mismatch fails the load |
| `0x418` | 4 | `unk418` | [blue-verified] | Getter `sub_8011C34` `0202A060` (called by `WriteSavetoPak`); setter `sub_8011C40` `0202A050` (called by `ReadSaveFromPak`). Global `0x02132D98`. Struct `+0x48`. Reset to `-1`. Meaning unknown |
| `0x41C` | 4 | `unk41C` | [blue-verified] | Setter `sub_8011C28` `0202A070` (called by `ReadSaveFromPak`). Global `0x020E048C`. Struct `+0x4C`. Reset to `0`. Meaning unknown |
| `0x420` | 4 | global RNG state | [lead] | `GetRNGState` / `SetRNGSeed`; see [`re/rng.md`](../re/rng.md). Struct `+0x50` |
| `0x424` | 4 | language | [blue-verified] | Written by `WriteSavetoPak` (`0202A53C`). Reader side `SetLanguage` `0202A010` is tentative. `ReadSaveFromPak` (`0202A240`) sets `-1` when `unk41C` is 0. Struct `+0x54` |
| `0x428`–`0x448` | 4 each | section results | [lead] | One value per section; compared on load |
| `0x44C` | | section data | [lead] | Starts here |

**`unk418` and `unk41C` accessors** (investigator C on [#171](https://github.com/TorratDev/rust-mm/issues/171); static Ghidra decompiles only, no runtime run)

- **[blue-verified]** `unk418` uses one global, `0x02132D98`. `sub_8011C40` (`0202A050`) writes it through `DAT_0202A05C`. Its only writer is `0202A054`. `sub_8011C34` (`0202A060`) returns it through `DAT_0202A06C`. Its only reader is `0202A064`.
- **[blue-verified]** `unk41C` has its own global, `0x020E048C`. `sub_8011C28` (`0202A070`) writes it through `DAT_0202A07C`. Its only writer is `0202A074`. `FUN_0202A080` reads it, at `0202A090`.
- **[blue-verified]** Callers of `sub_8011C40`: `FUN_0202A954` (`0202A968`), `ReadSaveFromPak` (`0202A248`), three others in overlay 0, and `FUN_0206C6E4`. Callers of `sub_8011C34`: `WriteSavetoPak` (`0202A578`) and seven others, including `FUN_0200D4DC`. Callers of `sub_8011C28`: `FUN_0202A954`, `ReadSaveFromPak` (`0202A240`), and about a dozen others.
- **[blue-verified]** Load: `ReadSaveFromPak` calls `sub_8011C28(+0x41C)` and `sub_8011C40(+0x418)`, then `SetRNGSeed(+0x420)`. If `+0x41C` is `0`, it sets language (`+0x424`) to `-1`.
- **[blue-verified]** Save: `WriteSavetoPak` first sets `+0x41C` from its argument, `+0x418` from `sub_8011C34()`, and `+0x420` from `GetRNGState()`. It then overwrites `+0x41C`, `+0x418`, `+0x420` and `+0x424` from the game struct, so those three values are not the final saved values.
- **[blue-verified]** Reset: `FUN_0202A954` calls `sub_8011C28(0)` and `sub_8011C40(-1)`, so `unk41C` becomes `0` and `unk418` becomes `-1`.
- **[blue-verified]** `unk418` is written by `FUN_0206C6E4` case 0xF, as a random value (Rand32 times 13 struct words, then a Mersenne Twister draw, forced not to be `-1`). `FUN_0200D4DC` stores the value it reads at its struct `+0x24` (#197 P8).
- **[blue-verified]** `unk41C` is the save kind passed to `WriteSavetoPak`. `FUN_0201ED4C` and `FUN_02069848` pass 1, and the quick-save write (`FUN_0202B140`) passes 2. `RunGameMode_Async` resumes a suspended dungeon only when the value is 2 and the marker check `FUN_0202A11C` matches `DAT_0201EB78`. Otherwise it takes the error path `FUN_0200E3FC` (#197 P8.2). The label "save kind" is a hypothesis.
- **[blue-verified]** Both routines read the struct pointer at `0x020E0490`, which they dereference. Save fields map to struct offsets: `+0x418` ↔ `+0x48`, `+0x41C` ↔ `+0x4C`, `+0x420` ↔ `+0x50`, `+0x424` ↔ `+0x54`.
- **[blue-verified]** `0x020E0490` is the session struct pointer, not a debug save-state pointer. The `gUnknown_203B184` label in `symbols.md` is wrong for this cell. It is set by `FUN_020660E4` and cleared by `FUN_02065F88`, and it is 0 in the new-game boot to a dungeon (#197 P7.2, P7.3).
- **[lead]** The names `unk418` and `unk41C` are kept. Struct `+0x48` and `+0x4C` have no meaning the code supports yet.
- **[open]** The meaning of `unk418` and `unk41C`, and of struct `+0x48` and `+0x4C`. The effect of the zero check on `+0x41C` beyond the language reset. Readers of `0x02132D98` and `0x020E048C` outside the x-ref lists above (the lists cover only the imported program, not runtime paths).

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
- **[blue-verified]** Read helpers (#197 P2). `FUN_02006140` reads the dungeon location as two 7-bit fields into record `+4` (x) and `+5` (y). `FUN_0200389C` is not a level reader: it is `ReadBits(…, 7)` through the pointer at `0x020038A8` (value `0x0200C050`), used for `unkC[0]` and `unkC[1]` at record `+8` and `+9`. The record's level is at `+3`, read directly by `FUN_02003CD8`. `FUN_020094A0` reads four 6-byte move records: `+0` 4 bits (`moveFlags`), `+2` 9 bits (move id), `+4` 7 bits (PP), the mirror of `WritePoke1MovesBits` (`0x020094D8`). Held item `ReadHeldItemBits` (`0x0200687C`) is decompiled but not re-read here.
- **[open]** Move bytes `+1` (`moveFlags2`) and `+5` (`ginseng`) are neither written nor read by the save code. `FUN_02003CD8` zeroes the record, so both are 0 after a load. Whether they are ever nonzero at save time is not checked (#197 P2c). Whether the largest move id fits in 9 bits is not checked either.
- **[open]** The meaning of the `unkC` fields. The names are hypotheses.
- **[open]** Meaning of the record fields (species, level, item, moves). Names are inferred only from offsets and widths.
- **[blue-verified]** Team block: 4 entries, each a 1-bit flag then `WritePoke1Bits`; 4 × 324 bits (`SaveRecruitedPokemon`, second loop).
- **[blue-verified] Flag asymmetry:** the writer stores `flags & 1`; the reader sets `3` or `0`. Writer loop at `+0x6890`; reader second loop. The round trip is exact only for values 0 and 3: a flags value 1 comes back as 3, and 2 as 0. Whether the game stores other values is **[open]** (#197 P6.3). The team block is the flags words of records 418–421 (`0x6890 = 418 × 0x40`).
- **[blue-verified]** Team-index list: six 16-bit entries, then one 16-bit leader index. The writer sets bit 2 on each listed record and byte `+2` = 1 on the leader (`SaveRecruitedPokemon` tail; `RestoreRecruitedPokemon` tail).
- **[blue-verified]** Valid bit: on load it is rebuilt from byte `+3` being nonzero (`FUN_02003CD8`, first lines). The writer (`0x0200409C`, line 36) zeroes byte `+3` in the live record for invalid roster records, so the save changes live memory. A record with `flags & 1` set and `+3 == 0` is written with `+3 = 0` and loads as invalid, so it does not round-trip (#197 P6.4). Whether valid records with level 0 occur in play is **[open]**. Team records 418–421 take validity from their 1-bit field, so a zero level does not change them, and the reader still overwrites their flags with 3 or 0 (#197 P6.5).
- **[lead]** Stream size: 413 × 323 + 4 × 324 + 112 = 134,807 bits = 16,851 bytes of the 0x4650 (18,000) section. Derived arithmetic.
- **[lead]** The section's header result is 7 (bit index after the last write). Not checked against a save.

**Team inventory** (`SaveTeamInventory` `020069C8`, `RestoreTeamInventory` `020068AC`)

- **[blue-verified]** 20 item slots × 23 bits, slot bytes 8, 7, 8 (`WriteItemSlotBits` `020067CC`); symmetric with `ReadItemSlotBits` (`0200680C`).
- **[blue-verified]** Storage: 240 counts × 10 bits, from the ushort at `+0x3C`.
- **[blue-verified]** Kecleon shop: 8 entries × 15 bits (`WriteHeldItemBits` 8 + 7), from `+0x21C`.
- **[blue-verified]** Kecleon wares: 4 entries × 15 bits, from `+0x22C`.
- **[blue-verified]** Money: 24 bits at `+0x234`; savings: 24 bits at `+0x238`.
- **[lead]** Total: 3,088 bits (386 bytes) of the `0x1D8`-byte section (arithmetic from the widths).
- **[blue-verified]** The bank is the savings account. Script id `0x24` `BANK_GOLD` (flag table A, entry 36) reads and writes `teamSavings` at `+0x238`, through `FUN_02020D90` (case 8) and the setters `FUN_020208E8` and `FUN_02020B48`. Script id `0x23` `GOLD` is `teamMoney` at `+0x234`. In-game dialogue names a bank ("Felicity Bank" and "Snobilikat-Bank" in overlays 11, 12 and 18). The 240-entry block is item storage: no script id maps to it, and the string search found no name for it as a bank (#197 P9). The `+0x238` base pointer, `gTeamInventoryRef` (`0x020DECF8`), is not re-checked here.

**Rescue team info and adventure data** (`SaveRescueTeamInfo` `02008758`, `SaveAdventureData` `02010F4C`)

- **[blue-verified]** Rescue team info widths: 0x58 bits from the struct at `*DAT_020087E8`, 0x20 bits at `+0x0C`, and a 1-bit flag at `+0x10` from `FUN_0201F994(0)`. Meanings **[open]**.
- **[lead]** Rescue team info is 121 bits in total (arithmetic from the widths).
- **[lead]** pmd-red: `learnedMoves` is 13 x 32 bits; not yet matched to a Blue sub-block.
- **[blue-verified]** Adventure sub-block widths: 15 bits (`FUN_0200C870`, `0x0200C870`), 32 bits (`FUN_0200CD64`, `0x0200CD64`), 1,463 bits (`FUN_02010520`, `0x02010520`), and `FUN_020112C8` (`0x020112C8`). Meaning of the flag arrays **[open]**.
- **[lead]** `FUN_020112C8` is 532 bits (arithmetic from the widths): 1 + 424 + 64 + 31 + 12 one-bit flags, indexed by ID through `FUN_0201151C`, `FUN_02011570`, `FUN_02011470`.
- **[lead]** Adventure data is 2,042 bits in a 2,048-bit section (arithmetic from the widths).
- **[blue-verified]** `DAT_02010ACC` is the serialized struct. The cells `DAT_02010ACC`, `DAT_02010648`, `DAT_02010E18` and `DAT_02010DF8` all hold `0x020DF6FC` (static cell values; the runtime value of that cell was not read). `FUN_02010520` serializes through `*DAT_02010648`, and `FUN_0201064C` writes through `*DAT_02010ACC`, so both are the same struct (#197 P5.1).
- **[blue-verified]** `FUN_0201064C` (`0x0201064C`) runs before the adventure write. It ORs species bits (the id split by 32, into the word at `+0x18 + 4·(id/32)`) and move bits (into `+0x88`) from each roster record with `flags & 1`. It sets the `+0x16` count (short) and calls the unlock setter `FUN_02010DFC` for IDs `0x0D`, `0x0E`, `0x0F`, `0x10`–`0x1F` and `0x07`. Every write lands in a serialized field, so the save changes the live adventure struct (#197 P5.2, P5.3). Repeated saves give the same bits, by the code's ORs and recount. This is argued from the code, not run (**[hypothesis]**, #197 P5.4).
- **[open]** The meaning of the unlock IDs, of `FUN_02008FC8` (which gates unlock `0x0D`), and of the excluded IDs (#197 P5).

**Other sections** (#197 P8; decompile only, widths blue-verified, meanings open)

- **4B98 sub-block** (section `0x4B98`, 0x594 bytes; save `sub_8095624` `0x0200D0CC`, restore `sub_80954CC` `0x0200D2B8`). 32 records of 0x30 bits through `sub_8095824` (`0x0200CEEC`), then one record at `*DAT_0200D2B0` with the same shape as a Pokémon record (its moves through `FUN_020094A0`, its held item through `WriteHeldItemBits`), then 0x20 bits and 32 words of 0x20 bits. Restore mirrors the save field by field.
- **Friend areas** (section `0x4A90`): 58 one-bit flags, one per element of a byte array. Save `SaveFriendAreas` `0x02008D30`, restore `ReadSavedFriendAreas` `0x02008CB0`. Symmetric by decompile.
- **Mail** (section `0x512C`): 4 + 8 + 8 items at stride 0x14, 56 + 1 flag bits, 0x140 and 0x3C0 bit blocks, and 16 entries at stride 0xC. Save `SaveMailInfo` `0x0200DB80`, restore `RestoreMailInfo` `0x0200DD5C`. Symmetric by decompile. The item width of `FUN_0200DA08` is not decoded.
- The restores return the bit position from the struct's trailing field. This is how the section result is compared, not a bug.

**Story flags** (`SaveGlobalScriptVars` `0201F884`, `RestoreGlobalScriptVars` `0201F81C`)

- **[blue-verified]** The 0x400-byte script-variable block is copied raw (`thunk_FUN_02021F5C`).
- **[blue-verified]** The debug asserts are empty in retail: `FUN_02029D8C` and `FUN_02029DAC` decompile to a bare `return`. The bounds checks in `FUN_02021108` and in the getters and setters (`FUN_02020D90`, `FUN_020208E8`, `FUN_02020B48`, `FUN_02020F74`) are therefore not run, so an out-of-range flag index is not caught (#197 P4a).
- **[blue-verified]** Descriptor tables of 16-byte entries: table A (83 entries at `0x02098060`, base word `0x021EE640`) and table B (index `0x400` and up; 4 valid entries by the layout, medium confidence). Entry: `+0` type (1 bit, 2 or 7 byte, 3 s8, 4 u16, 5 s16, 6 s32, 8 special); `+4` offset in the block; `+6` bit shift; `+8` count; `+0xA` a value read only by `RestoreGlobalScriptVars`; `+0xC` pointer to the debug name (#197 P4b).
- **[blue-verified]** The only check that runs is in `RestoreGlobalScriptVars` (`0x0201F81C`). It compares `VERSION` (table A id 0, expected 30 at `+0xA`) with the first word of the block, and a mismatch sets the load result to 4. It runs only in the branch of `ReadSaveFromPak` where `*DAT_0202A41C == 0`. The other branch copies the block with no check. So `VERSION == 30` is the only check, and only on that branch (#197 P4c). The meaning of 30 is not confirmed against a real save.

## 3. Corrupt or blank detection

**Load result** (`ReadSaveFromPak` `0202A1AC`; the codes are Blue's, tied to addresses where marked)

| Value | Meaning | Status |
|-------|---------|--------|
| 0 | valid | [lead]; the valid-copy runs are not done (#197 P1.5) |
| 1 | read failed: `ReadFlashData` gave up after 4 tries (its return 3), or the alternate branch failed. Copy B is tried | [blue-verified] code; a forced status 1 leads to the state-5 stall (section 4) |
| 2 | checksum mismatch (`ValidateChecksum` returns true on mismatch). Copy B is tried | [blue-verified] |
| 3 | a section's restore result does not match the header | [lead] |
| 4 | magic mismatch after the copy-A check, with no copy-B fallback; or `VERSION != 30` in `RestoreGlobalScriptVars` (first branch only) | [blue-verified] |

**Blank and zeroed cards**

- **[blue-verified runtime]** Blank card (no save; DeSmuME RAM bytes `FF`): both copies return status 2, the boot read result is 2, and the state machine goes 2 → 4 → 6 at frame 2473 with no message (#197 P1.3). This is DeSmuME's empty backup, not a real card.
- **[blue-verified, injected]** Zeroed copy A (zeros written at the `ReadSaveSector` return, so a fault injection and not a real card): the checksum passes (word 0 = 0 = sum), `ReadSaveFromPak` returns 4 from the magic check, and copy B is not read (#197 P1.4). A real all-zero card is not tested: importing a zeroed backup file did not reach the game in this build.
- **[open]** Whether a failed copy A followed by a failed copy B is reported as a read failure, and what the game does then.

**Sector read** (`ReadSaveSector` `0x02029E2C`)

- **[blue-verified]** Status 0 is valid. Status 1 is read failed: `ReadFlashData` returned 3 after its 4 tries, or the alternate branch returned 1 with no device. Status 2 is a checksum mismatch (`ValidateChecksum` `0x02029F98`, which returns true on mismatch).
- The `ReadSaveFromPak` table above stays [lead] until its codes are tied to addresses.

## 4. Open runtime checks

These need a run on Blue with `py-desmume` and the scripts in `verification/v7-trace/`. Outputs go to a scratch directory outside the repository; only the hash and the command are recorded in notes. The runs and their results are in [`verification.md`](../verification.md#197--save-load-and-session-runs).

- **[blue-verified runtime]** Load states from `save_load_probe.py`: a blank card (no save), an injected zeroed copy A, and a forced status 1 on both copies. Section 3 has the results.
- **[open]** Copy A valid, copy A bad with copy B valid, and both bad, with real data (#197 P1.5). No valid save image is available. A real save is user data and stays local.
- **[blue-verified runtime]** The state-5 stall: forced status 1 moves `FUN_0202AA14` from state 2 (mode 1) to state 5 at frame 2459, and state 5 has no exit, so the call repeats (1,543 more calls to frame 4000). The real trigger of status 1 is **[open]**: a flash read failure, or the alternate branch failing (P7).
- **[blue-verified runtime]** Size global `0x020F38B4` = `0x40000` and sector unit `0x020F38B8` = `0x1000` at the title screen.
- **[open]** The alternate read branch: which scene sets the session struct pointer in play, and whether a device is ever present (#197 P7.3, P7.5, P7.7). The boot-to-dungeon run did not enter it.
- **[open]** Friend areas, mail, `unk418` and `unk41C`: the layouts are read (section 2), but their meanings and runtime values beyond the title screen are not checked.
- **[open]** Marker erase (X.1): whether `MemoryAlloc` zeroes the buffer that the marker erase writes.
- **[open]** Quick-save payload size: whether the dungeon payload fits in `0x4800` bytes (#197 P3.7). Needs a runtime byte count at a dungeon exit.
- **[open]** P6.4 (a valid roster record at level 0), P6.3 (team flag values other than 0 and 3), P2c (`moveFlags2` and `ginseng`), P5 (the unlock meanings), and P8 (the meanings of the 4B98, friend-area, mail and `unk418` fields).
- **[open]** Check the whole layout on a real save file (local only, never committed).

## Links

- Symbols: [`symbols.md`](../symbols.md)
- Verification: [`verification.md`](../verification.md)
- RNG state: [`re/rng.md`](../re/rng.md)
- Legal: [`legal.md`](../legal.md)
