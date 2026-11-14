//! Buttons to a dungeon command (#347 P1). A pure function of the buttons held on one frame:
//! the sim never sees raw input. The rules are measured in Blue (#347 P1, investigator a's
//! decode of the dungeon's input reader).

use rescue_platform::{ButtonState, DsButton};
use rescue_sim::command::{Command, Direction};

/// The command the held buttons ask for, or `None`.
///
/// Two D-pad bits make a diagonal; one bit a straight direction; opposite or three bits none.
/// R held accepts only the diagonals. A+B passes the turn (`Wait`), whatever else is held.
/// B held with a direction runs. There is no repeat counter and no debounce: the caller asks
/// again on each frame the leader is idle.
///
/// A alone is the regular attack at `facing`; A with a direction attacks that way.
// shortcut: the attack input's button map is not measured (`docs/re/combat.md`, P8, gives only
// that the input loop `02166B10` writes action `0x32`). A, alone or with the D-pad, stands in.
pub fn command_for(held: ButtonState, facing: Direction) -> Option<Command> {
    if held.is_held(DsButton::A) && held.is_held(DsButton::B) {
        return Some(Command::Wait);
    }
    if held.is_held(DsButton::A) {
        return Some(Command::Attack(direction(held).unwrap_or(facing)));
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

/// Move slots in the move menu.
pub const MENU_SLOTS: usize = 4;

/// What one frame's presses do in the move menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInput {
    Stay,
    /// The cursor moves to this slot.
    Cursor(usize),
    /// Use this move slot.
    Use(usize),
    Close,
}

/// X opens the move menu.
// shortcut: X is a stand-in for the game's menu button, which is not measured.
pub fn opens_menu(pressed: ButtonState) -> bool {
    pressed.is_held(DsButton::X)
}

/// The menu's reply to the buttons that went down this frame, with the cursor on `cursor`.
/// Up and Down move it (wrapping), A uses the slot, B closes.
pub fn menu_input(pressed: ButtonState, cursor: usize) -> MenuInput {
    if pressed.is_held(DsButton::B) {
        MenuInput::Close
    } else if pressed.is_held(DsButton::A) {
        MenuInput::Use(cursor)
    } else if pressed.is_held(DsButton::Down) {
        MenuInput::Cursor((cursor + 1) % MENU_SLOTS)
    } else if pressed.is_held(DsButton::Up) {
        MenuInput::Cursor((cursor + MENU_SLOTS - 1) % MENU_SLOTS)
    } else {
        MenuInput::Stay
    }
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
            vec![A, Left],
            vec![A, B],
            vec![A, B, Left],
            vec![B, Left],
            vec![B],
            vec![Up, Down],
            vec![Left, Right, Up],
            vec![R, B, Down, Right],
            vec![X, Y, Start],
        ];
        let got: Vec<Option<Command>> = frames
            .iter()
            .map(|f| command_for(held(f), Direction::Down))
            .collect();
        assert_eq!(
            got,
            vec![
                None,
                Some(Command::Move(Direction::Down)),
                Some(Command::Move(Direction::UpRight)),
                Some(Command::Move(Direction::DownLeft)),
                None,
                Some(Command::Move(Direction::UpRight)),
                Some(Command::Attack(Direction::Down)),
                Some(Command::Attack(Direction::Left)),
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

    #[test]
    fn a_attacks_the_facing_tile_and_the_menu_maps_presses() {
        assert_eq!(
            command_for(held(&[DsButton::A]), Direction::Up),
            Some(Command::Attack(Direction::Up))
        );
        assert!(opens_menu(held(&[DsButton::X])));
        assert!(!opens_menu(held(&[DsButton::A])));
        use MenuInput::*;
        let got: Vec<MenuInput> = [
            (vec![], 0),
            (vec![DsButton::Down], 3),
            (vec![DsButton::Up], 0),
            (vec![DsButton::Down], 1),
            (vec![DsButton::A], 2),
            (vec![DsButton::B], 2),
        ]
        .iter()
        .map(|(b, c)| menu_input(held(b), *c))
        .collect();
        assert_eq!(
            got,
            vec![Stay, Cursor(0), Cursor(3), Cursor(2), Use(2), Close]
        );
    }
}
