# Blue vs Red deltas

Differences found between Blue (NDS, this project's target) and Red (GBA, the `pmd-red` decomp), per subsystem. Each row cites the Blue address and the `pmd-red` file or symbol. Matches are listed too, when they settle a hypothesis. Issue: [#58](https://github.com/TorratDev/rust-mm/issues/58).

Blue is the ground truth. A row saying "same" means Blue's code was read and found equivalent; it does not mean Red was assumed.

## Categories

Every row has one category:

- **bug fix**: Blue corrects a behaviour that Red gets wrong.
- **rebalance**: Blue has the same code path with different data values.
- **DS-only feature**: Blue has something Red does not have, or changes a value because of DS hardware or the Blue release.
- **same**: a match, checked against Blue's code or data.
- **unclassified**: a difference that none of the four above describes. It is a layout or packing change, an unconfirmed difference, or an unresolved table. A maintainer decides which category it gets.

No row is marked **bug fix** or **rebalance** yet. The repository records no difference that the sources describe as one of these.

## RNG ([`re/rng.md`](re/rng.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Global LCG step | `Rand16Bit` `020119A4`: `s * 0x5D588B65 + 1`, output `s >> 16` | `Rand16Bit`, `src/random.c` | same | same |
| Global LCG seeding | `SeedRng` `020119D8` with the 6-byte console MAC address from `027FFCF4` | `SeedRng` with 6 constant bytes, `src/main.c` | **different source**, same formula (`54021 + a*b + c*d + e*f`) | DS-only feature |
| `SetRNGSeed` | `020118EC`: set, then advance once through `Rand32Bit` | same, `src/random.c` | same | same |
| Dungeon LCG and wrappers | `ov_0001` `DungeonRand16Bit`, `DungeonRandInt`, `DungeonRandRange`, `DungeonRandOutcome`, `_2` | `src/dungeon_random.c` | same, including the duplicated `DungeonRandOutcome_2` | same |
| Floor seed | `GenerateDungeonRNGSeed` `ov_0001::0219D0C4`, preseed `& 0xFFFFFF \| 1` | same, `src/dungeon_random.c` | same | same |
| Run preseed source | `RunDungeon_Async` `0206DC20`: rescue seed or `Rand32Bit() & 0xFFFFFF` | `RunDungeon_Async`, `src/run_dungeon.c` | same | same |
| Seed fields in the dungeon struct | `+0x668` mission kind, `+0x66C` preseed, `+0x670` floor seed | `unk644` at `0x644`: `+0x34`, `+0x38`, `+0x3C` | **sub-struct starts 0xC earlier in Blue** (`0x638`), and its fields from `+0x14` on sit 4 bytes earlier inside it ([`re/structs.md`](re/structs.md)) | unclassified |
| Mersenne Twister | `0200CCB0` / `0200CB40`, state `020DED24`; users `0200D4DC` (`sub_809542C`, mail record) and `0206C6E4` (`HandleTestTrackerState`, personality test) | `src/random_mersenne_twister.c`; users `src/code_8094F88.c`, `src/personality_test1.c` | present in both; both users match `pmd-red` and seed from the global LCG | same |
| Save round trip | `WriteSavetoPak` stores `sPRNGState` at save `+0x420`; `ReadSaveFromPak` calls `SetRNGSeed`, which stores `Rand32Bit()`'s output, not the saved state | `src/save.c:198, 203, 323, 328`: `SetRNGSeed(playerSave->RngState)`, `GetRNGState()` | same: the round trip is not the identity on the state in Red either | same |
| RNG per frame | Emulator (retail ROM, no button held): the global LCG advances 4 steps per emulated frame in the measured phase; the caller is not identified yet ([`re/execution-model.md`](re/execution-model.md#rng-per-frame)) | `WaitForNextFrameAndAdvanceRNG` advances it once per frame, plus once more while A or Select is held | **Blue advances 4 steps per frame, Red 1** (measured once for Blue, medium confidence; the A and Select effect is not established) | unclassified |
| 16-bit generator (`other_random.c`) | `FUN_02023B9C` (`OtherRandom16`): `state * 109 + 1021` in 16 bits, state `gOtherRngState` at `020E03F4`, initial `0x348C`; wrappers `FUN_02023B84` (`OtherRandInt`), `FUN_02023B6C` (`OtherRandRange`) | `OtherRandom16`, `src/other_random.c`; its NDS addresses (`arm9.bin::020127F4`) come from another build | **same** constants and initial value; Blue's return is zero-extended (`ldrh`), Red's signedness not compared; the addresses differ ([`re/rng.md`](re/rng.md#16-bit-generator)) | unclassified |

Correction to V5: Red has an LCG (global and dungeon) as well as the Mersenne Twister. The Mersenne Twister is not Red's main generator.

## Execution model ([`re/execution-model.md`](re/execution-model.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Start-up | `GameLoop_Async` `0201F2E4` | `GameLoop_Async`, `src/main_loops.c` | same calls in the same order; Blue adds a 4 KiB memory check before reading the save | DS-only feature |
| Language | Set from a console record when unset; text per language in overlays 2–6 | single language | **Blue only** | DS-only feature |
| Title menu | choices 2, 3, 4 as in Red, plus choice 5 (`RunOverlay7Mode`, overlay 7) | choices 2, 4, `MENU_NEW_GAME` | **one extra mode in Blue** | DS-only feature |
| Ground and dungeon code | overlays 0 and 1, swapped by `NDS_LoadOverlay_GroundMain` and `LoadAndRunDungeon_Async` | always resident; the load points are empty stubs | NDS overlay swap | DS-only feature |
| Quicksave dungeon entry | `LoadAndRunQuickSaveDungeon_Async` `0201E204` | same, `src/main_loops.c` | same steps | same |
| Frame actions | 19 calls of `MainLoops_RunFrameActions`, not yet named | Red's list; Red notes 2–3 extra sprite calls in Blue for the second screen | **count not checked**; Red's note is unconfirmed | unclassified |

## Combat ([`re/combat.md`](re/combat.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Damage formula constants | `CalcDamage` `ov_0001::0217BD94` | `CalcDamage`, `src/dungeon_damage.c` | same constants and same RNG call order | same |
| Type tables | `gTypeEffectivenessChart`, `gEffectivenessChart`, stat-stage multipliers in overlay 1 | `src/dungeon_config.c` | same values | same |
| Wonder Guard | ability `0x35` in `sub_806F370_Async` | `ABILITY_WONDER_GUARD` | same | same |
| `EntityInfo.types` | `+0x56` | `+0x5C` | **6 bytes earlier in Blue** | unclassified |
| Entity info pointer | `entity+0x70` | `axObj.info` at `0x28 + 0x48` | same offset | same |
| First `DungeonRandInt(100)` result | Appears unused in Blue's decompiled output | Used in the stat-boost step | **to check in the disassembly before porting** (`re/combat.md`) | unclassified |
| `TargetNotImmuneTo` | No matching reference in Blue | A fifth reader of the type chart, marked unused | **Blue has no reference** to the unused Red function | unclassified |

## Structs ([`re/structs.md`](re/structs.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Entity size and offsets | `0x74` bytes, the same offsets wherever checked ([`re/structs.md`](re/structs.md#entity)) | `Entity`, `0x74` bytes | same, where checked; Red's struct is the working hypothesis for the rest | same |
| Entity info size | Shorter than Red's `0x208`: bytes missing before the types, in the status block, around the speed counters and before the belly. Exact size not found; the bound is 0x1C8 ≤ size ≤ 0x208 ([`re/structs.md`](re/structs.md#entity-info)) | `0x208` bytes | **shorter in Blue** | unclassified |
| Entity info, AI and action blocks | Offsets shift by block: `action` and `direction` −2, `aiTarget` −8, `decoyAITracker` −0x23 (probable), `aiAllySkip`, `recalculateFollow` and `waiting` −0x2E, `targetPos` and `numMoveTiles` −0x34 ([`re/structs.md`](re/structs.md#entity-info)) | `action` block `0x44`, `direction` `0x46`, `aiTarget` `0x78`, `decoyAITracker` `0x100`, `aiAllySkip` `0x144`, `targetPos` `0x16C`, `numMoveTiles` `0x1F4` | **shift differs by block** | unclassified |
| Dungeon struct offsets after `+0x17A` | Identical to Red up to `0x17A`; then fields move earlier, by 8 bytes at `0x5C0` and by up to `0x2268` from `traps` on ([`re/structs.md`](re/structs.md#dungeon-struct)) | same fields at Red's offsets | **offsets shift** | unclassified |
| Tile array | `tiles[y][x]` at dungeon `+0x34B8` | `0x3A18` | **`0x560` earlier in Blue** | unclassified |
| Tile size | `0x14` bytes | `0x18` bytes | **smaller in Blue** (checked by `ResetTile` and `GetTileMut`) | unclassified |
| `stairsSpawn` | dungeon `+0xC0BC` (`.x`), `+0xC0BE` (`.y`) | `0xE21C` by the layout; `pmd-red`'s comment says `0xEA1C` | **`0x2160` earlier in Blue**; Red's comment disagrees with Red's own layout | unclassified |
| Team roster `Pokemon` | Size `0x40`; fields match `pmd-red`'s Blue notes in `include/structs/str_pokemon.h` field for field | `0x58` bytes | **smaller in Blue**; fields same | unclassified |
| Item packing | Blue's struct is `0x23C` bytes: `Item` in 3 bytes and `BulkItem` in 2 | Red `0x268` bytes; 4-byte stride for both arrays | **packing differs** ([`re/structs.md`](re/structs.md)) | unclassified |

## Save ([`formats/save.md`](formats/save.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Media | 2 Mbit flash, `CARD_IdentifyBackup(0x1202)` | GBA flash | **different hardware**, same 4 KiB sector logic | DS-only feature |
| Block | `0x57D8` bytes, written twice | `0x57D4` bytes, written twice | **4 bytes longer in Blue** | unclassified |
| Header | adds a language field at `+0x424` | — | **Blue only**; later header fields 4 bytes later | DS-only feature |
| Magic | `0x06061320` | `0x05071412` | **different value** | DS-only feature |
| Checksum | sum of the other 32-bit words in word 0 | same | same | same |
| Sections | 8 sections, same sizes and order | same | same | same |

## Data tables ([`formats/data.md`](formats/data.md), #37)

Compared field by field against pmd-red's `data/monster/monster_data.json`,
`data/move/move_data.json` and `data/item/item_data.json` (2026-10-16, Blue EUR dump).

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Container | SIR0 sub-files in `system.sbin` (`monspara`, `wazapara`, `itempara`) | same names in Red's `system` pack | same layout | same |
| Species | 424 entries of 0x48 bytes | `MonsterDataEntry`, 424 entries (`monspara`, `SIRO`) | **all numeric fields identical**, including types and abilities; all 424 names identical. Bytes `0x0A`–`0x0B` and `0x46`–`0x47` differ in every species, and Red's struct has no field there (padding in Red, unknown in Blue) | same |
| Moves | 413 entries of 0x24 bytes; learnsets 8 bytes per species | `MoveDataEntry`, `MoveLearnset` | **all numeric fields identical** (power, PP, AI weight, accuracies, hits, upgrade, crit, flags, targeting) | same |
| Items | 240 entries of 0x20 bytes | `ItemDataEntry`, 240 entries | **all numeric fields identical** (prices, sprite, AI flags, order, spawn range, palette) | same |
| Item byte `0x1F` | No reader found [unverified] ([`re/data-tables.md`](re/data-tables.md)) | Padding | **Red pads the byte; Blue has no reader found** | unclassified |
| Translations | `wazapar{f,g,i,s}`, `itempar{f,g,i,s}`: same tables with translated text | single language | **Blue only** | DS-only feature |
| Level growth | `lvmpNNN`: 421 files, SIR0 + AT4PX, 100 x 12-byte `LevelData` | `LevelData`, `lvmpNNN` in the Red system archive (`SIRO` + AT4PX) | **all 421 files identical** (expected EXP, HP, attack and defence gains, all 100 levels) | same |
| Dungeon floors | `mapparam`: 64 dungeons, 1831 floors, 1764 `FloorProperties`, 839 monster, 148 trap, 178 item lists | `DungeonMapParam2` (`SIRO`, five VA pointers) | **all identical** in every table (floors: properties, monster, trap and item indices; floor properties and lists: raw values). The parser had counted five header pointers as item lists (183); fixed to 178. | same |
| Type chart | overlay 1 `0x72688` | `gTypeEffectivenessChart` | same values (V6) | same |
| Monster spawn record | 8 bytes `{u16 species \| level, s16 weight[2], u16 pad}`; loader `FUN_ov_0001__02138ccc`, end test `FUN_02000eec` | `str_spawn_pokemon_data.h`, comment "R=0x8 \| B=0x6" | **8 bytes in Blue; Red's "B=0x6" comment is wrong** (blue-verified). The parser used 6 bytes and kept one entry per list (839 instead of 5632) | unclassified |
| Monster spawn terminator | each list's species-0 record ends where the next list starts (838 of 838; the last list is followed by other data) | not checked | blue-verified; the parser now fails otherwise | unclassified |
| Monster spawn cap | loop cap of 32 entries per list (the largest list in the file has 11) | not checked | blue-verified (investigator C); Red not checked | unclassified |
| Learnsets of species 420 and 421 | empty stream from `FUN_0200b448`, `FUN_0200b48c`, `FUN_02001844` (pointer `0x02091C84`); the table has 2 and 59 TM/HM moves for them | not checked | **Blue only** (blue-verified); the parser now returns empty learnsets for them | unclassified |
| Level gate on learnset moves | moves 0xEE, 0xEF, 0x110, 0x162 skipped unless a monster field at `+0xA` is at least 333 (`02091E18`–`02091E34`) | not checked | blue-verified values; field meaning unknown | unclassified |
| Learn cap | 413 moves (`DAT_02001a2c`) | not checked | blue-verified | unclassified |
| Regen byte | speed `(b & 0x7F) << 1`; bit 7 is a separate flag (`FUN_02002368`) | regen byte, not split | **Blue reads a flag in bit 7** (blue-verified); flag meaning unknown | unclassified |
| Recruit level table | `gRecruitRateByLevel`, 101 `s16` in overlay 1 at `0x7210C` | `gRecruitRateByLevel`, `dungeon_config.c` | **identical**, 240 at levels 90–100 (blue-verified) | same |
| Recruit formula | `FUN_ov_0001__0217f9b8`: roll 1000, `-999` fails, Friend Bow +100, level rate | `DungeonMonRecruit` | **identical** (blue-verified) | same |
| Special item IDs | `0x69`, `0x77`–`0x7A` excluded from normal items (`FUN_0200747c`) | not checked | blue-verified; Red not checked | unclassified |
| Special move IDs | 8, 0x3C, 0x4B, 0x64, 0x97, 0x99, 0x9C, 0xCD, 0x15C (`FUN_020092bc`) | not checked | blue-verified; Red not checked | unclassified |
| Item move ID | `u16` at `+0x18` (`FUN_02007670`) | `ItemDataEntry` field | Blue is unsigned; the parser was `i16` and is now `u16` (blue-verified) | unclassified |
| `gTypeGummiIQBoost`, `gUnknown_810A808` | Not found by value in Blue | Tables in `pmd-red` | **unresolved**: moved, changed layout or removed; open in V6 | unclassified |

Level growth and dungeon floor data were compared on 2026-10-17 against the Red dump held in the
repository (`rs-pmdr.gba`, SHA-1 `afee3b060dd5fd4a68afb1b003456aef3a2af073`). Its SHA-1 differs from
the reference `pmd_red.gba` in `reference/pmd-red/red.sha1` (`9f4cfc5b…`), so this is another
revision, and the comparison is against that revision. Tables are read from the Red ROM, which
is never extracted into the repository.

Moves and items were compared with `pmd-red`'s JSON data (the first table above). Blue's accessors for the move and item fields were also checked against the offsets in `data.md` (2026-10-18). The Red ROM's move table at `0x243BCC` has not been mapped; the comparison did not need it.

## Text engine ([`re/text-engine.md`](re/text-engine.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| `#+` centres | x = (box width × 8 − text width) / 2 `[blue-verified]` | `CENTER_ALIGN` in `charmap.txt` | same meaning | same |
| `#~` waits | Waits frames | Lists `#~` as wait frames | same | same |
| `#>` | Sets an absolute x, not an alignment | `UNK_MACRO_3E` read as an alignment | **the Red reading is wrong for Blue** | unclassified |
| `#_` | Pushes a shade from a table | `UNK_MACRO_5F_*` (not described) | **Blue has a use Red does not describe** | unclassified |

## Sprites and effects

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| Sprite follow-up table | `021AE1D0` ([`formats/sprites.md`](formats/sprites.md)) | `gUnknown_8106EC8` in `src/dungeon_mon_sprite_render.c` | same rows and same sources (`sub_806CE94`, `sub_806CDD4`, `sub_806CEBC`) | same |
| Effect and animation file layout | Follows `dump_effect_sbin.py`'s layout; the pose table length is confirmed by the animation blocks ([`formats/effects.md`](formats/effects.md)) | `dump_effect_sbin.py` in `pmd-red` (`EfoFileData`, `EfbFileData`, `ax_pose`, `ax_anim`) | same layout | same |

## Turns ([`re/turns.md`](re/turns.md))

| Item | Blue | Red (`pmd-red`) | Verdict | Category |
|------|------|-----------------|---------|----------|
| `DoEndOfTurnEffects` return type | `bool DoEndOfTurnEffects` (`021861E8`) | `void DoEndOfTurnEffects_Async` (`src/dungeon_turn_effects.c:47`) | **different return type**; the behaviour matches on the paths checked. Whether the Blue callers use the return value was not checked | unclassified |
