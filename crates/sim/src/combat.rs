//! The damaging move of the leader: the hit check, the damage formula and the PP of one use
//! (`docs/re/combat.md`, "Damaging moves (#419)"). Integer math only: the game's 24.8 and 48.16
//! fixed point, with its rounding.
//!
//! Scope. The leader uses one of [`DAMAGING`]. Its target is the wild monster on the tile the
//! command faces; nothing beyond that tile is targeted (#420). The HP changes and nothing else
//! does: no defeat (#422), no secondary effect (#421). Regular Attack is not here, because its
//! route to the dispatcher is not traced. Items, statuses, weather and the Deoxys adjustment are
//! not modelled; each is a `shortcut:` note below.
//!
//! Draw order of one use (`docs/re/combat.md`, P6): the slot-0 hit check, then `CalcDamage`'s
//! boost roll, critical roll and variance roll, then the slot-1 hit check. A slot-0 miss stops
//! after its one draw. The hit check always draws, even when its accuracy forces the result.

use rescue_data::combat::CombatTables;
use rescue_data::moves::Move;

use crate::command::{Blocked, Direction, Outcome};
use crate::state::{DIRECTIONS, Dungeon, EntityInfo, MoveSlot};

/// The moves this module runs: Tackle (`0x09A`) and Scratch (`0x104`). The dispatch hands them
/// straight to `HandleDamagingMove` (`docs/re/move-effects.md`), and their `arg8` of 1.0 was
/// checked on 14 captured hits (#185).
pub const DAMAGING: [u16; 2] = [0x09A, 0x104];

/// `arg8`, the damage scale in 24.8, for [`DAMAGING`] moves: 1.0.
const ARG8: i64 = 0x100;

/// One in 48.16 (`0x10000`).
const ONE: i64 = 1 << 16;
/// One in 24.8 (`0x100`): the offensive and defensive multipliers, which no sim state changes.
// shortcut: the game's offensive and defensive multipliers (`info + 0x2C`, `+0x34`) are read as
// 1.0. Nothing in the sim sets them; read them from the info once an effect does.
const UNIT_248: i64 = 0x100;

/// The damage cap, and the upper clamp of the attack (`DAT_0217C7C8`, 999; `docs/re/combat.md`).
const DAMAGE_CAP: i64 = 999;
/// The level term of a team member, `level x 0xAAAA` in 48.16 (two thirds).
const TWO_THIRDS: i64 = 0xAAAA;
/// The 0.05 of the squared term, `0xCCC` in 48.16.
const ONE_TWENTIETH: i64 = 0xCCC;
/// The variance is `(0xE000 + roll) / 0x10000`, a factor from 0.875 to about 1.125.
const VARIANCE_BASE: i64 = 0xE000;
/// The type that Normal and Fighting moves do not reach a Ghost with (`sub_806E100`).
const GHOST: u8 = 14;
/// Normal and Fighting moves, the moves of the Ghost exception.
const NORMAL: u8 = 1;
const FIGHTING: u8 = 7;
/// The neutral chart cell (its multiplier is not applied).
const NEUTRAL_CELL: usize = 2;
/// The label of a super-effective pair in the label table.
const SUPER_LABEL: i32 = 3;

/// The ability IDs the attacker's attack boost reads (`CalcStatBoosts`; the IDs are the notes').
const HUGE_POWER: u8 = 0x22;
const PURE_POWER: u8 = 0x4B;
const HUSTLE: u8 = 0x30;

/// The move data and the combat tables a move command reads (see [`crate::state::GameState::step_with`]).
#[derive(Clone, Copy, Debug)]
pub struct CombatData<'a> {
    /// The damage and hit tables read from the dump.
    pub tables: &'a CombatTables,
    /// The move table, indexed by move ID (`rescue_data::moves::parse_moves`).
    pub moves: &'a [Move],
}

/// Stage index, clamped to the table (0 to 20).
fn stage_index(stage: i32) -> usize {
    usize::try_from(stage.clamp(0, 20)).unwrap_or(0)
}

/// 48.16 multiply, rounding half up on bit 15 (`F48_16_UMul`).
fn mul48(a: i64, b: i64) -> i64 {
    let p = a * b;
    (p >> 16) + ((p >> 15) & 1)
}

