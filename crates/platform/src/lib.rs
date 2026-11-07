//! SDL3 platform layer: window, input, audio output, timing.
//!
//! Only this crate and `apps/*` may depend on SDL3 (see scripts/check-deps.sh).
//!
//! [`run`] opens a window, pumps events, translates keyboard and gamepad input
//! into [`InputEvent`]s and DS buttons, advances the game in fixed ticks and draws
//! the two 256x192 DS screens into offscreen textures that are composited into the
//! window at an integer scale.

pub mod audio;
pub mod input;
pub mod layout;
pub mod settings;
pub mod timing;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

use rescue_render::screens::{ArrangedLayout, Arrangement};
use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};
use sdl3::event::Event;
use sdl3::gamepad::Gamepad;
use sdl3::pixels::Color;
use sdl3::render::{FRect, ScaleMode, WindowCanvas};
use sdl3::video::FullscreenType;

pub use input::{ButtonState, DsButton, InputEvent};
pub use layout::Screen;
pub use settings::WindowSettings;
pub use timing::{FixedTimestep, TICKS_PER_SECOND};

/// Version of the SDL3 library linked into the binary.
pub fn sdl_version() -> String {
    sdl3::version::version().to_string()
}

/// An error from SDL or the platform layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError(String);

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlatformError {}

/// Turns any displayable SDL error into a [`PlatformError`] with context.
fn ctx<E: fmt::Display>(what: &'static str) -> impl FnOnce(E) -> PlatformError {
    move |e| PlatformError(format!("{what}: {e}"))
}

/// Window and loop settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Window title.
    pub title: String,
    /// Initial integer scale of the window (window is `256*scale` wide and
    /// `(384+gap)*scale` high).
    pub scale: u32,
    /// Gap between the two screens, in DS pixels (scaled with the screens).
    pub gap: u32,
    /// Screen arrangement. `None` uses the saved one, else [`Arrangement::Vertical`].
    pub arrangement: Option<Arrangement>,
    /// Where to load and save the window settings (arrangement, size, position,
    /// fullscreen). `None` neither reads nor writes any file.
    pub settings_path: Option<PathBuf>,
    /// Stop after this many logic ticks (for headless runs). `None` runs until quit.
    pub tick_limit: Option<u64>,
    /// Tick at the DS frame rate (about 59.8261 Hz) instead of a flat 60 Hz.
    pub ds_rate: bool,
    /// Fast-forward multiplier (1 is normal speed).
    pub speed: u32,
    /// Start paused; use the debug keys below to step.
    pub start_paused: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            title: String::from("rescue"),
            scale: 2,
            gap: 0,
            arrangement: None,
            settings_path: None,
            tick_limit: None,
            ds_rate: false,
            speed: 1,
            start_paused: false,
        }
    }
}

/// Whether the loop keeps running after a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// An RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl From<Rgb> for Color {
    fn from(c: Rgb) -> Color {
        Color::RGB(c.0, c.1, c.2)
    }
}

/// Minimal drawing interface for one 256x192 screen, in screen pixels.
pub struct Painter<'a> {
    canvas: &'a mut WindowCanvas,
}

impl Painter<'_> {
    /// Fills the whole screen with `colour`.
    pub fn clear(&mut self, colour: Rgb) {
        self.canvas.set_draw_color(colour);
        self.canvas.clear();
    }

    /// Fills a rectangle with `colour`.
    pub fn fill_rect(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        colour: Rgb,
    ) -> Result<(), PlatformError> {
        self.canvas.set_draw_color(colour);
        self.canvas
            .fill_rect(FRect::new(x as f32, y as f32, w as f32, h as f32))
            .map_err(ctx("fill_rect"))
    }
}

/// The game side of the loop.
pub trait Game {
    /// Called for every translated input event, before the tick that sees it.
    fn on_input(&mut self, event: &InputEvent);
    /// Advances the game by one fixed tick, with the DS buttons currently held.
    fn tick(&mut self, buttons: ButtonState) -> Flow;
    /// Draws one screen.
    fn draw(&mut self, screen: Screen, painter: &mut Painter<'_>) -> Result<(), PlatformError>;
}

/// What happened during [`run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunStats {
    /// Logic ticks run.
    pub ticks: u64,
    /// Frames presented.
    pub frames: u64,
    /// Gamepads attached at start-up.
    pub gamepads_at_start: usize,
}

