//! The CLI's exit codes on hand-written v2 traces (no game data).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use rescue_nds::rom::SUPPORTED_DUMPS;
use rescue_sim::rng::DungeonPreseed;
use rescue_trace::{
    Action, Emulator, Entity, Header, InitialSeed, Inputs, Rom, SCHEMA_VERSION, TurnRow,
};

const RUN_SEED: u32 = 0x00FF_80BA;

fn action(id: u16, direction: u8, param: u8, slot: u8) -> Action {
    Action {
        id,
        direction,
        param,
        slot,
    }
}

fn row(actions: Vec<Action>) -> TurnRow {
    let mut preseed = DungeonPreseed::new(RUN_SEED);
    let floor_seed = preseed.next_floor_seed();
    TurnRow {
        fractional_turn: 0,
        run_seed: RUN_SEED,
        floor_seed,
        s_prng_state: 0x1234,
        dungeon_rng: 0x00AB_CDEF,
        dungeon_preseed: preseed.state(),
        team: [
            Some(Entity {
                kind: 1,
                x: 10,
                y: 4,
                species: 2,
                level: 7,
                hp: 20,
                max_hp: 30,
                speed_stage: 6,
            }),
            None,
            None,
            None,
        ],
        wild: Vec::new(),
        tiles_sha256: "0123456789abcdef".to_owned(),
        frame: 1,
        actions,
    }
}

/// Writes a v2 trace with `rows` into the test temp dir and runs the CLI on it.
fn run_cli(name: &str, rows: &[TurnRow]) -> Output {
    let header = Header {
        schema_version: SCHEMA_VERSION,
        rom: Rom {
            sha1: SUPPORTED_DUMPS[0].sha1.to_string(),
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
            boot: Vec::new(),
            turn: Vec::new(),
        },
    };
    let mut text = serde_json::to_string(&header).expect("header");
    text.push('\n');
    for r in rows {
        text.push_str(&serde_json::to_string(r).expect("row"));
        text.push('\n');
    }
    let path: PathBuf = [env!("CARGO_TARGET_TMPDIR"), name].iter().collect();
    fs::write(&path, text).expect("write trace");
    Command::new(env!("CARGO_BIN_EXE_rescue-trace-harness"))
        .arg(&path)
        .output()
        .expect("run cli")
}

#[test]
fn waits_match_and_exit_zero() {
    let out = run_cli("waits.jsonl", &[row(vec![action(1, 0, 0, 0)]), row(vec![])]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("match: 2 rows"));
}

#[test]
fn a_not_modelled_command_exits_one_with_its_row() {
    let out = run_cli(
        "attack.jsonl",
        &[row(vec![]), row(vec![action(0x32, 2, 0, 0)])],
    );
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("sim gap at row 2: Attack"), "{stdout}");
}

#[test]
fn an_unmapped_action_exits_two() {
    let out = run_cli("unknown.jsonl", &[row(vec![action(0x13, 0, 0, 0)])]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("row 1, action 1"), "{stderr}");
}
