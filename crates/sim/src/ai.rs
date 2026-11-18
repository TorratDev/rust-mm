//! A wild monster's turn (`docs/re/ai.md`, "Decision flow", "Movement and pathfinding", "RNG
//! calls"; `docs/re/turns.md`, "Other monsters' turns", "One enemy turn (#358 P7)"): the wild
//! loop `021408F8` runs `RunMonsterAI` (`02188030`) and Execute for each wild monster that is
//! gated in, then the end-of-turn effects of the ones left flagged `aiAllySkip`. The draws come
//! in the game's order. Integer math only.
//!
//! A partner's turn (`02140AC0`) is the same flow with the sides swapped: it attacks a foe it sees
//! next to it, else heads for the foe or the leader.
//!
//! A turn is: the weighted move pick (`02192D24`, one `DungeonRandInt`), then either the regular
//! attack (the leader is next to the monster) or `AIMovement` (`021918D4`): the target search
//! (`02191394`), the wander (`02190854`) when it finds none, and `DecideMovement` (`02190FEC`).
//! Execute then draws the end-of-turn roll (`02186894`), unless the monster is flagged
//! `aiAllySkip`, which defers it to the end of the loop.

use std::collections::BTreeMap;

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
/// The weight total team partners showed in the P7 trace (`docs/re/turns.md`, "One enemy turn").
const TEAM_PICK_TOTAL: i32 = 65;

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

/// AI objectives (`+0x70`; the names are Red's, see `docs/re/structs.md`, "AI target readers").
const CHASE_TARGET: u8 = 1;
const CHASE_REMEMBERED: u8 = 2;
const ROAM: u8 = 3;
const LEAVE_ROOM: u8 = 4;
const STAND_STILL: u8 = 6;

/// The persistent part of a monster's AI record: the objective (`+0x70`), the goal tile
/// (`+0x80`, `+0x82`) and the target (`+0x78`, a team slot).
// shortcut: the generation ID (`+0x74`) is not kept: a team slot is never reused on a floor
// here, so the ID always matches. `+0xF0` (move randomly) is read as 0: the notes give no writer
// that sets it. The record is cleared nowhere else (`0216C638`, petrify and decoy resets are not
// modelled, as the sim has no such statuses).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiRecord {
    objective: u8,
    goal: (i16, i16),
    target: Option<usize>,
}

/// A team member's AI record and its last four positions (`info+0x5E`, written by the move
/// helper `02177700`: the tiles it left, newest first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TeamAi {
    rec: AiRecord,
    last: (i16, i16),
    prev: [(i16, i16); 4],
}

/// The AI state of a floor: the wild monsters' records and the team's.
// shortcut: a wild monster's record is keyed by its tile (monsters have no ID here, and other
// modules remove and add them), so a monster moved by something other than its own walk starts
// a fresh record, and a monster that spawns on the tile of one that just fell inherits its
// record until the next wild loop prunes it.
// shortcut: `DungeonSnapshot` (its literals are in `apps/trace-harness`) does not hold this
// state, so a restored floor starts with fresh records and trails (#467), and equality ignores
// it, so a snapshot round trip compares equal.
#[derive(Clone, Debug, Default)]
pub(crate) struct AiState {
    wild: BTreeMap<(i16, i16), AiRecord>,
    team: Vec<TeamAi>,
    /// Team slots flagged `0x8000` by a leader swap, until the swap pass (`02140ED0`) runs.
    pub(crate) swapping: Vec<usize>,
    /// Team slots flagged `0x4000`: the team loop skips each once and clears the flag.
    held: Vec<usize>,
}