/// Initial window size in pixels: both screens and the gap at `config.scale`.
fn initial_window_size(config: &Config) -> (u32, u32) {
    let scale = config.scale.max(1);
    (
        SCREEN_WIDTH * scale,
        (2 * SCREEN_HEIGHT + config.gap) * scale,
    )
}

/// Whether a window of `size` at `pos` overlaps any display, given as `(x, y, w, h)` bounds.
fn position_is_visible(
    pos: (i32, i32),
    size: (u32, u32),
    displays: &[(i32, i32, u32, u32)],
) -> bool {
    let (x, y) = (i64::from(pos.0), i64::from(pos.1));
    displays.iter().any(|&(dx, dy, dw, dh)| {
        let (dx, dy) = (i64::from(dx), i64::from(dy));
        x < dx + i64::from(dw)
            && dx < x + i64::from(size.0)
            && y < dy + i64::from(dh)
            && dy < y + i64::from(size.1)
    })
}

/// Opens the window and runs the event/tick/draw loop until the window is closed,
/// Escape is pressed, the game returns [`Flow::Quit`], or `config.tick_limit` ticks
/// have run (frames, while paused). Debug keys: F5 pause, F6 step, Tab held fast-forward.
pub fn run(config: &Config, game: &mut dyn Game) -> Result<RunStats, PlatformError> {
    let sdl = sdl3::init().map_err(ctx("SDL init"))?;
    let video = sdl.video().map_err(ctx("video subsystem"))?;
    let gamepads = sdl.gamepad().map_err(ctx("gamepad subsystem"))?;
    let gamepads_at_start = gamepads.gamepads().map_err(ctx("list gamepads"))?.len();

    let saved = config
        .settings_path
        .as_deref()
        .map(WindowSettings::load)
        .unwrap_or_default();
    let (width, height) = saved.size.unwrap_or_else(|| initial_window_size(config));
    let mut arrangement = config.arrangement.or(saved.arrangement).unwrap_or_default();
    let mut fullscreen = saved.fullscreen;
    let mut windowed_size = (width, height);
    // A saved position is used only while it still lands on a connected display.
    let bounds: Vec<(i32, i32, u32, u32)> = video
        .displays()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d.get_bounds().ok())
        .map(|r| (r.x(), r.y(), r.width(), r.height()))
        .collect();
    let start_pos = saved
        .position
        .filter(|&p| position_is_visible(p, (width, height), &bounds));
    let mut windowed_pos = start_pos;
    let mut builder = video.window(&config.title, width, height);
    builder.resizable();
    match start_pos {
        Some((x, y)) => builder.position(x, y),
        None => builder.position_centered(),
    };
    if fullscreen {
        builder.fullscreen();
    }
    let window = builder.build().map_err(ctx("create window"))?;
    let mut canvas = window.into_canvas();
    let creator = canvas.texture_creator();
    let mut top = creator
        .create_texture_target(None, SCREEN_WIDTH, SCREEN_HEIGHT)
        .map_err(ctx("top screen texture"))?;
    let mut bottom = creator
        .create_texture_target(None, SCREEN_WIDTH, SCREEN_HEIGHT)
        .map_err(ctx("bottom screen texture"))?;
    top.set_scale_mode(ScaleMode::Nearest);
    bottom.set_scale_mode(ScaleMode::Nearest);

    let mut events = sdl.event_pump().map_err(ctx("event pump"))?;
    // Open pads stay open while they are in this map; SDL sends GamepadAdded for
    // pads already connected at start-up, so this also covers those.
    let mut open_pads: BTreeMap<u32, Gamepad> = BTreeMap::new();
    let mut buttons = ButtonState::default();
    let mut timestep = if config.ds_rate {
        FixedTimestep::ds(5)
    } else {
        FixedTimestep::from_rate(TICKS_PER_SECOND, 5)
    };
    let base_speed = config.speed.max(1);
    timestep.set_speed(base_speed);
    timestep.set_paused(config.start_paused);
    let mut stats = RunStats {
        ticks: 0,
        frames: 0,
        gamepads_at_start,
    };
    let mut last = Instant::now();

    'main: loop {
        for event in events.poll_iter() {
            let translated = match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } if input::is_quit_scancode(sc) => break 'main,
                // Reserved debug keys: handled here so the Game trait stays unchanged.
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } if debug_key(sc).is_some() => {
                    match debug_key(sc) {
                        Some(DebugKey::Pause) => {
                            timestep.set_paused(!timestep.is_paused());
                        }
                        Some(DebugKey::Step) => timestep.request_step(),
                        Some(DebugKey::FastForward) => {
                            timestep.set_speed(base_speed.max(HELD_FAST_FORWARD));
                        }
                        None => {}
                    }
                    None
                }
                Event::KeyUp {
                    scancode: Some(sc), ..
                } if debug_key(sc) == Some(DebugKey::FastForward) => {
                    timestep.set_speed(base_speed);
                    None
                }
                // Reserved window keys: F11 fullscreen, F10 next arrangement.
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } if WINDOW_SCANCODES.contains(&sc) => {
                    if sc == WINDOW_SCANCODES[0] {
                        fullscreen = !fullscreen;
                        if let Err(e) = canvas.window_mut().set_fullscreen(fullscreen) {
                            eprintln!("warning: could not change fullscreen: {e}");
                            fullscreen = !fullscreen;
                        }
                    } else {
                        arrangement = arrangement.next();
                    }
                    None
                }
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } => Some(key_event(sc, true)),
                Event::KeyUp {
                    scancode: Some(sc), ..
                } => Some(key_event(sc, false)),
                Event::GamepadButtonDown { which, button, .. } => {
                    Some(pad_event(which.raw(), button, true))
                }
                Event::GamepadButtonUp { which, button, .. } => {
                    Some(pad_event(which.raw(), button, false))
                }
                Event::GamepadAdded { which, .. } => match gamepads.open(which) {
                    Ok(pad) => {
                        let name = pad.name().unwrap_or_else(|| String::from("(unnamed)"));
                        open_pads.insert(which.raw(), pad);
                        Some(InputEvent::GamepadAdded {
                            pad: which.raw(),
                            name,
                        })
                    }
                    Err(e) => {
                        eprintln!("warning: could not open gamepad {which}: {e}");
                        None
                    }
                },
                Event::GamepadRemoved { which, .. } => {
                    open_pads.remove(&which.raw());
                    Some(InputEvent::GamepadRemoved { pad: which.raw() })
                }
                _ => None,
            };
            if let Some(ev) = translated {
                match &ev {
                    InputEvent::Key {
                        button: Some(b),
                        pressed,
                        ..
                    }
                    | InputEvent::GamepadButton {
                        button: Some(b),
                        pressed,
                        ..
                    } => buttons.set(*b, *pressed),
                    _ => {}
                }
                game.on_input(&ev);
            }
        }

        let now = Instant::now();
        let steps = timestep.advance(now - last);
        last = now;
        for _ in 0..steps {
            let flow = game.tick(buttons);
            stats.ticks += 1;
            if flow == Flow::Quit || config.tick_limit.is_some_and(|n| stats.ticks >= n) {
                break 'main;
            }
        }

        for (screen, texture) in [(Screen::Top, &mut top), (Screen::Bottom, &mut bottom)] {
            let mut result = Ok(());
            canvas
                .with_texture_canvas(texture, |c| {
                    result = game.draw(screen, &mut Painter { canvas: c });
                })
                .map_err(ctx("draw screen"))?;
            result?;
        }

        let (w, h) = canvas.output_size().map_err(ctx("output size"))?;
        if !fullscreen && canvas.window().fullscreen_state() == FullscreenType::Off {
            windowed_size = canvas.window().size();
            windowed_pos = Some(canvas.window().position());
        }
        let layout = ArrangedLayout::new(arrangement, w, h, config.gap);
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        for (screen, texture) in [(Screen::Top, &top), (Screen::Bottom, &bottom)] {
            let Some(r) = layout.rect(screen) else {
                continue;
            };
            let dst = FRect::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32);
            canvas
                .copy(texture, None, dst)
                .map_err(ctx("composite screen"))?;
        }
        canvas.present();
        stats.frames += 1;
        // Paused, no ticks run, so a headless limit counts presented frames instead.
        if timestep.is_paused() && config.tick_limit.is_some_and(|n| stats.frames >= n) {
            break 'main;
        }

        std::thread::sleep(timestep.time_until_next());
    }

    if let Some(path) = &config.settings_path {
        let settings = WindowSettings {
            arrangement: Some(arrangement),
            size: Some(windowed_size),
            position: windowed_pos,
            fullscreen,
        };
        if let Err(e) = settings.save(path) {
            eprintln!("warning: could not save {}: {e}", path.display());
        }
    }
    Ok(stats)
}

