//! The nine statuses of the first dungeon (`docs/re/status.md`, "First dungeon statuses (#359)"
//! and "Per-turn order"): paralysis, burn, sleep, flinch, whiffer, Focus Energy, enraged, Leech
//! Seed and a lowered speed. The fields live on [`EntityInfo`], so a snapshot keeps them.
//!
//! A setter follows its game routine: the checks that refuse come first, and the duration roll
//! (`CalculateStatusTurns`, `0219CF28`) is drawn where the routine draws it. Integer math only.

use serde::{Deserialize, Serialize};

use crate::rng::DungeonRng;
use crate::state::EntityInfo;

/// The turns value that never counts down (`DecrementStatusTurns`, `0219BF1C`).
pub const NO_END: u8 = 0x7F;

/// `+0x99` sleep class: sleep.
pub const SLEEP: u8 = 1;
/// `+0x9B` burn class: burn and paralysis.
pub const BURN: u8 = 1;
/// See [`BURN`].
pub const PARALYSIS: u8 = 4;
/// `+0xAC` cringe class: flinch.
pub const FLINCH: u8 = 1;
/// `+0xC4` sure-shot class: the target's attacks always miss.
pub const WHIFFER: u8 = 2;
/// `+0xC4` sure-shot class: Focus Energy.
pub const FOCUS_ENERGY: u8 = 4;
/// `+0xAE` bide class: enraged (Rage).
pub const ENRAGED: u8 = 0xC;
/// `+0xB8` leech seed class: Leech Seed.
pub const LEECH_SEED: u8 = 1;

/// The `(min, max)` pairs each setter passes to `CalculateStatusTurns` (overlay 1 words,
/// `docs/re/status.md`, "Setters, durations and effects"). The roll is `DungeonRandRange`, which
/// draws nothing when the two are equal.
const PARALYSIS_TURNS: (i32, i32) = (1, 2);
const BURN_TURNS: (i32, i32) = (127, 127);
const SLEEP_TURNS: (i32, i32) = (3, 7);
const FLINCH_TURNS: (i32, i32) = (1, 1);
const WHIFFER_TURNS: (i32, i32) = (1, 6);
const FOCUS_ENERGY_TURNS: (i32, i32) = (2, 4);
const RAGE_TURNS: (i32, i32) = (4, 10);
const LEECH_SEED_TURNS: (i32, i32) = (10, 12);
const SPEED_DOWN_TURNS: (i32, i32) = (6, 8);

/// Burn's damage: 5 (`021AA1BC`) every 20 end-of-turns (`021AA224`).
const BURN_DAMAGE: i16 = 5;
const BURN_PERIOD: u8 = 20;

/// The abilities the setters test (`docs/re/status.md`), by id.
const LIMBER: u8 = 0x16;
const WATER_VEIL: u8 = 0x42;
const INSOMNIA: u8 = 0x36;
const VITAL_SPIRIT: u8 = 0x49;
const EARLY_BIRD: u8 = 0x2F;
const FLINCH_BLOCK: u8 = 0x1F;
const TURNS_CAP: u8 = 0x14;
/// Fire (`MonsterIsType(e, 2)`) and Grass (`MonsterIsType(e, 4)`) type ids.
const FIRE: u8 = 2;
const GRASS: u8 = 4;

/// The speed stage a wild monster has before its counters: the wild loop gates every wild
/// monster at stage 1 (see [`crate::ai`]).
// shortcut: the wild species' movement speed is not kept, so the base is the loop's stage 1.
pub(crate) const WILD_BASE_STAGE: i32 = 1;

