//! The damaging move of the leader: the hit check, the damage formula and the PP of one use
//! (`docs/re/combat.md`, "Damaging moves (#419)"). Integer math only: the game's 24.8 and 48.16
//! fixed point, with its rounding.
//!
//! Scope. The leader uses one of [`DAMAGING`], or Regular Attack ([`REGULAR_ATTACK`], the attack
//! action `0x32`, `docs/re/combat.md`, P8). Its targets are the monsters the move's range picks
//! (`crate::targeting`, the executor's areas and classes), each hit in the game's order. The HP
//! changes, and the stage changes of [`crate::move_effects`] do: no defeat (#422), no secondary
//! effect (#421). Items, statuses, weather and the Deoxys adjustment are not modelled; each is a
//! `shortcut:` note below.
//!
//! Draw order of one use (`docs/re/combat.md`, P6 and P8): the slot-0 hit check, then
//! `CalcDamage`'s boost roll, critical roll and variance roll, then the slot-1 hit check. The
//! critical roll is not drawn for a target with Battle Armor or Shell Armor. A slot-0 miss stops
//! after its one draw. The hit check always draws, even when its accuracy forces the result.
//! Regular Attack draws in the same order, and takes no PP.

use rescue_data::combat::CombatTables;
use rescue_data::moves::Move;
use rescue_data::species::LevelData;

use crate::command::{Blocked, Direction, Outcome};
use crate::move_effects::{self, Effect, SPORT_TURNS, Sport};
use crate::state::{DIRECTIONS, Dungeon, EntityInfo, MoveSlot};
use crate::targeting::{self, Slot};

/// The moves this module runs from a move slot: Tackle (`0x09A`) and Scratch (`0x104`). The
/// dispatch hands them straight to `HandleDamagingMove` (`docs/re/move-effects.md`), and their
/// `arg8` of 1.0 was checked on 14 captured hits (#185). Razor Leaf and Vine Whip take the same
/// path too; [`crate::move_effects::effect_of`] lists them.
pub const DAMAGING: [u16; 2] = [0x09A, 0x104];

/// Regular Attack: the leader's attack action (`0218453C`, case `0x32`, which calls
/// `02172ADC(entity, 0x163)`). It has no move slot and takes no PP. Its accuracy, power and
/// critical odds are read from its move-data row, not written here.
pub const REGULAR_ATTACK: u16 = 0x163;

/// `arg8` of Regular Attack, 24.8: 0.5 (the dispatcher's case `0x163` passes `0x80`).
const REGULAR_ATTACK_ARG8: i64 = 0x80;

/// `arg8`, the damage scale in 24.8, for [`DAMAGING`] moves: 1.0.
pub(crate) const ARG8: i64 = 0x100;

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
/// The target's ability that cuts a physical attack (`CalcStatBoosts`, `AbilityIsActive(target, 6)`).
const INTIMIDATE: u8 = 0x06;
/// The attacker's ability that adds 2 to its accuracy stage (`AccuracyCalc`, `AbilityIsActive(attacker, 0x33)`).
const COMPOUND_EYES: u8 = 0x33;
/// The target's abilities that skip the critical roll (`CalcDamage`, the two `AbilityIsActive` tests).
const BATTLE_ARMOR: u8 = 0x0C;
const SHELL_ARMOR: u8 = 0x13;
/// The target's ability that sends Absorb's heal to the user as damage (`0215F314`, the test
/// `0218067C(target, 0x3A)`). The name is a hypothesis: Red's ID was not checked.
const LIQUID_OOZE: u8 = 0x3A;
/// The EXP total cap (`AddExpPoints`, `DAT_02183FE8`, 9,999,999; `docs/re/combat.md`).
const EXP_CAP: i32 = 9_999_999;
/// The level a monster stops gaining EXP at (`AddExpPoints`, the early return on level 100).
const LEVEL_CAP: u8 = 100;

/// The move data and the combat tables a move command reads (see [`crate::state::GameState::step_with`]).
#[derive(Clone, Copy, Debug)]
pub struct CombatData<'a> {
    /// The damage and hit tables read from the dump.
    pub tables: &'a CombatTables,
    /// The move table, indexed by move ID (`rescue_data::moves::parse_moves`).
    pub moves: &'a [Move],
    /// The species' EXP yield, indexed by species ID (`Species::exp_yield`).
    pub exp_yields: &'a [i32],
    /// The species' weight, indexed by species ID (`Species::weight`, species `+0x2C`). Low
    /// Kick's `arg8` is the target's weight, at its apparent species (`ov_0001::0215C600`).
    pub weights: &'a [u16],
    /// The level tables, indexed by species ID: the table for species `n` is the `lvmpNNN` file
    /// with `NNN = n`, one record per level from 1 (`rescue_data::species::parse_level_data`).
    pub levels: &'a [Vec<LevelData>],
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
    // `sub_806E100` are not modelled: the sim has no weather or charge state. The sport counters
    // (`Dungeon::mud_sport`, `water_sport`) are kept but not read here: the game's sport
    // multiplier (`0217CE88`, value red-hypothesis) is not applied (#421).
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
    // Compound Eyes adds 2 to the attacker's accuracy stage before the clamp (`AccuracyCalc`, P4).
    let mut stage = i32::from(attacker.accuracy_stage);
    if attacker.has_ability(COMPOUND_EYES) {
        stage += 2;
    }
    acc = acc * stage_entry(t.accuracy_stage[stage_index(stage)]) / 256;
    // Hustle makes the target easier to hit with a physical move (`docs/re/combat.md`, P4).
    let mut evasion = i32::from(target.evasion_stage);
    if physical && attacker.has_ability(HUSTLE) {
        evasion += 2;
    }
    acc = acc * stage_entry(t.evasion_stage[stage_index(evasion)]) / 256;
    i64::from(roll) < acc
}

/// Whether `CalcDamage` draws the critical roll for `target`: not for Battle Armor or Shell Armor
/// (`docs/re/combat.md`, P5).
fn crit_drawn(target: &EntityInfo) -> bool {
    !target.has_ability(BATTLE_ARMOR) && !target.has_ability(SHELL_ARMOR)
}

/// The random draws of one damaging hit after its hit check, in the game's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rolls {
    /// `DungeonRandInt(100)` at the top of `CalcDamage`: Huge Power and Pure Power boost only
    /// when it is 32 or less.
    pub boost: i32,
    /// `DungeonRandInt(100)`, the critical roll: a critical hit when it is below the odds. Not
    /// drawn for a target where [`crit_drawn`] is false, and then it is ignored.
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

