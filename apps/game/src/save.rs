//! The run's quicksave file: where it lives, and writing and reading it with `rescue_save`.
//!
//! The file is `<data dir>/rescue/suspend.sav`, where the data dir is `$XDG_DATA_HOME`, else
//! `~/.local/share` (or `%APPDATA%` on Windows), the user-data counterpart of the settings file
//! in the config dir (`rescue_platform::settings::default_path`). `rescue.toml` is read from the
//! working directory and holds the ROM path, so a save does not go beside it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rescue_sim::state::GameState;

/// The save's default path, or `None` if no data dir can be found.
pub fn default_path() -> Option<PathBuf> {
    path_in(
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("APPDATA"),
        std::env::var_os("HOME"),
    )
}

fn path_in(
    xdg: Option<OsString>,
    appdata: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let set = |v: Option<OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = set(xdg)
        .or_else(|| set(appdata))
        .or_else(|| set(home).map(|h| h.join(".local").join("share")))?;
    Some(base.join("rescue").join("suspend.sav"))
}

/// Writes the run to `path`, through a temporary file so a failed write keeps the old save.
pub fn write(path: &Path, game: &GameState) -> Result<(), String> {
    let bytes = rescue_save::write_run(game);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("sav.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads the run at `path`. The message names why a file was refused.
pub fn read(path: &Path) -> Result<GameState, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    rescue_save::load(&bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn the_path_follows_the_data_dir_order() {
        let tail = Path::new("rescue").join("suspend.sav");
        assert_eq!(
            path_in(os("/x"), os("/a"), os("/h")),
            Some(Path::new("/x").join(&tail))
        );
        assert_eq!(
            path_in(None, os("/a"), os("/h")),
            Some(Path::new("/a").join(&tail))
        );
        assert_eq!(
            path_in(os(""), None, os("/h")),
            Some(Path::new("/h/.local/share").join(&tail))
        );
        assert_eq!(path_in(None, None, None), None);
    }
}
