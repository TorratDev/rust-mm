//! Traps (`docs/re/items.md`, P5): trap entities from the spawn marks, who a trap springs on,
//! and the effects of the decoded ids.
//!
//! A trap is a floor object of entity type 2 with a two-byte record: the id and the side that
//! laid it (`by`, record byte 1), plus the seen flag (entity `+0x20`). [`place_traps`] is
//! `PlaceRandomTraps` (`02198824`): one trap per tile with the trap mark (spawn `0x4`), id 6 on a
//! tile that also has the warp mark (`0x40`), else a weighted id from `PickRandomTrapId`
//! (`02138C68`). A placed trap has `by` 0, or 2 and seen for the wonder tile (id `0x11`). The
//! step trigger is `02165B00` (leader) and `02187420` (every other monster), see
//! [`Dungeon::step_on_trap`]; the dispatcher is `02197D88`.
//!
//! Effects decoded here: 2 (sticky), `0xB` (seal), `0xF` (PP), `0x10` and `0x13` (damage).
//!
//! # Shortcuts
//!
//! - Placement: the scan order of `02198824` is not recorded, so tiles go row by row (y, then x).
//!   The id roll is `DungeonRandInt(0x2710)` against cumulative weights (`roll <= weight`,
//!   zero weights skipped, fallback `0x10`), the item picker's form: the bound is assumed, the
//!   note records only "cumulative weights" and the fallback. With no weights given
//!   ([`TrapTables::weights`] empty) no trap is placed and nothing is drawn, so floors that were
//!   built without trap tables keep their draws. The weight table (`mapparam` trap list) is
//!   taken from the caller; the cap on trap entities is not modelled.
//! - Not placed: fixed-room traps (`02152FAC`), traps laid by a move (`0215FEEC`) and
//!   `021983F8` reuse. Floor entry is the only writer.
//! - The see-traps effect (`021805B0(e, 0x13)`, which reveals a trap instead of firing) is not
//!   modelled: nobody has it. A trap is seen only when it fires or is wonder tile.
//! - The avoid roll: `DungeonRandInt(100) < 15` avoids an unseen trap for a wild monster
//!   (`gDungeon+0x15FA7` is taken as 0, id `0x11` skips it). The note says "monster target"
//!   and "not the leader's team"; this reads it as wild monsters. A team member avoids with the
//!   Trap Scarf (held item `0x1C`) and draws nothing. The order of the scarf test against the
//!   roll is not recorded; they never both apply.
//! - `0217BB20` (`info+8`) and the post-effect `02183BB4` are not read, so not run. The sprung
//!   message is not modelled.
//! - A team member that a leader swap moves onto a tile does not spring a trap there.
//! - Damage (ids `0x10`, `0x13`): `DealDamageToEntity_Async` is reduced to the HP loss, with no
//!   draw of its own; the damage values sit behind pointer slots the notes do not read, so they
//!   come from [`TrapTables`] (0 by default). A wild monster taken to 0 HP stays on the floor:
//!   the faint is not run (a wild turn is mid-loop).
//! - Sticky (id 2): the guard `021432E8(m, 0xE)` is not read and is taken as passing, and the
//!   bag pick is read as the bag slots that hold an item and are not sticky (the note says
//!   "bit 1", 1-based as in P7). Any team member's trap sticks an item of the team bag; a wild
//!   monster has none. No candidate draws nothing.
//! - Seal (id `0xB`): the guard `02180870(user, t, 1)` is not read and is taken as 0. Byte 1 bit
//!   0 of a move slot is the sealed bit; no candidate draws nothing.
//! - PP (id `0xF`): the tail call `02175C34` is not read.
//! - Id 0 (launcher), 1 (mud), 3 (grimy), 4 (summon), 5 and `0x1B` (pitfall), 6 (warp), 7
//!   (gust), 8 (spin), 9 (sleep), 0xA (slow), 0xC (poison), 0xD and 0xE (explosions), `0x11`
//!   (wonder tile), `0x12` (items to monsters), `0x14` to `0x1A` (empty in the game): springing
//!   one marks it seen and does nothing, with no draw. Ids 4 and `0x12` would consume the trap;
//!   here they stay.

use serde::{Deserialize, Serialize};

