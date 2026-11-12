//! Input translation: SDL keyboard scancodes and gamepad buttons to DS buttons.
//!
//! The default maps are [`DEFAULT_KEYS`] and [`DEFAULT_PADS`]. [`Bindings`] is the map in use:
//! the defaults, with the entries the settings file overrides.

use sdl3::gamepad::Button as PadButton;
use sdl3::keyboard::Scancode;

use crate::{DEBUG_SCANCODES, WINDOW_SCANCODES};

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

    /// Name used in the settings file (`key.<scancode>=<name>`, `pad.<button>=<name>`).
    pub fn name(self) -> &'static str {
        match self {
            DsButton::A => "A",
            DsButton::B => "B",
            DsButton::X => "X",
            DsButton::Y => "Y",
            DsButton::L => "L",
            DsButton::R => "R",
            DsButton::Start => "Start",
            DsButton::Select => "Select",
            DsButton::Up => "Up",
            DsButton::Down => "Down",
            DsButton::Left => "Left",
            DsButton::Right => "Right",
        }
    }

    /// Inverse of [`DsButton::name`], ignoring case.
    pub fn from_name(name: &str) -> Option<DsButton> {
        DsButton::ALL
            .into_iter()
            .find(|b| b.name().eq_ignore_ascii_case(name))
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// Default keyboard mapping, by physical key position (scancode).
///
/// Arrows: D-pad. X: A. Z: B. S: X. A: Y. Q: L. W: R. Enter: Start.
/// Right Shift or Backspace: Select.
const DEFAULT_KEYS: &[(Scancode, DsButton)] = &[
    (Scancode::Up, DsButton::Up),
    (Scancode::Down, DsButton::Down),
    (Scancode::Left, DsButton::Left),
    (Scancode::Right, DsButton::Right),
    (Scancode::X, DsButton::A),
    (Scancode::Z, DsButton::B),
    (Scancode::S, DsButton::X),
    (Scancode::A, DsButton::Y),
    (Scancode::Q, DsButton::L),
    (Scancode::W, DsButton::R),
    (Scancode::Return, DsButton::Start),
    (Scancode::KpEnter, DsButton::Start),
    (Scancode::RShift, DsButton::Select),
    (Scancode::Backspace, DsButton::Select),
];

/// Default gamepad mapping, by face-button position (DS layout: A right, B bottom,
/// X top, Y left), so the physical layout matches the DS whatever the pad's labels.
const DEFAULT_PADS: &[(PadButton, DsButton)] = &[
    (PadButton::East, DsButton::A),
    (PadButton::South, DsButton::B),
    (PadButton::North, DsButton::X),
    (PadButton::West, DsButton::Y),
    (PadButton::LeftShoulder, DsButton::L),
    (PadButton::RightShoulder, DsButton::R),
    (PadButton::Start, DsButton::Start),
    (PadButton::Back, DsButton::Select),
    (PadButton::DPadUp, DsButton::Up),
    (PadButton::DPadDown, DsButton::Down),
    (PadButton::DPadLeft, DsButton::Left),
    (PadButton::DPadRight, DsButton::Right),
];

/// Default keyboard mapping lookup; see [`Bindings`] for the map in use.
pub fn button_for_scancode(scancode: Scancode) -> Option<DsButton> {
    lookup(DEFAULT_KEYS, scancode)
}

/// Keys that close the application.
pub fn is_quit_scancode(scancode: Scancode) -> bool {
    scancode == Scancode::Escape
}

/// Default gamepad mapping lookup; see [`Bindings`] for the map in use.
pub fn button_for_gamepad(button: PadButton) -> Option<DsButton> {
    lookup(DEFAULT_PADS, button)
}

fn lookup<K: PartialEq + Copy>(table: &[(K, DsButton)], key: K) -> Option<DsButton> {
    table.iter().find(|(k, _)| *k == key).map(|&(_, b)| b)
}

/// Keys the platform loop handles itself (quit, debug, window). A binding on one would be
/// swallowed, so the settings file cannot bind them.
fn is_reserved(scancode: Scancode) -> bool {
    is_quit_scancode(scancode)
        || DEBUG_SCANCODES.contains(&scancode)
        || WINDOW_SCANCODES.contains(&scancode)
}

/// The keyboard and gamepad maps in use: the defaults, with the settings file's overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings {
    keys: Vec<(Scancode, DsButton)>,
    pads: Vec<(PadButton, DsButton)>,
}

impl Default for Bindings {
    fn default() -> Self {
        Bindings {
            keys: DEFAULT_KEYS.to_vec(),
            pads: DEFAULT_PADS.to_vec(),
        }
    }
}

