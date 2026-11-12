//! Replay tests on hand-written rows (no game data). The start state has an empty floor, so a
//! step is refused; the tests use that to check what a refused step does to the replay.

use super::*;
use rescue_sim::rng::DungeonPreseed;
use rescue_trace::Action;

const RUN_SEED: u32 = 0x00FF_80BA;

/// The replay of `rows`, which must not fail.
fn ok(rows: &[TurnRow]) -> Replay {
    replay(rows).expect("replay")
}

fn action(id: u16, direction: u8, param: u8, slot: u8) -> Action {
    Action {
        id,
        direction,
        param,
        slot,
    }
}

/// A row on the seed chain of `RUN_SEED` after one floor, with the leader at `x`.
fn row(x: i16, frame: u64, actions: Vec<Action>) -> TurnRow {
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
                x,
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
        frame,
        actions,
    }
}

#[test]
fn empty_trace_matches() {
    assert_eq!(ok(&[]), Replay::Match { rows: 0 });
}

#[test]
fn a_wait_leaves_the_rows_matching() {
    let rows = vec![row(10, 1, vec![action(1, 0, 0, 0)]), row(10, 2, vec![])];
    assert_eq!(ok(&rows), Replay::Match { rows: 2 });
}

#[test]
fn a_refused_move_is_a_divergence_at_the_next_row() {
    let rows = vec![row(10, 1, vec![action(2, 2, 1, 0)]), row(11, 2, vec![])];
    match replay(&rows) {
        Ok(Replay::Divergence(d)) => {
            assert_eq!(d.row, 2);
            assert_eq!(d.frame, 2);
            assert_eq!(d.diffs[0].path, "team[0].x");
            assert_eq!(d.diffs[0].expected, "11");
            assert_eq!(d.diffs[0].actual, "10");
        }
        other => panic!("expected a divergence at row 2, got {other:?}"),
    }
}

#[test]
fn a_run_of_records_is_applied_before_the_next_row() {
    // Three run records (param 0 after the first step): all refused, so the leader stays at
    // x 10 and the next row's expected x 13 is the divergence.
    let run = vec![action(2, 2, 1, 0), action(2, 2, 0, 0), action(2, 2, 0, 0)];
    let rows = vec![row(10, 1, run), row(13, 2, vec![])];
    match replay(&rows) {
        Ok(Replay::Divergence(d)) => {
            assert_eq!(d.row, 2);
            assert_eq!(d.diffs[0].expected, "13");
            assert_eq!(d.diffs[0].actual, "10");
        }
        other => panic!("expected a divergence at row 2, got {other:?}"),
    }
}

#[test]
fn a_not_modelled_command_is_a_sim_gap_with_its_row() {
    let rows = vec![row(10, 1, vec![]), row(10, 2, vec![action(0x32, 2, 0, 0)])];
    assert_eq!(
        ok(&rows),
        Replay::SimGap {
            row: 2,
            command: "Attack",
        }
    );
    assert_eq!(
        render_sim_gap(2, "Attack"),
        "sim gap at row 2: Attack is not modelled by the sim yet\n"
    );
}

#[test]
fn an_unmapped_action_is_a_typed_error_with_its_position() {
    let rows = vec![row(10, 1, vec![action(1, 0, 0, 0), action(0x13, 0, 0, 0)])];
    match replay(&rows) {
        Err(HarnessError::Action { row, index, source }) => {
            assert_eq!((row, index), (1, 2));
            assert!(matches!(source, ActionError::NoCommand { id: 0x13, .. }));
        }
        other => panic!("expected an action error, got {other:?}"),
    }
}
