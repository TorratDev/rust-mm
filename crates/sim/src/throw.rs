//! Throwing items: actions `0x0B`, `0x27` and `0x41` and the AI's `0x24` (`ov_0001::021732C4`,
//! `docs/re/items.md`, P3).
//!
//! The order of one throw: the item is picked and the refusals run, one is consumed, the held
//! items set the aim and the range (No-Aim Scope draws `DungeonRandInt(8)`), the path
//! (`02144F3C` line, `02144A54` arc) finds the targets, each target gets the hit test
//! (`0214498C`: one `DungeonRandInt(100)`, drawn even when a held item decides) for every
//! monster on the path first, then the damage of each hit target in path order (`021467F4`
//! through `CalcDamage`, or the fixed `021467BC`), and an item lands (`02143918`, no draw) when
//! it hit nothing, or at the end of a pierce line that stayed on the map. Integer math only.
//!
//! The numbers the notes do not embed (the hit chance, the arc damage, the species gate, the
//! landing ring) are [`ThrowParams`], given by the caller as the item tables are.
//!
//! Shortcut list (what the sim does not model):
//! - The path walks the line one tile at a time up to the range. The game's legs of 6 tiles and
//!   the wall-edge fixup of `02144F3C` are not read, so a diagonal passes a wall corner.
//! - Curve Band (`0x27`): `02144F3C` reads its flag (`param_5[1]`, lines ~69-87): without pierce,
//!   when the next tile is a wall it picks another direction from the table at `021A98D4`. The
//!   sim does not model it: the item stops at the wall.
//! - `0215A054(target, move)` runs after `InitPokemonMove` on both paths (`DAT_02145550` and
//!   `DAT_02144F24`); a non-zero result skips the target (no roll, no record, the line flies
//!   on). Not modelled: every monster on the path is a target.
//! - The arc's target tile comes from the caller (`0219BD04` is not read); without one it is the
//!   end of the line. An arc hits only the monster on that tile, and a target beyond the range
//!   is `Unmodelled`. The arc's sub-steps and height (`02017A60`) are cosmetic.
//! - The early misses of `0214498C` (`info+8 == 1`, `info+0x98` 1 or 4) are not modelled: the
//!   sim keeps neither byte. `0217ADBC(target, 8)` after a hit is not modelled either.
//! - Only team members hold items in the sim, so a wild thrower and a wild target's held item
//!   are not modelled.
//! - A thrown item that is not an ids 1 to 8 projectile (a seed, an orb) hits with no effect: the
//!   use-on-target path of `02146880` is not ported.
//! - A Bounce Band target (`0x26`): in `02144F3C` (a line, no pierce) it reverses the direction
//!   (`param_4 + 4 & 7`) and the item flies on; only in `02144A54` (the arc) does it skip the
//!   roll and land. The sim stops the item at the target on both paths: a miss with no roll.
//! - The monster-house wake-up (`0219037C`), the messages and the face step (`0217ADF8`) beyond
//!   the direction byte are not modelled.

use serde::{Deserialize, Serialize};

use crate::combat::{self, ARG8, CombatData, Rolls};
use crate::command::{Blocked, Direction};
use crate::floor::{TERRAIN_IMPASSABLE_WALL, TERRAIN_STAIRS, TERRAIN_TYPE_MASK};
use crate::items::{FLAG_MARK, Item, STACKABLE_BELOW};
use crate::state::{DIRECTIONS, Dungeon};
use crate::targeting::Slot;

/// The pseudo-move `021467F4` builds for ids 1 to 6 (the literal at `0214683C`, read as `0x195`
/// on floor 1). Its row (type, power, accuracy, critical odds) is read from the move table.
pub const THROW_MOVE: u16 = 0x195;

// Held-item ids (`docs/re/items.md`, P4; Red's names by numeric match).
const PIERCE_BAND: u8 = 0x0F;
const DODGE_SCARF: u8 = 0x25;
const BOUNCE_BAND: u8 = 0x26;
const CURVE_BAND: u8 = 0x27;
const WHIFF_SPECS: u8 = 0x28;
const NO_AIM_SCOPE: u8 = 0x29;
const LOCKON_SPECS: u8 = 0x2A;

/// The ids `021467F4` handles (line items, through the pseudo-move) and `021467BC` (arc items,
/// fixed damage).
const LINE_FIRST: u8 = 1;
const LINE_LAST: u8 = 6;
const ARC_FIRST: u8 = 7;
const ARC_LAST: u8 = 8;
/// Item category 1: the arc (`0219BD04` picks the tile).
const CATEGORY_ARC: u8 = 1;
/// The range when `info+0xC6` is 0, and with the Pierce Band or a status of 1 or 2.
const SHORT_RANGE: i32 = 10;
const LONG_RANGE: i32 = 99;

/// The numbers a throw reads from the ROM or the thrower's state, which `docs/re/items.md`
/// does not embed. The defaults are 0 and empty: a hit chance of 0 never hits (the roll is still
/// drawn), no species may throw, no ring tile takes a landing.
// shortcut: one set for the run; the game reads them from `021AA268`, the species table
// (`020DDF80`, `+0x33`), the ring at `02143BDC` and the thrower's `info+0xC6`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThrowParams {
    /// The s16 at `021AA268` (90 on floor 1): the roll `DungeonRandInt(100)` hits below it.
    pub hit_chance: i32,
    /// The damage of `021467BC` for ids 7 and 8 (the third argument of `02146880`, not traced).
    // hypothesis: that third argument is `0218F21C(thrower, target)` on both paths (export);
    // not read yet, so the sim takes one fixed value from the caller.
    pub arc_damage: i32,
    /// The thrower's `info+0xC6`: 0 gives range 10, 1 range 99, 2 range 99 and pierce. The sim
    /// keeps no such byte; a value of 3 or more is read as 0 (the game leaves the locals unset).
    pub range_status: u8,
    /// `021803B8` by species id: the byte at `+0x33` of the species entry is non-zero.
    pub can_throw: Vec<bool>,
    /// The landing offsets after the end tile (`02143BDC`, ends at the sentinel 99 in the game).
    pub landing_ring: Vec<(i8, i8)>,
}

