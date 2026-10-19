//! Golden-trace format, schema version 1. The spec is `docs/golden-traces.md`.
//!
//! A trace is JSON Lines: one header object, then one object per
//! `RunFractionalTurn_Async` call. The row shape is the one
//! `verification/v7-trace/retail_trace.py` writes. This crate reads and writes
//! the format and checks the values the spec states. It holds no game data.

#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

/// The schema version this crate reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Input key names, as they appear in the `FRAME KEY ...` lines.
pub const KEYS: [&str; 12] = [
    "A", "B", "SELECT", "START", "RIGHT", "LEFT", "UP", "DOWN", "R", "L", "X", "Y",
];

/// The first line of a trace file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Header {
    pub schema_version: u32,
    pub rom: Rom,
    pub emulator: Emulator,
    pub scenario: String,
    pub initial_seed: InitialSeed,
    pub inputs: Inputs,
}

/// The ROM the trace was captured from, by SHA-1 (40 lowercase hex digits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rom {
    pub sha1: String,
}

/// The emulator that captured the trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emulator {
    pub name: String,
    pub version: String,
}

/// The boot seed: 6 bytes copied from `source` (an 8-digit hex RAM address) at boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialSeed {
    pub source: String,
    pub bytes: String,
}

/// The input command lists, each line in the `FRAME KEY ...` format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub boot: Vec<String>,
    pub turn: Vec<String>,
}

/// One turn: the state read when `RunFractionalTurn_Async` runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnRow {
    pub fractional_turn: i16,
    pub run_seed: u32,
    pub floor_seed: u32,
    #[serde(rename = "sPRNGState")]
    pub s_prng_state: u32,
    pub dungeon_rng: u32,
    pub dungeon_preseed: u32,
    /// Four slots, `null` where no team member is set.
    pub team: [Option<Entity>; 4],
    /// Only the set slots; empty slots are left out.
    pub wild: Vec<Entity>,
    /// First 16 hex digits of SHA-256 over the tile grid (see the spec).
    pub tiles_sha256: String,
    /// Frame counter of the capture run.
    pub frame: u64,
}

/// One monster, item or trap, read through its entity pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entity {
    #[serde(rename = "type")]
    pub kind: u32,
    pub x: i16,
    pub y: i16,
    pub species: i16,
    pub level: u8,
    pub hp: i16,
    pub max_hp: i16,
    pub speed_stage: u32,
}

/// A whole trace: the header and its rows in capture order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub header: Header,
    pub rows: Vec<TurnRow>,
}

/// One parsed `FRAME KEY ...` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLine {
    pub frame: u64,
    /// Upper-case key names held from `frame`. Empty means all keys released.
    pub keys: Vec<String>,
}

#[derive(Debug)]
pub enum TraceError {
    Io(io::Error),
    /// A line that is not valid JSON or does not match the row or header shape.
    Json {
        line: usize,
        source: serde_json::Error,
    },
    /// Writing a value failed. Should not happen for the types here.
    Encode(serde_json::Error),
    /// The first line is not a header (no `schemaVersion`).
    MissingHeader,
    /// The header's `schemaVersion` is not one this crate reads.
    UnsupportedSchema(u64),
    /// A value breaks a rule the spec states.
    Invalid(String),
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json { line, source } => write!(f, "line {line}: {source}"),
            Self::Encode(e) => write!(f, "cannot encode trace: {e}"),
            Self::MissingHeader => write!(f, "first line is not a header (no schemaVersion)"),
            Self::UnsupportedSchema(v) => {
                write!(
                    f,
                    "schemaVersion {v} is not supported (this reader reads {SCHEMA_VERSION})"
                )
            }
            Self::Invalid(reason) => write!(f, "invalid trace: {reason}"),
        }
    }
}

