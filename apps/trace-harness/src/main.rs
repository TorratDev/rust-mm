//! Golden-trace replay and state diff CLI.
//!
//! Usage: `rescue-trace-harness <trace.jsonl> --sim <states.jsonl>`. The sim-states file
//! holds one `rescue_sim` snapshot per turn row (the stub source until the start-state
//! issue lands). Exit 0 on a match, 1 on a divergence (the first divergent row and each
//! differing field go to stdout), 2 on a bad file or bad usage.

#![forbid(unsafe_code)]

use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use rescue_nds::rom::SUPPORTED_DUMPS;
use rescue_trace_harness::{SKIPPED_FIELDS, check_rom, diff_trace, read_states, render_divergence};

const USAGE: &str = "usage: rescue-trace-harness <trace.jsonl> --sim <states.jsonl>";

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

/// Runs one comparison. `Ok(true)` on a match, `Ok(false)` on a divergence, `Err` on a bad
/// file or bad usage.
fn run(args: &[String]) -> Result<bool, String> {
    let [trace_path, flag, states_path] = args else {
        return Err(USAGE.to_owned());
    };
    if flag != "--sim" {
        return Err(USAGE.to_owned());
    }
    let trace_file = File::open(trace_path).map_err(|e| format!("{trace_path}: {e}"))?;
    let trace = rescue_trace::read_trace(BufReader::new(trace_file))
        .map_err(|e| format!("{trace_path}: {e}"))?;
    check_rom(&trace.header, SUPPORTED_DUMPS).map_err(|e| format!("{trace_path}: {e}"))?;
    let states_file = File::open(states_path).map_err(|e| format!("{states_path}: {e}"))?;
    let states =
        read_states(BufReader::new(states_file)).map_err(|e| format!("{states_path}: {e}"))?;
    match diff_trace(&trace.rows, &states).map_err(|e| e.to_string())? {
        None => {
            println!(
                "match: {} rows ({} fields not compared, see SKIPPED_FIELDS)",
                trace.rows.len(),
                SKIPPED_FIELDS.len()
            );
            Ok(true)
        }
        Some(divergence) => {
            print!("{}", render_divergence(&divergence));
            Ok(false)
        }
    }
}
