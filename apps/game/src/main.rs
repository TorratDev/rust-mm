//! Main game executable.
//!
//! For now this is the platform hello-world (#26): it opens a window with the two
//! DS screens as placeholders, logs keyboard and gamepad input to stdout, and shows
//! the held DS buttons on the bottom screen.
//!
//! Usage: `rescue [--frames N] [--scale N]`. `--frames` (or the `RESCUE_FRAMES`
//! environment variable) stops after N logic ticks, for headless runs in CI.

use std::process::ExitCode;

use rescue_platform::{
    ButtonState, Config, DsButton, Flow, Game, InputEvent, Painter, PlatformError, Rgb,
    SCREEN_HEIGHT, SCREEN_WIDTH, Screen,
};

const USAGE: &str = "usage: rescue [--frames N] [--scale N]
  --frames N   stop after N logic ticks (60 per second); also RESCUE_FRAMES=N
  --scale N    initial window scale (default 2)
Keys: arrows = D-pad, X = A, Z = B, S = X, A = Y, Q = L, W = R,
      Enter = Start, Right Shift/Backspace = Select, Esc = quit.";

/// Command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    frames: Option<u64>,
    scale: u32,
    help: bool,
}

/// Parses `args` (without the program name). `env_frames` is the value of
/// `RESCUE_FRAMES`, if set; `--frames` takes precedence over it.
fn parse_args<I>(args: I, env_frames: Option<&str>) -> Result<Options, String>
where
    I: IntoIterator<Item = String>,
{
    let mut opts = Options {
        frames: None,
        scale: 2,
        help: false,
    };
    if let Some(v) = env_frames {
        opts.frames = Some(parse_positive("RESCUE_FRAMES", v)?);
    }
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => opts.help = true,
            "--frames" | "--scale" => {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                let n = parse_positive(&arg, &value)?;
                if arg == "--frames" {
                    opts.frames = Some(n);
                } else {
                    opts.scale = u32::try_from(n)
                        .ok()
                        .filter(|s| *s <= 16)
                        .ok_or_else(|| format!("--scale must be 1..=16, got {n}"))?;
                }
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(opts)
}

fn parse_positive(name: &str, value: &str) -> Result<u64, String> {
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{name} must be a positive integer, got {value:?}")),
    }
}

/// Formats an input event as one log line.
fn describe(event: &InputEvent) -> String {
    let state = |pressed: bool| if pressed { "down" } else { "up" };
    let ds = |b: &Option<DsButton>| match b {
        Some(b) => format!(" -> DS {b:?}"),
        None => String::new(),
    };
    match event {
        InputEvent::Key {
            key,
            button,
            pressed,
        } => format!("key {key} {}{}", state(*pressed), ds(button)),
        InputEvent::GamepadButton {
            pad,
            name,
            button,
            pressed,
        } => format!("pad {pad} button {name} {}{}", state(*pressed), ds(button)),
        InputEvent::GamepadAdded { pad, name } => format!("pad {pad} connected: {name}"),
        InputEvent::GamepadRemoved { pad } => format!("pad {pad} disconnected"),
    }
}

/// Placeholder game: a moving bar on the top screen, held buttons on the bottom.
#[derive(Default)]
struct HelloWorld {
    tick: u64,
    buttons: ButtonState,
}

impl Game for HelloWorld {
    fn on_input(&mut self, event: &InputEvent) {
        println!("input: {}", describe(event));
    }

    fn tick(&mut self, buttons: ButtonState) -> Flow {
        self.tick += 1;
        self.buttons = buttons;
        Flow::Continue
    }

    fn draw(&mut self, screen: Screen, p: &mut Painter<'_>) -> Result<(), PlatformError> {
        match screen {
            Screen::Top => {
                p.clear(Rgb(24, 32, 72));
                let x = (self.tick % u64::from(SCREEN_WIDTH)) as i32;
                p.fill_rect(x, 0, 8, SCREEN_HEIGHT, Rgb(240, 200, 64))?;
            }
            Screen::Bottom => {
                p.clear(Rgb(16, 56, 32));
                // One 16x16 cell per DS button, lit while held.
                for (i, b) in DsButton::ALL.into_iter().enumerate() {
                    let colour = if self.buttons.is_held(b) {
                        Rgb(250, 250, 250)
                    } else {
                        Rgb(40, 90, 56)
                    };
                    p.fill_rect(8 + 20 * i as i32, 88, 16, 16, colour)?;
                }
            }
        }
        Ok(())
    }
}

fn main() -> ExitCode {
    let env_frames = std::env::var("RESCUE_FRAMES").ok();
    let opts = match parse_args(std::env::args().skip(1), env_frames.as_deref()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("rescue: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if opts.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    println!("rescue: SDL {}", rescue_platform::sdl_version());
    let config = Config {
        title: String::from("rescue"),
        scale: opts.scale,
        tick_limit: opts.frames,
    };
    match rescue_platform::run(&config, &mut HelloWorld::default()) {
        Ok(stats) => {
            println!(
                "rescue: ran {} ticks, presented {} frames, gamepads at start: {}",
                stats.ticks, stats.frames, stats.gamepads_at_start
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rescue: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults() {
        let o = parse_args(args(&[]), None).unwrap();
        assert_eq!(
            o,
            Options {
                frames: None,
                scale: 2,
                help: false
            }
        );
    }

    #[test]
    fn frames_and_scale() {
        let o = parse_args(args(&["--frames", "120", "--scale", "3"]), None).unwrap();
        assert_eq!(o.frames, Some(120));
        assert_eq!(o.scale, 3);
    }

    #[test]
    fn env_frames_and_override() {
        assert_eq!(parse_args(args(&[]), Some("30")).unwrap().frames, Some(30));
        let o = parse_args(args(&["--frames", "5"]), Some("30")).unwrap();
        assert_eq!(o.frames, Some(5));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_args(args(&["--frames"]), None).is_err());
        assert!(parse_args(args(&["--frames", "0"]), None).is_err());
        assert!(parse_args(args(&["--frames", "-1"]), None).is_err());
        assert!(parse_args(args(&["--scale", "17"]), None).is_err());
        assert!(parse_args(args(&["--bogus"]), None).is_err());
        assert!(parse_args(args(&[]), Some("abc")).is_err());
    }

    #[test]
    fn help() {
        assert!(parse_args(args(&["--help"]), None).unwrap().help);
    }

    #[test]
    fn describes_events() {
        let key = InputEvent::Key {
            key: "X".into(),
            button: Some(DsButton::A),
            pressed: true,
        };
        assert_eq!(describe(&key), "key X down -> DS A");
        let pad = InputEvent::GamepadButton {
            pad: 3,
            name: "guide".into(),
            button: None,
            pressed: false,
        };
        assert_eq!(describe(&pad), "pad 3 button guide up");
        let added = InputEvent::GamepadAdded {
            pad: 3,
            name: "Xbox Controller".into(),
        };
        assert_eq!(describe(&added), "pad 3 connected: Xbox Controller");
        assert_eq!(
            describe(&InputEvent::GamepadRemoved { pad: 3 }),
            "pad 3 disconnected"
        );
    }
}