impl std::error::Error for TraceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json { source, .. } | Self::Encode(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for TraceError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Reads a whole trace. Blank lines are skipped.
pub fn read_trace<R: BufRead>(reader: R) -> Result<Trace, TraceError> {
    let mut header = None;
    let mut rows = Vec::new();
    for (index, text) in reader.lines().enumerate() {
        let text = text?;
        let line = index + 1;
        if text.trim().is_empty() {
            continue;
        }
        if header.is_none() {
            header = Some(parse_header(&text, line)?);
        } else {
            rows.push(
                serde_json::from_str(&text).map_err(|source| TraceError::Json { line, source })?,
            );
        }
    }
    let trace = Trace {
        header: header.ok_or(TraceError::MissingHeader)?,
        rows,
    };
    validate(&trace)?;
    Ok(trace)
}

/// Writes a trace as JSON Lines, after checking it.
pub fn write_trace<W: Write>(mut writer: W, trace: &Trace) -> Result<(), TraceError> {
    validate(trace)?;
    let header = serde_json::to_string(&trace.header).map_err(TraceError::Encode)?;
    writeln!(writer, "{header}")?;
    for row in &trace.rows {
        let text = serde_json::to_string(row).map_err(TraceError::Encode)?;
        writeln!(writer, "{text}")?;
    }
    Ok(())
}

/// Parses one `FRAME KEY ...` line. A lone `-` after the frame releases all keys.
pub fn parse_input_line(text: &str) -> Result<InputLine, TraceError> {
    let invalid = |why: &str| TraceError::Invalid(format!("input line {text:?}: {why}"));
    let mut words = text.split_whitespace();
    let frame = words
        .next()
        .and_then(|w| w.parse::<u64>().ok())
        .ok_or_else(|| invalid("frame must be a number"))?;
    let names: Vec<String> = words.map(str::to_ascii_uppercase).collect();
    if names.is_empty() {
        return Err(invalid("no keys (use `-` to release all)"));
    }
    if names == ["-"] {
        return Ok(InputLine {
            frame,
            keys: Vec::new(),
        });
    }
    for name in &names {
        if !KEYS.contains(&name.as_str()) {
            return Err(invalid(&format!("unknown key {name}")));
        }
    }
    Ok(InputLine { frame, keys: names })
}

fn parse_header(text: &str, line: usize) -> Result<Header, TraceError> {
    let json = |source| TraceError::Json { line, source };
    let value: serde_json::Value = serde_json::from_str(text).map_err(json)?;
    match value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
    {
        None => return Err(TraceError::MissingHeader),
        Some(v) if v != u64::from(SCHEMA_VERSION) => return Err(TraceError::UnsupportedSchema(v)),
        Some(_) => {}
    }
    serde_json::from_value(value).map_err(json)
}

fn validate(trace: &Trace) -> Result<(), TraceError> {
    let header = &trace.header;
    if header.schema_version != SCHEMA_VERSION {
        return Err(TraceError::UnsupportedSchema(u64::from(
            header.schema_version,
        )));
    }
    let invalid = |what: &str| Err(TraceError::Invalid(what.to_owned()));
    if !is_lower_hex(&header.rom.sha1, 40) {
        return invalid("rom.sha1 must be 40 lowercase hex digits");
    }
    if !is_hex(&header.initial_seed.source, 8) {
        return invalid("initialSeed.source must be an 8-digit hex address");
    }
    if !is_hex(&header.initial_seed.bytes, 12) {
        return invalid("initialSeed.bytes must be 12 hex digits (6 bytes)");
    }
    for line in &header.inputs.boot {
        parse_input_line(line)?;
    }
    for line in &header.inputs.turn {
        parse_input_line(line)?;
    }
    for row in &trace.rows {
        if !is_lower_hex(&row.tiles_sha256, 16) {
            return invalid("tilesSha256 must be 16 lowercase hex digits");
        }
    }
    Ok(())
}

fn is_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

    fn header() -> Header {
        Header {
            schema_version: SCHEMA_VERSION,
            rom: Rom {
                sha1: SHA1.to_owned(),
            },
            emulator: Emulator {
                name: "synthetic".to_owned(),
                version: "0.0".to_owned(),
            },
            scenario: "synthetic-scenario".to_owned(),
            initial_seed: InitialSeed {
                source: "027FFCF4".to_owned(),
                bytes: "0011223344AA".to_owned(),
            },
            inputs: Inputs {
                boot: vec!["0 -".to_owned(), "10 A".to_owned()],
                turn: vec!["0 A RIGHT".to_owned(), "4 -".to_owned()],
            },
        }
    }

    fn entity(species: i16) -> Entity {
        Entity {
            kind: 1,
            x: 10,
            y: -12,
            species,
            level: 5,
            hp: 20,
            max_hp: 25,
            speed_stage: 6,
        }
    }

    fn row(frame: u64, s_prng: u32) -> TurnRow {
        TurnRow {
            fractional_turn: 0,
            run_seed: 123_456,
            floor_seed: 654_321,
            s_prng_state: s_prng,
            dungeon_rng: 0xAAAA_AAAA,
            dungeon_preseed: 0x5555_5555,
            team: [Some(entity(1)), None, None, None],
            wild: vec![entity(2)],
            tiles_sha256: "0123456789abcdef".to_owned(),
            frame,
        }
    }

    #[test]
    fn round_trip_header_and_two_rows() {
        let trace = Trace {
            header: header(),
            rows: vec![row(42, 1), row(43, 2)],
        };
        let mut bytes = Vec::new();
        write_trace(&mut bytes, &trace).expect("write");
        let back = read_trace(bytes.as_slice()).expect("read");
        assert_eq!(back, trace);
    }

    #[test]
    fn rejects_unknown_schema_version() {
        for version in [0_u64, 2] {
            let mut value = serde_json::to_value(header()).expect("to_value");
            value["schemaVersion"] = serde_json::json!(version);
            let text = format!("{value}\n");
            match read_trace(text.as_bytes()) {
                Err(TraceError::UnsupportedSchema(v)) => assert_eq!(v, version),
                other => {
                    panic!("schemaVersion {version}: expected UnsupportedSchema, got {other:?}")
                }
            }
        }
    }

    #[test]
    fn accepts_row_exactly_as_retail_trace_py_emits_it() {
        // `json.dumps(row, sort_keys=True)` from retail_trace.py, for a synthetic row
        // with one team member and no wild Pokemon.
        let line = r#"{"dungeonPreseed": 1431655765, "dungeonRng": 2863311531, "floorSeed": 654321, "fractionalTurn": 0, "frame": 42, "runSeed": 123456, "sPRNGState": 3735928559, "team": [{"hp": 20, "level": 5, "maxHp": 25, "species": 1, "speedStage": 6, "type": 1, "x": 10, "y": 12}, null, null, null], "tilesSha256": "0123456789abcdef", "wild": []}"#;
        let parsed: TurnRow = serde_json::from_str(line).expect("retail row");
        assert_eq!(parsed.s_prng_state, 3_735_928_559);
        assert_eq!(parsed.frame, 42);
        assert_eq!(parsed.team[0].map(|e| e.max_hp), Some(25));
        assert!(parsed.team[1].is_none());
        assert!(parsed.wild.is_empty());
    }

    #[test]
    fn rejects_unknown_row_field_such_as_inventory() {
        let line = r#"{"dungeonPreseed": 1, "dungeonRng": 2, "floorSeed": 3, "fractionalTurn": 0, "frame": 4, "inventory": [], "runSeed": 5, "sPRNGState": 6, "team": [null, null, null, null], "tilesSha256": "0123456789abcdef", "wild": []}"#;
        assert!(serde_json::from_str::<TurnRow>(line).is_err());
    }

    #[test]
    fn rejects_bad_tiles_hash_and_sha1() {
        let mut bad_row = row(1, 1);
        bad_row.tiles_sha256 = "0123456789ABCDEF".to_owned();
        let trace = Trace {
            header: header(),
            rows: vec![bad_row],
        };
        assert!(matches!(validate(&trace), Err(TraceError::Invalid(_))));

        let mut bad_header = header();
        bad_header.rom.sha1 = "xyz".to_owned();
        let trace = Trace {
            header: bad_header,
            rows: vec![],
        };
        assert!(matches!(validate(&trace), Err(TraceError::Invalid(_))));
    }

    #[test]
    fn first_line_must_be_a_header() {
        let text = r#"{"dungeonPreseed": 1, "frame": 4}"#;
        assert!(matches!(
            read_trace(text.as_bytes()),
            Err(TraceError::MissingHeader)
        ));
    }

    #[test]
    fn parses_input_lines() {
        let line = parse_input_line("20 A right").expect("valid");
        assert_eq!(
            line,
            InputLine {
                frame: 20,
                keys: vec!["A".to_owned(), "RIGHT".to_owned()]
            }
        );
        assert_eq!(
            parse_input_line("40 -").expect("release").keys,
            Vec::<String>::new()
        );
        assert!(parse_input_line("40 JUMP").is_err());
        assert!(parse_input_line("40").is_err());
        assert!(parse_input_line("x A").is_err());
    }
}
