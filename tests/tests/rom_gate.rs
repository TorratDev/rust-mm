//! ROM-gated test infrastructure (#27).
//!
//! The helper is exercised with synthetic files and an injected supported-dumps table,
//! so these tests run everywhere. `whole_rom_is_supported` is a real ROM-gated test: it
//! skips without a ROM and runs locally when one is configured.

use std::path::{Path, PathBuf};

use rescue_nds::rom::{KnownDump, RomError, RomId, SUPPORTED_DUMPS, Sha1};
use rescue_tests::rom::{Gate, TestRom, find_rom_with, fixtures_root, gate};

/// Fresh scratch directory for one test, under the git-ignored fixtures root.
fn scratch(name: &str) -> PathBuf {
    let dir = fixtures_root()
        .join("_selftest")
        .join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Synthetic "dump": deterministic bytes, nothing from a real ROM.
fn synthetic_rom(dir: &Path) -> (PathBuf, Vec<u8>) {
    let data: Vec<u8> = (0..65_537u32)
        .map(|i| (i.wrapping_mul(31) % 256) as u8)
        .collect();
    let path = dir.join("synthetic.bin");
    std::fs::write(&path, &data).unwrap();
    (path, data)
}

fn fake_table(data: &[u8]) -> [KnownDump; 1] {
    [KnownDump {
        sha1: Sha1::of_bytes(data),
        size: data.len() as u64,
        ..SUPPORTED_DUMPS[0]
    }]
}

#[test]
fn no_rom_configured_skips() {
    let dir = scratch("none");
    let found = find_rom_with(None, Some(&dir.join("rescue.toml")), SUPPORTED_DUMPS)
        .map_err(|e| e.to_string());
    assert_eq!(found, Ok(None));
    match gate("t", &found, false) {
        Gate::Skip(msg) => {
            assert!(msg.starts_with("SKIPPED t:"), "{msg}");
            assert!(msg.contains("RESCUE_ROM"), "{msg}");
        }
        other => panic!("expected skip, got {other:?}"),
    }
    assert!(matches!(gate("t", &found, true), Gate::Fail(_)));
}

#[test]
fn known_rom_via_env_runs_and_gets_fixture_dir() {
    let dir = scratch("env");
    let (path, data) = synthetic_rom(&dir);
    let table = fake_table(&data);

    let rom = find_rom_with(Some(path.clone().into()), None, &table)
        .unwrap()
        .unwrap();
    assert_eq!(rom.path, path);
    assert_eq!(rom.id, RomId::BlueRescueTeamEur);
    assert_eq!(rom.sha1, Sha1::of_bytes(&data));
    assert_eq!(rom.read(), data);

    let fixtures = rom.fixture_dir_in(&dir.join("fixtures"), "decoded");
    assert!(fixtures.is_dir());
    assert!(fixtures.ends_with(Path::new(&rom.sha1.to_string()).join("decoded")));

    assert_eq!(gate("t", &Ok(Some(rom.clone())), true), Gate::Run(rom));
}

#[test]
fn known_rom_via_config_file() {
    let dir = scratch("config");
    let (path, data) = synthetic_rom(&dir);
    let cfg = dir.join("rescue.toml");
    std::fs::write(&cfg, "# local\nrom_path = \"synthetic.bin\"\n").unwrap();

    let rom = find_rom_with(None, Some(&cfg), &fake_table(&data))
        .unwrap()
        .unwrap();
    assert_eq!(
        std::fs::canonicalize(rom.path).unwrap(),
        std::fs::canonicalize(path).unwrap()
    );
}

#[test]
fn unknown_rom_fails_with_supported_list() {
    let dir = scratch("unknown");
    let (path, data) = synthetic_rom(&dir);

    // Against the real list, synthetic data is an unknown dump.
    let err = find_rom_with(Some(path.into()), None, SUPPORTED_DUMPS).unwrap_err();
    assert!(matches!(err, RomError::UnknownDump { .. }));
    let found: Result<Option<TestRom>, String> = Err(err.to_string());
    match gate("t", &found, false) {
        Gate::Fail(msg) => {
            assert!(msg.contains(&Sha1::of_bytes(&data).to_string()), "{msg}");
            assert!(
                msg.contains("6f01887356268de39085df01490347bbcc3aa330"),
                "{msg}"
            );
        }
        other => panic!("expected failure, got {other:?}"),
    }
}

#[test]
fn missing_rom_file_is_an_error_not_a_skip() {
    let dir = scratch("missing");
    let err = find_rom_with(Some(dir.join("nope.nds").into()), None, SUPPORTED_DUMPS).unwrap_err();
    assert!(matches!(err, RomError::Io { .. }));
}

rescue_tests::rom_test! {
    /// Runs only with a configured ROM: the real dump is on the supported list.
    fn whole_rom_is_supported(rom) {
        let dump = rom.id.dump();
        assert_eq!(rom.sha1, dump.sha1);
        assert_eq!(std::fs::metadata(&rom.path).unwrap().len(), dump.size);
    }
}
