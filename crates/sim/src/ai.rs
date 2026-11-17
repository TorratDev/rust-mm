//! A wild monster's turn (`docs/re/ai.md`, "Decision flow", "Movement and pathfinding", "RNG
//! calls"; `docs/re/turns.md`, "Other monsters' turns", "One enemy turn (#358 P7)"): the wild
//! loop `021408F8` runs `RunMonsterAI` (`02188030`) and Execute for each wild monster that is
//! gated in, then the end-of-turn effects of the ones left flagged `aiAllySkip`. The draws come
//! in the game's order. Integer math only.
//!
//! A turn is: the weighted move pick (`02192D24`, one `DungeonRandInt`), then either the regular
//! attack (the leader is next to the monster) or `AIMovement` (`021918D4`): the target search
//! (`02191394`), the wander (`02190854`) when it finds none, and `DecideMovement` (`02190FEC`).
//! Execute then draws the end-of-turn roll (`02186894`), unless the monster is flagged
//! `aiAllySkip`, which defers it to the end of the loop.

use crate::floor::{self, NO_ROOM};
use crate::state::{DIRECTIONS, Dungeon};

/// The weight total of the move pick, `DungeonRandInt(total)` at `02193260`. The game's total is
/// the regular-attack weight plus the weights of the enabled moves; wild monsters showed 35 and
/// 45 (`docs/re/turns.md`, P7).
// shortcut: the total is fixed at 35, the most common wild value (150 of 163 walks); the sim has
// no wild move sets, so which slots are enabled is not known. Upgrade with the move-selection
// issue (#454), which reads the monster's moves.
const MOVE_PICK_TOTAL: i32 = 35;

/// `gFaceDirectionIncrements`, the order `DecideMovement` tries directions in: ahead, then 45
/// degrees one way and the other (the loop stops at index 2 without a run-away or turn-around
/// state, so only these three are reached).
// shortcut: the pool table `0x021A98F4` was not read; the values are Red's table (the note says
// Blue's order matches, `ai.md` P3-e). Check with an overlay byte read before the sign of the
// 45-degree turns is relied on.
const TURNS: [usize; 3] = [0, 1, 7];

/// `0200B9A4`-style straight-corridor masks of the wander's first branch (`02180C98`): the tile's
/// walkable mask equal to one of these makes the monster pick a new random direction.
const STRAIGHT_MASKS: [u8; 5] = [0x54, 0x51, 0x45, 0x15, 0x55];

/// What a turn's decision step ended with.
enum Plan {
    /// Pass the turn (action 1).
    Pass,
    /// Walk one tile in direction index `0..8`.
    Walk(usize),
}

impl Dungeon {
    /// The wild loop of one fractional turn (`021408F8`): every wild monster acts once in slot
    /// order, and each one still flagged `aiAllySkip` at the end gets its end-of-turn effects.
    // shortcut: every wild monster is gated at speed stage 1 (the caller picks the gated turns);
    // its own speed counters are not read. A sleeping, asleep-on-spawn or statused monster is not
    // modelled, so every wild monster acts. The `0x8000` and `0x4000` flags never get set.
    pub(crate) fn run_wild_monsters(&mut self) {
        let mut deferred = 0;
        for i in 0..self.wild.len() {
            if self.wild_turn(i) {
                deferred += 1;
            }
        }
        for _ in 0..deferred {
            self.monster_end_of_turn();
        }
    }

