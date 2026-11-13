//! The fractional-turn scheduler: which fractional turn the leader acts in (`docs/re/turns.md`,
//! "The floor loop", "One fractional turn", "Speed" and "One leader turn").
//!
//! A dungeon turn is 24 fractional turns, counted by `fractionalTurn` (`dungeon+0x650`). A
//! monster of speed stage `s` acts in fractional turn `k` only if `SPEED_TURNS[s]` lists `k`.
//! A leader command takes effect in the first fractional turn from the current one on that
//! lists it, and the fractional turns it runs through first only run the other monsters' steps.
//! Integer math only.

use crate::state::Dungeon;

/// Fractional turns in one dungeon turn (`fractionalTurn` cycles 0 to 23).
pub const FRACTIONAL_TURNS: u8 = 24;

/// The fastest speed stage. `CalcSpeedStage` clamps to 0 to 4.
pub const MAX_SPEED_STAGE: u8 = 4;

/// `gSpeedTurns`, one row per speed stage: the fractional turns (0 to 23) in which a monster of
/// that stage acts. Same values as Red (`docs/re/turns.md`, "Speed", table; 3, 6, 12, 18 and 24
/// gated turns per 24).
const SPEED_TURNS: [&[u8]; 5] = [
    &[7, 15, 23],
    &[3, 7, 11, 15, 19, 23],
    &[1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23],
    &[
        1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15, 17, 18, 19, 21, 22, 23,
    ],
    &[
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    ],
];

/// Whether a monster of speed stage `stage` acts in fractional turn `k`. A stage above
/// [`MAX_SPEED_STAGE`] is treated as the fastest, so every stage has a gated turn.
fn acts(stage: u8, k: u8) -> bool {
    let row = usize::from(stage.min(MAX_SPEED_STAGE));
    SPEED_TURNS.get(row).is_some_and(|turns| turns.contains(&k))
}

/// The next fractional turn after `k`, wrapping at [`FRACTIONAL_TURNS`].
const fn next(k: u8) -> u8 {
    (k + 1) % FRACTIONAL_TURNS
}

/// Belly lost per leader turn, in thousandths: 100, that is 0.1 (index 10 of the belly table at
/// `0x021AA824`; `docs/re/turns.md`, "Per-turn effects", belly).
const BELLY_PER_TURN: i32 = 100;

/// The regeneration speed bounds (`TickStatusAndHealthRegen`; `docs/re/turns.md`, HP regen).
const REGEN_SPEED_MIN: i32 = 30;
const REGEN_SPEED_MAX: i32 = 500;

impl Dungeon {
    /// Runs fractional turns until the leader may act, then runs `act`, the leader's command,
    /// and the rest of that fractional turn. Counts one dungeon turn. The caller checks that
    /// the command is valid first, so a refused command never advances the clock.
    ///
    /// The leader's end-of-turn (belly, hunger, the Shed Skin roll) runs at the weather step
    /// that follows the action, and the clock then runs on to the leader's next gate, where the
    /// game waits for input. So a command returns with its end-of-turn applied, as the game's
    /// row shows it (`docs/re/turns.md`, "Row timing"). Only a leader that acts on a fractional
    /// turn with no weather step after it can leave an end-of-turn pending.
    pub(crate) fn take_leader_turn(&mut self, act: impl FnOnce(&mut Dungeon)) {
        let k = self.run_to_leader_gate(self.fractional_turn);
        // The floor's first pass skips the regeneration tick (`param` TRUE in the floor loop).
        if self.turn > 0 || k > 0 {
            self.regenerate_leader();
        }
        act(self);
        self.pending_end_of_turns = self.pending_end_of_turns.saturating_add(1);
        self.run_other_monsters(k);
        self.weather_step(k);
        self.fractional_turn = self.run_to_leader_gate(next(k));
        self.turn = self.turn.saturating_add(1);
    }

    /// Runs the steps of fractional turns from `k` on, up to the leader's next gate, and returns
    /// that gate. Every stage has a gated turn, so this ends within one cycle of 24. The stage is
    /// read on each fractional turn, as the gate does (`CalcSpeedStage` runs per turn).
    fn run_to_leader_gate(&mut self, mut k: u8) -> u8 {
        while !acts(self.leader_stage(), k) {
            self.run_other_monsters(k);
            self.weather_step(k);
            k = next(k);
        }
        k
    }

    /// The other monsters' steps of fractional turn `k` (team non-leaders and wild monsters).
    fn run_other_monsters(&mut self, _k: u8) {
        // shortcut: no other monster acts yet, so a floor only moves with the leader; port
        // their turns (#73) before any monster other than the leader is simulated.
    }