/// The attack after `CalcStatBoosts` (step 9): one numerator and one denominator, then a single
/// truncating divide, so the boosts do not truncate one by one. Huge Power and Pure Power (on a
/// boost roll of 32 or less), Hustle and Intimidate (the target's ability) apply to physical
/// moves only.
fn boosted_attack(
    attack: i64,
    physical: bool,
    boost: i32,
    attacker: &EntityInfo,
    target: &EntityInfo,
) -> i64 {
    if !physical {
        return attack;
    }
    let (mut num, mut den) = (1, 1);
    if boost <= 32 && (attacker.has_ability(HUGE_POWER) || attacker.has_ability(PURE_POWER)) {
        num *= 3;
        den *= 2;
    }
    if attacker.has_ability(HUSTLE) {
        num *= 3;
        den *= 2;
    }
    if target.has_ability(INTIMIDATE) {
        num *= 4;
        den *= 5;
    }
    attack * num / den
}

/// The damage of `mv` from a team member `attacker` against `target` (`CalcDamage`). Steps as
/// `docs/re/combat.md` lists them. The attacker is always a team member (the leader), so the
/// level term is the team branch.
// shortcut: the wild attacker's term (`attack / 3`, after a second boost pass) is not modelled,
// because the sim's attackers are the leader's moves only.
// shortcut: the held-item boosts, the Deoxys adjustment, the target's `+0xAE` defence stage and
// the Wonder Guard branch are not modelled.
// shortcut: Guts, Marvel Scale and Plus/Minus are not modelled: they need a negative status or a
// dungeon flag, and the sim keeps neither.
// shortcut: the IQ type-advantage critical term (`FUN_021805b0(attacker, 1)` on a super-effective
// hit, odds 40 in place of the others) is not modelled: the sim keeps no IQ skill state.
#[must_use]
pub fn damage(
    t: &CombatTables,
    mv: &Move,
    attacker: &EntityInfo,
    target: &EntityInfo,
    rolls: Rolls,
) -> Damage {
    damage_scaled(t, mv, ARG8, attacker, target, rolls)
}

/// [`damage`] with the move's `arg8` (24.8) given: a Regular Attack's is 0.5.
fn damage_scaled(
    t: &CombatTables,
    mv: &Move,
    arg8: i64,
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
    let attack = boosted_attack(
        (stat / 256).clamp(0, DAMAGE_CAP),
        physical,
        rolls.boost,
        attacker,
        target,
    );

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
    let crit = crit_drawn(target) && i64::from(rolls.crit) < crit_odds;
    let multiplier = if crit {
        mul48(typing, i64::from(t.stab))
    } else {
        typing
    };

    // arg8 (step 15), then the variance (step 16), then the integer (step 17).
    let value = mul48(base, multiplier);
    let scaled = mul48(value, arg8 << 8);
    let damage = to_int(mul48(scaled, VARIANCE_BASE + i64::from(rolls.variance)));
    Damage {
        value: i32::try_from(damage).unwrap_or(0),
        crit: crit && damage != 0,
    }
}

/// The EXP of one defeat: the yield plus `yield * (level - 1) / 10`, at least 1
/// (`CalculateEXPGain`, `docs/re/combat.md`; the multiplier is 1).
fn exp_amount(yield_: i32, level: u8) -> i32 {
    let base = yield_ + yield_ * (i32::from(level) - 1) / 10;
    base.max(1)
}

/// Adds `amount` to a monster's EXP, as `AddExpPoints` does (the cap, the level-100 lock), then
/// raises its level while the total reaches the level table (`docs/re/combat.md`).
// hypothesis: the level-up test reads `levels[level]` as the total needed for the next level
// (record `i` is the total for level `i + 1`). The game's trigger is not checked at runtime.
// shortcut: the stat gains of a level-up (`LevelData::hp`, `attack`, `defense`) are not applied:
// the game's gains are not verified.
fn add_exp(info: &mut EntityInfo, amount: i32, levels: &[LevelData]) {
    if info.level >= LEVEL_CAP {
        return;
    }
    let total = info.exp.saturating_add(amount).min(EXP_CAP);
    let delta = total - info.exp;
    info.exp = total;
    info.exp_gained = info.exp_gained.saturating_add(delta);
    while info.level < LEVEL_CAP {
        let Some(needed) = levels.get(usize::from(info.level)) else {
            break;
        };
        if info.exp < needed.exp_required {
            break;
        }
        info.level += 1;
    }
}

