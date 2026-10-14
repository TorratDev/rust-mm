# Turn loop

How a dungeon floor advances, turn by turn. Issue: [#49](https://github.com/TorratDev/rust-mm/issues/49). Names, addresses and tags are in [`symbols.md`](../symbols.md). The code is in overlay 1 ([`execution-model.md`](execution-model.md)) and matches `pmd-red`'s `src/dungeon_engine.c` call for call. Not confirmed by emulator trace yet.

## The floor loop

`RunDungeon_Async` (`0206DC20`) runs each floor with:

```
param = TRUE
do {
    RunFractionalTurn_Async(param)   // ov_0001::021411BC
    param = FALSE
} while (!IsFloorOver())            // ov_0001::0214067C
```

Before the loop it clears `dungeon+0x648` (Red: `unk644.unk10`, the floor-end reason that `IsFloorOver` sets to 1 or 2).

## One fractional turn

`RunFractionalTurn_Async` does, stopping as soon as `IsFloorOver()` is true:

1. `TrySpawnMonsterAndActivatePlusMinus_Async`
2. `RunLeaderTurn_Async(param)`; if the leader acted, `UpdateWindTurns`
3. `sub_8044574_Async`, `sub_8044820_Async`, `sub_8044AB4` (in Red: the other monsters' turns and end-of-turn effects; names not checked in Blue yet)
4. `fractionalTurn` (`dungeon+0x650`, Red: `unk644.fractionalTurn`) goes up by one and wraps at 24.

The 24 fractional turns let monsters with different speeds act different numbers of times; in Red, `RunLeaderTurn_Async` looks up `gSpeedTurns[speed][fractionalTurn]` to decide whether the leader acts in this fractional turn.

## For golden traces (V7)

- **Trigger:** the entry of `RunFractionalTurn_Async`, RAM `0x021411BC`, while overlay 1 is loaded. Overlays 0 and 7 use the same RAM range when no dungeon runs, so the hook must only count hits after the dungeon has started (for example after `LoadAndRunDungeon_Async` has loaded overlay 1).
- **Per-trigger state:** `fractionalTurn` at `dungeon+0x650`, with the dungeon struct at the pointer stored in `gDungeon` (`0x020E0788`), plus the RNG state from [`rng.md`](rng.md).
- One player turn spans several fractional turns. To dump once per player action instead, hook `RunLeaderTurn_Async` (`ov_0001::02140FAC`) and keep the hits where it returns true.

## Dungeon struct offsets seen so far

| Blue offset | Red field | Red offset | Notes |
|-------------|-----------|------------|-------|
| `+0x648` | `unk644.unk10` (floor-end reason) | `0x654` | |
| `+0x650` | `unk644.fractionalTurn` | `0x660` | wraps at 24 |
| `+0x668` | `unk644.missionKind` | `0x678` | [`rng.md`](rng.md) |
| `+0x66C` | `unk644.prngPreseed23Bit` | `0x67C` | [`rng.md`](rng.md) |
| `+0x670` | `unk644.unk3C` (floor seed) | `0x680` | [`rng.md`](rng.md) |
| `+0x15FB0`, `+0x15FB1` | `unk181e8.unk18218`, `unk18219` | `0x18218`, `0x18219` | Blue's struct is about 0x2268 bytes shorter before these |

Most fields sit 0x10 earlier than in Red; `unk644.unk10` sits only 0xC earlier, so the first part of the sub-struct differs slightly. This is input for [#46](https://github.com/TorratDev/rust-mm/issues/46).

## Open questions

- Check `gSpeedTurns` in Blue (table read by `RunLeaderTurn_Async`) against Red by value.
- Name the three end-of-turn functions by their contents (other monsters' turns, status countdowns, belly, regeneration).
- Confirm the trigger on the retail ROM with the V7 script.