/// The status bytes of one monster (`docs/re/structs.md`, "Entity info"). The speed counters are
/// [`EntityInfo::speed_up`] and [`EntityInfo::speed_down`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// `+0x99`: sleep class, 0 or [`SLEEP`].
    pub sleep: u8,
    /// `+0x9A`: sleep turns left.
    pub sleep_turns: u8,
    /// `+0x9B`: burn class, 0, [`BURN`] or [`PARALYSIS`].
    pub burn_class: u8,
    /// `+0x9C`: burn class turns left.
    pub burn_turns: u8,
    /// `+0x9D`: end-of-turns until the burn's next damage.
    pub burn_countdown: u8,
    /// `+0xAC`: cringe class, 0 or [`FLINCH`].
    pub cringe: u8,
    /// `+0xAD`: cringe turns left.
    pub cringe_turns: u8,
    /// `+0xAE`: bide class, 0 or [`ENRAGED`].
    pub enraged: u8,
    /// `+0xAF`: enraged turns left.
    pub enraged_turns: u8,
    /// `+0xB8`: leech seed class, 0 or [`LEECH_SEED`].
    pub leech_seed: u8,
    /// `+0xC1`: leech seed turns left.
    pub leech_turns: u8,
    /// `+0xC4`: sure-shot class, 0, [`WHIFFER`] or [`FOCUS_ENERGY`].
    pub sure_shot: u8,
    /// `+0xC5`: sure-shot turns left.
    pub sure_shot_turns: u8,
}

/// One of the nine statuses, as a setter call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Inflict {
    Paralysis,
    Burn,
    Sleep,
    Flinch,
    Whiffer,
    FocusEnergy,
    Enraged,
    LeechSeed,
    SpeedDown,
}

impl Inflict {
    /// Whether the move's range is the user (Focus Energy and Rage: the status lands on the user).
    // shortcut: both ranges are read as the user's, as in the game's move rows; the dump's
    // targeting word is not checked here, and `use_move` refuses a row that disagrees.
    pub(crate) const fn on_user(self) -> bool {
        matches!(self, Self::FocusEnergy | Self::Enraged)
    }
}

/// `CalculateStatusTurns` (`0219CF28`): one `DungeonRandRange` on `pair`, then, with `flag`, a cap
/// of 5 above 4 for ability `0x14`, and at least 1.
// shortcut: the IQ skill `0x12` halving (`021805B0`) is not applied: the sim keeps no IQ skills.
fn status_turns(rng: &mut DungeonRng, target: &EntityInfo, pair: (i32, i32), flag: bool) -> u8 {
    let mut roll = rng.range(pair.0, pair.1);
    if flag && target.has_ability(TURNS_CAP) && roll != i32::from(NO_END) && roll > 4 {
        roll = 5;
    }
    u8::try_from(roll.max(1)).unwrap_or(NO_END)
}

/// Runs the setter of `kind` on `target`. `same` is whether the user is the target; `base_stage`
/// is the speed stage before the counters (the leader's species speed, or [`WILD_BASE_STAGE`]).
// shortcut: Safeguard (`02180870`) is not modelled, so it never blocks a setter; the sim has no
// reflect-class state. Synchronize (`0x19`) after a paralysis or burn is not modelled either.
pub(crate) fn inflict(
    kind: Inflict,
    rng: &mut DungeonRng,
    target: &mut EntityInfo,
    same: bool,
    base_stage: i32,
) {
    match kind {
        Inflict::Paralysis => paralyse(rng, target),
        Inflict::Burn => burn(rng, target),
        Inflict::Sleep => sleep(rng, target),
        Inflict::Flinch => flinch(rng, target),
        Inflict::Whiffer => whiffer(rng, target),
        Inflict::FocusEnergy => focus_energy(rng, target),
        Inflict::Enraged => enrage(rng, target),
        Inflict::LeechSeed => leech_seed(rng, target, same),
        Inflict::SpeedDown => lower_speed(rng, target, base_stage),
    }
}

/// `021885A0`: Limber blocks; an already paralysed target keeps its turns; the draw comes after
/// both. Resets the burn countdown, which shares the class.
fn paralyse(rng: &mut DungeonRng, t: &mut EntityInfo) {
    if t.has_ability(LIMBER) || t.status.burn_class == PARALYSIS {
        return;
    }
    t.status.burn_class = PARALYSIS;
    t.status.burn_turns = status_turns(rng, t, PARALYSIS_TURNS, true).saturating_add(1);
    t.status.burn_countdown = 0;
}

