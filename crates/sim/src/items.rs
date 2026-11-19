//! Items: the bag, the held slots, floor items, pickup, drop, swap and money (`docs/re/items.md`,
//! P1, P2 "#459" for the picker, P7 for the layout).
//!
//! The bag is the team's 20 three-byte slots (`*0x020DECF8`, P7); a held item sits in each team
//! member's info record (`info+0x5A`). Floor items are 64 slots with `numItems` as a high-water
//! mark (`gDungeon+0x3364`, `+0x3424`). Money never takes a bag slot. Integer math only.

use serde::{Deserialize, Serialize};

use crate::command::Blocked;
use crate::floor::{self, Floor};
use crate::rng::DungeonRng;
use crate::state::{Dungeon, TEAM_SLOTS};

/// Bag slots (`0x14`, the loop bound in `021441D8`, `02142B70`, `02007884`, `02007B68`).
pub const BAG_SLOTS: usize = 20;
/// Floor item slots (the 64 entity pointers at `gDungeon+0x113B4`).
pub const FLOOR_ITEM_SLOTS: usize = 64;
/// The stack cap: a sum above it clamps to 99.
pub const STACK_CAP: u8 = 99;
/// The team money clamp (`AddToTeamMoney`, the literal at `020076C8`).
pub const MONEY_CAP: i32 = 99_999;
/// The Poké item id, category 6: it is money, never a bag entry.
pub const POKE: u8 = 0x69;
/// Only item ids below this stack (the thrown stacks, ids 1 to 8).
pub const STACKABLE_BELOW: u8 = 9;

/// Item flag bit 0: the slot holds an item.
pub const FLAG_EXISTS: u8 = 0x01;
/// Item flag bit 1: shop stock (set by the shop pass, which the sim does not run yet).
pub const FLAG_SHOP: u8 = 0x02;
/// Item flag bit 3: sticky.
pub const FLAG_STICKY: u8 = 0x08;
/// Item flag bit 4: the "L+R throw" marker (a held item never carries it).
pub const FLAG_MARK: u8 = 0x10;

/// Category weights per picker row (`02138B74`: 12 category weights).
pub const PICKER_CATEGORIES: usize = 12;
/// Item weights per picker row (240 of them).
pub const PICKER_ITEMS: usize = 240;
/// The picker's roll bound (`DungeonRandInt(0x2710)`).
const PICKER_ROLL: i32 = 0x2710;

/// One item, the game's three bytes: flags (`+0`), quantity (`+1`), id (`+2`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub flags: u8,
    pub qty: u8,
    pub id: u8,
}

impl Item {
    /// A plain item of `id` and `qty`.
    #[must_use]
    pub const fn new(id: u8, qty: u8) -> Self {
        Self {
            flags: FLAG_EXISTS,
            qty,
            id,
        }
    }

    /// Whether the slot holds an item (flag bit 0).
    #[must_use]
    pub const fn exists(self) -> bool {
        self.flags & FLAG_EXISTS != 0
    }

    /// Whether the item is sticky (flag bit 3).
    #[must_use]
    pub const fn sticky(self) -> bool {
        self.flags & FLAG_STICKY != 0
    }
}

/// The team bag and money (`+0x000` and `+0x234` of the team inventory).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bag {
    pub slots: [Item; BAG_SLOTS],
    /// The team's money, 0 to [`MONEY_CAP`].
    pub money: i32,
}

impl Bag {
    /// `02007884`: puts `item` in the first free slot. `false` when the bag is full.
    pub fn add(&mut self, item: Item) -> bool {
        match self.slots.iter_mut().find(|s| !s.exists()) {
            Some(slot) => {
                *slot = item;
                true
            }
            None => false,
        }
    }

    /// `02007B68`: moves the used slots to the front, keeping their order.
    pub fn compact(&mut self) {
        let mut used = self.slots.iter().copied().filter(|s| s.exists());
        let mut out = [Item::default(); BAG_SLOTS];
        for slot in &mut out {
            match used.next() {
                Some(item) => *slot = item,
                None => break,
            }
        }
        self.slots = out;
    }

    /// `AddToTeamMoney`: adds `amount` and clamps to 0 to [`MONEY_CAP`].
    pub fn add_money(&mut self, amount: i32) {
        self.money = self.money.saturating_add(amount).clamp(0, MONEY_CAP);
    }
}

/// One row of the floor-item picker (`gDungeon+0x1A312 + row * 0x1F8`): 12 category weights,
/// then 240 item weights. Both are cumulative on the `DungeonRandInt(0x2710)` roll: the
/// category weights over the categories, the item weights within an item's category.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickerRow {
    pub categories: Vec<u16>,
    pub items: Vec<u16>,
}

/// The item tables and setup flags a run needs. The game reads them from the ROM and the
/// story state; the sim takes them from the caller (see `GameState::with_item_tables`).
// shortcut: one set of tables for the whole run. The game loads the picker rows per floor
// (from the floor's item list) and the sticky chance from the floor setup (`gDungeon+0x1A300`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTables {
    /// The picker rows (row 0 ordinary tiles, 1 shop, 2 monster house, 3 buried). A missing
    /// row picks nothing, so the fallback Poké (the roll is still drawn).
    pub rows: Vec<PickerRow>,
    /// Item category by item id (item table `+0xC`); an id past the end reads 0.
    pub categories: Vec<u8>,
    /// Stack size of a thrown item (ids 1 to 8) by id: the item record's spawn amount `+0x1B`.
    pub stack_amount: Vec<u8>,
    /// The percent that a floor item is sticky (`gDungeon+0x1A300`).
    pub sticky_chance: u8,
    /// The money value by quantity (the 100-entry table at `02091FA8`). With none, the money
    /// of a Poké item is its quantity.
    pub money_values: Vec<u32>,
    /// The thresholds of the quantity roll of a Poké floor item (`02143290`).
    pub money_roll: Vec<u8>,
    /// `gDungeon+0x64C`: the player has a bag in this dungeon (`docs/re/items.md`, #464).
    pub has_inventory: bool,
}

