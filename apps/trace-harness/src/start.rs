//! Builds the sim's starting state from a trace's first row (#332, P1 and P2 of the
//! investigation). The header is not needed: the row carries the generator states.
//!
//! The sim's team is a `Vec` with no empty slots, but the trace keeps four slots, `null`
//! where none is set. [`team_index`] maps each trace slot to its place in the sim's list, so
//! a gap in the slots is not read as a divergence.

use std::fmt;

use rescue_sim::floor::{HEIGHT, Tile, WIDTH};
use rescue_sim::rng::DungeonPreseed;
use rescue_sim::state::{
    DungeonSnapshot, EntityInfo, FloorSnapshot, GameState, MAX_RESTORED_FLOORS, Monster,
    NO_MONSTER_HOUSE, Snapshot, StateError, TEAM_SLOTS, TileSnapshot,
};
use rescue_trace::{Entity, TurnRow};

/// Row fields [`state_from_row`] does not carry, so no comparison can check them. The
/// trace's `tilesSha256` is deferred (see the sub-issue); `type` and `speedStage` have no
/// slot in the sim's entity; `fractionalTurn` and `frame` are the game's turn fraction and
/// the capture run's counter.
pub const NOT_CARRIED: &[&str] = &[
    "fractionalTurn",
    "frame",
    "tilesSha256",
    "type",
    "speedStage",
];

/// Why a starting state cannot be built from a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowError {
    /// No step of the preseed chain from `runSeed`, within [`MAX_RESTORED_FLOORS`], gives
    /// both the row's `floorSeed` and its `dungeonPreseed`. The row is not on the chain
    /// (for example, a resume from a save).
    NotOnSeedChain {
        floor_seed: u32,
        dungeon_preseed: u32,
    },
    /// A value the sim cannot hold, named by its row path (for example `team[0].species`).
    Unrepresentable { field: String, value: i64 },
    /// The sim refused the state built from the row.
    State(StateError),
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOnSeedChain {
                floor_seed,
                dungeon_preseed,
            } => write!(
                f,
                "row is not on its run's seed chain: no floor count gives floorSeed {floor_seed:#x} and dungeonPreseed {dungeon_preseed:#x}"
            ),
            Self::Unrepresentable { field, value } => {
                write!(f, "{field} = {value} cannot be represented in the sim")
            }
            Self::State(e) => write!(f, "sim refused the state: {e:?}"),
        }
    }
}

impl std::error::Error for RowError {}

/// Where each trace team slot sits in the sim's team list. The sim keeps only the occupied
/// slots, in slot order, so a slot's index is the number of occupied slots before it.
#[must_use]
pub fn team_index(team: &[Option<Entity>; TEAM_SLOTS]) -> [Option<usize>; TEAM_SLOTS] {
    let mut next = 0;
    let mut out = [None; TEAM_SLOTS];
    for (slot, entity) in team.iter().enumerate() {
        if entity.is_some() {
            out[slot] = Some(next);
            next += 1;
        }
    }
    out
}

/// The sim state at the row's turn: the generators from the row, the floor count found on
/// the preseed chain, and the monsters. The floor's tiles are left empty (see the shortcut
/// in the body).
pub fn state_from_row(row: &TurnRow) -> Result<GameState, RowError> {
    let floors_entered = floors_on_chain(row)?;
    let mut team = Vec::new();
    for (slot, entity) in row.team.iter().enumerate() {
        if let Some(entity) = entity {
            // Slot 0 is the leader (`Dungeon::team`).
            team.push(monster(entity, &format!("team[{slot}]"), false, slot == 0)?);
        }
    }
    let wild = row
        .wild
        .iter()
        .enumerate()
        .map(|(index, entity)| monster(entity, &format!("wild[{index}]"), true, false))
        .collect::<Result<Vec<_>, _>>()?;
    // shortcut: the floor is left empty, so the floor-dependent fields (tilesSha256, the
    // walkable masks) are not reproduced; the floor is generated from floorSeed with the
    // dungeon's FloorProperties later (P2 of #332, needs the dungeon and floor ids in the
    // trace or scenario). Upgrade when a captured row is compared with a generated floor.
    let tiles = vec![TileSnapshot::from(Tile::EMPTY); WIDTH * HEIGHT];
    let snapshot = Snapshot {
        global: row.s_prng_state,
        run_seed: row.run_seed,
        floors_entered,
        dungeon: Some(DungeonSnapshot {
            rng: row.dungeon_rng,
            floor: FloorSnapshot {
                tiles,
                stairs: None,
            },
            team,
            wild,
            turn: 0,
            shop_pass: false,
            monster_house_room: NO_MONSTER_HOUSE,
            player_spawn: None,
            stairs_spawn: None,
        }),
    };
    GameState::restore(&snapshot).map_err(RowError::State)
}

/// The floor count `n` such that `n` steps of the preseed chain from `runSeed` give the row's
/// `floorSeed` (the n-th floor seed) and `dungeonPreseed` (the state after step n).
fn floors_on_chain(row: &TurnRow) -> Result<u32, RowError> {
    let mut preseed = DungeonPreseed::new(row.run_seed);
    for floors in 1..=MAX_RESTORED_FLOORS {
        let floor_seed = preseed.next_floor_seed();
        if floor_seed == row.floor_seed && preseed.state() == row.dungeon_preseed {
            return Ok(floors);
        }
    }
    Err(RowError::NotOnSeedChain {
        floor_seed: row.floor_seed,
        dungeon_preseed: row.dungeon_preseed,
    })
}