/// Debug timing keys, reserved by the platform loop (never sent to the game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugKey {
    /// F5: toggle pause.
    Pause,
    /// F6: run one tick while paused.
    Step,
    /// Tab (held): fast-forward.
    FastForward,
}

/// Fast-forward multiplier while Tab is held (at least the configured speed).
const HELD_FAST_FORWARD: u32 = 4;

/// Keys reserved for the debug timing controls, in [`DebugKey`] order (pause, step,
/// fast-forward). The loop handles them before DS input, so key rebinding (#63) must
/// reject these keys: a binding on one would be swallowed silently.
pub const DEBUG_SCANCODES: [sdl3::keyboard::Scancode; 3] = [
    sdl3::keyboard::Scancode::F5,
    sdl3::keyboard::Scancode::F6,
    sdl3::keyboard::Scancode::Tab,
];

/// Keys reserved for the window controls: F11 toggles borderless fullscreen, F10 cycles the
/// screen arrangement. Handled before DS input, so rebinding (#63) must reject them too.
pub const WINDOW_SCANCODES: [sdl3::keyboard::Scancode; 2] =
    [sdl3::keyboard::Scancode::F11, sdl3::keyboard::Scancode::F10];

/// The debug timing key for `scancode`, if it is one.
fn debug_key(scancode: sdl3::keyboard::Scancode) -> Option<DebugKey> {
    let [pause, step, fast_forward] = DEBUG_SCANCODES;
    match scancode {
        s if s == pause => Some(DebugKey::Pause),
        s if s == step => Some(DebugKey::Step),
        s if s == fast_forward => Some(DebugKey::FastForward),
        _ => None,
    }
}

