//! ROM-gated test support.
//!
//! Tests that need the user's dump use [`rom_test!`](crate::rom_test). With no ROM
//! configured (the CI case) they print a `SKIPPED` line and return without running
//! their body. A ROM that is configured but cannot be read or is not a supported dump
//! fails the test: silently skipping would hide a misconfiguration.
//!
//! Set `RESCUE_REQUIRE_ROM=1` to turn "no ROM configured" into a failure too, so a
//! local run cannot pass by skipping everything.
//!
//! Fixtures derived from the ROM are written at test time under
//! `target/rom-fixtures/<sha1>/` (git-ignored with the rest of `target/`) and are never
//! committed. See `docs/rom-tests.md`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rescue_nds::rom::{
    self, CONFIG_FILE_NAME, KnownDump, ROM_PATH_ENV, RomError, RomId, SUPPORTED_DUMPS, Sha1,
};

use crate::workspace_root;

/// Environment variable that makes a missing ROM a test failure instead of a skip.
pub const REQUIRE_ROM_ENV: &str = "RESCUE_REQUIRE_ROM";

/// A configured, identified ROM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestRom {
    /// Path to the dump.
    pub path: PathBuf,
    /// Which supported dump it is.
    pub id: RomId,
    /// Its whole-file SHA-1.
    pub sha1: Sha1,
}

impl TestRom {
    /// Reads the whole dump into memory.
    pub fn read(&self) -> Vec<u8> {
        std::fs::read(&self.path)
            .unwrap_or_else(|e| panic!("cannot read ROM {}: {e}", self.path.display()))
    }

    /// Directory for fixtures generated from this ROM at test time, created if needed:
    /// `<fixtures_root>/<sha1>/<name>`. Never commit its contents.
    pub fn fixture_dir_in(&self, fixtures_root: &Path, name: &str) -> PathBuf {
        let dir = fixtures_root.join(self.sha1.to_string()).join(name);
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|e| panic!("cannot create {}: {e}", dir.display()));
        dir
    }

    /// [`fixture_dir_in`](Self::fixture_dir_in) under the default, git-ignored root.
    pub fn fixture_dir(&self, name: &str) -> PathBuf {
        self.fixture_dir_in(&fixtures_root(), name)
    }
}

/// Default root for generated fixtures: `<workspace>/target/rom-fixtures`.
pub fn fixtures_root() -> PathBuf {
    workspace_root().join("target").join("rom-fixtures")
}

/// Finds and identifies the ROM given explicit inputs (injectable for testing).
///
/// `Ok(None)` means no ROM is configured. An error means one is configured but
/// unreadable or not on `table`.
pub fn find_rom_with(
    env_value: Option<OsString>,
    config_file: Option<&Path>,
    table: &[KnownDump],
) -> Result<Option<TestRom>, RomError> {
    let Some(path) = rom::resolve_rom_path(env_value, config_file)? else {
        return Ok(None);
    };
    let file = std::fs::File::open(&path).map_err(|source| RomError::Io {
        path: path.clone(),
        source,
    })?;
    let (sha1, size) = Sha1::of_reader(file).map_err(|source| RomError::Io {
        path: path.clone(),
        source,
    })?;
    let id = rom::identify_hash(sha1, size, table)?;
    Ok(Some(TestRom { path, id, sha1 }))
}

/// The ROM configured through `RESCUE_ROM` or `<workspace>/rescue.toml`, identified
/// against the real supported list. Computed once per test binary.
pub fn configured_rom() -> &'static Result<Option<TestRom>, String> {
    static ROM: OnceLock<Result<Option<TestRom>, String>> = OnceLock::new();
    ROM.get_or_init(|| {
        let config = workspace_root().join(CONFIG_FILE_NAME);
        find_rom_with(
            std::env::var_os(ROM_PATH_ENV),
            Some(&config),
            SUPPORTED_DUMPS,
        )
        .map_err(|e| e.to_string())
    })
}

/// What a ROM-gated test should do.
#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    /// Run the test body with this ROM.
    Run(TestRom),
    /// Skip, with the reason to print.
    Skip(String),
    /// Fail, with the reason.
    Fail(String),
}

/// Decides what a ROM-gated test does, from a lookup result and the
/// `RESCUE_REQUIRE_ROM` setting. Pure, so it can be tested without a ROM.
pub fn gate(test: &str, found: &Result<Option<TestRom>, String>, require: bool) -> Gate {
    match found {
        Ok(Some(rom)) => Gate::Run(rom.clone()),
        Ok(None) if require => Gate::Fail(format!(
            "{test}: {REQUIRE_ROM_ENV} is set but no ROM is configured \
             (set {ROM_PATH_ENV} or rom_path in {CONFIG_FILE_NAME}; see docs/rom-tests.md)"
        )),
        Ok(None) => Gate::Skip(format!(
            "SKIPPED {test}: no ROM configured \
             (set {ROM_PATH_ENV} or rom_path in {CONFIG_FILE_NAME}; see docs/rom-tests.md)"
        )),
        Err(e) => Gate::Fail(format!("{test}: configured ROM rejected: {e}")),
    }
}

/// Entry point used by [`rom_test!`](crate::rom_test): returns the ROM, or `None` after
/// printing a skip message, or panics if the ROM is misconfigured.
pub fn require_rom(test: &str) -> Option<TestRom> {
    let require = std::env::var_os(REQUIRE_ROM_ENV).is_some_and(|v| !v.is_empty() && v != "0");
    match gate(test, configured_rom(), require) {
        Gate::Run(rom) => Some(rom),
        Gate::Skip(msg) => {
            eprintln!("{msg}");
            None
        }
        Gate::Fail(msg) => panic!("{msg}"),
    }
}

/// Declares a test that needs the user's ROM.
///
/// ```text
/// rescue_tests::rom_test! {
///     /// Doc comments and attributes are kept.
///     fn header_parses(rom) {
///         let bytes = rom.read();
///         // ...
///     }
/// }
/// ```
///
/// Without a configured ROM the test prints `SKIPPED <name>: ...` and returns.
#[macro_export]
macro_rules! rom_test {
    ($(#[$meta:meta])* fn $name:ident($rom:ident) $body:block) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            let Some($rom) = $crate::rom::require_rom(stringify!($name)) else {
                return;
            };
            $body
        }
    };
}