    /// One wild monster's turn: `RunMonsterAI`, Execute and the end-of-turn effects Execute runs.
    /// Returns whether the monster is flagged `aiAllySkip`, which defers the effects.
    fn wild_turn(&mut self, i: usize) -> bool {
        let Some(me) = self.wild.get(i).map(|m| m.pos) else {
            return false;
        };
        if self.team.is_empty() {
            return false;
        }
        // `RunMonsterAI` steps 1 to 9 draw nothing for a monster with no status or item.
        // `ChooseAIMove`: the weighted pick is drawn whatever the target.
        let _pick = self.rng.int(MOVE_PICK_TOTAL);
        let target = self.find_target(me);
        if let Some(t) = target.filter(|&t| self.adjacent_attackable(me, t)) {
            // The regular attack: the target pick among one candidate (`02192CC0`).
            let _candidate = self.rng.int(1);
            // shortcut: Execute's attack draws (accuracy, crit, damage: `0215A950`, `0217C02C`,
            // `0217C698`, `0217C720`) and the damage are not run: the combat path
            // (`Dungeon::resolve`) takes the leader as the attacker, so an enemy swing needs it
            // generalised first. The monster faces the target; nothing else changes.
            self.wild[i].info.direction = direction_towards(me, t);
            self.monster_end_of_turn();
            return false;
        }
        let goal = target.unwrap_or_else(|| self.wander(i));
        let (plan, skip) = self.decide_movement(me, goal);
        if let Plan::Walk(dir) = plan {
            let (dx, dy) = DIRECTIONS[dir].delta();
            let m = &mut self.wild[i];
            // The map is at most 56 by 32, so the step stays in `i16`.
            m.pos = (m.pos.0 + dx as i16, m.pos.1 + dy as i16);
            m.info.direction = dir as u8;
        }
        if !skip {
            self.monster_end_of_turn();
        }
        skip
    }

    /// `DoEndOfTurnEffects` for a wild monster: the one draw it makes, `DungeonRandInt(100)`
    /// (`02186894`).
    // shortcut: a wild monster has no belly or status, so only the roll is kept (see
    // `leader_end_of_turn`).
    fn monster_end_of_turn(&mut self) {
        let _roll = self.rng.int(100);
    }

