//! Replay tests on hand-written rows (no game data). The start state has an empty floor, so a
//! step is refused; the tests use that to check what a refused step does to the replay.

use super::*;
use rescue_sim::command::Command;
use rescue_sim::command::Direction;
use rescue_sim::floor::TERRAIN_FLOOR;
use rescue_sim::items::{FloorItem, ItemsSnapshot, POKE, SwapSource};
use rescue_sim::rng::{DungeonPreseed, DungeonRng};
use rescue_sim::state::Snapshot;
use rescue_trace::Action;

const RUN_SEED: u32 = 0x00FF_80BA;
const RNG_AT_ROW: u32 = 0x00AB_CDEF;

/// The replay of `rows`, which must not fail.
fn ok(rows: &[TurnRow]) -> Replay {
    replay(rows, None).expect("replay")
}

/// A stage-1 row on fractional turn `fractional_turn`, with a full-HP leader (the regeneration
/// tick then changes nothing) and the given dungeon RNG state.
fn stage_one_row(
    fractional_turn: i16,
    frame: u64,
    dungeon_rng: u32,
    actions: Vec<Action>,
) -> TurnRow {
    let mut r = row(10, frame, actions);
    r.fractional_turn = fractional_turn;
    r.dungeon_rng = dungeon_rng;
    if let Some(leader) = r.team[0].as_mut() {
        leader.hp = 30;
        leader.speed_stage = 1;
    }
    r
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
        items: None,
    }
}

#[test]
fn empty_trace_matches() {
    assert_eq!(ok(&[]), Replay::Match { rows: 0 });
}

#[test]
fn a_wait_leaves_the_rows_matching() {
    // The leader is at speed stage 4 (every turn is a gate), so the wait leaves the clock on 1.
    let mut second = row(10, 2, vec![]);
    second.fractional_turn = 1;
    let rows = vec![row(10, 1, vec![action(1, 0, 0, 0)]), second];
    assert_eq!(ok(&rows), Replay::Match { rows: 2 });
}

#[test]
fn a_stage_one_wait_is_compared_at_the_next_gate_only() {
    // Stage 1 acts on fractional turns 3, 7, 11, ... A wait at 3 runs the leader's end-of-turn at
    // the weather step of turn 6, so the game's row at 7 holds one Shed Skin roll. Rows 4 to 6
    // show the game before that roll, with the RNG still unchanged, so they are skipped.
    let mut stepped = DungeonRng::from_state(RNG_AT_ROW);
    stepped.int(100);
    let rows = vec![
        stage_one_row(3, 1, RNG_AT_ROW, vec![action(1, 0, 0, 0)]),
        stage_one_row(4, 2, RNG_AT_ROW, vec![]),
        stage_one_row(5, 3, RNG_AT_ROW, vec![]),
        stage_one_row(6, 4, RNG_AT_ROW, vec![]),
        stage_one_row(7, 5, stepped.state(), vec![]),
    ];
    assert_eq!(ok(&rows), Replay::Match { rows: 5 });
}

#[test]
fn a_gate_row_on_another_clock_is_a_fractional_turn_divergence() {
    // The wait at 3 moves the sim's clock to its next gate, 7. Row 2 is on 11, also a gate of
    // stage 1, so it is compared and its clock is the divergence.
    let rows = vec![
        stage_one_row(3, 1, RNG_AT_ROW, vec![action(1, 0, 0, 0)]),
        stage_one_row(11, 2, RNG_AT_ROW, vec![]),
    ];
    match replay(&rows, None) {
        Ok(Replay::Divergence(d)) => {
            assert_eq!((d.row, d.frame, d.fractional_turn), (2, 2, 11));
            assert_eq!(d.diffs[0].path, "fractionalTurn");
            assert_eq!(d.diffs[0].expected, "11");
            assert_eq!(d.diffs[0].actual, "7");
        }
        other => panic!("expected a clock divergence at row 2, got {other:?}"),
    }
}

#[test]
fn a_gate_row_before_any_command_is_compared_not_a_clock_divergence() {
    // The sim starts on fractional turn 0 and has not run to its first gate (3). No command has
    // applied yet, so the gate row at 3 is compared: its fields match, so the replay matches.
    let rows = vec![
        stage_one_row(0, 1, RNG_AT_ROW, vec![]),
        stage_one_row(3, 2, RNG_AT_ROW, vec![]),
    ];
    assert_eq!(ok(&rows), Replay::Match { rows: 2 });
}

