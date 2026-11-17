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

use crate::combat::{CombatData, REGULAR_ATTACK, REGULAR_ATTACK_ARG8};
use crate::floor::{self, NO_ROOM};
use crate::move_effects::{self, Effect};
use crate::state::{DIRECTIONS, Dungeon, MoveSlot};
use crate::targeting::Slot;

/// The weight total of the move pick when the sim has no combat data, `DungeonRandInt(total)` at
/// `02193260`. The game's total is the regular-attack weight plus the weights of the enabled
/// moves; wild monsters showed 35 and 45 (`docs/re/turns.md`, P7).
// shortcut: without combat data (`GameState::step`, no move table) the total is fixed at 35, the
// most common wild value (150 of 163 walks), and nothing is attacked. With the data,
// `choose_ai_move` computes the total from the monster's move slots.
const MOVE_PICK_TOTAL: i32 = 35;

/// The entries of the weighted pick: the four move slots, then the regular attack
/// (`02192D24`, `docs/re/ai.md`, "Move and target selection").
const REGULAR_ENTRY: usize = 4;
/// Bit 2 of a move slot's flag byte: the slot is enabled for the AI (`+0xF4 + 6 * slot`).
const SLOT_AI_ENABLED: u8 = 0x04;
/// The area nibble classes of a move's AI targeting field the sim reads as the adjacent foes
/// (`02192728`: `0x00`, `0x10` and `0x20`).
const ADJACENT_AREAS: [u16; 3] = [0x00, 0x10, 0x20];

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
    // its own speed counters and paralysis are not read. A monster that spawns asleep is not
    // modelled. The `0x8000` and `0x4000` flags never get set.
    pub(crate) fn run_wild_monsters(&mut self, data: Option<&CombatData<'_>>) {
        let mut deferred = Vec::new();
        for i in 0..self.wild.len() {
            // A fallen leader ends the loop: the run is over.
            if !self.leader_alive() {
                break;
            }
            if self.wild_turn(i, data) {
                deferred.push(i);
            }
        }
        for i in deferred {
            self.monster_end_of_turn(i);
        }
    }

    /// One wild monster's turn: `RunMonsterAI`, Execute and the end-of-turn effects Execute runs.
    /// Returns whether the monster is flagged `aiAllySkip`, which defers the effects.
    fn wild_turn(&mut self, i: usize, data: Option<&CombatData<'_>>) -> bool {
        let Some(me) = self.wild.get(i).map(|m| m.pos) else {
            return false;
        };
        if self.team.is_empty() {
            return false;
        }
        // `TickStatusAndHealthRegen` comes first in the gated monster's turn (`docs/re/turns.md`,
        // "Other monsters' turns"). A wild monster has no HP regeneration (team members only).
        self.wild[i].info.tick_status();
        // A sleeper does not act (`0217FD78`).
        // shortcut: its turn draws nothing but the end-of-turn roll; whether `RunMonsterAI`
        // steps 1 to 9 draw for a sleeper is not read. A paralysed or flinching monster's attack
        // is refused (`02172ADC`), but the sim does not keep those statuses on a wild monster's
        // attack path, so nothing is refused here.
        if self.wild[i].info.is_asleep() {
            self.monster_end_of_turn(i);
            return false;
        }
        // `RunMonsterAI` steps 1 to 9 draw nothing for a monster with no status or item.
        let target = self.find_target(me);
        // shortcut: a wild monster the sim gave no moves (the spawn loads no learnset yet) keeps
        // the fixed pick of the no-data path; a real monster always holds a move.
        let has_moves = self.wild[i]
            .info
            .moves
            .iter()
            .any(|m| m.flags & MoveSlot::HOLDS_MOVE != 0);
        let attacked = match data.filter(|_| has_moves) {
            Some(data) => self.choose_ai_move(i, data),
            None => {
                // `ChooseAIMove`: the weighted pick is drawn whatever the target.
                let _pick = self.rng.int(MOVE_PICK_TOTAL);
                if let Some(t) = target.filter(|&t| self.adjacent_attackable(me, t)) {
                    // The regular attack: the target pick among one candidate (`02192CC0`).
                    let _candidate = self.rng.int(1);
                    self.wild[i].info.direction = direction_towards(me, t);
                    true
                } else {
                    false
                }
            }
        };
        if attacked {
            if self.wild.get(i).is_some() {
                self.monster_end_of_turn(i);
            }
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
            self.monster_end_of_turn(i);
        }
        skip
    }

    /// `ChooseAIMove` (`02192D24`) for wild monster `i`, then the action it picks. Returns whether
    /// the monster attacked (the end-of-turn follows) or not (it moves: `AIMovement`).
    ///
    /// The flow, in the game's order: the move slots are counted (`n` enabled, and the PP sum);
    /// with no PP at all the regular attack's target is searched and used, with no weighted pick.
    /// Otherwise each usable enabled move weighs its AI weight (`0200B428`), the regular attack
    /// weighs `table[n]` (`regular_attack_weight`), and `DungeonRandInt(total)` (`02193260`)
    /// picks; then the regular attack's target is searched (`02191F84`, one draw when a foe is
    /// adjacent); then the entry the roll lands on (the first with `cumulative >= roll`) is used:
    /// a move needs a target (`02192728`, one draw), and a move with none falls back to the
    /// regular attack when it has one.
    // shortcut: the gates before the move choice (`02181530`, `02180B68`, `021805CC(8)`, the
    // confusion roll of `+0xAC`) and the bide block (`+0xAE`) read statuses the sim keeps no
    // field for, so they pass. No IQ skill is set (PP Checker, Weak-Type Picker, Exclusive
    // Move-User), as the sim derives none from the monster's IQ (`+0x87`).
    // shortcut: a move is usable (`02159E58`) when it has PP; its other tests are not read.
    // shortcut: a move's target search (`02192728`) is modelled for the adjacent areas of its AI
    // field (`0x00`, `0x10`, `0x20`), every foe weighing 1 (`021921B0`'s type weights are not
    // read), and for moves `move_effects` models. A move with another area, a non-zero condition
    // nibble (`02192274`, which can draw) or no modelled effect finds no target, so the monster
    // falls back to the regular attack. The game's eligibility filters beyond adjacency
    // (`02192508`) are read as passing.
    fn choose_ai_move(&mut self, i: usize, data: &CombatData<'_>) -> bool {
        let me = Slot::Wild(i);
        let slots = self.wild[i].info.moves;
        let enabled = slots
            .iter()
            .filter(|m| m.flags & MoveSlot::HOLDS_MOVE != 0 && m.flags & SLOT_AI_ENABLED != 0)
            .count();
        let pp_sum: u32 = slots
            .iter()
            .filter(|m| m.flags & MoveSlot::HOLDS_MOVE != 0)
            .map(|m| u32::from(m.pp))
            .sum();
        if pp_sum == 0 {
            // `0200B568(0x160)` then `02192728`: the regular attack, no weighted pick.
            return match self.regular_attack_direction(i) {
                Some(dir) => self.use_regular_attack(i, dir, data),
                None => false,
            };
        }
        // The weights: slots 0 to 3 by `GetMoveAIWeight`, then the regular attack.
        let mut weights = [0i32; 5];
        for (w, slot) in weights.iter_mut().zip(slots.iter()) {
            let usable = slot.flags & MoveSlot::HOLDS_MOVE != 0
                && slot.flags & SLOT_AI_ENABLED != 0
                && slot.pp > 0;
            if usable {
                *w = data
                    .moves
                    .get(usize::from(slot.id))
                    .map_or(0, |mv| i32::from(mv.ai_weight));
            }
        }
        weights[REGULAR_ENTRY] = data
            .tables
            .regular_attack_weight
            .get(enabled)
            .map_or(0, |&w| i32::from(w));
        let total: i32 = weights.iter().sum();
        if total <= 0 {
            return false;
        }
        let roll = self.rng.int(total);
        // `02191F84`: the regular attack's target, searched whatever the roll picks.
        let regular = self.regular_attack_direction(i);
        let mut cumulative = 0;
        for (entry, &w) in weights.iter().enumerate() {
            if w == 0 {
                continue;
            }
            cumulative += w;
            if roll > cumulative {
                continue;
            }
            if entry == REGULAR_ENTRY {
                return regular.is_some_and(|dir| self.use_regular_attack(i, dir, data));
            }
            if let Some(dir) = self.move_target_direction(i, entry, data) {
                return self.use_move_slot(me, entry, dir, data);
            }
            break;
        }
        regular.is_some_and(|dir| self.use_regular_attack(i, dir, data))
    }

    /// The directions (indices into [`DIRECTIONS`], in table order) of the adjacent foes monster
    /// `i` can hit with a regular attack: the team members one tile away with no wall corner
    /// between (`02192728`, the `02180FFC` test).
    fn adjacent_foe_directions(&self, i: usize) -> Vec<usize> {
        let me = self.wild[i].pos;
        (0..DIRECTIONS.len())
            .filter(|&d| {
                let (dx, dy) = DIRECTIONS[d].delta();
                let at = (me.0 + dx as i16, me.1 + dy as i16);
                self.team.iter().any(|t| t.pos == at && t.info.hp > 0)
                    && self.corner_open(i32::from(me.0), i32::from(me.1), dx, dy)
            })
            .collect()
    }

    /// The weighted target pick among `dirs`, every foe weighing 1: one `DungeonRandInt(count)`
    /// (`0219D058` at `02192CC0`), the first candidate whose running sum passes the roll.
    fn pick_direction(&mut self, dirs: &[usize]) -> Option<usize> {
        if dirs.is_empty() {
            return None;
        }
        let roll = self.rng.int(dirs.len() as i32);
        dirs.get(usize::try_from(roll).unwrap_or(0)).copied()
    }

    /// `02191F84`: the regular attack's direction, drawing the target pick when a foe is adjacent.
    fn regular_attack_direction(&mut self, i: usize) -> Option<usize> {
        let dirs = self.adjacent_foe_directions(i);
        self.pick_direction(&dirs)
    }

    /// `02192728` for the move in slot `slot` of monster `i`: the direction of the foe it would
    /// use the move on, with the target pick drawn, or `None` when the move has no target.
    fn move_target_direction(
        &mut self,
        i: usize,
        slot: usize,
        data: &CombatData<'_>,
    ) -> Option<usize> {
        let id = self.wild[i].info.moves[slot].id;
        let mv = data.moves.get(usize::from(id))?;
        let ai = mv.targeting[1];
        if ai & 0x0F00 != 0 || !ADJACENT_AREAS.contains(&(ai & 0xF0)) {
            return None;
        }
        move_effects::effect_of(id)?;
        let dirs = self.adjacent_foe_directions(i);
        self.pick_direction(&dirs)
    }

    /// The regular attack by wild monster `i` towards direction index `dir`: the monster turns to
    /// face it and the regular attack's damage resolves on the foes its range picks.
    fn use_regular_attack(&mut self, i: usize, dir: usize, data: &CombatData<'_>) -> bool {
        self.wild[i].info.direction = dir as u8;
        let me = Slot::Wild(i);
        let Some(mv) = data.moves.get(usize::from(REGULAR_ATTACK)) else {
            return true;
        };
        let effect = Effect::Damage {
            arg8: REGULAR_ATTACK_ARG8,
        };
        if let Some(targets) = self
            .targets_of(me, mv, DIRECTIONS[dir])
            .filter(|t| !t.is_empty())
        {
            self.resolve_all(me, &targets, mv, &effect, data);
        }
        true
    }

    /// The move in `slot` of `user` used towards direction index `dir`, with its PP.
    fn use_move_slot(
        &mut self,
        user: Slot,
        slot: usize,
        dir: usize,
        data: &CombatData<'_>,
    ) -> bool {
        let Slot::Wild(i) = user else {
            return false;
        };
        self.wild[i].info.direction = dir as u8;
        let id = self.wild[i].info.moves[slot].id;
        let (Some(mv), Some(effect)) =
            (data.moves.get(usize::from(id)), move_effects::effect_of(id))
        else {
            return false;
        };
        if let Some(targets) = self
            .targets_of(user, mv, DIRECTIONS[dir])
            .filter(|t| !t.is_empty())
        {
            self.strike(user, slot, &targets, mv, &effect, data);
        }
        true
    }

    /// `DoEndOfTurnEffects` for a wild monster: the one draw it makes, `DungeonRandInt(100)`
    /// (`02186894`).
    // shortcut: a wild monster has no belly, so the roll and the status part are kept (see
    // `leader_end_of_turn`).
    fn monster_end_of_turn(&mut self, i: usize) {
        let _roll = self.rng.int(100);
        self.wild[i].info.end_of_turn_status();
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
    use crate::state::{EntityInfo, GameState, Monster, wild_monster};

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
            status: Default::default(),
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

    /// The weights of the combat tests' tables for 0 to 4 enabled slots (their own values).
    const REGULAR: [i32; 5] = [9, 7, 5, 3, 1];

    /// A leader at (10, 10) with a wild monster beside it at (11, 10) that holds `moves` (the
    /// move ID, the AI flag byte and the PP of each slot), and the combat data's move table.
    fn duel(
        moves: &[(u16, u8, u8)],
        weights: &[(u16, u8)],
    ) -> (GameState, Vec<rescue_data::moves::Move>) {
        let mut g = room_with((10, 10), &[(11, 10)]);
        let d = g.dungeon_mut().unwrap();
        for (slot, &(id, flags, pp)) in d.wild[0].info.moves.iter_mut().zip(moves) {
            *slot = MoveSlot {
                flags,
                flags2: 0,
                id,
                pp,
                ginseng: 0,
            };
        }
        let mut list = vec![crate::combat::tests::move_of(
            REGULAR_ATTACK,
            1,
            0,
            [100; 3],
            0,
        )];
        for &(id, weight) in weights {
            let mut mv = crate::combat::tests::move_of(id, 40, 1, [100; 3], 0);
            mv.ai_weight = weight;
            list.push(mv);
        }
        (g, crate::combat::tests::table(&list))
    }

    /// Runs the wild loop with `seed` and checks the draws against the game's order: `pick`
    /// (the weighted pick's total, if any), the regular attack's target draw when `regular_target`,
    /// then `used`, the move slot the monster is expected to use (the move's own target draw, the
    /// use and its PP) or the regular attack's resolve, then the end-of-turn roll. Returns the
    /// dungeon.
    fn check_turn(
        g: &GameState,
        moves: &[rescue_data::moves::Move],
        seed: u32,
        pick: Option<i32>,
        used: impl Fn(i32) -> Option<usize>,
        regular_target: bool,
    ) -> Dungeon {
        let tables = crate::combat::tests::tables();
        let data = CombatData {
            tables: &tables,
            moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut actual = g.dungeon().unwrap().clone();
        actual.rng = crate::rng::DungeonRng::new(seed);
        let mut model = actual.clone();
        actual.run_wild_monsters(Some(&data));

        let roll = pick.map_or(0, |total| model.rng.int(total));
        if regular_target {
            model.rng.int(1);
        }
        let attack = Effect::Damage {
            arg8: REGULAR_ATTACK_ARG8,
        };
        match used(roll) {
            Some(slot) => {
                model.rng.int(1);
                let id = model.wild[0].info.moves[slot].id;
                let mv = &moves[usize::from(id)];
                let effect = move_effects::effect_of(id).unwrap();
                model.strike(Slot::Wild(0), slot, &[Slot::Team(0)], mv, &effect, &data);
            }
            None => {
                let mv = &moves[usize::from(REGULAR_ATTACK)];
                model.resolve_all(Slot::Wild(0), &[Slot::Team(0)], mv, &attack, &data);
            }
        }
        model.rng.int(100);
        assert_eq!(actual.rng, model.rng, "seed {seed}");
        assert_eq!(actual.team[0].info.hp, model.team[0].info.hp, "seed {seed}");
        assert_eq!(
            actual.wild[0].info.moves, model.wild[0].info.moves,
            "seed {seed}"
        );
        actual
    }

    #[test]
    fn one_usable_move_weighs_its_ai_weight_against_the_regular_attacks_table_entry() {
        // One enabled move of weight 6: the total is 6 + table[1], and the pick uses the move
        // when the roll is within its weight, the regular attack otherwise.
        let (g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 6)]);
        let total = 6 + REGULAR[1];
        let (mut by_move, mut by_regular) = (false, false);
        for seed in 0..64 {
            let d = check_turn(
                &g,
                &moves,
                seed,
                Some(total),
                |roll| (roll <= 6).then_some(0),
                true,
            );
            let used = d.wild[0].info.moves[0].pp == 9;
            by_move |= used;
            by_regular |= !used;
            assert_eq!(d.wild[0].pos, (11, 10), "an attack does not move");
            assert_eq!(d.wild[0].info.direction, 6, "it faces the leader");
        }
        assert!(by_move && by_regular, "both branches must be reached");
    }

    #[test]
    fn several_usable_moves_pick_by_running_sum_in_slot_order() {
        // Two enabled moves of weights 3 and 4: n = 2, total 3 + 4 + table[2].
        let (g, moves) = duel(&[(0x9A, 5, 10), (0x104, 5, 10)], &[(0x9A, 3), (0x104, 4)]);
        let total = 3 + 4 + REGULAR[2];
        let mut seen = [false; 3];
        for seed in 0..96 {
            let d = check_turn(
                &g,
                &moves,
                seed,
                Some(total),
                |roll| match roll {
                    ..=3 => Some(0),
                    4..=7 => Some(1),
                    _ => None,
                },
                true,
            );
            let pps = (d.wild[0].info.moves[0].pp, d.wild[0].info.moves[1].pp);
            seen[match pps {
                (9, 10) => 0,
                (10, 9) => 1,
                _ => 2,
            }] = true;
        }
        assert_eq!(seen, [true; 3]);
    }

    #[test]
    fn with_no_enabled_move_the_regular_attack_is_picked_by_table_entry_0() {
        // The slots exist and have PP but none is enabled for the AI (bit 2 clear): the total is
        // table[0], so the roll always lands on the regular attack.
        let (g, moves) = duel(&[(0x9A, 1, 10)], &[(0x9A, 6)]);
        for seed in 0..8 {
            let d = check_turn(&g, &moves, seed, Some(REGULAR[0]), |_| None, true);
            assert_eq!(d.wild[0].info.moves[0].pp, 10);
        }
    }

    #[test]
    fn a_monster_with_no_pp_skips_the_pick_and_attacks_with_the_regular_attack() {
        let (g, moves) = duel(&[(0x9A, 5, 0)], &[(0x9A, 6)]);
        for seed in 0..8 {
            check_turn(&g, &moves, seed, None, |_| None, true);
        }
    }

    #[test]
    fn with_no_foe_adjacent_the_pick_is_drawn_and_nothing_else_attacks() {
        let (mut g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 6)]);
        g.dungeon_mut().unwrap().wild[0].pos = (14, 10);
        let tables = crate::combat::tests::tables();
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_wild_monsters(Some(&data));
        // The pick, then the end-of-turn roll; the walk draws nothing.
        expected.int(6 + REGULAR[1]);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.team[0].info.hp, 30);
        assert_eq!(d.wild[0].pos, (13, 10));
    }

    #[test]
    fn a_wild_attack_that_defeats_the_leader_ends_the_command_with_its_defeat() {
        let (mut g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 6)]);
        let d = g.dungeon_mut().unwrap();
        d.team[0].info.hp = 1;
        d.team[0].info.defence = [0, 0];
        d.wild[0].info.attack = [200, 200];
        let tables = crate::combat::tests::tables();
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut defeated = false;
        for _ in 0..30 {
            if g.step_with(Command::Wait, &data) == crate::command::Outcome::LeaderDefeated {
                defeated = true;
                break;
            }
        }
        assert!(
            defeated,
            "the wild monster's hits must take the leader down"
        );
        assert!(g.dungeon().is_none());
    }

    #[test]
    fn a_walk_turn_draws_the_pick_and_the_end_of_turn_and_steps_toward_the_leader() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_wild_monsters(None);
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
        d.run_wild_monsters(None);
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
        d.run_wild_monsters(None);
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
        d.run_wild_monsters(None);
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

    #[test]
    fn a_sleeping_monster_draws_only_the_end_of_turn_roll_and_stays_put() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.wild[0].info.status.sleep = 1;
        d.wild[0].info.status.sleep_turns = 3;
        let mut expected = d.rng;
        d.run_wild_monsters(None);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.wild[0].pos, (5, 5));
        // Its countdown ran first, in its own turn.
        assert_eq!(d.wild[0].info.status.sleep_turns, 2);
    }

    #[test]
    fn a_monster_wakes_when_its_last_sleep_turn_is_ticked_and_acts_that_turn() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.wild[0].info.status.sleep = 1;
        d.wild[0].info.status.sleep_turns = 1;
        d.run_wild_monsters(None);
        assert_eq!(d.wild[0].info.status.sleep, 0);
        assert_eq!(d.wild[0].pos, (6, 5));
    }

    #[test]
    fn a_burned_wild_monster_takes_5_after_its_end_of_turn_roll() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        let hp = d.wild[0].info.hp;
        d.wild[0].info.status.burn_class = 1;
        d.wild[0].info.status.burn_turns = 128;
        d.run_wild_monsters(None);
        assert_eq!(d.wild[0].info.hp, hp - 5);
        assert_eq!(d.wild[0].info.status.burn_turns, 127);
    }
}
