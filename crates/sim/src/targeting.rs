//! The executor's target list of a move the leader uses (`docs/re/combat.md`, "Range and
//! targeting", the executor path `0215A520` and `0215A3A8`). The area (high nibble of the move's
//! targeting byte) gives the candidates in the game's order, and the class (low nibble) keeps
//! some of them.
//!
//! Only the areas whose rule is in the notes are modelled: facing (`0x00`), cone (`0x10`), line
//! (`0x50`) and self (`0x70`). The others return `None` and the use is not modelled. The
//! class-4 rewrite is modelled for Solarbeam (`0x97`), and only with the weather it needs.
//! Integer math only.

use crate::command::Direction;
use crate::floor::{TERRAIN_IMPASSABLE_WALL, TERRAIN_TYPE_MASK};
use crate::state::{DIRECTIONS, Dungeon, EntityInfo};

/// A monster a move can reach: team member `i` (0 is the leader) or wild monster `i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Team(usize),
    Wild(usize),
}

/// The area of a targeting byte (its high nibble, `docs/re/combat.md`, "Area (high nibble)").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Area {
    /// `0x00`: the facing tile.
    Facing,
    /// `0x10`: the facing tile and both tiles beside it.
    Cone,
    /// `0x20`: the eight neighbours.
    Around,
    /// `0x30`: the slots in the same room.
    Room,
    /// `0x40`: the facing tile, or the tile two ahead when it is empty.
    TwoAhead,
    /// `0x50`: the first monster on the line in the facing direction.
    Line,
    /// `0x60`: every slot.
    Floor,
    /// `0x70`: the user only.
    User,
    /// `0x80`: the facing tile, with the corner-cut check.
    CutCorners,
}

impl Area {
    /// The area a high nibble names, or `None` for a nibble the notes do not list.
    #[must_use]
    pub(crate) const fn from_nibble(nibble: u8) -> Option<Self> {
        Some(match nibble {
            0x0 => Self::Facing,
            0x1 => Self::Cone,
            0x2 => Self::Around,
            0x3 => Self::Room,
            0x4 => Self::TwoAhead,
            0x5 => Self::Line,
            0x6 => Self::Floor,
            0x7 => Self::User,
            0x8 => Self::CutCorners,
            _ => return None,
        })
    }
}

/// Tiles the line walk looks at (`docs/re/combat.md`, the `0x50` row: "up to 10 tiles").
const LINE_TILES: usize = 10;

/// Move `0xFC` is rejected when it is aimed at the user (`0215A3A8`, the first test).
const MOVE_AIMED_AT_USER: u16 = 0xFC;
/// Move `0xED` takes the self class `0x73` for a user that is not a Ghost (`02191A30`).
const MOVE_SELF_CLASS: u16 = 0xED;
/// Solarbeam: its class 4 is a foe when the weather is sunny (`0215A520`, export line 44).
const SOLARBEAM: u16 = 0x97;
/// The type that makes [`MOVE_SELF_CLASS`] keep its targeting byte (Ghost).
const GHOST: u8 = 14;
/// The self class byte of [`MOVE_SELF_CLASS`].
const SELF_CLASS_BYTE: u8 = 0x73;

/// The targeting byte the executor uses after its class-4 rewrite (`0215A520`, lines 41 to 52 of
/// the export, `docs/re/combat.md`, "Class 4 is decided before filtering"). `None` when the
/// rewrite needs `0215A2C0`, which the notes do not decode.
// shortcut: the sim has no weather state, so the caller passes `None` for the sunny test and
// Solarbeam is refused. `0215A2C0` is open (not decompiled in the notes), so every other class 4
// is refused too.
#[must_use]
pub(crate) const fn rewrite_class_four(move_id: u16, byte: u8, sunny: Option<bool>) -> Option<u8> {
    if byte & 0xF != 4 {
        return Some(byte);
    }
    match (move_id == SOLARBEAM, sunny) {
        // The foe branch, area 0 class 0, is forced for Solarbeam in sunny weather.
        (true, Some(true)) => Some(0),
        _ => None,
    }
}

