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

impl Dungeon {
    /// Runs fractional turns until the leader may act, then runs `act`, the leader's command,
    /// and the rest of that fractional turn. Counts one dungeon turn. The caller checks that
    /// the command is valid first, so a refused command never advances the clock.
    pub(crate) fn take_leader_turn(&mut self, act: impl FnOnce(&mut Dungeon)) {
        // Every stage has a gated turn, so this ends within one cycle of 24. The stage is read
        // on each fractional turn, as the gate does (`CalcSpeedStage` runs per turn).
        let mut k = self.fractional_turn;
        while !acts(self.leader_stage(), k) {
            self.run_other_monsters(k);
            k = next(k);
        }
        act(self);
        self.run_other_monsters(k);
        self.fractional_turn = next(k);
        self.turn = self.turn.saturating_add(1);
    }

    /// The other monsters' steps of fractional turn `k` (team non-leaders and wild monsters).
    fn run_other_monsters(&mut self, _k: u8) {
        // shortcut: no other monster acts yet, so a floor only moves with the leader; port
        // their turns (#73) before any monster other than the leader is simulated.
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
        let info = EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: true,
            level: 8,
            iq: 0,
            hp: 30,
            max_hp: 30,
            attack: [10, 10],
            defence: [5, 5],
            offensive_stages: [0, 0],
            speed_up: [0; 5],
            speed_down: [0; 5],
            direction: 0,
            belly: 0,
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
    /// Each wait runs fractional turns up to and including the leader's, so the leader acted
    /// in the turn just before the counter now stands on.
    fn acting_turns(stage: u8, cycles: u32) -> Vec<u8> {
        let mut g = leader_run(stage);
        let mut acted = Vec::new();
        let mut elapsed = 0u32;
        while elapsed < u32::from(FRACTIONAL_TURNS) * cycles {
            let before = fractional_turn(&g);
            assert_eq!(g.step(Command::Wait), Outcome::Applied);
            let after = fractional_turn(&g);
            elapsed += u32::from((after + FRACTIONAL_TURNS - before) % FRACTIONAL_TURNS);
            acted.push((after + FRACTIONAL_TURNS - 1) % FRACTIONAL_TURNS);
        }
        acted
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
