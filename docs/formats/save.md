# Save format

Blue's main save: the backup media, how the data is laid out on it, and how it is checked. Issues: [#53](https://github.com/TorratDev/rust-mm/issues/53), V4 in [`verification.md`](../verification.md#v4--non-audio-formats-and-save-layout). Function names and addresses are in [`symbols.md`](../symbols.md).

Found by reading Blue's ARM9 code, matched to `pmd-red`'s `src/save.c` (same functions, same header fields and the same eight sections in the same order). Not yet checked against a real save file; real saves are user data and are never committed ([`legal.md`](../legal.md)).

## Media

`InitFlash` (`020190CC`), called from the boot code, identifies the backup chip with NitroSDK's `CARD_IdentifyBackup(0x1202)` (`0207E598`). In NitroSDK's encoding (device type in the low byte, log2 of the size in the next), `0x1202` is **`CARD_BACKUP_TYPE_FLASH_2MBITS`: 2 Mbit (256 KiB) serial flash**. The size reported by the SDK is stored at `0x020F38B4`.

Reads and writes go through `ReadFlashData` (`02018F0C`) and `WriteFlashData` (`02018DC4`), in **4 KiB sectors** (`0x1000`, stored at `0x020F38B8`). Each makes up to 4 attempts.

## Layout of the main save

`WriteSavetoPak` (`0202A53C`) builds one block of `0x57D8` bytes and writes it **twice** in a row with `WriteSaveSector`, which advances the sector number after each copy. Each copy takes 6 sectors (`0x6000` bytes). `GameLoop_Async` reads the main save starting at sector 0, so copy A is at `0x0000` and copy B at `0x6000` (the write side's starting sector is not checked yet).

`ReadSaveFromPak` (`0202A1AC`) reads copy A; if that read fails, it skips to copy B.

### Header

| Offset | Size | Field | Notes |
|--------|------|-------|-------|
| `0x000` | 4 | checksum | Sum of all following 32-bit words of the block (`CalculateChecksum`, `ValidateChecksum`) |
| `0x004` | 0x400 | global script variables | `SaveGlobalScriptVars` / `RestoreGlobalScriptVars` |
| `0x404` | 0x10 | game internal name | written by `WriteSavetoPak`, not checked on read |
| `0x414` | 4 | magic | `0x06061320` (Red: `0x05071412`); a mismatch fails the load |
| `0x418` | 4 | `unk418` | |
| `0x41C` | 4 | `unk41C` | |
| `0x420` | 4 | global RNG state | `GetRNGState` / `SetRNGSeed` ([`re/rng.md`](../re/rng.md)) |
| `0x424` | 4 | **language** | **Blue only**; `-1` when `unk41C` is 0 |
| `0x428`–`0x448` | 4 each | section results | one value per section below, compared on load (Red's fields, 4 bytes later) |
| `0x44C` | | section data | |

Blue's block is Red's plus the 4-byte language field.

### Sections

Offsets are from `0x44C`. Sizes and order are the same as in Red; each section has its own save and restore function, and the restore result must equal the stored value.

| Offset | Size | Contents | Restore / save |
|--------|------|----------|----------------|
| `0x0000` | `0x4650` | recruited Pokémon | `RestoreRecruitedPokemon` / `SaveRecruitedPokemon` |
| `0x4650` | `0x258` | `Poke2` records (150 × 4) | `RestorePoke2s` / `SavePoke2s` |
| `0x48A8` | `0x1D8` | team inventory | `RestoreTeamInventory` / `SaveTeamInventory` |
| `0x4A80` | `0x10` | rescue team info | `ReadRescueTeamInfo` / `SaveRescueTeamInfo` |
| `0x4A90` | `0x8` | friend areas | `ReadSavedFriendAreas` / `SaveFriendAreas` |
| `0x4A98` | `0x100` | adventure data | `RestoreAdventureData` / `SaveAdventureData` |
| `0x4B98` | `0x594` | (unnamed in Red) | `sub_80954CC` / `sub_8095624` |
| `0x512C` | `0x221` | mail | `RestoreMailInfo` / `SaveMailInfo` |

The sections end at `0x534D`, inside the `0x57D8` block.

## Section encoding

Every section except the script variables is **bit-packed** with the same serializer as Red (`src/data_serializer.c`), matched function by function:

- `InitBitWriter` clears the section buffer; `WriteBits(seri, src, n)` copies the lowest `n` bits of `src`, **least significant bit first**, into the stream, filling each byte from bit 0 up. `ReadBits` is the reverse; `FinishBitSerializer` does nothing.
- A section's result value in the header is the serializer's bit position inside its last byte (0–7) when the section ended; on load, the reader must end at the same position.
- Multi-byte values are written from their lowest byte up, so a 24-bit field holds the low 3 bytes of a little-endian integer.

The sequence of field widths in each section is the same as in Red's save functions. This was checked by extracting, for each section, every `WriteBits` width in call order (following helper calls) from Blue's code and from Red's source, and comparing the two lists:

| Section | Red function (field list) | Writes compared | Result |
|---------|---------------------------|-----------------|--------|
| Recruited Pokémon | `SaveRecruitedPokemon`, `WritePoke1Bits` | per Pokémon record | same |
| `Poke2` records | `SavePoke2s` | 34 field widths | same |
| Team inventory | `SaveTeamInventory` | all | same (below) |
| Rescue team info | `SaveRescueTeamInfo` | 3 | same |
| Friend areas | `SaveFriendAreas` | 1 per area | same |
| Adventure data | `SaveAdventureData` | 32 | same (`learnedMoves` is 13 × 32 bits in both) |
| Mail and rescue records | `sub_8095624`, `sub_8095824` | all | same |
| Mail info | `SaveMailInfo` | 44 | same |
| Script variables | `SaveGlobalScriptVars` | — | raw 0x400-byte copy in both |

So each field's meaning and width can be read from those `pmd-red` functions.

### Example: team inventory

| Field | Count | Bits each |
|-------|-------|-----------|
| team items (`flags` 8, `quantity` 7, `id` 8) | 20 | 23 |
| storage counts | 240 | 10 |
| Kecleon shop items (`id` 8, `quantity` 7) | 8 | 15 |
| Kecleon wares | 4 | 15 |
| money | 1 | 24 |
| savings | 1 | 24 |

That is 3,088 bits (386 bytes) of the 0x1D8-byte section.

## Checks on load

`ReadSaveFromPak` returns:

| Value | Meaning (Red's `ReadSaveStatus`) |
|-------|----------------------------------|
| 0 | valid |
| 1 | read failed |
| 2 | checksum error |
| 3 | a section's restore result does not match the header |
| 4 | wrong magic, or the script variables were rejected |

## Open questions

- The quicksave (suspended dungeon) area: it uses a separate buffer of `0x4800` bytes (`PrepareQuickSaveRead`); find its sector.
- What `unk418` and `unk41C` hold.
- Name every field of the remaining sections in Rust types, following the `pmd-red` functions listed above.
- Check all of this on a real save file (local only).
