//! What the leader sees and what it reveals on the map (`docs/re/visibility.md`, "Rule for a
//! leader with no status" and "Claims").
//!
//! Each leader step marks the tiles it sees in the tile word `+2` (`Tile::spawn_or_visibility`):
//! bit 0 is "revealed" (what the minimap reads) and bit 1 is written with it. Inside a room the
//! marked set is the room's rect with a wall ring on every side; in a corridor it is a square of
//! half-width R, where R is the floor's darkness setting (0, a lit floor, reads as 2).
//! [`Dungeon::in_view`] is the in-view test of `ov_0001::0219C6BC`, keyed on the viewer's room.
//! Statuses (blindness, see-through), the upper-screen darkening and the AI's targets are not
//! modelled here. Integer math only.

use rescue_data::dungeon::FLOOR_PROPERTIES;

use crate::floor::{ANCHOR_ROOM, Floor, HEIGHT, NO_ROOM, Room, WIDTH};
use crate::state::Dungeon;

/// Tile word `+2`, bit 0: the tile is revealed on the map (`ov_0001::02147DC4`).
pub const REVEALED_BIT: u16 = 0x1;
/// Tile word `+2`, bit 1: written together with [`REVEALED_BIT`] by both reveal paths. The name
/// "lit" is the investigator's; no reader of this bit was found (`docs/re/visibility.md`).
pub const LIT_BIT: u16 = 0x2;

/// Offset of the darkness setting in a floor-properties record (`props +0x16`, `& 3`).
const DARKNESS_OFFSET: usize = 0x16;

// The record must hold the darkness byte.
const _: () = assert!(DARKNESS_OFFSET < FLOOR_PROPERTIES);

/// The darkness setting `D` of a floor-properties record: `props +0x16 & 3`. 0 is a lit floor.
#[must_use]
pub fn darkness(record: &[u8; FLOOR_PROPERTIES]) -> u8 {
    record[DARKNESS_OFFSET] & 3
}

/// The corridor range `R` for darkness `D`: `R = D`, and 2 on a lit floor (`D = 0`).
#[must_use]
pub fn corridor_range(darkness: u8) -> i32 {
    match darkness & 3 {
        0 => 2,
        d => i32::from(d),
    }
}

/// The floor entry step, before the arrival reveal (`RunDungeon_Async`, `0206DC20`). It clears
/// the generation markers in the tile word `+2` (`ov_0001::0214809C`: stairs, items and traps
/// are not yet visibility), then builds the room table from the final tiles
/// (`ov_0001::02147EB4`). Both run here and not in the generator, so the layout is unchanged.
// shortcut: the hallway anchor (0xFE) gets no record, so it stays a corridor tile. Blue's table
// has 32 records and the anchor index would run past it; the anchor's record is not known.
pub fn enter_floor(floor: &mut Floor) {
    let mut rooms: Vec<Option<Room>> = Vec::new();
    for y in 0..HEIGHT as i32 {
        for x in 0..WIDTH as i32 {
            let Some(tile) = floor.tile_mut(x, y) else {
                continue;
            };
            tile.spawn_or_visibility = 0;
            let index = tile.room;
            if index == NO_ROOM || index == ANCHOR_ROOM {
                continue;
            }
            let i = usize::from(index);
            if rooms.len() <= i {
                rooms.resize(i + 1, None);
            }
            rooms[i] = Some(match rooms[i] {
                None => Room {
                    x0: x,
                    y0: y,
                    x1: x + 1,
                    y1: y + 1,
                    revealed: false,
                },
                Some(r) => Room {
                    x0: r.x0.min(x),
                    y0: r.y0.min(y),
                    x1: r.x1.max(x + 1),
                    y1: r.y1.max(y + 1),
                    revealed: false,
                },
            });
        }
    }
    // A number no tile carries has no record to read, so its placeholder is never used.
    floor.rooms = rooms
        .into_iter()
        .map(|r| {
            r.unwrap_or(Room {
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 0,
                revealed: false,
            })
        })
        .collect();
}