impl ThrowParams {
    /// Empty parameters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hit_chance: 0,
            arc_damage: 0,
            range_status: 0,
            can_throw: Vec::new(),
            landing_ring: Vec::new(),
        }
    }

    /// `021803B8`: whether species `species` may throw. A species past the table cannot.
    #[must_use]
    pub fn species_can_throw(&self, species: u16) -> bool {
        self.can_throw
            .get(usize::from(species))
            .copied()
            .unwrap_or(false)
    }

    /// The eight neighbours in the direction table's order.
    // hypothesis: the ring at `02143BDC` is not read; this is a stand-in for tests and callers
    // with no table.
    #[must_use]
    pub fn neighbour_ring() -> Vec<(i8, i8)> {
        DIRECTIONS
            .iter()
            .map(|d| {
                let (dx, dy) = d.delta();
                (dx as i8, dy as i8)
            })
            .collect()
    }
}

/// Where the thrown item comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowSource {
    /// The leader's bag slot (actions `0x0B`, `0x27`, `0x41`): the leader's turn passes.
    Bag(u8),
    /// The held item of team member `member` (the AI's `0x24`, or the leader's selector `0x81`).
    /// The leader's turn passes only when `member` is 0.
    Held(usize),
}

/// What one throw did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThrowReport {
    /// The id of the thrown item.
    pub item: u8,
    /// How many targets the hit test passed.
    pub hits: u8,
    /// How many targets it did not (a Bounce Band counts).
    pub misses: u8,
    /// The HP the throw took off, summed.
    pub damage: i32,
    /// The tile the item landed on, if it hit nothing and a tile took it.
    pub landed: Option<(i16, i16)>,
    /// The thrower was the leader and its HP reached 0.
    pub leader_down: bool,
}

/// The result of a throw command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowResult {
    /// The command was refused and nothing changed (no item, a sticky item).
    Blocked(Blocked),
    /// The species gate `021803B8` is 0: the action is not offered.
    SpeciesGate,
    /// The run-away gate `02180B68` refused the thrower.
    RunsAway,
    /// A valid input the sim does not model; nothing changed.
    Unmodelled,
    /// The leader is asleep: its turn passed and nothing was thrown.
    Skipped,
    /// The item was thrown.
    Thrown(ThrowReport),
}

/// The thrower's `(pierce, range)` from `info+0xC6` and the Pierce Band (`021732C4`, step 4).
fn range_of(status: u8, pierce_band: bool) -> (bool, i32) {
    let (mut pierce, mut range) = match status {
        1 => (false, LONG_RANGE),
        2 => (true, LONG_RANGE),
        _ => (false, SHORT_RANGE),
    };
    if pierce_band {
        pierce = true;
        range = LONG_RANGE;
    }
    (pierce, range)
}

/// Whether the item lands (`02144F3C`, `iVar17`/`iVar18`): never after leaving the map; after a
/// pierce line always (hit or not); else only when nothing was hit.
fn lands(gone: bool, hits: u8, pierce_line: bool) -> bool {
    !gone && (hits == 0 || pierce_line)
}

impl Dungeon {
    /// Whether team member `member` holds `id` (`021432E8`: exists, not sticky, same id).
    fn has_held_item(&self, member: usize, id: u8) -> bool {
        self.items
            .held
            .get(member)
            .is_some_and(|h| h.exists() && !h.sticky() && h.id == id)
    }

    /// The held item of the monster at `slot`; only team members hold items in the sim.
    fn holds(&self, slot: Slot, id: u8) -> bool {
        match slot {
            Slot::Team(i) => self.has_held_item(i, id),
            Slot::Wild(_) => false,
        }
    }