    /// The weather step of fractional turn `k` (`sub_8044AB4`, `docs/re/turns.md`, "One leader
    /// turn" step 7). It runs when `gSpeedTurns[1][k + 1]` is set, and its display pass runs the
    /// end-of-turn of one pending leader move.
    fn weather_step(&mut self, k: u8) {
        if acts(1, next(k)) && self.pending_end_of_turns > 0 {
            self.pending_end_of_turns -= 1;
            self.leader_end_of_turn();
        }
    }

    /// `TickStatusAndHealthRegen` for the leader, before its action (`docs/re/turns.md`, HP
    /// regeneration). Skipped while the belly is empty. The status gate (status 2 or 3) is not
    /// modelled, as the sim has no statuses.
    fn regenerate_leader(&mut self) {
        let Some(leader) = self.team.first_mut() else {
            return;
        };
        let info = &mut leader.info;
        if info.belly_empty {
            return;
        }
        // shortcut: the Heal Ribbon, Wish and Rain Dish terms are not modelled (no held item,
        // status or weather yet); each would add to the speed before the clamp.
        let speed = i32::from(info.regen_speed).clamp(REGEN_SPEED_MIN, REGEN_SPEED_MAX);
        let mut acc = i32::from(info.regen_acc) + i32::from(info.max_hp);
        let mut hp = i32::from(info.hp);
        while acc >= speed {
            hp += 1;
            acc -= speed;
        }
        let hp = hp.min(i32::from(info.max_hp));
        info.hp = i16::try_from(hp).unwrap_or(info.hp);
        info.regen_acc = i16::try_from(acc).unwrap_or(info.regen_acc);
    }

    /// `DoEndOfTurnEffects` for the leader (`docs/re/turns.md`, "Per-turn effects"), in game
    /// order. It clears `bellyEmpty`. The Warp Scarf roll and the weather damage are not
    /// modelled (no held item, no weather). The Shed Skin roll, `DungeonRandInt(100)`, is drawn
    /// unconditionally so the floor generator advances as in the game; its result is unused
    /// until a status or ability reads it. Then the belly block: the belly drops by 0.1, and at
    /// 0 the leader is hungry (`bellyEmpty` set, 1 HP lost).
    fn leader_end_of_turn(&mut self) {
        // shortcut: the held-item decrement terms (Tight Belt, Stamina Band, Energy Saver, Diet
        // Ribbon, Heal Ribbon, Munch Belt) and the linked-move term are not modelled; every turn
        // costs the base 0.1. The empty-belly alert counter (`dungeon+0x669`) is not kept: it
        // changes no state the sim holds.
        // shortcut: the 1 HP of hunger is taken without a faint check; HP stops at 0 and the
        // faint exit (`HandleFaint_Async`) is not ported.
        let _shed_skin = self.rng.int(100);
        let Some(leader) = self.team.first_mut() else {
            return;
        };
        let info = &mut leader.info;
        info.belly_empty = false;
        info.belly = (info.belly - BELLY_PER_TURN).max(0);
        if info.belly == 0 {
            info.belly_empty = true;
            info.hp = (info.hp - 1).max(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Outcome};
    use crate::floor::Floor;
    use crate::state::{EntityInfo, GameState, Monster};

    /// The gated fractional turns, transcribed from `docs/re/turns.md` ("Speed", table).
    const NOTE: [&[u8]; 5] = [
        &[7, 15, 23],
        &[3, 7, 11, 15, 19, 23],
        &[1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23],
        &[
            1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15, 17, 18, 19, 21, 22, 23,
        ],
        &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
        ],
    ];

    fn leader_run(stage: u8) -> GameState {
        leader_with(stage, 30, 100_000)
    }

    /// A stage-`stage` floor with one leader of `hp` HP (max 30) and `belly` thousandths.
    fn leader_with(stage: u8, hp: i16, belly: i32) -> GameState {
        let info = EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: true,
            level: 8,
            iq: 0,
            hp,
            max_hp: 30,
            attack: [10, 10],
            defence: [5, 5],
            offensive_stages: [0, 0],
            speed_up: [0; 5],
            speed_down: [0; 5],
            direction: 0,
            belly,
            belly_empty: false,
            regen_acc: 0,
            regen_speed: 200,
        };
        let leader = Monster {
            pos: (5, 5),
            visible: true,
            info,
        };
        let mut g = GameState::in_dungeon(1, Floor::new(), vec![leader], 0).unwrap();
        g.set_speed_stage(stage);
        g
    }