use crate::floor::{self, Floor};
use crate::items::FLAG_STICKY;
use crate::rng::DungeonRng;
use crate::state::{Dungeon, MoveSlot};
use crate::targeting::Slot;

/// Trap ids in a floor's weight list.
pub const TRAP_IDS: usize = 20;

/// The trap mark in a tile's spawn word (`SPAWN_TRAP`, `docs/re/structs.md`).
const SPAWN_TRAP: u16 = 0x4;
/// The warp mark: the tile gets trap id 6 (`SPAWN_WARP_TILE`).
const SPAWN_WARP: u16 = 0x40;
/// The id roll's bound (the item picker's, see the module shortcuts).
const ROLL: i32 = 0x2710;
/// The id `PickRandomTrapId` falls back to.
const FALLBACK_ID: u8 = 0x10;

/// Trap ids with a decoded effect.
pub mod id {
    pub const STICKY: u8 = 2;
    pub const WARP: u8 = 6;
    pub const SEAL: u8 = 0xB;
    pub const PP: u8 = 0xF;
    /// Damage trap (Red's chestnut).
    pub const DAMAGE: u8 = 0x10;
    pub const WONDER: u8 = 0x11;
    /// Spike trap.
    pub const SPIKE: u8 = 0x13;
}

/// Who laid the trap (record byte 1): natural, a team member, or a wild monster.
pub mod by {
    pub const NATURAL: u8 = 0;
    pub const TEAM: u8 = 1;
    pub const WILD: u8 = 2;
}

/// The held item that always avoids a trap (Trap Scarf).
pub const TRAP_SCARF: u8 = 0x1C;
/// The avoid roll's bound and threshold (`02197D88`).
const AVOID_BOUND: i32 = 100;
const AVOID_UNDER: i32 = 15;

/// One trap entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trap {
    /// Record byte 0.
    pub id: u8,
    /// The tile (entity `+4`, `+6`).
    pub pos: (i16, i16),
    /// Record byte 1: who laid it (see [`by`]).
    pub by: u8,
    /// Entity `+0x20`: the trap has been seen.
    pub seen: bool,
}

/// What the traps of a run need from the ROM: the id weights of the floor and the damage
/// numbers. The game reads them per floor from `mapparam` and two pointer slots; the sim takes
/// one set for the run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrapTables {
    /// The floor's cumulative weights, one per trap id (see [`TRAP_IDS`]). Empty: no traps are
    /// placed.
    pub weights: Vec<u16>,
    /// The damage of id `0x10` (`s16 *PTR_021AA084`).
    pub damage: i16,
    /// The damage of id `0x13` (`s16 *PTR_021AA0A8`).
    pub spike_damage: i16,
}

/// The traps of a floor and the tables they use.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrapState {
    pub(crate) list: Vec<Trap>,
    pub(crate) tables: TrapTables,
}

/// `PickRandomTrapId` (`02138C68`): one roll, the first id with a non-zero cumulative weight at
/// or above it, else [`FALLBACK_ID`].
fn pick_id(tables: &TrapTables, rng: &mut DungeonRng) -> u8 {
    let roll = rng.int(ROLL);
    tables
        .weights
        .iter()
        .take(TRAP_IDS)
        .position(|&w| w != 0 && roll <= i32::from(w))
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(FALLBACK_ID)
}

/// `PlaceRandomTraps` (`02198824`): the traps of a floor just generated, read from the spawn
/// marks (call before the visibility reset clears them, and before the floor items).
pub(crate) fn place_traps(floor: &Floor, tables: &TrapTables, rng: &mut DungeonRng) -> Vec<Trap> {
    let mut list = Vec::new();
    if tables.weights.is_empty() {
        return list;
    }
    for y in 0..floor::HEIGHT as i32 {
        for x in 0..floor::WIDTH as i32 {
            let Some(tile) = floor.tile(x, y) else {
                continue;
            };
            let marks = tile.spawn_or_visibility;
            if marks & SPAWN_TRAP == 0 {
                continue;
            }
            let id = if marks & SPAWN_WARP != 0 {
                id::WARP
            } else {
                pick_id(tables, rng)
            };
            let wonder = id == id::WONDER;
            list.push(Trap {
                id,
                pos: (x as i16, y as i16),
                by: if wonder { by::WILD } else { by::NATURAL },
                seen: wonder,
            });
        }
    }
    list
}