    /// Throws `source` in `direction` (`021732C4`). `arc_target` is the tile of an arc item.
    /// Refusals come before any change and take no time; a throw takes the leader's turn when the
    /// thrower is the leader (see [`crate::turn`]). The caller's turn is the monster's own for an
    /// AI thrower.
    pub fn throw_item(
        &mut self,
        source: ThrowSource,
        direction: Direction,
        arc_target: Option<(i16, i16)>,
        params: &ThrowParams,
        data: &CombatData<'_>,
    ) -> ThrowResult {
        let thrower = match source {
            ThrowSource::Bag(_) => 0,
            ThrowSource::Held(member) => member,
        };
        let Some(member) = self.team.get(thrower) else {
            return ThrowResult::Blocked(Blocked::NoLeader);
        };
        let species = member.info.species;
        let asleep = member.info.is_asleep();
        let picked = match source {
            ThrowSource::Bag(slot) => self.items.bag.slots.get(usize::from(slot)).copied(),
            ThrowSource::Held(member) => self.items.held.get(member).copied(),
        }
        .filter(|i| i.exists());
        let Some(item) = picked else {
            return ThrowResult::Blocked(Blocked::NoItem);
        };
        // Sticky: refused for the held selector (0x81) or an item of category 0 or 1.
        let category = self.items.tables.category(item.id);
        if item.sticky() && (matches!(source, ThrowSource::Held(_)) || category <= CATEGORY_ARC) {
            return ThrowResult::Blocked(Blocked::ItemSticky);
        }
        if matches!(source, ThrowSource::Bag(_)) && !params.species_can_throw(species) {
            return ThrowResult::SpeciesGate;
        }
        if self.runs_away(thrower) {
            return ThrowResult::RunsAway;
        }
        let arc = category == CATEGORY_ARC;
        if (LINE_FIRST..=LINE_LAST).contains(&item.id)
            && data.moves.get(usize::from(THROW_MOVE)).is_none()
        {
            return ThrowResult::Unmodelled;
        }
        // The consumed held item is no longer held when the modifiers are read.
        let held_gone = matches!(source, ThrowSource::Held(_));
        let pierce_band = !held_gone && self.has_held_item(thrower, PIERCE_BAND);
        let (_, range) = range_of(params.range_status, pierce_band);
        if arc {
            let from = self.team[thrower].pos;
            if let Some(t) = arc_target
                && i32::from(t.0 - from.0)
                    .abs()
                    .max(i32::from(t.1 - from.1).abs())
                    > range
            {
                return ThrowResult::Unmodelled;
            }
        }
        if thrower == 0 && asleep {
            self.take_leader_turn(Some(data), |_| {});
            return ThrowResult::Skipped;
        }

        let mut report = ThrowReport::default();
        let run = |d: &mut Dungeon| {
            report = d.run_throw(source, thrower, item, direction, arc_target, params, data);
        };
        if thrower == 0 {
            self.take_leader_turn(Some(data), run);
        } else {
            let mut run = run;
            run(self);
        }
        ThrowResult::Thrown(report)
    }

    /// The body of a throw, after the refusals.
    #[allow(clippy::too_many_arguments)]
    fn run_throw(
        &mut self,
        source: ThrowSource,
        thrower: usize,
        item: Item,
        direction: Direction,
        arc_target: Option<(i16, i16)>,
        params: &ThrowParams,
        data: &CombatData<'_>,
    ) -> ThrowReport {
        let mut report = ThrowReport {
            item: item.id,
            ..ThrowReport::default()
        };
        // Step 2: one is consumed. A stackable item thrown from a stack leaves one behind.
        match source {
            ThrowSource::Bag(slot) => self.consume_bag_slot(slot),
            ThrowSource::Held(member) => {
                // A held stack of category 0 or 1 loses one (`pbVar2[1] - 1`).
                let stack = self.items.tables.category(item.id) <= CATEGORY_ARC && item.qty > 1;
                if let Some(h) = self.items.held.get_mut(member) {
                    if stack {
                        h.qty -= 1;
                    } else {
                        *h = Item::default();
                    }
                }
            }
        }
        let thrown = Item {
            flags: item.flags & !FLAG_MARK,
            qty: if item.id < STACKABLE_BELOW {
                1
            } else {
                item.qty
            },
            id: item.id,
        };

        // Step 3: the held items. The No-Aim Scope draws the facing; the Curve Band's flag is not
        // modelled (see the module's shortcut list).
        let mut facing = DIRECTIONS.iter().position(|&d| d == direction).unwrap_or(0);
        if self.has_held_item(thrower, NO_AIM_SCOPE) {
            facing = usize::try_from(self.rng.int(8)).unwrap_or(0);
        }
        let _curve = self.has_held_item(thrower, CURVE_BAND);
        // Step 4: the range switch, then the Pierce Band.
        let (pierce, range) = range_of(
            params.range_status,
            self.has_held_item(thrower, PIERCE_BAND),
        );

        // Step 5: the path and the monsters on it.
        let from = self.team[thrower].pos;
        let arc = self.items.tables.category(item.id) == CATEGORY_ARC;
        let (targets, end, gone) = if arc {
            self.arc_path(thrower, from, DIRECTIONS[facing], arc_target, range)
        } else {
            self.line_path(thrower, from, DIRECTIONS[facing], pierce, range)
        };

        // `02144F3C` draws every hit roll during the walk, then runs the damage of each hit
        // target in path order.
        let mut hit = Vec::new();
        for target in targets {
            if self.throw_hit(thrower, target, params) {
                hit.push(target);
            } else {
                report.misses += 1;
            }
        }
        let mut defeated = Vec::new();
        for target in hit {
            report.hits += 1;
            let (dealt, down) = self.throw_damage(thrower, target, item.id, params, data);
            report.damage += dealt;
            if down {
                defeated.push(target);
            }
        }
        report.leader_down = defeated.contains(&Slot::Team(0));
        if thrower == 0 {
            self.award_exp(&defeated, data);
        }
        self.remove_defeated(&defeated);

        // Step 6: the item lands when it hit nothing; a pierce line always ends with a landing,
        // unless it left the map (the item is gone).
        if lands(gone, report.hits, pierce && !arc) {
            report.landed = self.land(end, thrown, params);
        }
        if let Some(m) = self.team.get_mut(thrower) {
            m.info.direction = u8::try_from(facing).unwrap_or(0);
        }
        report
    }

    /// The monsters on the tile `pos` other than `thrower`.
    fn monster_at(&self, thrower: usize, pos: (i16, i16)) -> Option<Slot> {
        let team = self
            .team
            .iter()
            .enumerate()
            .find(|(i, m)| *i != thrower && m.pos == pos && m.info.hp > 0)
            .map(|(i, _)| Slot::Team(i));
        team.or_else(|| {
            self.wild
                .iter()
                .position(|m| m.pos == pos && m.info.hp > 0)
                .map(Slot::Wild)
        })
    }

