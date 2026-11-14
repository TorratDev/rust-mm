//! The effects of the first dungeon's moves (`docs/re/move-effects.md`, "First dungeon: move list,
//! handlers and draws (#357)"). The dispatch runs the slot-0 hit check before any handler: a miss
//! stops there, and no handler runs. A handler that is not listed here is a refused use
//! (`Outcome::NotModelled`), with a `shortcut:` note at [`effect_of`].
//!
//! Integer math only. Each handler's draws are those the notes record, in their order.

use rescue_data::combat::STAGES;

use crate::combat::{ARG8, DAMAGING};
use crate::state::EntityInfo;

/// Growl (`0x0D9`): the target's attack stage on split 0 goes down one.
pub(crate) const GROWL: u16 = 0x0D9;
/// Leer (`0x0E0`) and Tail Whip (`0x07A`): the same handler, the target's defence stage on split 0
/// goes down one (`02161548`).
pub(crate) const LEER: u16 = 0x0E0;
pub(crate) const TAIL_WHIP: u16 = 0x07A;
/// Sand-Attack (`0x08E`): the target's accuracy stage goes down one.
pub(crate) const SAND_ATTACK: u16 = 0x08E;
/// Helping Hand (`0x0B9`): the ally's attack stage on both splits goes up one. A use on the user
/// fails in the game and is not modelled (see [`effect_of`]).
pub(crate) const HELPING_HAND: u16 = 0x0B9;

/// Razor Leaf (`0x0FB`) and Vine Whip (`0x0B6`): the damaging handler with `arg8` 1.0. The
/// dispatcher's cases pass `0x100` to `HandleDamagingMove` (`ov_0001::02156824`, export read on
/// #421), as they do for Tackle and Scratch.
pub(crate) const RAZOR_LEAF: u16 = 0x0FB;
pub(crate) const VINE_WHIP: u16 = 0x0B6;

/// Absorb (`0x087`): the damaging path with `arg8` 1.0, then the user heals half the damage, at
/// least 1, up to its max HP (`ov_0001::0215F314`, heal `0218DF38`; `docs/re/move-effects.md`,
/// P1 of #421).
pub(crate) const ABSORB: u16 = 0x087;

/// Low Kick (`0x05C`): the damaging path with `arg8` the target's species weight (24.8), read at
/// the target's apparent species (`ov_0001::0215C600`, `02002314`; `docs/re/move-effects.md`, P2
/// of #421). The weight is the species table's `+0x2C` field, so `arg8` is that value unchanged.
pub(crate) const LOW_KICK: u16 = 0x05C;

/// Water Sport (`0x133`) and Mud Sport (`0x0D4`): a floor counter that runs out over a few turns
/// (`ov_0001::021617C0`, `021955B0`; `docs/re/move-effects.md`, P-c1 of #421). The move row's
/// executor targeting is the user (`0x73`), and the accuracy forces a hit.
pub(crate) const WATER_SPORT: u16 = 0x133;
pub(crate) const MUD_SPORT: u16 = 0x0D4;
/// The turns a sport's counter is set to: `DungeonRandRange(10, 12)`, the bounds the handler
/// passes (`low` inclusive, `high` exclusive, as `DungeonRng::range`).
pub(crate) const SPORT_TURNS: (i32, i32) = (10, 12);

/// Struggle (`0x160`): the damaging path with `arg8` 1.0 (`ov_0001::02162C70` passes `0x100`, P-arg8
/// of #421), then a recoil on the user of max HP / 4, at least 1, when the hit does damage. The
/// recoil is in `combat`, which owns the user's HP.
pub(crate) const STRUGGLE: u16 = 0x160;

/// Withdraw (`0x046`): the user's defence stage goes up one (`ov_0001::0215CF7C`, P3 of #421). The
/// move's range is the user (`0x73`), so its one target is the user.
pub(crate) const WITHDRAW: u16 = 0x046;

/// The highest stage index: the stage table has 21 entries and 10 is neutral.
const LAST_STAGE: i16 = (STAGES - 1) as i16;

/// The floor counter a sport sets (`gDungeon+0xC117` for Mud, `+0xC118` for Water).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sport {
    /// Mud Sport: the mud counter, which the game reads to weaken Electric moves.
    Mud,
    /// Water Sport: the water counter, which the game reads to weaken Fire moves.
    Water,
}

