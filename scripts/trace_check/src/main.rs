//! Reads one golden trace with `rescue-trace` and prints a JSON summary.
//!
//! Exits with status 1 and a message on stderr if the file is rejected.

use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: trace_check FILE.jsonl");
        return ExitCode::from(2);
    };
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("cannot open {path}: {err}");
            return ExitCode::from(1);
        }
    };
    match rescue_trace::read_trace(BufReader::new(file)) {
        Ok(trace) => {
            let summary = serde_json::json!({
                "schemaVersion": trace.header.schema_version,
                "romSha1": trace.header.rom.sha1,
                "scenario": trace.header.scenario,
                "rows": trace.rows.len(),
            });
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}
