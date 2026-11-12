//! Main game executable.
//!
//! Main game executable.
//!
//! Boot flow (#69): the ROM check (`boot`), then splash, title and the main menu (`scene`),
//! driven by `app`. Drawing is placeholder blocks for now (`draw`); the original art and
//! text are not loaded yet. Logs keyboard and gamepad input to stdout.
//!
//! Usage: `rescue [--frames N] [--scale N] [--gap N] [--layout NAME] [--ff N] [--pause]`. `--frames` (or the `RESCUE_FRAMES`
//! environment variable) stops after N logic ticks, for headless runs in CI.

mod app;
mod boot;
mod draw;
mod scene;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rescue_nds::rom::{CONFIG_FILE_NAME, ROM_PATH_ENV, SUPPORTED_DUMPS_DOC};
use rescue_platform::{
    ButtonState, Config, DsButton, Flow, Game, InputEvent, Painter, PlatformError, Screen,
};
use rescue_render::screens::Arrangement;

use crate::app::App;
use crate::boot::RomCheck;

const USAGE: &str =
    "usage: rescue [--frames N] [--scale N] [--gap N] [--layout NAME] [--ff N] [--pause]
  --frames N   stop after N logic ticks (DS rate, ~59.83 per second); also RESCUE_FRAMES=N
  --scale N    initial window scale (default 2)
  --gap N      gap between the two screens in DS pixels, 0..=64 (default 0)
  --layout L   vertical, side, top, overlay or large (default: saved, else vertical)
  --settings P settings file (default: the user config dir); --no-settings uses none;
               --frames implies --no-settings unless --settings is given
  --ff N       fast-forward multiplier 1..=16 (default 1)
  --pause      start paused (with --frames, N counts presented frames)
Window keys: F11 = fullscreen, F10 = next layout.
Debug keys: F5 = pause, F6 = step one tick, Tab (held) = fast-forward.
Keys: arrows = D-pad, X = A, Z = B, S = X, A = Y, Q = L, W = R,
      Enter = Start, Right Shift/Backspace = Select, Esc = quit.
ROM: set RESCUE_ROM to a supported dump, or rom_path in rescue.toml (see docs/supported-dumps.md).";

/// Where the window settings live.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Settings {
    Default,
    Path(PathBuf),
    Off,
}

/// Command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    frames: Option<u64>,
    scale: u32,
    gap: u32,
    layout: Option<Arrangement>,
    settings: Settings,
    speed: u32,
    pause: bool,
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
        gap: 0,
        layout: None,
        settings: Settings::Default,
        speed: 1,
        pause: false,
        help: false,
    };
    if let Some(v) = env_frames {
        opts.frames = Some(parse_positive("RESCUE_FRAMES", v)?);
    }
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => opts.help = true,
            "--pause" => opts.pause = true,
            "--no-settings" => opts.settings = Settings::Off,
            "--settings" => {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                opts.settings = Settings::Path(PathBuf::from(value));
            }
            "--layout" => {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                opts.layout = Some(Arrangement::from_name(&value).ok_or_else(|| {
                    format!("--layout must be vertical, side, top, overlay or large, got {value:?}")
                })?);
            }
            "--gap" => {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                opts.gap = value
                    .parse::<u32>()
                    .ok()
                    .filter(|g| *g <= 64)
                    .ok_or_else(|| format!("--gap must be 0..=64, got {value:?}"))?;
            }
            "--frames" | "--scale" | "--ff" => {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                let n = parse_positive(&arg, &value)?;
                if arg == "--frames" {
                    opts.frames = Some(n);
                } else if arg == "--ff" {
                    opts.speed = u32::try_from(n)
                        .ok()
                        .filter(|s| *s <= 16)
                        .ok_or_else(|| format!("--ff must be 1..=16, got {n}"))?;
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

impl Game for App {
    fn on_input(&mut self, event: &InputEvent) {
        println!("input: {}", describe(event));
    }

    fn tick(&mut self, buttons: ButtonState) -> Flow {
        App::tick(self, buttons)
    }

    fn draw(&mut self, screen: Screen, p: &mut Painter<'_>) -> Result<(), PlatformError> {
        draw::screen(self.scene, screen, p)
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
    let rom = boot::check(
        std::env::var_os(ROM_PATH_ENV),
        Some(Path::new(CONFIG_FILE_NAME)),
    );
    match &rom {
        RomCheck::Ready(id) => println!("rescue: ROM {}", id.dump().title),
        RomCheck::NotConfigured => eprintln!(
            "rescue: no ROM configured: set {ROM_PATH_ENV} or rom_path in {CONFIG_FILE_NAME} (see {SUPPORTED_DUMPS_DOC})"
        ),
        RomCheck::Refused(why) => eprintln!("rescue: {why}"),
    }
    let config = Config {
        title: String::from("rescue"),
        scale: opts.scale,
        gap: opts.gap,
        arrangement: opts.layout,
        settings_path: match opts.settings {
            // A headless --frames run must not touch the user's settings.
            Settings::Default if opts.frames.is_some() => None,
            Settings::Default => rescue_platform::settings::default_path(),
            Settings::Path(p) => Some(p),
            Settings::Off => None,
        },
        tick_limit: opts.frames,
        ds_rate: true,
        speed: opts.speed,
        start_paused: opts.pause,
    };
    match rescue_platform::run(&config, &mut App::new(&rom)) {
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
                gap: 0,
                layout: None,
                settings: Settings::Default,
                speed: 1,
                pause: false,
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
    fn gap() {
        assert_eq!(parse_args(args(&["--gap", "8"]), None).unwrap().gap, 8);
        assert_eq!(parse_args(args(&["--gap", "0"]), None).unwrap().gap, 0);
        assert!(parse_args(args(&["--gap", "65"]), None).is_err());
        assert!(parse_args(args(&["--gap", "-1"]), None).is_err());
        assert!(parse_args(args(&["--gap"]), None).is_err());
    }

    #[test]
    fn layout_and_settings() {
        for a in Arrangement::ALL {
            let o = parse_args(args(&["--layout", a.name()]), None).unwrap();
            assert_eq!(o.layout, Some(a));
        }
        assert!(parse_args(args(&["--layout", "diagonal"]), None).is_err());
        assert!(parse_args(args(&["--layout"]), None).is_err());
        let o = parse_args(args(&["--settings", "x.cfg"]), None).unwrap();
        assert_eq!(o.settings, Settings::Path(PathBuf::from("x.cfg")));
        let o = parse_args(args(&["--no-settings"]), None).unwrap();
        assert_eq!(o.settings, Settings::Off);
        assert!(parse_args(args(&["--settings"]), None).is_err());
    }

    #[test]
    fn ff_and_pause() {
        let o = parse_args(args(&["--ff", "4", "--pause"]), None).unwrap();
        assert_eq!((o.speed, o.pause), (4, true));
        assert!(parse_args(args(&["--ff", "17"]), None).is_err());
        assert!(parse_args(args(&["--ff", "0"]), None).is_err());
        assert!(parse_args(args(&["--ff"]), None).is_err());
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