    fn fractional_turn(g: &GameState) -> u8 {
        g.dungeon().unwrap().fractional_turn()
    }

    /// The fractional turns in which the leader acts over `cycles` dungeon turns of waiting.
    /// Each wait acts at the first gate from the counter's turn on, and then runs on to the
    /// next gate.
    fn acting_turns(stage: u8, cycles: u32) -> Vec<u8> {
        let mut g = leader_run(stage);
        let mut acted = Vec::new();
        let mut elapsed = 0u32;
        while elapsed < u32::from(FRACTIONAL_TURNS) * cycles {
            let before = fractional_turn(&g);
            assert_eq!(g.step(Command::Wait), Outcome::Applied);
            let after = fractional_turn(&g);
            elapsed += u32::from((after + FRACTIONAL_TURNS - before) % FRACTIONAL_TURNS);
            acted.push(first_gate(stage, before));
        }
        acted
    }

    /// The first gated turn of `stage` at or after `from`, in the cycle.
    fn first_gate(stage: u8, from: u8) -> u8 {
        (0..FRACTIONAL_TURNS)
            .map(|i| (from + i) % FRACTIONAL_TURNS)
            .find(|&k| acts(stage, k))
            .expect("every stage has a gated turn")
    }

    #[test]
    fn the_belly_falls_a_tenth_per_leader_turn() {
        let mut g = leader_run(1);
        for waits in 1..=5 {
            assert_eq!(g.step(Command::Wait), Outcome::Applied);
            let belly = g.dungeon().unwrap().team()[0].info.belly;
            assert_eq!(belly, 100_000 - 100 * waits, "after {waits} waits");
        }
    }

    #[test]
    fn regeneration_follows_the_accumulator() {
        // Speed 200, max HP 30: each tick adds 30 to the accumulator, and the seventh reaches
        // 210, so one HP and 10 are left (`docs/re/turns.md`, HP regeneration; the starter's run).
        let mut g = leader_with(1, 25, 100_000);
        for _ in 0..6 {
            assert_eq!(g.step(Command::Wait), Outcome::Applied);
        }
        let info = &g.dungeon().unwrap().team()[0].info;
        assert_eq!((info.hp, info.regen_acc), (25, 180));
        assert_eq!(g.step(Command::Wait), Outcome::Applied);
        let info = &g.dungeon().unwrap().team()[0].info;
        assert_eq!((info.hp, info.regen_acc), (26, 10));
    }

    #[test]
    fn the_dungeon_rng_steps_once_per_leader_turn() {
        let mut g = leader_run(1);
        let mut expected = *g.dungeon().unwrap().rng();
        for _ in 0..3 {
            assert_eq!(g.step(Command::Wait), Outcome::Applied);
            // The Shed Skin roll: one DungeonRandInt(100), and nothing else moves the generator.
            expected.int(100);
            assert_eq!(g.dungeon().unwrap().rng(), &expected);
        }
    }

    #[test]
    fn hunger_at_an_empty_belly_costs_one_hp_and_stops_regeneration() {
        let mut g = leader_with(1, 30, 100);
        assert_eq!(g.step(Command::Wait), Outcome::Applied);
        let info = &g.dungeon().unwrap().team()[0].info;
        assert_eq!((info.belly, info.belly_empty, info.hp), (0, true, 29));
        for hp in [28, 27] {
            assert_eq!(g.step(Command::Wait), Outcome::Applied);
            let info = &g.dungeon().unwrap().team()[0].info;
            assert_eq!((info.belly, info.belly_empty, info.hp), (0, true, hp));
            // The tick is skipped while empty, so the accumulator stays where it was.
            assert_eq!(info.regen_acc, 30);
        }
    }

    #[test]
    fn the_table_matches_the_note() {
        for (stage, expected) in NOTE.iter().enumerate() {
            let gated: Vec<u8> = (0..FRACTIONAL_TURNS)
                .filter(|&k| acts(stage as u8, k))
                .collect();
            assert_eq!(gated, *expected, "stage {stage}");
        }
    }

    #[test]
    fn a_leader_acts_on_the_table_turns_at_every_stage() {
        for (stage, expected) in NOTE.iter().enumerate() {
            let twice: Vec<u8> = expected.iter().chain(expected.iter()).copied().collect();
            assert_eq!(acting_turns(stage as u8, 2), twice, "stage {stage}");
        }
    }

    #[test]
    fn a_stage_above_the_fastest_acts_like_the_fastest() {
        for k in 0..FRACTIONAL_TURNS {
            assert_eq!(acts(9, k), acts(MAX_SPEED_STAGE, k), "turn {k}");
        }
    }
}