    /// Whether a projectile cannot enter the tile: a wall, a tile of terrain type 0, or off the
    /// floor.
    fn blocks_projectile(&self, x: i32, y: i32) -> bool {
        self.floor.tile(x, y).is_none_or(|t| {
            t.terrain & TERRAIN_IMPASSABLE_WALL != 0 || t.terrain & TERRAIN_TYPE_MASK == 0
        })
    }

    /// `02144F3C`: the monsters on the line in path order, the tile the projectile ends on, and
    /// whether it left the map (x < 0, y < 0, x > 0x37 or y > 0x1F: the item is gone). Without
    /// `pierce` it stops before a wall and at the first monster; a piercing item flies through
    /// walls. Both stop after `range` tiles.
    fn line_path(
        &self,
        thrower: usize,
        from: (i16, i16),
        direction: Direction,
        pierce: bool,
        range: i32,
    ) -> (Vec<Slot>, (i16, i16), bool) {
        let (dx, dy) = direction.delta();
        let mut targets = Vec::new();
        let mut end = from;
        for n in 1..=range {
            let x = i32::from(from.0) + dx * n;
            let y = i32::from(from.1) + dy * n;
            if !(0..=0x37).contains(&x) || !(0..=0x1F).contains(&y) {
                return (targets, end, pierce);
            }
            if !pierce && self.blocks_projectile(x, y) {
                break;
            }
            let pos = (x as i16, y as i16);
            end = pos;
            if let Some(slot) = self.monster_at(thrower, pos) {
                targets.push(slot);
                if !pierce {
                    break;
                }
            }
        }
        (targets, end, false)
    }

    /// `02144A54`: the arc lands on `target` (the end of the line when the caller gives none) and
    /// hits the monster there.
    fn arc_path(
        &self,
        thrower: usize,
        from: (i16, i16),
        direction: Direction,
        target: Option<(i16, i16)>,
        range: i32,
    ) -> (Vec<Slot>, (i16, i16), bool) {
        let tile = match target {
            Some(t) => t,
            None => return self.line_path(thrower, from, direction, false, range),
        };
        (
            self.monster_at(thrower, tile).into_iter().collect(),
            tile,
            false,
        )
    }

    /// `0214498C`: whether the throw hits `target`. A Bounce Band target skips the roll. Else one
    /// `DungeonRandInt(100)` is drawn, before the overrides: Whiff Specs on the thrower always
    /// miss, else Lockon Specs always hit, else a Dodge Scarf target always misses.
    // hypothesis: the overrides apply in that order, as the notes list them.
    fn throw_hit(&mut self, thrower: usize, target: Slot, params: &ThrowParams) -> bool {
        if self.holds(target, BOUNCE_BAND) {
            return false;
        }
        let roll = self.rng.int(100);
        if self.has_held_item(thrower, WHIFF_SPECS) {
            return false;
        }
        if self.has_held_item(thrower, LOCKON_SPECS) {
            return true;
        }
        if self.holds(target, DODGE_SCARF) {
            return false;
        }
        roll < params.hit_chance
    }

    /// `02146880(isThrown = 1)` on a hit target: the damage handler of the item id. Returns the
    /// HP taken and whether the target reached 0 HP.
    fn throw_damage(
        &mut self,
        thrower: usize,
        target: Slot,
        id: u8,
        params: &ThrowParams,
        data: &CombatData<'_>,
    ) -> (i32, bool) {
        match id {
            LINE_FIRST..=LINE_LAST => self.pseudo_move_damage(thrower, target, data),
            ARC_FIRST..=ARC_LAST => self.fixed_damage(target, params.arc_damage),
            _ => (0, false),
        }
    }

    /// `021467F4`: the pseudo-move through `CalcDamage`: the three draws of the damage, then the
    /// second hit check of `0215652C` (the move row's second accuracy).
    fn pseudo_move_damage(
        &mut self,
        thrower: usize,
        target: Slot,
        data: &CombatData<'_>,
    ) -> (i32, bool) {
        let Some(mv) = data.moves.get(usize::from(THROW_MOVE)).cloned() else {
            return (0, false);
        };
        let (Some(attacker), Some(defender)) = (
            self.info_of(Slot::Team(thrower)).cloned(),
            self.info_of(target).cloned(),
        ) else {
            return (0, false);
        };
        let t = data.tables;
        let rolls = Rolls {
            boost: self.rng.int(100),
            crit: if combat::crit_drawn(&defender) {
                self.rng.int(100)
            } else {
                0
            },
            variance: self.rng.int(0x4000),
        };
        let dealt = combat::damage_scaled(t, &mv, ARG8, &attacker, &defender, rolls);
        let second = self.rng.int(100);
        let physical = combat::is_physical(t, mv.kind);
        if !combat::hits(t, second, mv.accuracy[1], &attacker, &defender, physical) {
            return (0, false);
        }
        self.take_hp(target, dealt.value)
    }

    /// `021467BC`: a fixed damage, no draw.
    // shortcut: `HandleDealingDamage_Async` (`0217BB94`) is reduced to the HP loss.
    fn fixed_damage(&mut self, target: Slot, dmg: i32) -> (i32, bool) {
        self.take_hp(target, dmg)
    }

    fn take_hp(&mut self, target: Slot, dmg: i32) -> (i32, bool) {
        let Some(info) = self.info_mut(target) else {
            return (0, false);
        };
        let taken = dmg.clamp(0, i32::from(info.hp));
        info.hp -= i16::try_from(taken).unwrap_or(0);
        (taken, info.hp == 0)
    }