    /// `AIFindTarget` (`02191394`) for a wild monster at `me`: the nearest team member it sees,
    /// the first at the smallest distance, stopping at one within 1 tile.
    // shortcut: the remembered-target block (a target seen earlier, its 4 previous positions) is
    // not kept, as the sim has no AI target record: a monster that loses sight wanders at once.
    // Sight is `in_view` (`0219C6BC`); the game's wild-monster test is `0219C5C4`, not read. The
    // behaviour, petrified and decoy filters are not modelled (no statuses).
    fn find_target(&self, me: (i16, i16)) -> Option<(i16, i16)> {
        let mut best: Option<((i16, i16), i32)> = None;
        for t in self.team.iter().filter(|t| t.info.hp > 0) {
            if !self.in_view(me, t.pos) {
                continue;
            }
            let dist = chebyshev(me, t.pos);
            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((t.pos, dist));
                if dist < 2 {
                    break;
                }
            }
        }
        best.map(|(pos, _)| pos)
    }

    /// Whether the regular attack reaches `t` from `me`: one tile away with no wall corner
    /// between.
    // shortcut: the range check of `02192728` is read as "the target is on one of the 8 adjacent
    // tiles", with the corner rule of a walk; the target flag filters are not modelled.
    fn adjacent_attackable(&self, me: (i16, i16), t: (i16, i16)) -> bool {
        let (dx, dy) = (i32::from(t.0 - me.0), i32::from(t.1 - me.1));
        chebyshev(me, t) == 1 && self.corner_open(i32::from(me.0), i32::from(me.1), dx, dy)
    }

    /// `AIWander` (`02190854`): the tile the monster heads to. The corridor branch (the monster's
    /// tile has no room) is read from the decompile; it returns the objective-3 tile (the first
    /// direction that can be walked) or, with none, a random neighbour (objective 6).
    // shortcut: the in-room branch needs the room's exit list (`dungeon+0xE5E4`, with
    // `+0xE5E4 + 0x80 * room` entries), which the sim does not hold. It draws the fallback
    // `Int(8)` only, which is the game's path for a room with no exits; a room with exits draws
    // `Int(count)` (the `Int(3)` of the P7 table) and then walks to one. Upgrade once the room
    // exit lists are generated at floor entry.
    // shortcut: the mobility class is 0 (a regular walker) and the objective and the `+0xF0`
    // state are read as unset, as the sim keeps no AI target record.
    fn wander(&mut self, i: usize) -> (i16, i16) {
        let m = &self.wild[i];
        let (me, facing) = (m.pos, usize::from(m.info.direction) % 8);
        let in_corridor = self
            .floor
            .tile(i32::from(me.0), i32::from(me.1))
            .is_some_and(|t| t.room == NO_ROOM);
        let mut dir = facing;
        if in_corridor {
            let jitter = self
                .floor
                .tile(i32::from(me.0), i32::from(me.1))
                .is_some_and(|t| STRAIGHT_MASKS.contains(&t.walkable_neighbors[0]));
            if jitter {
                dir = self.rng.int(8) as usize;
            }
            // The 8 candidates: `facing` (or the roll), then alternately either side of it.
            for k in [0, 1, 7, 2, 6, 3, 5, 4] {
                let d = (dir + k) % 8;
                // After a roll the way back, `facing + 4`, is skipped.
                if jitter && d == (facing + 4) % 8 {
                    continue;
                }
                if self.can_move(me, d).0 {
                    return step(me, d);
                }
            }
        }
        step(me, self.rng.int(8) as usize)
    }

    /// `DecideMovement` (`02190FEC`) for a wild monster at `me` heading to `goal`: the plan and
    /// whether the monster ends flagged `aiAllySkip` (it was blocked by a monster).
    // shortcut: the 90-degree turns (a run-away or turn-around state) and the tactic that keeps a
    // distance are not modelled; the monster has none of those.
    fn decide_movement(&self, me: (i16, i16), goal: (i16, i16)) -> (Plan, bool) {
        if me == goal {
            return (Plan::Pass, false);
        }
        let dir = direction_towards(me, goal) as usize;
        let mut blocked_by_monster = false;
        for (n, turn) in TURNS.iter().enumerate() {
            let d = (dir + turn) % 8;
            let (ok, monster) = self.can_move(me, d);
            if ok {
                return (Plan::Walk(d), false);
            }
            if monster {
                // A goal tile held by a monster: wait there, not flagged.
                if n == 0 && step(me, d) == goal {
                    return (Plan::Pass, false);
                }
                blocked_by_monster = true;
            }
        }
        // A non-leader that saw a monster in the way skips to the end-of-turn pass.
        (Plan::Pass, blocked_by_monster)
    }

    /// `CanAIMonsterMoveInDirection` (`02180DF8`) from `me` in direction index `dir`: whether the
    /// step is allowed, and whether a monster stands on the tile (which blocks it).
    // shortcut: the walk rule of `leader_target`, a plain walker: plain floor, no wall corner cut.
    // The per-tile walkable masks, the monster-house, trap and lava IQ skills and the all-terrain
    // and super-mobile classes are not modelled.
    fn can_move(&self, me: (i16, i16), dir: usize) -> (bool, bool) {
        let (dx, dy) = DIRECTIONS[dir].delta();
        let (x, y) = (i32::from(me.0) + dx, i32::from(me.1) + dy);
        let Some(tile) = self.floor.tile(x, y) else {
            return (false, false);
        };
        if tile.terrain & floor::TERRAIN_IMPASSABLE_WALL != 0
            || tile.terrain & floor::TERRAIN_TYPE_MASK != floor::TERRAIN_FLOOR
            || !self.corner_open(i32::from(me.0), i32::from(me.1), dx, dy)
        {
            return (false, false);
        }
        let monster = self
            .team
            .iter()
            .chain(self.wild.iter())
            .any(|m| i32::from(m.pos.0) == x && i32::from(m.pos.1) == y);
        (!monster, monster)
    }
}

/// `02011834`: the Chebyshev distance.
fn chebyshev(a: (i16, i16), b: (i16, i16)) -> i32 {
    (i32::from(a.0) - i32::from(b.0))
        .abs()
        .max((i32::from(a.1) - i32::from(b.1)).abs())
}

/// The tile one step from `p` in direction index `dir`.
fn step(p: (i16, i16), dir: usize) -> (i16, i16) {
    let (dx, dy) = DIRECTIONS[dir % 8].delta();
    (p.0 + dx as i16, p.1 + dy as i16)
}