impl PartialEq for AiState {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for AiState {}

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
        self.sync_team_ai();
        let live: Vec<_> = self.wild.iter().map(|m| m.pos).collect();
        self.ai.wild.retain(|p, _| live.contains(p));
        let mut deferred = Vec::new();
        let mut i = 0;
        while i < self.wild.len() {
            // A fallen leader ends the loop: the run is over.
            if !self.leader_alive() {
                break;
            }
            let before = self.wild.len();
            if self.wild_turn(i, data) {
                deferred.push(i);
            }
            // Struggle's recoil can remove the acting monster (`remove_defeated` compacts the
            // list): the next monster now sits at `i`, and the deferred indices all lie below it.
            if self.wild.len() == before {
                i += 1;
            }
        }
        for i in deferred {
            self.monster_end_of_turn(i);
        }
    }

    /// The team loop (`02140AC0`, `docs/re/turns.md`, "Other monsters' turns"): each gated
    /// partner acts once in slot order (phase 1), then the members flagged `aiAllySkip` act again
    /// in up to three passes, nearest the leader first, last slot first within a distance
    /// (phase 2); each member still flagged at the end gets its end-of-turn effects.
    // shortcut: every partner is gated at speed stage 1 (the caller picks the gated turns), and
    // runs one `RunMonsterAI` and Execute cycle where the game repeats up to three while Execute
    // returns TRUE (what makes it return TRUE is not decoded).
    // The `0x4000` gate (phase 1) skips a partner the swap pass just ran, see
    // [`Self::run_swap_pass`] (`docs/re/turns.md`, "Swap flags (#181 P2)").
    pub(crate) fn run_team_members(&mut self, data: Option<&CombatData<'_>>) {
        self.sync_team_ai();
        // The slots flagged `aiAllySkip`.
        let mut skipped = Vec::new();
        for i in 1..self.team.len() {
            if !self.leader_alive() {
                return;
            }
            if let Some(h) = self.ai.held.iter().position(|&s| s == i) {
                self.ai.held.remove(h);
                continue;
            }
            if self.ally_turn(i, data, false) {
                skipped.push(i);
            }
        }
        for _ in 0..3 {
            let leader = self.team[0].pos;
            let mut order = std::mem::take(&mut skipped);
            // The bucket is the distance to the leader capped at 2; each bucket is LIFO.
            order.sort_by_key(|&i| (chebyshev(leader, self.team[i].pos).min(2), usize::MAX - i));
            for i in order {
                if !self.leader_alive() {
                    return;
                }
                if self.ally_turn(i, data, true) {
                    skipped.push(i);
                }
            }
        }
        for i in skipped {
            self.ally_end_of_turn(i);
        }
    }

    /// The swap pass (`02140ED0`, called at `0214114C` in the leader's turn): each team ally the
    /// leader swapped with (flag `0x8000`) gets the status tick and Execute, then its flag becomes
    /// `0x4000`, which the team loop skips once. `RunMonsterAI` returns at once on `0x8000`, so
    /// only Execute's end-of-turn draw (`Int(100)`) happens: no pick, target search or move.
    // shortcut: the pass is gated by `gDungeon+0xE`, which the swap sets, so it runs when
    // `swapping` is non-empty. Its early return is `IsFloorOver`, checked per entity; the sim
    // checks the leader's fall instead. Its `TickStatusAndHealthRegen` is the status tick only.
    pub(crate) fn run_swap_pass(&mut self) {
        for i in std::mem::take(&mut self.ai.swapping) {
            if !self.leader_alive() {
                return;
            }
            if self.team[i].info.hp != 0 {
                self.team[i].info.tick_status();
                // Asleep or not, Execute still runs the end of turn.
                self.ally_end_of_turn(i);
            }
            self.ai.held.push(i);
        }
    }

    /// Brings the team's position trails up to date: a member found away from its last position
    /// has moved, so that position enters its trail (`02177700`, a walk; a teleport would fill
    /// all four, not modelled). Called at the start of each monster loop.
    // shortcut: one shift per loop however many tiles the member moved since.
    fn sync_team_ai(&mut self) {
        self.ai.team.truncate(self.team.len());
        for (slot, m) in self.team.iter().enumerate() {
            if let Some(t) = self.ai.team.get_mut(slot) {
                if t.last != m.pos {
                    t.prev = [t.last, t.prev[0], t.prev[1], t.prev[2]];
                    t.last = m.pos;
                }
            } else {
                self.ai.team.push(TeamAi {
                    last: m.pos,
                    prev: [m.pos; 4],
                    ..TeamAi::default()
                });
            }
        }
    }

    /// One partner's turn: `RunMonsterAI` and Execute. `recalc` is phase 2's `+0x117`. Returns
    /// whether the partner is flagged `aiAllySkip`, which defers its end of turn.
    // shortcut: the partner's own moves are not used: the pick draws the team total 65 seen in
    // the P7 trace (the sum of its five weights is not computed) and only the regular attack
    // is used. IQ skills and tactics are not read (`ai.md`, P4-b, P4-c): the partner attacks a
    // foe it sees, else follows the leader, whatever its tactic. A phase-2 turn never re-flags
    // (`recalculateFollow` set, `DecideMovement` lines 358-361), though the trailing branch
    // (lines 436-443) may in the game.
    // shortcut: HP regeneration (`TickStatusAndHealthRegen`) and the partner's belly are not
    // modelled; only the status countdowns run.
    fn ally_turn(&mut self, i: usize, data: Option<&CombatData<'_>>, recalc: bool) -> bool {
        let me = self.team[i].pos;
        if self.team[i].info.hp == 0 {
            return false;
        }
        self.team[i].info.tick_status();
        if self.team[i].info.is_asleep() {
            self.ally_end_of_turn(i);
            return false;
        }
        // `AIFindTarget` (`02191394`): the foe search is gated by `02184074`, which for a team
        // member reads the byte table `0x02091E48` at `info+0x8A` (the tactic); entry 0 (the
        // default tactic) is 1, so the search is skipped and the target is the leader if in view
        // (`02183FF0`, `+0x72` = 0). `docs/re/ai.md` P3-c, #348 row 8 findings.
        // shortcut: every partner is read as tactic 0 (the sim keeps no tactic byte); the table
        // reads 1,0,0,0,1,1,1,1 for entries 0 to 7 (entries 1 to 3 would search a foe, the rest
        // were not read, entries 8+ not read at all). Upgrade with the tactic field.
        let leader = self.team[0].pos;
        let foe = self.nearest_seen(me, self.wild.iter().map(|m| m.pos));
        let target = self.in_view(me, leader).then_some(leader);
        // `ChooseAIMove`: the weighted pick is drawn whatever the target.
        let _pick = self.rng.int(TEAM_PICK_TOTAL);
        if let Some(f) = foe.filter(|&f| self.adjacent_attackable(me, f)) {
            // The regular attack's target pick among one candidate (`02192CC0`).
            let _candidate = self.rng.int(1);
            let dir = usize::from(direction_towards(me, f));
            self.team[i].info.direction = dir as u8;
            if let Some(data) = data {
                self.regular_attack_hits(Slot::Team(i), dir, data);
            }
            self.ally_end_of_turn(i);
            return false;
        }
        // shortcut: [hypothesis] a follow (walk) turn runs two `RunMonsterAI`/Execute cycles, the
        // P7 team row (`Int(65), Int(65), Int(100)`): Execute returns 1 only on the early
        // `DisplayActions_Async` path, before the action runs, so the first cycle does not walk
        // and the second does. Not checked beyond that trace row. The attack turn above stays at
        // one pick: the P7 "Attack" row shows one `Int(65)` for the 3 team attacks.
        let _second_pick = self.rng.int(TEAM_PICK_TOTAL);
        let facing = usize::from(self.team[i].info.direction);
        let mut rec = self.ai.team.get(i).map(|t| t.rec).unwrap_or_default();
        let goal = match target {
            Some(t) => {
                // shortcut: a partner that loses sight does not follow remembered positions: it
                // keeps no target, so its objective 1 has none to follow.
                rec = AiRecord {
                    objective: CHASE_TARGET,
                    goal: t,
                    target: None,
                };
                t
            }
            None => self.wander(me, facing, &mut rec),
        };
        if let Some(t) = self.ai.team.get_mut(i) {
            t.rec = rec;
        }
        let (plan, skip) = self.decide_movement(me, goal);
        if let Plan::Walk(dir) = plan {
            let m = &mut self.team[i];
            m.pos = step(m.pos, dir);
            m.info.direction = dir as u8;
        }
        let skip = skip && !recalc;
        if !skip {
            self.ally_end_of_turn(i);
        }
        skip
    }

    /// The game's floor-generation AI step for the initial wild monsters (`02179CFC` ->
    /// `021918D4` -> `02190854`, the `Int(exits)` draw at `02190A10`): one wander per monster in
    /// slot order, which leaves objective 4 (leave the room) and the exit as its goal. With
    /// `draw` false (a restored floor: the RNG is already past generation) nothing is drawn and
    /// the first exit that is not the monster's tile stands in for the draw.
    // shortcut: a monster outside a room gets no record (its generation-time wander is not read);
    // `draw` false takes the first exit where the game drew one (#348), so such a monster can
    // head for another exit than the game's. Upgrade with the snapshot holding the AI records.
    pub(crate) fn seed_wild_ai(&mut self, draw: bool) {
        for i in 0..self.wild.len() {
            let me = self.wild[i].pos;
            let room = self
                .floor
                .tile(i32::from(me.0), i32::from(me.1))
                .map_or(NO_ROOM, |t| t.room);
            if room == NO_ROOM {
                continue;
            }
            let mut rec = AiRecord::default();
            if draw {
                let facing = usize::from(self.wild[i].info.direction);
                self.wander(me, facing, &mut rec);
            } else if let Some(&exit) = self.floor.room_exits(room).iter().find(|&&e| e != me) {
                rec.objective = LEAVE_ROOM;
                rec.goal = exit;
            }
            if rec.objective == LEAVE_ROOM {
                self.ai.wild.insert(me, rec);
            }
        }
    }

    /// `DoEndOfTurnEffects` for a partner: the one draw it makes, `DungeonRandInt(100)`
    /// (`02186894`), then the status part (see [`Self::monster_end_of_turn`]).
    fn ally_end_of_turn(&mut self, i: usize) {
        let _roll = self.rng.int(100);
        self.team[i].info.end_of_turn_status();
    }

    /// One wild monster's turn: `RunMonsterAI`, Execute and the end-of-turn effects Execute runs.
    /// Returns whether the monster is flagged `aiAllySkip`, which defers the effects.
    fn wild_turn(&mut self, i: usize, data: Option<&CombatData<'_>>) -> bool {
        let Some(me) = self.wild.get(i).map(|m| m.pos) else {
            return false;
        };
        let len = self.wild.len();
        let mut rec = self.ai.wild.remove(&me).unwrap_or_default();
        let skip = self.wild_turn_with(i, data, &mut rec);
        // A monster the recoil removed leaves `i` naming the next one: its record is not ours.
        if self.wild.len() == len {
            self.ai.wild.insert(self.wild[i].pos, rec);
        }
        skip
    }

    /// [`Self::wild_turn`] with the monster's AI record.
    fn wild_turn_with(
        &mut self,
        i: usize,
        data: Option<&CombatData<'_>>,
        rec: &mut AiRecord,
    ) -> bool {
        let me = self.wild[i].pos;
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
        let len_before = self.wild.len();
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
            // A monster the recoil removed gets no end of turn (`i` now names the next one).
            if self.wild.len() == len_before {
                self.monster_end_of_turn(i);
            }
            return false;
        }
        let facing = usize::from(self.wild[i].info.direction);
        let goal = self.movement_goal(me, facing, target, rec);
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
            // `InitPokemonMove(0x160)` then `02192728`, action 0x17: Struggle, no weighted pick.
            // The target search draws as the regular attack's does when a foe is adjacent.
            return match self.regular_attack_direction(i) {
                Some(dir) => self.use_struggle(i, dir, data),
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
        self.regular_attack_hits(Slot::Wild(i), dir, data)
    }

    /// The damage of a regular attack by `me` towards direction index `dir` (either side).
    fn regular_attack_hits(&mut self, me: Slot, dir: usize, data: &CombatData<'_>) -> bool {
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

    /// Struggle (`0x160`) by wild monster `i` towards direction index `dir`: the normal damage
    /// path with the recoil on the user, and no PP taken.
    fn use_struggle(&mut self, i: usize, dir: usize, data: &CombatData<'_>) -> bool {
        self.wild[i].info.direction = dir as u8;
        let me = Slot::Wild(i);
        let Some(mv) = data.moves.get(usize::from(move_effects::STRUGGLE)) else {
            return true;
        };
        if let Some(targets) = self
            .targets_of(me, mv, DIRECTIONS[dir])
            .filter(|t| !t.is_empty())
        {
            self.resolve_all(me, &targets, mv, &Effect::Struggle, data);
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
    // Sight is `in_view` (`0219C6BC`); the game's wild-monster test is `0219C5C4`, not read. The
    // behaviour, petrified and decoy filters are not modelled (no statuses).
    fn find_target(&self, me: (i16, i16)) -> Option<(i16, i16)> {
        self.nearest_seen(
            me,
            self.team.iter().filter(|t| t.info.hp > 0).map(|t| t.pos),
        )
    }

    /// The nearest of `foes` that `me` sees, the first at the smallest distance, stopping at one
    /// within 1 tile.
    fn nearest_seen(
        &self,
        me: (i16, i16),
        foes: impl Iterator<Item = (i16, i16)>,
    ) -> Option<(i16, i16)> {
        let mut best: Option<((i16, i16), i32)> = None;
        for pos in foes {
            if !self.in_view(me, pos) {
                continue;
            }
            let dist = chebyshev(me, pos);
            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((pos, dist));
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

    /// `AIFindTarget` (`02191394`) after the candidate loop, for a wild monster at `me` that saw
    /// `seen` (or nothing): the goal tile, with the record updated. A seen target is objective 1.
    /// Otherwise a monster that was chasing (objective 1 or 2) with a target takes the first of
    /// the target's four previous positions it can see (objective 2); with none it wanders.
    // shortcut: "can see" is `in_view`, not `02141AD0` (`0219C5C4`, not read). The tactic and
    // leader branches (`+7`, tactic 1) do not apply to a wild monster.
    fn movement_goal(
        &mut self,
        me: (i16, i16),
        facing: usize,
        seen: Option<(i16, i16)>,
        rec: &mut AiRecord,
    ) -> (i16, i16) {
        if let Some(t) = seen {
            let slot = self.team.iter().position(|m| m.pos == t && m.info.hp > 0);
            *rec = AiRecord {
                objective: CHASE_TARGET,
                goal: t,
                target: slot,
            };
            return t;
        }
        let chasing = matches!(rec.objective, CHASE_TARGET | CHASE_REMEMBERED);
        if let Some(slot) = rec.target.filter(|_| chasing) {
            let prev = self.ai.team.get(slot).map_or([me; 4], |t| t.prev);
            if let Some(p) = prev.into_iter().find(|&p| self.in_view(me, p)) {
                rec.objective = CHASE_REMEMBERED;
                rec.goal = p;
                return p;
            }
        }
        self.wander(me, facing, rec)
    }

    /// `AIWander` (`02190854`): the tile the monster heads to, written to `rec` with the
    /// objective. A corridor tile picks the first direction that can be walked (objective 3) or a
    /// random neighbour (6). A room tile heads for one of the room's exits (objective 4, kept
    /// until the monster is chosen again), or, standing on a junction, steps out to the corridor
    /// (3); a room with no exits draws a random neighbour (6). When no branch fires the record
    /// is left as it was, as the game's is.
    // shortcut: the mobility class is 0 (a regular walker) and `+0xF0` is read as 0 (see
    // [`AiRecord`]).
    fn wander(&mut self, me: (i16, i16), facing: usize, rec: &mut AiRecord) -> (i16, i16) {
        let facing = facing % 8;
        let tile = self.floor.tile(i32::from(me.0), i32::from(me.1));
        let room = tile.map_or(NO_ROOM, |t| t.room);
        let on_junction = tile.is_some_and(|t| t.terrain & floor::TERRAIN_JUNCTION != 0);
        if room == NO_ROOM {
            let jitter = tile.is_some_and(|t| STRAIGHT_MASKS.contains(&t.walkable_neighbors[0]));
            let mut dir = facing;
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
                    *rec = AiRecord {
                        objective: ROAM,
                        goal: step(me, d),
                        ..*rec
                    };
                    return rec.goal;
                }
            }
            return self.stand_still(me, rec);
        }
        if rec.objective != LEAVE_ROOM {
            let exits = self.floor.room_exits(room);
            if exits.is_empty() {
                return self.stand_still(me, rec);
            }
            for _ in 0..10 {
                let exit = exits[self.rng.int(exits.len() as i32) as usize];
                if exit != me {
                    rec.objective = LEAVE_ROOM;
                    rec.goal = exit;
                    return exit;
                }
            }
        }
        if on_junction {
            let mut d = self.rng.int(8) as usize;
            for _ in 0..8 {
                d &= 7;
                let n = step(me, d);
                let in_corridor = self
                    .floor
                    .tile(i32::from(n.0), i32::from(n.1))
                    .is_some_and(|t| t.room == NO_ROOM);
                if in_corridor && self.can_move(me, d).0 {
                    rec.objective = ROAM;
                    rec.goal = n;
                    return n;
                }
                d += 1;
            }
        }
        rec.goal
    }

    /// The wander's last resort: a random neighbour as the goal, objective 6.
    fn stand_still(&mut self, me: (i16, i16), rec: &mut AiRecord) -> (i16, i16) {
        rec.objective = STAND_STILL;
        rec.goal = step(me, self.rng.int(8) as usize);
        rec.goal
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
    fn a_monster_with_no_pp_skips_the_pick_and_uses_struggle_with_its_recoil() {
        let (g, moves) = duel(&[(0x9A, 5, 0)], &[(0x9A, 6), (move_effects::STRUGGLE, 0)]);
        let tables = crate::combat::tests::tables();
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        for seed in 0..8 {
            let mut actual = g.dungeon().unwrap().clone();
            actual.rng = crate::rng::DungeonRng::new(seed);
            let mut model = actual.clone();
            actual.run_wild_monsters(Some(&data));
            // No pick; the target draw, Struggle's resolve and recoil, then the end-of-turn roll.
            model.rng.int(1);
            let mv = &moves[usize::from(move_effects::STRUGGLE)];
            model.resolve_all(
                Slot::Wild(0),
                &[Slot::Team(0)],
                mv,
                &Effect::Struggle,
                &data,
            );
            model.rng.int(100);
            assert_eq!(actual.rng, model.rng, "seed {seed}");
            assert_eq!(actual.team[0].info.hp, model.team[0].info.hp, "seed {seed}");
            assert_eq!(actual.wild[0].info.hp, model.wild[0].info.hp, "seed {seed}");
            assert_eq!(actual.wild[0].info.moves, model.wild[0].info.moves);
        }
    }

    #[test]
    fn a_wild_monster_removed_by_recoil_does_not_make_the_next_one_skip_its_turn() {
        let mut g = room_with((10, 10), &[(11, 10), (9, 10)]);
        let moves = duel(&[], &[(move_effects::STRUGGLE, 0)]).1;
        let d = g.dungeon_mut().unwrap();
        for w in &mut d.wild {
            w.info.moves[0] = MoveSlot {
                flags: 0x01,
                flags2: 0,
                id: 0x9A,
                pp: 0,
                ginseng: 0,
            };
        }
        // The first monster dies of its recoil; the second must still act.
        d.wild[0].info.hp = 1;
        let tables = crate::combat::tests::tables();
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut model = d.clone();
        d.run_wild_monsters(Some(&data));
        let mv = &moves[usize::from(move_effects::STRUGGLE)];
        // A: target draw, Struggle, removed (no end of turn). B, now at index 0: the same, then
        // its end of turn.
        model.rng.int(1);
        model.resolve_all(
            Slot::Wild(0),
            &[Slot::Team(0)],
            mv,
            &Effect::Struggle,
            &data,
        );
        assert_eq!(model.wild.len(), 1);
        model.rng.int(1);
        model.resolve_all(
            Slot::Wild(0),
            &[Slot::Team(0)],
            mv,
            &Effect::Struggle,
            &data,
        );
        model.rng.int(100);
        assert_eq!(d.wild.len(), 1);
        assert_eq!(d.rng, model.rng);
        assert_eq!(d.team[0].info.hp, model.team[0].info.hp);
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

    /// `room_with`, with the room cut to 20 by 20 so the leader at (50, 28) is out of sight.
    fn room_apart(wild: (i16, i16)) -> GameState {
        let mut g = room_with((50, 28), &[wild]);
        let d = g.dungeon_mut().unwrap();
        d.floor.rooms[0].x1 = 20;
        d.floor.rooms[0].y1 = 20;
        g
    }

    #[test]
    fn seeding_draws_one_exit_per_room_monster_and_the_first_turn_draws_none() {
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        for (x, y) in [(10, 5), (12, 19)] {
            d.floor.tile_mut(x, y).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        }
        let mut expected = d.rng;
        d.seed_wild_ai(true);
        expected.int(2);
        assert_eq!(d.rng, expected);
        let rec = *d.ai.wild.values().next().unwrap();
        assert_eq!(rec.objective, LEAVE_ROOM);
        d.run_wild_monsters(None);
        expected.int(35);
        expected.int(100);
        assert_eq!(d.rng, expected);
    }

    #[test]
    fn restoring_seeds_the_first_foreign_exit_without_a_draw_and_skips_monsters_outside_rooms() {
        let mut g = room_apart((10, 5));
        let d = g.dungeon_mut().unwrap();
        for (x, y) in [(10, 5), (12, 19)] {
            d.floor.tile_mut(x, y).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        }
        // A second monster on a tile that belongs to no room.
        d.floor.tile_mut(30, 10).unwrap().room = NO_ROOM;
        d.wild.push(wild_monster(7, 4, (30, 10)));
        let before = d.rng;
        d.seed_wild_ai(false);
        assert_eq!(d.rng, before, "no draw");
        assert_eq!(d.ai.wild.len(), 1);
        let rec = d.ai.wild[&(10, 5)];
        assert_eq!(rec.objective, LEAVE_ROOM);
        assert_eq!(
            rec.goal,
            (12, 19),
            "the first exit that is not its own tile"
        );
    }

    #[test]
    fn a_monster_in_a_room_with_exits_draws_one_exit_and_walks_there_without_more_draws() {
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        let exits: [(i16, i16); 2] = [(10, 5), (12, 19)];
        for (x, y) in exits {
            d.floor.tile_mut(x.into(), y.into()).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        }
        assert_eq!(d.floor.room_exits(0), exits);
        let mut expected = d.rng;
        d.run_wild_monsters(None);
        // Pick, the exit draw (`Int(count)`, one try as it is not the monster's tile), end.
        expected.int(35);
        let goal = exits[expected.int(2) as usize];
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(
            d.wild[0].pos,
            step((5, 5), direction_towards((5, 5), goal) as usize)
        );
        // Objective 4 stays: the next turns draw the pick and the end of turn only.
        for _ in 0..3 {
            d.run_wild_monsters(None);
            expected.int(35);
            expected.int(100);
            assert_eq!(d.rng, expected);
        }
        assert_eq!(
            d.ai.wild.values().next().map(|r| (r.objective, r.goal)),
            Some((4, goal))
        );
    }

    #[test]
    fn a_monster_on_a_junction_holding_objective_4_steps_out_to_the_corridor() {
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        d.floor.tile_mut(5, 5).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        d.floor.tile_mut(6, 5).unwrap().room = NO_ROOM;
        let held = AiRecord {
            objective: LEAVE_ROOM,
            goal: (9, 9),
            target: None,
        };
        d.ai.wild.insert((5, 5), held);
        let mut expected = d.rng;
        d.run_wild_monsters(None);
        // Pick, the direction roll of the step-out, end of turn: no `Int(count)` is drawn.
        expected.int(35);
        expected.int(8);
        expected.int(100);
        assert_eq!(d.rng, expected);
        let rec = *d.ai.wild.values().next().unwrap();
        assert_eq!((rec.objective, rec.goal), (ROAM, (6, 5)));
        assert_eq!(d.wild[0].pos, (6, 5));
    }

    #[test]
    fn an_exit_drawn_equal_to_the_own_tile_is_drawn_again_and_a_room_without_exits_stands_still() {
        let exits: [(i16, i16); 2] = [(5, 5), (10, 5)];
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        for (x, y) in exits {
            d.floor.tile_mut(x.into(), y.into()).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        }
        let mut redrawn = false;
        for seed in 0..32 {
            let mut run = d.clone();
            run.rng = crate::rng::DungeonRng::new(seed);
            let mut model = run.rng;
            run.run_wild_monsters(None);
            model.int(35);
            // Only the other exit can be taken, so the draws end at the first one that is not (5, 5).
            for tries in 1.. {
                if model.int(2) == 1 {
                    redrawn |= tries > 1;
                    break;
                }
            }
            model.int(100);
            assert_eq!(run.rng, model, "seed {seed}");
            let rec = *run.ai.wild.values().next().unwrap();
            assert_eq!(
                (rec.objective, rec.goal),
                (LEAVE_ROOM, (10, 5)),
                "seed {seed}"
            );
        }
        assert!(redrawn, "some seed must draw the own tile first");

        // No exit: one `Int(8)` and objective 6, the goal a neighbour.
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_wild_monsters(None);
        expected.int(35);
        let dir = expected.int(8) as usize;
        expected.int(100);
        assert_eq!(d.rng, expected);
        let rec = *d.ai.wild.values().next().unwrap();
        assert_eq!((rec.objective, rec.goal), (STAND_STILL, step((5, 5), dir)));
    }

    #[test]
    fn a_monster_that_loses_sight_follows_the_leaders_remembered_positions() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        // Turn 1: the leader is seen (objective 1).
        d.run_wild_monsters(None);
        assert_eq!(d.ai.wild.values().next().map(|r| r.objective), Some(1));
        // The leader steps out of sight, leaving (12, 5) first in its trail.
        d.floor.rooms[0].x1 = 20;
        d.floor.rooms[0].y1 = 20;
        d.team[0].pos = (50, 28);
        let mut expected = d.rng;
        d.run_wild_monsters(None);
        // Pick and end of turn only: no wander draw, the goal is the remembered tile.
        expected.int(35);
        expected.int(100);
        assert_eq!(d.rng, expected);
        let rec = *d.ai.wild.values().next().unwrap();
        assert_eq!((rec.objective, rec.goal), (2, (12, 5)));
        assert_eq!(d.wild[0].pos, (7, 5));
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

    /// `room_with` plus a partner at `partner`, as team slot 1.
    fn with_partner(leader: (i16, i16), partner: (i16, i16), wild: &[(i16, i16)]) -> GameState {
        let mut g = room_with(leader, wild);
        let d = g.dungeon_mut().unwrap();
        let mut ally = d.team[0].clone();
        ally.pos = partner;
        ally.info.is_team_leader = false;
        d.team.push(ally);
        g
    }

    #[test]
    fn a_swapped_partner_acts_in_the_swap_pass_and_the_team_loop_skips_it_once() {
        let mut g = with_partner((12, 5), (5, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        let start = d.rng;
        d.ai.swapping.push(1);
        d.run_swap_pass();
        // Only Execute's end-of-turn draw: one `Int(100)`, no pick, and the partner stays put.
        let mut expected = start;
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.team[1].pos, (5, 5));
        let after_pass = d.rng;
        d.run_team_members(None);
        assert_eq!(d.rng, after_pass);
        // The flag is cleared by the skip: the next team loop runs the partner.
        d.run_team_members(None);
        assert_ne!(d.rng, after_pass);
    }

    #[test]
    fn a_partner_follows_the_leader_and_draws_the_pick_and_the_end_of_turn() {
        let mut g = with_partner((12, 5), (5, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_team_members(None);
        // `Int(65)@02193260` twice (two cycles), then `Int(100)@02186894`; the walk draws nothing.
        expected.int(65);
        expected.int(65);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.team[1].pos, (6, 5));
        assert_eq!(d.team[1].info.direction, 2);
    }

    #[test]
    fn a_partner_with_a_distant_foe_in_view_still_follows_the_leader() {
        // Leader to the south (4), foe 3 tiles east (2), not adjacent: cause B of #348 row 8.
        let mut g = with_partner((10, 14), (10, 10), &[(13, 10)]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_team_members(None);
        expected.int(65);
        expected.int(65);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(
            d.team[1].pos,
            (10, 11),
            "it heads for the leader, not the foe"
        );
        assert_eq!(d.team[1].info.direction, 4);
    }

    #[test]
    fn a_partner_next_to_a_foe_it_sees_attacks_it_instead_of_following() {
        let mut g = with_partner((12, 5), (5, 5), &[(6, 5)]);
        let moves = duel(&[], &[]).1;
        let tables = crate::combat::tests::tables();
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let d = g.dungeon_mut().unwrap();
        let mut model = d.clone();
        d.run_team_members(Some(&data));
        // The pick, the target draw, the damage, then the end of turn.
        model.rng.int(65);
        model.rng.int(1);
        let mv = &moves[usize::from(REGULAR_ATTACK)];
        let attack = Effect::Damage {
            arg8: REGULAR_ATTACK_ARG8,
        };
        model.resolve_all(Slot::Team(1), &[Slot::Wild(0)], mv, &attack, &data);
        model.rng.int(100);
        assert_eq!(d.rng, model.rng);
        assert_eq!(d.team[1].pos, (5, 5), "an attack does not move");
        assert_eq!(d.team[1].info.direction, 2, "it faces the foe");
        assert_eq!(d.wild.len(), model.wild.len());
        assert_eq!(
            d.wild.first().map(|w| w.info.hp),
            model.wild.first().map(|w| w.info.hp)
        );
    }

    #[test]
    fn a_partner_blocked_by_a_monster_acts_again_in_phase_2_and_its_end_of_turn_comes_last() {
        // The partner at (5,5) heads right for the leader at (9,5); a wild monster far off in a
        // closed corridor cannot be seen, so only the other team member blocks: a second partner
        // stands at (6,5), walls close the two 45 degree tiles.
        let mut g = with_partner((9, 5), (5, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        let mut blocker = d.team[1].clone();
        blocker.pos = (6, 5);
        d.team.push(blocker);
        for p in [(6, 4), (6, 6)] {
            d.floor.tile_mut(p.0, p.1).unwrap().terrain = floor::TERRAIN_IMPASSABLE_WALL;
        }
        let mut expected = d.rng;
        d.run_team_members(None);
        // Phase 1: A picks twice (blocked, flagged); B picks twice and steps on, end of turn.
        // Phase 2: A, `recalculateFollow` set, picks twice and steps into the freed tile with
        // its end of turn; nothing is left for the final pass.
        for cap in [65, 65, 65, 65, 100, 65, 65, 100] {
            expected.int(cap);
        }
        assert_eq!(d.rng, expected);
        assert_eq!((d.team[1].pos, d.team[2].pos), ((6, 5), (7, 5)));
    }
}
