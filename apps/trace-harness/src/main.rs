//! Golden-trace replay and state diff CLI.
//!
//! Usage: `rescue-trace-harness <trace.jsonl>`. The sim starts from the trace's first row and
//! steps through each row's leader actions (see `rescue_trace_harness::replay`). Exit 0 on a
//! match. Exit 1 on a divergence (the first divergent row and each differing field go to
//! stdout) or a sim gap (a command the sim does not model yet, with its row). Exit 2 on a bad
//! file, a bad action record or bad usage.

#![forbid(unsafe_code)]

use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

use rescue_nds::rom::SUPPORTED_DUMPS;
use rescue_trace_harness::{
    Replay, SKIPPED_FIELDS, check_rom, render_divergence, render_sim_gap, replay,
};

const USAGE: &str = "usage: rescue-trace-harness <trace.jsonl>";

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
    let [trace_path] = args else {
        return Err(USAGE.to_owned());
    };
    let trace_file = File::open(trace_path).map_err(|e| format!("{trace_path}: {e}"))?;
    let trace = rescue_trace::read_trace(BufReader::new(trace_file))
        .map_err(|e| format!("{trace_path}: {e}"))?;
    check_rom(&trace.header, SUPPORTED_DUMPS).map_err(|e| format!("{trace_path}: {e}"))?;
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
