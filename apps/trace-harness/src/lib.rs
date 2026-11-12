//! Replays golden-trace rows against `rescue_sim` snapshots and reports the first row that
//! differs. The comparison is here, not in `main.rs`, so tests can call it without a process.
//!
//! The sim side comes in as one [`Snapshot`] per row (a JSON Lines file, `--sim`). That is the
//! stub until the start-state and input-mapping issues build the states from the trace.

#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, BufRead};

use rescue_nds::rom::KnownDump;
use rescue_sim::state::{Monster, Snapshot, TEAM_SLOTS};
use rescue_trace::{Entity, Header, TurnRow};

/// Row fields the comparison does not check, with the reason. `render_divergence` does not
/// print these; the CLI prints the count on a match.
pub const SKIPPED_FIELDS: &[(&str, &str)] = &[
    (
        "sPRNGState",
        "global LCG; whether display code also steps it is open in #351",
    ),
    (
        "dungeonPreseed",
        "the preseed generator is not in the snapshot (restore rebuilds it from floorsEntered)",
    ),
    (
        "turn counter",
        "the sim counts commands that take a turn; fractionalTurn is the game's 0-23 cycle, a different unit",
    ),
    (
        "runSeed",
        "in the snapshot as runSeed, but not in #331's comparison list",
    ),
    (
        "floorSeed",
        "the snapshot keeps the floor generator's current state, not the seed it started from",
    ),
    (
        "fractionalTurn",
        "game turn fraction (0-23); the sim has no field for it yet",
    ),
    ("frame", "capture-run frame counter, not game state"),
    (
        "tilesSha256",
        "a hash over the tile grid; the comparison is per field",
    ),
    ("type", "entity kind; the snapshot does not store it"),
    (
        "speedStage",
        "speed stage; the snapshot's entity info leaves it out",
    ),
];

/// Why a harness run cannot compare the files it was given.
#[derive(Debug)]
pub enum HarnessError {
    Io(io::Error),
    /// The ROM SHA-1 is not one of the supported dumps.
    UnknownRom {
        sha1: String,
    },
    /// A line of the sim-states file is not a snapshot.
    BadStates {
        line: usize,
        source: serde_json::Error,
    },
    /// The sim-states file does not hold one snapshot per row.
    StateCount {
        rows: usize,
        states: usize,
    },
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::UnknownRom { sha1 } => write!(
                f,
                "trace is for ROM SHA-1 {sha1}, which is not a supported dump (see docs/supported-dumps.md)"
            ),
            Self::BadStates { line, source } => write!(f, "sim states, line {line}: {source}"),
            Self::StateCount { rows, states } => write!(
                f,
                "trace has {rows} turn rows but the sim-states file has {states} snapshots"
            ),
        }
    }
}

impl std::error::Error for HarnessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::BadStates { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// One field that differs. `path` names it in the row, for example `team[0].hp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    pub path: String,
    /// The value in the trace (retail).
    pub expected: String,
    /// The value in the sim.
    pub actual: String,
}

/// The first row that differs from its sim state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// 1-based turn row number (the header is not counted).
    pub row: usize,
    pub frame: u64,
    pub fractional_turn: i16,
    pub diffs: Vec<FieldDiff>,
}

/// Compares one row with one sim snapshot and returns every field that differs.
/// Fields in [`SKIPPED_FIELDS`] are not compared.
#[must_use]
pub fn compare_row(row: &TurnRow, state: &Snapshot) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    let Some(dungeon) = &state.dungeon else {
        push(&mut diffs, "dungeon", "present", "absent");
        return diffs;
    };
    check(
        &mut diffs,
        "dungeonRng",
        i64::from(row.dungeon_rng),
        i64::from(dungeon.rng),
    );
    for slot in 0..dungeon.team.len().max(TEAM_SLOTS) {
        let expected = row.team.get(slot).copied().flatten();
        compare_monster(
            &mut diffs,
            &format!("team[{slot}]"),
            expected,
            dungeon.team.get(slot),
        );
    }
    for index in 0..dungeon.wild.len().max(row.wild.len()) {
        let expected = row.wild.get(index).copied();
        compare_monster(
            &mut diffs,
            &format!("wild[{index}]"),
            expected,
            dungeon.wild.get(index),
        );
    }
    diffs
}

/// Compares the rows in order and returns the first divergent one, or `None` on a match.
/// `states` must hold one snapshot per row.
pub fn diff_trace(
    rows: &[TurnRow],
    states: &[Snapshot],
) -> Result<Option<Divergence>, HarnessError> {
    if rows.len() != states.len() {
        return Err(HarnessError::StateCount {
            rows: rows.len(),
            states: states.len(),
        });
    }
    for (index, (row, state)) in rows.iter().zip(states).enumerate() {
        let diffs = compare_row(row, state);
        if !diffs.is_empty() {
            return Ok(Some(Divergence {
                row: index + 1,
                frame: row.frame,
                fractional_turn: row.fractional_turn,
                diffs,
            }));
        }
    }
    Ok(None)
}