/// `0218A710`: Water Veil, a Fire type and an already burned target stop it; the roll is a
/// constant pair, so no draw.
// shortcut: the water-tile block (a water tile with a non-lava dungeon) is not modelled: floors
// 1 to 3 of dungeon 0 have no such tile in the notes, and the sim keeps no per-monster tile test.
fn burn(rng: &mut DungeonRng, t: &mut EntityInfo) {
    if t.has_ability(WATER_VEIL) || t.types.contains(&FIRE) || t.status.burn_class == BURN {
        return;
    }
    t.status.burn_class = BURN;
    t.status.burn_turns = status_turns(rng, t, BURN_TURNS, true).saturating_add(1);
    t.status.burn_countdown = 0;
}

/// `0215E07C` then `0218B2D0` and `0218B420`: the roll is drawn first, even for a target that is
/// immune. Insomnia and Vital Spirit block; a sleeper keeps its turns; Early Bird halves a
/// finite roll to at least 1. The turns are stored as rolled, with no +1.
// shortcut: the immunity gate's Safeguard, IQ skill `0x11` and held item `0x15` are not modelled
// (no reflect-class state, IQ skills or held items); the sleepless (2) and napping (5) blocks are
// not reachable, as no first-dungeon move sets them.
fn sleep(rng: &mut DungeonRng, t: &mut EntityInfo) {
    let mut turns = status_turns(rng, t, SLEEP_TURNS, true);
    if t.has_ability(INSOMNIA) || t.has_ability(VITAL_SPIRIT) || t.status.sleep == SLEEP {
        return;
    }
    if turns != NO_END && t.has_ability(EARLY_BIRD) {
        turns = (turns / 2).max(1);
    }
    t.status.sleep = SLEEP;
    t.status.sleep_turns = turns;
}

/// `021887E0`: ability `0x1F` blocks; an already flinching target keeps its turns. The pair is
/// constant, so no draw.
fn flinch(rng: &mut DungeonRng, t: &mut EntityInfo) {
    if t.has_ability(FLINCH_BLOCK) || t.status.cringe == FLINCH {
        return;
    }
    t.status.cringe = FLINCH;
    t.status.cringe_turns = status_turns(rng, t, FLINCH_TURNS, true).saturating_add(1);
}

/// Smokescreen (`0215D5BC`): the wrapper rolls first, then `0218D778` stores roll + 1 unless the
/// target is already a whiffer.
fn whiffer(rng: &mut DungeonRng, t: &mut EntityInfo) {
    let turns = status_turns(rng, t, WHIFFER_TURNS, true);
    if t.status.sure_shot == WHIFFER {
        return;
    }
    t.status.sure_shot = WHIFFER;
    t.status.sure_shot_turns = turns.saturating_add(1);
}

/// `0218D5F0`: the roll sits after the already-set check, with the flag 0.
fn focus_energy(rng: &mut DungeonRng, t: &mut EntityInfo) {
    if t.status.sure_shot == FOCUS_ENERGY {
        return;
    }
    t.status.sure_shot = FOCUS_ENERGY;
    t.status.sure_shot_turns = status_turns(rng, t, FOCUS_ENERGY_TURNS, false).saturating_add(1);
}

/// `0218E428` with `0xC`: returns at once if already enraged; else sets the class and rolls
/// with the flag 0.
// shortcut: the move slot (`+0xB0`) and the flag `+0x11C` the game sets are not stored: no first
// dungeon reader of them is known.
fn enrage(rng: &mut DungeonRng, t: &mut EntityInfo) {
    if t.status.enraged == ENRAGED {
        return;
    }
    t.status.enraged = ENRAGED;
    t.status.enraged_turns = status_turns(rng, t, RAGE_TURNS, false).saturating_add(1);
}