impl Dungeon {
    /// Whether `viewer` sees `target` (`ov_0001::0219C6BC`). If the viewer stands in a room, the
    /// target is in the room's rect with a wall ring on every side (`x0 - 1 <= x <= x1`, and the
    /// same for y; `x1` and `y1` are exclusive ends, see [`Room`]). Otherwise it is in the square
    /// of half-width `R` around the viewer.
    #[must_use]
    pub fn in_view(&self, viewer: (i16, i16), target: (i16, i16)) -> bool {
        let (vx, vy) = (i32::from(viewer.0), i32::from(viewer.1));
        let (tx, ty) = (i32::from(target.0), i32::from(target.1));
        let room = self
            .floor()
            .tile(vx, vy)
            .filter(|t| t.room != NO_ROOM)
            .and_then(|t| self.floor().rooms().get(usize::from(t.room)));
        match room {
            Some(r) => (r.x0 - 1..=r.x1).contains(&tx) && (r.y0 - 1..=r.y1).contains(&ty),
            None => {
                let range = corridor_range(self.darkness);
                (tx - vx).abs() <= range && (ty - vy).abs() <= range
            }
        }
    }

    /// The leader's step reveal: the room rect (with the wall ring on every side) when the
    /// leader stands in a room, once per room; otherwise the square of half-width `R` every
    /// time. Sets both bits on the set's tiles. Does nothing without a leader.
    // shortcut: the blindness gate (`+0x15FA2`) is not modelled, as the sim has no statuses, so
    // the reveal always runs. Blue runs it only after a move; `turn.rs` notes why running it
    // after a wait too changes nothing.
    // shortcut: the room id is taken as a room only when the table has it; the hallway anchor
    // (0xFE) is treated as a corridor (see `enter_floor`).
    pub(crate) fn reveal_from_leader(&mut self) {
        let Some(pos) = self.team().first().map(|leader| leader.pos) else {
            return;
        };
        let (lx, ly) = (i32::from(pos.0), i32::from(pos.1));
        let Some(index) = self.floor().tile(lx, ly).map(|t| t.room) else {
            return;
        };
        if index != NO_ROOM {
            let room = self.floor().rooms().get(usize::from(index)).copied();
            if let Some(room) = room {
                if !room.revealed {
                    if let Some(record) = self.floor.room_mut(index) {
                        record.revealed = true;
                    }
                    mark_rect(self, (room.x0 - 1, room.y0 - 1), (room.x1, room.y1));
                }
                return;
            }
        }
        let range = corridor_range(self.darkness);
        mark_rect(self, (lx - range, ly - range), (lx + range, ly + range));
    }
}