/// One monster from a trace entity. Fields the row does not carry take their zero value
/// (`attack`, `defence`, `iq`, `direction`, `belly`), and `visible` is set on.
fn monster(
    entity: &Entity,
    path: &str,
    not_team_member: bool,
    is_team_leader: bool,
) -> Result<Monster, RowError> {
    let species = u16::try_from(entity.species).map_err(|_| RowError::Unrepresentable {
        field: format!("{path}.species"),
        value: i64::from(entity.species),
    })?;
    Ok(Monster {
        pos: (entity.x, entity.y),
        visible: true,
        info: EntityInfo {
            species,
            apparent_species: species,
            not_team_member,
            is_team_leader,
            level: entity.level,
            iq: 0,
            hp: entity.hp,
            max_hp: entity.max_hp,
            attack: [0, 0],
            defence: [0, 0],
            offensive_stages: [0, 0],
            direction: 0,
            belly: 0,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare_row;

    /// The row a run would show after `floors` floors: the floor seed and preseed come from
    /// the chain, the rest is arbitrary.
    fn row_after(
        run_seed: u32,
        floors: u32,
        team: [Option<Entity>; 4],
        wild: Vec<Entity>,
    ) -> TurnRow {
        let mut preseed = DungeonPreseed::new(run_seed);
        let mut floor_seed = 0;
        for _ in 0..floors {
            floor_seed = preseed.next_floor_seed();
        }
        TurnRow {
            fractional_turn: 3,
            run_seed,
            floor_seed,
            s_prng_state: 0x1234,
            dungeon_rng: 0x00AB_CDEF,
            dungeon_preseed: preseed.state(),
            team,
            wild,
            tiles_sha256: "0123456789abcdef".to_owned(),
            frame: 99,
        }
    }

    fn entity(x: i16, species: i16, hp: i16) -> Entity {
        Entity {
            kind: 1,
            x,
            y: 4,
            species,
            level: 7,
            hp,
            max_hp: 30,
            speed_stage: 6,
        }
    }

    #[test]
    fn floor_count_is_found_on_the_chain() {
        let row = row_after(
            0x00FF_80BA,
            3,
            [Some(entity(1, 2, 20)), None, None, None],
            vec![],
        );
        let state = state_from_row(&row).expect("row 3 is on the chain");
        assert_eq!(state.floors_entered(), 3);
        assert_eq!(state.run_seed(), 0x00FF_80BA);
        assert_eq!(state.global().state(), 0x1234);
    }

    #[test]
    fn start_state_compares_equal_to_its_row() {
        let row = row_after(
            0x00FF_80BA,
            2,
            [Some(entity(10, 2, 20)), None, Some(entity(11, 3, 9)), None],
            vec![entity(4, 5, 8)],
        );
        let state = state_from_row(&row).expect("row on the chain");
        let snapshot = state.snapshot();
        // Every field the start state carries; tilesSha256 is deferred and not checked.
        assert_eq!(snapshot.global, row.s_prng_state);
        assert_eq!(snapshot.run_seed, row.run_seed);
        assert_eq!(
            snapshot.dungeon.as_ref().expect("dungeon").rng,
            row.dungeon_rng
        );
        assert_eq!(compare_row(&row, &snapshot), Vec::new());
    }

    #[test]
    fn a_gap_in_the_team_slots_is_not_a_divergence() {
        // Slot 1 empty, slot 2 set: the sim holds two members, so slot 2 is sim index 1.
        let row = row_after(
            0x00FF_80BA,
            1,
            [Some(entity(10, 2, 20)), None, Some(entity(12, 3, 9)), None],
            vec![],
        );
        let snapshot = state_from_row(&row).expect("row on the chain").snapshot();
        let team = &snapshot.dungeon.as_ref().expect("dungeon").team;
        assert_eq!(team.len(), 2);
        assert_eq!(team[1].pos, (12, 4));
        assert!(compare_row(&row, &snapshot).is_empty());
    }

    #[test]
    fn a_row_off_the_chain_is_refused_by_name() {
        let mut row = row_after(0x00FF_80BA, 2, [None; 4], vec![]);
        row.dungeon_preseed ^= 2;
        assert_eq!(
            state_from_row(&row),
            Err(RowError::NotOnSeedChain {
                floor_seed: row.floor_seed,
                dungeon_preseed: row.dungeon_preseed,
            })
        );
        let message = state_from_row(&row).expect_err("off chain").to_string();
        assert!(message.contains("dungeonPreseed"), "{message}");
    }

    #[test]
    fn a_species_the_sim_cannot_hold_is_named() {
        let row = row_after(0x00FF_80BA, 1, [None; 4], vec![entity(4, -1, 5)]);
        assert_eq!(
            state_from_row(&row),
            Err(RowError::Unrepresentable {
                field: "wild[0].species".to_owned(),
                value: -1,
            })
        );
    }

    #[test]
    fn too_many_wild_monsters_is_refused() {
        let wild = vec![entity(1, 2, 3); 17];
        let row = row_after(0x00FF_80BA, 1, [None; 4], wild);
        assert_eq!(
            state_from_row(&row),
            Err(RowError::State(StateError::WildTooLarge(17)))
        );
    }
}