/// Checks the header's ROM against the supported dumps.
pub fn check_rom(header: &Header, supported: &[KnownDump]) -> Result<(), HarnessError> {
    if supported
        .iter()
        .any(|d| d.sha1.to_string() == header.rom.sha1)
    {
        Ok(())
    } else {
        Err(HarnessError::UnknownRom {
            sha1: header.rom.sha1.clone(),
        })
    }
}

/// Reads one snapshot per line. Blank lines are skipped.
pub fn read_states<R: BufRead>(reader: R) -> Result<Vec<Snapshot>, HarnessError> {
    let mut states = Vec::new();
    for (index, text) in reader.lines().enumerate() {
        let text = text.map_err(HarnessError::Io)?;
        if text.trim().is_empty() {
            continue;
        }
        let state = serde_json::from_str(&text).map_err(|source| HarnessError::BadStates {
            line: index + 1,
            source,
        })?;
        states.push(state);
    }
    Ok(states)
}

/// The report for a divergence: the row, then one line per differing field.
#[must_use]
pub fn render_divergence(divergence: &Divergence) -> String {
    let mut out = format!(
        "divergence at row {} (frame {}, fractionalTurn {})\n",
        divergence.row, divergence.frame, divergence.fractional_turn
    );
    for diff in &divergence.diffs {
        out.push_str(&format!(
            "  {}: expected {}, actual {}\n",
            diff.path, diff.expected, diff.actual
        ));
    }
    out
}

fn compare_monster(
    diffs: &mut Vec<FieldDiff>,
    path: &str,
    expected: Option<Entity>,
    actual: Option<&Monster>,
) {
    match (expected, actual) {
        (None, None) => {}
        (Some(_), None) => push(diffs, path, "present", "absent"),
        (None, Some(_)) => push(diffs, path, "absent", "present"),
        (Some(e), Some(m)) => {
            check(
                diffs,
                &format!("{path}.x"),
                i64::from(e.x),
                i64::from(m.pos.0),
            );
            check(
                diffs,
                &format!("{path}.y"),
                i64::from(e.y),
                i64::from(m.pos.1),
            );
            check(
                diffs,
                &format!("{path}.species"),
                i64::from(e.species),
                i64::from(m.info.species),
            );
            check(
                diffs,
                &format!("{path}.level"),
                i64::from(e.level),
                i64::from(m.info.level),
            );
            check(
                diffs,
                &format!("{path}.hp"),
                i64::from(e.hp),
                i64::from(m.info.hp),
            );
            check(
                diffs,
                &format!("{path}.maxHp"),
                i64::from(e.max_hp),
                i64::from(m.info.max_hp),
            );
        }
    }
}

fn check(diffs: &mut Vec<FieldDiff>, path: &str, expected: i64, actual: i64) {
    if expected != actual {
        push(diffs, path, &expected.to_string(), &actual.to_string());
    }
}