/// `0218DAE4`: refuses a user that is its own target, a Grass target and an existing seed; the
/// roll comes after all three.
// shortcut: the seeder's slot and id (`+0xC0`, `+0xBC`) and the drain counter (`+0xC2`) are not
// stored: the per-turn drain is an open item (`docs/re/status.md`), so nothing reads them. The
// game's flag for the roll is read as 1 (`docs/re/status.md`, "Duration roll").
fn leech_seed(rng: &mut DungeonRng, t: &mut EntityInfo, same: bool) {
    if same || t.types.contains(&GRASS) || t.status.leech_seed == LEECH_SEED {
        return;
    }
    t.status.leech_seed = LEECH_SEED;
    t.status.leech_turns = status_turns(rng, t, LEECH_SEED_TURNS, true).saturating_add(1);
}

/// `021882D0` with one stage: at speed stage 0 nothing changes and nothing is drawn; else the
/// first free down counter takes roll + 1 (no free one: no draw).
fn lower_speed(rng: &mut DungeonRng, t: &mut EntityInfo, base_stage: i32) {
    if speed_stage(t, base_stage) == 0 {
        return;
    }
    if let Some(i) = t.speed_down.iter().position(|&c| c == 0) {
        let turns = status_turns(rng, t, SPEED_DOWN_TURNS, true);
        t.speed_down[i] = turns.saturating_add(1);
    }
}

/// `CalcSpeedStage` (`02181E58`): `base_stage` plus one per speed-up counter, minus one per
/// speed-down counter and one for paralysis, clamped to 0 to 4.
// shortcut: the terms the sim has no state for are left out: snow on an ice type, Deoxys in
// Speed form, and a Kecleon that stole (see `Dungeon::calc_speed_stage`).
pub(crate) fn speed_stage(info: &EntityInfo, base_stage: i32) -> u8 {
    let count = |counters: &[u8; 5]| counters.iter().map(|&c| i32::from(c != 0)).sum::<i32>();
    let stage = base_stage + count(&info.speed_up)
        - count(&info.speed_down)
        - i32::from(info.status.burn_class == PARALYSIS);
    u8::try_from(stage.clamp(0, 4)).unwrap_or(4)
}

/// `DecrementStatusTurns` (`0219BF1C`): a nonzero value other than [`NO_END`] goes down by one.
/// Returns whether the value is 0 afterwards.
fn count_down(turns: &mut u8) -> bool {
    if *turns != 0 && *turns != NO_END {
        *turns -= 1;
    }
    *turns == 0
}

impl EntityInfo {
    /// Whether the monster does not act: `0217FD78` is true for a sleeper (`+0x99` of 1, 3 or 5).
    #[must_use]
    pub fn is_asleep(&self) -> bool {
        self.status.sleep != 0
    }

    /// The countdowns of `TickStatusAndHealthRegen` (`02185C70`), in the chain's order: sleep
    /// (step 2), burn class (3), cringe (5), leech seed (8), sure shot (9) and the speed counters
    /// (14). A class ends when its turns reach 0. It draws nothing.
    // shortcut: the steps for classes no first-dungeon move sets (frozen, reflect, curse and the
    // others) are not ticked, and the expiry messages are not printed. Yawning, nightmare and
    // napping are not sleep values here, so the yawn-end roll never happens.
    pub(crate) fn tick_status(&mut self) {
        let s = &mut self.status;
        if s.sleep != 0 && count_down(&mut s.sleep_turns) {
            s.sleep = 0;
        }
        if s.burn_class != 0 && count_down(&mut s.burn_turns) {
            s.burn_class = 0;
        }
        if s.cringe != 0 && count_down(&mut s.cringe_turns) {
            s.cringe = 0;
        }
        if s.leech_seed != 0 && count_down(&mut s.leech_turns) {
            s.leech_seed = 0;
        }
        if s.sure_shot != 0 && count_down(&mut s.sure_shot_turns) {
            s.sure_shot = 0;
        }
        for c in self.speed_down.iter_mut().chain(&mut self.speed_up) {
            count_down(c);
        }
    }