/// 24.8 multiply, rounding half up on bit 7 (`s24_8_mul`).
fn mul248(a: i64, b: i64) -> i64 {
    let p = a * b;
    (p >> 8) + ((p >> 7) & 1)
}

/// 48.16 to integer: the integer part, plus 1 when bit 15 is set (`FP48_16_ToS32`).
fn to_int(v: i64) -> i64 {
    (v >> 16) + ((v >> 15) & 1)
}

/// Whether a move of type `kind` is physical (`FUN_02008C64` is nonzero).
fn is_physical(t: &CombatTables, kind: u8) -> bool {
    t.physical.get(usize::from(kind)).copied().unwrap_or(0) != 0
}

/// The chart cell (0 to 3) of move type `kind` against defender type `defender`. A type outside
/// the chart is neutral. The ROM test checks that every cell is 0 to 3.
fn chart_cell(t: &CombatTables, kind: u8, defender: u8) -> usize {
    let cell = t
        .type_chart
        .get(usize::from(kind))
        .and_then(|row| row.get(usize::from(defender)))
        .copied()
        .unwrap_or(2);
    usize::try_from(cell)
        .ok()
        .filter(|c| *c < 4)
        .unwrap_or(NEUTRAL_CELL)
}

/// The type factor of one hit (`sub_806E100`), 48.16, and whether the pair is super effective.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeFactor {
    /// The product of the two cells' multipliers, then STAB when the attacker shares the type.
    pub factor: i64,
    /// Whether the label is super effective (label 3).
    pub super_effective: bool,
}

/// The type factor of a move of type `kind` from `attacker` against `target` (`sub_806E100`).
/// The chart cell of each of the defender's two types multiplies the factor from the pool; the
/// label table combines the two cells; STAB multiplies the factor when the attacker has the
/// move's type.
#[must_use]
pub fn type_factor(
    t: &CombatTables,
    kind: u8,
    attacker: &EntityInfo,
    target: &EntityInfo,
) -> TypeFactor {
    let mut factor = ONE;
    let mut cells = [NEUTRAL_CELL; 2];
    for (slot, &defender) in target.types.iter().enumerate() {
        // shortcut: the game reads the defender's exposed flag (`info + 0xD2`) for the Ghost
        // exception. The sim has no such flag, so every Ghost is unexposed.
        let cell = if matches!(kind, NORMAL | FIGHTING) && defender == GHOST {
            0
        } else {
            chart_cell(t, kind, defender)
        };
        if cell != NEUTRAL_CELL {
            factor = mul48(factor, i64::from(t.cell_multiplier[cell]));
        }
        cells[slot] = cell;
    }
    let label = t.effectiveness[cells[0]][cells[1]];
    // shortcut: the weather (sun, rain, cloudy, fog), charging and the Wonder Guard branches of
    // `sub_806E100` are not modelled: the sim has no weather or charge state.
    if kind != 0 && attacker.types.contains(&kind) && factor != 0 {
        factor = mul48(factor, i64::from(t.stab));
    }
    TypeFactor {
        factor,
        super_effective: label == SUPER_LABEL,
    }
}

/// An accuracy entry, clamped to `0..=0x6400` as the game clamps it.
fn stage_entry(v: i32) -> i64 {
    i64::from(v).clamp(0, 0x6400)
}

/// Whether a hit check with accuracy byte `accuracy` hits on `roll` (`AccuracyCalc`, after its
/// draw). An accuracy above 100 always hits. Otherwise the roll must be below the threshold:
/// the accuracy scaled by the attacker's accuracy stage, then by the target's evasion stage.
#[must_use]
pub fn hits(
    t: &CombatTables,
    roll: i32,
    accuracy: u8,
    attacker: &EntityInfo,
    target: &EntityInfo,
    physical: bool,
) -> bool {
    if accuracy > 100 {
        return true;
    }
    let mut acc = i64::from(accuracy);
    acc =
        acc * stage_entry(t.accuracy_stage[stage_index(i32::from(attacker.accuracy_stage))]) / 256;
    // Hustle makes the target easier to hit with a physical move (`docs/re/combat.md`, P4).
    let mut evasion = i32::from(target.evasion_stage);
    if physical && attacker.has_ability(HUSTLE) {
        evasion += 2;
    }
    acc = acc * stage_entry(t.evasion_stage[stage_index(evasion)]) / 256;
    i64::from(roll) < acc
}

