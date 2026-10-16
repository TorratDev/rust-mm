# Blue vs Red deltas

Differences found between Blue (NDS, this project's target) and Red (GBA, the `pmd-red` decomp), per subsystem. Each row cites the Blue address and the `pmd-red` file or symbol. Matches are listed too, when they settle a hypothesis. Issue: [#58](https://github.com/TorratDev/rust-mm/issues/58).

Blue is the ground truth. A row saying "same" means Blue's code was read and found equivalent; it does not mean Red was assumed.

## RNG ([`re/rng.md`](re/rng.md))

| Item | Blue | Red (`pmd-red`) | Verdict |
|------|------|-----------------|---------|
| Global LCG step | `Rand16Bit` `020119A4`: `s * 0x5D588B65 + 1`, output `s >> 16` | `Rand16Bit`, `src/random.c` | same |
| Global LCG seeding | `SeedRng` `020119D8` with the 6-byte console MAC address from `027FFCF4` | `SeedRng` with 6 constant bytes, `src/main.c` | **different source**, same formula (`54021 + a*b + c*d + e*f`) |
| `SetRNGSeed` | `020118EC`: set, then advance once through `Rand32Bit` | same, `src/random.c` | same |
| Dungeon LCG and wrappers | `ov_0001` `DungeonRand16Bit`, `DungeonRandInt`, `DungeonRandRange`, `DungeonRandOutcome`, `_2` | `src/dungeon_random.c` | same, including the duplicated `DungeonRandOutcome_2` |
| Floor seed | `GenerateDungeonRNGSeed` `ov_0001::0219D0C4`, preseed `& 0xFFFFFF \| 1` | same, `src/dungeon_random.c` | same |
| Run preseed source | `RunDungeon_Async` `0206DC20`: rescue seed or `Rand32Bit() & 0xFFFFFF` | `RunDungeon_Async`, `src/run_dungeon.c` | same |
| Seed fields in the dungeon struct | `+0x668` mission kind, `+0x66C` preseed, `+0x670` floor seed | `unk644` at `0x644`: `+0x34`, `+0x38`, `+0x3C` | **sub-struct starts 0xC earlier in Blue** (`0x638`), and its fields from `+0x14` on sit 4 bytes earlier inside it ([`re/structs.md`](re/structs.md)) |
| Mersenne Twister | `0200CCB0` / `0200CB40`, state `020DED24` | `src/random_mersenne_twister.c` | present in both; Blue's users not identified yet |
| RNG per frame | No path from `MainLoops_RunFrameActions` to the global RNG within 4 call levels (static) | `WaitForNextFrameAndAdvanceRNG` advances it every frame, twice while A or Select is held | **probably removed in Blue**; confirm by emulator |

Correction to V5: Red has an LCG (global and dungeon) as well as the Mersenne Twister. The Mersenne Twister is not Red's main generator.

## Execution model ([`re/execution-model.md`](re/execution-model.md))

| Item | Blue | Red (`pmd-red`) | Verdict |
|------|------|-----------------|---------|
| Start-up | `GameLoop_Async` `0201F2E4` | `GameLoop_Async`, `src/main_loops.c` | same calls in the same order; Blue adds a 4 KiB memory check before reading the save |
| Language | Set from a console record when unset; text per language in overlays 2–6 | single language | **Blue only** |
| Title menu | choices 2, 3, 4 as in Red, plus choice 5 (`RunOverlay7Mode`, overlay 7) | choices 2, 4, `MENU_NEW_GAME` | **one extra mode in Blue** |
| Ground and dungeon code | overlays 0 and 1, swapped by `NDS_LoadOverlay_GroundMain` and `LoadAndRunDungeon_Async` | always resident; the load points are empty stubs | NDS overlay swap |
| Quicksave dungeon entry | `LoadAndRunQuickSaveDungeon_Async` `0201E204` | same, `src/main_loops.c` | same steps |

## Combat ([`re/combat.md`](re/combat.md))

| Item | Blue | Red (`pmd-red`) | Verdict |
|------|------|-----------------|---------|
| Damage formula constants | `CalcDamage` `ov_0001::0217BD94` | `CalcDamage`, `src/dungeon_damage.c` | same constants and same RNG call order |
| Type tables | `gTypeEffectivenessChart`, `gEffectivenessChart`, stat-stage multipliers in overlay 1 | `src/dungeon_config.c` | same values |
| Wonder Guard | ability `0x35` in `sub_806F370_Async` | `ABILITY_WONDER_GUARD` | same |
| `EntityInfo.types` | `+0x56` | `+0x5C` | **6 bytes earlier in Blue** |
| Entity info pointer | `entity+0x70` | `axObj.info` at `0x28 + 0x48` | same offset |

## Save ([`formats/save.md`](formats/save.md))

| Item | Blue | Red (`pmd-red`) | Verdict |
|------|------|-----------------|---------|
| Media | 2 Mbit flash, `CARD_IdentifyBackup(0x1202)` | GBA flash | **different hardware**, same 4 KiB sector logic |
| Block | `0x57D8` bytes, written twice | `0x57D4` bytes, written twice | **4 bytes longer in Blue** |
| Header | adds a language field at `+0x424` | — | **Blue only**; later header fields 4 bytes later |
| Magic | `0x06061320` | `0x05071412` | **different value** |
| Checksum | sum of the other 32-bit words in word 0 | same | same |
| Sections | 8 sections, same sizes and order | same | same |

## Data tables ([`formats/data.md`](formats/data.md), #37)

Compared field by field against pmd-red's `data/monster/monster_data.json`,
`data/move/move_data.json` and `data/item/item_data.json` (2026-10-16, Blue EUR dump).

| Item | Blue | Red (`pmd-red`) | Verdict |
|------|------|-----------------|---------|
| Container | SIR0 sub-files in `system.sbin` (`monspara`, `wazapara`, `itempara`) | same names in Red's `system` pack | same layout |
| Species | 424 entries of 0x48 bytes | `MonsterDataEntry`, 424 entries (`monspara`, `SIRO`) | **all numeric fields identical**, including types and abilities; all 424 names identical. Bytes `0x0A`–`0x0B` and `0x46`–`0x47` differ in every species, and Red's struct has no field there (padding in Red, unknown in Blue) |
| Moves | 413 entries of 0x24 bytes; learnsets 8 bytes per species | `MoveDataEntry`, `MoveLearnset` | **all numeric fields identical** (power, PP, AI weight, accuracies, hits, upgrade, crit, flags, targeting) |
| Items | 240 entries of 0x20 bytes | `ItemDataEntry`, 240 entries | **all numeric fields identical** (prices, sprite, AI flags, order, spawn range, palette) |
| Translations | `wazapar{f,g,i,s}`, `itempar{f,g,i,s}`: same tables with translated text | single language | **Blue only** |
| Level growth | `lvmpNNN`: 421 files, SIR0 + AT4PX, 100 x 12-byte `LevelData` | `LevelData`, `lvmpNNN` in the Red system archive (`SIRO` + AT4PX) | **all 421 files identical** (expected EXP, HP, attack and defence gains, all 100 levels) |
| Dungeon floors | `mapparam`: 64 dungeons, 1831 floors, 1764 `FloorProperties`, 839 monster, 148 trap, 178 item lists | `DungeonMapParam2` (`SIRO`, five VA pointers) | **all identical** in every table (floors: properties, monster, trap and item indices; floor properties and lists: raw values). The parser had counted five header pointers as item lists (183); fixed to 178. |
| Type chart | overlay 1 `0x72688` | `gTypeEffectivenessChart` | same values (V6) |

Level growth and dungeon floor data were compared on 2026-10-17 against the Red dump held in the
repository (`rs-pmdr.gba`, SHA-1 `afee3b060dd5fd4a68afb1b003456aef3a2af073`). Its SHA-1 differs from
the reference `pmd_red.gba` in `reference/pmd-red/red.sha1` (`9f4cfc5b…`), so this is another
revision, and the comparison is against that revision. Tables are read from the Red ROM, which
is never extracted into the repository.

Not compared yet: moves and items. The Red move table at `0x243BCC` gives the right names for some entries, but the stride or order differs from Blue's, so the layout is not mapped yet.