    /// `02143918`: the item goes to the end tile when it takes one, else to the first ring tile
    /// that does. A tile takes it when its terrain type is not 0, it is not the stairs, and it
    /// holds no item. No draw. With no tile, or no free floor slot, the item is gone.
    // shortcut: the trap branch (`02142ADC == 2` consumes the item) is not modelled: the sim has
    // no traps.
    fn land(&mut self, end: (i16, i16), item: Item, params: &ThrowParams) -> Option<(i16, i16)> {
        let ring = params
            .landing_ring
            .iter()
            .map(|&(dx, dy)| (end.0 + i16::from(dx), end.1 + i16::from(dy)));
        let tile = std::iter::once(end).chain(ring).find(|&pos| {
            self.floor
                .tile(i32::from(pos.0), i32::from(pos.1))
                .is_some_and(|t| {
                    t.terrain & TERRAIN_TYPE_MASK != 0 && t.terrain & TERRAIN_STAIRS == 0
                })
                && self.items.slot_at(pos).is_none()
        })?;
        self.items.spawn(tile, item).map(|_| tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::tests::{battle_with, move_of, tables};
    use crate::floor::TERRAIN_FLOOR;
    use crate::rng::DungeonRng;
    use crate::state::{GameState, Monster};

    const ROCK: u8 = 7;

    /// The leader at (5, 5) facing right with a bag of three Iron Thorns (id 1), an open row of
    /// floor from x = 1 to 40 on y = 5, and no wild monster.
    fn setup() -> GameState {
        let mut g = battle_with(&move_of(0x9A, 40, 1, [100; 3], 0));
        let d = g.dungeon_mut().unwrap();
        d.wild.clear();
        for x in 1..=40 {
            d.floor.tile_mut(x, 5).unwrap().terrain = TERRAIN_FLOOR;
        }
        d.items.bag.slots[0] = Item::new(1, 3);
        d.items.tables.categories = vec![0; 16];
        d.items.tables.categories[usize::from(ROCK)] = CATEGORY_ARC;
        g
    }

    fn foe(g: &mut GameState, x: i16) {
        let d = g.dungeon_mut().unwrap();
        let mut m: Monster = d.team[0].clone();
        m.pos = (x, 5);
        m.info.is_team_leader = false;
        m.info.not_team_member = true;
        d.wild.push(m);
    }

    fn foe_at(g: &mut GameState, pos: (i16, i16)) {
        foe(g, 0);
        g.dungeon_mut().unwrap().wild.last_mut().unwrap().pos = pos;
    }

    fn params(hit_chance: i32) -> ThrowParams {
        ThrowParams {
            hit_chance,
            arc_damage: 7,
            can_throw: vec![true, true],
            landing_ring: ThrowParams::neighbour_ring(),
            ..ThrowParams::new()
        }
    }

    fn moves() -> Vec<rescue_data::moves::Move> {
        let mut table: Vec<_> = (0..=usize::from(THROW_MOVE))
            .map(|id| move_of(id as u16, 0, 0, [0; 3], 0))
            .collect();
        table[usize::from(THROW_MOVE)] = move_of(THROW_MOVE, 2, 0, [100; 3], 0);
        table
    }

    /// Runs `f` with the combat data of the tests.
    fn with_data<R>(f: impl FnOnce(&CombatData<'_>) -> R) -> R {
        let t = tables();
        let m = moves();
        f(&CombatData {
            tables: &t,
            moves: &m,
            exp_yields: &[],
            weights: &[],
            levels: &[],
        })
    }

    /// The throw body on the leader's bag slot 0, without the turn, so the draws are the throw's.
    fn run(g: &mut GameState, p: &ThrowParams) -> ThrowReport {
        let d = g.dungeon_mut().unwrap();
        let item = d.items.bag.slots[0];
        with_data(|data| {
            d.run_throw(
                ThrowSource::Bag(0),
                0,
                item,
                Direction::Right,
                None,
                p,
                data,
            )
        })
    }

    fn rng_of(g: &GameState) -> DungeonRng {
        *g.dungeon().unwrap().rng()
    }

    /// `rng` after one `int` draw per cap.
    fn after(mut rng: DungeonRng, caps: &[i32]) -> DungeonRng {
        for &c in caps {
            rng.int(c);
        }
        rng
    }

    fn wild_hp(g: &GameState) -> i16 {
        g.dungeon().unwrap().wild[0].info.hp
    }

    #[test]
    fn a_line_throw_that_hits_draws_the_roll_then_the_four_damage_draws() {
        let mut g = setup();
        foe(&mut g, 7);
        let start = rng_of(&g);
        let mut r = start;
        let roll = r.int(100);
        let rolls = Rolls {
            boost: r.int(100),
            crit: r.int(100),
            variance: r.int(0x4000),
        };
        let (attacker, defender) = {
            let d = g.dungeon().unwrap();
            (d.team[0].info.clone(), d.wild[0].info.clone())
        };
        let t = tables();
        let mv = moves()[usize::from(THROW_MOVE)].clone();
        let want = combat::damage_scaled(&t, &mv, ARG8, &attacker, &defender, rolls).value;
        let report = run(&mut g, &params(roll + 1));
        assert_eq!((report.hits, report.misses), (1, 0));
        assert!(want > 0);
        assert_eq!(report.damage, want);
        assert_eq!(wild_hp(&g), 100 - want as i16);
        assert_eq!(report.landed, None);
        // 100 (hit), 100, 100, 16384 (CalcDamage), 100 (second check).
        assert_eq!(rng_of(&g), after(start, &[100, 100, 100, 0x4000, 100]));
        // One consumed from the stack of three.
        assert_eq!(g.dungeon().unwrap().bag().slots[0], Item::new(1, 2));
    }

    #[test]
    fn a_missed_line_throw_draws_one_value_and_the_item_lands_on_the_monsters_tile() {
        let mut g = setup();
        foe(&mut g, 7);
        let start = rng_of(&g);
        let roll = { start }.int(100);
        let report = run(&mut g, &params(roll));
        assert_eq!((report.hits, report.misses), (0, 1));
        assert_eq!(wild_hp(&g), 100);
        assert_eq!(rng_of(&g), after(start, &[100]));
        assert_eq!(report.landed, Some((7, 5)));
        let d = g.dungeon().unwrap();
        assert_eq!(d.floor_item_at((7, 5)), Some(Item::new(1, 1)));
    }

    #[test]
    fn a_target_past_the_range_is_not_reached_and_a_longer_range_reaches_it() {
        let mut g = setup();
        foe(&mut g, 17);
        let start = rng_of(&g);
        let report = run(&mut g, &params(100));
        assert_eq!((report.hits, report.misses), (0, 0));
        assert_eq!(rng_of(&g), start);
        // Range 10 from x = 5 ends on x = 15.
        assert_eq!(report.landed, Some((15, 5)));

        let mut g = setup();
        foe(&mut g, 17);
        let mut p = params(100);
        p.range_status = 1;
        let report = run(&mut g, &p);
        assert_eq!(report.hits, 1);
    }

    #[test]
    fn the_item_lands_beside_a_blocked_end_tile() {
        let mut g = setup();
        {
            let d = g.dungeon_mut().unwrap();
            // The row ends in a wall at x = 9, so the end tile is 8, which holds an item.
            d.floor.tile_mut(9, 5).unwrap().terrain = 0;
            d.items.spawn((8, 5), Item::new(0x77, 1));
        }
        let start = rng_of(&g);
        let report = run(&mut g, &params(90));
        assert_eq!(rng_of(&g), start, "landing draws nothing");
        // The ring's first free tile in table order: (0, 1) and its row are walls, so 7 on the row.
        assert_eq!(report.landed, Some((7, 5)));
        assert_eq!(
            g.dungeon().unwrap().floor_item_at((8, 5)),
            Some(Item::new(0x77, 1))
        );
        // With no ring the item is gone.
        let mut g = setup();
        {
            let d = g.dungeon_mut().unwrap();
            d.floor.tile_mut(9, 5).unwrap().terrain = 0;
            d.items.spawn((8, 5), Item::new(0x77, 1));
        }
        let mut p = params(90);
        p.landing_ring.clear();
        assert_eq!(run(&mut g, &p).landed, None);
    }

    #[test]
    fn the_stairs_do_not_take_a_landing() {
        let mut g = setup();
        {
            let d = g.dungeon_mut().unwrap();
            d.floor.tile_mut(9, 5).unwrap().terrain = 0;
            d.floor.tile_mut(8, 5).unwrap().terrain = TERRAIN_FLOOR | TERRAIN_STAIRS;
        }
        assert_eq!(run(&mut g, &params(90)).landed, Some((7, 5)));
    }

    #[test]
    fn held_items_decide_the_hit_after_the_roll_is_drawn() {
        let cases = [
            // (holder is the thrower, item, hit chance, hits)
            (true, WHIFF_SPECS, 100, false),
            (true, LOCKON_SPECS, 0, true),
        ];
        for (_, id, chance, hit) in cases {
            let mut g = setup();
            foe(&mut g, 7);
            g.dungeon_mut().unwrap().items.held[0] = Item::new(id, 1);
            let start = rng_of(&g);
            let report = run(&mut g, &params(chance));
            assert_eq!(report.hits == 1, hit, "item {id:#x}");
            let draws: &[i32] = if hit {
                &[100, 100, 100, 0x4000, 100]
            } else {
                &[100]
            };
            assert_eq!(rng_of(&g), after(start, draws), "item {id:#x}");
        }
        // A Dodge Scarf target always misses, still after the roll.
        let mut g = setup();
        foe(&mut g, 7);
        let d = g.dungeon_mut().unwrap();
        d.team.push(d.team[0].clone());
        d.team[1].pos = (8, 5);
        d.items.held[1] = Item::new(DODGE_SCARF, 1);
        d.wild.clear();
        let start = *d.rng();
        // The ally stands at 8; make it the first monster by moving it to 7.
        d.team[1].pos = (7, 5);
        let report = run(&mut g, &params(100));
        assert_eq!((report.hits, report.misses), (0, 1));
        assert_eq!(rng_of(&g), after(start, &[100]));
    }

    #[test]
    fn a_bounce_band_target_skips_the_roll() {
        let mut g = setup();
        let d = g.dungeon_mut().unwrap();
        d.team.push(d.team[0].clone());
        d.team[1].pos = (7, 5);
        d.items.held[1] = Item::new(BOUNCE_BAND, 1);
        let start = *d.rng();
        let report = run(&mut g, &params(100));
        assert_eq!((report.hits, report.misses), (0, 1));
        assert_eq!(rng_of(&g), start);
    }

    #[test]
    fn the_no_aim_scope_draws_the_facing_first() {
        let mut g = setup();
        foe(&mut g, 7);
        g.dungeon_mut().unwrap().items.held[0] = Item::new(NO_AIM_SCOPE, 1);
        let start = rng_of(&g);
        let facing = { start }.int(8);
        let report = run(&mut g, &params(100));
        let d = g.dungeon().unwrap();
        assert_eq!(
            usize::from(d.team[0].info.direction),
            usize::try_from(facing).unwrap()
        );
        // 8 for the facing, then the throw's own draws if the monster was in line, else none.
        let hit_line = facing == 2;
        let draws: Vec<i32> = if hit_line {
            vec![8, 100, 100, 100, 0x4000, 100]
        } else {
            vec![8]
        };
        assert_eq!(rng_of(&g), after(start, &draws));
        assert_eq!(report.hits == 1, hit_line);
    }

    #[test]
    fn a_pierce_band_hits_every_monster_on_the_line_in_order() {
        for via_status in [false, true] {
            let mut g = setup();
            foe(&mut g, 7);
            foe(&mut g, 9);
            let mut p = params(100);
            if via_status {
                p.range_status = 2;
            } else {
                g.dungeon_mut().unwrap().items.held[0] = Item::new(PIERCE_BAND, 1);
            }
            let start = rng_of(&g);
            let report = run(&mut g, &p);
            assert_eq!(report.hits, 2);
            assert_eq!(
                rng_of(&g),
                after(
                    start,
                    &[100, 100, 100, 100, 0x4000, 100, 100, 100, 0x4000, 100]
                )
            );
        }
        // Without it the first monster stops the item.
        let mut g = setup();
        foe(&mut g, 7);
        foe(&mut g, 9);
        assert_eq!(run(&mut g, &params(100)).hits, 1);
        assert_eq!(g.dungeon().unwrap().wild[1].info.hp, 100);
    }

    #[test]
    fn a_pierce_throw_that_hits_still_lands_at_the_path_end() {
        let mut g = setup();
        foe(&mut g, 7);
        let p = params(100);
        g.dungeon_mut().unwrap().items.held[0] = Item::new(PIERCE_BAND, 1);
        let report = run(&mut g, &p);
        assert_eq!(report.hits, 1);
        assert_eq!(report.landed, None, "left the map at x = 0x38");

        // A pierce throw down a column that stays on the map lands at the range end after a hit.
        let mut g = setup();
        {
            let d = g.dungeon_mut().unwrap();
            for y in 5..=30 {
                d.floor.tile_mut(5, y).unwrap().terrain = TERRAIN_FLOOR;
            }
            d.items.held[0] = Item::new(PIERCE_BAND, 1);
        }
        foe_at(&mut g, (5, 8));
        let d = g.dungeon_mut().unwrap();
        let item = d.items.bag.slots[0];
        let p = params(100);
        let report = with_data(|data| {
            d.run_throw(
                ThrowSource::Bag(0),
                0,
                item,
                Direction::Down,
                None,
                &p,
                data,
            )
        });
        assert_eq!(report.hits, 1);
        assert_eq!(report.landed, None, "left the map at y = 0x20");
    }

    #[test]
    fn a_pierce_line_lands_after_a_hit_unless_it_left_the_map() {
        assert!(lands(false, 1, true), "pierce hit, stayed on the map");
        assert!(!lands(true, 1, true), "left the map");
        assert!(!lands(true, 0, true));
        assert!(lands(false, 0, false), "nothing hit");
        assert!(!lands(false, 1, false), "a single target hit");
    }

    #[test]
    fn a_pierce_throw_flies_through_a_wall() {
        let mut g = setup();
        g.dungeon_mut()
            .unwrap()
            .floor
            .tile_mut(8, 5)
            .unwrap()
            .terrain = 0;
        foe(&mut g, 12);
        let mut p = params(100);
        p.range_status = 2;
        assert_eq!(run(&mut g, &p).hits, 1, "the wall at 8 does not stop it");
        // Without pierce the wall stops it and the item lands before the wall.
        let mut g = setup();
        g.dungeon_mut()
            .unwrap()
            .floor
            .tile_mut(8, 5)
            .unwrap()
            .terrain = 0;
        foe(&mut g, 12);
        let report = run(&mut g, &params(100));
        assert_eq!((report.hits, report.landed), (0, Some((7, 5))));
    }

    #[test]
    fn a_held_stack_loses_one_and_a_single_item_clears_the_slot() {
        let mut g = setup();
        let p = params(100);
        g.dungeon_mut().unwrap().items.held[0] = Item::new(1, 3);
        let thrown = with_data(|data| {
            g.dungeon_mut().unwrap().throw_item(
                ThrowSource::Held(0),
                Direction::Right,
                None,
                &p,
                data,
            )
        });
        assert!(matches!(thrown, ThrowResult::Thrown(_)));
        assert_eq!(g.dungeon().unwrap().items.held[0], Item::new(1, 2));
        g.dungeon_mut().unwrap().items.held[0] = Item::new(1, 1);
        with_data(|data| {
            g.dungeon_mut().unwrap().throw_item(
                ThrowSource::Held(0),
                Direction::Right,
                None,
                &p,
                data,
            )
        });
        assert_eq!(g.dungeon().unwrap().items.held[0], Item::default());
    }

    #[test]
    fn a_sticky_item_is_refused_by_selector_or_category_only() {
        let p = params(100);
        let go = |g: &mut GameState, source| {
            with_data(|data| {
                g.dungeon_mut()
                    .unwrap()
                    .throw_item(source, Direction::Right, None, &p, data)
            })
        };
        // A sticky bag item of category 5 is thrown.
        let mut g = setup();
        {
            let d = g.dungeon_mut().unwrap();
            d.items.tables.categories[1] = 5;
            d.items.bag.slots[0].flags |= crate::items::FLAG_STICKY;
        }
        assert!(matches!(
            go(&mut g, ThrowSource::Bag(0)),
            ThrowResult::Thrown(_)
        ));
        // The same item held (selector 0x81) is refused.
        let mut g = setup();
        {
            let d = g.dungeon_mut().unwrap();
            d.items.tables.categories[1] = 5;
            d.items.held[0] = Item::new(1, 1);
            d.items.held[0].flags |= crate::items::FLAG_STICKY;
        }
        assert_eq!(
            go(&mut g, ThrowSource::Held(0)),
            ThrowResult::Blocked(Blocked::ItemSticky)
        );
    }

    #[test]
    fn an_arc_item_hits_the_monster_on_its_tile_for_the_fixed_damage() {
        let mut g = setup();
        foe(&mut g, 8);
        {
            let d = g.dungeon_mut().unwrap();
            d.items.bag.slots[0] = Item::new(ROCK, 1);
        }
        let start = rng_of(&g);
        let d = g.dungeon_mut().unwrap();
        let item = d.items.bag.slots[0];
        let p = params(100);
        let report = with_data(|data| {
            d.run_throw(
                ThrowSource::Bag(0),
                0,
                item,
                Direction::Right,
                Some((8, 5)),
                &p,
                data,
            )
        });
        assert_eq!((report.hits, report.damage), (1, 7));
        assert_eq!(wild_hp(&g), 93);
        // The hit roll only: the fixed damage draws nothing.
        assert_eq!(rng_of(&g), after(start, &[100]));
        // No monster on the tile: no roll, and the item lands there.
        let mut g = setup();
        g.dungeon_mut().unwrap().items.bag.slots[0] = Item::new(ROCK, 1);
        let start = rng_of(&g);
        let d = g.dungeon_mut().unwrap();
        let item = d.items.bag.slots[0];
        let report = with_data(|data| {
            d.run_throw(
                ThrowSource::Bag(0),
                0,
                item,
                Direction::Right,
                Some((8, 5)),
                &p,
                data,
            )
        });
        assert_eq!(report.landed, Some((8, 5)));
        assert_eq!(rng_of(&g), start);
    }

    #[test]
    fn the_range_switch_and_the_pierce_band() {
        assert_eq!(range_of(0, false), (false, 10));
        assert_eq!(range_of(1, false), (false, 99));
        assert_eq!(range_of(2, false), (true, 99));
        assert_eq!(range_of(3, false), (false, 10));
        assert_eq!(range_of(0, true), (true, 99));
    }

    #[test]
    fn the_species_gate_refuses_a_bag_throw_and_not_a_held_one() {
        let mut g = setup();
        let mut p = params(100);
        p.can_throw.clear();
        let before = g.dungeon().unwrap().clone();
        let result = with_data(|data| {
            g.dungeon_mut().unwrap().throw_item(
                ThrowSource::Bag(0),
                Direction::Right,
                None,
                &p,
                data,
            )
        });
        assert_eq!(result, ThrowResult::SpeciesGate);
        assert_eq!(g.dungeon().unwrap().bag(), before.bag());
        assert!(params(1).species_can_throw(1));
        assert!(!params(1).species_can_throw(9));
    }

    #[test]
    fn a_refused_throw_changes_nothing() {
        let mut g = setup();
        let p = params(100);
        let go = |g: &mut GameState, source| {
            with_data(|data| {
                g.dungeon_mut()
                    .unwrap()
                    .throw_item(source, Direction::Right, None, &p, data)
            })
        };
        assert_eq!(
            go(&mut g, ThrowSource::Bag(5)),
            ThrowResult::Blocked(Blocked::NoItem)
        );
        g.dungeon_mut().unwrap().items.bag.slots[0].flags |= crate::items::FLAG_STICKY;
        let (turn, rng) = (g.dungeon().unwrap().turn, rng_of(&g));
        assert_eq!(
            go(&mut g, ThrowSource::Bag(0)),
            ThrowResult::Blocked(Blocked::ItemSticky)
        );
        assert_eq!((g.dungeon().unwrap().turn, rng_of(&g)), (turn, rng));
    }

    #[test]
    fn a_leader_throw_takes_the_turn_and_one_item() {
        let mut g = setup();
        let turn = g.dungeon().unwrap().turn;
        let p = params(100);
        let result = with_data(|data| {
            g.dungeon_mut().unwrap().throw_item(
                ThrowSource::Bag(0),
                Direction::Right,
                None,
                &p,
                data,
            )
        });
        let ThrowResult::Thrown(report) = result else {
            panic!("{result:?}");
        };
        assert_eq!(report.landed, Some((15, 5)));
        let d = g.dungeon().unwrap();
        assert_eq!(d.turn, turn + 1);
        assert_eq!(d.bag().slots[0], Item::new(1, 2));
    }

    #[test]
    fn a_held_item_throw_clears_the_slot_and_takes_no_leader_turn_for_an_ally() {
        let mut g = setup();
        let d = g.dungeon_mut().unwrap();
        let mut ally = d.team[0].clone();
        ally.pos = (5, 6);
        ally.info.is_team_leader = false;
        d.team.push(ally);
        d.floor.tile_mut(6, 6).unwrap().terrain = TERRAIN_FLOOR;
        d.items.held[1] = Item::new(1, 1);
        let turn = d.turn;
        let p = params(100);
        let result =
            with_data(|data| d.throw_item(ThrowSource::Held(1), Direction::Right, None, &p, data));
        assert!(matches!(result, ThrowResult::Thrown(_)));
        assert_eq!(d.items.held[1], Item::default());
        assert_eq!(d.turn, turn);
    }
}