#[test]
fn a_gate_row_before_any_command_is_still_compared_field_by_field() {
    // The same start, but the game's RNG at the gate differs from the sim's: the row is
    // compared, so the RNG is the divergence (not a clock).
    let rows = vec![
        stage_one_row(0, 1, RNG_AT_ROW, vec![]),
        stage_one_row(3, 2, RNG_AT_ROW + 1, vec![]),
    ];
    match replay(&rows, None) {
        Ok(Replay::Divergence(d)) => {
            assert_eq!((d.row, d.fractional_turn), (2, 3));
            assert_eq!(d.diffs[0].path, "dungeonRng");
        }
        other => panic!("expected an RNG divergence at row 2, got {other:?}"),
    }
}

#[test]
fn leader_scope_ignores_fields_that_are_not_the_leader() {
    // Only the RNG differs on row 2: the all-field replay diverges there, the leader scope
    // matches.
    let rows = vec![
        stage_one_row(0, 1, RNG_AT_ROW, vec![]),
        stage_one_row(0, 2, RNG_AT_ROW + 1, vec![]),
    ];
    assert!(matches!(replay(&rows, None), Ok(Replay::Divergence(d)) if d.row == 2));
    assert_eq!(
        replay_with(&rows, None, Scope::Leader).expect("replay"),
        Replay::Match { rows: 2 }
    );
}

#[test]
fn leader_scope_still_reports_the_leader() {
    // A refused move leaves the leader on x 10, so the leader scope diverges on team[0].x.
    let rows = vec![row(10, 1, vec![action(2, 2, 1, 0)]), row(11, 2, vec![])];
    match replay_with(&rows, None, Scope::Leader) {
        Ok(Replay::Divergence(d)) => {
            assert_eq!(d.row, 2);
            assert_eq!(d.diffs[0].path, "team[0].x");
        }
        other => panic!("expected a leader divergence at row 2, got {other:?}"),
    }
}

