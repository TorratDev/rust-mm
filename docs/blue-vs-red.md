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
| Run preseed source | `RunDungeon` `0206DC20`: rescue seed or `Rand32Bit() & 0xFFFFFF` | `RunDungeon_Async`, `src/run_dungeon.c` | same |
| Seed fields in the dungeon struct | `+0x668` mission kind, `+0x66C` preseed, `+0x670` floor seed | `unk644` at `0x644`: `+0x34`, `+0x38`, `+0x3C` | same layout inside the sub-struct; **sub-struct starts 0x10 earlier in Blue** (`0x634`) |
| Mersenne Twister | `0200CCB0` / `0200CB40`, state `020DED24` | `src/random_mersenne_twister.c` | present in both; Blue's users not identified yet |

Correction to V5: Red has an LCG (global and dungeon) as well as the Mersenne Twister. The Mersenne Twister is not Red's main generator.