fn key_event(scancode: sdl3::keyboard::Scancode, pressed: bool) -> InputEvent {
    InputEvent::Key {
        key: scancode.name().to_owned(),
        button: input::button_for_scancode(scancode),
        pressed,
    }
}

fn pad_event(pad: u32, button: sdl3::gamepad::Button, pressed: bool) -> InputEvent {
    InputEvent::GamepadButton {
        pad,
        name: button.string(),
        button: input::button_for_gamepad(button),
        pressed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_keys_do_not_collide_with_game_input() {
        for sc in DEBUG_SCANCODES {
            assert!(debug_key(sc).is_some());
            assert_eq!(input::button_for_scancode(sc), None, "{sc:?}");
            assert!(!input::is_quit_scancode(sc), "{sc:?}");
        }
    }

    #[test]
    fn window_keys_do_not_collide_with_game_or_debug_input() {
        for sc in WINDOW_SCANCODES {
            assert_eq!(input::button_for_scancode(sc), None, "{sc:?}");
            assert!(!input::is_quit_scancode(sc), "{sc:?}");
            assert!(debug_key(sc).is_none(), "{sc:?}");
        }
    }

    #[test]
    fn saved_position_must_overlap_a_display() {
        let displays = [(0, 0, 1920, 1080), (1920, 0, 1280, 1024)];
        let size = (512, 768);
        assert!(position_is_visible((100, 100), size, &displays));
        assert!(position_is_visible((2500, 300), size, &displays));
        // Partly off the edge still overlaps; fully past it does not.
        assert!(position_is_visible((-400, 10), size, &displays));
        assert!(!position_is_visible((-512, 10), size, &displays));
        assert!(!position_is_visible((50000, -40000), size, &displays));
        assert!(!position_is_visible((3200, 0), size, &displays));
        assert!(!position_is_visible((0, 0), size, &[]));
    }

    #[test]
    fn window_size_includes_the_gap_at_scale() {
        let mut c = Config::default();
        assert_eq!(initial_window_size(&c), (512, 768));
        c.gap = 8;
        assert_eq!(initial_window_size(&c), (512, 784));
        c.scale = 0;
        assert_eq!(initial_window_size(&c), (256, 392));
    }

    #[test]
    fn default_gap_is_zero() {
        assert_eq!(Config::default().gap, 0);
    }
}
