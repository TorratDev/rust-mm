//! Boot check: the game runs only with a configured, supported ROM dump (#21).
//! The dump is read from `RESCUE_ROM`, else from `rom_path` in `rescue.toml`.

use std::ffi::OsString;
use std::path::Path;

use rescue_nds::rom::{self, KnownDump, RomId, SUPPORTED_DUMPS};

/// Outcome of the ROM check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomCheck {
    Ready(RomId),
    NotConfigured,
    /// The reason, for the user.
    Refused(String),
}

/// Checks the ROM named by `env` (the `RESCUE_ROM` value) or by `config` (`rescue.toml`).
pub fn check(env: Option<OsString>, config: Option<&Path>) -> RomCheck {
    check_with(env, config, SUPPORTED_DUMPS)
}

fn check_with(env: Option<OsString>, config: Option<&Path>, table: &[KnownDump]) -> RomCheck {
    match rom::resolve_rom_path(env, config) {
        Err(e) => RomCheck::Refused(e.to_string()),
        Ok(None) => RomCheck::NotConfigured,
        Ok(Some(path)) => match rom::identify_file_with(&path, table) {
            Ok(id) => RomCheck::Ready(id),
            Err(e) => RomCheck::Refused(format!("{}: {e}", path.display())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_nds::rom::Sha1;

    #[test]
    fn nothing_configured() {
        assert_eq!(
            check_with(None, None, SUPPORTED_DUMPS),
            RomCheck::NotConfigured
        );
        assert_eq!(
            check_with(Some(OsString::new()), None, SUPPORTED_DUMPS),
            RomCheck::NotConfigured
        );
    }

    #[test]
    fn missing_file_is_refused() {
        let missing = std::env::temp_dir().join("rescue-boot-test-missing.nds");
        let out = check_with(Some(missing.into_os_string()), None, SUPPORTED_DUMPS);
        assert!(matches!(out, RomCheck::Refused(ref m) if m.contains("cannot read")));
    }

    #[test]
    fn supported_and_unknown_dumps() {
        // Synthetic bytes, so the table is injected: the real dump is never read here.
        let data = b"synthetic rom";
        let path =
            std::env::temp_dir().join(format!("rescue-boot-test-{}.nds", std::process::id()));
        std::fs::write(&path, data).unwrap();
        let table = [KnownDump {
            sha1: Sha1::of_bytes(data),
            size: data.len() as u64,
            ..SUPPORTED_DUMPS[0]
        }];
        let ok = check_with(Some(path.clone().into_os_string()), None, &table);
        assert_eq!(ok, RomCheck::Ready(RomId::BlueRescueTeamEur));
        let refused = check_with(Some(path.clone().into_os_string()), None, SUPPORTED_DUMPS);
        assert!(matches!(refused, RomCheck::Refused(ref m) if m.contains("unsupported ROM dump")));
        std::fs::remove_file(&path).unwrap();
    }
}