impl Default for ItemTables {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemTables {
    /// Empty tables: every pick falls back to Poké, and the player has a bag.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: Vec::new(),
            categories: Vec::new(),
            stack_amount: Vec::new(),
            sticky_chance: 0,
            money_values: Vec::new(),
            money_roll: Vec::new(),
            has_inventory: true,
        }
    }

    /// `02007350`: the money a Poké item of quantity `qty` is worth.
    #[must_use]
    pub fn money_value(&self, qty: u8) -> i32 {
        if self.money_values.is_empty() {
            return i32::from(qty);
        }
        // The game does not bounds-check the index; the table has 100 entries and qty <= 99.
        let value = self
            .money_values
            .get(usize::from(qty))
            .copied()
            .unwrap_or(0);
        i32::try_from(value).unwrap_or(i32::MAX)
    }
}

/// `02138B74(row)`: the weighted floor-item pick. Draws `DungeonRandInt(0x2710)` for the
/// category; a hit gives a second `DungeonRandInt(0x2710)` for the item of that category. The
/// first item (by id) of the category whose cumulative weight is above the roll wins; no hit
/// at either roll gives [`POKE`].
// shortcut: the walk (first cumulative weight above the roll, items of the category in id
// order) is read from the table layout and the fallback, not from the function body; no second
// roll is drawn after a category miss. Medium confidence (`docs/re/items.md`, P2 #459).
pub fn pick_floor_item(tables: &ItemTables, row: usize, rng: &mut DungeonRng) -> u8 {
    let roll = rng.int(PICKER_ROLL);
    let Some(r) = tables.rows.get(row) else {
        return POKE;
    };
    let Some(category) = r
        .categories
        .iter()
        .take(PICKER_CATEGORIES)
        .position(|&w| roll < i32::from(w))
    else {
        return POKE;
    };
    let roll = rng.int(PICKER_ROLL);
    r.items
        .iter()
        .take(PICKER_ITEMS)
        .enumerate()
        .find(|&(id, &w)| {
            roll < i32::from(w)
                && tables.categories.get(id).copied().map(usize::from) == Some(category)
        })
        .and_then(|(id, _)| u8::try_from(id).ok())
        .unwrap_or(POKE)
}

/// `021448A0` (mode 0) for a floor item of `id`: the quantity and sticky flag. A Poké item
/// rolls its quantity (`02143290`: `DungeonRandInt(100)` against the thresholds); anything else
/// draws `DungeonRandInt(100)` against the sticky chance.
// shortcut: the quantity roll walk of `02143290` halves an index over a table the sim does not
// read; it takes the highest threshold at or below the roll (quantity 1 when none). A thrown
// stack takes its table amount without a draw (the game's range roll `020083D4` is not read).
fn new_floor_item(tables: &ItemTables, id: u8, rng: &mut DungeonRng) -> Item {
    let mut item = Item::new(id, 1);
    if id == POKE {
        let n = rng.int(100);
        let hit = tables
            .money_roll
            .iter()
            .rposition(|&t| i32::from(t) <= n)
            .and_then(|i| u8::try_from(i).ok());
        item.qty = hit.unwrap_or(1).max(1);
        return item;
    }
    if rng.int(100) < i32::from(tables.sticky_chance) {
        item.flags |= FLAG_STICKY;
    }
    if id < STACKABLE_BELOW {
        item.qty = tables
            .stack_amount
            .get(usize::from(id))
            .copied()
            .unwrap_or(1)
            .clamp(1, STACK_CAP);
    }
    item
}

/// One item on the floor and the tile it lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorItem {
    pub pos: (i16, i16),
    pub item: Item,
}

/// The item state of a floor: the bag (it travels with the team), the held items, and the
/// floor item slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemState {
    pub(crate) bag: Bag,
    /// The held item of each team slot (`info+0x5A`).
    pub(crate) held: [Item; TEAM_SLOTS],
    /// The 64 floor item slots.
    pub(crate) floor: Vec<Option<FloorItem>>,
    /// `numItems` (`gDungeon+0x3424`): one past the highest slot ever used and still held.
    pub(crate) num_items: u16,
    pub(crate) tables: ItemTables,
}

impl ItemState {
    pub(crate) fn new(bag: Bag, tables: ItemTables) -> Self {
        Self {
            bag,
            held: [Item::default(); TEAM_SLOTS],
            floor: vec![None; FLOOR_ITEM_SLOTS],
            num_items: 0,
            tables,
        }
    }

    /// `02143F98`'s tail: `numItems` is one past the highest used slot.
    fn refresh_count(&mut self) {
        let high = self
            .floor
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        self.num_items = u16::try_from(high).unwrap_or(0);
    }

