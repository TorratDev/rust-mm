//! The executor's target list of a move the leader uses (`docs/re/combat.md`, "Range and
//! targeting", the executor path `0215A520` and `0215A3A8`). The area (high nibble of the move's
//! targeting byte) gives the candidates in the game's order, and the class (low nibble) keeps
//! some of them.
//!
//! Every area of the executor path is modelled: facing (`0x00`), cone (`0x10`), around (`0x20`),
//! room (`0x30`), two ahead (`0x40`), line (`0x50`), floor (`0x60`), self (`0x70`) and cut
//! corners (`0x80`). The class-4 rewrite is modelled with no charge state (see
//! [`rewrite_class_four`]). Integer math only.

use crate::command::Direction;
use crate::floor::{TERRAIN_IMPASSABLE_WALL, TERRAIN_TYPE_MASK};
use crate::state::{DIRECTIONS, Dungeon, EntityInfo, Monster};

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
    /// `0x40`: the facing tile's monster, or the one two ahead when the first gives no target.
    TwoAhead,
    /// `0x50`: the first monster on the line in the facing direction.
    Line,
    /// `0x60`: every slot.
    Floor,
    /// `0x70`: the user only.
    User,
    /// `0x80`: the facing tile's monster, with the blocked test and no corner check.
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

/// The targeting byte the executor uses after its class-4 rewrite (`0215A520`, `docs/re/combat.md`,
/// "Class 4 is decided before filtering"). `0215A2C0` returns 0 (self, `0x73`) unless the move is
/// in the charge table with the user's `+0xAE` at its row. Solarbeam in sunny weather is the foe
/// branch (`0x00`) whatever `0215A2C0` returned (export line 44). `None` when that needs the
/// weather, which the caller does not know.
// shortcut: `0215A2C0` reads `info +0xAE` (the charge state of Fly, Dig and the like). The sim has
// no charge state, so that value is always 0 and the rewrite is self for every class 4 move, as
// `0215A2C0` returns 0. Porting that is right only while no charge is in flight, which holds for
// this sim.
// shortcut: the sim has no weather state, so the caller passes `None` for the sunny test and
// Solarbeam is refused. Solarbeam with the weather known is `Some(true)` (foe) or `Some(false)`
// (self).
#[must_use]
pub(crate) const fn rewrite_class_four(move_id: u16, byte: u8, sunny: Option<bool>) -> Option<u8> {
    if byte & 0xF != 4 {
        return Some(byte);
    }
    if move_id != SOLARBEAM {
        return Some(SELF_CLASS_BYTE);
    }
    match sunny {
        Some(true) => Some(0),
        Some(false) => Some(SELF_CLASS_BYTE),
        None => None,
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

/// The monster record at `slot`, or `None` for a slot that is not there.
fn monster_of(d: &Dungeon, slot: Slot) -> Option<&Monster> {
    match slot {
        Slot::Team(i) => d.team().get(i),
        Slot::Wild(i) => d.wild().get(i),
    }
}

/// The 20 slots of `gDungeon+0x11364` in the game's order: the team, then the wild monsters.
/// `02141A08` rebuilds that list this way (`docs/re/combat.md`, "Executor in the sim"), and the
/// sim's vectors hold no gaps, so the list is the two vectors in turn.
fn all_slots(d: &Dungeon) -> Vec<Slot> {
    (0..d.team().len())
        .map(Slot::Team)
        .chain((0..d.wild().len()).map(Slot::Wild))
        .collect()
}

/// `021820B0` (`IsBlockedForCut`, `docs/re/combat.md`): true when the tile at `(x, y)` blocks a
/// step onto it. The tile is off the map, a wall (flag `0x10`), or of type 0. Only the
/// destination counts: the user's own tile and the two sides of a diagonal are not read.
// shortcut: the test uses class 2 for every user. Class 3 (movement type 3 species, `info +0xC7 ==
// 3`, held item 9, IQ bit 0xD) never blocks on type 0, so a class-3 user may be blocked here
// when the game is not. The gate `FUN_0206d474` is not modelled either.
// shortcut: step 3 of `021820B0` (a non-monster entity on the tile blocks) is not modelled. The
// sim keeps monsters only on the tile's entity link, so it cannot arise here.
fn blocked_for_cut(d: &Dungeon, x: i32, y: i32) -> bool {
    match d.floor().tile(x, y) {
        None => true,
        Some(t) => t.terrain & TERRAIN_IMPASSABLE_WALL != 0 || t.terrain & TERRAIN_TYPE_MASK == 0,
    }
}

/// `021775F0` (`FacingTileMonster`): the monster on the facing tile, when `021820B0` does not
/// block that tile. No corner test: a diagonal facing needs no open sides.
fn facing_monster(d: &Dungeon, lx: i32, ly: i32, facing: Direction) -> Option<Slot> {
    let (x, y) = step(lx, ly, facing);
    if blocked_for_cut(d, x, y) {
        return None;
    }
    monster_at(d, x, y)
}

/// `021774FC` (`TwoAheadMonster`): the monster two tiles ahead. Nothing is returned when the
/// facing tile or the tile two ahead is blocked, so a wall or an empty map edge stops the shot.
fn two_ahead_monster(d: &Dungeon, lx: i32, ly: i32, facing: Direction) -> Option<Slot> {
    let (x1, y1) = step(lx, ly, facing);
    if blocked_for_cut(d, x1, y1) {
        return None;
    }
    let (x2, y2) = step(x1, y1, facing);
    if blocked_for_cut(d, x2, y2) {
        return None;
    }
    monster_at(d, x2, y2)
}

/// The candidates of `area` for the leader facing `facing`, in the game's order, before the class
/// filter. `None` when the leader is missing. For `0x40` this is the facing monster only: the
/// tile two ahead is the fallback that [`executor_targets`] tries when that one is filtered out.
fn candidates(d: &Dungeon, area: Area, facing: Direction) -> Option<Vec<Slot>> {
    let leader = d.team().first()?;
    let (lx, ly) = (i32::from(leader.pos.0), i32::from(leader.pos.1));
    let dir = index_of(facing);
    Some(match area {
        Area::Facing => {
            let (x, y) = step(lx, ly, facing);
            let (dx, dy) = facing.delta();
            // `02180FFC` at class 2: the facing tile is not blocked (`021820B0` at class 2: not a
            // wall, not type 0), and a diagonal step has both orthogonal neighbours open. The
            // corner test is shared with the leader's step (`Dungeon::corner_open`).
            // shortcut: class 2 is used for every user. Class 3 (movement type 3 species,
            // `info +0xC7 == 3`, held item 9, IQ bits 0xC and 0xD) and the gate `FUN_0206d474`
            // are not modelled, so a user in those cases may get a different answer here.
            if !blocked_for_cut(d, x, y) && d.corner_open(lx, ly, dx, dy) {
                monster_at(d, x, y).into_iter().collect()
            } else {
                Vec::new()
            }
        }
        Area::Cone => [(dir + 7) % 8, dir, (dir + 1) % 8]
            .into_iter()
            .filter_map(|k| {
                let (x, y) = step(lx, ly, DIRECTIONS[k]);
                monster_at(d, x, y)
            })
            .collect(),
        // The eight neighbours from the facing direction on, wrapping (`0215A520`: the loop starts
        // at `info +0x44` and runs eight times, `& 7`). No gate, as in the cone.
        Area::Around => (0..8)
            .filter_map(|k| {
                let (x, y) = step(lx, ly, DIRECTIONS[(dir + k) % 8]);
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
        // `0x30`: the slots the room test `02141AF0` passes (`docs/re/combat.md`, the `0x30` row).
        // The test needs the target to be visible (`+0x20`), and the room of the user's tile to
        // hold the target's tile: `Dungeon::in_view` is `0219C6BC`. The user is in the list when
        // it is visible, as `02141AF0(user, user)` passes then.
        Area::Room => all_slots(d)
            .into_iter()
            .filter(|&slot| {
                monster_of(d, slot).is_some_and(|m| m.visible && d.in_view(leader.pos, m.pos))
            })
            .collect(),
        // `0x40`: the facing monster; the fallback (two ahead) is in `executor_targets`.
        Area::TwoAhead => facing_monster(d, lx, ly, facing).into_iter().collect(),
        // `0x60`: every valid slot, with no room test.
        Area::Floor => all_slots(d),
        // `0x80`: the facing monster, with the blocked test and no corner check (`0215A520`,
        // branch `0x80`, calls `021775F0` alone).
        Area::CutCorners => facing_monster(d, lx, ly, facing).into_iter().collect(),
    })
}

/// The fallback of `0x40` (`0215A520`): the monster two tiles ahead, through `021774FC`, when the
/// facing monster gives no target. Its tile must not be blocked, and the facing one must not be
/// either (`021774FC` returns nothing for a blocked facing tile).
fn two_ahead_fallback(d: &Dungeon, facing: Direction) -> Vec<Slot> {
    let Some(leader) = d.team().first() else {
        return Vec::new();
    };
    let (lx, ly) = (i32::from(leader.pos.0), i32::from(leader.pos.1));
    two_ahead_monster(d, lx, ly, facing).into_iter().collect()
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
    let kept = |list: Vec<Slot>| -> Vec<Slot> {
        list.into_iter()
            .filter(|&slot| keeps(class, move_id, slot))
            .collect()
    };
    let targets = kept(candidates(d, area, facing)?);
    // `0215A520`, area `0x40`: the tile two ahead is tried only when the facing monster is
    // missing or the class filter drops it. The list holds at most one monster.
    if area == Area::TwoAhead && targets.is_empty() {
        return Some(kept(two_ahead_fallback(d, facing)));
    }
    Some(targets)
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
                exp: 0,
                exp_gained: 0,
                types: [9, 0],
                abilities: [0, 0],
                moves: [MoveSlot::default(); 4],
            },
        }
    }

    /// A dungeon with the leader at (10, 10) and a floor of open tiles, so every line tile is
    /// ground and none is a wall.
    fn dungeon(team: Vec<Monster>, wild: Vec<Monster>) -> GameState {
        dungeon_with(team, wild, |_| {})
    }

    /// [`dungeon`], with `edit` applied to the floor before the dungeon is entered, so a room
    /// table built from the tiles sees the edit.
    fn dungeon_with(
        team: Vec<Monster>,
        wild: Vec<Monster>,
        edit: impl FnOnce(&mut Floor),
    ) -> GameState {
        let mut floor = Floor::new();
        for y in 1..31 {
            for x in 1..55 {
                floor.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        edit(&mut floor);
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
    fn class_four_is_self_except_solarbeam_in_sunny_weather() {
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        let d = lead(&g);
        // Another move with class 4: 0215A2C0 is 0 with no charge state, so the self class 0x73
        // (the user only), whatever the weather.
        assert_eq!(
            executor_targets(d, 0x9A, 0x04, Direction::Right, None),
            Some(vec![Slot::Team(0)])
        );
        // Solarbeam (0x97) in sunny weather: the foe branch, area 0 class 0 (export line 44).
        assert_eq!(
            executor_targets(d, SOLARBEAM, 0x74, Direction::Right, Some(true)),
            Some(vec![Slot::Wild(0)])
        );
        // Not sunny: 0215A2C0 decides, which is self with no charge.
        assert_eq!(
            executor_targets(d, SOLARBEAM, 0x74, Direction::Right, Some(false)),
            Some(vec![Slot::Team(0)])
        );
        // The weather is not known in the sim, so Solarbeam is refused.
        assert_eq!(
            executor_targets(d, SOLARBEAM, 0x74, Direction::Right, None),
            None
        );
    }

    /// Sets the terrain of tile `(x, y)` of the dungeon in `g`.
    fn set_terrain(g: &mut GameState, x: i32, y: i32, terrain: u16) {
        g.dungeon_mut()
            .unwrap()
            .floor
            .tile_mut(x, y)
            .unwrap()
            .terrain = terrain;
    }

    #[test]
    fn the_around_area_lists_the_eight_neighbours_from_the_facing_tile_on() {
        // The foes are listed in table order from Down, so only the facing order can sort them.
        let g = dungeon(
            vec![monster((10, 10), false)],
            vec![
                monster((10, 11), true), // Down, Wild(0)
                monster((11, 11), true), // DownRight, Wild(1)
                monster((11, 10), true), // Right, Wild(2)
                monster((11, 9), true),  // UpRight, Wild(3)
                monster((10, 9), true),  // Up, Wild(4)
                monster((9, 9), true),   // UpLeft, Wild(5)
                monster((9, 10), true),  // Left, Wild(6)
                monster((9, 11), true),  // DownLeft, Wild(7)
            ],
        );
        // Facing Right (index 2): Right, UpRight, Up, UpLeft, Left, DownLeft, then wraps to Down
        // and DownRight.
        assert_eq!(
            targets(&g, 0x20, Direction::Right),
            Some((2..8).chain(0..2).map(Slot::Wild).collect::<Vec<_>>())
        );
        // Facing Down (index 0) starts at Down and runs to DownRight, the last.
        assert_eq!(
            targets(&g, 0x20, Direction::Down),
            Some((0..8).map(Slot::Wild).collect::<Vec<_>>())
        );
        // Facing DownLeft (index 7) wraps at once: DownLeft, then Down, and so on to Left.
        assert_eq!(
            targets(&g, 0x20, Direction::DownLeft),
            Some(
                [7].into_iter()
                    .chain(0..7)
                    .map(Slot::Wild)
                    .collect::<Vec<_>>()
            )
        );
    }

    /// The targets of byte `0x00` for a foe at (11, 9) and the leader at (10, 10) facing UpRight,
    /// after `edit` has changed the dungeon.
    fn up_right_foe(edit: impl FnOnce(&mut GameState)) -> Option<Vec<Slot>> {
        let mut g = dungeon(vec![monster((10, 10), false)], vec![monster((11, 9), true)]);
        edit(&mut g);
        targets(&g, 0x00, Direction::UpRight)
    }

    #[test]
    fn a_diagonal_facing_foe_needs_both_orthogonal_sides_open() {
        // The sides of the diagonal from (10, 10) to (11, 9) are (11, 10) and (10, 9).
        assert_eq!(up_right_foe(|_| {}), Some(vec![Slot::Wild(0)]));
        // A wall on either side cuts the corner.
        assert_eq!(
            up_right_foe(|g| set_terrain(g, 11, 10, TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL)),
            Some(vec![])
        );
        assert_eq!(
            up_right_foe(|g| set_terrain(g, 10, 9, TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL)),
            Some(vec![])
        );
        // A side of type 0 (not a wall flag) is not open either.
        assert_eq!(up_right_foe(|g| set_terrain(g, 10, 9, 0)), Some(vec![]));
        // A facing tile of type 0 gives no candidate.
        assert_eq!(up_right_foe(|g| set_terrain(g, 11, 9, 0)), Some(vec![]));
    }

    #[test]
    fn a_cardinal_facing_foe_has_no_corner_test() {
        // A foe on the tile to the right, with walls on both sides of the row above it: a
        // cardinal step cuts no corner, so the foe is still the candidate.
        let mut g = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        set_terrain(&mut g, 11, 9, TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL);
        set_terrain(&mut g, 10, 9, TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL);
        assert_eq!(
            targets(&g, 0x00, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
    }

    /// Sets the room of tile `(x, y)` to `room`.
    fn set_room(floor: &mut Floor, x: i32, y: i32, room: u8) {
        floor.tile_mut(x, y).unwrap().room = room;
    }

    #[test]
    fn the_room_area_keeps_the_visible_slots_in_the_users_room() {
        // Room 0 covers (8..=12, 8..=12); the leader stands in it. The ring is one tile further.
        let mut g = dungeon_with(
            vec![monster((10, 10), false), monster((12, 8), false)],
            vec![
                monster((9, 11), true),  // in the room, visible: Wild(0)
                monster((20, 10), true), // outside the room and its ring: out of view
                monster((11, 11), true), // in the room, invisible: out
            ],
            |f| {
                for x in 8..=12 {
                    for y in 8..=12 {
                        set_room(f, x, y, 0);
                    }
                }
            },
        );
        g.dungeon_mut().unwrap().wild[2].visible = false;
        // Class 2 keeps every candidate, so the list is the room test: the user (visible), the
        // ally in the room, then the wild monster in the room, in slot order.
        assert_eq!(
            targets(&g, 0x32, Direction::Right),
            Some(vec![Slot::Team(0), Slot::Team(1), Slot::Wild(0)])
        );
        // An invisible user drops out of the room list.
        g.dungeon_mut().unwrap().team[0].visible = false;
        assert_eq!(
            targets(&g, 0x32, Direction::Right),
            Some(vec![Slot::Team(1), Slot::Wild(0)])
        );
    }

    #[test]
    fn the_floor_area_keeps_every_valid_slot_in_team_then_wild_order() {
        let g = dungeon(
            vec![monster((10, 10), false), monster((9, 10), false)],
            vec![monster((30, 25), true), monster((2, 2), true)],
        );
        // Class 2 keeps all: the team first, then the wild monsters, with no room test.
        assert_eq!(
            targets(&g, 0x62, Direction::Right),
            Some(vec![
                Slot::Team(0),
                Slot::Team(1),
                Slot::Wild(0),
                Slot::Wild(1)
            ])
        );
        // Class 0 keeps the foes only.
        assert_eq!(
            targets(&g, 0x60, Direction::Right),
            Some(vec![Slot::Wild(0), Slot::Wild(1)])
        );
    }

    #[test]
    fn the_two_ahead_area_takes_the_facing_monster_then_the_one_two_ahead() {
        // Leader (10, 10) facing Right: the facing tile is (11, 10), two ahead is (12, 10).
        let two_ahead = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((12, 10), true)],
        );
        assert_eq!(
            targets(&two_ahead, 0x40, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
        // A foe on the facing tile is taken alone: the one two ahead is not reached.
        let both = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true), monster((12, 10), true)],
        );
        assert_eq!(
            targets(&both, 0x40, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
        // An ally on the facing tile is filtered out by class 0, so the foe two ahead is tried.
        let ally_ahead = dungeon(
            vec![monster((10, 10), false), monster((11, 10), false)],
            vec![monster((12, 10), true)],
        );
        assert_eq!(
            targets(&ally_ahead, 0x40, Direction::Right),
            Some(vec![Slot::Wild(0)])
        );
        // A wall on the facing tile blocks both: the shot stops there, even with a foe behind.
        let walled = dungeon_with(
            vec![monster((10, 10), false)],
            vec![monster((12, 10), true)],
            |f| set_wall(f, 11, 10),
        );
        assert_eq!(targets(&walled, 0x40, Direction::Right), Some(vec![]));
        // A type-0 tile on the facing tile is blocked too.
        let type_zero = dungeon_with(
            vec![monster((10, 10), false)],
            vec![monster((12, 10), true)],
            |f| f.tile_mut(11, 10).unwrap().terrain = TERRAIN_FLOOR & !TERRAIN_TYPE_MASK,
        );
        assert_eq!(targets(&type_zero, 0x40, Direction::Right), Some(vec![]));
        // A wall on the tile two ahead blocks the foe standing on it.
        let walled_far = dungeon_with(
            vec![monster((10, 10), false)],
            vec![monster((12, 10), true)],
            |f| set_wall(f, 12, 10),
        );
        assert_eq!(targets(&walled_far, 0x40, Direction::Right), Some(vec![]));
        // No corner test: a diagonal two ahead with both sides walled still reaches the foe.
        let mut diagonal = dungeon(vec![monster((10, 10), false)], vec![monster((12, 8), true)]);
        set_terrain(
            &mut diagonal,
            11,
            10,
            TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL,
        );
        set_terrain(
            &mut diagonal,
            10,
            9,
            TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL,
        );
        assert_eq!(
            targets(&diagonal, 0x40, Direction::UpRight),
            Some(vec![Slot::Wild(0)])
        );
    }

    /// Makes tile `(x, y)` an impassable wall.
    fn set_wall(f: &mut Floor, x: i32, y: i32) {
        f.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL;
    }

    #[test]
    fn the_cut_corners_area_takes_the_facing_monster_with_no_corner_test() {
        // A diagonal foe with walls on both sides of the step: `0x00` refuses it (the corner
        // test), `0x80` takes it (`021775F0` has no corner test).
        let mut g = dungeon(vec![monster((10, 10), false)], vec![monster((11, 9), true)]);
        set_terrain(&mut g, 11, 10, TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL);
        set_terrain(&mut g, 10, 9, TERRAIN_FLOOR | TERRAIN_IMPASSABLE_WALL);
        assert_eq!(targets(&g, 0x00, Direction::UpRight), Some(vec![]));
        assert_eq!(
            targets(&g, 0x80, Direction::UpRight),
            Some(vec![Slot::Wild(0)])
        );
        // The monster must be on the facing tile: one tile further is out of reach.
        let far = dungeon(vec![monster((10, 10), false)], vec![monster((12, 8), true)]);
        assert_eq!(targets(&far, 0x80, Direction::UpRight), Some(vec![]));
        // The blocked test applies to the facing tile: a wall or a type-0 tile gives no target.
        let walled = dungeon_with(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
            |f| set_wall(f, 11, 10),
        );
        assert_eq!(targets(&walled, 0x80, Direction::Right), Some(vec![]));
        let mut type_zero = dungeon(
            vec![monster((10, 10), false)],
            vec![monster((11, 10), true)],
        );
        set_terrain(&mut type_zero, 11, 10, 0);
        assert_eq!(targets(&type_zero, 0x80, Direction::Right), Some(vec![]));
    }

    #[test]
    fn a_nibble_outside_the_areas_is_not_modelled() {
        let g = dungeon(vec![monster((10, 10), false)], vec![]);
        assert_eq!(targets(&g, 0x90, Direction::Right), None);
    }
}
