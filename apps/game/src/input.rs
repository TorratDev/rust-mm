//! Buttons to a dungeon command (#347 P1). A pure function of the buttons held on one frame:
//! the sim never sees raw input. The rules are measured in Blue (#347 P1, investigator a's
//! decode of the dungeon's input reader).

use rescue_platform::{ButtonState, DsButton};
use rescue_sim::command::{Command, Direction};

/// The command the held buttons ask for, or `None`.
///
/// Two D-pad bits make a diagonal; one bit a straight direction; opposite or three bits none.
/// R held accepts only the diagonals. A+B is a turn in place (`Wait`), whatever else is held.
/// B held with a direction runs. There is no repeat counter and no debounce: the caller asks
/// again on each frame the leader is idle.
pub fn command_for(held: ButtonState) -> Option<Command> {
    if held.is_held(DsButton::A) && held.is_held(DsButton::B) {
        return Some(Command::Wait);
    }
    let dir = direction(held)?;
    let (dx, dy) = dir.delta();
    if held.is_held(DsButton::R) && (dx == 0 || dy == 0) {
        return None;
    }
    Some(if held.is_held(DsButton::B) {
        Command::Run(dir)
    } else {
        Command::Move(dir)
    })
}

/// The direction of the D-pad bits held, if they name one.
fn direction(held: ButtonState) -> Option<Direction> {
    let bits = (
        held.is_held(DsButton::Up),
        held.is_held(DsButton::Down),
        held.is_held(DsButton::Left),
        held.is_held(DsButton::Right),
    );
    Some(match bits {
        (true, false, false, false) => Direction::Up,
        (false, true, false, false) => Direction::Down,
        (false, false, true, false) => Direction::Left,
        (false, false, false, true) => Direction::Right,
        (true, false, false, true) => Direction::UpRight,
        (true, false, true, false) => Direction::UpLeft,
        (false, true, false, true) => Direction::DownRight,
        (false, true, true, false) => Direction::DownLeft,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(buttons: &[DsButton]) -> ButtonState {
        let mut s = ButtonState::default();
        for b in buttons {
            s.set(*b, true);
        }
        s
    }

    #[test]
    fn a_list_of_held_buttons_gives_the_list_of_commands() {
        use DsButton::*;
        let frames = [
            vec![],
            vec![Down],
            vec![Up, Right],
            vec![Down, Left],
            vec![R, Right],
            vec![R, Up, Right],
            vec![A],
            vec![A, B],
            vec![A, B, Left],
            vec![B, Left],
            vec![B],
            vec![Up, Down],
            vec![Left, Right, Up],
            vec![R, B, Down, Right],
            vec![X, Y, Start],
        ];
        let got: Vec<Option<Command>> = frames.iter().map(|f| command_for(held(f))).collect();
        assert_eq!(
            got,
            vec![
                None,
                Some(Command::Move(Direction::Down)),
                Some(Command::Move(Direction::UpRight)),
                Some(Command::Move(Direction::DownLeft)),
                None,
                Some(Command::Move(Direction::UpRight)),
                None,
                Some(Command::Wait),
                Some(Command::Wait),
                Some(Command::Run(Direction::Left)),
                None,
                None,
                None,
                Some(Command::Run(Direction::DownRight)),
                None,
            ]
        );
    }
}