impl Dungeon {
    /// The leader uses move slot `slot` at `direction` (its facing when `None`). The move must
    /// have an effect in [`move_effects::effect_of`], have PP, and its range picks at least one
    /// monster. A refused use changes nothing and takes no time; a use takes the leader's turn
    /// (see [`crate::turn`]).
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
        let Some(effect) = move_effects::effect_of(moves.id) else {
            return Outcome::NotModelled;
        };
        let Some(mv) = data.moves.get(usize::from(moves.id)) else {
            return Outcome::NotModelled;
        };
        if moves.pp == 0 {
            return Outcome::Blocked(Blocked::NoPp);
        }
        let facing = self.facing(direction);
        let targets = match effect {
            // A sport's range is the user (`0x73`, the move row's executor value): its handler
            // sets a floor counter and reads no target, so the user is its one target.
            Effect::Sport(_) => {
                targeting::executor_targets(self, mv.id, mv.targeting[0], facing, None)
                    .filter(|t| t.as_slice() == [Slot::Team(0)])
            }
            _ => self.targets_of(mv, facing),
        };
        let Some(targets) = targets else {
            return Outcome::NotModelled;
        };
        // Absorb on a Liquid Ooze target takes the ooze branch, which damages the user through
        // `0217BCA4`; that routine is not read, so the use is refused before any draw (#421).
        if effect == Effect::Absorb
            && targets
                .iter()
                .any(|&t| self.info_of(t).is_some_and(|i| i.has_ability(LIQUID_OOZE)))
        {
            return Outcome::NotModelled;
        }
        // Low Kick needs the weight of each target's species; a species with no weight in the
        // table is refused before any draw, rather than dealing a damage of 0.
        if effect == Effect::LowKick
            && targets.iter().any(|&t| {
                self.info_of(t)
                    .is_none_or(|i| weight_of(data, i.apparent_species).is_none())
            })
        {
            return Outcome::NotModelled;
        }
        let slot = usize::from(slot);
        let mut leader_down = false;
        self.take_leader_turn(|d| leader_down = d.strike(slot, &targets, mv, &effect, data));
        if leader_down {
            return Outcome::LeaderDefeated;
        }
        Outcome::Applied
    }

    /// The leader's Regular Attack (action `0x32`) at `direction`: move `0x163` with no move slot
    /// and no PP (`docs/re/combat.md`, P8). Its targets are the monsters its range picks.
    // shortcut: the refusals of `02172ADC` (`info + 0xAC == 1`, `info + 0x9B == 4`) are not
    // modelled: the sim keeps neither field.
    // shortcut: the second strike of `02181D98` (weather or an ability) is not modelled.
    pub(crate) fn regular_attack(
        &mut self,
        direction: Direction,
        data: &CombatData<'_>,
    ) -> Outcome {
        if self.team.is_empty() {
            return Outcome::Blocked(Blocked::NoLeader);
        }
        let Some(mv) = data.moves.get(usize::from(REGULAR_ATTACK)) else {
            return Outcome::NotModelled;
        };
        let Some(targets) = self.targets_of(mv, direction) else {
            return Outcome::NotModelled;
        };
        let effect = Effect::Damage {
            arg8: REGULAR_ATTACK_ARG8,
        };
        let mut leader_down = false;
        self.take_leader_turn(|d| leader_down = d.resolve_all(&targets, mv, &effect, data));
        if leader_down {
            return Outcome::LeaderDefeated;
        }
        Outcome::Applied
    }

    /// The direction a command uses: `direction`, or the leader's stored direction when it is
    /// `None`.
    fn facing(&self, direction: Option<Direction>) -> Direction {
        let stored = self
            .team
            .first()
            .map_or(0, |l| usize::from(l.info.direction));
        direction.unwrap_or(DIRECTIONS[stored % DIRECTIONS.len()])
    }

    /// The monsters a use of `mv` by the leader facing `facing` hits, in the game's order
    /// (`targeting`). `None` when the range is not modelled, or when no monster would be hit.
    // shortcut: what the game does with an empty target is not read, so a use with no target
    // is not modelled.
    // shortcut: a target that is the user is not modelled: the damage path against the user is
    // not in the notes.
    fn targets_of(&self, mv: &Move, facing: Direction) -> Option<Vec<Slot>> {
        let targets = targeting::executor_targets(self, mv.id, mv.targeting[0], facing, None)?;
        if targets.is_empty() || targets.contains(&Slot::Team(0)) {
            return None;
        }
        Some(targets)
    }

    /// One use of the move in the leader's `slot` on each of `targets`, in order: the PP mark, the
    /// hit checks and effect of each target, then the PP taken at the end (`docs/re/combat.md`,
    /// P1, P3, P6). A miss takes PP too.
    pub(crate) fn strike(
        &mut self,
        slot: usize,
        targets: &[Slot],
        mv: &Move,
        effect: &Effect,
        data: &CombatData<'_>,
    ) -> bool {
        if let Some(leader) = self.team.first_mut() {
            leader.info.moves[slot].flags2 |= MoveSlot::USE_MARK;
        }
        let leader_down = self.resolve_all(targets, mv, effect, data);

        // The PP is taken at the end of the action, once per use (`02159B18`).
        // shortcut: the Pressure double take (mark `0x10`) and the bit-`0x04` free use are not
        // modelled, as their writers are not traced.
        let marked = self
            .team
            .first_mut()
            .and_then(|l| l.info.moves.get_mut(slot))
            .filter(|m| m.flags2 & MoveSlot::USE_MARK != 0);
        if let Some(mslot) = marked {
            mslot.flags2 &= !MoveSlot::USE_MARK;
            mslot.pp = mslot.pp.saturating_sub(1);
        }
        leader_down
    }

    /// The info of the monster at `slot`, if there is one.
    fn info_of(&self, slot: Slot) -> Option<&EntityInfo> {
        match slot {
            Slot::Team(i) => self.team.get(i).map(|m| &m.info),
            Slot::Wild(i) => self.wild.get(i).map(|m| &m.info),
        }
    }

    /// The info of the monster at `slot`, mutably, if there is one.
    fn info_mut(&mut self, slot: Slot) -> Option<&mut EntityInfo> {
        match slot {
            Slot::Team(i) => self.team.get_mut(i).map(|m| &mut m.info),
            Slot::Wild(i) => self.wild.get_mut(i).map(|m| &mut m.info),
        }
    }

    /// One use's hits on each of `targets`, in order, then the EXP of each wild monster the use
    /// took to 0 HP (see [`award_exp`](Self::award_exp)), then their defeat (see
    /// [`remove_defeated`](Self::remove_defeated)). Returns whether the leader was defeated.
    // shortcut: the game runs the faint of a target inside the damage path, as its hit takes it to
    // 0 HP, so the next target is hit after that faint. The sim removes the defeated monsters only
    // after the last hit, and does not run a faint's own draws or effects, so a use that defeats
    // one target and then hits another may differ from the game.
    fn resolve_all(
        &mut self,
        targets: &[Slot],
        mv: &Move,
        effect: &Effect,
        data: &CombatData<'_>,
    ) -> bool {
        let mut defeated = Vec::new();
        for &target in targets {
            if self.resolve(target, mv, effect, data) {
                defeated.push(target);
            }
        }
        let leader_down = defeated.contains(&Slot::Team(0));
        self.award_exp(&defeated, data);
        self.remove_defeated(&defeated);
        leader_down
    }

    /// The EXP of each wild monster in `defeated`, awarded to every team member
    /// (`docs/re/combat.md`, "Defeat in the sim (#422)"). One award per defeated monster.
    // hypothesis: the amount is `yield + yield * (level - 1) / 10` with the defeated monster's
    // level and species yield. The formula is static (matches Red); which level it reads is not
    // checked at runtime.
    // shortcut: the EXP multiplier at `+0xD8` is read as 1 (its writer of 2 is not located).
    // Absorb's write of `+0xD8` (0 to 1) is not stored: no entity field holds it, and the award
    // reads 1 either way (#421).
    // shortcut: a team member's join-location lock (`'J'`, `'G'` at `+0x40`) is not modelled.
    fn award_exp(&mut self, defeated: &[Slot], data: &CombatData<'_>) {
        let amounts: Vec<i32> = defeated
            .iter()
            .filter(|slot| matches!(slot, Slot::Wild(_)))
            .filter_map(|&slot| {
                let info = self.info_of(slot)?;
                let yield_ = *data.exp_yields.get(usize::from(info.species))?;
                Some(exp_amount(yield_, info.level))
            })
            .collect();
        for amount in amounts {
            for member in &mut self.team {
                // Each member levels from its own species' table; a species with no table
                // (missing from `levels`) gains EXP but does not level up.
                let table = data
                    .levels
                    .get(usize::from(member.info.species))
                    .map_or(&[][..], Vec::as_slice);
                add_exp(&mut member.info, amount, table);
            }
        }
    }

    /// Removes the wild monsters in `defeated` from the wild list. The list is compacted: the
    /// monsters that remain keep their order, which is the order the others act and draw in,
    /// but the ones after a removed monster move down an index.
    // shortcut: the wild list is compacted on a defeat; the game keeps the slot empty. Only the
    // indices change, not the order of the monsters that remain.
    // shortcut: a defeated team member stays in the team at 0 HP, and no faint is run for it
    // (`HandleFaint_Async`).
    fn remove_defeated(&mut self, defeated: &[Slot]) {
        let mut wild: Vec<usize> = defeated
            .iter()
            .filter_map(|slot| match *slot {
                Slot::Wild(i) => Some(i),
                Slot::Team(_) => None,
            })
            .collect();
        // Highest index first, so each removal leaves the lower indices where they are.
        wild.sort_unstable_by(|a, b| b.cmp(a));
        wild.dedup();
        for i in wild {
            if i < self.wild.len() {
                self.wild.remove(i);
            }
        }
    }

    /// The slot-0 hit check of one use by the leader on monster `target`, then the effect of
    /// `effect`: the damage with its second hit check, or the stage changes (`move_effects`). It
    /// takes no PP; [`strike`](Self::strike) and Regular Attack add that. Returns whether a hit
    /// took the target to 0 HP.
    fn resolve(&mut self, target: Slot, mv: &Move, effect: &Effect, data: &CombatData<'_>) -> bool {
        let Some(attacker) = self.team.first().map(|m| m.info.clone()) else {
            return false;
        };
        let Some(defender) = self.info_of(target).cloned() else {
            return false;
        };
        let physical = is_physical(data.tables, mv.kind);
        let t = data.tables;

        let first = self.rng.int(100);
        if !hits(t, first, mv.accuracy[0], &attacker, &defender, physical) {
            return false;
        }
        let arg8 = match effect {
            // The dispatch's slot-0 check is the only one a stage change makes.
            Effect::Stages(changes) => {
                if let Some(info) = self.info_mut(target) {
                    move_effects::change_stages(info, changes);
                }
                return false;
            }
            // A sport's handler draws its turns once, after the slot-0 check, and no damage.
            Effect::Sport(sport) => {
                self.start_sport(*sport);
                return false;
            }
            Effect::Damage { arg8 } => *arg8,
            // Absorb's HDM is the normal one, `0x100` (`0215F314`); its heal follows the damage.
            Effect::Absorb => ARG8,
            // Low Kick's HDM takes the target's weight as `arg8` (`0215C600`, P2 of #421).
            Effect::LowKick => weight_of(data, defender.apparent_species).unwrap_or(0),
        };
        // The critical roll is drawn only where `CalcDamage` draws it (`crit_drawn`).
        let rolls = Rolls {
            boost: self.rng.int(100),
            crit: if crit_drawn(&defender) {
                self.rng.int(100)
            } else {
                0
            },
            variance: self.rng.int(0x4000),
        };
        let dealt = damage_scaled(t, mv, arg8, &attacker, &defender, rolls);
        let second = self.rng.int(100);
        if !hits(t, second, mv.accuracy[1], &attacker, &defender, physical) {
            return false;
        }
        // HP stops at 0; a monster at 0 is defeated (`remove_defeated`).
        let hp = i32::from(defender.hp) - dealt.value;
        let hp = i16::try_from(hp.max(0)).unwrap_or(i16::MAX);
        if let Some(info) = self.info_mut(target) {
            info.hp = hp;
        }
        if matches!(effect, Effect::Absorb) && dealt.value != 0 {
            self.absorb_heal(dealt.value);
        }
        hp == 0
    }

    /// Absorb's heal: the leader gains half of `dealt` (at least 1), up to its max HP
    /// (`0215F314`, `0218DF38`, `docs/re/move-effects.md`). A miss or a 0 damage does not heal.
    fn absorb_heal(&mut self, dealt: i32) {
        let heal = (dealt / 2).max(1);
        if let Some(info) = self.team.first_mut().map(|m| &mut m.info) {
            let hp = (i32::from(info.hp) + heal).min(i32::from(info.max_hp));
            info.hp = i16::try_from(hp).unwrap_or(info.hp);
        }
    }

    /// A sport's handler (`021617C0`, then `021955B0`): the floor counter of `sport` takes the
    /// turns `CalculateStatusTurns(0, 10..12)` gives, one draw (`docs/re/move-effects.md`, P-c1
    /// of #421). The counter is ticked by [`crate::turn`].
    fn start_sport(&mut self, sport: Sport) {
        let (low, high) = SPORT_TURNS;
        let turns = u8::try_from(self.rng.range(low, high)).unwrap_or(0);
        match sport {
            Sport::Mud => self.mud_sport = turns,
            Sport::Water => self.water_sport = turns,
        }
    }
}

