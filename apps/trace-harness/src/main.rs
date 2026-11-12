//! Golden-trace replay and state diff CLI.
//!
//! Usage: `rescue-trace-harness <trace.jsonl> [--scenario <scenario.json>]`. The sim starts from
//! the trace's first row and steps through each row's leader actions (see
//! `rescue_trace_harness::replay`). The optional scenario file gives the dungeon and floor ids
//! (see `rescue_trace_harness::scenario`). Exit 0 on a match. Exit 1 on a divergence (the first
//! divergent row and each differing field go to stdout) or a sim gap (a command the sim does
//! not model yet, with its row). Exit 2 on a bad file, a bad scenario, a bad action record or
//! bad usage.

#![forbid(unsafe_code)]

use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use rescue_nds::rom::SUPPORTED_DUMPS;
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
    // shortcut: the ids are read and checked but the replay does not use them yet, #371 starts
    // the floor from them.
    let _scenario: Option<Scenario> = scenario_path
        .map(String::as_str)
        .map(load_scenario)
        .transpose()?;
    match replay(&trace.rows).map_err(|e| format!("{trace_path}: {e}"))? {
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