/// The random draws of one damaging hit after its hit check, in the game's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rolls {
    /// `DungeonRandInt(100)` at the top of `CalcDamage`: Huge Power and Pure Power boost only
    /// when it is 32 or less.
    pub boost: i32,
    /// `DungeonRandInt(100)`, the critical roll: a critical hit when it is below the odds.
    pub crit: i32,
    /// `DungeonRandInt(0x4000)`, the variance.
    pub variance: i32,
}

/// The damage of one hit and whether it was critical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Damage {
    /// The HP the hit takes off.
    pub value: i32,
    /// Whether the hit is critical. A critical hit that does no damage is not critical.
    pub crit: bool,
}

/// The damage of `mv` from a team member `attacker` against `target` (`CalcDamage`). Steps as
/// `docs/re/combat.md` lists them. The attacker is always a team member (the leader), so the
/// level term is the team branch.
// shortcut: the wild attacker's term (`attack / 3`, after a second boost pass) is not modelled,
// because the sim's attackers are the leader's moves only.
// shortcut: the held-item boosts, the Deoxys adjustment, the target's `+0xAE` defence stage, the
// Wonder Guard branch and the Battle and Shell Armor gate on the critical roll are not modelled.
// The game skips the critical roll for a target with Battle Armor (`0x0C`) or Shell Armor (`0x13`);
// this sim rolls for it, as the brief asks, and a runtime run on an armoured target is the check.
#[must_use]
pub fn damage(
    t: &CombatTables,
    mv: &Move,
    attacker: &EntityInfo,
    target: &EntityInfo,
    rolls: Rolls,
) -> Damage {
    let physical = is_physical(t, mv.kind);
    let split = usize::from(!physical);

    // Attack: (power + attack) x 256, times the attack stage and the offensive multiplier, in
    // 24.8; then truncated to an integer and clamped to 0 to 999 (step 5, step 8).
    let stage = stage_index(i32::from(attacker.offensive_stages[split]));
    let stat = mul248(
        mul248(
            (i64::from(mv.base_power) + i64::from(attacker.attack[split])) * 256,
            i64::from(t.attack_stage[stage]),
        ),
        UNIT_248,
    );
    let mut attack = (stat / 256).clamp(0, DAMAGE_CAP);

    // CalcStatBoosts (step 9): Huge Power and Pure Power for a roll of 32 or less, Hustle; both
    // for physical moves, each x 3/2 truncated.
    if physical {
        if rolls.boost <= 32
            && (attacker.has_ability(HUGE_POWER) || attacker.has_ability(PURE_POWER))
        {
            attack = attack * 3 / 2;
        }
        if attacker.has_ability(HUSTLE) {
            attack = attack * 3 / 2;
        }
    }

    // Defence (step 6): the defence times its stage multiplier, truncated; not clamped.
    let defence_stage = stage_index(i32::from(target.defensive_stages[split]));
    let defence = mul248(
        mul248(
            i64::from(target.defence[split]) * 256,
            i64::from(t.defence_stage[defence_stage]),
        ),
        UNIT_248,
    ) / 256;

    // Base damage x (steps 10 and 11): (attack - defence) / 8 plus the level term, in 48.16.
    let level_term = i64::from(attacker.level) * TWO_THIRDS;
    let x = (attack - defence) * ONE / 8 + level_term;

    // Base damage (step 12): x * x * 0.05 + (2x - defence + 10), clamped to [1, 999].
    let base = mul48(mul48(x, x), ONE_TWENTIETH) + mul48(x, 2 * ONE) - defence * ONE + 10 * ONE;
    let base = base.clamp(ONE, DAMAGE_CAP * ONE);

    // Type (step 13) and critical (step 14). The critical multiplier is the STAB one (1.5).
    let typing = type_factor(t, mv.kind, attacker, target).factor;
    let crit_odds = i64::from(mv.crit_chance);
    let crit = i64::from(rolls.crit) < crit_odds;
    let multiplier = if crit {
        mul48(typing, i64::from(t.stab))
    } else {
        typing
    };

    // arg8 (step 15), then the variance (step 16), then the integer (step 17).
    let value = mul48(base, multiplier);
    let scaled = mul48(value, ARG8 << 8);
    let damage = to_int(mul48(scaled, VARIANCE_BASE + i64::from(rolls.variance)));
    Damage {
        value: i32::try_from(damage).unwrap_or(0),
        crit: crit && damage != 0,
    }
}