    /// `021440B0`: lays `item` at `pos` in the first free slot. `None` when all 64 are used.
    pub(crate) fn spawn(&mut self, pos: (i16, i16), item: Item) -> Option<usize> {
        let slot = self.floor.iter().position(Option::is_none)?;
        self.floor[slot] = Some(FloorItem { pos, item });
        self.num_items = self.num_items.max(u16::try_from(slot + 1).unwrap_or(0));
        Some(slot)
    }

    /// The slot of the floor item on `pos`.
    pub(crate) fn slot_at(&self, pos: (i16, i16)) -> Option<usize> {
        self.floor
            .iter()
            .position(|f| f.is_some_and(|f| f.pos == pos))
    }

    /// `02143F98`: removes the floor item of `slot`.
    fn remove(&mut self, slot: usize) {
        if let Some(entry) = self.floor.get_mut(slot) {
            *entry = None;
        }
        self.refresh_count();
    }
}

/// What a pickup did (`021441D8` and `02184E50`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pickup {
    /// No floor item on the tile.
    NoItem,
    /// The run-away gate `02180B68` refused.
    RunAway,
    /// The item is shop stock: the game opens the ground menu instead (`gDungeon+0x5B8 = 4`),
    /// which the sim does not model.
    ShopStock,
    /// A Poké item: this much was added to the team money.
    Money(i32),
    /// The item merged into an existing stack.
    Stacked,
    /// The item went to a free bag slot, or to the held slot.
    Placed,
    /// No stack and no free slot: the floor item stays.
    Full,
}

impl Dungeon {
    /// The floor item on `pos`, if any (the tile's item word).
    #[must_use]
    pub fn floor_item_at(&self, pos: (i16, i16)) -> Option<Item> {
        let slot = self.items.slot_at(pos)?;
        self.items.floor[slot].map(|f| f.item)
    }

    /// The 64 floor item slots.
    #[must_use]
    pub fn floor_items(&self) -> &[Option<FloorItem>] {
        &self.items.floor
    }

    /// `numItems`, the floor item high-water mark.
    #[must_use]
    pub fn num_items(&self) -> u16 {
        self.items.num_items
    }

    /// The team bag and money.
    #[must_use]
    pub fn bag(&self) -> &Bag {
        &self.items.bag
    }

    /// The held item of team slot `member`.
    #[must_use]
    pub fn held_item(&self, member: usize) -> Option<Item> {
        self.items.held.get(member).copied()
    }

    /// The item tables of the run.
    #[must_use]
    pub fn item_tables(&self) -> &ItemTables {
        &self.items.tables
    }

    /// `02180B68(entity, 1)`: whether team member `member` runs away instead of picking up
    /// (`02180BA0`). The leader is exempt. An ally runs with Run Away (ability `0x2B`) below
    /// half HP, with tactic `0x0A`, or with tactic 6 at or below half HP.
    // shortcut: the terrified status (`info+0xD7`) is not modelled, so it never refuses.
    fn runs_away(&self, member: usize) -> bool {
        let Some(m) = self.team.get(member) else {
            return false;
        };
        if m.info.is_team_leader {
            return false;
        }
        let half = m.info.max_hp / 2;
        let tactic = self.ai.tactic(member);
        (m.info.has_ability(0x2B) && m.info.hp < half)
            || tactic == 0x0A
            || (tactic == 6 && m.info.hp <= half)
    }

    /// Pickup for team member `member` on its tile (`021441D8` for the leader, `02184E50` for
    /// an ally; the bodies are the same). Order: the floor item, the shop-stock refusal, the
    /// run-away gate, money, stacking, the first free slot, bag full.
    ///
    /// With no bag (`+0x64C == 0`) or a member that is not on the team, only the held slot is a
    /// candidate. A stack needs the same id (below 9), neither item sticky, and room (not 99);
    /// the fullest match wins, the bag before the held slot on a tie.
    // shortcut: the ally guards on the species byte (`info+0x40` not 'J' or 'G') and on
    // `info+0x98 != 1`, `info+8 != 1` and `0214067C` are not modelled; no AI action calls this
    // for an ally yet (the pickup decision is in `ai.md`, not ported).
    pub fn pickup(&mut self, member: usize) -> Pickup {
        let Some(m) = self.team.get(member) else {
            return Pickup::NoItem;
        };
        let (pos, off_team) = (m.pos, m.info.not_team_member);
        let Some(slot) = self.items.slot_at(pos) else {
            return Pickup::NoItem;
        };
        let Some(floor_item) = self.items.floor[slot] else {
            return Pickup::NoItem;
        };
        let item = floor_item.item;
        if item.flags & FLAG_SHOP != 0 {
            return Pickup::ShopStock;
        }
        if self.runs_away(member) {
            return Pickup::RunAway;
        }
        let st = &mut self.items;
        if item.id == POKE {
            let amount = st.tables.money_value(item.qty);
            st.bag.add_money(amount);
            st.remove(slot);
            return Pickup::Money(amount);
        }
        let use_bag = st.tables.has_inventory && !off_team;
        if item.id < STACKABLE_BELOW && !item.sticky() {
            // Candidates in order: the bag slots, then the held slot.
            let fits = |c: Item| c.exists() && c.id == item.id && !c.sticky() && c.qty != STACK_CAP;
            let mut best: Option<(usize, u8)> = None;
            if use_bag {
                for (i, c) in st.bag.slots.iter().enumerate() {
                    if fits(*c) && best.is_none_or(|(_, q)| c.qty > q) {
                        best = Some((i, c.qty));
                    }
                }
            }
            let held = st.held.get(member).copied().unwrap_or_default();
            let held_wins = fits(held) && best.is_none_or(|(_, q)| held.qty > q);
            let merged = |c: Item| Item {
                qty: c.qty.saturating_add(item.qty).min(STACK_CAP),
                ..c
            };
            if held_wins {
                st.held[member] = merged(held);
                st.remove(slot);
                return Pickup::Stacked;
            }
            if let Some((i, _)) = best {
                st.bag.slots[i] = merged(st.bag.slots[i]);
                st.remove(slot);
                return Pickup::Stacked;
            }
        }
        if use_bag {
            if !st.bag.add(item) {
                return Pickup::Full;
            }
        } else {
            match st.held.get_mut(member) {
                Some(h) if !h.exists() => {
                    *h = Item {
                        flags: item.flags & !FLAG_MARK,
                        ..item
                    }
                }
                _ => return Pickup::Full,
            }
        }
        st.remove(slot);
        Pickup::Placed
    }

