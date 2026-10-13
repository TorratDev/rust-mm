//! Input translation: SDL keyboard scancodes and gamepad buttons to DS buttons.
//!
//! This is the fixed default mapping for the platform hello-world. Configurable
//! bindings come later (input abstraction, #63).

use sdl3::gamepad::Button as PadButton;
use sdl3::keyboard::Scancode;

/// A button on the Nintendo DS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DsButton {
    A,
    B,
    X,
    Y,
    L,
    R,
    Start,
    Select,
    Up,
    Down,
    Left,
    Right,
}

impl DsButton {
    /// Every DS button, in a fixed order.
    pub const ALL: [DsButton; 12] = [
        DsButton::A,
        DsButton::B,
        DsButton::X,
        DsButton::Y,
        DsButton::L,
        DsButton::R,
        DsButton::Start,
        DsButton::Select,
        DsButton::Up,
        DsButton::Down,
        DsButton::Left,
        DsButton::Right,
    ];

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// Default keyboard mapping, by physical key position (scancode).
///
/// Arrows: D-pad. X: A. Z: B. S: X. A: Y. Q: L. W: R. Enter: Start.
/// Right Shift or Backspace: Select.
pub fn button_for_scancode(scancode: Scancode) -> Option<DsButton> {
    Some(match scancode {
        Scancode::Up => DsButton::Up,
        Scancode::Down => DsButton::Down,
        Scancode::Left => DsButton::Left,
        Scancode::Right => DsButton::Right,
        Scancode::X => DsButton::A,
        Scancode::Z => DsButton::B,
        Scancode::S => DsButton::X,
        Scancode::A => DsButton::Y,
        Scancode::Q => DsButton::L,
        Scancode::W => DsButton::R,
        Scancode::Return | Scancode::KpEnter => DsButton::Start,
        Scancode::RShift | Scancode::Backspace => DsButton::Select,
        _ => return None,
    })
}

/// Keys that close the application.
pub fn is_quit_scancode(scancode: Scancode) -> bool {
    scancode == Scancode::Escape
}

/// Default gamepad mapping, by face-button position (DS layout: A right, B bottom,
/// X top, Y left), so the physical layout matches the DS whatever the pad's labels.
pub fn button_for_gamepad(button: PadButton) -> Option<DsButton> {
    Some(match button {
        PadButton::East => DsButton::A,
        PadButton::South => DsButton::B,
        PadButton::North => DsButton::X,
        PadButton::West => DsButton::Y,
        PadButton::LeftShoulder => DsButton::L,
        PadButton::RightShoulder => DsButton::R,
        PadButton::Start => DsButton::Start,
        PadButton::Back => DsButton::Select,
        PadButton::DPadUp => DsButton::Up,
        PadButton::DPadDown => DsButton::Down,
        PadButton::DPadLeft => DsButton::Left,
        PadButton::DPadRight => DsButton::Right,
        _ => return None,
    })
}

/// Set of DS buttons currently held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ButtonState {
    bits: u16,
}

impl ButtonState {
    /// Marks `button` as held (`pressed == true`) or released.
    pub fn set(&mut self, button: DsButton, pressed: bool) {
        if pressed {
            self.bits |= button.bit();
        } else {
            self.bits &= !button.bit();
        }
    }

    /// Whether `button` is held.
    pub fn is_held(self, button: DsButton) -> bool {
        self.bits & button.bit() != 0
    }

    /// Held buttons, in [`DsButton::ALL`] order.
    pub fn held(self) -> impl Iterator<Item = DsButton> {
        DsButton::ALL.into_iter().filter(move |b| self.is_held(*b))
    }
}

/// An input event, already translated from SDL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// A keyboard key went down or up. Key repeats are not reported.
    Key {
        /// SDL's name for the physical key.
        key: String,
        /// The DS button it maps to, if any.
        button: Option<DsButton>,
        pressed: bool,
    },
    /// A gamepad button went down or up.
    GamepadButton {
        /// SDL joystick instance id of the pad.
        pad: u32,
        /// SDL's name for the button (positional, e.g. `a` for the bottom face button).
        name: String,
        /// The DS button it maps to, if any.
        button: Option<DsButton>,
        pressed: bool,
    },
    /// A gamepad was connected (including pads already present at start-up).
    GamepadAdded { pad: u32, name: String },
    /// A gamepad was disconnected.
    GamepadRemoved { pad: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_maps_every_ds_button() {
        let keys = [
            Scancode::Up,
            Scancode::Down,
            Scancode::Left,
            Scancode::Right,
            Scancode::X,
            Scancode::Z,
            Scancode::S,
            Scancode::A,
            Scancode::Q,
            Scancode::W,
            Scancode::Return,
            Scancode::RShift,
        ];
        let mut mapped: Vec<_> = keys
            .iter()
            .filter_map(|k| button_for_scancode(*k))
            .collect();
        mapped.sort();
        assert_eq!(mapped, DsButton::ALL.to_vec());
    }

    #[test]
    fn keyboard_specific_keys() {
        assert_eq!(button_for_scancode(Scancode::X), Some(DsButton::A));
        assert_eq!(button_for_scancode(Scancode::Z), Some(DsButton::B));
        assert_eq!(
            button_for_scancode(Scancode::KpEnter),
            Some(DsButton::Start)
        );
        assert_eq!(
            button_for_scancode(Scancode::Backspace),
            Some(DsButton::Select)
        );
        assert_eq!(button_for_scancode(Scancode::F1), None);
        assert_eq!(button_for_scancode(Scancode::Escape), None);
    }

    #[test]
    fn escape_quits() {
        assert!(is_quit_scancode(Scancode::Escape));
        assert!(!is_quit_scancode(Scancode::Return));
    }

    #[test]
    fn gamepad_maps_every_ds_button_positionally() {
        let pads = [
            PadButton::East,
            PadButton::South,
            PadButton::North,
            PadButton::West,
            PadButton::LeftShoulder,
            PadButton::RightShoulder,
            PadButton::Start,
            PadButton::Back,
            PadButton::DPadUp,
            PadButton::DPadDown,
            PadButton::DPadLeft,
            PadButton::DPadRight,
        ];
        let mut mapped: Vec<_> = pads.iter().filter_map(|b| button_for_gamepad(*b)).collect();
        mapped.sort();
        assert_eq!(mapped, DsButton::ALL.to_vec());
        assert_eq!(button_for_gamepad(PadButton::East), Some(DsButton::A));
        assert_eq!(button_for_gamepad(PadButton::South), Some(DsButton::B));
        assert_eq!(button_for_gamepad(PadButton::Guide), None);
        assert_eq!(button_for_gamepad(PadButton::Touchpad), None);
    }

    #[test]
    fn button_state_tracks_press_and_release() {
        let mut state = ButtonState::default();
        assert_eq!(state.held().count(), 0);
        state.set(DsButton::A, true);
        state.set(DsButton::Left, true);
        assert!(state.is_held(DsButton::A));
        assert!(!state.is_held(DsButton::B));
        assert_eq!(
            state.held().collect::<Vec<_>>(),
            [DsButton::A, DsButton::Left]
        );
        state.set(DsButton::A, false);
        state.set(DsButton::A, false);
        assert_eq!(state.held().collect::<Vec<_>>(), [DsButton::Left]);
    }

    #[test]
    fn all_buttons_have_distinct_bits() {
        let mut state = ButtonState::default();
        for b in DsButton::ALL {
            state.set(b, true);
        }
        assert_eq!(state.held().count(), DsButton::ALL.len());
    }
}