/// A stat stage a handler changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stat {
    /// The offensive stage of split `0` (physical) or `1`: `info + 0x1C`, `+0x1E`.
    Attack(usize),
    /// The defensive stage of split `0` or `1`: `info + 0x20`, `+0x22`.
    Defence(usize),
    /// The accuracy stage: `info + 0x24`.
    Accuracy,
}

/// One stage change of a handler: the stat, the number of stages it moves by, and the abilities
/// of the target that stop it. A blocked change moves nothing and draws nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StageChange {
    pub(crate) stat: Stat,
    pub(crate) stages: i16,
    pub(crate) blocked_by: &'static [u8],
}

/// What a move does after its slot-0 hit check (see the module comment).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// The damaging path, `HandleDamagingMove`, with the move's `arg8` (24.8). Its second hit
    /// check comes after the damage (`combat`).
    Damage { arg8: i64 },
    /// Stage changes, applied in order on the target.
    Stages(&'static [StageChange]),
    /// Absorb: the damaging path with `arg8` 1.0 (`combat`), then the user heals (see
    /// [`ABSORB`]). The heal is in `combat`, which owns the attacker and the damage.
    Absorb,
    /// Low Kick: the damaging path with `arg8` the target species' weight (see [`LOW_KICK`]),
    /// read by `combat`, which owns the species table.
    LowKick,
    /// Water Sport and Mud Sport: one draw for the turns of `sport`'s floor counter, no damage
    /// and no target change (see [`WATER_SPORT`]).
    Sport(Sport),
    /// Struggle: the damaging path with `arg8` 1.0, then the user's recoil (see [`STRUGGLE`]).
    /// `combat` reads the variant for the damage scale and the recoil.
    Struggle,
    /// Stage changes on the user, whose range is the user (see [`WITHDRAW`]). The one target is
    /// the user, and its slot-0 check always hits.
    SelfStages(&'static [StageChange]),
}

// The block lists are the abilities the handler's own routine tests (`docs/re/move-effects.md`,
// "Stage-change blocks"). The lowering routine `02180798` stops a change on abilities 0x0F and 0x18,
// and each handler adds its own: 0x09 for the attack change (`02189534`), 0x1E for the accuracy
// change (`02188910`). The status check and the held-item check are not modelled (see
// `change_stages`).
const GROWL_CHANGE: &[StageChange] = &[StageChange {
    stat: Stat::Attack(0),
    stages: -1,
    blocked_by: &[0x0F, 0x18, 0x09],
}];
const DEFENCE_DOWN: &[StageChange] = &[StageChange {
    stat: Stat::Defence(0),
    stages: -1,
    blocked_by: &[0x0F, 0x18],
}];
const ACCURACY_DOWN: &[StageChange] = &[StageChange {
    stat: Stat::Accuracy,
    stages: -1,
    blocked_by: &[0x0F, 0x18, 0x1E],
}];
// Withdraw's raise has no block check (`021890C4`, P3 of #421).
const WITHDRAW_CHANGE: &[StageChange] = &[StageChange {
    stat: Stat::Defence(0),
    stages: 1,
    blocked_by: &[],
}];
const HELPING_HAND_CHANGE: &[StageChange] = &[
    StageChange {
        stat: Stat::Attack(0),
        stages: 1,
        blocked_by: &[],
    },
    StageChange {
        stat: Stat::Attack(1),
        stages: 1,
        blocked_by: &[],
    },
];

/// The effect of move `id`, or `None` for a move whose handler is not ported. A `None` is a
/// refused use, and each refused move below has a `shortcut:` for the part its notes leave open.
// shortcut: Thundershock (`0BC`), Ember (`106`), Bubble (`010`) and Bite (`03F`) draw a chance
// gate, then call a status or stat routine whose body is not read (#73 for the status codes).
// shortcut: Thunder Wave (`0C0`), Disable (`03B`), Hypnosis (`06F`), Smokescreen (`025`), Focus
// Energy (`04A`), Rage (`013`) and Leech Seed (`149`) call routines whose bodies are not read;
// their status codes and durations belong to #73 and #359.
// shortcut: Absorb's ooze branch (a target with ability `0x3A`) damages the user through
// `0217BCA4`, which is not read; `combat` refuses that use (#421).
// shortcut: Struggle's trigger (every move at 0 PP, `02166B10` and `02192D24`) is not in the command
// path: `use_move` refuses a move with 0 PP, and Struggle is reached only through `strike` (#421).
// shortcut: the sport counters' effects in the game are not applied: the type factor's Fire and
// Electric multipliers (`0217CE88`, value red-hypothesis) and the AI's and menu's checks
// (`0217CB3C`, `021644B8`). The counters are kept and ticked (`turn.rs`), but nothing reads them
// (#421).
// shortcut: the sport handler's end-of-effect message (`021953B4`, text only) is not modelled.
#[must_use]
pub(crate) fn effect_of(id: u16) -> Option<Effect> {
    match id {
        id if DAMAGING.contains(&id) || id == RAZOR_LEAF || id == VINE_WHIP => {
            Some(Effect::Damage { arg8: ARG8 })
        }
        STRUGGLE => Some(Effect::Struggle),
        WITHDRAW => Some(Effect::SelfStages(WITHDRAW_CHANGE)),
        GROWL => Some(Effect::Stages(GROWL_CHANGE)),
        LEER | TAIL_WHIP => Some(Effect::Stages(DEFENCE_DOWN)),
        SAND_ATTACK => Some(Effect::Stages(ACCURACY_DOWN)),
        HELPING_HAND => Some(Effect::Stages(HELPING_HAND_CHANGE)),
        ABSORB => Some(Effect::Absorb),
        LOW_KICK => Some(Effect::LowKick),
        WATER_SPORT => Some(Effect::Sport(Sport::Water)),
        MUD_SPORT => Some(Effect::Sport(Sport::Mud)),
        _ => None,
    }
}

/// Applies `changes` to `info` in order. Each stage moves by its change and is kept in 0 to 20.
/// A change is skipped when `info` has one of its `blocked_by` abilities.
// shortcut: at a limit the game may print a message and take another path; the message is not
// modelled, and the notes record no draw there (#421 lead).
// shortcut: the game also stops a lowering change when the target's status byte `+0xB1` is `0x0E`
// (`02180798`, which Growl, Leer and Sand-Attack call), and Growl also stops when the target holds
// item `0x0B` (`021432E8`, in `02189534`). The sim has no statuses (#73) or held items (#359), so
// those two blocks are not modelled.
pub(crate) fn change_stages(info: &mut EntityInfo, changes: &[StageChange]) {
    for change in changes {
        if change.blocked_by.iter().any(|&id| info.has_ability(id)) {
            continue;
        }
        let stage = match change.stat {
            Stat::Attack(split) => info.offensive_stages.get_mut(split),
            Stat::Defence(split) => info.defensive_stages.get_mut(split),
            Stat::Accuracy => Some(&mut info.accuracy_stage),
        };
        if let Some(stage) = stage {
            *stage = stage.saturating_add(change.stages).clamp(0, LAST_STAGE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatData;
    use crate::combat::tests::{battle_with, leader_pp, table, tables, wild_hp};
    use crate::command::{Command, Direction, Outcome};
    use crate::rng::DungeonRng;
    use crate::state::{GameState, Monster};
    use crate::targeting::Slot;
    use rescue_data::moves::Move;

    /// A move row for the tests: no critical odds, 10 PP, targeting 0 (the monster on the facing
    /// tile). Its accuracy is the one the test passes.
    fn stat_move(id: u16, accuracy: [u8; 3]) -> Move {
        crate::combat::tests::move_of(id, 0, 0, accuracy, 0)
    }

    /// The rng after one `DungeonRandInt(100)` from `rng`: the one draw of a slot-0 hit check.
    fn after_one_draw(rng: &DungeonRng) -> DungeonRng {
        let mut r = *rng;
        r.int(100);
        r
    }

    /// The handler of `mv` on `target`, without the leader's turn, so the draws are the
    /// handler's alone. Returns whether the use defeated the leader.
    fn use_on(g: &mut GameState, mv: &Move, effect: &Effect, target: Slot) -> bool {
        use_on_weighed(g, mv, effect, target, &[])
    }

    /// [`use_on`] with the species weights `weights` in the data (Low Kick's `arg8`).
    fn use_on_weighed(
        g: &mut GameState,
        mv: &Move,
        effect: &Effect,
        target: Slot,
        weights: &[u16],
    ) -> bool {
        let t = tables();
        let moves = table(std::slice::from_ref(mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights,
            levels: &[],
        };
        g.dungeon_mut()
            .unwrap()
            .strike(0, &[target], mv, effect, &data)
    }

    /// The wild monster's stages: (offensive, defensive, accuracy).
    fn wild_stages(g: &GameState) -> ([i16; 2], [i16; 2], i16) {
        let w = &g.dungeon().unwrap().wild()[0].info;
        (w.offensive_stages, w.defensive_stages, w.accuracy_stage)
    }

    /// Runs the handler of `id` on the wild monster with hit check `accuracy`, and checks the
    /// draws: one value, always (the dispatch's slot-0 check, which no handler here repeats).
    fn run_on_wild(id: u16, accuracy: [u8; 3]) -> (GameState, DungeonRng) {
        let mv = stat_move(id, accuracy);
        let effect = effect_of(id).unwrap();
        let mut g = battle_with(&mv);
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        (g, before)
    }

    /// Runs the handler of `id` on the wild monster, with its abilities set to `abilities` and a
    /// hit check of 100. Returns the state and the rng before the use.
    fn run_on_wild_with_abilities(id: u16, abilities: [u8; 2]) -> (GameState, DungeonRng) {
        let mv = stat_move(id, [100; 3]);
        let effect = effect_of(id).unwrap();
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().wild[0].info.abilities = abilities;
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        (g, before)
    }

    #[test]
    fn growl_hit_lowers_the_target_attack_stage_on_split_0_and_draws_one_value() {
        let (g, before) = run_on_wild(GROWL, [100; 3]);
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
        assert_eq!(wild_stages(&g), ([9, 10], [10, 10], 10));
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn growl_miss_at_the_dispatch_changes_nothing_and_draws_one_value() {
        let (g, before) = run_on_wild(GROWL, [0; 3]);
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
        assert_eq!(wild_stages(&g), ([10, 10], [10, 10], 10));
    }

    #[test]
    fn growl_is_blocked_by_abilities_0x0f_0x18_and_0x09_and_still_draws_one_value() {
        for ability in [0x0F, 0x18, 0x09] {
            let (g, before) = run_on_wild_with_abilities(GROWL, [0, ability]);
            assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
            assert_eq!(
                wild_stages(&g),
                ([10, 10], [10, 10], 10),
                "ability {ability:#04X}"
            );
        }
    }

    #[test]
    fn leer_is_blocked_by_abilities_0x0f_and_0x18_and_still_draws_one_value() {
        for ability in [0x0F, 0x18] {
            let (g, before) = run_on_wild_with_abilities(LEER, [ability, 0]);
            assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
            assert_eq!(
                wild_stages(&g),
                ([10, 10], [10, 10], 10),
                "ability {ability:#04X}"
            );
        }
    }

    #[test]
    fn sand_attack_is_blocked_by_abilities_0x0f_0x18_and_0x1e_and_still_draws_one_value() {
        for ability in [0x0F, 0x18, 0x1E] {
            let (g, before) = run_on_wild_with_abilities(SAND_ATTACK, [0, ability]);
            assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
            assert_eq!(
                wild_stages(&g),
                ([10, 10], [10, 10], 10),
                "ability {ability:#04X}"
            );
        }
    }

    #[test]
    fn each_block_applies_only_to_the_stage_change_it_is_listed_for() {
        // 0x1E stops the accuracy change only, so Growl still lowers the attack stage.
        let (g, _) = run_on_wild_with_abilities(GROWL, [0x1E, 0]);
        assert_eq!(wild_stages(&g), ([9, 10], [10, 10], 10));
        // 0x09 stops the attack change only, so Leer and Sand-Attack still take effect.
        let (g, _) = run_on_wild_with_abilities(LEER, [0x09, 0]);
        assert_eq!(wild_stages(&g), ([10, 10], [9, 10], 10));
        let (g, _) = run_on_wild_with_abilities(SAND_ATTACK, [0x09, 0]);
        assert_eq!(wild_stages(&g), ([10, 10], [10, 10], 9));
    }

    #[test]
    fn helping_hand_is_not_blocked_by_the_lowering_abilities() {
        let mv = stat_move(HELPING_HAND, [100; 3]);
        let effect = effect_of(HELPING_HAND).unwrap();
        let mut g = battle_with(&mv);
        let mut partner = Monster {
            pos: (6, 5),
            visible: true,
            info: g.dungeon().unwrap().team()[0].info.clone(),
        };
        partner.info.abilities = [0x0F, 0x18];
        g.dungeon_mut().unwrap().team.push(partner);
        use_on(&mut g, &mv, &effect, Slot::Team(1));
        assert_eq!(
            g.dungeon().unwrap().team()[1].info.offensive_stages,
            [11, 11]
        );
    }

    #[test]
    fn a_stage_at_the_bottom_stays_at_0() {
        let g = battle_with(&stat_move(GROWL, [100; 3]));
        let mut info = g.dungeon().unwrap().wild()[0].info.clone();
        info.offensive_stages = [0, 10];
        change_stages(&mut info, GROWL_CHANGE);
        assert_eq!(info.offensive_stages, [0, 10]);
    }

    #[test]
    fn leer_and_tail_whip_share_one_handler_that_lowers_the_defence_stage() {
        assert_eq!(effect_of(LEER), effect_of(TAIL_WHIP));
        for id in [LEER, TAIL_WHIP] {
            let (g, before) = run_on_wild(id, [100; 3]);
            assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
            assert_eq!(wild_stages(&g), ([10, 10], [9, 10], 10), "move {id:#05X}");
        }
    }

    #[test]
    fn leer_miss_at_the_dispatch_changes_nothing() {
        let (g, _) = run_on_wild(LEER, [0; 3]);
        assert_eq!(wild_stages(&g), ([10, 10], [10, 10], 10));
    }

    #[test]
    fn sand_attack_hit_lowers_the_target_accuracy_stage_and_draws_one_value() {
        let (g, before) = run_on_wild(SAND_ATTACK, [100; 3]);
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
        assert_eq!(wild_stages(&g), ([10, 10], [10, 10], 9));
    }

    #[test]
    fn helping_hand_raises_both_attack_splits_of_the_ally_and_not_the_user() {
        let mv = stat_move(HELPING_HAND, [100; 3]);
        let effect = effect_of(HELPING_HAND).unwrap();
        let mut g = battle_with(&mv);
        // A partner on the tile in front of the leader's team member.
        let partner = Monster {
            pos: (6, 5),
            visible: true,
            info: g.dungeon().unwrap().team()[0].info.clone(),
        };
        g.dungeon_mut().unwrap().team.push(partner);
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Team(1));
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
        let d = g.dungeon().unwrap();
        assert_eq!(d.team()[1].info.offensive_stages, [11, 11]);
        assert_eq!(d.team()[1].info.defensive_stages, [10, 10]);
        assert_eq!(d.team()[0].info.offensive_stages, [10, 10]);
    }

    #[test]
    fn a_stage_change_is_kept_within_the_table() {
        let mut info = battle_with(&stat_move(SAND_ATTACK, [100; 3]))
            .dungeon()
            .unwrap()
            .wild()[0]
            .info
            .clone();
        info.accuracy_stage = 20;
        change_stages(
            &mut info,
            &[StageChange {
                stat: Stat::Accuracy,
                stages: 3,
                blocked_by: &[],
            }],
        );
        assert_eq!(info.accuracy_stage, 20);
    }

    #[test]
    fn a_use_of_growl_takes_its_pp_on_a_hit_and_on_a_miss() {
        for (accuracy, expect_stage) in [([100u8; 3], 9i16), ([0u8; 3], 10i16)] {
            let mv = stat_move(GROWL, accuracy);
            let t = tables();
            let moves = table(std::slice::from_ref(&mv));
            let data = CombatData {
                tables: &t,
                moves: &moves,
                exp_yields: &[],
                weights: &[],
                levels: &[],
            };
            let mut g = battle_with(&mv);
            let before_pp = leader_pp(&g);
            let outcome = g.step_with(
                Command::UseMove {
                    slot: 0,
                    direction: Some(Direction::Right),
                },
                &data,
            );
            assert_eq!(outcome, Outcome::Applied);
            assert_eq!(leader_pp(&g), before_pp - 1);
            assert_eq!(wild_stages(&g).0[0], expect_stage);
        }
    }

    #[test]
    fn the_moves_not_ported_have_no_handler() {
        // Thundershock and Hypnosis (see the shortcuts).
        for id in [0x0BC, 0x06F] {
            assert_eq!(effect_of(id), None, "move {id:#05X}");
        }
        // The damaging moves of the leader are ported.
        assert_eq!(effect_of(0x09A), Some(Effect::Damage { arg8: ARG8 }));
        assert_eq!(effect_of(0x104), Some(Effect::Damage { arg8: ARG8 }));
    }

    /// The wild monster's HP and the rng after one use of `id` on it, with power 40, physical
    /// type 1 and hit check `accuracy`. Tackle's use is the reference for the damaging path.
    fn damage_use(id: u16, accuracy: u8) -> (i16, DungeonRng) {
        let mv = move_of_power(id, [accuracy; 3]);
        let effect = effect_of(id).unwrap();
        let mut g = battle_with(&mv);
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        (wild_hp(&g), *g.dungeon().unwrap().rng())
    }

    fn move_of_power(id: u16, accuracy: [u8; 3]) -> Move {
        crate::combat::tests::move_of(id, 40, 1, accuracy, 0)
    }

    #[test]
    fn razor_leaf_is_the_damaging_path_with_arg8_one_and_one_draw_on_a_miss() {
        assert_eq!(effect_of(RAZOR_LEAF), Some(Effect::Damage { arg8: ARG8 }));
        // A hit does what Tackle does with the same power, type and accuracy: same HP, same rng.
        let (hp, rng) = damage_use(RAZOR_LEAF, 100);
        assert_eq!((hp, rng), damage_use(0x09A, 100));
        assert!(hp < 100);
        // A slot-0 miss stops after its one draw: no damage, one value drawn.
        let (hp, rng) = damage_use(RAZOR_LEAF, 0);
        assert_eq!(hp, 100);
        let before = {
            let mv = move_of_power(RAZOR_LEAF, [0; 3]);
            *battle_with(&mv).dungeon().unwrap().rng()
        };
        assert_eq!(rng, after_one_draw(&before));
    }

    #[test]
    fn struggle_is_the_damaging_path_at_arg8_one_then_a_recoil_of_a_quarter_of_max_hp() {
        assert_eq!(effect_of(STRUGGLE), Some(Effect::Struggle));
        // The target loses what Tackle takes at arg8 1.0 (same power, type and checks), and the
        // same draws are taken; the leader takes max HP / 4 (100 / 4 = 25) after the hit.
        let mv = move_of_power(STRUGGLE, [100; 3]);
        let effect = effect_of(STRUGGLE).unwrap();
        let mut g = battle_with(&mv);
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        assert_eq!(
            (wild_hp(&g), *g.dungeon().unwrap().rng()),
            damage_use(0x09A, 100)
        );
        assert!(wild_hp(&g) < 100);
        assert_eq!(g.dungeon().unwrap().team()[0].info.hp, 75);
    }

    #[test]
    fn struggle_second_check_miss_deals_nothing_and_takes_no_recoil() {
        // Accuracy 100 on the first check, 0 on the second: the first hits and the second misses.
        let mut mv = move_of_power(STRUGGLE, [100; 3]);
        mv.accuracy = [100, 0, 100];
        let effect = effect_of(STRUGGLE).unwrap();
        let mut g = battle_with(&mv);
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        let d = g.dungeon().unwrap();
        assert_eq!(wild_hp(&g), 100);
        assert_eq!(d.team()[0].info.hp, 100);
        // Five draws: the slot-0 check, the three CalcDamage draws, then the slot-1 check.
        let mut expect = before;
        expect.int(100);
        expect.int(100);
        expect.int(100);
        expect.int(0x4000);
        expect.int(100);
        assert_eq!(d.rng(), &expect);
    }

    #[test]
    fn struggle_recoil_is_at_least_one_and_a_recoil_that_takes_the_leader_to_0_is_a_defeat() {
        // Max HP 3: a quarter is 0, so the recoil is the minimum of 1.
        let mv = move_of_power(STRUGGLE, [100; 3]);
        let effect = effect_of(STRUGGLE).unwrap();
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().team[0].info.max_hp = 3;
        g.dungeon_mut().unwrap().team[0].info.hp = 2;
        assert!(!use_on(&mut g, &mv, &effect, Slot::Wild(0)));
        assert_eq!(g.dungeon().unwrap().team()[0].info.hp, 1);
        // At 1 HP the recoil takes the leader to 0: the use reports the leader's defeat.
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().team[0].info.max_hp = 3;
        g.dungeon_mut().unwrap().team[0].info.hp = 1;
        assert!(use_on(&mut g, &mv, &effect, Slot::Wild(0)));
        assert_eq!(g.dungeon().unwrap().team()[0].info.hp, 0);
    }

    #[test]
    fn absorb_is_the_damaging_path_then_the_user_heals_half_the_damage_at_least_one() {
        assert_eq!(effect_of(ABSORB), Some(Effect::Absorb));
        // The target loses what Tackle would take (same power, type and checks), and the same
        // draws are taken: the heal draws nothing.
        let mv = move_of_power(ABSORB, [100; 3]);
        let effect = effect_of(ABSORB).unwrap();
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().team[0].info.hp = 50;
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        let dealt = 100 - wild_hp(&g);
        assert!(dealt > 0);
        let d = g.dungeon().unwrap();
        assert_eq!(d.team()[0].info.hp, 50 + (dealt / 2).max(1));
        assert_eq!(d.rng(), &damage_use(0x09A, 100).1);
        // A heal stops at the leader's max HP.
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().team[0].info.hp = 99;
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        assert_eq!(g.dungeon().unwrap().team()[0].info.hp, 100);
        // A slot-0 miss takes one draw and heals nothing.
        let mv = move_of_power(ABSORB, [0; 3]);
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().team[0].info.hp = 50;
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Wild(0));
        assert_eq!(g.dungeon().unwrap().team()[0].info.hp, 50);
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
    }

    #[test]
    fn absorb_on_a_liquid_ooze_target_is_refused_before_any_draw() {
        // The ooze branch damages the user through `0217BCA4`, which is not read (see `combat`).
        let mv = move_of_power(ABSORB, [100; 3]);
        let t = tables();
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().wild[0].info.abilities = [0x3A, 0];
        let before = *g.dungeon().unwrap().rng();
        let outcome = g.step_with(
            Command::UseMove {
                slot: 0,
                direction: Some(Direction::Right),
            },
            &data,
        );
        assert_eq!(outcome, Outcome::NotModelled);
        assert_eq!(g.dungeon().unwrap().rng(), &before);
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn vine_whip_is_the_damaging_path_with_arg8_one_and_one_draw_on_a_miss() {
        assert_eq!(effect_of(VINE_WHIP), Some(Effect::Damage { arg8: ARG8 }));
        let (hp, rng) = damage_use(VINE_WHIP, 100);
        assert_eq!((hp, rng), damage_use(0x09A, 100));
        assert!(hp < 100);
        let (hp, rng) = damage_use(VINE_WHIP, 0);
        assert_eq!(hp, 100);
        let before = {
            let mv = move_of_power(VINE_WHIP, [0; 3]);
            *battle_with(&mv).dungeon().unwrap().rng()
        };
        assert_eq!(rng, after_one_draw(&before));
    }

    /// The weights table with `weight` at species `species` and 0 elsewhere.
    fn weights_with(species: usize, weight: u16) -> Vec<u16> {
        let mut w = vec![0u16; species + 1];
        w[species] = weight;
        w
    }

    #[test]
    fn low_kick_hit_is_the_damaging_path_with_the_target_species_weight_as_arg8() {
        assert_eq!(effect_of(LOW_KICK), Some(Effect::LowKick));
        let mv = move_of_power(LOW_KICK, [100; 3]);
        let weights = weights_with(5, 0x180);
        // The reference: the damaging path with `arg8` equal to the weight.
        let mut reference = battle_with(&mv);
        reference.dungeon_mut().unwrap().wild[0].info.species = 5;
        reference.dungeon_mut().unwrap().wild[0]
            .info
            .apparent_species = 5;
        use_on(
            &mut reference,
            &mv,
            &Effect::Damage { arg8: 0x180 },
            Slot::Wild(0),
        );
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().wild[0].info.species = 5;
        g.dungeon_mut().unwrap().wild[0].info.apparent_species = 5;
        use_on_weighed(&mut g, &mv, &Effect::LowKick, Slot::Wild(0), &weights);
        assert!(wild_hp(&g) < 100);
        assert_eq!(wild_hp(&g), wild_hp(&reference));
        assert_eq!(
            g.dungeon().unwrap().rng(),
            reference.dungeon().unwrap().rng()
        );
    }

    #[test]
    fn low_kick_miss_at_the_dispatch_draws_one_value_and_deals_nothing() {
        let mv = move_of_power(LOW_KICK, [0; 3]);
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().wild[0].info.species = 5;
        g.dungeon_mut().unwrap().wild[0].info.apparent_species = 5;
        let before = *g.dungeon().unwrap().rng();
        use_on_weighed(
            &mut g,
            &mv,
            &Effect::LowKick,
            Slot::Wild(0),
            &weights_with(5, 0x180),
        );
        assert_eq!(wild_hp(&g), 100);
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
    }

    #[test]
    fn low_kick_is_refused_before_any_draw_when_the_species_has_no_weight() {
        let mv = move_of_power(LOW_KICK, [100; 3]);
        let t = tables();
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with(&mv);
        g.dungeon_mut().unwrap().wild[0].info.species = 5;
        g.dungeon_mut().unwrap().wild[0].info.apparent_species = 5;
        let before = *g.dungeon().unwrap().rng();
        let outcome = g.step_with(
            Command::UseMove {
                slot: 0,
                direction: Some(Direction::Right),
            },
            &data,
        );
        assert_eq!(outcome, Outcome::NotModelled);
        assert_eq!(g.dungeon().unwrap().rng(), &before);
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn withdraw_raises_the_users_defence_stage_and_always_hits_the_user_after_one_draw() {
        assert_eq!(
            effect_of(WITHDRAW),
            Some(Effect::SelfStages(WITHDRAW_CHANGE))
        );
        // Accuracy 0 would miss a foe; on the user the check hits after its one draw, and the
        // stage goes up one with no block check.
        let mv = stat_move(WITHDRAW, [0; 3]);
        let effect = effect_of(WITHDRAW).unwrap();
        let mut g = battle_with(&mv);
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Team(0));
        let d = g.dungeon().unwrap();
        assert_eq!(d.rng(), &after_one_draw(&before));
        assert_eq!(d.team()[0].info.defensive_stages, [11, 10]);
        assert_eq!(d.team()[0].info.offensive_stages, [10, 10]);
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn withdraw_use_targets_the_user_and_takes_its_pp() {
        // The move row's targeting is the self class (`0x73`), as read from the dump (#421).
        let mut mv = stat_move(WITHDRAW, [125; 3]);
        mv.targeting = [0x73, 0x0];
        let t = tables();
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with(&mv);
        let before_pp = leader_pp(&g);
        let outcome = g.step_with(
            Command::UseMove {
                slot: 0,
                direction: Some(Direction::Right),
            },
            &data,
        );
        assert_eq!(outcome, Outcome::Applied);
        assert_eq!(leader_pp(&g), before_pp - 1);
        assert_eq!(
            g.dungeon().unwrap().team()[0].info.defensive_stages,
            [11, 10]
        );
    }

    #[test]
    fn water_sport_sets_the_water_counter_from_one_range_draw_and_changes_nothing_else() {
        assert_eq!(effect_of(WATER_SPORT), Some(Effect::Sport(Sport::Water)));
        // Accuracy 125 forces the slot-0 check; the handler then draws the turns once.
        let mv = move_of_power(WATER_SPORT, [125; 3]);
        let effect = effect_of(WATER_SPORT).unwrap();
        let mut g = battle_with(&mv);
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &effect, Slot::Team(0));
        let mut expect = before;
        expect.int(100);
        let turns = expect.range(SPORT_TURNS.0, SPORT_TURNS.1);
        let d = g.dungeon().unwrap();
        assert_eq!(d.rng(), &expect);
        assert_eq!(i32::from(d.water_sport), turns);
        assert!((10..=11).contains(&turns));
        assert_eq!(d.mud_sport, 0);
        assert_eq!(wild_hp(&g), 100);
        assert_eq!(d.team()[0].info.offensive_stages, [10, 10]);
    }

    #[test]
    fn mud_sport_sets_the_mud_counter_and_a_slot_0_miss_sets_nothing() {
        assert_eq!(effect_of(MUD_SPORT), Some(Effect::Sport(Sport::Mud)));
        let mv = move_of_power(MUD_SPORT, [125; 3]);
        let mut g = battle_with(&mv);
        use_on(&mut g, &mv, &Effect::Sport(Sport::Mud), Slot::Team(0));
        let d = g.dungeon().unwrap();
        assert!((10..=11).contains(&d.mud_sport));
        assert_eq!(d.water_sport, 0);
        // A miss stops after the slot-0 draw: no counter, one value drawn.
        let mv = move_of_power(MUD_SPORT, [0; 3]);
        let mut g = battle_with(&mv);
        let before = *g.dungeon().unwrap().rng();
        use_on(&mut g, &mv, &Effect::Sport(Sport::Mud), Slot::Team(0));
        assert_eq!(g.dungeon().unwrap().mud_sport, 0);
        assert_eq!(g.dungeon().unwrap().rng(), &after_one_draw(&before));
    }

    #[test]
    fn a_sport_use_takes_its_pp_and_its_user_is_its_only_target() {
        // The move row's targeting: the executor's self class (`0x73`), then the room area.
        let mut mv = move_of_power(WATER_SPORT, [125; 3]);
        mv.targeting = [0x73, 0x30];
        let t = tables();
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with(&mv);
        let before_pp = leader_pp(&g);
        let outcome = g.step_with(
            Command::UseMove {
                slot: 0,
                direction: Some(Direction::Right),
            },
            &data,
        );
        assert_eq!(outcome, Outcome::Applied);
        assert_eq!(leader_pp(&g), before_pp - 1);
        assert!((10..=11).contains(&g.dungeon().unwrap().water_sport));
    }
}