    /// The status part of `DoEndOfTurnEffects` (`021861E8`), after the Shed Skin roll: the burn
    /// damage (when its countdown is 0: 5 damage, then the countdown is 20), then the enraged
    /// count.
    // shortcut: the Leech Seed drain (`+0xC2`, then HP from the seeded to the seeder) is an open
    // item of `docs/re/status.md` and is not applied; poison, the frozen and curse blocks are not
    // set by a first-dungeon move. A burn that takes HP to 0 is not a faint (`HandleFaint_Async`
    // is not ported), as with the hunger damage. The burn's wake-on-damage check (`0218F21C`)
    // only matters for a `0x7F` sleep, which no first-dungeon move gives.
    pub(crate) fn end_of_turn_status(&mut self) {
        if self.status.burn_class == BURN {
            self.status.burn_countdown = self.status.burn_countdown.saturating_sub(1);
            if self.status.burn_countdown == 0 {
                self.status.burn_countdown = BURN_PERIOD;
                self.hp = (self.hp - BURN_DAMAGE).max(0);
            }
        }
        if self.status.enraged == ENRAGED && count_down(&mut self.status.enraged_turns) {
            self.status.enraged = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::DungeonRng;
    use crate::state::MoveSlot;

    /// A level 10 monster with 100 HP and no status.
    fn info() -> EntityInfo {
        EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: true,
            is_team_leader: false,
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
            types: [0, 0],
            abilities: [0, 0],
            moves: [MoveSlot::default(); 4],
            status: Status::default(),
        }
    }

    /// `(stored turns, draws)` of a setter, from a fresh rng: the draws are told by comparing the
    /// rng with one advanced by `n` values.
    fn run(kind: Inflict, info: &mut EntityInfo) -> (DungeonRng, DungeonRng) {
        let start = DungeonRng::from_state(0x1234_5678);
        let mut rng = start;
        inflict(kind, &mut rng, info, false, 1);
        (start, rng)
    }

    fn drew(start: &DungeonRng, end: &DungeonRng, n: u32) -> bool {
        let mut r = *start;
        for _ in 0..n {
            r.int(1);
        }
        &r == end
    }

    #[test]
    fn paralysis_stores_2_turns_after_one_draw_and_ends_by_countdown() {
        let mut t = info();
        let (s, e) = run(Inflict::Paralysis, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!((t.status.burn_class, t.status.burn_turns), (PARALYSIS, 2));
        t.tick_status();
        assert_eq!(t.status.burn_class, PARALYSIS);
        t.tick_status();
        assert_eq!(t.status.burn_class, 0);
    }

    #[test]
    fn paralysis_is_refused_by_limber_and_when_already_set_and_draws_nothing() {
        let mut t = info();
        t.abilities = [LIMBER, 0];
        let (s, e) = run(Inflict::Paralysis, &mut t);
        assert!(drew(&s, &e, 0));
        assert_eq!(t.status.burn_class, 0);
        let mut t = info();
        t.status.burn_class = PARALYSIS;
        t.status.burn_turns = 9;
        let (s, e) = run(Inflict::Paralysis, &mut t);
        assert!(drew(&s, &e, 0));
        assert_eq!(t.status.burn_turns, 9);
    }

    #[test]
    fn paralysis_takes_one_from_the_speed_stage() {
        let mut t = info();
        assert_eq!(speed_stage(&t, 2), 2);
        t.status.burn_class = PARALYSIS;
        assert_eq!(speed_stage(&t, 2), 1);
        assert_eq!(speed_stage(&t, 0), 0);
    }

    #[test]
    fn burn_stores_128_with_no_draw_ticks_to_127_and_never_expires() {
        let mut t = info();
        let (s, e) = run(Inflict::Burn, &mut t);
        assert!(drew(&s, &e, 0));
        assert_eq!((t.status.burn_class, t.status.burn_turns), (BURN, 128));
        t.tick_status();
        assert_eq!(t.status.burn_turns, 127);
        for _ in 0..300 {
            t.tick_status();
        }
        assert_eq!((t.status.burn_class, t.status.burn_turns), (BURN, 127));
    }

    #[test]
    fn burn_is_refused_for_fire_types_and_water_veil() {
        for (types, abilities) in [([FIRE, 0], [0, 0]), ([0, 0], [0, WATER_VEIL])] {
            let mut t = info();
            t.types = types;
            t.abilities = abilities;
            run(Inflict::Burn, &mut t);
            assert_eq!(t.status.burn_class, 0);
        }
    }

    #[test]
    fn burn_hurts_5_at_the_first_end_of_turn_then_every_20th() {
        let mut t = info();
        run(Inflict::Burn, &mut t);
        let mut hp = Vec::new();
        for _ in 0..41 {
            t.end_of_turn_status();
            hp.push(t.hp);
        }
        assert_eq!((hp[0], hp[1], hp[19], hp[20]), (95, 95, 95, 90));
        assert_eq!(hp[40], 85);
        assert_eq!(t.status.burn_countdown, BURN_PERIOD);
    }

    #[test]
    fn sleep_stores_3_to_6_after_one_draw_and_wakes_when_the_turns_run_out() {
        let mut t = info();
        let (s, e) = run(Inflict::Sleep, &mut t);
        assert!(drew(&s, &e, 1));
        let turns = t.status.sleep_turns;
        assert!((3..=6).contains(&turns));
        assert!(t.is_asleep());
        for _ in 0..turns - 1 {
            t.tick_status();
            assert!(t.is_asleep());
        }
        t.tick_status();
        assert!(!t.is_asleep());
    }

    #[test]
    fn sleep_draws_before_the_immunity_check_and_keeps_a_sleeper() {
        let mut t = info();
        t.abilities = [0, INSOMNIA];
        let (s, e) = run(Inflict::Sleep, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!(t.status.sleep, 0);
        let mut t = info();
        t.status.sleep = SLEEP;
        t.status.sleep_turns = 9;
        run(Inflict::Sleep, &mut t);
        assert_eq!(t.status.sleep_turns, 9);
    }

    #[test]
    fn early_bird_halves_the_sleep_turns_to_at_least_1() {
        let mut plain = info();
        run(Inflict::Sleep, &mut plain);
        let mut bird = info();
        bird.abilities = [EARLY_BIRD, 0];
        run(Inflict::Sleep, &mut bird);
        assert_eq!(
            bird.status.sleep_turns,
            (plain.status.sleep_turns / 2).max(1)
        );
    }

    #[test]
    fn flinch_stores_2_with_no_draw_and_clears_on_the_second_tick() {
        let mut t = info();
        let (s, e) = run(Inflict::Flinch, &mut t);
        assert!(drew(&s, &e, 0));
        assert_eq!((t.status.cringe, t.status.cringe_turns), (FLINCH, 2));
        t.tick_status();
        assert_eq!((t.status.cringe, t.status.cringe_turns), (FLINCH, 1));
        t.tick_status();
        assert_eq!(t.status.cringe, 0);
        let mut t = info();
        t.abilities = [FLINCH_BLOCK, 0];
        run(Inflict::Flinch, &mut t);
        assert_eq!(t.status.cringe, 0);
    }

    #[test]
    fn whiffer_stores_2_to_6_after_one_draw_and_ends_by_countdown() {
        let mut t = info();
        let (s, e) = run(Inflict::Whiffer, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!(t.status.sure_shot, WHIFFER);
        let turns = t.status.sure_shot_turns;
        assert!((2..=6).contains(&turns));
        for _ in 0..turns {
            t.tick_status();
        }
        assert_eq!(t.status.sure_shot, 0);
    }

    #[test]
    fn whiffer_draws_even_when_already_set_and_keeps_its_turns() {
        let mut t = info();
        t.status.sure_shot = WHIFFER;
        t.status.sure_shot_turns = 9;
        let (s, e) = run(Inflict::Whiffer, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!(t.status.sure_shot_turns, 9);
    }

    #[test]
    fn focus_energy_stores_3_or_4_after_one_draw_and_none_when_already_set() {
        let mut t = info();
        let (s, e) = run(Inflict::FocusEnergy, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!(t.status.sure_shot, FOCUS_ENERGY);
        assert!((3..=4).contains(&t.status.sure_shot_turns));
        let turns = t.status.sure_shot_turns;
        let (s, e) = run(Inflict::FocusEnergy, &mut t);
        assert!(drew(&s, &e, 0));
        assert_eq!(t.status.sure_shot_turns, turns);
        for _ in 0..turns {
            t.tick_status();
        }
        assert_eq!(t.status.sure_shot, 0);
    }

    #[test]
    fn enraged_stores_5_to_10_after_one_draw_and_ends_at_the_end_of_turn() {
        let mut t = info();
        let (s, e) = run(Inflict::Enraged, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!(t.status.enraged, ENRAGED);
        let turns = t.status.enraged_turns;
        assert!((5..=10).contains(&turns));
        // The Tick chain does not count it.
        t.tick_status();
        assert_eq!(t.status.enraged_turns, turns);
        for _ in 0..turns {
            assert_eq!(t.status.enraged, ENRAGED);
            t.end_of_turn_status();
        }
        assert_eq!(t.status.enraged, 0);
        let mut t = info();
        t.status.enraged = ENRAGED;
        let (s, e) = run(Inflict::Enraged, &mut t);
        assert!(drew(&s, &e, 0));
    }

    #[test]
    fn leech_seed_stores_11_or_12_after_one_draw_and_ends_by_countdown() {
        let mut t = info();
        let (s, e) = run(Inflict::LeechSeed, &mut t);
        assert!(drew(&s, &e, 1));
        assert_eq!(t.status.leech_seed, LEECH_SEED);
        let turns = t.status.leech_turns;
        assert!((11..=12).contains(&turns));
        for _ in 0..turns {
            t.tick_status();
        }
        assert_eq!(t.status.leech_seed, 0);
    }

    #[test]
    fn leech_seed_is_refused_on_self_grass_and_a_seeded_target_with_no_draw() {
        let mut grass = info();
        grass.types = [GRASS, 0];
        let mut seeded = info();
        seeded.status.leech_seed = LEECH_SEED;
        seeded.status.leech_turns = 3;
        for mut t in [grass, seeded] {
            let (s, e) = run(Inflict::LeechSeed, &mut t);
            assert!(drew(&s, &e, 0));
            assert!(t.status.leech_turns != 11 && t.status.leech_turns != 12);
        }
        let mut t = info();
        let mut rng = DungeonRng::from_state(1);
        inflict(Inflict::LeechSeed, &mut rng, &mut t, true, 1);
        assert_eq!(t.status.leech_seed, 0);
        assert_eq!(rng, DungeonRng::from_state(1));
    }

    #[test]
    fn speed_down_stores_7_or_8_in_the_first_free_counter_and_counts_down() {
        let mut t = info();
        let (s, e) = run(Inflict::SpeedDown, &mut t);
        assert!(drew(&s, &e, 1));
        assert!((7..=8).contains(&t.speed_down[0]));
        assert_eq!(&t.speed_down[1..], &[0; 4]);
        assert_eq!(speed_stage(&t, 1), 0);
        let turns = t.speed_down[0];
        for _ in 0..turns {
            t.tick_status();
        }
        assert_eq!(t.speed_down, [0; 5]);
        assert_eq!(speed_stage(&t, 1), 1);
    }

    #[test]
    fn speed_down_at_stage_0_changes_nothing_and_draws_nothing() {
        let mut t = info();
        let mut rng = DungeonRng::from_state(7);
        inflict(Inflict::SpeedDown, &mut rng, &mut t, false, 0);
        assert_eq!(t.speed_down, [0; 5]);
        assert_eq!(rng, DungeonRng::from_state(7));
    }

    #[test]
    fn the_cap_ability_limits_a_team_rolls_to_5_only_with_the_flag() {
        let mut t = info();
        t.abilities = [TURNS_CAP, 0];
        let mut rng = DungeonRng::from_state(1);
        // The pair (6, 8) rolls above 4; with the flag it is capped, without it is kept.
        assert_eq!(status_turns(&mut rng, &t, (6, 8), true), 5);
        assert!(status_turns(&mut rng, &t, (6, 8), false) >= 6);
        assert_eq!(status_turns(&mut rng, &t, (127, 127), true), NO_END);
    }
}