/// The weight of the species at `species` (Low Kick's `arg8`, 24.8), or `None` for a species
/// the table does not hold.
fn weight_of(data: &CombatData<'_>, species: u16) -> Option<i64> {
    data.weights
        .get(usize::from(species))
        .map(|&w| i64::from(w))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::command::Command;
    use crate::floor::Floor;
    use crate::floor::{TERRAIN_FLOOR, TERRAIN_TYPE_MASK};
    use crate::floor_gen::{FloorProperties, GenState, PortedSteps, generate_floor};
    use crate::rng::DungeonRng;
    use crate::state::{GameState, Monster};
    use rescue_data::combat::STAGES;
    use rescue_data::dungeon::MonsterSpawn;
    use rescue_data::types;

    /// Hand-made tables. The values are the tests' own, not the game's: neutral stages are 256,
    /// the pool is 1/4, 3/4, 1 and 2, and a pair is super when either cell is 3.
    pub(crate) fn tables() -> CombatTables {
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

    pub(crate) fn move_of(id: u16, power: i16, kind: u8, accuracy: [u8; 3], crit: u8) -> Move {
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
    pub(crate) fn table(moves: &[Move]) -> Vec<Move> {
        let mut table: Vec<Move> = (0..=REGULAR_ATTACK)
            .map(|id| move_of(id, 0, 0, [0; 3], 0))
            .collect();
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
            exp: 0,
            exp_gained: 0,
            types,
            abilities,
            moves: [MoveSlot::default(); 4],
        }
    }

    /// The leader with Tackle in slot 0 and a wild monster on the tile to its right.
    pub(crate) fn battle(accuracy: [u8; 3], crit: u8) -> GameState {
        battle_with(&tackle(accuracy, crit))
    }

    /// The leader with `mv` in slot 0 and a wild monster on the tile to its right.
    pub(crate) fn battle_with(mv: &Move) -> GameState {
        let mut leader_info = info([9, 0], [0, 0]);
        leader_info.moves[0] = MoveSlot::new(mv);
        let leader = Monster {
            pos: (5, 5),
            visible: true,
            info: leader_info,
        };
        let mut g = GameState::in_dungeon(1, Floor::new(), vec![leader], 2).unwrap();
        // The facing tile is open ground: `Floor::new()` leaves its type at 0 (no walkable tile).
        g.dungeon_mut()
            .unwrap()
            .floor
            .tile_mut(6, 5)
            .unwrap()
            .terrain = TERRAIN_FLOOR;
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
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        g.dungeon_mut().unwrap().strike(
            0,
            &[Slot::Wild(0)],
            mv,
            &Effect::Damage { arg8: ARG8 },
            &data,
        );
    }

    /// A fresh floor, generated with monsters, gets wild monsters on its enemy spawn marks, and
    /// the leader beside one reaches it with Regular Attack in a real run's order: the floor is
    /// generated, the wild list is set on entry, then the leader attacks.
    #[test]
    fn a_fresh_floor_has_a_wild_target_the_attack_reaches() {
        let props = FloorProperties {
            byte_06: 8,
            ..FloorProperties::default()
        };
        let mut floor = Floor::new();
        let mut rng = DungeonRng::from_state(0x1234_5678);
        let mut g = GenState::default();
        generate_floor(
            &props,
            false,
            &mut floor,
            &mut rng,
            &mut g,
            &mut PortedSteps::default(),
            false,
        )
        .expect("the floor generates");
        let leader = Monster {
            pos: (g.stairs_a.0, g.stairs_a.1),
            visible: true,
            info: info([0, 0], [0, 0]),
        };
        let spawn = MonsterSpawn {
            species: 1,
            level: 3,
            weights: [1, 1],
        };
        let mut game =
            GameState::in_dungeon_with_spawns(1, floor, vec![leader], 2, &[spawn]).unwrap();
        let dungeon = game.dungeon().unwrap();
        assert!(!dungeon.wild().is_empty(), "the floor has wild monsters");
        let target = dungeon.wild()[0].pos;

        // The tile one step from the target, in a direction whose tile is open ground.
        let (dir, stand) = DIRECTIONS
            .iter()
            .find_map(|&d| {
                let (dx, dy) = d.delta();
                let stand = (i32::from(target.0) - dx, i32::from(target.1) - dy);
                let tile = dungeon.floor().tile(stand.0, stand.1)?;
                let open = tile.terrain & TERRAIN_TYPE_MASK == TERRAIN_FLOOR;
                let stand = (i16::try_from(stand.0).ok()?, i16::try_from(stand.1).ok()?);
                let free = dungeon.wild().iter().all(|m| m.pos != stand);
                (open && free).then_some((d, stand))
            })
            .expect("a wild monster has open ground beside it");
        game.dungeon_mut().unwrap().team[0].pos = stand;

        let tables = tables();
        let moves = table(&[]);
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let turn = game.dungeon().unwrap().turn();
        assert_eq!(
            game.step_with(Command::Attack(dir), &data),
            Outcome::Applied
        );
        assert_eq!(
            game.dungeon().unwrap().turn(),
            turn + 1,
            "the attack takes a turn"
        );
    }

    pub(crate) fn wild_hp(g: &GameState) -> i16 {
        g.dungeon().unwrap().wild()[0].info.hp
    }

    pub(crate) fn leader_pp(g: &GameState) -> u8 {
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
    fn an_armoured_target_draws_no_critical_roll() {
        let t = tables();
        let mv = tackle([100; 3], 100);
        for armour in [BATTLE_ARMOR, SHELL_ARMOR] {
            let mut g = battle([100; 3], 100);
            g.dungeon_mut().unwrap().wild[0].info.abilities = [armour, 0];
            let before = *g.dungeon().unwrap().rng();
            // Slot-0 check, boost, variance, slot-1 check: four draws, with no critical roll.
            let mut rest = before;
            rest.int(100);
            let boost = rest.int(100);
            let variance = rest.int(0x4000);
            rest.int(100);
            let attacker = g.dungeon().unwrap().team()[0].info.clone();
            let target = g.dungeon().unwrap().wild()[0].info.clone();
            let expected = damage(
                &t,
                &mv,
                &attacker,
                &target,
                Rolls {
                    boost,
                    crit: 0,
                    variance,
                },
            );
            // The odds are 100, so a drawn roll would be critical: the armour makes it not.
            assert!(!expected.crit);
            strike(&mut g, &mv, &t);
            assert_eq!(g.dungeon().unwrap().rng(), &rest);
            assert_eq!(i32::from(wild_hp(&g)), 100 - expected.value);
        }
    }

    #[test]
    fn stat_boosts_take_one_divide_so_hustle_and_intimidate_differ_from_sequential_truncation() {
        let neutral = info([0, 0], [0, 0]);
        let hustle = info([0, 0], [HUSTLE, 0]);
        let huge = info([0, 0], [HUGE_POWER, 0]);
        let intimidate = info([9, 0], [INTIMIDATE, 0]);
        // Hustle and Intimidate in one fraction: 5 x 3 x 4 / (2 x 5) = 6. Truncating after each
        // boost would give 7, then 5.
        assert_eq!(boosted_attack(5, true, 100, &hustle, &intimidate), 6);
        assert_eq!(boosted_attack(5, true, 100, &neutral, &intimidate), 4);
        // Huge Power needs a boost roll of 32 or less.
        assert_eq!(boosted_attack(5, true, 32, &huge, &neutral), 7);
        assert_eq!(boosted_attack(5, true, 33, &huge, &neutral), 5);
        // A special move gets none of them.
        assert_eq!(boosted_attack(5, false, 100, &hustle, &intimidate), 5);
    }

    #[test]
    fn compound_eyes_adds_two_to_the_accuracy_stage_before_the_clamp() {
        let t = tables();
        let target = info([9, 0], [0, 0]);
        // Stage 8 is 0.75 (192): 100 x 192 / 256 = 75, so a roll of 75 misses.
        let mut low = info([0, 0], [0, 0]);
        low.accuracy_stage = 8;
        assert!(!hits(&t, 75, 100, &low, &target, true));
        // With Compound Eyes it is stage 10 (neutral), so the threshold is 100.
        let mut eyes = low.clone();
        eyes.abilities = [COMPOUND_EYES, 0];
        assert!(hits(&t, 75, 100, &eyes, &target, true));
        assert!(!hits(&t, 100, 100, &eyes, &target, true));
    }

    #[test]
    fn a_use_at_the_leaders_turn_applies_the_damage_and_the_pp() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
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
            exp_yields: &[],
            weights: &[],
            levels: &[],
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

    /// Regular Attack's move row for the tests: power 1, no critical odds, and the accuracy the
    /// test passes (the dump's are 125 and 93, `docs/re/combat.md`, P8).
    fn regular_attack_move(accuracy: [u8; 3]) -> Move {
        move_of(REGULAR_ATTACK, 1, 0, accuracy, 0)
    }

    /// One Regular Attack resolved on the wild monster at the tile to the right, without the
    /// leader's turn.
    fn regular_hit(g: &mut GameState, mv: &Move, t: &CombatTables) {
        let moves = table(std::slice::from_ref(mv));
        let data = CombatData {
            tables: t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        g.dungeon_mut().unwrap().resolve(
            Slot::Wild(0),
            mv,
            &Effect::Damage {
                arg8: REGULAR_ATTACK_ARG8,
            },
            &data,
        );
    }

    #[test]
    fn a_regular_attack_hit_draws_five_values_in_order_and_takes_no_pp() {
        let t = tables();
        // Both checks force a hit, so the five draws all run.
        let mv = regular_attack_move([125, 125, 0]);
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
        let expected = damage_scaled(&t, &mv, REGULAR_ATTACK_ARG8, &attacker, &target, rolls).value;
        regular_hit(&mut g, &mv, &t);
        let mut rest = before;
        for cap in [100, 100, 100, 0x4000, 100] {
            rest.int(cap);
        }
        assert_eq!(g.dungeon().unwrap().rng(), &rest);
        assert_eq!(i32::from(wild_hp(&g)), 100 - expected);
        assert_eq!(leader_pp(&g), 10, "Regular Attack takes no PP");
        let flags = g.dungeon().unwrap().team()[0].info.moves[0].flags2;
        assert_eq!(flags & MoveSlot::USE_MARK, 0, "no PP mark is set");
    }

    #[test]
    fn a_regular_attack_slot_zero_miss_draws_one_value_and_no_damage() {
        let t = tables();
        let draws = draws_at_start();
        // The slot-0 roll is `draws[0]`; a threshold equal to it is a miss.
        let mv = regular_attack_move([draws[0] as u8, 93, 0]);
        let mut g = battle([100; 3], 0);
        let before = *g.dungeon().unwrap().rng();
        regular_hit(&mut g, &mv, &t);
        let mut rest = before;
        rest.int(100);
        assert_eq!(g.dungeon().unwrap().rng(), &rest);
        assert_eq!(wild_hp(&g), 100);
        assert_eq!(leader_pp(&g), 10, "a miss takes no PP either");
    }

    #[test]
    fn regular_attack_arg8_halves_the_damage_of_a_damaging_move() {
        let t = tables();
        let attacker = info([0, 0], [0, 0]);
        let target = info([9, 0], [0, 0]);
        let rolls = Rolls {
            boost: 100,
            crit: 0,
            variance: 0x2000,
        };
        let mv = regular_attack_move([125, 125, 0]);
        let full = damage(&t, &mv, &attacker, &target, rolls).value;
        let half = damage_scaled(&t, &mv, REGULAR_ATTACK_ARG8, &attacker, &target, rolls).value;
        assert!(half > 0 && half < full, "half {half}, full {full}");
        // Each scaled value is rounded once, so the two may differ by a point.
        assert!((full - 2 * half).abs() <= 2, "half {half}, full {full}");
    }

    #[test]
    fn attack_is_modelled_on_a_foe_and_not_without_one() {
        let t = tables();
        let moves = table(&[regular_attack_move([125, 125, 0])]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle([100; 3], 0);
        let out = g.step_with(Command::Attack(Direction::Right), &data);
        assert_eq!(out, Outcome::Applied);
        assert!(
            wild_hp(&g) < 100,
            "both checks hit, so the foe takes damage"
        );
        assert_eq!(leader_pp(&g), 10);

        // Without the tables, or with no foe on the facing tile, nothing changes.
        let mut plain = battle([100; 3], 0);
        assert_eq!(
            plain.step(Command::Attack(Direction::Right)),
            Outcome::NotModelled
        );
        assert_eq!(wild_hp(&plain), 100);
        let mut open = battle([100; 3], 0);
        assert_eq!(
            open.step_with(Command::Attack(Direction::Left), &data),
            Outcome::NotModelled
        );
        assert_eq!(wild_hp(&open), 100);
    }

    /// `count` five-draw blocks of a hit, from a copy of `rng`, in order.
    fn draw_blocks(rng: &DungeonRng, count: usize) -> Vec<[i32; 5]> {
        let mut r = *rng;
        (0..count)
            .map(|_| {
                [
                    r.int(100),
                    r.int(100),
                    r.int(100),
                    r.int(0x4000),
                    r.int(100),
                ]
            })
            .collect()
    }

    /// A wild monster of the test info at `pos`.
    fn foe_at(pos: (i16, i16)) -> Monster {
        Monster {
            pos,
            visible: true,
            info: EntityInfo {
                not_team_member: true,
                is_team_leader: false,
                ..info([9, 0], [0, 0])
            },
        }
    }

    #[test]
    fn a_cone_move_hits_each_monster_in_cone_order_with_five_draws_each() {
        let t = tables();
        let mut mv = tackle([100; 3], 0);
        mv.targeting[0] = 0x10; // the cone: facing - 1, facing, facing + 1
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        // The leader at (5, 5) faces Right. Foes stand on the three cone tiles, listed out of
        // cone order, so only the cone order can give the draws to them in turn.
        let mut g = battle([100; 3], 0);
        g.dungeon_mut().unwrap().wild = vec![foe_at((6, 5)), foe_at((6, 4)), foe_at((6, 6))];
        let before = *g.dungeon().unwrap().rng();
        let blocks = draw_blocks(&before, 3);
        let attacker = g.dungeon().unwrap().team()[0].info.clone();
        let target = foe_at((6, 5)).info;
        let dealt = |b: [i32; 5]| {
            let rolls = Rolls {
                boost: b[1],
                crit: b[2],
                variance: b[3],
            };
            damage(&t, &mv, &attacker, &target, rolls).value
        };

        // The targets are the cone's, as the executor lists them; `strike` runs the draws.
        let targets =
            targeting::executor_targets(g.dungeon().unwrap(), 0x9A, 0x10, Direction::Right, None)
                .unwrap();
        assert_eq!(targets, vec![Slot::Wild(2), Slot::Wild(0), Slot::Wild(1)]);
        g.dungeon_mut()
            .unwrap()
            .strike(0, &targets, &mv, &Effect::Damage { arg8: ARG8 }, &data);
        let hp = |g: &GameState, i: usize| i32::from(g.dungeon().unwrap().wild()[i].info.hp);
        // DownRight (6, 6) is wild 2, Right (6, 5) is wild 0, UpRight (6, 4) is wild 1.
        assert_eq!(hp(&g, 2), 100 - dealt(blocks[0]));
        assert_eq!(hp(&g, 0), 100 - dealt(blocks[1]));
        assert_eq!(hp(&g, 1), 100 - dealt(blocks[2]));
        // Fifteen draws in all, and the PP is taken once for the use.
        let mut rest = before;
        for _ in 0..3 {
            for cap in [100, 100, 100, 0x4000, 100] {
                rest.int(cap);
            }
        }
        assert_eq!(*g.dungeon().unwrap().rng(), rest);
        assert_eq!(leader_pp(&g), 9);
    }

    #[test]
    fn an_around_move_hits_each_neighbour_in_facing_order_with_five_draws_each() {
        let t = tables();
        let mut mv = tackle([100; 3], 0);
        mv.targeting[0] = 0x20; // around: the eight neighbours from the facing direction on
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        // The leader at (5, 5) faces Right. Foes stand on three neighbours, listed out of
        // facing order: from Right on, the order is Right, UpRight, then DownRight last.
        let mut g = battle([100; 3], 0);
        g.dungeon_mut().unwrap().wild = vec![foe_at((6, 6)), foe_at((6, 5)), foe_at((6, 4))];
        let before = *g.dungeon().unwrap().rng();
        let blocks = draw_blocks(&before, 3);
        let attacker = g.dungeon().unwrap().team()[0].info.clone();
        let target = foe_at((6, 5)).info;
        let dealt = |b: [i32; 5]| {
            let rolls = Rolls {
                boost: b[1],
                crit: b[2],
                variance: b[3],
            };
            damage(&t, &mv, &attacker, &target, rolls).value
        };

        let targets =
            targeting::executor_targets(g.dungeon().unwrap(), 0xA0, 0x20, Direction::Right, None)
                .unwrap();
        assert_eq!(targets, vec![Slot::Wild(1), Slot::Wild(2), Slot::Wild(0)]);
        g.dungeon_mut()
            .unwrap()
            .strike(0, &targets, &mv, &Effect::Damage { arg8: ARG8 }, &data);
        let hp = |g: &GameState, i: usize| i32::from(g.dungeon().unwrap().wild()[i].info.hp);
        // Right (6, 5) is wild 1 and takes the first block, UpRight (6, 4) is wild 2 and takes
        // the second, DownRight (6, 6) is wild 0 and takes the third.
        assert_eq!(hp(&g, 1), 100 - dealt(blocks[0]));
        assert_eq!(hp(&g, 2), 100 - dealt(blocks[1]));
        assert_eq!(hp(&g, 0), 100 - dealt(blocks[2]));
        // Fifteen draws in all, and the PP is taken once for the use.
        let mut rest = before;
        for _ in 0..3 {
            for cap in [100, 100, 100, 0x4000, 100] {
                rest.int(cap);
            }
        }
        assert_eq!(*g.dungeon().unwrap().rng(), rest);
        assert_eq!(leader_pp(&g), 9);
    }

    #[test]
    fn a_line_move_hits_the_first_monster_on_the_line_even_an_ally() {
        let t = tables();
        let mut mv = tackle([100; 3], 0);
        mv.targeting[0] = 0x52; // the line, class 2: any monster
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        // The wild foe is past an ally on the line, so the ally is the first monster met.
        let mut g = battle([100; 3], 0);
        let d = g.dungeon_mut().unwrap();
        // The line walk reads the tiles: the test floor is open ground on the row.
        for x in 5..=10 {
            d.floor.tile_mut(x, 5).unwrap().terrain = TERRAIN_FLOOR;
        }
        d.wild[0].pos = (9, 5);
        d.team.push(Monster {
            pos: (7, 5),
            visible: true,
            info: info([9, 0], [0, 0]),
        });
        let out = g.step_with(
            Command::UseMove {
                slot: 0,
                direction: Some(Direction::Right),
            },
            &data,
        );
        assert_eq!(out, Outcome::Applied);
        let d = g.dungeon().unwrap();
        assert!(d.team()[1].info.hp < 100, "the ally is hit");
        assert_eq!(d.wild()[0].info.hp, 100, "the foe beyond it is not");
    }

    #[test]
    fn tackle_does_not_hit_an_ally_on_the_facing_tile() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        // The ally stands where the foe was: class 0 is a foe only, so the use is refused.
        let mut g = battle([100; 3], 0);
        let d = g.dungeon_mut().unwrap();
        d.wild.clear();
        d.team.push(Monster {
            pos: (6, 5),
            visible: true,
            info: info([9, 0], [0, 0]),
        });
        let before = g.clone();
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::NotModelled);
        assert_eq!(g, before);
    }

    /// The battle, with a second wild monster at (7, 5) at 50 HP beyond the first, which is at 1
    /// HP: a hit on the first takes it to 0.
    fn battle_with_a_defeat() -> GameState {
        let mut g = battle([100; 3], 0);
        let d = g.dungeon_mut().unwrap();
        d.wild[0].info.hp = 1;
        let mut far = d.wild[0].clone();
        far.pos = (7, 5);
        far.info.hp = 50;
        d.wild.push(far);
        g
    }

    /// The wild list after a defeat: one monster left, the one at (7, 5) with 50 HP.
    fn assert_survivor_only(g: &GameState) {
        let wild = g.dungeon().unwrap().wild();
        assert_eq!(wild.len(), 1, "the defeated monster is removed");
        assert_eq!(wild[0].pos, (7, 5));
        assert_eq!(wild[0].info.hp, 50, "the survivor is not hit");
    }

    #[test]
    fn a_move_that_takes_a_wild_monster_to_0_hp_removes_it() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with_a_defeat();
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::Applied);
        assert_survivor_only(&g);
    }

    #[test]
    fn a_regular_attack_that_takes_a_wild_monster_to_0_hp_removes_it() {
        let t = tables();
        let moves = table(&[regular_attack_move([100; 3])]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with_a_defeat();
        let attack = Command::Attack(Direction::Right);
        assert_eq!(g.step_with(attack, &data), Outcome::Applied);
        assert_survivor_only(&g);
    }

    #[test]
    fn one_use_that_defeats_two_wild_monsters_removes_both_and_keeps_the_middle_one() {
        let t = tables();
        let mut mv = tackle([100; 3], 0);
        mv.targeting[0] = 0x10; // the cone: facing - 1, facing, facing + 1
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        // The cone order is (6, 6), (6, 5), (6, 4): wild 2, wild 0, wild 1. Wild 0 and wild 2 are
        // at 1 HP, so the use defeats both; wild 1 at 100 HP survives its own hit.
        let mut g = battle([100; 3], 0);
        let mut low = foe_at((6, 5));
        low.info.hp = 1;
        let mut mid = foe_at((6, 4));
        mid.info.hp = 100;
        let mut low2 = foe_at((6, 6));
        low2.info.hp = 1;
        let target = mid.info.clone();
        g.dungeon_mut().unwrap().wild = vec![low, mid, low2];
        let before = *g.dungeon().unwrap().rng();
        let blocks = draw_blocks(&before, 3);
        let attacker = g.dungeon().unwrap().team()[0].info.clone();
        let rolls = Rolls {
            boost: blocks[2][1],
            crit: blocks[2][2],
            variance: blocks[2][3],
        };
        let dealt = damage(&t, &mv, &attacker, &target, rolls).value;
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::Applied);
        let wild = g.dungeon().unwrap().wild();
        assert_eq!(wild.len(), 1, "both defeated monsters are removed");
        assert_eq!(wild[0].pos, (6, 4));
        assert_eq!(
            i32::from(wild[0].info.hp),
            100 - dealt,
            "the survivor's HP changes only by its own hit"
        );
    }

    #[test]
    fn a_hit_that_leaves_a_wild_monster_above_0_hp_keeps_it() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle_with_a_defeat();
        g.dungeon_mut().unwrap().wild[0].info.hp = 100;
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::Applied);
        let wild = g.dungeon().unwrap().wild();
        assert_eq!(wild.len(), 2);
        assert!(wild[0].info.hp > 0 && wild[0].info.hp < 100);
    }

    #[test]
    fn a_move_whose_area_is_not_modelled_changes_nothing() {
        let t = tables();
        let mut mv = tackle([100; 3], 0);
        mv.targeting[0] = 0x90; // area nibble 9: not an area the notes list
        let moves = table(std::slice::from_ref(&mv));
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle([100; 3], 0);
        let before = g.clone();
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::NotModelled);
        assert_eq!(g, before);
    }

    /// `levels[i]` is the total for level `i + 1`: `step` EXP per level.
    fn linear_levels(step: i32) -> Vec<LevelData> {
        (0..100)
            .map(|i| LevelData {
                exp_required: i * step,
                hp: 0,
                attack: [0; 2],
                defense: [0; 2],
            })
            .collect()
    }

    #[test]
    fn the_exp_amount_is_the_yield_plus_a_tenth_per_level_above_1() {
        assert_eq!(exp_amount(15, 1), 15);
        assert_eq!(exp_amount(15, 11), 30);
        assert_eq!(exp_amount(40, 6), 60);
        assert_eq!(exp_amount(0, 5), 1, "an award is at least 1");
    }

    #[test]
    fn a_wild_defeat_awards_the_leader_the_formula_amount_for_its_level() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let mut g = battle_with_a_defeat();
        let (species, level) = {
            let wild = &g.dungeon().unwrap().wild()[0].info;
            (usize::from(wild.species), wild.level)
        };
        let mut yields = vec![0; species + 1];
        yields[species] = 40;
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &yields,
            weights: &[],
            levels: &[],
        };
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::Applied);
        let leader = &g.dungeon().unwrap().team()[0].info;
        assert_eq!(leader.exp_gained, exp_amount(40, level));
        assert_eq!(leader.exp, exp_amount(40, level));
        assert_eq!(leader.level, level, "no level table, so no level-up");
    }

    #[test]
    fn a_level_up_follows_the_table_and_stops_at_the_next_threshold() {
        let levels = linear_levels(100);
        let mut info = info([0, 0], [0, 0]);
        info.level = 5;
        // 600 EXP reaches levels[5] (500) and levels[6] (600), not levels[7] (700).
        add_exp(&mut info, 600, &levels);
        assert_eq!(info.level, 7);
        assert_eq!(info.exp, 600);
        assert_eq!(info.exp_gained, 600);
    }

    #[test]
    fn a_monster_at_level_100_gains_no_exp_and_the_total_is_capped() {
        let levels = linear_levels(100);
        let mut capped = info([0, 0], [0, 0]);
        capped.level = 100;
        add_exp(&mut capped, 500, &levels);
        assert_eq!(capped.exp, 0);
        assert_eq!(capped.exp_gained, 0);

        let mut near = info([0, 0], [0, 0]);
        near.exp = EXP_CAP - 5;
        add_exp(&mut near, 100, &[]);
        assert_eq!(near.exp, EXP_CAP, "the total stops at the cap");
        assert_eq!(near.exp_gained, 5, "the delta is what the cap let in");
    }

    #[test]
    fn each_team_member_levels_from_its_own_species_table() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let mut g = battle_with_a_defeat();
        let d = g.dungeon_mut().unwrap();
        // The leader is species 1 (its table is too high to reach); the partner is species 2
        // (its table is 10 EXP per level, so the 76 EXP award reaches level 8 and no further).
        d.team[0].info.level = 5;
        let mut partner = d.team[0].clone();
        partner.info.species = 2;
        partner.info.is_team_leader = false;
        partner.pos = (4, 5);
        d.team.push(partner);
        let wild_info = d.wild[0].info.clone();
        let amount = exp_amount(40, wild_info.level);
        let by_species = vec![Vec::new(), linear_levels(1000), linear_levels(10)];
        let yields = [0, 40, 0];
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &yields,
            weights: &[],
            levels: &by_species,
        };
        d.award_exp(&[Slot::Wild(0)], &data);
        let leader = &d.team()[0].info;
        assert_eq!(leader.level, 5, "species 1's table does not reach 5000 EXP");
        assert_eq!(leader.exp, amount);
        let partner = &d.team()[1].info;
        assert_eq!(partner.level, 8, "species 2's table reaches 70 EXP, not 80");
        assert_eq!(partner.exp, amount);
    }

    #[test]
    fn the_leaders_defeat_is_reported_and_its_hp_stops_at_0() {
        let t = tables();
        let moves = table(&[tackle([100; 3], 0)]);
        let data = CombatData {
            tables: &t,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut g = battle([100; 3], 0);
        let d = g.dungeon_mut().unwrap();
        d.team[0].info.hp = 1;
        let mv = tackle([100; 3], 0);
        assert!(d.resolve_all(&[Slot::Team(0)], &mv, &Effect::Damage { arg8: ARG8 }, &data));
        assert_eq!(d.team()[0].info.hp, 0);
        assert_eq!(
            d.wild().len(),
            1,
            "a leader's defeat removes no wild monster"
        );
    }
}
