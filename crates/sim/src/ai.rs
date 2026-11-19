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

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::combat::{CombatData, REGULAR_ATTACK, REGULAR_ATTACK_ARG8};
use crate::floor::{self, NO_ROOM};
use crate::move_effects::{self, Effect};
use crate::state::{DIRECTIONS, Dungeon, MoveSlot};
use crate::status;
use crate::targeting::Slot;
use crate::turn::acts;

/// The chance (percent) that a confused monster skips `ChooseAIMove` (`0x021AA184`).
const CONFUSED_SKIP_PERCENT: i32 = 70;

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
/// (`02192728`: `0x00`, `0x10` and `0x20`, and `0x40`, Growl's: the foe one or two tiles ahead).
// shortcut: area `0x40` is read for the adjacent foes only; a foe two tiles ahead is not a target
// (#507: the partner's Growl at a foe next to it).
const ADJACENT_AREAS: [u16; 4] = [0x00, 0x10, 0x20, 0x40];

/// `gFaceDirectionIncrements`, the order `DecideMovement` tries directions in: ahead, then 45
/// degrees one way and the other (the loop stops at index 2 without a run-away or turn-around
/// state, so only these three are reached).
// shortcut: the pool table `0x021A98F4` was not read; the values are Red's table (the note says
// Blue's order matches, `ai.md` P3-e). Check with an overlay byte read before the sign of the
// 45-degree turns is relied on.
const TURNS: [usize; 5] = [0, 1, 7, 2, 6];

/// The byte table at `0x02091E48` that `02184074` reads for a team member, indexed by the
/// tactic (`info+0x8A`): non-zero skips `AIFindTarget`'s enemy scan (tactics 0, 4 to 8 and 10;
/// `docs/re/ai.md`, #73 investigator b P5).
const TACTIC_SKIPS_SCAN: [bool; 12] = [
    true, false, false, false, true, true, true, true, true, false, true, false,
];

/// Who moves, for the team-only branches of `DecideMovement` (`02190FEC`).
#[derive(Clone, Copy)]
struct Mover {
    /// `info+6 == 0`: a team member.
    team: bool,
    /// `info+0x117`: phase 2 of the team loop.
    recalc: bool,
    /// The wide candidate loop (4 turns, not 2): run-away (`02180B68`) or `+0x73` set.
    wide: bool,
}

impl Mover {
    const WILD: Self = Self {
        team: false,
        recalc: false,
        wide: false,
    };
}

/// `0200B9A4`-style straight-corridor masks of the wander's first branch (`02180C98`): the tile's
/// walkable mask equal to one of these makes the monster pick a new random direction.
const STRAIGHT_MASKS: [u8; 5] = [0x54, 0x51, 0x45, 0x15, 0x55];

/// Wild-loop flags: `0x8000` skips the monster, `0x4000` skips it once and is cleared.
const HELD_FLAG: u16 = 0x8000;
const SKIP_ONCE_FLAG: u16 = 0x4000;

/// Whether the wild monster's own speed stage acts in fractional turn `k`.
fn wild_acts(m: &crate::state::Monster, k: u8) -> bool {
    acts(status::speed_stage(&m.info, status::WILD_BASE_STAGE), k)
}

/// AI objectives (`+0x70`; the names are Red's, see `docs/re/structs.md`, "AI target readers").
const CHASE_TARGET: u8 = 1;
const CHASE_REMEMBERED: u8 = 2;
const ROAM: u8 = 3;
const LEAVE_ROOM: u8 = 4;
const STAND_STILL: u8 = 6;

/// The persistent part of a monster's AI record: the objective (`+0x70`), the goal tile
/// (`+0x80`, `+0x82`) and the target (`+0x78`, a team slot).
// shortcut: the generation ID (`+0x74`) is not kept: a team slot is never reused on a floor
// here, so the ID always matches. The record is cleared nowhere else (`0216C638`, petrify and decoy resets are not
// modelled, as the sim has no such statuses).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiRecord {
    objective: u8,
    goal: (i16, i16),
    target: Option<usize>,
    /// The monster flags `0x8000` and `0x4000` (`info+0x..`, see [`HELD_FLAG`]).
    flags: u16,
    /// `+0xF0`, "move randomly": set at spawn from the movement roll (`021826A0`, stored by
    /// `02179CFC` line 31); `AIWander` then always takes the random-neighbour branch (objective 6).
    random_walk: bool,
}

/// A team member's AI record and its last four positions (`info+0x5E`, written by the move
/// helper `02177700`: the tiles it left, newest first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct TeamAi {
    rec: AiRecord,
    last: (i16, i16),
    prev: [(i16, i16); 4],
    /// `info+0x8A`, the tactic (0 is LETS_GO_TOGETHER, the default).
    tactic: u8,
    /// The partner walked and its end of turn waits for the movement flush
    /// ([`Dungeon::flush_moves`]).
    pending_move: bool,
}

/// The AI state of a floor: the wild monsters' records and the team's.
// shortcut: a wild monster's record is keyed by its tile (monsters have no ID here, and other
// modules remove and add them), so a monster moved by something other than its own walk starts
// a fresh record, and a monster that spawns on the tile of one that just fell inherits its
// record until the next wild loop prunes it.
// `Snapshot` does not hold this state (its literals are in `apps/trace-harness`); the save's
// [`Resume`](crate::state::Resume) does, through [`AiSnapshot`] (#535).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiState {
    wild: BTreeMap<(i16, i16), AiRecord>,
    team: Vec<TeamAi>,
    /// The tiles of the wild monsters that walked and wait for the movement flush.
    pending_wild: BTreeSet<(i16, i16)>,
    /// Team slots flagged `0x8000` by a leader swap, until the swap pass (`02140ED0`) runs.
    pub(crate) swapping: Vec<usize>,
    /// Team slots flagged `0x4000`: the team loop skips each once and clears the flag.
    held: Vec<usize>,
    /// The slot (0 to 15) of each wild monster, ascending: the list is in slot order and a
    /// spawn takes the lowest free slot. Rebuilt as 0, 1, ... when its length differs.
    pub(crate) slots: Vec<u8>,
    /// `Species::chance_asleep` (species `+0x1D`) by species ID, read by the spawn's sleep roll
    /// (`02179CFC`). A species past the end reads 0 (never spawns asleep).
    pub(crate) chance_asleep: Vec<u8>,
}

/// An [`AiState`] as serde data (JSON has no tuple map keys, so the wild map is a list). The
/// run's `chance_asleep` table is saved once, by the state, not here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSnapshot {
    wild: Vec<((i16, i16), AiRecord)>,
    team: Vec<TeamAi>,
    pending_wild: Vec<(i16, i16)>,
    swapping: Vec<usize>,
    held: Vec<usize>,
    slots: Vec<u8>,
}

impl AiState {
    pub(crate) fn capture(&self) -> AiSnapshot {
        AiSnapshot {
            wild: self.wild.iter().map(|(k, v)| (*k, *v)).collect(),
            team: self.team.clone(),
            pending_wild: self.pending_wild.iter().copied().collect(),
            swapping: self.swapping.clone(),
            held: self.held.clone(),
            slots: self.slots.clone(),
        }
    }

    pub(crate) fn from_snapshot(s: &AiSnapshot, chance_asleep: Vec<u8>) -> Self {
        Self {
            wild: s.wild.iter().copied().collect(),
            team: s.team.clone(),
            pending_wild: s.pending_wild.iter().copied().collect(),
            swapping: s.swapping.clone(),
            held: s.held.clone(),
            slots: s.slots.clone(),
            chance_asleep,
        }
    }
}

/// What a turn's decision step ended with.
enum Plan {
    /// Pass the turn (action 1).
    Pass,
    /// Walk one tile in direction index `0..8`.
    Walk(usize),
}

impl Dungeon {
    /// Keeps [`AiState::slots`] the length of the wild list (a list set from outside holds its
    /// monsters in slots 0, 1, ...).
    pub(crate) fn sync_wild_slots(&mut self) {
        if self.ai.slots.len() != self.wild.len() {
            self.ai.slots = (0..self.wild.len()).map(|i| i as u8).collect();
        }
    }