/// The index of `dir` in the game's direction table.
fn index_of(dir: Direction) -> usize {
    DIRECTIONS.iter().position(|&d| d == dir).unwrap_or(0)
}

/// The tile `steps` away from `(x, y)` in `dir`, in `i32` so the map edge needs no wrap check.
fn step(x: i32, y: i32, dir: Direction) -> (i32, i32) {
    let (dx, dy) = dir.delta();
    (x + dx, y + dy)
}

/// The monster standing on `(x, y)`: a team member first, then a wild one.
fn monster_at(d: &Dungeon, x: i32, y: i32) -> Option<Slot> {
    let on = |m: &crate::state::Monster| i32::from(m.pos.0) == x && i32::from(m.pos.1) == y;
    d.team()
        .iter()
        .position(on)
        .map(Slot::Team)
        .or_else(|| d.wild().iter().position(on).map(Slot::Wild))
}

/// The candidates of `area` for the leader facing `facing`, in the game's order, before the class
/// filter. `None` when the area's rule is not in the notes.
fn candidates(d: &Dungeon, area: Area, facing: Direction) -> Option<Vec<Slot>> {
    let leader = d.team().first()?;
    let (lx, ly) = (i32::from(leader.pos.0), i32::from(leader.pos.1));
    let dir = index_of(facing);
    Some(match area {
        Area::Facing => {
            let (x, y) = step(lx, ly, facing);
            // A wall on the facing tile gives no candidate (`02180FFC`).
            // shortcut: the direction-open bit of `02180FFC` (a walkable-neighbour bit of the
            // user's tile, `docs/re/combat.md`, "Helpers") is not read: a facing monster is
            // always a candidate.
            match d.floor().tile(x, y) {
                Some(t) if t.terrain & TERRAIN_IMPASSABLE_WALL == 0 => {
                    monster_at(d, x, y).into_iter().collect()
                }
                _ => Vec::new(),
            }
        }
        Area::Cone => [(dir + 7) % 8, dir, (dir + 1) % 8]
            .into_iter()
            .filter_map(|k| {
                let (x, y) = step(lx, ly, DIRECTIONS[k]);
                monster_at(d, x, y)
            })
            .collect(),
        Area::Line => {
            // The first monster the walk meets. A wall stops the walk (its type is 0, the `& 3`
            // test of `0215AB34`); the map edge is a wall too, as no tile lies beyond it.
            let (mut x, mut y) = (lx, ly);
            let mut found = Vec::new();
            for _ in 0..LINE_TILES {
                (x, y) = step(x, y, facing);
                let Some(tile) = d.floor().tile(x, y) else {
                    break;
                };
                if tile.terrain & TERRAIN_TYPE_MASK == 0 {
                    break;
                }
                if let Some(slot) = monster_at(d, x, y) {
                    found.push(slot);
                    break;
                }
            }
            found
        }
        Area::User => vec![Slot::Team(0)],
        // shortcut: these areas are not modelled, because their rule is not in the notes or
        // needs the game's 20-slot order, which the sim does not keep. Each refuses the use.
        // `0x20`: the order of the eight neighbours is not in the notes (the export's loop starts
        // at the facing direction; a lead, not verified). `0x30` and `0x60`: the 20 slots of
        // `gDungeon+0x11364` and the room test `02141AF0`. `0x40` and `0x80`: the facing tile
        // helper `021775F0` and the tile test `021774FC` are not read.
        Area::Around | Area::Room | Area::TwoAhead | Area::Floor | Area::CutCorners => {
            return None;
        }
    })
}