/// Whether a trap laid by `by` springs on a monster (`02165B00` for the leader, `02187420` for
/// the others): the leader on side 0 or 2; a team partner on side 0 once seen, or side 2; a wild
/// monster on side 1. See the table in `docs/re/items.md`, P5b.
const fn springs_on(slot: Slot, by: u8, seen: bool) -> bool {
    match slot {
        Slot::Team(0) => by == self::by::NATURAL || by == self::by::WILD,
        Slot::Team(_) => (by == self::by::NATURAL && seen) || by == self::by::WILD,
        Slot::Wild(_) => by == self::by::TEAM,
    }
}

impl Dungeon {
    /// The traps of the floor.
    #[must_use]
    pub fn traps(&self) -> &[Trap] {
        &self.traps.list
    }

    /// The trap on `pos`, if any.
    #[must_use]
    pub fn trap_at(&self, pos: (i16, i16)) -> Option<&Trap> {
        self.traps.list.iter().find(|t| t.pos == pos)
    }

    /// A monster has stepped onto its tile: springs the trap there when the rule allows
    /// (`02165B00`, `02187420`), through the dispatcher `02197D88(entity, pos, 0, 1)`. Returns
    /// the id of the trap that took effect; `None` when there is no trap, it does not spring on
    /// this monster, or the monster avoided it.
    pub(crate) fn step_on_trap(&mut self, slot: Slot) -> Option<u8> {
        let pos = match slot {
            Slot::Team(i) => self.team.get(i)?.pos,
            Slot::Wild(i) => self.wild.get(i)?.pos,
        };
        let index = self.traps.list.iter().position(|t| t.pos == pos)?;
        let trap = self.traps.list[index];
        if !springs_on(slot, trap.by, trap.seen) {
            return None;
        }
        // The avoid test comes before the seen flag is set, as the roll only counts while it is
        // clear.
        if trap.id != id::WONDER {
            let avoided = match slot {
                Slot::Wild(_) => {
                    let roll = self.rng.int(AVOID_BOUND);
                    roll < AVOID_UNDER && !trap.seen
                }
                Slot::Team(i) => self
                    .items
                    .held
                    .get(i)
                    .is_some_and(|h| h.exists() && h.id == TRAP_SCARF),
            };
            if avoided {
                return None;
            }
        }
        self.traps.list[index].seen = true;
        self.trap_effect(trap.id, slot);
        Some(trap.id)
    }

    /// The switch of `02197D88`.
    fn trap_effect(&mut self, trap: u8, slot: Slot) {
        match trap {
            id::STICKY => self.sticky_trap(slot),
            id::SEAL => self.seal_trap(slot),
            id::PP => self.pp_trap(slot),
            id::DAMAGE => self.trap_damage(slot, self.traps.tables.damage),
            id::SPIKE => self.trap_damage(slot, self.traps.tables.spike_damage),
            // shortcut: see the module doc.
            _ => {}
        }
    }