fn push(diffs: &mut Vec<FieldDiff>, path: &str, expected: &str, actual: &str) {
    diffs.push(FieldDiff {
        path: path.to_owned(),
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_nds::rom::SUPPORTED_DUMPS;
    use rescue_sim::state::{DungeonSnapshot, EntityInfo, FloorSnapshot};
    use rescue_trace::{Emulator, InitialSeed, Inputs, Rom, SCHEMA_VERSION};

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
                bytes: "0011223344aa".to_owned(),
            },
            inputs: Inputs {
                boot: vec!["0 -".to_owned()],
                turn: vec!["0 A".to_owned()],
            },
        }
    }

    fn entity(x: i16, hp: i16) -> Entity {
        Entity {
            kind: 1,
            x,
            y: -12,
            species: 2,
            level: 5,
            hp,
            max_hp: 25,
            speed_stage: 6,
        }
    }

    fn row(frame: u64, hp: i16) -> TurnRow {
        TurnRow {
            fractional_turn: 3,
            run_seed: 123_456,
            floor_seed: 654_321,
            s_prng_state: 7,
            dungeon_rng: 0xAAAA_AAAA,
            dungeon_preseed: 0x5555_5555,
            team: [Some(entity(10, hp)), None, None, None],
            wild: vec![entity(4, 9)],
            tiles_sha256: "0123456789abcdef".to_owned(),
            frame,
        }
    }

    fn monster(e: Entity) -> Monster {
        Monster {
            pos: (e.x, e.y),
            visible: true,
            info: EntityInfo {
                species: e.species as u16,
                apparent_species: e.species as u16,
                not_team_member: false,
                is_team_leader: false,
                level: e.level,
                iq: 0,
                hp: e.hp,
                max_hp: e.max_hp,
                attack: [10, 10],
                defence: [5, 5],
                offensive_stages: [0, 0],
                direction: 0,
                belly: 0,
            },
        }
    }

    /// The snapshot that matches `row` exactly.
    fn state_for(row: &TurnRow) -> Snapshot {
        Snapshot {
            global: 7,
            run_seed: 123_456,
            floors_entered: 1,
            dungeon: Some(DungeonSnapshot {
                rng: row.dungeon_rng,
                floor: FloorSnapshot {
                    tiles: Vec::new(),
                    stairs: None,
                },
                team: row.team.iter().flatten().copied().map(monster).collect(),
                wild: row.wild.iter().copied().map(monster).collect(),
                turn: 0,
                shop_pass: false,
                monster_house_room: 0xFF,
                player_spawn: None,
                stairs_spawn: None,
            }),
        }
    }

    #[test]
    fn matching_rows_have_no_diff() {
        let rows = vec![row(1, 20), row(2, 19)];
        let states: Vec<Snapshot> = rows.iter().map(state_for).collect();
        assert!(compare_row(&rows[0], &states[0]).is_empty());
        assert_eq!(diff_trace(&rows, &states).expect("same count"), None);
    }

    #[test]
    fn one_edited_field_is_the_first_divergence() {
        let rows = vec![row(1, 20), row(2, 19)];
        let mut states: Vec<Snapshot> = rows.iter().map(state_for).collect();
        // The sim says the leader has 18 HP on the second row.
        states[1].dungeon.as_mut().expect("dungeon").team[0].info.hp = 18;
        let divergence = diff_trace(&rows, &states)
            .expect("same count")
            .expect("row 2 differs");
        assert_eq!(divergence.row, 2);
        assert_eq!(divergence.frame, 2);
        assert_eq!(divergence.fractional_turn, 3);
        assert_eq!(
            divergence.diffs,
            vec![FieldDiff {
                path: "team[0].hp".to_owned(),
                expected: "19".to_owned(),
                actual: "18".to_owned(),
            }]
        );
        let text = render_divergence(&divergence);
        assert!(text.starts_with("divergence at row 2 (frame 2, fractionalTurn 3)\n"));
        assert!(text.contains("  team[0].hp: expected 19, actual 18\n"));
    }

    #[test]
    fn missing_wild_monster_and_rng_are_reported_by_path() {
        let rows = [row(1, 20)];
        let mut state = state_for(&rows[0]);
        let dungeon = state.dungeon.as_mut().expect("dungeon");
        dungeon.wild.clear();
        dungeon.rng ^= 1;
        let diffs = compare_row(&rows[0], &state);
        assert_eq!(
            diffs,
            vec![
                FieldDiff {
                    path: "dungeonRng".to_owned(),
                    expected: 0xAAAA_AAAA_i64.to_string(),
                    actual: (0xAAAA_AAAA_i64 ^ 1).to_string(),
                },
                FieldDiff {
                    path: "wild[0]".to_owned(),
                    expected: "present".to_owned(),
                    actual: "absent".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn state_count_must_match_row_count() {
        let rows = vec![row(1, 20), row(2, 19)];
        let states = vec![state_for(&rows[0])];
        assert!(matches!(
            diff_trace(&rows, &states),
            Err(HarnessError::StateCount { rows: 2, states: 1 })
        ));
    }

    #[test]
    fn bad_header_is_refused() {
        // Unknown ROM SHA-1.
        assert!(check_rom(&header(), SUPPORTED_DUMPS).is_err());
        let mut known = header();
        known.rom.sha1 = SUPPORTED_DUMPS[0].sha1.to_string();
        assert!(check_rom(&known, SUPPORTED_DUMPS).is_ok());
        // Wrong schema version.
        let mut value = serde_json::to_value(header()).expect("to_value");
        value["schemaVersion"] = serde_json::json!(2);
        let text = format!("{value}\n");
        assert!(matches!(
            rescue_trace::read_trace(text.as_bytes()),
            Err(rescue_trace::TraceError::UnsupportedSchema(2))
        ));
    }

    #[test]
    fn states_file_round_trips_and_bad_line_is_located() {
        let rows = [row(1, 20)];
        let state = state_for(&rows[0]);
        let good = serde_json::to_string(&state).expect("encode");
        let text = format!("{good}\n\n{{not json\n");
        match read_states(text.as_bytes()) {
            Err(HarnessError::BadStates { line, .. }) => assert_eq!(line, 3),
            other => panic!("expected BadStates on line 3, got {other:?}"),
        }
        let back = read_states(format!("{good}\n").as_bytes()).expect("one snapshot");
        assert_eq!(back, vec![state]);
    }
}
