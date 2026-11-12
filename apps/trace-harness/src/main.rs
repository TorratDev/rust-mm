//! Golden-trace replay and state diff CLI.
//!
//! Usage: `rescue-trace-harness <trace.jsonl> [--scenario <scenario.json>]`. The sim starts from
//! the trace's first row and steps through each row's leader actions (see
//! `rescue_trace_harness::replay`). The optional scenario file gives the dungeon and floor ids
//! (see `rescue_trace_harness::scenario`). With a scenario, the first floor is generated from
//! the floor data in the ROM, named by `RESCUE_ROM` or `rescue.toml` (see
//! `rescue_trace_harness::floor`). Exit 0 on a match. Exit 1 on a divergence (the first
//! divergent row and each differing field go to stdout) or a sim gap (a command the sim does
//! not model yet, with its row). Exit 2 on a bad file, a bad scenario, a bad action record or
//! bad usage.

#![forbid(unsafe_code)]

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rescue_nds::rom::{
    CONFIG_FILE_NAME, ROM_PATH_ENV, SUPPORTED_DUMPS, identify_bytes, resolve_rom_path,
};
use rescue_sim::floor_gen::FloorProperties;
use rescue_trace_harness::floor::start_properties;
use rescue_trace_harness::scenario::{Scenario, parse_scenario};
use rescue_trace_harness::{
    Replay, SKIPPED_FIELDS, check_rom, render_divergence, render_sim_gap, replay,
};

const USAGE: &str = "usage: rescue-trace-harness <trace.jsonl> [--scenario <scenario.json>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("rescue-trace-harness: {message}");
            ExitCode::from(2)
        }
    }
}

/// Runs one replay. `Ok(true)` on a match, `Ok(false)` on a divergence or a sim gap, `Err`
/// on a bad file or bad usage.
fn run(args: &[String]) -> Result<bool, String> {
    let (trace_path, scenario_path) = match args {
        [trace] => (trace, None),
        [trace, flag, scenario] if flag == "--scenario" => (trace, Some(scenario)),
        _ => return Err(USAGE.to_owned()),
    };
    let trace_file = File::open(trace_path).map_err(|e| format!("{trace_path}: {e}"))?;
    let trace = rescue_trace::read_trace(BufReader::new(trace_file))
        .map_err(|e| format!("{trace_path}: {e}"))?;
    check_rom(&trace.header, SUPPORTED_DUMPS).map_err(|e| format!("{trace_path}: {e}"))?;
    let floor = match scenario_path {
        Some(path) => {
            let scenario = load_scenario(path)?;
            Some(load_floor(&trace.header.rom.sha1, scenario)?)
        }
        None => {
            eprintln!("note: no --scenario, the floor is left empty and moves may be refused");
            None
        }
    };
    match replay(&trace.rows, floor.as_ref()).map_err(|e| format!("{trace_path}: {e}"))? {
        Replay::Match { rows } => {
            println!(
                "match: {rows} rows ({} fields not compared, see SKIPPED_FIELDS)",
                SKIPPED_FIELDS.len()
            );
            Ok(true)
        }
        Replay::Divergence(divergence) => {
            print!("{}", render_divergence(&divergence));
            Ok(false)
        }
        Replay::SimGap { row, command } => {
            print!("{}", render_sim_gap(row, command));
            Ok(false)
        }
    }
}

/// Reads and checks a scenario file. Any problem is an error, so the run exits 2.
fn load_scenario(path: &str) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    parse_scenario(&text).map_err(|e| format!("{path}: {e}"))
}

/// The first floor's properties from the ROM. The ROM must be the trace's own dump.
fn load_floor(trace_sha1: &str, scenario: Scenario) -> Result<FloorProperties, String> {
    let path = rom_path()?;
    let rom = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let id = identify_bytes(&rom).map_err(|e| format!("{}: {e}", path.display()))?;
    let rom_sha1 = id.dump().sha1.to_string();
    if rom_sha1 != trace_sha1 {
        return Err(format!(
            "{} is ROM SHA-1 {rom_sha1}, but the trace is for {trace_sha1}",
            path.display(),
        ));
    }
    start_properties(&rom, scenario).map_err(|e| e.to_string())
}

/// The ROM path: `RESCUE_ROM`, else `rom_path` in `rescue.toml`.
fn rom_path() -> Result<PathBuf, String> {
    resolve_rom_path(
        std::env::var_os(ROM_PATH_ENV),
        Some(Path::new(CONFIG_FILE_NAME)),
    )
    .map_err(|e| e.to_string())?
    .ok_or_else(|| {
        format!("the scenario needs the ROM: set {ROM_PATH_ENV} or rom_path in {CONFIG_FILE_NAME}")
    })
}
