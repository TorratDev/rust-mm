//! Remembered window settings: arrangement, size, position and fullscreen.
//!
//! Stored as a few `key=value` lines. Parsing never fails: unknown or malformed lines are
//! ignored, so a hand-edited or older file cannot stop the game from starting. Full
//! settings come with #86.

use std::path::{Path, PathBuf};

use rescue_render::screens::Arrangement;

/// Smallest and largest window dimension accepted from a file, in pixels.
const MIN_SIZE: u32 = 64;
const MAX_SIZE: u32 = 16_384;

/// Window state that survives between runs. `None` fields were never saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowSettings {
    pub arrangement: Option<Arrangement>,
    /// Windowed size in pixels.
    pub size: Option<(u32, u32)>,
    /// Windowed position in pixels.
    pub position: Option<(i32, i32)>,
    pub fullscreen: bool,
}

impl WindowSettings {
    /// Parses settings text. Bad values leave the field unset.
    pub fn parse(text: &str) -> WindowSettings {
        let mut s = WindowSettings::default();
        let (mut w, mut h, mut x, mut y) = (None, None, None, None);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "arrangement" => s.arrangement = Arrangement::from_name(value),
                "width" => w = value.parse::<u32>().ok(),
                "height" => h = value.parse::<u32>().ok(),
                "x" => x = value.parse::<i32>().ok(),
                "y" => y = value.parse::<i32>().ok(),
                "fullscreen" => s.fullscreen = value == "true",
                _ => {}
            }
        }
        let ok = |v: u32| (MIN_SIZE..=MAX_SIZE).contains(&v);
        if let (Some(w), Some(h)) = (w, h)
            && ok(w)
            && ok(h)
        {
            s.size = Some((w, h));
        }
        if let (Some(x), Some(y)) = (x, y) {
            s.position = Some((x, y));
        }
        s
    }

    /// Settings text that [`WindowSettings::parse`] reads back.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if let Some(a) = self.arrangement {
            out += &format!("arrangement={}\n", a.name());
        }
        if let Some((w, h)) = self.size {
            out += &format!("width={w}\nheight={h}\n");
        }
        if let Some((x, y)) = self.position {
            out += &format!("x={x}\ny={y}\n");
        }
        out += &format!("fullscreen={}\n", self.fullscreen);
        out
    }

    /// Reads `path`; a missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> WindowSettings {
        std::fs::read_to_string(path)
            .map(|t| WindowSettings::parse(&t))
            .unwrap_or_default()
    }

    /// Writes `path`, creating its directory.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

/// Default settings file: `<config dir>/rescue/window.cfg`, where the config dir is
/// `$XDG_CONFIG_HOME`, else `~/.config` (or `%APPDATA%` on Windows). `None` if none is set.
pub fn default_path() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = var("XDG_CONFIG_HOME")
        .or_else(|| var("APPDATA"))
        .or_else(|| var("HOME").map(|h| h.join(".config")))?;
    Some(base.join("rescue").join("window.cfg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = WindowSettings {
            arrangement: Some(Arrangement::LargeTop),
            size: Some((1280, 720)),
            position: Some((-20, 40)),
            fullscreen: true,
        };
        assert_eq!(WindowSettings::parse(&s.to_text()), s);
        let d = WindowSettings::default();
        assert_eq!(WindowSettings::parse(&d.to_text()), d);
    }

    #[test]
    fn garbage_is_ignored() {
        let s = WindowSettings::parse(
            "nonsense\narrangement=bogus\nwidth=abc\nheight=480\nx=5\nfullscreen=maybe\n=\n",
        );
        assert_eq!(s, WindowSettings::default());
    }

    #[test]
    fn out_of_range_size_is_dropped_but_position_kept() {
        let s = WindowSettings::parse("width=10\nheight=480\nx=1\ny=2\n");
        assert_eq!(s.size, None);
        assert_eq!(s.position, Some((1, 2)));
        assert_eq!(WindowSettings::parse("width=99999\nheight=480").size, None);
    }

    #[test]
    fn save_and_load_through_a_file() {
        let dir = std::env::temp_dir().join(format!("rescue-settings-{}", std::process::id()));
        let path = dir.join("sub").join("window.cfg");
        let s = WindowSettings {
            arrangement: Some(Arrangement::SideBySide),
            size: Some((800, 600)),
            position: None,
            fullscreen: false,
        };
        s.save(&path).unwrap();
        assert_eq!(WindowSettings::load(&path), s);
        assert_eq!(
            WindowSettings::load(&dir.join("missing.cfg")),
            WindowSettings::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