    /// The wild loop of fractional turn `k` (`021408F8`): every wild monster gated in acts once in
    /// slot order, and each one still flagged `aiAllySkip` at the end gets its end-of-turn
    /// effects. The gate is per monster (`docs/re/turns.md`, #73 P1.1), in this order: flag
    /// `0x8000` skips it, `0x4000` is cleared and skips it once (even in a turn its stage does not
    /// act); then its own speed stage (`CalcSpeedStage`) must act in `k`.
    // shortcut: a wild monster's base stage is [`status::WILD_BASE_STAGE`] (all floor 1 to 3
    // species have movement speed 1). The `+0x11A` (attacking) skip and `+0x117 = 0` are not
    // modelled: nothing in the sim sets the first or reads the second. Nothing sets the
    // `0x8000` and `0x4000` flags either; the record holds them for when a setter exists. A
    // monster that spawns asleep is not modelled.
    pub(crate) fn run_wild_monsters(&mut self, k: u8, data: Option<&CombatData<'_>>) {
        self.sync_team_ai();
        let live: Vec<_> = self.wild.iter().map(|m| m.pos).collect();
        self.ai.wild.retain(|p, _| live.contains(p));
        self.ai.pending_wild.retain(|p| live.contains(p));
        let mut deferred = Vec::new();
        let mut i = 0;
        while i < self.wild.len() {
            // A fallen leader ends the loop: the run is over.
            if !self.leader_alive() {
                break;
            }
            if !self.pass_wild_flags(i) || !wild_acts(&self.wild[i], k) {
                i += 1;
                continue;
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

    /// The flag part of the gate: false for a monster flagged `0x8000` or `0x4000` (which is
    /// cleared); else clears `0xC000` and returns true.
    fn pass_wild_flags(&mut self, i: usize) -> bool {
        let rec = self.ai.wild.entry(self.wild[i].pos).or_default();
        if rec.flags & HELD_FLAG != 0 {
            return false;
        }
        if rec.flags & SKIP_ONCE_FLAG != 0 {
            rec.flags &= !SKIP_ONCE_FLAG;
            return false;
        }
        true
    }

    /// The team loop (`02140AC0`, `docs/re/turns.md`, "Other monsters' turns"): each gated
    /// partner acts once in slot order (phase 1), then the members flagged `aiAllySkip` act again
    /// in up to three passes, nearest the leader first, last slot first within a distance
    /// (phase 2); each member still flagged at the end gets its end-of-turn effects.
    // shortcut: every partner is gated at speed stage 1 (the caller picks the gated turns), and
    // runs one `RunMonsterAI` and Execute cycle per call. The game repeats up to three while
    // Execute returns TRUE, but it returned 0 on every partner call of the floor-1 capture (#504):
    // the second pick of a partner that stays is this phase-2 rerun, not a second cycle.
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
    // shortcut: without combat data the pick draws the team total 65 seen in the P7 trace and
    // only the regular attack is used; with it the partner picks by weight (`choose_ai_move`).
    // IQ skills are not read (`ai.md`, P4-b): the tactic table gates the enemy scan
    // (`TACTIC_SKIPS_SCAN`); a partner that scans attacks a foe it sees, else follows the
    // leader. A phase-2 turn may re-flag
    // through the trailing branch of `DecideMovement` (branch 2a is phase 1 only).
    // shortcut: the partner's belly is not modelled (`bellyEmpty` stays false).
    fn ally_turn(&mut self, i: usize, data: Option<&CombatData<'_>>, recalc: bool) -> bool {
        let me = self.team[i].pos;
        if self.team[i].info.hp == 0 {
            return false;
        }
        // The `TickStatusAndHealthRegen` HP block runs once per team pass (`02140BA0`), not in the
        // recalc one.
        if !recalc {
            Self::regenerate_gated(&mut self.team[i].info);
        }
        // shortcut: the status countdowns still tick again on the recalc pass (recalc == true); the
        // game ticks once per pass (#514 P-R32e, hypothesis: no status was active in the floor-1
        // capture).
        self.team[i].info.tick_status();
        if self.team[i].info.is_asleep() {
            self.ally_end_of_turn(i);
            return false;
        }
        // `AIFindTarget` (`02191394`): the foe search is gated by `02184074`, which for a team
        // member reads the byte table `0x02091E48` at the tactic (`info+0x8A`); non-zero skips
        // the scan (tactics 0, 4 to 8 and 10), and the target is the leader if in view
        // (`GetLeaderIfVisible`, `02183FF0`). `docs/re/ai.md` P3-c, #73 investigator b P5.
        // shortcut: a scanning tactic (1 to 3, 9, 11) targets the nearest foe in view, else the
        // leader; its own branches (tactic 1's leader rule, 9 holding still) are not read.
        let leader = self.team[0].pos;
        let tactic = self.ai.team.get(i).map_or(0, |t| t.tactic);
        let scans = !TACTIC_SKIPS_SCAN
            .get(usize::from(tactic))
            .copied()
            .unwrap_or(true);
        let foe = self.nearest_seen(me, self.wild.iter().map(|m| m.pos));
        let leader_seen = self.in_view(me, leader).then_some(leader);
        let target = if scans {
            foe.or(leader_seen)
        } else {
            leader_seen
        };
        // `ChooseAIMove` (`02192D24`), the wild monsters' own: the weighted pick is drawn
        // whatever the target (#507).
        let attacked = match data {
            Some(data) => self.choose_ai_move(Slot::Team(i), data),
            None => {
                let _pick = self.rng.int(TEAM_PICK_TOTAL);
                if let Some(f) = foe.filter(|&f| self.adjacent_attackable(me, f)) {
                    // The regular attack's target pick among one candidate (`02192CC0`).
                    let _candidate = self.rng.int(1);
                    self.team[i].info.direction = direction_towards(me, f);
                    true
                } else {
                    false
                }
            }
        };
        if attacked {
            self.ally_end_of_turn(i);
            return false;
        }
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
                    flags: 0,
                    random_walk: false,
                };
                t
            }
            None => self.wander(me, facing, &mut rec),
        };
        if let Some(t) = self.ai.team.get_mut(i) {
            t.rec = rec;
        }
        let mover = Mover {
            team: true,
            recalc,
            wide: false,
        };
        let (plan, skip) = self.decide_movement(me, goal, mover);
        if let Plan::Walk(dir) = plan {
            let m = &mut self.team[i];
            m.pos = step(m.pos, dir);
            m.info.direction = dir as u8;
        }
        if !skip && let Some(t) = self.ai.team.get_mut(i) {
            t.pending_move = true;
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

    /// The rest of `02179CFC` for the wild monster just added at `pos` (slot allocation is done):
    /// `random_walk` is the movement roll's result (`+0xF0`). First objective (`021794CC` ->
    /// `AIMovement`, line 19 of `02179CFC`): a team member in view becomes the target and nothing
    /// is drawn, else `AIWander`. That runs before `+0xF0` is written (line 31), so it sees
    /// `random_walk` false; only later wanders read the flag. Then the asleep roll, `Int(100)` against `Species::chance_asleep`, which
    /// sleeps the monster with the `0x7F` "no countdown" duration (`0218B420`).
    // shortcut: the roll's other conditions (record `+2`, `+14`, `param_2`) are read as 0, true
    // for a natural spawn; the `0217ADF8(monster, 8)` call after the sleep is not modelled.
    pub(crate) fn setup_spawned_wild(&mut self, pos: (i16, i16), random_walk: bool) {
        let Some(i) = self.wild.iter().position(|m| m.pos == pos) else {
            return;
        };
        let mut rec = AiRecord::default();
        if let Some(t) = self.find_target(pos) {
            rec.objective = CHASE_TARGET;
            rec.goal = t;
        } else {
            let facing = usize::from(self.wild[i].info.direction);
            self.wander(pos, facing, &mut rec);
        }
        rec.random_walk = random_walk;
        self.ai.wild.insert(pos, rec);
        let species = usize::from(self.wild[i].info.species);
        let chance = self.ai.chance_asleep.get(species).copied().unwrap_or(0);
        if self.rng.int(100) < i32::from(chance) {
            let s = &mut self.wild[i].info.status;
            s.sleep = status::SLEEP;
            s.sleep_turns = status::NO_END;
        }
    }

    /// The movement flush (`DisplayActions_Async`, `0218766C`): the end of turn of every monster
    /// that walked since the last flush, the leader first, then the team slots, then the wild
    /// monsters in list order. A walker only marks itself; Execute of a non-walk action, the
    /// weather step and the leader's next action flush (`docs/re/turns.md`, #510).
    pub(crate) fn flush_moves(&mut self) {
        if self.pending_end_of_turns > 0 {
            self.pending_end_of_turns = 0;
            self.leader_end_of_turn();
        }
        for i in 1..self.team.len() {
            if self
                .ai
                .team
                .get_mut(i)
                .is_some_and(|t| std::mem::take(&mut t.pending_move))
            {
                self.ally_end_of_turn(i);
            }
        }
        for i in 0..self.wild.len() {
            if self.ai.pending_wild.remove(&self.wild[i].pos) {
                self.monster_end_of_turn(i);
            }
        }
        self.ai.pending_wild.clear();
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
        // `ChooseAIMove`'s gates: `02181530(self, 0)` (sleep-like modes, `+0x9B == 4`),
        // `02180B68` (run away) and `IsTacticSet(8)` skip the whole function with no draw; then
        // a confused monster (`+0xAC == 2`) skips it on a 70% roll (`0x021AA184`).
        // After `ChooseAIMove`, `02188030` runs `AIMovement` (`021918D4`) only for a monster that
        // is not confused and set no action; a confused one that set none goes to `02141878`,
        // which sets action 1 or 2 and draws nothing. So a confused monster never walks, whether
        // or not the 70% roll skipped.
        // shortcut: the run-away test (ability `+` below half HP, tactics 6 and 10), tactic 8
        // and the frozen byte `+0xA0` are not modelled: a wild monster has no tactic, the
        // ability is not identified, and the sim keeps no frozen state. Upgrade each when the
        // sim keeps it (a frozen status, an ability, a tactic).
        let info = &self.wild[i].info;
        let (gated, confused) = (info.skips_move_choice(), info.is_confused());
        let confusion_skip = !gated && confused && self.rng.outcome(CONFUSED_SKIP_PERCENT);
        let choice = data.filter(|_| has_moves);
        let attacked = match choice {
            _ if gated => false,
            _ if confusion_skip => true,
            Some(data) => self.choose_ai_move(Slot::Wild(i), data),
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
        if attacked || confused {
            // A monster the recoil removed gets no end of turn (`i` now names the next one).
            if self.wild.len() == len_before {
                self.monster_end_of_turn(i);
            }
            return false;
        }
        let facing = usize::from(self.wild[i].info.direction);
        let goal = self.movement_goal(me, facing, target, rec);
        let (plan, skip) = self.decide_movement(me, goal, Mover::WILD);
        if let Plan::Walk(dir) = plan {
            let (dx, dy) = DIRECTIONS[dir].delta();
            let m = &mut self.wild[i];
            // The map is at most 56 by 32, so the step stays in `i16`.
            m.pos = (m.pos.0 + dx as i16, m.pos.1 + dy as i16);
            m.info.direction = dir as u8;
        }
        if !skip {
            let at = self.wild[i].pos;
            self.ai.pending_wild.remove(&me);
            self.ai.pending_wild.insert(at);
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
    // The gates before the move choice (`02181530`, the confusion roll) run in `wild_turn_with`.
    // shortcut: the bide block (`+0xAE`) reads a status the sim keeps no field for, so it passes.
    // No IQ skill is set (PP Checker, Weak-Type Picker, Exclusive
    // Move-User), as the sim derives none from the monster's IQ (`+0x87`).
    // shortcut: a move is usable (`02159E58`) when it has PP; its other tests are not read.
    // shortcut: a move's target search (`02192728`) is modelled for the adjacent areas of its AI
    // field (`0x00`, `0x10`, `0x20`), every foe weighing 1 (`021921B0`'s type weights are not
    // read), and for moves `move_effects` models. A move with another area, a non-zero condition
    // nibble (`02192274`, which can draw) or no modelled effect finds no target, so the monster
    // falls back to the regular attack. The game's eligibility filters beyond adjacency
    // (`02192508`) are read as passing.
    fn choose_ai_move(&mut self, me: Slot, data: &CombatData<'_>) -> bool {
        let Some(slots) = self.ai_info(me).map(|info| info.moves) else {
            return false;
        };
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
            // The target search is `02192728`'s, so it draws the candidate pick.
            let dirs = self.adjacent_foe_directions(me);
            return match self.pick_direction(&dirs) {
                Some(dir) => self.use_struggle(me, dir, data),
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
        // `02191F84`: the regular attack's target, searched whatever the roll picks. A wild
        // monster's IQ set has no multi-target mode, so it takes the first adjacent foe, no draw.
        let regular = self.regular_attack_direction(me);
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
                return regular.is_some_and(|dir| self.use_regular_attack(me, dir, data));
            }
            if let Some(dir) = self.move_target_direction(me, entry, data) {
                return self.use_move_slot(me, entry, dir, data);
            }
            break;
        }
        regular.is_some_and(|dir| self.use_regular_attack(me, dir, data))
    }

    /// The info of the monster at `me` (`Dungeon::info_of` is private to `combat`).
    fn ai_info(&self, me: Slot) -> Option<&crate::state::EntityInfo> {
        match me {
            Slot::Team(i) => self.team.get(i).map(|m| &m.info),
            Slot::Wild(i) => self.wild.get(i).map(|m| &m.info),
        }
    }

    /// [`ai_info`](Self::ai_info), mutably.
    fn ai_info_mut(&mut self, me: Slot) -> Option<&mut crate::state::EntityInfo> {
        match me {
            Slot::Team(i) => self.team.get_mut(i).map(|m| &mut m.info),
            Slot::Wild(i) => self.wild.get_mut(i).map(|m| &mut m.info),
        }
    }

    /// The directions (indices into [`DIRECTIONS`]) of the adjacent foes monster `i` can hit with
    /// a regular attack: the team members one tile away with no wall corner between (the
    /// `02180FFC` test), scanned from the facing byte `+0x44` round the 8 neighbours
    /// (`02191F84`; `02192728` uses the same set).
    fn adjacent_foe_directions(&self, me: Slot) -> Vec<usize> {
        let (pos, facing, foes) = match me {
            Slot::Wild(i) => (self.wild[i].pos, self.wild[i].info.direction, &self.team),
            Slot::Team(i) => (self.team[i].pos, self.team[i].info.direction, &self.wild),
        };
        let facing = usize::from(facing);
        (0..DIRECTIONS.len())
            .map(|k| (facing + k) % DIRECTIONS.len())
            .filter(|&d| {
                let (dx, dy) = DIRECTIONS[d].delta();
                let at = (pos.0 + dx as i16, pos.1 + dy as i16);
                foes.iter().any(|t| t.pos == at && t.info.hp > 0)
                    && self.corner_open(i32::from(pos.0), i32::from(pos.1), dx, dy)
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

    /// `02191F84`: the regular attack's direction. A wild monster's IQ set (`021804CC`: skills 8,
    /// 6, 2, 18, 22) has no multi-target mode (9, 10), so the scan takes the first adjacent foe
    /// and draws nothing (#73 P2.4).
    fn regular_attack_direction(&self, me: Slot) -> Option<usize> {
        self.adjacent_foe_directions(me).first().copied()
    }

    /// `02192728` for the move in slot `slot` of monster `i`: the direction of the foe it would
    /// use the move on, with the target pick drawn, or `None` when the move has no target.
    fn move_target_direction(
        &mut self,
        me: Slot,
        slot: usize,
        data: &CombatData<'_>,
    ) -> Option<usize> {
        let id = self.ai_info(me)?.moves[slot].id;
        let mv = data.moves.get(usize::from(id))?;
        let ai = mv.targeting[1];
        if ai & 0x0F00 != 0 || !ADJACENT_AREAS.contains(&(ai & 0xF0)) {
            return None;
        }
        move_effects::effect_of(id)?;
        let dirs = self.adjacent_foe_directions(me);
        self.pick_direction(&dirs)
    }

    /// The regular attack by wild monster `i` towards direction index `dir`: the monster turns to
    /// face it and the regular attack's damage resolves on the foes its range picks.
    fn use_regular_attack(&mut self, me: Slot, dir: usize, data: &CombatData<'_>) -> bool {
        if let Some(info) = self.ai_info_mut(me) {
            info.direction = dir as u8;
        }
        self.regular_attack_hits(me, dir, data)
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
    fn use_struggle(&mut self, me: Slot, dir: usize, data: &CombatData<'_>) -> bool {
        if let Some(info) = self.ai_info_mut(me) {
            info.direction = dir as u8;
        }
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
        let Some(info) = self.ai_info_mut(user) else {
            return false;
        };
        info.direction = dir as u8;
        let id = info.moves[slot].id;
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
                flags: rec.flags,
                random_walk: rec.random_walk,
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
    // shortcut: the mobility class is 0 (a regular walker). `+0xF0` is set from the movement roll
    // at spawn (`02179CFC`) and read here (see [`AiRecord`]).
    fn wander(&mut self, me: (i16, i16), facing: usize, rec: &mut AiRecord) -> (i16, i16) {
        if rec.random_walk {
            return self.stand_still(me, rec);
        }
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

    /// `DecideMovement` (`02190FEC`) for the monster at `me` heading to `goal`: the plan and
    /// whether the monster ends flagged `aiAllySkip` (`+0x116`).
    ///
    /// In the game's order: the direct step (ok: walk). Blocked by a monster, a team member in
    /// phase 1 (`+0x117 == 0`) passes flagged at once (branch 2a); otherwise a goal that is
    /// exactly that adjacent tile passes unflagged (2b). Then the candidate loop tries the two
    /// 45-degree turns, or four turns when `mover.wide` (2d); for a team member on a diagonal
    /// with both axis distances under 3 and unequal, one of the two 45-degree turns is
    /// disabled (2c). A monster seen on the way flags a non-leader that passes at the end.
    /// `docs/re/ai.md` and #73 investigator b P4.2.
    // shortcut: the 90-degree turns of a run-away or turn-around state (`02180C6C`,
    // `0219C3CC`) and the table `0x02191390` (read as Red's `0, 1, 7, 2, 6`) are not read; no
    // caller sets `wide` yet (it needs the run-away predicate or `+0x73`, tactic 8). The flag
    // `+0x71` is read as clear at the start of each turn, and a leader never gets here.
    fn decide_movement(&self, me: (i16, i16), goal: (i16, i16), mover: Mover) -> (Plan, bool) {
        if me == goal {
            return (Plan::Pass, false);
        }
        let dir = direction_towards(me, goal) as usize;
        let (ok, monster) = self.can_move(me, dir);
        if ok {
            return (Plan::Walk(dir), false);
        }
        let mut blocked_by_monster = false;
        if monster {
            if mover.team && !mover.recalc {
                return (Plan::Pass, true);
            }
            if step(me, dir) == goal {
                return (Plan::Pass, false);
            }
            blocked_by_monster = true;
        }
        let mut enabled = [true; 5];
        if mover.team && dir & 1 == 1 {
            let dx = (i32::from(me.0) - i32::from(goal.0)).abs();
            let dy = (i32::from(me.1) - i32::from(goal.1)).abs();
            if dx < 3 && dy < 3 && dx != dy {
                // With `dir & 2` clear the turn at index 2 goes when x is the longer axis and
                // index 1 when y is; with it set, the other way round.
                let off = if (dir & 2 == 0) == (dx > dy) { 2 } else { 1 };
                enabled[off] = false;
            }
        }
        let count = if mover.wide { 5 } else { 3 };
        let tries: Vec<(usize, bool, bool)> = TURNS
            .iter()
            .enumerate()
            .take(count)
            .skip(1)
            .map(|(n, turn)| {
                let (ok, monster) = self.can_move(me, (dir + turn) % 8);
                (n, ok, monster)
            })
            .collect();
        // A terrain-blocked candidate (`02190FEC`, first loop) re-enables both dropped turns.
        if tries.iter().any(|&(_, ok, monster)| !ok && !monster) {
            enabled = [true; 5];
        }
        for (n, ok, monster) in tries {
            if ok && enabled[n] {
                return (Plan::Walk((dir + TURNS[n]) % 8), false);
            }
            blocked_by_monster |= monster;
        }
        // A non-leader that saw a monster in the way skips to the end-of-turn pass.
        (Plan::Pass, blocked_by_monster)
    }

    /// `CanAIMonsterMoveInDirection` (`02180DF8`) from `me` in direction index `dir`: whether the
    /// step is allowed, and whether a monster stands on the tile (which blocks it).
    ///
    /// In the game's order: the target tile's wall bit `0x10` blocks; the monster-house bit
    /// `0x40`, a trap and lava block only with IQ skills 15, 14 and 20, which a plain monster
    /// lacks; the mobility class (plain: regular, class 0) reads the current tile's walkable
    /// byte `+6` ANDed with the direction's bit; last, a monster on the tile fails the move and
    /// sets the flag.
    // shortcut: every monster is class 0 and has no IQ skill, status `+0xC7` or held item that
    // changes it; the species mobility (`0218267C`) is not kept. The floor generator does not
    // fill `walkable_neighbors`, so the regular byte is derived from the terrain by the mask
    // writer's rule (`02148F50`, `docs/re/turns.md`, "Movement") instead of being read.
    fn can_move(&self, me: (i16, i16), dir: usize) -> (bool, bool) {
        let (dx, dy) = DIRECTIONS[dir].delta();
        let (x, y) = (i32::from(me.0) + dx, i32::from(me.1) + dy);
        let Some(tile) = self.floor.tile(x, y) else {
            return (false, false);
        };
        if tile.terrain & floor::TERRAIN_IMPASSABLE_WALL != 0
            || !self.regular_mask_allows(i32::from(me.0), i32::from(me.1), dx, dy)
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

    /// The regular-class bit of the walkable byte (`+6`) of tile (`x`, `y`) for the step
    /// (`dx`, `dy`): a cardinal bit needs the neighbour to be exactly NORMAL terrain; a diagonal
    /// one needs the diagonal NORMAL and both orthogonal neighbours not walls (type 0).
    fn regular_mask_allows(&self, x: i32, y: i32, dx: i32, dy: i32) -> bool {
        let kind = |xx: i32, yy: i32| {
            self.floor
                .tile(xx, yy)
                .map_or(0, |t| t.terrain & floor::TERRAIN_TYPE_MASK)
        };
        kind(x + dx, y + dy) == floor::TERRAIN_FLOOR
            && (dx == 0 || dy == 0 || (kind(x + dx, y) != 0 && kind(x, y + dy) != 0))
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
    use crate::rng::DungeonRng;

    /// A fractional turn in which a speed stage 1 monster acts.
    const K: u8 = 3;
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
    /// (the weighted pick's total, if any), then `used`, the move slot the monster is expected to use (the move's own target draw, the
    /// use and its PP) or the regular attack's resolve, then the end-of-turn roll. Returns the
    /// dungeon.
    fn check_turn(
        g: &GameState,
        moves: &[rescue_data::moves::Move],
        seed: u32,
        pick: Option<i32>,
        used: impl Fn(i32) -> Option<usize>,
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
        actual.run_wild_monsters(K, Some(&data));
        actual.flush_moves();

        let roll = pick.map_or(0, |total| model.rng.int(total));
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
            let d = check_turn(&g, &moves, seed, Some(total), |roll| {
                (roll <= 6).then_some(0)
            });
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
            let d = check_turn(&g, &moves, seed, Some(total), |roll| match roll {
                ..=3 => Some(0),
                4..=7 => Some(1),
                _ => None,
            });
            let pps = (d.wild[0].info.moves[0].pp, d.wild[0].info.moves[1].pp);
            seen[match pps {
                (9, 10) => 0,
                (10, 9) => 1,
                _ => 2,
            }] = true;
        }
        assert_eq!(seen, [true; 3]);
    }

    /// The wild totals of `#358` P7: a Tackle-only monster (Pidgey) and an Absorb-only one
    /// (Sunkern), with the table entry for one enabled move set to the remainder.
    fn total_case(weight: u8, total: i32) {
        let (g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, weight)]);
        let mut tables = crate::combat::tests::tables();
        tables.regular_attack_weight[1] = (total - i32::from(weight)) as i16;
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let (mut by_move, mut by_regular) = (false, false);
        for seed in 0..64 {
            let mut actual = g.dungeon().unwrap().clone();
            actual.rng = crate::rng::DungeonRng::new(seed);
            let mut model = actual.clone();
            actual.run_wild_monsters(K, Some(&data));
            actual.flush_moves();
            let roll = model.rng.int(total);
            if roll <= i32::from(weight) {
                by_move = true;
                model.rng.int(1); // the move's own target draw
                let mv = &moves[0x9A];
                let effect = move_effects::effect_of(0x9A).unwrap();
                model.strike(Slot::Wild(0), 0, &[Slot::Team(0)], mv, &effect, &data);
            } else {
                by_regular = true; // the regular attack draws no target
                let mv = &moves[usize::from(REGULAR_ATTACK)];
                let effect = Effect::Damage {
                    arg8: REGULAR_ATTACK_ARG8,
                };
                model.resolve_all(Slot::Wild(0), &[Slot::Team(0)], mv, &effect, &data);
            }
            model.rng.int(100);
            assert_eq!(actual.rng, model.rng, "total {total}, seed {seed}");
        }
        assert!(by_move && by_regular);
    }

    #[test]
    fn a_tackle_only_monster_rolls_against_45_and_a_sunkern_against_35() {
        total_case(10, 45);
        total_case(5, 35);
    }

    #[test]
    fn a_wild_tackle_on_the_partner_draws_the_games_list_and_hurts_the_partner() {
        // #505 row 16: the leader is away and the partner stands next to the wild monster. The
        // game draws `Int(45)@02193260`, no draw in `02191F84`, `Int(1)@02192CC0`, then Execute's
        // `100, 100, 100, 16384, 100` (`0215A950`, `0217C02C`, `0217C698`, `0217C720`,
        // `0215A950`), then the end-of-turn `Int(100)`.
        let (mut g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 10)]);
        let mut tables = crate::combat::tests::tables();
        tables.regular_attack_weight[1] = 35;
        let d = g.dungeon_mut().unwrap();
        let partner = Monster {
            pos: (12, 10),
            ..d.team[0].clone()
        };
        d.team[0].pos = (3, 3);
        d.team.push(partner);
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut hit = false;
        for seed in 0..64 {
            let mut actual = g.dungeon().unwrap().clone();
            actual.rng = crate::rng::DungeonRng::new(seed);
            let mut model = actual.clone();
            actual.run_wild_monsters(K, Some(&data));
            actual.flush_moves();
            if model.rng.int(45) > 10 {
                continue; // the roll landed on the regular attack
            }
            for total in [1, 100, 100, 100, 16384, 100, 100] {
                model.rng.int(total);
            }
            assert_eq!(actual.rng, model.rng, "seed {seed}");
            assert_eq!(actual.team[0].info.hp, g.dungeon().unwrap().team[0].info.hp);
            hit |= actual.team[1].info.hp < g.dungeon().unwrap().team[1].info.hp;
        }
        assert!(hit, "the Tackle must reach the partner");
    }

    #[test]
    fn with_no_enabled_move_the_regular_attack_is_picked_by_table_entry_0() {
        // The slots exist and have PP but none is enabled for the AI (bit 2 clear): the total is
        // table[0], so the roll always lands on the regular attack.
        let (g, moves) = duel(&[(0x9A, 1, 10)], &[(0x9A, 6)]);
        for seed in 0..8 {
            let d = check_turn(&g, &moves, seed, Some(REGULAR[0]), |_| None);
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
            actual.run_wild_monsters(K, Some(&data));
            actual.flush_moves();
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
        // The leader gets 100 HP: a wild hit is 30 with the `attack / 3` term (#512), which would
        // kill a 30 HP leader and change the draws.
        d.team[0].info.hp = 100;
        d.team[0].info.max_hp = 100;
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
        d.run_wild_monsters(K, Some(&data));
        d.flush_moves();
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
        d.run_wild_monsters(K, Some(&data));
        d.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
        // `Int(35)@02193260`, then `Int(100)@02186894`; the walk itself draws nothing.
        expected.int(35);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.wild[0].pos, (6, 5));
        assert_eq!(d.wild[0].info.direction, 2);
    }

    #[test]
    fn a_walker_draws_its_end_of_turn_at_the_flush_after_the_leaders() {
        // #510: a walk only marks the monster; the flush draws the leader's, then the walkers'.
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        let belly = d.team[0].info.belly;
        d.pending_end_of_turns = 1;
        let mut expected = d.rng;
        d.run_wild_monsters(K, None);
        // The pick only: no end-of-turn draw yet, the leader's belly has not ticked.
        expected.int(35);
        assert_eq!(d.rng, expected);
        assert_eq!(d.team[0].info.belly, belly);
        d.flush_moves();
        expected.int(100); // the leader
        expected.int(100); // the wild walker
        assert_eq!(d.rng, expected);
        assert!(d.team[0].info.belly < belly);
        assert_eq!(d.pending_end_of_turns, 0);
        // Nothing is pending any more.
        d.flush_moves();
        assert_eq!(d.rng, expected);
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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

    /// A forced spawn: a species-7 monster at `at`, set up as `try_spawn_wild` does after its slot.
    fn forced_spawn(
        leader: (i16, i16),
        at: (i16, i16),
        chance: u8,
        random_walk: bool,
    ) -> (GameState, DungeonRng) {
        let mut g = room_apart(at);
        g.dungeon_mut().unwrap().team[0].pos = leader;
        let d = g.dungeon_mut().unwrap();
        d.ai.chance_asleep = vec![0, 0, 0, 0, 0, 0, 0, chance];
        let expected = d.rng;
        d.setup_spawned_wild(at, random_walk);
        (g, expected)
    }

    #[test]
    fn a_spawn_in_a_room_draws_the_exit_then_the_sleep_roll() {
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        let exits: [(i16, i16); 3] = [(5, 5), (10, 5), (12, 19)];
        for (x, y) in exits {
            d.floor.tile_mut(x.into(), y.into()).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        }
        let mut expected = d.rng;
        d.setup_spawned_wild((5, 5), false);
        // The exit count is the draw's bound; the monster's own tile is retried.
        let goal = loop {
            let e = exits[expected.int(3) as usize];
            if e != (5, 5) {
                break e;
            }
        };
        expected.int(100);
        assert_eq!(d.rng, expected);
        let rec = d.ai.wild[&(5, 5)];
        assert_eq!((rec.objective, rec.goal), (LEAVE_ROOM, goal));
        assert!(!d.wild[0].info.is_asleep());
    }

    #[test]
    fn a_spawn_in_a_corridor_takes_the_corridor_branch_and_draws_no_exit() {
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        d.floor.tile_mut(5, 5).unwrap().room = NO_ROOM;
        let mut expected = d.rng;
        d.setup_spawned_wild((5, 5), false);
        // Open neighbours that are not a straight run: the first walkable direction, no draw.
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.ai.wild[&(5, 5)].objective, ROAM);
    }

    #[test]
    fn a_spawn_that_sees_the_team_targets_it_and_draws_only_the_sleep_roll() {
        let (g, mut expected) = forced_spawn((5, 12), (5, 5), 0, false);
        let d = g.dungeon().unwrap();
        expected.int(100);
        assert_eq!(d.rng, expected);
        let rec = d.ai.wild[&(5, 5)];
        assert_eq!((rec.objective, rec.goal), (CHASE_TARGET, (5, 12)));
    }

    #[test]
    fn the_movement_flag_is_set_after_the_first_wander_and_read_by_the_next() {
        // `02179CFC`: the first objective (line 19) runs before `+0xF0` is written (line 31).
        let mut g = room_apart((5, 5));
        let d = g.dungeon_mut().unwrap();
        let exits: [(i16, i16); 3] = [(5, 5), (10, 5), (12, 19)];
        for (x, y) in exits {
            d.floor.tile_mut(x.into(), y.into()).unwrap().terrain |= floor::TERRAIN_JUNCTION;
        }
        let mut expected = d.rng;
        d.setup_spawned_wild((5, 5), true);
        let goal = loop {
            let e = exits[expected.int(3) as usize];
            if e != (5, 5) {
                break e;
            }
        };
        expected.int(100);
        assert_eq!(
            d.rng, expected,
            "the spawn's exit and sleep draws are unchanged"
        );
        let mut rec = d.ai.wild[&(5, 5)];
        assert_eq!((rec.objective, rec.goal), (LEAVE_ROOM, goal));
        assert!(rec.random_walk, "the record keeps the flag");
        // The next wander takes the random-neighbour branch.
        d.wander((5, 5), 0, &mut rec);
        expected.int(8);
        assert_eq!(d.rng, expected);
        assert_eq!(rec.objective, STAND_STILL);
    }

    #[test]
    fn a_sleeping_spawn_gets_the_no_countdown_sleep_when_the_roll_is_below_the_species_chance() {
        // The roll after the wander's draw is a fixed value of this seed: bracket it.
        let (g, mut probe) = forced_spawn((50, 28), (5, 5), 0, false);
        let _ = g;
        let roll = {
            // The room has no exits, so the wander draws a neighbour (`Int(8)`) first.
            probe.int(8);
            probe.int(100) as u8
        };
        let (g, _) = forced_spawn((50, 28), (5, 5), roll + 1, false);
        let s = g.dungeon().unwrap().wild[0].info.status;
        assert_eq!((s.sleep, s.sleep_turns), (status::SLEEP, status::NO_END));
        assert!(g.dungeon().unwrap().wild[0].info.is_asleep());
        let (g, _) = forced_spawn((50, 28), (5, 5), roll, false);
        assert!(!g.dungeon().unwrap().wild[0].info.is_asleep());
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
            d.run_wild_monsters(K, None);
            d.flush_moves();
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
            flags: 0,
            random_walk: false,
        };
        d.ai.wild.insert((5, 5), held);
        let mut expected = d.rng;
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
            run.run_wild_monsters(K, None);
            run.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
        assert_eq!(d.ai.wild.values().next().map(|r| r.objective), Some(1));
        // The leader steps out of sight, leaving (12, 5) first in its trail.
        d.floor.rooms[0].x1 = 20;
        d.floor.rooms[0].y1 = 20;
        d.team[0].pos = (50, 28);
        let mut expected = d.rng;
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
    fn each_wild_monster_is_gated_by_its_own_speed_stage() {
        let mut g = room_with((12, 5), &[(5, 5), (5, 20)]);
        let d = g.dungeon_mut().unwrap();
        // Paralysed: stage 0 acts at 7, 15 and 23 only. Hasted: stage 2 also acts at 1.
        d.wild[0].info.status.burn_class = status::PARALYSIS;
        d.wild[1].info.speed_up[0] = 9;
        let moved = |d: &Dungeon| (d.wild[0].pos != (5, 5), d.wild[1].pos != (5, 20));
        d.run_wild_monsters(3, None);
        d.flush_moves();
        assert_eq!(moved(d), (false, true));
        // At 1 only the hasted one acts: its draws advance the generator, a paralysed one's would too.
        let before = d.rng;
        d.run_wild_monsters(1, None);
        d.flush_moves();
        assert_ne!(d.rng, before);
        assert_eq!(d.wild[0].pos, (5, 5));
        d.run_wild_monsters(7, None);
        d.flush_moves();
        assert!(d.wild[0].pos != (5, 5));
    }

    #[test]
    fn the_flags_skip_a_wild_monster_and_the_skip_once_flag_is_cleared() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.ai.wild.insert(
            (5, 5),
            AiRecord {
                flags: 0x8000,
                ..AiRecord::default()
            },
        );
        d.run_wild_monsters(K, None);
        d.flush_moves();
        assert_eq!(d.wild[0].pos, (5, 5));
        d.ai.wild.insert(
            (5, 5),
            AiRecord {
                flags: 0x4000,
                ..AiRecord::default()
            },
        );
        d.run_wild_monsters(K, None);
        d.flush_moves();
        assert_eq!((d.wild[0].pos, d.ai.wild[&(5, 5)].flags), ((5, 5), 0));
        d.run_wild_monsters(K, None);
        d.flush_moves();
        assert_ne!(d.wild[0].pos, (5, 5));
    }

    #[test]
    fn the_skip_once_flag_is_cleared_in_a_turn_the_stage_does_not_act() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.ai.wild.insert(
            (5, 5),
            AiRecord {
                flags: 0x4000,
                ..AiRecord::default()
            },
        );
        d.run_wild_monsters(1, None);
        d.flush_moves();
        assert_eq!((d.wild[0].pos, d.ai.wild[&(5, 5)].flags), ((5, 5), 0));
    }

    #[test]
    fn a_sleeping_monster_draws_only_the_end_of_turn_roll_and_stays_put() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.wild[0].info.status.sleep = 1;
        d.wild[0].info.status.sleep_turns = 3;
        let mut expected = d.rng;
        d.run_wild_monsters(K, None);
        d.flush_moves();
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.wild[0].pos, (5, 5));
        // Its countdown ran first, in its own turn.
        assert_eq!(d.wild[0].info.status.sleep_turns, 2);
    }

    fn tackle_data(moves: &[rescue_data::moves::Move]) -> CombatData<'_> {
        CombatData {
            tables: Box::leak(Box::new(crate::combat::tests::tables())),
            moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        }
    }

    #[test]
    fn a_sleeping_or_flinching_monster_next_to_the_leader_skips_the_pick_and_the_attack() {
        // Paralysis (`+0x9B == 4`) is gated too; it also slows the monster, so only the gate is
        // checked for it.
        let mut paralysed = crate::state::wild_monster(7, 4, (5, 5)).info;
        paralysed.status.burn_class = status::PARALYSIS;
        assert!(paralysed.skips_move_choice());
        paralysed.status.burn_class = status::BURN;
        assert!(!paralysed.skips_move_choice());
        paralysed.status.cringe = status::CONFUSED;
        assert!(!paralysed.skips_move_choice() && paralysed.is_confused());
        for (n, set) in [
            |s: &mut crate::status::Status| (s.sleep, s.sleep_turns) = (status::SLEEP, 5),
            |s: &mut crate::status::Status| (s.cringe, s.cringe_turns) = (status::FLINCH, 5),
        ]
        .into_iter()
        .enumerate()
        {
            let (mut g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 6)]);
            let data = tackle_data(&moves);
            let d = g.dungeon_mut().unwrap();
            set(&mut d.wild[0].info.status);
            let hp = d.team[0].info.hp;
            let mut expected = d.rng;
            d.run_wild_monsters(K, Some(&data));
            d.flush_moves();
            // No weighted pick and no attack: only the end-of-turn roll (a sleeper stays; the
            // others take the movement step, which here is blocked by the leader).
            expected.int(100);
            assert_eq!(d.rng, expected, "mode {n}");
            assert_eq!(d.team[0].info.hp, hp);
        }
    }

