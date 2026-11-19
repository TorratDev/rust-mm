//! The EXP of a wild defeat at the values the #439 runtime probe read from the game
//! (`docs/re/combat.md`, "Defeat and experience"): `CalculateEXPGain` reads the defeated
//! monster's species and level, not the leader's. The yields are those the probe implied
//! (`020020F8` at level 1: species 290 gave 15, 191 gave 16, 16 gave 12), so the test needs no
//! ROM: it drives the sim through its public API with its own tables.

use rescue_data::combat::CombatTables;
use rescue_data::dungeon::MonsterSpawn;
use rescue_data::moves::Move;
use rescue_data::species::LevelData;
use rescue_data::types;
use rescue_sim::combat::{CombatData, REGULAR_ATTACK};
use rescue_sim::command::{Command, Direction, Outcome};
use rescue_sim::floor::{Floor, TERRAIN_FLOOR};
use rescue_sim::state::{EntityInfo, GameState, Monster, MoveSlot};

fn tables() -> CombatTables {
    let stage = |slope: i32, base: i32, sign: i32| {
        let mut t = [0i32; rescue_data::combat::STAGES];
        for (i, v) in t.iter_mut().enumerate() {
            *v = base + sign * (i as i32 - 10) * slope;
        }
        t
    };
    CombatTables {
        type_chart: [[2i16; types::TYPES]; types::TYPES],
        effectiveness: [[2i32; 4]; 4],
        cell_multiplier: [0x4000, 0xC000, 0x10000, 0x20000],
        stab: 0x14000,
        physical: [1; types::TYPES],
        attack_stage: stage(0x10, 0x100, 1),
        defence_stage: stage(0x10, 0x100, 1),
        accuracy_stage: stage(0x20, 0x100, 1),
        evasion_stage: stage(0x10, 0x100, -1),
        regular_attack_weight: [9, 7, 5, 3, 1],
    }
}

fn move_of(id: u16, power: i16) -> Move {
    Move {
        id,
        name: String::new(),
        base_power: power,
        kind: 1,
        targeting: [0, 0],
        base_pp: 200,
        ai_weight: 0,
        accuracy: [100; 3],
        chained_hits: 0,
        max_upgrade: 0,
        crit_chance: 0,
        magic_coat: false,
        snatchable: false,
        uses_mouth: false,
        cannot_hit_frozen: false,
        ignores_taunted: false,
        range: 0,
        description: String::new(),
        use_text: String::new(),
    }
}

/// A level-5 leader (the probe's team) facing a wild monster of `species` at `level` on the tile
/// to its right. The wild monster comes from the floor's spawn list and spawn mark.
fn battle(species: u16, level: u8, mv: &Move) -> GameState {
    let leader = Monster {
        pos: (5, 5),
        visible: true,
        info: EntityInfo {
            species: 158,
            apparent_species: 158,
            not_team_member: false,
            is_team_leader: true,
            level: 5,
            iq: 0,
            hp: 100,
            max_hp: 100,
            attack: [50, 50],
            defence: [5, 5],
            offensive_stages: [10, 10],
            defensive_stages: [10, 10],
            accuracy_stage: 10,
            evasion_stage: 10,
            speed_up: [0; 5],
            speed_down: [0; 5],
            direction: 2,
            belly: 100_000,
            belly_empty: false,
            regen_acc: 0,
            status: Default::default(),
            regen_speed: 200,
            exp: 60,
            exp_gained: 0,
            types: [1, 0],
            abilities: [0, 0],
            moves: [
                MoveSlot::new(mv),
                MoveSlot::default(),
                MoveSlot::default(),
                MoveSlot::default(),
            ],
        },
    };
    let mut floor = Floor::new();
    floor.tile_mut(6, 5).unwrap().terrain = TERRAIN_FLOOR;
    floor.tile_mut(5, 5).unwrap().terrain = TERRAIN_FLOOR;
    floor.tile_mut(6, 5).unwrap().spawn_or_visibility = 0x8; // the enemy spawn mark
    let spawns = [MonsterSpawn {
        species,
        level,
        weights: [1, 1],
    }];
    GameState::in_dungeon_with_spawns(1, floor, vec![leader], 2, &spawns).unwrap()
}

/// The EXP the leader gains from the first defeat of a `species` monster at `level` whose yield
/// is `yield_`.
fn exp_of_one_defeat(species: u16, level: u8, yield_: i32) -> i32 {
    let mv = move_of(0x9A, 40);
    let mut moves: Vec<Move> = (0..=REGULAR_ATTACK).map(|id| move_of(id, 0)).collect();
    moves[0x9A] = mv.clone();
    let mut exp_yields = vec![0; usize::from(species) + 1];
    exp_yields[usize::from(species)] = yield_;
    let t = tables();
    let levels: Vec<Vec<LevelData>> = Vec::new();
    let data = CombatData {
        tables: &t,
        moves: &moves,
        exp_yields: &exp_yields,
        weights: &[],
        levels: &levels,
    };
    let mut g = battle(species, level, &mv);
    assert_eq!(
        g.dungeon().unwrap().wild().len(),
        1,
        "one wild monster on the marked tile"
    );
    for _ in 0..50 {
        let use_it = Command::UseMove {
            slot: 0,
            direction: Some(Direction::Right),
        };
        assert_eq!(g.step_with(use_it, &data), Outcome::Applied);
        let leader = &g.dungeon().unwrap().team()[0].info;
        if leader.exp_gained != 0 {
            assert_eq!(leader.level, 5, "no level table, no level-up");
            assert_eq!(leader.exp, 60 + leader.exp_gained);
            return leader.exp_gained;
        }
    }
    panic!("the wild monster was never defeated");
}

#[test]
fn the_exp_of_a_defeat_reads_the_defeated_monsters_level() {
    // The probe's runs (`020020F8` result; the leader is level 5 in each, so its level would give
    // other numbers). The sim's multiplier is 1, so the award is the full result.
    assert_eq!(exp_of_one_defeat(290, 1, 15), 15);
    assert_eq!(exp_of_one_defeat(191, 1, 16), 16);
    assert_eq!(exp_of_one_defeat(16, 1, 12), 12);
    assert_eq!(exp_of_one_defeat(191, 10, 16), 30);
    assert_eq!(exp_of_one_defeat(290, 100, 15), 163);
}