#[test]
fn a_refused_move_is_a_divergence_at_the_next_row() {
    let rows = vec![row(10, 1, vec![action(2, 2, 1, 0)]), row(11, 2, vec![])];
    match replay(&rows, None) {
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
    match replay(&rows, None) {
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
    match replay(&rows, None) {
        Err(HarnessError::Action { row, index, source }) => {
            assert_eq!((row, index), (1, 2));
            assert!(matches!(source, ActionError::NoCommand { id: 0x13, .. }));
        }
        other => panic!("expected an action error, got {other:?}"),
    }
}

/// The leader's action record of one command (ids as the game writes them, see
/// `docs/golden-traces.md`): a step right, a drop of bag slot `param - 1`, a swap of the floor
/// item underfoot (`param` 0x80) with the source in `slot`.
fn step_right() -> Action {
    action(2, 2, 1, 0)
}

fn drop_slot(slot: u8) -> Action {
    action(0x08, 0, slot + 1, 0)
}

fn swap_with(source: u8) -> Action {
    action(0x0A, 0, 0x80, source)
}

fn item_row(item: Item) -> Option<ItemRow> {
    item.exists().then_some(ItemRow {
        flags: item.flags,
        qty: item.qty,
        id: item.id,
    })
}

/// The item state of a snapshot, as a trace row carries it.
fn items_row(items: &ItemsSnapshot) -> ItemsRow {
    ItemsRow {
        money: items.bag.money,
        bag: items.bag.slots.iter().map(|&i| item_row(i)).collect(),
        held: items.held.iter().map(|&i| item_row(i)).collect(),
        floor: items
            .floor
            .iter()
            .map(|f| {
                f.map(|f| FloorItemRow {
                    x: f.pos.0,
                    y: f.pos.1,
                    item: item_row(f.item).expect("a floor item exists"),
                })
            })
            .collect(),
        num_items: items.num_items,
    }
}

/// The row the game would write before `action`, taken from the sim's state at that moment.
fn row_of(snapshot: &Snapshot, actions: Vec<Action>, frame: u64) -> TurnRow {
    let d = snapshot.dungeon.as_ref().expect("a dungeon");
    let leader = &d.team[0];
    TurnRow {
        fractional_turn: i16::from(d.fractional_turn),
        run_seed: snapshot.run_seed,
        floor_seed: 0,
        s_prng_state: snapshot.global,
        dungeon_rng: d.rng,
        dungeon_preseed: 0,
        team: [
            Some(Entity {
                kind: 1,
                x: leader.pos.0,
                y: leader.pos.1,
                species: leader.info.species as i16,
                level: leader.info.level,
                hp: leader.info.hp,
                max_hp: leader.info.max_hp,
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
        items: Some(items_row(&d.items)),
    }
}

/// A floor of open ground with the leader at (5, 4), a bag item, a held item, a Poké of 7 at
/// (6, 4) and an item at (7, 4), all set through the snapshot (the sim has no other way in).
fn item_start() -> GameState {
    let mut snapshot = state_from_row(&row(5, 0, Vec::new()), None)
        .expect("start")
        .snapshot();
    let d = snapshot.dungeon.as_mut().expect("a dungeon");
    for tile in &mut d.floor.tiles {
        tile.terrain = TERRAIN_FLOOR;
    }
    d.team[0].info.hp = 30;
    d.team[0].info.max_hp = 30;
    d.items.bag.slots[0] = Item::new(0x20, 1);
    d.items.held[0] = Item::new(0x22, 1);
    d.items.floor[0] = Some(FloorItem {
        pos: (6, 4),
        item: Item::new(POKE, 7),
    });
    d.items.floor[1] = Some(FloorItem {
        pos: (7, 4),
        item: Item::new(0x21, 1),
    });
    GameState::restore(&snapshot).expect("restore")
}

/// Runs the script on `state` and records a row before each command, then one row after the
/// last. Returns the rows and the final state.
fn record(mut state: GameState, script: &[(Command, Action)]) -> (Vec<TurnRow>, GameState) {
    let mut rows = Vec::new();
    for (frame, (command, action)) in script.iter().enumerate() {
        rows.push(row_of(&state.snapshot(), vec![*action], frame as u64));
        assert_eq!(state.step(*command), Outcome::Applied, "{command:?}");
    }
    rows.push(row_of(&state.snapshot(), Vec::new(), script.len() as u64));
    (rows, state)
}

#[test]
fn item_trace_replays_pickup_drop_swap_and_money() {
    // Pickup: the Poké is money (7), the item at (7, 4) goes to the bag's second slot.
    // Drop: the leader stands on (7, 4), so the bag item goes onto that tile.
    // Swap: the item underfoot goes to the bag, then to the held slot.
    let script = [
        (Command::Move(Direction::Right), step_right()),
        (Command::Move(Direction::Right), step_right()),
        (Command::Drop { slot: 1 }, drop_slot(1)),
        (Command::Swap(SwapSource::Bag(0)), swap_with(1)),
        (Command::Swap(SwapSource::Held), swap_with(0x81)),
    ];
    let (rows, state) = record(item_start(), &script);

    let d = state.dungeon().expect("a dungeon");
    assert_eq!(d.bag().money, 7, "the Poké is money");
    assert_eq!(
        d.bag().slots[0],
        Item::new(0x21, 1),
        "the swapped-in bag item"
    );
    assert_eq!(
        d.held_item(0),
        Some(Item::new(0x20, 1)),
        "the floor item went to the held slot"
    );
    assert_eq!(
        d.floor_item_at((7, 4)),
        Some(Item::new(0x22, 1)),
        "the held item went down"
    );

    // The replay starts from the prepared state (the rows' seed chain is not checked here).
    match replay_from(item_start(), &rows, Scope::All, None).expect("replay") {
        Replay::Match { rows } => assert_eq!(rows, script.len() + 1),
        other => panic!("expected a match, got {other:?}"),
    }
}

#[test]
fn an_item_difference_is_a_divergence_in_the_items() {
    let script = [(Command::Move(Direction::Right), step_right())];
    let (mut rows, _) = record(item_start(), &script);
    rows[1].items.as_mut().expect("items").money = 99;
    match replay_from(item_start(), &rows, Scope::All, None).expect("replay") {
        Replay::Divergence(d) => {
            assert_eq!(d.row, 2);
            assert!(
                d.diffs
                    .iter()
                    .any(|diff| diff.path == "items.money" && diff.expected == "99"),
                "{:?}",
                d.diffs
            );
        }
        other => panic!("expected a divergence, got {other:?}"),
    }
}