    #[test]
    fn a_confused_monster_skips_the_move_choice_on_a_70_percent_roll_and_otherwise_picks() {
        let (g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 6)]);
        let data = tackle_data(&moves);
        let seed_with = |skip: bool| {
            (1u32..)
                .find(|&s| crate::rng::DungeonRng::new(s).outcome(70) == skip)
                .unwrap()
        };
        // Skipped: the roll, then only the end of turn; the leader is untouched.
        let mut skipped = g.dungeon().unwrap().clone();
        skipped.wild[0].info.status.cringe = status::CONFUSED;
        skipped.wild[0].info.status.cringe_turns = status::NO_END;
        skipped.rng = crate::rng::DungeonRng::new(seed_with(true));
        let mut model = skipped.rng;
        skipped.run_wild_monsters(K, Some(&data));
        skipped.flush_moves();
        model.outcome(70);
        model.int(100);
        assert_eq!(skipped.rng, model);
        assert_eq!(
            skipped.team[0].info.hp,
            g.dungeon().unwrap().team[0].info.hp
        );
        // Not skipped: the roll is one extra draw ahead of the same turn without confusion.
        let mut rolled = g.dungeon().unwrap().clone();
        rolled.wild[0].info.status.cringe = status::CONFUSED;
        rolled.wild[0].info.status.cringe_turns = status::NO_END;
        rolled.rng = crate::rng::DungeonRng::new(seed_with(false));
        let mut plain = g.dungeon().unwrap().clone();
        plain.rng = rolled.rng;
        plain.rng.outcome(70);
        rolled.run_wild_monsters(K, Some(&data));
        rolled.flush_moves();
        plain.run_wild_monsters(K, Some(&data));
        plain.flush_moves();
        assert_eq!(rolled.rng, plain.rng);
        assert_eq!(rolled.team[0].info.hp, plain.team[0].info.hp);
    }

    #[test]
    fn a_confused_monster_that_finds_no_action_passes_and_never_walks() {
        // One move of weight 6 plus the regular attack's `REGULAR[1]`: the pick draws 13.
        let (g, moves) = duel(&[(0x9A, 5, 10)], &[(0x9A, 6)]);
        let data = tackle_data(&moves);
        let seed = (1u32..)
            .find(|&s| !crate::rng::DungeonRng::new(s).outcome(70))
            .unwrap();
        let mut d = g.dungeon().unwrap().clone();
        // No foe adjacent: `ChooseAIMove` sets no action; `02141878` draws nothing.
        d.wild[0].pos = (14, 10);
        d.wild[0].info.status.cringe = status::CONFUSED;
        d.wild[0].info.status.cringe_turns = status::NO_END;
        d.rng = crate::rng::DungeonRng::new(seed);
        let mut model = d.rng;
        let start = d.wild[0].pos;
        d.run_wild_monsters(K, Some(&data));
        d.flush_moves();
        model.outcome(70);
        model.int(REGULAR[1] + 6);
        model.int(100);
        assert_eq!(d.wild[0].pos, start);
        assert_eq!(d.rng, model);
    }

    #[test]
    fn the_regular_attack_takes_the_first_adjacent_foe_from_the_facing_byte() {
        // The leader on the left (direction 6) and the partner on the right (direction 2).
        let mut g = with_partner((10, 10), (12, 10), &[(11, 10)]);
        let d = g.dungeon_mut().unwrap();
        for (facing, first) in [(0, 2), (2, 2), (3, 6), (4, 6), (6, 6), (7, 2)] {
            d.wild[0].info.direction = facing;
            assert_eq!(
                d.regular_attack_direction(Slot::Wild(0)),
                Some(first),
                "facing {facing}"
            );
        }
        // A wall on both sides of a diagonal foe closes the corner: it is not a candidate.
        d.team[1].pos = (12, 11);
        d.floor.tile_mut(11, 11).unwrap().terrain = 0;
        d.wild[0].info.direction = 0;
        assert_eq!(d.regular_attack_direction(Slot::Wild(0)), Some(6));
    }

    #[test]
    fn a_wild_monster_does_not_step_round_a_wall_corner_or_onto_non_floor() {
        let mut g = room_with((20, 20), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.floor.tile_mut(6, 5).unwrap().terrain = 0;
        assert_eq!(d.can_move((5, 5), 2), (false, false), "the wall");
        assert_eq!(d.can_move((5, 5), 1), (false, false), "the corner");
        assert_eq!(d.can_move((5, 5), 0), (true, false));
        d.floor.tile_mut(5, 6).unwrap().terrain = floor::TERRAIN_FLOOR | 0x10;
        assert_eq!(d.can_move((5, 5), 0), (false, false), "the wall bit");
        d.wild.push(crate::state::wild_monster(7, 4, (4, 5)));
        assert_eq!(d.can_move((5, 5), 6), (false, true), "a monster");
    }

    #[test]
    fn a_monster_wakes_when_its_last_sleep_turn_is_ticked_and_acts_that_turn() {
        let mut g = room_with((12, 5), &[(5, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.wild[0].info.status.sleep = 1;
        d.wild[0].info.status.sleep_turns = 1;
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
        d.run_wild_monsters(K, None);
        d.flush_moves();
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
    fn a_partner_regenerates_one_hp_at_its_seventh_tick_and_draws_nothing() {
        // Speed 200, max HP 30: 7 x 30 = 210 >= 200 (#514, the trace's partner at row 32).
        let mut g = with_partner((12, 5), (5, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        d.team[1].info.hp = 20;
        for tick in 1..=7 {
            d.ally_turn(1, None, false);
            d.flush_moves();
            let want = if tick < 7 { 20 } else { 21 };
            assert_eq!(d.team[1].info.hp, want, "tick {tick}");
        }
        assert_eq!(d.team[1].info.regen_acc, 10);
        // The recalc pass does not tick again.
        d.ally_turn(1, None, true);
        assert_eq!(d.team[1].info.regen_acc, 10);
        // The regeneration itself draws nothing.
        let rng = d.rng;
        Dungeon::regenerate_gated(&mut d.team[1].info);
        assert_eq!(d.rng, rng);
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
        d.flush_moves();
        assert_eq!(d.rng, after_pass);
        // The flag is cleared by the skip: the next team loop runs the partner.
        d.run_team_members(None);
        d.flush_moves();
        assert_ne!(d.rng, after_pass);
    }

    #[test]
    fn a_partner_follows_the_leader_and_draws_the_pick_and_the_end_of_turn() {
        let mut g = with_partner((12, 5), (5, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_team_members(None);
        d.flush_moves();
        // `Int(65)@02193260` once (a walking turn), then `Int(100)@02186894`.
        expected.int(65);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(d.team[1].pos, (6, 5));
        assert_eq!(d.team[1].info.direction, 2);
    }

    #[test]
    fn a_partner_with_a_distant_foe_in_view_still_follows_the_leader() {
        // Leader to the south (0), foe 3 tiles east (2), not adjacent: cause B of #348 row 8.
        let mut g = with_partner((10, 14), (10, 10), &[(13, 10)]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_team_members(None);
        d.flush_moves();
        expected.int(65);
        expected.int(100);
        assert_eq!(d.rng, expected);
        assert_eq!(
            d.team[1].pos,
            (10, 11),
            "it heads for the leader, not the foe"
        );
        assert_eq!(d.team[1].info.direction, 0);
    }

    /// A partner (Growl pp 20, Scratch pp 22, flags `05`: the game's) next to a foe, the move
    /// table with the weights Growl 10 and Scratch 25, and the game's regular-attack table, so
    /// the pick total is 10 + 25 + 30 = 65 (#507, P-R7a).
    fn partner_duel() -> (GameState, Vec<rescue_data::moves::Move>) {
        let mut g = with_partner((12, 5), (5, 5), &[(6, 5)]);
        let d = g.dungeon_mut().unwrap();
        for (slot, id, pp) in d.team[1]
            .info
            .moves
            .iter_mut()
            .zip([(move_effects::GROWL, 20), (0x104, 22)])
            .map(|(slot, (id, pp))| (slot, id, pp))
        {
            *slot = MoveSlot {
                flags: MoveSlot::HOLDS_MOVE | SLOT_AI_ENABLED,
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
        for (id, weight) in [(move_effects::GROWL, 10), (0x104, 25)] {
            let mut mv = crate::combat::tests::move_of(id, 40, 1, [100; 3], 0);
            mv.ai_weight = weight;
            list.push(mv);
        }
        (g, crate::combat::tests::table(&list))
    }

    #[test]
    fn a_partner_picks_growl_scratch_or_the_regular_attack_by_weight() {
        let (g, moves) = partner_duel();
        let mut tables = crate::combat::tests::tables();
        tables.regular_attack_weight = [100, 20, 30, 40, 50];
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let mut seen = [false; 3];
        for seed in 0..200 {
            let mut actual = g.dungeon().unwrap().clone();
            actual.rng = crate::rng::DungeonRng::new(seed);
            let mut model = actual.clone();
            actual.run_team_members(Some(&data));
            actual.flush_moves();
            // `Int(65)@02193260`, 0-based: Growl 1..=10, Scratch 11..=35, the regular attack after.
            let roll = model.rng.int(65);
            let entry = match roll {
                ..=10 => Some(0),
                11..=35 => Some(1),
                _ => None,
            };
            match entry {
                Some(slot) => {
                    // The move's target pick (`02192CC0`), then its use.
                    model.rng.int(1);
                    let id = model.team[1].info.moves[slot].id;
                    let mv = &moves[usize::from(id)];
                    let effect = move_effects::effect_of(id).unwrap();
                    model.strike(Slot::Team(1), slot, &[Slot::Wild(0)], mv, &effect, &data);
                }
                None => {
                    // The regular entry draws no target pick.
                    let mv = &moves[usize::from(REGULAR_ATTACK)];
                    let attack = Effect::Damage {
                        arg8: REGULAR_ATTACK_ARG8,
                    };
                    model.resolve_all(Slot::Team(1), &[Slot::Wild(0)], mv, &attack, &data);
                }
            }
            model.rng.int(100);
            assert_eq!(actual.rng, model.rng, "seed {seed}, roll {roll}");
            assert_eq!(actual.team[1].info.moves, model.team[1].info.moves);
            assert_eq!(
                actual
                    .wild
                    .first()
                    .map(|w| (w.info.hp, w.info.offensive_stages)),
                model
                    .wild
                    .first()
                    .map(|w| (w.info.hp, w.info.offensive_stages)),
                "seed {seed}"
            );
            assert_eq!(actual.team[1].pos, (5, 5), "an attack does not move");
            assert_eq!(actual.team[1].info.direction, 2, "it faces the foe");
            let pps = (
                actual.team[1].info.moves[0].pp,
                actual.team[1].info.moves[1].pp,
            );
            let used = match pps {
                (19, 22) => 0,
                (20, 21) => 1,
                _ => 2,
            };
            assert_eq!(Some(used), entry.or(Some(2)), "roll {roll}");
            seen[used] = true;
        }
        assert_eq!(seen, [true; 3]);
    }

    #[test]
    fn a_partner_rolls_8_to_growl_and_32_to_scratch() {
        let (g, moves) = partner_duel();
        let mut tables = crate::combat::tests::tables();
        tables.regular_attack_weight = [100, 20, 30, 40, 50];
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        for (want, slot) in [(8, 0), (32, 1)] {
            let seed = (0..100_000)
                .find(|&s| crate::rng::DungeonRng::new(s).int(65) == want)
                .unwrap();
            let mut d = g.dungeon().unwrap().clone();
            d.rng = crate::rng::DungeonRng::new(seed);
            d.run_team_members(Some(&data));
            d.flush_moves();
            let pp = [20, 22];
            for (i, m) in d.team[1].info.moves.iter().take(2).enumerate() {
                assert_eq!(m.pp, pp[i] - u8::from(i == slot), "roll {want} slot {i}");
            }
        }
    }

    #[test]
    fn a_partner_regular_entry_roll_draws_no_candidate_pick() {
        let (g, moves) = partner_duel();
        let mut tables = crate::combat::tests::tables();
        tables.regular_attack_weight = [100, 20, 30, 40, 50];
        let data = CombatData {
            tables: &tables,
            moves: &moves,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        };
        let seed = (0..100_000)
            .find(|&s| crate::rng::DungeonRng::new(s).int(65) == 61)
            .unwrap();
        let mut d = g.dungeon().unwrap().clone();
        d.rng = crate::rng::DungeonRng::new(seed);
        let mut model = d.clone();
        d.run_team_members(Some(&data));
        d.flush_moves();
        model.rng.int(65);
        let mv = &moves[usize::from(REGULAR_ATTACK)];
        let attack = Effect::Damage {
            arg8: REGULAR_ATTACK_ARG8,
        };
        model.resolve_all(Slot::Team(1), &[Slot::Wild(0)], mv, &attack, &data);
        model.rng.int(100);
        assert_eq!(d.rng, model.rng);
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
        d.flush_moves();
        // Phase 1: A picks once (blocked, flagged); B picks once and steps on, end of turn.
        // Phase 2: A, `recalculateFollow` set, picks once and steps into the freed tile with
        // its end of turn; nothing is left for the final pass.
        for cap in [65, 65, 100, 65, 100] {
            expected.int(cap);
        }
        assert_eq!(d.rng, expected);
        assert_eq!((d.team[1].pos, d.team[2].pos), ((6, 5), (7, 5)));
    }

    #[test]
    fn a_partner_next_to_the_leader_stops_flagged_in_phase_1_and_passes_in_phase_2() {
        // Branch 2a: the direct step is blocked by a monster (the leader) in phase 1, so it
        // passes flagged. Branch 2b: in phase 2 the goal is exactly that tile, so it passes
        // unflagged and its end-of-turn runs. One pick per pass: no second cycle (#504).
        let mut g = with_partner((6, 5), (5, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        let mut expected = d.rng;
        d.run_team_members(None);
        d.flush_moves();
        for cap in [65, 65, 100] {
            expected.int(cap);
        }
        assert_eq!(d.rng, expected);
        assert_eq!(d.team[1].pos, (5, 5));
    }

    #[test]
    fn a_partner_whose_leader_is_out_of_sight_does_not_chase_it() {
        // The partner stands in a corridor far from the leader: `GetLeaderIfVisible` fails and
        // the partner wanders (`AIWander`) instead of taking the leader's tile as its goal.
        let mut g = with_partner((10, 10), (40, 20), &[]);
        let d = g.dungeon_mut().unwrap();
        let t = d.floor.tile_mut(40, 20).unwrap();
        t.room = NO_ROOM;
        assert!(!d.in_view((40, 20), (10, 10)));
        d.run_team_members(None);
        d.flush_moves();
        assert_ne!(d.ai.team[1].rec.objective, CHASE_TARGET);
        assert_ne!(d.ai.team[1].rec.goal, (10, 10));
    }

    #[test]
    fn a_tactic_that_scans_enemies_targets_the_foe_in_view() {
        // Tactic 1 reads 0 in the table at `0x02091E48`: the scan runs and the foe is the target.
        let mut g = with_partner((10, 14), (10, 10), &[(13, 10)]);
        let d = g.dungeon_mut().unwrap();
        d.sync_team_ai();
        d.ai.team[1].tactic = 1;
        d.run_team_members(None);
        d.flush_moves();
        assert_eq!(d.team[1].pos, (11, 10), "it heads for the foe");
        assert!(TACTIC_SKIPS_SCAN[0] && !TACTIC_SKIPS_SCAN[1]);
        assert!([4, 5, 6, 7, 8, 10].iter().all(|&t| TACTIC_SKIPS_SCAN[t]));
    }

    fn team_mover() -> Mover {
        Mover {
            team: true,
            recalc: false,
            wide: false,
        }
    }

    #[test]
    fn a_team_member_on_a_diagonal_drops_one_45_degree_turn() {
        // Branch 2c: heading down-right (1) with the goal 2 right and 1 down, the direct tile
        // is a wall, the turn along the longer axis (right) holds a wild monster and the other
        // turn (down) is open but dropped for a team member, so it passes (flagged by the
        // monster), while a wild monster walks down.
        let mut g = with_partner((12, 6), (10, 5), &[(11, 5)]);
        let d = g.dungeon_mut().unwrap();
        d.floor.tile_mut(11, 6).unwrap().terrain = floor::TERRAIN_IMPASSABLE_WALL;
        let (plan, flagged) = d.decide_movement((10, 5), (12, 6), team_mover());
        assert!(matches!(plan, Plan::Pass) && flagged);
        let (plan, _) = d.decide_movement((10, 5), (12, 6), Mover::WILD);
        assert!(matches!(plan, Plan::Walk(0)));
    }

    #[test]
    fn a_terrain_blocked_candidate_re_enables_the_dropped_turn() {
        // Same layout with a wall at (11,5) instead of the monster: the terrain block re-enables
        // both turns (`02190FEC`), so the team member walks down too.
        let mut g = with_partner((12, 6), (10, 5), &[]);
        let d = g.dungeon_mut().unwrap();
        for p in [(11, 6), (11, 5)] {
            d.floor.tile_mut(p.0, p.1).unwrap().terrain = floor::TERRAIN_IMPASSABLE_WALL;
        }
        let (plan, _) = d.decide_movement((10, 5), (12, 6), team_mover());
        assert!(matches!(plan, Plan::Walk(0)));
    }
}
