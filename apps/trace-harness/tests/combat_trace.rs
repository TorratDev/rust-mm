//! The leader's combat replay of a local first-floor trace (#424): the leader's attacks on an
//! adjacent wild monster, compared on the leader and the target. The trace is a capture from the
//! retail dump (see `docs/golden-traces.md`), so it is not in the repository. The test runs only
//! when `RESCUE_TRACE_COMBAT` names the trace; otherwise it prints SKIPPED and passes.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rescue_nds::rom::{
    CONFIG_FILE_NAME, ROM_PATH_ENV, SUPPORTED_DUMPS, identify_bytes, resolve_rom_path,
};
use rescue_trace_harness::combat::{CombatSetup, replay_combat};
use rescue_trace_harness::floor::start_properties;
use rescue_trace_harness::scenario::parse_scenario;
use rescue_trace_harness::{Replay, check_rom, render_divergence, render_sim_gap};

const TRACE_VAR: &str = "RESCUE_TRACE_COMBAT";
const SCENARIO_VAR: &str = "RESCUE_TRACE_COMBAT_SCENARIO";
const TEST: &str = "leader_and_target_match_every_attack_row_of_the_combat_trace";

#[test]
fn leader_and_target_match_every_attack_row_of_the_combat_trace() {
    let Some(trace_path) = std::env::var_os(TRACE_VAR) else {
        eprintln!("SKIPPED {TEST}: {TRACE_VAR} not set");
        return;
    };
    let scenario_path = std::env::var_os(SCENARIO_VAR)
        .unwrap_or_else(|| panic!("{SCENARIO_VAR} must name the scenario file of the trace"));
    let trace_path = PathBuf::from(trace_path);
    let trace = rescue_trace::read_trace(BufReader::new(
        File::open(&trace_path).unwrap_or_else(|e| panic!("{}: {e}", trace_path.display())),
    ))
    .unwrap_or_else(|e| panic!("{}: {e}", trace_path.display()));
    check_rom(&trace.header, SUPPORTED_DUMPS).unwrap_or_else(|e| panic!("{e}"));

    // The floor and the tables need the trace's own dump (as `rescue-trace-harness --scenario`).
    let rom_path = resolve_rom_path(
        std::env::var_os(ROM_PATH_ENV),
        Some(Path::new(CONFIG_FILE_NAME)),
    )
    .unwrap_or_else(|e| panic!("{e}"))
    .unwrap_or_else(|| panic!("the trace needs the ROM: set {ROM_PATH_ENV}"));
    let rom = std::fs::read(&rom_path).unwrap_or_else(|e| panic!("{}: {e}", rom_path.display()));
    let rom_sha1 = identify_bytes(&rom)
        .unwrap_or_else(|e| panic!("{}: {e}", rom_path.display()))
        .dump()
        .sha1
        .to_string();
    assert_eq!(
        rom_sha1, trace.header.rom.sha1,
        "{ROM_PATH_ENV} is not the trace's dump"
    );
    let text = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("{scenario_path:?}: {e}"));
    let scenario = parse_scenario(&text).unwrap_or_else(|e| panic!("{scenario_path:?}: {e}"));
    let floor = start_properties(&rom, scenario).unwrap_or_else(|e| panic!("{e}"));
    let leader = trace
        .rows
        .first()
        .and_then(|row| row.team[0])
        .map(|e| e.species)
        .and_then(|s| u16::try_from(s).ok())
        .unwrap_or_else(|| panic!("the trace's first row has no leader species"));
    let setup = CombatSetup::from_rom(&rom, leader).unwrap_or_else(|e| panic!("{e}"));

    match replay_combat(&trace.rows, Some(&floor), &setup).expect("replay") {
        Replay::Match { rows } => {
            eprintln!("{TEST}: no leader or target difference over {rows} trace rows");
        }
        Replay::Divergence(d) => panic!("{}", render_divergence(&d)),
        Replay::SimGap { row, command } => panic!("{}", render_sim_gap(row, command)),
    }
}