/// `GetDirectionTowardsPosition` (`0201187C`): the direction of the signs of the offset (0 when
/// the positions are equal), in the game's direction index.
fn direction_towards(from: (i16, i16), to: (i16, i16)) -> u8 {
    let (dx, dy) = ((to.0 - from.0).signum(), (to.1 - from.1).signum());
    DIRECTIONS
        .iter()
        .position(|d| d.delta() == (i32::from(dx), i32::from(dy)))
        .map_or(0, |p| p as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::floor::{Floor, Room};
    use crate::state::{EntityInfo, GameState, Monster, MoveSlot, wild_monster};

    /// A 56 by 32 open room with a leader at `leader`, and wild monsters at `wild`.
    fn room_with(leader: (i16, i16), wild: &[(i16, i16)]) -> GameState {
        let info = EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: true,
            level: 8,
            iq: 0,
            hp: 30,
            max_hp: 30,
            attack: [10, 10],
            defence: [5, 5],
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
        let mut floor = Floor::new();
        for x in 0..56 {
            for y in 0..32 {
                let t = floor.tile_mut(x, y).unwrap();
                t.terrain = floor::TERRAIN_FLOOR;
                t.room = 0;
            }
        }
        floor.rooms.push(Room {
            x0: 0,
            y0: 0,
            x1: 56,
            y1: 32,
            revealed: true,
        });
        let leader = Monster {
            pos: leader,
            visible: true,
            info,
        };
        let mut g = GameState::in_dungeon(1, floor, vec![leader], 0).unwrap();
        let d = g.dungeon_mut().unwrap();
        d.wild = wild.iter().map(|&p| wild_monster(7, 4, p)).collect();
        g
    }

    #[test]
    fn a_walk_turn_draws_the_pick_and_the_end_of_turn_and_steps_toward_the_leader() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_wild_monsters();
        // `Int(35)@02193260`, then `Int(100)@02186894`; the walk itself draws nothing.
        expected.int(35);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.wild[0].pos, (6, 5));
        assert_eq!(d.wild[0].info.direction, 2);
    }

    #[test]
    fn a_wander_turn_in_a_straight_corridor_rolls_a_direction_and_steps_that_way() {
        let mut g = room_with((50, 28), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        // The monster is in a corridor (no room) the leader is too far to see from.
        for x in 0..56 {
            for y in 0..32 {
                let t = d.floor.tile_mut(x, y).unwrap();
                t.room = NO_ROOM;
                t.walkable_neighbors = [0x55, 0, 0, 0];
            }
        }
        d.floor.rooms.clear();
        let mut expected = d.rng;
        d.run_wild_monsters();
        expected.int(35);
        let roll = expected.int(8) as usize;
        expected.int(100);
        assert_eq!(d.rng, expected);
        // Facing 0, so the way back is 4; the roll is taken unless it is that.
        let dir = if roll == 4 { 5 } else { roll };
        assert_eq!(d.wild[0].pos, step((5, 5), dir));
    }

    #[test]
    fn a_monster_blocked_by_another_defers_its_end_of_turn_to_the_end_of_the_loop() {
        // A at (5,5) heads right for the leader; B stands at (6,5) and walls close the two 45
        // degree tiles, so A cannot move. B then steps on, and A's roll comes last.
        let mut g = room_with((9, 5), &[(5, 5), (6, 5)]);
        let d = g.dungeon_mut().unwrap();
        for p in [(6, 4), (6, 6)] {
            d.floor.tile_mut(p.0, p.1).unwrap().terrain = floor::TERRAIN_IMPASSABLE_WALL;
        }
        let mut expected = d.rng;
        d.run_wild_monsters();
        // A: pick. B: pick, end of turn. Then A's deferred end of turn.
        for cap in [35, 35, 100, 100] {
            expected.int(cap);
        }
        assert_eq!(d.rng, expected);
        assert_eq!((d.wild[0].pos, d.wild[1].pos), ((5, 5), (7, 5)));
    }

    #[test]
    fn a_leader_next_to_the_monster_gets_the_attack_draws_and_the_monster_stays() {
        let mut g = room_with((6, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_wild_monsters();
        for cap in [35, 1, 100] {
            expected.int(cap);
        }
        assert_eq!(d.rng, expected);
        assert_eq!(d.wild[0].pos, (5, 5));
    }

    #[test]
    fn the_wild_loop_runs_in_the_leader_turn_on_gated_turns() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        // Several waits cover every kind of fractional turn; the monster has closed in by then.
        for _ in 0..6 {
            assert_eq!(g.step(Command::Wait), crate::command::Outcome::Applied);
        }
        assert!(g.dungeon().unwrap().wild()[0].pos.0 > 5);
    }
}
