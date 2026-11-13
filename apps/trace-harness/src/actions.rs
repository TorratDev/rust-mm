//! Maps the leader's recorded action records (schema v2, `actions` in each row) to sim
//! [`Command`]s (#333, option A, static findings: `ov_0001` decompiles, not yet run in the
//! emulator). The ids and fields are the ones the game writes; the names are the sim's.
//!
//! Only the ids below are mapped. Any other id, and each id that has no command yet, is an
//! [`ActionError`] with its reason, never a silent skip.

use std::fmt;

use rescue_sim::command::{Command, Direction};
use rescue_trace::Action;

/// Id 1: pass a turn in place.
const ID_WAIT: u16 = 0x01;
/// Id 2: walk one tile. `param` 1 is a step, 0 a run record (one tile each, see the findings).
const ID_WALK: u16 = 0x02;
/// Id 0xB: use the inventory item at slot `param - 1`.
const ID_USE_ITEM: u16 = 0x0B;
/// Id 0x14: use move slot `slot` in `direction`.
const ID_USE_MOVE: u16 = 0x14;
/// Id 0x32: attack in `direction`.
const ID_ATTACK: u16 = 0x32;

/// Ids the game writes that have no command here, with the reason. Each one is from the
/// findings in #333 (`ov_0001` decompiles).
const NO_COMMAND: &[(u16, &str)] = &[
    (
        0x13,
        "the leader faces a talk or swap target; the sim has no talk or ally swap",
    ),
    (
        0x17,
        "no move slot has a nonzero PP-like byte; the meaning is not established",
    ),
    (
        0x2B,
        "the result of a yes/no dialog; dialogs are not modelled",
    ),
    (
        0x2D,
        "source not established: the id comes from a table filled at run time",
    ),
    (
        0x2E,
        "the result of a yes/no dialog; dialogs are not modelled",
    ),
];

/// Inventory slots the team carries (20 slots, 3 bytes each, in the findings).
pub const INVENTORY_SLOTS: u8 = 20;
/// Move slots per Pokémon.
pub const MOVE_SLOTS: u8 = 4;

/// The game's eight directions, indexed by the raw value (`Direction`'s order is the same).
const DIRECTIONS: [Direction; 8] = [
    Direction::Down,
    Direction::DownRight,
    Direction::Right,
    Direction::UpRight,
    Direction::Up,
    Direction::UpLeft,
    Direction::Left,
    Direction::DownLeft,
];

/// Why an action record has no command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    /// An id this mapping does not name.
    UnknownId(u16),
    /// An id with no command, and why.
    NoCommand { id: u16, reason: &'static str },
    /// A direction above 7.
    BadDirection(u8),
    /// Id 2 with a `param` other than 0 (run) or 1 (step).
    BadWalkParam(u8),
    /// Id 0xB with `param` 0 (no slot) or above [`INVENTORY_SLOTS`].
    ItemSlot(u8),
    /// Id 0x14 with a slot above [`MOVE_SLOTS`] - 1.
    MoveSlot(u8),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "action id {id:#x} is not mapped to a command"),
            Self::NoCommand { id, reason } => {
                write!(f, "action id {id:#x} has no command: {reason}")
            }
            Self::BadDirection(d) => write!(f, "action direction {d} is above 7"),
            Self::BadWalkParam(p) => write!(f, "walk action param {p} is not 0 (run) or 1 (step)"),
            Self::ItemSlot(p) => write!(
                f,
                "use-item action param {p} is not a slot of 1 to {INVENTORY_SLOTS}"
            ),
            Self::MoveSlot(s) => write!(f, "use-move action slot {s} is above {}", MOVE_SLOTS - 1),
        }
    }
}

impl std::error::Error for ActionError {}