impl Dungeon {
    /// The leader uses move slot `slot` at `direction` (its facing when `None`). The move must
    /// be one of [`DAMAGING`], target the foe on the facing tile, and have PP. A refused use
    /// changes nothing and takes no time; a use takes the leader's turn (see [`crate::turn`]).
    // shortcut: the leader does not turn to face `direction`, as the sim does not turn it on a
    // move either. With no facing direction in the command, the leader's stored direction is used.
    pub(crate) fn use_move(
        &mut self,
        slot: u8,
        direction: Option<Direction>,
        data: &CombatData<'_>,
    ) -> Outcome {
        let Some(leader) = self.team.first() else {
            return Outcome::Blocked(Blocked::NoLeader);
        };
        let Some(&moves) = leader.info.moves.get(usize::from(slot)) else {
            return Outcome::Blocked(Blocked::NoMove);
        };
        if moves.flags & MoveSlot::HOLDS_MOVE == 0 {
            return Outcome::Blocked(Blocked::NoMove);
        }
        if !DAMAGING.contains(&moves.id) {
            return Outcome::NotModelled;
        }
        let Some(mv) = data.moves.get(usize::from(moves.id)) else {
            return Outcome::NotModelled;
        };
        // Only a foe on the facing tile is modelled (`0x00` area, class 0; #420).
        if mv.targeting[0] & 0xFF != 0 {
            return Outcome::NotModelled;
        }
        if moves.pp == 0 {
            return Outcome::Blocked(Blocked::NoPp);
        }
        let facing =
            direction.unwrap_or(DIRECTIONS[usize::from(leader.info.direction) % DIRECTIONS.len()]);
        let (dx, dy) = facing.delta();
        let at = (
            leader.pos.0.saturating_add(i16::try_from(dx).unwrap_or(0)),
            leader.pos.1.saturating_add(i16::try_from(dy).unwrap_or(0)),
        );
        // shortcut: a facing tile with no wild monster is not modelled: what the game does with
        // an empty target is not read.
        let Some(index) = self.wild.iter().position(|m| m.pos == at) else {
            return Outcome::NotModelled;
        };
        let slot = usize::from(slot);
        self.take_leader_turn(|d| d.strike(slot, index, mv, data));
        Outcome::Applied
    }