impl Bindings {
    /// The DS button for a key, if any.
    pub fn button_for_scancode(&self, scancode: Scancode) -> Option<DsButton> {
        lookup(&self.keys, scancode)
    }

    /// The DS button for a gamepad button, if any.
    pub fn button_for_gamepad(&self, button: PadButton) -> Option<DsButton> {
        lookup(&self.pads, button)
    }

    /// Reads `key.<scancode name>=<DS button>` and `pad.<button name>=<DS button>` lines from
    /// settings text. Other lines are ignored. A broken line (unknown name, unknown button,
    /// reserved key) leaves its entry at the default, so it never stops the game.
    pub fn parse(text: &str) -> Bindings {
        let mut out = Bindings::default();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let Some(button) = DsButton::from_name(value.trim()) else {
                continue;
            };
            let name = name.trim();
            if let Some(key) = name.strip_prefix("key.") {
                if let Some(sc) = Scancode::from_name(key.trim()).filter(|sc| !is_reserved(*sc)) {
                    out.keys.retain(|(k, _)| *k != sc);
                    out.keys.push((sc, button));
                }
            } else if let Some(pad) = name.strip_prefix("pad.")
                && let Some(b) = PadButton::from_string(pad.trim())
            {
                out.pads.retain(|(p, _)| *p != b);
                out.pads.push((b, button));
            }
        }
        out
    }
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

    /// A DS button other than `b`, so an override is visible.
    fn other(b: DsButton) -> DsButton {
        if b == DsButton::Up {
            DsButton::Down
        } else {
            DsButton::Up
        }
    }

    #[test]
    fn button_names_round_trip() {
        for b in DsButton::ALL {
            assert_eq!(DsButton::from_name(b.name()), Some(b));
        }
        assert_eq!(DsButton::from_name("start"), Some(DsButton::Start));
        assert_eq!(DsButton::from_name("Turbo"), None);
    }

    #[test]
    fn no_settings_gives_the_default_maps() {
        let b = Bindings::default();
        for (sc, ds) in DEFAULT_KEYS {
            assert_eq!(b.button_for_scancode(*sc), Some(*ds));
            assert_eq!(button_for_scancode(*sc), Some(*ds));
        }
        for (pad, ds) in DEFAULT_PADS {
            assert_eq!(b.button_for_gamepad(*pad), Some(*ds));
        }
        assert_eq!(Bindings::parse(""), Bindings::default());
        assert_eq!(
            Bindings::parse("width=800\nfullscreen=true\n"),
            Bindings::default()
        );
    }

    #[test]
    fn settings_lines_rebind_keys_and_pads() {
        let b = Bindings::parse("key.X=B\npad.a=Y\n");
        assert_eq!(b.button_for_scancode(Scancode::X), Some(DsButton::B));
        assert_eq!(b.button_for_scancode(Scancode::Z), Some(DsButton::B));
        // "a" is SDL's bottom face button (South), which the default maps to B.
        assert_eq!(b.button_for_gamepad(PadButton::South), Some(DsButton::Y));
        assert_eq!(b.button_for_gamepad(PadButton::East), Some(DsButton::A));
        // The free functions stay on the defaults.
        assert_eq!(button_for_scancode(Scancode::X), Some(DsButton::A));
    }

    #[test]
    fn a_later_line_replaces_an_earlier_one() {
        let b = Bindings::parse("key.X=B\nkey.X=R");
        assert_eq!(b.button_for_scancode(Scancode::X), Some(DsButton::R));
        assert_eq!(b.keys.iter().filter(|(k, _)| *k == Scancode::X).count(), 1);
    }

    #[test]
    fn broken_lines_keep_the_default() {
        let text = "key.X=Bogus\nkey.NoSuchKey=A\npad.nosuch=A\npad.a=\nkey.A\n\
                    key.Escape=A\nkey.F5=A\nkey.F10=A\nkey.Tab=A\n= A\nkey. =A\n";
        assert_eq!(Bindings::parse(text), Bindings::default());
    }

    #[test]
    fn every_default_key_and_pad_name_is_read_from_the_file() {
        for (sc, ds) in DEFAULT_KEYS {
            let line = format!("key.{}={}", sc.name(), other(*ds).name());
            let b = Bindings::parse(&line);
            assert_eq!(b.button_for_scancode(*sc), Some(other(*ds)), "{line}");
        }
        for (pad, ds) in DEFAULT_PADS {
            let line = format!("pad.{}={}", pad.string(), other(*ds).name());
            let b = Bindings::parse(&line);
            assert_eq!(b.button_for_gamepad(*pad), Some(other(*ds)), "{line}");
        }
    }
}