/// Sets the revealed and lit bits on every tile of the inclusive rect from `(x0, y0)` to
/// `(x1, y1)`. Tiles off the map are skipped.
fn mark_rect(dungeon: &mut Dungeon, (x0, y0): (i32, i32), (x1, y1): (i32, i32)) {
    for x in x0..=x1 {
        for y in y0..=y1 {
            if let Some(tile) = dungeon.floor.tile_mut(x, y) {
                tile.spawn_or_visibility |= REVEALED_BIT | LIT_BIT;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Direction, Outcome};
    use crate::floor::{Floor, HEIGHT, Room, TERRAIN_FLOOR, Tile, WIDTH};
    use crate::state::{EntityInfo, GameState, Monster, MoveSlot};

    /// A run whose first floor is `floor`, with the leader at `pos` and darkness `darkness`. The
    /// floor is entered the way the game enters it, so the darkness comes from the record table.
    fn entered(floor: Floor, pos: (i16, i16), darkness: u8) -> GameState {
        let info = EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: true,
            level: 1,
            iq: 0,
            hp: 10,
            max_hp: 10,
            attack: [1, 1],
            defence: [1, 1],
            offensive_stages: [10, 10],
            defensive_stages: [10, 10],
            accuracy_stage: 10,
            evasion_stage: 10,
            types: [0, 0],
            abilities: [0, 0],
            moves: [MoveSlot::default(); 4],
            speed_up: [0; 5],
            speed_down: [0; 5],
            direction: 0,
            belly: 100_000,
            belly_empty: false,
            regen_acc: 0,
            regen_speed: 200,
            exp: 0,
            exp_gained: 0,
        };
        let leader = Monster {
            pos,
            visible: true,
            info,
        };
        // Record 0 is the placeholder; record 1 is the floor the run enters first.
        let mut record = [0u8; FLOOR_PROPERTIES];
        record[DARKNESS_OFFSET] = darkness;
        let mut g = GameState::new(1).with_floors(vec![[0; FLOOR_PROPERTIES], record]);
        g.enter_dungeon(floor, vec![leader], 0)
            .expect("one leader fits");
        g
    }

    fn dungeon_with(floor: Floor, pos: (i16, i16), darkness: u8) -> Dungeon {
        entered(floor, pos, darkness)
            .dungeon()
            .expect("a floor is loaded")
            .clone()
    }

    /// A floor with every inner tile plain floor and no room.
    fn open_floor() -> Floor {
        let mut floor = Floor::new();
        for x in 1..WIDTH as i32 - 1 {
            for y in 1..(HEIGHT as i32 - 1) {
                if let Some(t) = floor.tile_mut(x, y) {
                    t.terrain = TERRAIN_FLOOR;
                }
            }
        }
        floor
    }

    #[test]
    fn a_step_reveals_the_square_around_the_new_tile() {
        // Darkness 1: R = 1. The arrival reveals x 19 to 21; the step to (21, 20) adds x 22.
        let mut g = entered(open_floor(), (20, 20), 1);
        let arrival = dungeon_with(open_floor(), (20, 20), 1);
        assert_eq!(bits(&arrival, 22, 21), 0);
        assert_eq!(g.step(Command::Move(Direction::Right)), Outcome::Applied);
        let d = g.dungeon().expect("a floor is loaded");
        assert_eq!(d.team()[0].pos, (21, 20));
        assert_eq!(bits(d, 22, 21), REVEALED_BIT | LIT_BIT);
        assert_eq!(bits(d, 22, 22), 0, "outside the square around (21, 20)");
    }

    #[test]
    fn arriving_reveals_the_start_square() {
        let g = entered(open_floor(), (20, 20), 2);
        let d = g.dungeon().expect("a floor is loaded");
        assert_eq!(revealed_count(d), 25);
    }

    /// A floor with every inner tile plain floor, and a room at (5, 5) to (9, 8) numbered 0.
    fn floor_with_room() -> Floor {
        let mut floor = Floor::new();
        for x in 1..WIDTH as i32 - 1 {
            for y in 1..(HEIGHT as i32 - 1) {
                if let Some(t) = floor.tile_mut(x, y) {
                    t.terrain = TERRAIN_FLOOR;
                }
            }
        }
        for x in 5..=9 {
            for y in 5..=8 {
                if let Some(t) = floor.tile_mut(x, y) {
                    t.room = 0;
                }
            }
        }
        floor
    }

    #[test]
    fn entry_clears_the_markers_and_builds_the_room_table_from_the_tiles() {
        let mut floor = floor_with_room();
        // Generation markers: the stairs (bit 0) and an item (bit 1), outside the reveal set.
        for (x, y, marker) in [(2, 2, 0x1), (3, 3, 0x2)] {
            floor.tile_mut(x, y).unwrap().spawn_or_visibility = marker;
        }
        let d = dungeon_with(floor, (7, 6), 0);
        assert_eq!(d.floor().tile(2, 2).unwrap().spawn_or_visibility & 0x1, 0);
        assert_eq!(d.floor().tile(3, 3).unwrap().spawn_or_visibility, 0);
        // The record's end is exclusive: the last floor tile is (9, 8), so the end is (10, 9).
        assert_eq!(
            d.floor().rooms(),
            &[Room {
                x0: 5,
                y0: 5,
                x1: 10,
                y1: 9,
                revealed: true,
            }]
        );
    }

    #[test]
    fn a_hallway_anchor_has_no_record_and_stays_a_corridor() {
        let mut floor = open_floor();
        floor.tile_mut(20, 20).unwrap().room = crate::floor::ANCHOR_ROOM;
        enter_floor(&mut floor);
        assert!(floor.rooms().is_empty());
    }

    fn bits(d: &Dungeon, x: i32, y: i32) -> u16 {
        d.floor()
            .tile(x, y)
            .map_or(0, |t: &Tile| t.spawn_or_visibility)
            & (REVEALED_BIT | LIT_BIT)
    }

    fn revealed_count(d: &Dungeon) -> usize {
        (0..crate::floor::HEIGHT as i32)
            .flat_map(|y| (0..WIDTH as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| bits(d, x, y) & REVEALED_BIT != 0)
            .count()
    }

    #[test]
    fn darkness_is_the_low_two_bits_of_the_record_byte() {
        let mut record = [0u8; FLOOR_PROPERTIES];
        record[DARKNESS_OFFSET] = 0b1110_0110;
        assert_eq!(darkness(&record), 2);
        record[DARKNESS_OFFSET] = 0;
        assert_eq!(darkness(&record), 0);
    }

    #[test]
    fn a_lit_floor_has_a_corridor_range_of_two() {
        assert_eq!(corridor_range(0), 2);
        assert_eq!(corridor_range(1), 1);
        assert_eq!(corridor_range(2), 2);
    }

    #[test]
    fn entering_a_room_reveals_its_rect_with_the_wall_ring_once() {
        let mut d = dungeon_with(floor_with_room(), (7, 6), 0);
        d.reveal_from_leader();
        // The room's floor is x 5 to 9, y 5 to 8; the ring is one tile on every side: x 4 to 10,
        // y 4 to 9.
        for x in 4..=10 {
            for y in 4..=9 {
                assert_eq!(bits(&d, x, y), REVEALED_BIT | LIT_BIT, "({x}, {y})");
            }
        }
        assert_eq!(bits(&d, 11, 6), 0, "past the far ring");
        assert_eq!(bits(&d, 4, 3), 0, "the row above the ring is not revealed");
        assert!(d.floor().rooms()[0].revealed);
        // A second step in the room leaves the set alone: a stale tile is not cleared or
        // added, because the reveal returns once the room is revealed.
        let before = revealed_count(&d);
        d.reveal_from_leader();
        assert_eq!(revealed_count(&d), before);
    }

    #[test]
    fn a_corridor_step_reveals_the_range_square() {
        let mut d = dungeon_with(Floor::new(), (20, 20), 2);
        d.reveal_from_leader();
        // Darkness 2: R = 2, so the 5 by 5 square.
        assert_eq!(revealed_count(&d), 25);
        assert_eq!(bits(&d, 18, 18), REVEALED_BIT | LIT_BIT);
        assert_eq!(bits(&d, 22, 22), REVEALED_BIT | LIT_BIT);
        assert_eq!(bits(&d, 23, 20), 0);
    }

    #[test]
    fn a_dark_floor_reveals_by_its_own_range() {
        // Darkness 1: R = 1, so only the 3 by 3 square.
        let mut d = dungeon_with(Floor::new(), (20, 20), 1);
        d.reveal_from_leader();
        assert_eq!(revealed_count(&d), 9);
        assert_eq!(bits(&d, 21, 21), REVEALED_BIT | LIT_BIT);
        assert_eq!(bits(&d, 22, 20), 0);
    }

    #[test]
    fn a_lit_floor_corridor_square_is_five_wide_even_at_darkness_zero() {
        let mut d = dungeon_with(Floor::new(), (20, 20), 0);
        d.reveal_from_leader();
        assert_eq!(revealed_count(&d), 25);
    }

    #[test]
    fn the_square_is_clipped_to_the_map() {
        let mut d = dungeon_with(Floor::new(), (0, 0), 2);
        d.reveal_from_leader();
        // The square reaches (-2, -2), off the map; only the in-map quarter is marked.
        assert_eq!(revealed_count(&d), 9);
    }

    #[test]
    fn in_view_in_a_room_uses_the_rect_and_takes_no_range() {
        let d = dungeon_with(floor_with_room(), (7, 6), 1);
        // Inside the rect and its ring on both sides: yes, even far from the viewer.
        assert!(d.in_view((7, 6), (4, 4)));
        assert!(d.in_view((7, 6), (9, 8)));
        assert!(d.in_view((7, 6), (10, 9)));
        // Past the far ring of the room: no.
        assert!(!d.in_view((7, 6), (11, 6)));
        assert!(!d.in_view((7, 6), (3, 4)));
    }

    #[test]
    fn in_view_in_a_corridor_uses_the_range_square() {
        let d = dungeon_with(Floor::new(), (20, 20), 1);
        assert!(d.in_view((20, 20), (21, 21)));
        assert!(!d.in_view((20, 20), (22, 20)));
        let lit = dungeon_with(Floor::new(), (20, 20), 0);
        assert!(lit.in_view((20, 20), (22, 22)));
        assert!(!lit.in_view((20, 20), (23, 20)));
    }
}