    /// One use of the move in the leader's `slot` on wild monster `target`: the PP mark, the hit
    /// check, the damage, the second hit check, then the PP taken at the end (`docs/re/combat.md`,
    /// P1, P3, P6). A miss takes PP too.
    fn strike(&mut self, slot: usize, target: usize, mv: &Move, data: &CombatData<'_>) {
        if let Some(leader) = self.team.first_mut() {
            leader.info.moves[slot].flags2 |= MoveSlot::USE_MARK;
        }
        let Some(attacker) = self.team.first().map(|m| m.info.clone()) else {
            return;
        };
        let defender = self.wild[target].info.clone();
        let physical = is_physical(data.tables, mv.kind);
        let t = data.tables;

        let first = self.rng.int(100);
        if hits(t, first, mv.accuracy[0], &attacker, &defender, physical) {
            let rolls = Rolls {
                boost: self.rng.int(100),
                crit: self.rng.int(100),
                variance: self.rng.int(0x4000),
            };
            let dealt = damage(t, mv, &attacker, &defender, rolls);
            let second = self.rng.int(100);
            if hits(t, second, mv.accuracy[1], &attacker, &defender, physical) {
                // shortcut: no defeat (#422): HP stops at 0 and the faint is not run.
                let hp = i32::from(defender.hp) - dealt.value;
                self.wild[target].info.hp = i16::try_from(hp.max(0)).unwrap_or(i16::MAX);
            }
        }

        // The PP is taken at the end of the action, once per use (`02159B18`).
        // shortcut: the Pressure double take (mark `0x10`) and the bit-`0x04` free use are not
        // modelled, as their writers are not traced.
        let mslot = &mut self.team[0].info.moves[slot];
        if mslot.flags2 & MoveSlot::USE_MARK != 0 {
            mslot.flags2 &= !MoveSlot::USE_MARK;
            mslot.pp = mslot.pp.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::floor::Floor;
    use crate::rng::DungeonRng;
    use crate::state::{GameState, Monster};
    use rescue_data::combat::STAGES;
    use rescue_data::types;

    /// Hand-made tables. The values are the tests' own, not the game's: neutral stages are 256,
    /// the pool is 1/4, 3/4, 1 and 2, and a pair is super when either cell is 3.
    fn tables() -> CombatTables {
        let mut type_chart = [[2i16; types::TYPES]; types::TYPES];
        type_chart[1][2] = 3; // Normal is super effective against type 2
        type_chart[1][3] = 0; // and the immune class against type 3
        type_chart[5][2] = 1; // type 5 is resisted by type 2
        type_chart[5][3] = 0; // and by type 3
        let mut effectiveness = [[2i32; 4]; 4];
        for (c0, row) in effectiveness.iter_mut().enumerate() {
            for (c1, label) in row.iter_mut().enumerate() {
                *label = if c0 == 3 || c1 == 3 {
                    3
                } else if c0 == 0 && c1 == 0 {
                    0
                } else {
                    2
                };
            }
        }
        // Stage `i` is `base + sign * (i - 10) * slope`: neutral (10) is `base` in each table.
        let stage = |slope: i32, base: i32, sign: i32| {
            let mut t = [0i32; STAGES];
            for (i, v) in t.iter_mut().enumerate() {
                *v = base + sign * (i as i32 - 10) * slope;
            }
            t
        };
        let mut physical = [0u8; types::TYPES];
        for (t, flag) in physical.iter_mut().enumerate() {
            *flag = (t % 2) as u8;
        }
        CombatTables {
            type_chart,
            effectiveness,
            cell_multiplier: [0x4000, 0xC000, 0x10000, 0x20000],
            stab: 0x14000,
            physical,
            attack_stage: stage(0x10, 0x100, 1),
            defence_stage: stage(0x10, 0x100, 1),
            accuracy_stage: stage(0x20, 0x100, 1),
            evasion_stage: stage(0x10, 0x100, -1),
        }
    }

    fn move_of(id: u16, power: i16, kind: u8, accuracy: [u8; 3], crit: u8) -> Move {
        Move {
            id,
            name: String::new(),
            base_power: power,
            kind,
            targeting: [0, 0],
            base_pp: 10,
            ai_weight: 0,
            accuracy,
            chained_hits: 0,
            max_upgrade: 0,
            crit_chance: crit,
            magic_coat: false,
            snatchable: false,
            uses_mouth: false,
            cannot_hit_frozen: false,
            ignores_taunted: false,
            range: 0,
            description: String::new(),
            use_text: String::new(),
        }
    }

    /// Tackle (`0x9A`, power 40, physical type 1) with the given accuracy and critical odds.
    fn tackle(accuracy: [u8; 3], crit: u8) -> Move {
        move_of(0x9A, 40, 1, accuracy, crit)
    }

    /// A move table indexed by move ID, with `moves` placed at their IDs.
    fn table(moves: &[Move]) -> Vec<Move> {
        let mut table: Vec<Move> = (0..=0x104).map(|id| move_of(id, 0, 0, [0; 3], 0)).collect();
        for mv in moves {
            table[usize::from(mv.id)] = mv.clone();
        }
        table
    }

    /// A team member of level 10 with attack 11 and defence 5 (both splits), neutral stages.
    fn info(types: [u8; 2], abilities: [u8; 2]) -> EntityInfo {
        EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: true,
            level: 10,
            iq: 0,
            hp: 100,
            max_hp: 100,
            attack: [11, 11],
            defence: [5, 5],
            offensive_stages: [10, 10],
            defensive_stages: [10, 10],
            accuracy_stage: 10,
            evasion_stage: 10,
            speed_up: [0; 5],
            speed_down: [0; 5],
            direction: 2,
            belly: 100_000,
            belly_empty: false,
            regen_acc: 0,
            regen_speed: 200,
            types,
            abilities,
            moves: [MoveSlot::default(); 4],
        }
    }

    /// The leader with Tackle in slot 0 and a wild monster on the tile to its right.
    fn battle(accuracy: [u8; 3], crit: u8) -> GameState {
        let mv = tackle(accuracy, crit);
        let mut leader_info = info([9, 0], [0, 0]);
        leader_info.moves[0] = MoveSlot::new(&mv);
        let leader = Monster {
            pos: (5, 5),
            visible: true,
            info: leader_info,
        };
        let mut g = GameState::in_dungeon(1, Floor::new(), vec![leader], 2).unwrap();
        let wild = Monster {
            pos: (6, 5),
            visible: true,
            info: EntityInfo {
                not_team_member: true,
                is_team_leader: false,
                ..info([9, 0], [0, 0])
            },
        };
        g.dungeon_mut().unwrap().wild.push(wild);
        g
    }

    /// The five draws of a hit from a copy of `rng`: slot-0 check, boost, critical, variance,
    /// slot-1 check (`docs/re/combat.md`, P6).
    fn five_draws(rng: &DungeonRng) -> [i32; 5] {
        let mut r = *rng;
        [
            r.int(100),
            r.int(100),
            r.int(100),
            r.int(0x4000),
            r.int(100),
        ]
    }

    /// The draws of a battle at its start, so a test can choose accuracies that hit or miss.
    fn draws_at_start() -> [i32; 5] {
        five_draws(battle([100; 3], 0).dungeon().unwrap().rng())
    }

    /// One use of `mv` in slot 0 on the wild monster, without the leader's turn.
    fn strike(g: &mut GameState, mv: &Move, t: &CombatTables) {
        let moves = table(std::slice::from_ref(mv));
        let data = CombatData {
            tables: t,
            moves: &moves,
        };
        g.dungeon_mut().unwrap().strike(0, 0, mv, &data);
    }

    fn wild_hp(g: &GameState) -> i16 {
        g.dungeon().unwrap().wild()[0].info.hp
    }

    fn leader_pp(g: &GameState) -> u8 {
        g.dungeon().unwrap().team()[0].info.moves[0].pp
    }

    #[test]
    fn a_hit_draws_five_values_in_the_game_order() {
        let t = tables();
        let mv = tackle([100; 3], 0);
        let mut g = battle([100; 3], 0);
        let before = *g.dungeon().unwrap().rng();
        let draws = five_draws(&before);
        let attacker = g.dungeon().unwrap().team()[0].info.clone();
        let target = g.dungeon().unwrap().wild()[0].info.clone();
        let rolls = Rolls {
            boost: draws[1],
            crit: draws[2],
            variance: draws[3],
        };
        let expected = damage(&t, &mv, &attacker, &target, rolls).value;
        strike(&mut g, &mv, &t);
        let mut rest = before;
        for cap in [100, 100, 100, 0x4000, 100] {
            rest.int(cap);
        }
        assert_eq!(g.dungeon().unwrap().rng(), &rest);
        assert_eq!(i32::from(wild_hp(&g)), 100 - expected);
    }

    #[test]
    fn a_slot_zero_miss_draws_one_value_and_no_damage() {
        let t = tables();
        let draws = draws_at_start();
        // The slot-0 roll is `draws[0]`; a threshold equal to it is a miss.
        let mv = tackle([draws[0] as u8, 100, 100], 0);
        let mut g = battle([draws[0] as u8, 100, 100], 0);
        let before = *g.dungeon().unwrap().rng();
        strike(&mut g, &mv, &t);
        let mut rest = before;
        rest.int(100);
        assert_eq!(g.dungeon().unwrap().rng(), &rest);
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn a_slot_one_miss_keeps_the_five_draws_and_no_damage() {
        let t = tables();
        let draws = draws_at_start();
        let mv = tackle([100, draws[4] as u8, 100], 0);
        let mut g = battle([100, draws[4] as u8, 100], 0);
        let before = *g.dungeon().unwrap().rng();
        strike(&mut g, &mv, &t);
        let mut rest = before;
        for cap in [100, 100, 100, 0x4000, 100] {
            rest.int(cap);
        }
        assert_eq!(g.dungeon().unwrap().rng(), &rest);
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn pp_is_taken_once_after_a_hit_and_after_a_miss() {
        let t = tables();
        let mut g = battle([100; 3], 0);
        strike(&mut g, &tackle([100; 3], 0), &t);
        assert_eq!(leader_pp(&g), 9);
        let flags = g.dungeon().unwrap().team()[0].info.moves[0].flags2;
        assert_eq!(flags & MoveSlot::USE_MARK, 0, "the mark is cleared");
        let draws = draws_at_start();
        let miss = [draws[0] as u8, 100, 100];
        let mut g = battle(miss, 0);
        strike(&mut g, &tackle(miss, 0), &t);
        assert_eq!(leader_pp(&g), 9, "a miss takes PP as well");
        assert_eq!(wild_hp(&g), 100);
    }

    #[test]
    fn a_critical_roll_below_the_odds_is_critical_and_scales_by_the_stab_multiplier() {
        let t = tables();
        let attacker = info([0, 0], [0, 0]);
        let target = info([9, 0], [0, 0]);
        let rolls = Rolls {
            boost: 100,
            crit: 0,
            variance: 0x2000,
        };
        let plain = damage(&t, &tackle([100; 3], 0), &attacker, &target, rolls);
        assert_eq!(
            plain,
            Damage {
                value: 38,
                crit: false
            }
        );
        let crit = damage(&t, &tackle([100; 3], 100), &attacker, &target, rolls);
        assert_eq!(
            crit,
            Damage {
                value: 47,
                crit: true
            }
        );
        // The critical multiplier is the table's STAB value (1.25 here): a base of 37.5 times
        // 1.25 is 46.9, which rounds to 47.
        // A roll equal to the odds is not below them.
        let edge = damage(
            &t,
            &tackle([100; 3], 100),
            &attacker,
            &target,
            Rolls { crit: 100, ..rolls },
        );
        assert!(!edge.crit);
    }

    #[test]
    fn the_type_factor_multiplies_the_two_cells_and_reads_the_label() {
        let t = tables();
        let attacker = info([0, 0], [0, 0]);
        // Super effective against one type: the pool's 2.0, label 3.
        let s = type_factor(&t, 1, &attacker, &info([2, 9], [0, 0]));
        assert_eq!(
            s,
            TypeFactor {
                factor: 0x20000,
                super_effective: true
            }
        );
        // A dual type: 3/4 then 1/4 gives 3/16, label 2 (not super).
        let d = type_factor(&t, 5, &attacker, &info([2, 3], [0, 0]));
        assert_eq!(
            d,
            TypeFactor {
                factor: 0x3000,
                super_effective: false
            }
        );
        // A Ghost takes the immune cell from a Normal move (the exception, unexposed).
        let g = type_factor(&t, 1, &attacker, &info([14, 9], [0, 0]));
        assert_eq!(
            g,
            TypeFactor {
                factor: 0x4000,
                super_effective: false
            }
        );
        // STAB: the attacker has the move's type, so the factor takes the STAB multiplier.
        let stab = type_factor(&t, 1, &info([1, 0], [0, 0]), &info([9, 0], [0, 0]));
        assert_eq!(stab.factor, 0x14000);
    }

    #[test]
    fn the_accuracy_stages_move_the_threshold_both_ways() {
        let t = tables();
        let neutral = info([0, 0], [0, 0]);
        let target = info([9, 0], [0, 0]);
        // Accuracy stage 5 is 0.375: 100 * 96 / 256 = 37.
        let mut low = neutral.clone();
        low.accuracy_stage = 5;
        assert!(hits(&t, 36, 100, &low, &target, true));
        assert!(!hits(&t, 37, 100, &low, &target, true));
        // Evasion stage 15 is 0.6875: 100 * 176 / 256 = 68.
        let mut high = target.clone();
        high.evasion_stage = 15;
        assert!(hits(&t, 67, 100, &neutral, &high, true));
        assert!(!hits(&t, 68, 100, &neutral, &high, true));
        // Both: 37 * 176 / 256 = 25, each step truncated on its own.
        assert!(hits(&t, 24, 100, &low, &high, true));
        assert!(!hits(&t, 25, 100, &low, &high, true));
        // Hustle adds 2 to the target's evasion for a physical move: stage 12 is 224, so 87.
        let hustle = info([0, 0], [0x30, 0]);
        assert!(hits(&t, 86, 100, &hustle, &target, true));
        assert!(!hits(&t, 87, 100, &hustle, &target, true));
        // A special move does not get it: the threshold stays 100.
        assert!(hits(&t, 99, 100, &hustle, &target, false));
        // An accuracy above 100 always hits, even with a stage that lowers it.
        assert!(hits(&t, 99, 125, &low, &high, true));
    }

    #[test]
    fn the_attack_stage_sets_the_damage_at_neutral_lowered_and_raised() {
        let t = tables();
        let target = info([9, 0], [0, 0]);
        let mv = tackle([100; 3], 0);
        let rolls = Rolls {
            boost: 100,
            crit: 99,
            variance: 0x2000,
        };
        // Neutral: 51 attack against 5 defence at level 10: 38 (worked by hand).
        let neutral = info([0, 0], [0, 0]);
        assert_eq!(damage(&t, &mv, &neutral, &target, rolls).value, 38);
        // Lowered (stage 5, 0.6875): 51 * 0.6875 = 35 attack, which gives 31.
        let mut low = neutral.clone();
        low.offensive_stages = [5, 5];
        assert_eq!(damage(&t, &mv, &low, &target, rolls).value, 31);
        // Raised (stage 15, 1.3125): more damage than the neutral one.
        let mut high = neutral.clone();
        high.offensive_stages = [15, 15];
        assert!(damage(&t, &mv, &high, &target, rolls).value > 38);
    }

    #[test]
    fn a_use_at_the_leaders_turn_applies_the_damage_and_the_pp() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
        };
        let mut g = battle([100; 3], 0);
        let out = g.step_with(
            Command::UseMove {
                slot: 0,
                direction: Some(Direction::Right),
            },
            &data,
        );
        assert_eq!(out, Outcome::Applied);
        assert!(wild_hp(&g) < 100);
        assert_eq!(leader_pp(&g), 9);
    }

    #[test]
    fn a_refused_or_unmodelled_use_changes_nothing() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
        };
        let right = Some(Direction::Right);
        let mut g = battle([100; 3], 0);
        g.dungeon_mut().unwrap().team[0].info.moves[0].pp = 0;
        let before = g.clone();
        let no_pp = Command::UseMove {
            slot: 0,
            direction: right,
        };
        assert_eq!(g.step_with(no_pp, &data), Outcome::Blocked(Blocked::NoPp));
        assert_eq!(g, before);

        let empty = Command::UseMove {
            slot: 1,
            direction: right,
        };
        assert_eq!(g.step_with(empty, &data), Outcome::Blocked(Blocked::NoMove));
        let off = Command::UseMove {
            slot: 9,
            direction: right,
        };
        assert_eq!(g.step_with(off, &data), Outcome::Blocked(Blocked::NoMove));
        assert_eq!(g, before);

        // No monster on the facing tile: not modelled, so nothing changes.
        let left = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Left),
        };
        let mut open = battle([100; 3], 0);
        assert_eq!(open.step_with(left, &data), Outcome::NotModelled);

        // Without the tables a move is not modelled, as with `step`.
        let mut plain = battle([100; 3], 0);
        let use_it = Command::UseMove {
            slot: 0,
            direction: right,
        };
        assert_eq!(plain.step(use_it), Outcome::NotModelled);
        assert_eq!(wild_hp(&plain), 100);
    }
}