/// Whether the class `class` keeps candidate `slot` (`0215A3A8`, the relation tests). Relation 1
/// is a foe (a wild monster); relation 0 is an ally or the user (a team member).
fn keeps(class: u8, move_id: u16, slot: Slot) -> bool {
    let user = slot == Slot::Team(0);
    if move_id == MOVE_AIMED_AT_USER && user {
        return false;
    }
    // shortcut: the game's other rejects (`info +0x8 == 1`, `info +0x98` of 1 or 4) and the
    // veto `0215A054` (`info +0xDC`, `+0xAE`) read fields the sim does not keep, so they are
    // not applied. The sim's monsters all pass them.
    // shortcut: `skipRelation` (argument 7) reads `info +0xAC` and `+0xC9`, which the sim does
    // not keep. It is 0 here, as it is for a leader with both fields clear.
    let foe = matches!(slot, Slot::Wild(_));
    match class {
        0 | 4 => foe,
        1 => !foe,
        2 | 3 => true,
        5 => !user,
        6 => !foe && !user,
        _ => false,
    }
}

/// The targets of move `move_id` (targeting byte `targeting`, the low byte of `targeting[0]`)
/// used by the leader facing `facing`, in the game's order: the candidates that pass the class
/// filter. `None` when the range is not modelled. An empty list is a use with no target, which
/// the caller refuses.
// shortcut: `sunny` is always `None` in the sim (see [`rewrite_class_four`]).
#[must_use]
pub(crate) fn executor_targets(
    d: &Dungeon,
    move_id: u16,
    targeting: u16,
    facing: Direction,
    sunny: Option<bool>,
) -> Option<Vec<Slot>> {
    let mut byte = (targeting & 0xFF) as u8;
    // `02191A30`: move 0xED takes the self class for a user that is not a Ghost.
    if move_id == MOVE_SELF_CLASS {
        let leader: &EntityInfo = &d.team().first()?.info;
        if !leader.types.contains(&GHOST) {
            byte = SELF_CLASS_BYTE;
        }
    }
    let byte = rewrite_class_four(move_id, byte, sunny)?;
    let area = Area::from_nibble(byte >> 4)?;
    let class = byte & 0xF;
    let list = candidates(d, area, facing)?;
    Some(
        list.into_iter()
            .filter(|&slot| keeps(class, move_id, slot))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floor::{Floor, TERRAIN_FLOOR};
    use crate::state::{GameState, Monster, MoveSlot};

    /// An entity at `pos`, a team member or a wild one.
    fn monster(pos: (i16, i16), wild: bool) -> Monster {
        Monster {
            pos,
            visible: true,
            info: EntityInfo {
                species: 1,
                apparent_species: 1,
                not_team_member: wild,
                is_team_leader: false,
                level: 10,
                iq: 0,
                hp: 100,
                max_hp: 100,
                attack: [10, 10],
                defence: [10, 10],
                offensive_stages: [10, 10],
                defensive_stages: [10, 10],
                accuracy_stage: 10,
                evasion_stage: 10,
                speed_up: [0; 5],
                speed_down: [0; 5],
                direction: 0,
                belly: 100_000,
                belly_empty: false,
                regen_acc: 0,
                regen_speed: 200,
                types: [9, 0],
                abilities: [0, 0],
                moves: [MoveSlot::default(); 4],
            },
        }
    }

    /// A dungeon with the leader at (10, 10) and a floor of open tiles, so every line tile is
    /// ground and none is a wall.
    fn dungeon(team: Vec<Monster>, wild: Vec<Monster>) -> GameState {
        let mut floor = Floor::new();
        for y in 1..31 {
            for x in 1..55 {
                floor.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        let mut team = team;
        team[0].info.is_team_leader = true;
        let mut g = GameState::in_dungeon(1, floor, team, 2).unwrap();
        g.dungeon_mut().unwrap().wild = wild;
        g
    }

    fn lead(g: &GameState) -> &Dungeon {
        g.dungeon().unwrap()
    }

    /// `executor_targets` for a move whose targeting byte is `byte` (the low byte).
    fn targets(g: &GameState, byte: u16, facing: Direction) -> Option<Vec<Slot>> {
        executor_targets(lead(g), 0x9A, byte, facing, None)
    }

    #[test]
    fn the_facing_tile_gives_its_monster_to_a_foe_class_only() {
        // Foe on the tile to the right, ally on the tile to the left.
        let g = dungeon(
            vec![monster((10, 10), false), monster((9, 10), false)],
            vec![monster((11, 10), true)],
        );
        assert_eq!(
            targets(&g, 0x00, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
        // The ally on the left is not a foe, so class 0 keeps nothing there.
        assert_eq!(targets(&g, 0x00, Direction::Left), Some(vec![]));
    }

    #[test]
    fn class_one_keeps_an_ally_and_class_six_keeps_an_ally_that_is_not_the_user() {
        let g = dungeon(
            vec![monster((10, 10), false), monster((9, 10), false)],
            vec![],
        );
        // Class 1 (ally or user): the ally on the left.
        assert_eq!(
            targets(&g, 0x01, Direction::Left),
            Some(vec![Slot::Team(1)])
        );
        assert_eq!(
            targets(&g, 0x06, Direction::Left),
            Some(vec![Slot::Team(1)])
        );
        // Class 2 and 3 keep any candidate.
        assert_eq!(
            targets(&g, 0x02, Direction::Left),
            Some(vec![Slot::Team(1)])
        );
        assert_eq!(
            targets(&g, 0x03, Direction::Left),
            Some(vec![Slot::Team(1)])
        );
        // Class 5 keeps any candidate but the user. Class 7 and up keep nothing.
        assert_eq!(
            targets(&g, 0x05, Direction::Left),
            Some(vec![Slot::Team(1)])
        );
        assert_eq!(targets(&g, 0x07, Direction::Left), Some(vec![]));
    }

    #[test]
    fn a_wall_on_the_facing_tile_gives_no_candidate() {
        let mut g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        g.dungeon_mut()
            .unwrap()
            .floor
            .tile_mut(11, 10)
            .unwrap()
            .terrain |= TERRAIN_IMPASSABLE_WALL;
        assert_eq!(targets(&g, 0x00, Direction::Right), Some(vec![]));
    }

    #[test]
    fn the_cone_lists_the_left_tile_the_facing_tile_and_the_right_tile_in_that_order() {
        // Facing Right (index 2): the cone is facing - 1 (DownRight), facing, facing + 1 (UpRight).
        // The monsters are in the vector out of cone order, so only the cone order can sort them.
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![
                monster((11, 10), true), // Right, facing
                monster((11, 9), true),  // UpRight, facing + 1
                monster((11, 11), true), // DownRight, facing - 1
            ],
        );
        assert_eq!(
            targets(&g, 0x10, Direction::Right),
            Some(vec![Slot::Wild(2), Slot::Wild(0), Slot::Wild(1)])
        );
    }

    #[test]
    fn the_cone_wraps_round_the_direction_table() {
        // Facing Down (index 0): facing - 1 is DownLeft (index 7), facing + 1 is DownRight.
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![
                monster((10, 11), true), // Down, facing
                monster((9, 11), true),  // DownLeft, facing - 1 (wraps to 7)
                monster((11, 11), true), // DownRight, facing + 1
            ],
        );
        assert_eq!(
            targets(&g, 0x10, Direction::Down),
            Some(vec![Slot::Wild(1), Slot::Wild(0), Slot::Wild(2)])
        );
    }

    #[test]
    fn the_line_takes_the_first_monster_within_ten_tiles() {
        // A foe three tiles to the right, and another behind it: the walk stops at the first.
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((13, 10), true), monster((14, 10), true)],
        );
        assert_eq!(
            targets(&g, 0x52, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
        // Class 0 keeps the foe, class 1 refuses it (the walk still stops at it).
        assert_eq!(targets(&g, 0x51, Direction::Right), Some(vec![]));
    }

    #[test]
    fn the_line_stops_at_a_wall_and_at_the_tenth_tile() {
        let mut g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((13, 10), true)],
        );
        g.dungeon_mut()
            .unwrap()
            .floor
            .tile_mut(12, 10)
            .unwrap()
            .terrain = 0; // a type-0 wall in front of the foe
        assert_eq!(targets(&g, 0x52, Direction::Right), Some(vec![]));

        // A foe at the eleventh tile is out of reach: the tenth tile is (20, 10).
        let mut far = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((21, 10), true)],
        );
        assert_eq!(targets(&far, 0x52, Direction::Right), Some(vec![]));
        far.dungeon_mut().unwrap().wild[0].pos = (20, 10);
        assert_eq!(
            targets(&far, 0x52, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
    }

    #[test]
    fn the_user_class_targets_only_the_user() {
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        // Area 0x70: the user. Classes 1 and 3 keep it, class 0 (a foe) and class 5 (not the
        // user) do not.
        assert_eq!(
            targets(&g, 0x71, Direction::Right),
            Some(vec![Slot::Team(0)])
        );
        assert_eq!(
            targets(&g, 0x73, Direction::Right),
            Some(vec![Slot::Team(0)])
        );
        assert_eq!(targets(&g, 0x70, Direction::Right), Some(vec![]));
        assert_eq!(targets(&g, 0x75, Direction::Right), Some(vec![]));
        assert_eq!(targets(&g, 0x76, Direction::Right), Some(vec![]));
    }

    #[test]
    fn move_0xfc_is_refused_when_aimed_at_the_user() {
        let g = dungeon(vec![monster((10, 10), false)], vec![]);
        assert_eq!(
            executor_targets(lead(&g), MOVE_AIMED_AT_USER, 0x71, Direction::Right, None),
            Some(vec![])
        );
    }

    #[test]
    fn move_0xed_takes_the_self_class_unless_the_user_is_a_ghost() {
        let mut g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        // Targeting byte 0x00 (a foe ahead) becomes 0x73 (self) for a user that is not a Ghost.
        assert_eq!(
            executor_targets(lead(&g), MOVE_SELF_CLASS, 0x00, Direction::Right, None),
            Some(vec![Slot::Team(0)])
        );
        g.dungeon_mut().unwrap().team[0].info.types = [GHOST, 0];
        assert_eq!(
            executor_targets(lead(&g), MOVE_SELF_CLASS, 0x00, Direction::Right, None),
            Some(vec![Slot::Wild(0)])
        );
    }

    #[test]
    fn class_four_is_a_foe_only_for_solarbeam_in_sunny_weather() {
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        let d = lead(&g);
        // Solarbeam (0x97) in sunny weather: the foe branch, area 0 class 0.
        assert_eq!(
            executor_targets(d, SOLARBEAM, 0x74, Direction::Right, Some(true)),
            Some(vec![Slot::Wild(0)])
        );
        // Not sunny, or not known: 0215A2C0 decides, and it is not in the notes.
        assert_eq!(
            executor_targets(d, SOLARBEAM, 0x74, Direction::Right, Some(false)),
            None
        );
        assert_eq!(
            executor_targets(d, SOLARBEAM, 0x74, Direction::Right, None),
            None
        );
        // Another move with class 4 has no weather rule, so it is refused too.
        assert_eq!(
            executor_targets(d, 0x9A, 0x04, Direction::Right, Some(true)),
            None
        );
    }

    #[test]
    fn areas_whose_rule_is_not_in_the_notes_are_not_modelled() {
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        for byte in [0x20u16, 0x30, 0x40, 0x60, 0x80] {
            assert_eq!(targets(&g, byte, Direction::Right), None, "{byte:#x}");
        }
    }

    #[test]
    fn a_nibble_outside_the_areas_is_not_modelled() {
        let g = dungeon(vec![monster((10, 10), false)], vec![]);
        assert_eq!(targets(&g, 0x90, Direction::Right), None);
    }
}