    /// `02197AF8`: one bag item that exists and is not sticky becomes sticky. The pick is
    /// `DungeonRandInt(n)` over those items in slot order.
    fn sticky_trap(&mut self, slot: Slot) {
        if matches!(slot, Slot::Wild(_)) {
            return;
        }
        let candidates: Vec<usize> = self
            .items
            .bag
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.exists() && !s.sticky())
            .map(|(i, _)| i)
            .collect();
        let Some(n) = i32::try_from(candidates.len()).ok().filter(|&n| n > 0) else {
            return;
        };
        let pick = usize::try_from(self.rng.int(n)).unwrap_or(0);
        if let Some(&i) = candidates.get(pick) {
            self.items.bag.slots[i].flags |= FLAG_STICKY;
        }
    }

    /// `021972E8`: one move slot that holds a move and is not sealed gets sealed, picked by
    /// `DungeonRandInt(n)`.
    fn seal_trap(&mut self, slot: Slot) {
        let Some(info) = self.info_of(slot) else {
            return;
        };
        let candidates: Vec<usize> = info
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.flags & MoveSlot::HOLDS_MOVE != 0 && m.flags2 & MOVE_SEALED == 0)
            .map(|(i, _)| i)
            .collect();
        let Some(n) = i32::try_from(candidates.len()).ok().filter(|&n| n > 0) else {
            return;
        };
        let pick = usize::try_from(self.rng.int(n)).unwrap_or(0);
        if let (Some(&i), Some(info)) = (candidates.get(pick), self.info_mut(slot)) {
            info.moves[i].flags2 |= MOVE_SEALED;
        }
    }

    /// `02197414`: one move that holds a move and has PP loses all of it, picked by
    /// `DungeonRandInt(n)`.
    fn pp_trap(&mut self, slot: Slot) {
        let Some(info) = self.info_of(slot) else {
            return;
        };
        let candidates: Vec<usize> = info
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.flags & MoveSlot::HOLDS_MOVE != 0 && m.pp != 0)
            .map(|(i, _)| i)
            .collect();
        let Some(n) = i32::try_from(candidates.len()).ok().filter(|&n| n > 0) else {
            return;
        };
        let pick = usize::try_from(self.rng.int(n)).unwrap_or(0);
        if let (Some(&i), Some(info)) = (candidates.get(pick), self.info_mut(slot)) {
            info.moves[i].pp = 0;
        }
    }

    /// Ids `0x10` and `0x13`: `DealDamageToEntity_Async` as the HP loss.
    fn trap_damage(&mut self, slot: Slot, damage: i16) {
        if let Some(info) = self.info_mut(slot) {
            info.hp = info.hp.saturating_sub(damage.max(0)).max(0);
        }
    }
}