/// The command for one action record.
pub fn command_for(action: &Action) -> Result<Command, ActionError> {
    match action.id {
        ID_WAIT => Ok(Command::Wait),
        // A run record is one tile, so it maps to the same command as a step.
        ID_WALK => match action.param {
            0 | 1 => Ok(Command::Move(direction(action.direction)?)),
            other => Err(ActionError::BadWalkParam(other)),
        },
        ID_USE_ITEM => match action.param {
            1..=INVENTORY_SLOTS => Ok(Command::UseItem {
                slot: action.param - 1,
            }),
            other => Err(ActionError::ItemSlot(other)),
        },
        ID_USE_MOVE => {
            if action.slot >= MOVE_SLOTS {
                return Err(ActionError::MoveSlot(action.slot));
            }
            Ok(Command::UseMove {
                slot: action.slot,
                direction: Some(direction(action.direction)?),
            })
        }
        ID_ATTACK => Ok(Command::Attack(direction(action.direction)?)),
        id => match NO_COMMAND.iter().find(|(known, _)| *known == id) {
            Some((_, reason)) => Err(ActionError::NoCommand { id, reason }),
            None => Err(ActionError::UnknownId(id)),
        },
    }
}

/// The sim's name for a command, for reports.
#[must_use]
pub const fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Move(_) => "Move",
        Command::Run(_) => "Run",
        Command::Attack(_) => "Attack",
        Command::UseMove { .. } => "UseMove",
        Command::UseItem { .. } => "UseItem",
        Command::Wait => "Wait",
        Command::Menu(_) => "Menu",
    }
}

fn direction(raw: u8) -> Result<Direction, ActionError> {
    DIRECTIONS
        .get(usize::from(raw))
        .copied()
        .ok_or(ActionError::BadDirection(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: u16, direction: u8, param: u8, slot: u8) -> Action {
        Action {
            id,
            direction,
            param,
            slot,
        }
    }

    #[test]
    fn each_mapped_id_gives_its_command() {
        assert_eq!(command_for(&action(1, 0, 0, 0)), Ok(Command::Wait));
        assert_eq!(
            command_for(&action(2, 2, 1, 0)),
            Ok(Command::Move(Direction::Right))
        );
        assert_eq!(
            command_for(&action(0x0B, 0, 3, 0)),
            Ok(Command::UseItem { slot: 2 })
        );
        assert_eq!(
            command_for(&action(0x14, 4, 0, 1)),
            Ok(Command::UseMove {
                slot: 1,
                direction: Some(Direction::Up),
            })
        );
        assert_eq!(
            command_for(&action(0x32, 6, 0, 0)),
            Ok(Command::Attack(Direction::Left))
        );
    }

    #[test]
    fn a_run_is_one_move_per_record_in_order() {
        // Run records: param 0, the direction kept from the first record.
        let run = [action(2, 1, 1, 0), action(2, 1, 0, 0), action(2, 1, 0, 0)];
        let commands: Vec<Command> = run
            .iter()
            .map(|a| command_for(a).expect("walk maps"))
            .collect();
        assert_eq!(commands, vec![Command::Move(Direction::DownRight); 3]);
    }

    #[test]
    fn ids_without_a_command_name_their_reason() {
        for id in [0x13, 0x17, 0x2B, 0x2D, 0x2E] {
            match command_for(&action(id, 0, 0, 0)) {
                Err(ActionError::NoCommand { id: got, reason }) => {
                    assert_eq!(got, id);
                    assert!(!reason.is_empty());
                }
                other => panic!("id {id:#x}: expected NoCommand, got {other:?}"),
            }
        }
    }

    #[test]
    fn unknown_id_and_bad_fields_are_errors() {
        assert_eq!(
            command_for(&action(0x99, 0, 0, 0)),
            Err(ActionError::UnknownId(0x99))
        );
        assert_eq!(
            command_for(&action(2, 8, 1, 0)),
            Err(ActionError::BadDirection(8))
        );
        assert_eq!(
            command_for(&action(0x32, 9, 0, 0)),
            Err(ActionError::BadDirection(9))
        );
        assert_eq!(
            command_for(&action(2, 0, 2, 0)),
            Err(ActionError::BadWalkParam(2))
        );
        assert_eq!(
            command_for(&action(0x0B, 0, 0, 0)),
            Err(ActionError::ItemSlot(0))
        );
        assert_eq!(
            command_for(&action(0x0B, 0, 21, 0)),
            Err(ActionError::ItemSlot(21))
        );
        assert_eq!(
            command_for(&action(0x14, 0, 0, 4)),
            Err(ActionError::MoveSlot(4))
        );
    }

    #[test]
    fn command_names_match_the_variants() {
        assert_eq!(command_name(&Command::Wait), "Wait");
        assert_eq!(command_name(&Command::Attack(Direction::Up)), "Attack");
    }
}