    /// `02173CCC` (action 8): drops bag slot `slot` of the leader on its tile. Refusals: no
    /// item, a sticky item carrying the marker, a tile that is the stairs or not plain floor, a
    /// tile that already holds an item, and no free floor slot. The item loses its marker.
    // shortcut: a trap on the tile (an object too) is not checked: the sim has no traps. A full
    // floor refuses the drop; the game's `021440B0` result for 64 used slots is not read.
    pub(crate) fn drop_check(&self, slot: u8) -> Result<(), Blocked> {
        let leader = self.team.first().ok_or(Blocked::NoLeader)?;
        let item = self
            .items
            .bag
            .slots
            .get(usize::from(slot))
            .copied()
            .filter(|i| i.exists())
            .ok_or(Blocked::NoItem)?;
        if item.sticky() && item.flags & FLAG_MARK != 0 {
            return Err(Blocked::ItemSticky);
        }
        let tile = self
            .floor
            .tile(i32::from(leader.pos.0), i32::from(leader.pos.1))
            .ok_or(Blocked::OutOfBounds)?;
        if tile.terrain & floor::TERRAIN_STAIRS != 0
            || tile.terrain & floor::TERRAIN_TYPE_MASK != floor::TERRAIN_FLOOR
            || self.items.slot_at(leader.pos).is_some()
        {
            return Err(Blocked::DropTile);
        }
        if self.items.floor.iter().all(Option::is_some) {
            return Err(Blocked::FloorItemsFull);
        }
        Ok(())
    }

    /// The effect of a checked drop (see [`Self::drop_check`]).
    pub(crate) fn drop_apply(&mut self, slot: u8) {
        let Some(pos) = self.team.first().map(|l| l.pos) else {
            return;
        };
        let st = &mut self.items;
        let Some(&entry) = st.bag.slots.get(usize::from(slot)) else {
            return;
        };
        let item = Item {
            flags: entry.flags & !FLAG_MARK,
            ..entry
        };
        if st.spawn(pos, item).is_some() {
            st.bag.slots[usize::from(slot)] = Item::default();
            st.bag.compact();
        }
    }

    /// `02173A48` (action 0x0A): swaps bag slot `slot` with the leader's held item. Refused with
    /// no item in the slot, or when either item is sticky.
    // shortcut: "refuses sticky pairs" is read as either item being sticky; the function body
    // beyond the refusal and the `info+0x5A..5C` write is not read.
    pub(crate) fn swap_check(&self, slot: u8) -> Result<(), Blocked> {
        if self.team.is_empty() {
            return Err(Blocked::NoLeader);
        }
        let item = self
            .items
            .bag
            .slots
            .get(usize::from(slot))
            .copied()
            .filter(|i| i.exists())
            .ok_or(Blocked::NoItem)?;
        if item.sticky() || self.items.held[0].sticky() {
            return Err(Blocked::ItemSticky);
        }
        Ok(())
    }

    /// The effect of a checked swap (see [`Self::swap_check`]). The held item never carries the
    /// marker. An empty held slot leaves the bag slot empty, and the bag is compacted.
    pub(crate) fn swap_apply(&mut self, slot: u8) {
        let st = &mut self.items;
        let Some(entry) = st.bag.slots.get_mut(usize::from(slot)) else {
            return;
        };
        let incoming = Item {
            flags: entry.flags & !FLAG_MARK,
            ..*entry
        };
        *entry = std::mem::replace(&mut st.held[0], incoming);
        if !entry.exists() {
            *entry = Item::default();
        }
        st.bag.compact();
    }
}