/// Byte 1 bit 0 of a move slot: sealed.
const MOVE_SEALED: u8 = 0x01;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::tests::battle_with;
    use crate::combat::tests::move_of;
    use crate::floor::TERRAIN_FLOOR;
    use crate::items::Item;
    use crate::state::GameState;

    /// A floor with the leader at (5, 5), a wild monster at (9, 9), and an ally (slot 1) at
    /// (7, 7), all on floor tiles.
    fn setup() -> GameState {
        let mut g = battle_with(&move_of(0x9A, 40, 1, [100; 3], 0));
        let d = g.dungeon_mut().unwrap();
        for y in 1..12 {
            for x in 1..12 {
                d.floor.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        d.wild.truncate(1);
        let mut ally = d.team[0].clone();
        ally.info.is_team_leader = false;
        ally.pos = (7, 7);
        d.team.push(ally);
        d.team[0].pos = (5, 5);
        d.wild[0].pos = (9, 9);
        g
    }

    fn lay(g: &mut GameState, id: u8, pos: (i16, i16), by: u8, seen: bool) {
        g.dungeon_mut()
            .unwrap()
            .traps
            .list
            .push(Trap { id, pos, by, seen });
    }

    fn d(g: &mut GameState) -> &mut Dungeon {
        g.dungeon_mut().unwrap()
    }

    /// How many draws `act` makes on the floor generator, by replaying single steps.
    fn counted(g: &mut GameState, act: impl FnOnce(&mut Dungeon)) -> u32 {
        let before = d(g).rng;
        act(d(g));
        let after = d(g).rng.state();
        let mut probe = before;
        let mut n = 0;
        while probe.state() != after && n < 64 {
            probe.next_u16();
            n += 1;
        }
        assert!(n < 64, "more than 64 draws");
        n
    }

    #[test]
    fn placement_reads_the_marks_and_draws_one_roll_per_random_trap() {
        let mut g = setup();
        let tables = TrapTables {
            weights: {
                let mut w = vec![0; TRAP_IDS];
                w[0xB] = 10_000;
                w
            },
            ..TrapTables::default()
        };
        let floor = &mut d(&mut g).floor;
        for (x, y, m) in [
            (3, 2, SPAWN_TRAP),
            (8, 2, SPAWN_TRAP | SPAWN_WARP),
            (2, 4, SPAWN_TRAP),
        ] {
            floor.tile_mut(x, y).unwrap().spawn_or_visibility = m;
        }
        let floor = d(&mut g).floor.clone();
        let mut rng = DungeonRng::new(99);
        let start = rng;
        let list = place_traps(&floor, &tables, &mut rng);
        // Row by row: y = 2 (x = 3, then the warp tile at x = 8), then y = 4.
        let ids: Vec<(u8, (i16, i16))> = list.iter().map(|t| (t.id, t.pos)).collect();
        assert_eq!(ids, [(0xB, (3, 2)), (id::WARP, (8, 2)), (0xB, (2, 4))]);
        // Two rolls: the warp tile draws nothing.
        let mut model = start;
        model.int(ROLL);
        model.int(ROLL);
        assert_eq!(rng.state(), model.state());
        assert!(list.iter().all(|t| t.by == by::NATURAL && !t.seen));
    }

    #[test]
    fn the_id_roll_is_cumulative_skips_zero_weights_and_falls_back() {
        let mut weights = vec![0; TRAP_IDS];
        weights[3] = 3000;
        weights[0x11] = 10_000;
        let tables = TrapTables {
            weights,
            ..TrapTables::default()
        };
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let mut rng = DungeonRng::new(seed);
            let mut probe = rng;
            let roll = probe.int(ROLL);
            let picked = pick_id(&tables, &mut rng);
            assert_eq!(picked, if roll <= 3000 { 3 } else { 0x11 });
            seen.insert(picked);
        }
        assert_eq!(seen.len(), 2);
        // No weight reaches the roll: the fallback.
        let low = TrapTables {
            weights: vec![1; TRAP_IDS],
            ..TrapTables::default()
        };
        let mut rng = DungeonRng::new(5);
        let mut probe = rng;
        if probe.int(ROLL) > 1 {
            assert_eq!(pick_id(&low, &mut rng), FALLBACK_ID);
        }
    }

    #[test]
    fn a_wonder_tile_is_placed_seen_and_laid_by_a_wild_monster() {
        let mut g = setup();
        let mut weights = vec![0; TRAP_IDS];
        weights[0x11] = 10_000;
        let tables = TrapTables {
            weights,
            ..TrapTables::default()
        };
        d(&mut g).floor.tile_mut(4, 4).unwrap().spawn_or_visibility = SPAWN_TRAP;
        let floor = d(&mut g).floor.clone();
        let list = place_traps(&floor, &tables, &mut DungeonRng::new(1));
        assert_eq!(list.len(), 1);
        assert_eq!(
            (list[0].id, list[0].by, list[0].seen),
            (id::WONDER, by::WILD, true)
        );
    }

    #[test]
    fn no_tables_place_no_trap_and_draw_nothing() {
        let mut g = setup();
        d(&mut g).floor.tile_mut(4, 4).unwrap().spawn_or_visibility = SPAWN_TRAP;
        let floor = d(&mut g).floor.clone();
        let mut rng = DungeonRng::new(1);
        let state = rng.state();
        assert!(place_traps(&floor, &TrapTables::default(), &mut rng).is_empty());
        assert_eq!(rng.state(), state);
    }

    #[test]
    fn a_floor_entered_with_tables_places_its_marked_traps() {
        let mut g = setup();
        let mut weights = vec![0; TRAP_IDS];
        weights[0xF] = 10_000;
        let tables = TrapTables {
            weights,
            ..TrapTables::default()
        };
        let mut floor = d(&mut g).floor.clone();
        floor.tile_mut(6, 6).unwrap().spawn_or_visibility = SPAWN_TRAP;
        let team = d(&mut g).team.clone();
        let mut state = GameState::new(7).with_trap_tables(tables);
        state.enter_dungeon(floor, team, 0, &[]).unwrap();
        let dungeon = state.dungeon().unwrap();
        assert_eq!(dungeon.traps().len(), 1);
        assert_eq!(dungeon.trap_at((6, 6)).map(|t| t.id), Some(id::PP));
        // The marks were cleared with the floor reset, and the trap stays.
        let snap = GameState::restore_with(&state.snapshot(), &state.resume()).unwrap();
        assert_eq!(snap.dungeon().unwrap().traps(), dungeon.traps());
    }

    #[test]
    fn who_springs_what_follows_the_side_table() {
        // Rows: (by, seen) -> leader, partner, wild.
        let rows = [
            (by::NATURAL, false, [true, false, false]),
            (by::NATURAL, true, [true, true, false]),
            (by::TEAM, false, [false, false, true]),
            (by::TEAM, true, [false, false, true]),
            (by::WILD, false, [true, true, false]),
            (by::WILD, true, [true, true, false]),
        ];
        for (b, seen, want) in rows {
            let got = [Slot::Team(0), Slot::Team(1), Slot::Wild(0)].map(|s| springs_on(s, b, seen));
            assert_eq!(got, want, "by {b} seen {seen}");
        }
    }

    #[test]
    fn the_leader_springs_an_unseen_natural_trap_and_sets_it_seen_with_no_draw() {
        let mut g = setup();
        lay(&mut g, id::DAMAGE, (5, 5), by::NATURAL, false);
        d(&mut g).traps.tables.damage = 7;
        let hp = d(&mut g).team[0].info.hp;
        let n = counted(&mut g, |dd| {
            assert_eq!(dd.step_on_trap(Slot::Team(0)), Some(id::DAMAGE));
        });
        assert_eq!(n, 0);
        assert_eq!(d(&mut g).team[0].info.hp, hp - 7);
        assert!(d(&mut g).trap_at((5, 5)).unwrap().seen);
    }

    #[test]
    fn a_trap_laid_by_the_team_does_not_spring_on_the_leader_or_a_partner() {
        let mut g = setup();
        lay(&mut g, id::DAMAGE, (5, 5), by::TEAM, true);
        lay(&mut g, id::DAMAGE, (7, 7), by::TEAM, true);
        d(&mut g).traps.tables.damage = 7;
        assert_eq!(d(&mut g).step_on_trap(Slot::Team(0)), None);
        assert_eq!(d(&mut g).step_on_trap(Slot::Team(1)), None);
        let l = &d(&mut g).team[0].info;
        assert_eq!(l.hp, l.max_hp);
    }

    #[test]
    fn a_partner_springs_a_natural_trap_only_once_it_is_seen() {
        let mut g = setup();
        lay(&mut g, id::DAMAGE, (7, 7), by::NATURAL, false);
        d(&mut g).traps.tables.damage = 3;
        assert_eq!(d(&mut g).step_on_trap(Slot::Team(1)), None);
        d(&mut g).traps.list[0].seen = true;
        let hp = d(&mut g).team[1].info.hp;
        assert_eq!(d(&mut g).step_on_trap(Slot::Team(1)), Some(id::DAMAGE));
        assert_eq!(d(&mut g).team[1].info.hp, hp - 3);
    }

    #[test]
    fn a_wild_monster_springs_a_team_trap_after_one_avoid_roll() {
        let mut g = setup();
        lay(&mut g, id::SPIKE, (9, 9), by::TEAM, false);
        d(&mut g).traps.tables.spike_damage = 10;
        let mut model = d(&mut g).rng;
        let roll = model.int(100);
        let hp = d(&mut g).wild[0].info.hp;
        let n = counted(&mut g, |dd| {
            dd.step_on_trap(Slot::Wild(0));
        });
        assert_eq!(n, 1, "one DungeonRandInt(100)");
        if roll < AVOID_UNDER {
            assert_eq!(d(&mut g).wild[0].info.hp, hp);
            assert!(
                !d(&mut g).trap_at((9, 9)).unwrap().seen,
                "an avoided trap stays unseen"
            );
        } else {
            assert_eq!(d(&mut g).wild[0].info.hp, hp - 10);
        }
    }

    #[test]
    fn the_avoid_roll_only_counts_for_an_unseen_trap() {
        // Find a seed whose first roll is under 15, then check seen and unseen traps.
        let seed = (0..10_000)
            .find(|&s| DungeonRng::new(s).int(100) < AVOID_UNDER)
            .unwrap();
        for (seen, hurt) in [(false, false), (true, true)] {
            let mut g = setup();
            d(&mut g).rng = DungeonRng::new(seed);
            lay(&mut g, id::SPIKE, (9, 9), by::TEAM, seen);
            d(&mut g).traps.tables.spike_damage = 10;
            let hp = d(&mut g).wild[0].info.hp;
            let fired = d(&mut g).step_on_trap(Slot::Wild(0));
            assert_eq!(fired.is_some(), hurt);
            assert_eq!(d(&mut g).wild[0].info.hp < hp, hurt);
        }
    }

    #[test]
    fn the_wonder_tile_skips_the_avoid_roll() {
        let seed = (0..10_000)
            .find(|&s| DungeonRng::new(s).int(100) < AVOID_UNDER)
            .unwrap();
        let mut g = setup();
        d(&mut g).rng = DungeonRng::new(seed);
        // Laid by the team so a wild monster springs it; the wonder id never rolls.
        lay(&mut g, id::WONDER, (9, 9), by::TEAM, false);
        let state = d(&mut g).rng.state();
        assert_eq!(d(&mut g).step_on_trap(Slot::Wild(0)), Some(id::WONDER));
        assert_eq!(d(&mut g).rng.state(), state);
    }

    #[test]
    fn a_trap_scarf_avoids_for_a_team_member_with_no_draw() {
        let mut g = setup();
        d(&mut g).items.held[0] = Item::new(TRAP_SCARF, 1);
        lay(&mut g, id::DAMAGE, (5, 5), by::NATURAL, false);
        d(&mut g).traps.tables.damage = 7;
        let state = d(&mut g).rng.state();
        assert_eq!(d(&mut g).step_on_trap(Slot::Team(0)), None);
        assert_eq!(d(&mut g).rng.state(), state);
        assert!(!d(&mut g).trap_at((5, 5)).unwrap().seen);
        let l = &d(&mut g).team[0].info;
        assert_eq!(l.hp, l.max_hp);
    }

    #[test]
    fn no_trap_underfoot_does_nothing() {
        let mut g = setup();
        assert_eq!(d(&mut g).step_on_trap(Slot::Team(0)), None);
    }

    #[test]
    fn the_sticky_trap_picks_one_bag_item_with_one_draw() {
        let mut g = setup();
        d(&mut g).items.bag.slots[0] = Item::new(1, 1);
        d(&mut g).items.bag.slots[1] = Item {
            flags: crate::items::FLAG_EXISTS | FLAG_STICKY,
            qty: 1,
            id: 2,
        };
        d(&mut g).items.bag.slots[2] = Item::new(3, 1);
        lay(&mut g, id::STICKY, (5, 5), by::NATURAL, false);
        // Two candidates (slots 0 and 2).
        let mut model = d(&mut g).rng;
        let pick = model.int(2);
        let n = counted(&mut g, |dd| {
            dd.step_on_trap(Slot::Team(0));
        });
        assert_eq!(n, 1);
        let slots = d(&mut g).items.bag.slots;
        let target = if pick == 0 { 0 } else { 2 };
        assert!(slots[target].sticky());
        assert!(slots[1].sticky());
        assert_eq!(slots.iter().filter(|s| s.sticky()).count(), 2);
    }

    #[test]
    fn the_sticky_trap_with_no_candidate_draws_nothing() {
        let mut g = setup();
        d(&mut g).items.bag.slots = [Item::default(); crate::items::BAG_SLOTS];
        lay(&mut g, id::STICKY, (5, 5), by::NATURAL, false);
        let n = counted(&mut g, |dd| {
            dd.step_on_trap(Slot::Team(0));
        });
        assert_eq!(n, 0);
    }

    fn moves_of(d: &mut Dungeon, n: usize, pp: u8) {
        for (i, m) in d.team[0].info.moves.iter_mut().enumerate() {
            *m = if i < n {
                MoveSlot {
                    flags: MoveSlot::HOLDS_MOVE,
                    flags2: 0,
                    id: 1,
                    pp,
                    ginseng: 0,
                }
            } else {
                MoveSlot::default()
            };
        }
    }

    #[test]
    fn the_seal_trap_seals_one_more_slot_per_firing_with_one_draw() {
        let mut g = setup();
        moves_of(d(&mut g), 3, 5);
        lay(&mut g, id::SEAL, (5, 5), by::NATURAL, false);
        for expected in [1, 2, 3] {
            let n = counted(&mut g, |dd| {
                dd.step_on_trap(Slot::Team(0));
            });
            assert_eq!(n, 1);
            let sealed = d(&mut g).team[0]
                .info
                .moves
                .iter()
                .filter(|m| m.flags2 & MOVE_SEALED != 0)
                .count();
            assert_eq!(sealed, expected);
        }
        // All sealed: no candidate, no draw.
        let n = counted(&mut g, |dd| {
            dd.step_on_trap(Slot::Team(0));
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn the_seal_trap_picks_the_slot_the_roll_names() {
        let mut g = setup();
        moves_of(d(&mut g), 4, 5);
        d(&mut g).team[0].info.moves[0].flags2 = MOVE_SEALED;
        lay(&mut g, id::SEAL, (5, 5), by::NATURAL, false);
        let mut model = d(&mut g).rng;
        let pick = usize::try_from(model.int(3)).unwrap();
        d(&mut g).step_on_trap(Slot::Team(0));
        let sealed: Vec<usize> = (0..4)
            .filter(|&i| d(&mut g).team[0].info.moves[i].flags2 & MOVE_SEALED != 0)
            .collect();
        assert_eq!(sealed, [0, 1 + pick]);
    }

    #[test]
    fn the_pp_trap_empties_one_move_with_pp_and_skips_empty_ones() {
        let mut g = setup();
        moves_of(d(&mut g), 3, 5);
        d(&mut g).team[0].info.moves[0].pp = 0;
        lay(&mut g, id::PP, (5, 5), by::NATURAL, false);
        let mut model = d(&mut g).rng;
        let pick = usize::try_from(model.int(2)).unwrap();
        let n = counted(&mut g, |dd| {
            dd.step_on_trap(Slot::Team(0));
        });
        assert_eq!(n, 1);
        let pp: Vec<u8> = d(&mut g).team[0].info.moves.iter().map(|m| m.pp).collect();
        let mut want = [0, 5, 5, 0];
        want[1 + pick] = 0;
        assert_eq!(pp, want);
        // The other trap cases: no move with PP left draws nothing.
        d(&mut g).step_on_trap(Slot::Team(0));
        let n = counted(&mut g, |dd| {
            dd.step_on_trap(Slot::Team(0));
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn the_damage_and_spike_traps_use_their_own_numbers_and_draw_nothing() {
        for (trap, damage, spike) in [(id::DAMAGE, 9, 2), (id::SPIKE, 2, 9)] {
            let mut g = setup();
            d(&mut g).traps.tables.damage = damage;
            d(&mut g).traps.tables.spike_damage = spike;
            lay(&mut g, trap, (5, 5), by::NATURAL, false);
            let hp = d(&mut g).team[0].info.hp;
            let n = counted(&mut g, |dd| {
                dd.step_on_trap(Slot::Team(0));
            });
            assert_eq!(n, 0);
            assert_eq!(d(&mut g).team[0].info.hp, hp - 9);
        }
    }

    #[test]
    fn trap_damage_stops_at_0_hp() {
        let mut g = setup();
        d(&mut g).traps.tables.spike_damage = 500;
        lay(&mut g, id::SPIKE, (5, 5), by::NATURAL, false);
        d(&mut g).step_on_trap(Slot::Team(0));
        assert_eq!(d(&mut g).team[0].info.hp, 0);
    }

    #[test]
    fn an_undecoded_id_marks_the_trap_seen_and_changes_nothing_else() {
        for trap in [
            0, 1, 3, 4, 5, 6, 7, 8, 9, 0xA, 0xC, 0xD, 0xE, 0x11, 0x12, 0x14,
        ] {
            let mut g = setup();
            d(&mut g).traps.tables.damage = 9;
            lay(&mut g, trap, (5, 5), by::WILD, false);
            let before = d(&mut g).team[0].clone();
            let bag = d(&mut g).items.bag;
            let n = counted(&mut g, |dd| {
                assert_eq!(dd.step_on_trap(Slot::Team(0)), Some(trap));
            });
            assert_eq!(n, 0, "id {trap:#x}");
            assert_eq!(d(&mut g).team[0], before, "id {trap:#x}");
            assert_eq!(d(&mut g).items.bag, bag);
            assert!(d(&mut g).trap_at((5, 5)).unwrap().seen);
            assert_eq!(d(&mut g).traps().len(), 1, "no trap is consumed");
        }
    }

    #[test]
    fn a_leader_step_springs_the_trap_underfoot() {
        use crate::command::{Command, Direction};
        let mut g = setup();
        lay(&mut g, id::DAMAGE, (5, 6), by::NATURAL, false);
        d(&mut g).traps.tables.damage = 4;
        d(&mut g).wild.clear();
        let hp = d(&mut g).team[0].info.hp;
        g.step(Command::Move(Direction::Down));
        assert_eq!(d(&mut g).team[0].pos, (5, 6));
        assert_eq!(d(&mut g).team[0].info.hp, hp - 4);
        assert!(d(&mut g).trap_at((5, 6)).unwrap().seen);
    }
}