/// `02144714`: the floor items of a floor just generated. Each tile with the item spawn mark,
/// in column order (x outer, y inner), takes the picker row of its terrain (3 for a tile with
/// terrain type 0, 1 for the shop bit, 2 for the monster-house bit, else 0), the pick
/// ([`pick_floor_item`]), then the item's own draws. Returns the slots, at most
/// [`FLOOR_ITEM_SLOTS`]; a mark past the 64th makes no item.
// shortcut: the scan order (column order, as the enemy marks) and the draw order inside one
// tile (pick, then quantity or sticky) are not read from `02144714`; the monster-house tiles
// are placed like any others (the house's own item list is not read).
pub(crate) fn spawn_floor_items(
    floor: &Floor,
    tables: &ItemTables,
    rng: &mut DungeonRng,
) -> ItemState {
    let mut state = ItemState::new(Bag::default(), tables.clone());
    for pos in floor::item_spawn_spots(floor) {
        let Some(tile) = floor.tile(i32::from(pos.0), i32::from(pos.1)) else {
            continue;
        };
        let row = if tile.terrain & floor::TERRAIN_TYPE_MASK == 0 {
            3
        } else if tile.terrain & floor::TERRAIN_SHOP != 0 {
            1
        } else if tile.terrain & floor::TERRAIN_MONSTER_HOUSE != 0 {
            2
        } else {
            0
        };
        let id = pick_floor_item(tables, row, rng);
        let item = new_floor_item(tables, id, rng);
        state.spawn(pos, item);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floor::{Floor, HEIGHT, TERRAIN_FLOOR, WIDTH};
    use crate::state::{EntityInfo, GameState, Monster, MoveSlot};

    fn info(leader: bool) -> EntityInfo {
        EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: leader,
            level: 5,
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
            regen_speed: 0,
            exp: 0,
            exp_gained: 0,
        }
    }

    fn open_floor() -> Floor {
        let mut f = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                f.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        f
    }

    /// A run with the leader at (5, 5) and an ally at (9, 9).
    fn run() -> GameState {
        let mut g = GameState::new(7);
        let team = vec![
            Monster {
                pos: (5, 5),
                visible: true,
                info: info(true),
            },
            Monster {
                pos: (9, 9),
                visible: true,
                info: info(false),
            },
        ];
        g.enter_dungeon(open_floor(), team, 0, &[]).unwrap();
        g.set_speed_stage(crate::turn::MAX_SPEED_STAGE);
        g
    }

    fn dungeon(g: &mut GameState) -> &mut Dungeon {
        g.dungeon_mut().unwrap()
    }

    #[test]
    fn bag_add_takes_the_first_free_slot_and_reports_full() {
        let mut bag = Bag::default();
        assert!(bag.add(Item::new(0x40, 1)));
        bag.slots[0] = Item::default();
        assert!(bag.add(Item::new(0x41, 1)));
        assert_eq!(bag.slots[0].id, 0x41);
        for _ in 1..BAG_SLOTS {
            assert!(bag.add(Item::new(0x42, 1)));
        }
        assert!(!bag.add(Item::new(0x43, 1)));
    }

    #[test]
    fn compact_keeps_order() {
        let mut bag = Bag::default();
        bag.slots[2] = Item::new(5, 1);
        bag.slots[7] = Item::new(6, 1);
        bag.compact();
        assert_eq!((bag.slots[0].id, bag.slots[1].id), (5, 6));
        assert!(!bag.slots[2].exists());
    }

    #[test]
    fn money_clamps_at_99999() {
        let mut bag = Bag::default();
        bag.add_money(99_000);
        bag.add_money(5_000);
        assert_eq!(bag.money, MONEY_CAP);
        bag.add_money(i32::MAX);
        assert_eq!(bag.money, MONEY_CAP);
    }

    #[test]
    fn picker_draws_category_then_item_and_falls_back_to_poke() {
        let mut tables = ItemTables::default();
        let mut cats = vec![0u16; PICKER_CATEGORIES];
        cats[3] = 0x2710;
        let mut items = vec![0u16; PICKER_ITEMS];
        items[0x37] = 5000;
        items[0x38] = 10_000;
        tables.rows.push(PickerRow {
            categories: cats,
            items,
        });
        tables.categories = vec![0; 256];
        tables.categories[0x37] = 3;
        tables.categories[0x38] = 3;
        let mut a = DungeonRng::new(11);
        let mut b = a;
        let id = pick_floor_item(&tables, 0, &mut a);
        // The same two draws, by hand.
        let r1 = b.int(0x2710);
        assert!(r1 < 0x2710);
        let r2 = b.int(0x2710);
        assert_eq!(id, if r2 < 5000 { 0x37 } else { 0x38 });
        assert_eq!(a.state(), b.state());
        // A missing row still draws the category roll, then gives Poké.
        let mut c = DungeonRng::new(11);
        let mut d = c;
        assert_eq!(pick_floor_item(&tables, 9, &mut c), POKE);
        d.int(0x2710);
        assert_eq!(c.state(), d.state());
        // A category miss draws one roll and gives Poké.
        tables.rows[0].categories = vec![0; PICKER_CATEGORIES];
        let mut e = DungeonRng::new(11);
        let mut f = e;
        assert_eq!(pick_floor_item(&tables, 0, &mut e), POKE);
        f.int(0x2710);
        assert_eq!(e.state(), f.state());
    }

    #[test]
    fn floor_items_come_from_the_marks_in_column_order() {
        let mut floor = open_floor();
        for (x, y) in [(10, 3), (3, 8), (3, 2)] {
            floor.tile_mut(x, y).unwrap().spawn_or_visibility |= 0x2;
        }
        let tables = ItemTables::default();
        let mut rng = DungeonRng::new(5);
        let st = spawn_floor_items(&floor, &tables, &mut rng);
        let at: Vec<_> = st.floor.iter().flatten().map(|f| f.pos).collect();
        assert_eq!(at, vec![(3, 2), (3, 8), (10, 3)]);
        assert_eq!(st.num_items, 3);
        // Default tables: a Poké item per mark, each with the pick roll and the quantity roll.
        assert!(st.floor.iter().flatten().all(|f| f.item.id == POKE));
        let mut by_hand = DungeonRng::new(5);
        for _ in 0..3 {
            by_hand.int(0x2710);
            by_hand.int(100);
        }
        assert_eq!(rng.state(), by_hand.state());
    }

    #[test]
    fn floor_item_row_follows_the_tile() {
        let mut floor = open_floor();
        floor.tile_mut(1, 1).unwrap().terrain = 0; // type 0: row 3
        floor.tile_mut(2, 1).unwrap().terrain = TERRAIN_FLOOR | floor::TERRAIN_SHOP; // row 1
        floor.tile_mut(3, 1).unwrap().terrain = TERRAIN_FLOOR | floor::TERRAIN_MONSTER_HOUSE; // row 2
        floor.tile_mut(4, 1).unwrap().terrain = TERRAIN_FLOOR; // row 0
        for x in 1..=4 {
            floor.tile_mut(x, 1).unwrap().spawn_or_visibility |= 0x2;
        }
        // Each row holds one category-0 item of its own id.
        let mut tables = ItemTables {
            categories: vec![0; 256],
            ..ItemTables::default()
        };
        for row in 0..4u16 {
            let mut items = vec![0u16; PICKER_ITEMS];
            items[usize::from(10 + row)] = 0x2710;
            let mut cats = vec![0u16; PICKER_CATEGORIES];
            cats[0] = 0x2710;
            tables.rows.push(PickerRow {
                categories: cats,
                items,
            });
        }
        let st = spawn_floor_items(&floor, &tables, &mut DungeonRng::new(1));
        let ids: Vec<_> = st
            .floor
            .iter()
            .flatten()
            .map(|f| (f.pos.0, f.item.id))
            .collect();
        assert_eq!(ids, vec![(1, 13), (2, 11), (3, 12), (4, 10)]);
    }

    #[test]
    fn floor_slots_hold_64_and_a_mark_past_them_makes_nothing() {
        let mut floor = open_floor();
        let mut marks = 0;
        for x in 1..20 {
            for y in 1..6 {
                floor.tile_mut(x, y).unwrap().spawn_or_visibility |= 0x2;
                marks += 1;
            }
        }
        assert!(marks > FLOOR_ITEM_SLOTS);
        let st = spawn_floor_items(&floor, &ItemTables::default(), &mut DungeonRng::new(2));
        assert_eq!(st.floor.iter().flatten().count(), FLOOR_ITEM_SLOTS);
        assert_eq!(st.num_items, 64);
    }

    #[test]
    fn num_items_is_a_high_water_mark_over_the_used_slots() {
        let mut st = ItemState::new(Bag::default(), ItemTables::default());
        for i in 0..3 {
            st.spawn((i, 0), Item::new(0x40, 1));
        }
        assert_eq!(st.num_items, 3);
        st.remove(1);
        // Slot 2 is still used, so the mark stays 3; a spawn takes the free slot 1.
        assert_eq!(st.num_items, 3);
        assert_eq!(st.spawn((9, 9), Item::new(0x41, 1)), Some(1));
        st.remove(2);
        assert_eq!(st.num_items, 2);
        st.remove(1);
        st.remove(0);
        assert_eq!(st.num_items, 0);
    }

    fn lay(g: &mut GameState, pos: (i16, i16), item: Item) {
        dungeon(g).items.spawn(pos, item).unwrap();
    }

    #[test]
    fn stepping_on_an_item_picks_it_up() {
        let mut g = run();
        lay(&mut g, (6, 5), Item::new(0x40, 1));
        g.step(crate::command::Command::Move(
            crate::command::Direction::Right,
        ));
        let d = g.dungeon().unwrap();
        assert_eq!(d.bag().slots[0], Item::new(0x40, 1));
        assert!(d.floor_item_at((6, 5)).is_none());
        assert_eq!(d.num_items(), 0);
    }

    #[test]
    fn money_never_takes_a_slot_and_uses_the_value_table() {
        let mut g = run();
        dungeon(&mut g).items.tables.money_values = (0..100).map(|q| q * 10).collect();
        lay(&mut g, (5, 5), Item::new(POKE, 7));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Money(70));
        let d = g.dungeon().unwrap();
        assert_eq!(d.bag().money, 70);
        assert!(!d.bag().slots[0].exists());
        // Money clamps.
        let mut g = run();
        lay(&mut g, (5, 5), Item::new(POKE, 99));
        dungeon(&mut g).items.bag.money = 99_990;
        dungeon(&mut g).pickup(0);
        assert_eq!(g.dungeon().unwrap().bag().money, MONEY_CAP);
    }

    #[test]
    fn money_is_taken_even_with_a_full_bag() {
        let mut g = run();
        for i in 0..BAG_SLOTS {
            dungeon(&mut g).items.bag.slots[i] = Item::new(0x50, 1);
        }
        lay(&mut g, (5, 5), Item::new(POKE, 4));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Money(4));
    }

    #[test]
    fn thrown_stacks_merge_into_the_fullest_and_clamp_at_99() {
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(3, 10);
            d.items.bag.slots[1] = Item::new(3, 40);
            d.items.bag.slots[2] = Item::new(3, 99);
        }
        lay(&mut g, (5, 5), Item::new(3, 70));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Stacked);
        let d = g.dungeon().unwrap();
        // The 99 stack is skipped; the fullest other (40) takes the sum, clamped.
        assert_eq!(d.bag().slots[1].qty, 99);
        assert_eq!(d.bag().slots[0].qty, 10);
        assert!(d.floor_item_at((5, 5)).is_none());
    }

    #[test]
    fn only_ids_below_9_stack_and_sticky_items_do_not() {
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(9, 1);
        lay(&mut g, (5, 5), Item::new(9, 1));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
        assert_eq!(g.dungeon().unwrap().bag().slots[1].id, 9);
        // A sticky floor item of id 3 does not merge.
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(3, 5);
        lay(
            &mut g,
            (5, 5),
            Item {
                flags: FLAG_EXISTS | FLAG_STICKY,
                qty: 2,
                id: 3,
            },
        );
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
        assert_eq!(g.dungeon().unwrap().bag().slots[0].qty, 5);
    }

    #[test]
    fn a_stack_can_merge_into_the_held_slot() {
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.held[0] = Item::new(2, 50);
            d.items.bag.slots[0] = Item::new(2, 20);
        }
        lay(&mut g, (5, 5), Item::new(2, 3));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Stacked);
        assert_eq!(g.dungeon().unwrap().held_item(0).unwrap().qty, 53);
        assert_eq!(g.dungeon().unwrap().bag().slots[0].qty, 20);
    }

    #[test]
    fn full_bag_leaves_the_item_on_the_floor() {
        let mut g = run();
        for i in 0..BAG_SLOTS {
            dungeon(&mut g).items.bag.slots[i] = Item::new(0x50, 1);
        }
        lay(&mut g, (5, 5), Item::new(0x40, 1));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Full);
        assert!(g.dungeon().unwrap().floor_item_at((5, 5)).is_some());
    }

    #[test]
    fn no_inventory_gate_uses_only_the_held_slot() {
        let mut g = run();
        dungeon(&mut g).items.tables.has_inventory = false;
        lay(&mut g, (5, 5), Item::new(0x40, 1));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
        {
            let d = g.dungeon().unwrap();
            assert_eq!(d.held_item(0), Some(Item::new(0x40, 1)));
            assert!(!d.bag().slots[0].exists());
        }
        // The held slot is now used: the next item does not fit and stays.
        let d = dungeon(&mut g);
        d.items.spawn((5, 5), Item::new(0x41, 1));
        assert_eq!(d.pickup(0), Pickup::Full);
        // A stack still merges into the held slot.
        d.items.held[0] = Item::new(4, 5);
        d.items.remove(d.items.slot_at((5, 5)).unwrap());
        d.items.spawn((5, 5), Item::new(4, 2));
        assert_eq!(d.pickup(0), Pickup::Stacked);
        assert_eq!(d.held_item(0).unwrap().qty, 7);
    }

    #[test]
    fn a_member_not_on_the_team_uses_the_held_slot_only() {
        let mut g = run();
        dungeon(&mut g).team[1].info.not_team_member = true;
        lay(&mut g, (9, 9), Item::new(0x40, 1));
        assert_eq!(dungeon(&mut g).pickup(1), Pickup::Placed);
        let d = g.dungeon().unwrap();
        assert_eq!(d.held_item(1), Some(Item::new(0x40, 1)));
        assert!(!d.bag().slots[0].exists());
    }

    #[test]
    fn shop_stock_is_not_picked_up() {
        let mut g = run();
        lay(
            &mut g,
            (5, 5),
            Item {
                flags: FLAG_EXISTS | FLAG_SHOP,
                qty: 1,
                id: 0x40,
            },
        );
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::ShopStock);
        assert!(g.dungeon().unwrap().floor_item_at((5, 5)).is_some());
    }

    #[test]
    fn the_run_away_gate_refuses_an_ally_but_not_the_leader() {
        let mut g = run();
        lay(&mut g, (9, 9), Item::new(0x40, 1));
        {
            let d = dungeon(&mut g);
            d.team[1].info.abilities = [0x2B, 0];
            d.team[1].info.hp = 14;
        }
        assert_eq!(dungeon(&mut g).pickup(1), Pickup::RunAway);
        // At exactly half HP the ability does not trigger (strictly below).
        dungeon(&mut g).team[1].info.hp = 15;
        assert_eq!(dungeon(&mut g).pickup(1), Pickup::Placed);
        // The leader is exempt even when the same conditions hold.
        let mut g = run();
        lay(&mut g, (5, 5), Item::new(0x40, 1));
        {
            let d = dungeon(&mut g);
            d.team[0].info.abilities = [0x2B, 0];
            d.team[0].info.hp = 1;
        }
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
    }

    #[test]
    fn nothing_underfoot_is_no_item() {
        let mut g = run();
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::NoItem);
        assert_eq!(dungeon(&mut g).pickup(7), Pickup::NoItem);
    }

    #[test]
    fn drop_puts_the_item_on_the_tile_and_compacts() {
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(0x40, 1);
            d.items.bag.slots[1] = Item {
                flags: FLAG_EXISTS | FLAG_MARK,
                qty: 1,
                id: 0x41,
            };
            d.items.bag.slots[2] = Item::new(0x42, 1);
        }
        assert_eq!(g.drop_item(1), crate::command::Outcome::Applied);
        let d = g.dungeon().unwrap();
        assert_eq!(d.bag().slots[0].id, 0x40);
        assert_eq!(d.bag().slots[1].id, 0x42);
        assert!(!d.bag().slots[2].exists());
        // The marker is cleared on the floor item.
        assert_eq!(d.floor_item_at((5, 5)), Some(Item::new(0x41, 1)));
        assert_eq!(d.num_items(), 1);
    }

    #[test]
    fn drop_refusals() {
        use crate::command::{Blocked, Outcome};
        let mut g = run();
        assert_eq!(g.drop_item(0), Outcome::Blocked(Blocked::NoItem));
        assert_eq!(g.drop_item(99), Outcome::Blocked(Blocked::NoItem));
        dungeon(&mut g).items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_STICKY | FLAG_MARK,
            qty: 1,
            id: 0x40,
        };
        assert_eq!(g.drop_item(0), Outcome::Blocked(Blocked::ItemSticky));
        // Sticky alone, without the marker, drops.
        dungeon(&mut g).items.bag.slots[0].flags = FLAG_EXISTS | FLAG_STICKY;
        // An item already on the tile blocks.
        lay(&mut g, (5, 5), Item::new(0x41, 1));
        assert_eq!(g.drop_item(0), Outcome::Blocked(Blocked::DropTile));
        // Stairs and non-floor terrain block.
        dungeon(&mut g).items.remove(0);
        dungeon(&mut g).floor.tile_mut(5, 5).unwrap().terrain |= floor::TERRAIN_STAIRS;
        assert_eq!(g.drop_item(0), Outcome::Blocked(Blocked::DropTile));
        let t = dungeon(&mut g).floor.tile_mut(5, 5).unwrap();
        t.terrain = 2;
        assert_eq!(g.drop_item(0), Outcome::Blocked(Blocked::DropTile));
        // The refusals changed nothing and took no turn.
        assert_eq!(g.dungeon().unwrap().turn(), 0);
        let t = dungeon(&mut g).floor.tile_mut(5, 5).unwrap();
        t.terrain = TERRAIN_FLOOR;
        assert_eq!(g.drop_item(0), Outcome::Applied);
    }

    #[test]
    fn drop_refuses_when_all_floor_slots_are_used() {
        use crate::command::{Blocked, Outcome};
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(0x40, 1);
        for i in 0..FLOOR_ITEM_SLOTS {
            dungeon(&mut g).items.spawn(
                (20 + (i % 30) as i16, 20 + (i / 30) as i16),
                Item::new(1, 1),
            );
        }
        assert_eq!(g.drop_item(0), Outcome::Blocked(Blocked::FloorItemsFull));
    }

    #[test]
    fn swap_exchanges_the_bag_item_and_the_held_item() {
        use crate::command::Outcome;
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(0x10, 1);
            d.items.bag.slots[1] = Item {
                flags: FLAG_EXISTS | FLAG_MARK,
                qty: 1,
                id: 0x11,
            };
            d.items.held[0] = Item::new(0x12, 1);
        }
        assert_eq!(g.swap_held_item(1), Outcome::Applied);
        let d = g.dungeon().unwrap();
        // The marker is stripped from the held item; the old held item takes the bag slot.
        assert_eq!(d.held_item(0), Some(Item::new(0x11, 1)));
        assert_eq!(d.bag().slots[1], Item::new(0x12, 1));
        // With an empty held slot the bag slot empties and the bag compacts.
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(0x10, 1);
            d.items.bag.slots[1] = Item::new(0x11, 1);
        }
        assert_eq!(g.swap_held_item(0), Outcome::Applied);
        let d = g.dungeon().unwrap();
        assert_eq!(d.held_item(0), Some(Item::new(0x10, 1)));
        assert_eq!(d.bag().slots[0].id, 0x11);
        assert!(!d.bag().slots[1].exists());
    }

    #[test]
    fn swap_refusals() {
        use crate::command::{Blocked, Outcome};
        let mut g = run();
        assert_eq!(g.swap_held_item(0), Outcome::Blocked(Blocked::NoItem));
        let sticky = Item {
            flags: FLAG_EXISTS | FLAG_STICKY,
            qty: 1,
            id: 0x13,
        };
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(0x10, 1);
            d.items.held[0] = sticky;
        }
        assert_eq!(g.swap_held_item(0), Outcome::Blocked(Blocked::ItemSticky));
        {
            let d = dungeon(&mut g);
            d.items.held[0] = Item::default();
            d.items.bag.slots[0] = sticky;
        }
        assert_eq!(g.swap_held_item(0), Outcome::Blocked(Blocked::ItemSticky));
        assert_eq!(g.dungeon().unwrap().turn(), 0);
    }

    #[test]
    fn items_survive_snapshot_and_resume() {
        let tables = ItemTables {
            sticky_chance: 9,
            ..ItemTables::new()
        };
        let mut g = run().with_item_tables(tables);
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[3] = Item::new(0x40, 1);
            d.items.bag.money = 1234;
            d.items.held[1] = Item::new(0x41, 1);
            d.items.spawn((12, 12), Item::new(0x42, 1));
            d.items.spawn((13, 12), Item::new(0x43, 1));
            d.items.remove(0);
        }
        let back = GameState::restore_with(&g.snapshot(), &g.resume()).unwrap();
        let (a, b) = (g.dungeon().unwrap(), back.dungeon().unwrap());
        assert_eq!(a.items, b.items, "items");
        assert_eq!(g.bag(), back.bag());
        assert_eq!(back.dungeon().unwrap().num_items(), 2);
    }

    #[test]
    fn a_resume_with_too_many_floor_slots_is_refused() {
        let g = run();
        let mut resume = g.resume();
        resume.dungeon.as_mut().unwrap().floor_items = vec![None; FLOOR_ITEM_SLOTS + 1];
        assert!(GameState::restore_with(&g.snapshot(), &resume).is_err());
    }
}
