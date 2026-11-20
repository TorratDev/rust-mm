//! Items: the bag, the held slots, floor items, pickup, drop, swap and money (`docs/re/items.md`,
//! P1, P2 "#459" for the picker, P7 for the layout).
//!
//! The bag is the team's 20 three-byte slots (`*0x020DECF8`, P7); a held item sits in each team
//! member's info record (`info+0x5A`). Floor items are 64 slots with `numItems` as a high-water
//! mark (`gDungeon+0x3364`, `+0x3424`). Money never takes a bag slot. Integer math only.

use serde::{Deserialize, Serialize};

use crate::command::Blocked;
use crate::floor::{self, Floor};
use crate::item_use::{START_MAX_BELLY, UseParams, UseResult};
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

    /// `HasHeldItem` (`021432E8`): the slot exists, is not sticky, and has item `id`. Applies to
    /// a held slot; the sticky bit makes a held item count as absent here.
    #[must_use]
    pub const fn holds(self, id: u8) -> bool {
        self.exists() && !self.sticky() && self.id == id
    }
}

/// Held item ids the sim tests (`docs/re/items.md`, P4; the ids are blue-verified, the names are
/// Red's by numeric match).
pub const MOBILE_SCARF: u8 = 0x09;
pub const TWIST_BAND: u8 = 0x0B;
pub const SCOPE_LENS: u8 = 0x0C;
pub const PATSY_BAND: u8 = 0x0D;
pub const INSOMNISCOPE: u8 = 0x15;
pub const POWER_BAND: u8 = 0x13;
pub const DEF_SCARF: u8 = 0x1E;
pub const SPECIAL_BAND: u8 = 0x21;
pub const ZINC_BAND: u8 = 0x22;
pub const MUNCH_BELT: u8 = 0x2B;
pub const DIET_RIBBON: u8 = 0x1B;

/// Why a held-item action did nothing: `give`, `take`, `set_marker` and `unset_marker`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldBlocked {
    /// The bag slot or the held slot is empty, or the bag slot is not one of the 20.
    NoItem,
    /// The item is sticky. A bag item refused by [`Dungeon::give`] or a marker change must be
    /// sticky and marked; a held item must only be sticky.
    ItemSticky,
    /// The bag has no free slot.
    BagFull,
    /// The target is not another team member (the leader gives to an ally).
    NoAlly,
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
    /// The int[100] table of the quantity roll of a Poké floor item (`DAT_021432e4`, `02143290`).
    pub quantity_table: Vec<i32>,
    /// The floor's item level, the byte at `gDungeon+0x1A30D`: the roll's limit is `level * 0x28`.
    pub floor_level: u8,
    /// `gDungeon+0x64C`: the player has a bag in this dungeon (`docs/re/items.md`, #464).
    pub has_inventory: bool,
    /// The numbers of the item handlers (`docs/re/items.md`, P2).
    #[serde(default)]
    pub use_params: UseParams,
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
            quantity_table: Vec::new(),
            floor_level: 0,
            has_inventory: true,
            use_params: UseParams::new(),
        }
    }

    /// `020082FC`: the category of item `id`. An id past the table reads 0, except that Poké
    /// reads 6 when the caller gave no table (it is the money item, see [`POKE`]).
    #[must_use]
    pub fn category(&self, id: u8) -> u8 {
        let default = if id == POKE { 6 } else { 0 };
        self.categories
            .get(usize::from(id))
            .copied()
            .unwrap_or(default)
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

/// `02138B74(row)`: the weighted floor-item pick. Draws `DungeonRandInt(0x2710)` first. The
/// first category (0 to 11) with a non-zero weight at or above the roll wins; with no hit the
/// result is [`POKE`] and no second roll is drawn. A hit draws a second `DungeonRandInt(0x2710)`
/// and takes the first item id (0 to 239) with a non-zero weight at or above that roll whose
/// category (`020082FC`) is the winner; no such item gives [`POKE`]. Both tests are
/// `roll <= weight` (the weights are cumulative).
pub fn pick_floor_item(tables: &ItemTables, row: usize, rng: &mut DungeonRng) -> u8 {
    let roll = rng.int(PICKER_ROLL);
    let Some(r) = tables.rows.get(row) else {
        return POKE;
    };
    let Some(category) = r
        .categories
        .iter()
        .take(PICKER_CATEGORIES)
        .position(|&w| w != 0 && roll <= i32::from(w))
    else {
        return POKE;
    };
    let roll = rng.int(PICKER_ROLL);
    r.items
        .iter()
        .take(PICKER_ITEMS)
        .enumerate()
        .find(|&(id, &w)| {
            w != 0
                && roll <= i32::from(w)
                && u8::try_from(id).is_ok_and(|id| usize::from(tables.category(id)) == category)
        })
        .and_then(|(id, _)| u8::try_from(id).ok())
        .unwrap_or(POKE)
}

/// `02007400(id)`: whether the sticky roll applies. False for 0, Poké and `0x77` to `0x7A`.
const fn can_be_sticky(id: u8) -> bool {
    !matches!(id, 0 | POKE | 0x77..=0x7A)
}

/// `02143290(item, limit)`: the quantity of a Poké floor item. Draws `DungeonRandInt(100)` as an
/// index into the table, then up to 200 times: when `table[index] <= limit` the index is the
/// quantity, otherwise the index halves. After 200 misses the quantity is 1.
fn money_quantity(tables: &ItemTables, rng: &mut DungeonRng) -> u8 {
    let limit = i32::from(tables.floor_level) * 0x28;
    let mut q = rng.int(100);
    for _ in 0..200 {
        let hit = usize::try_from(q)
            .ok()
            .and_then(|i| tables.quantity_table.get(i))
            .is_some_and(|&t| t <= limit);
        if hit {
            return u8::try_from(q).unwrap_or(1);
        }
        q /= 2;
    }
    1
}

/// `021448A0(item, id, 0)` for a floor item of `id`: `02008458` makes the item, then the sticky
/// roll `DungeonRandInt(100) < sticky_chance` is drawn when [`can_be_sticky`] holds, then a
/// category 6 item rolls its quantity ([`money_quantity`]).
// shortcut: `02008458` sets a thrown stack's quantity with `02011924(min, max)`, which draws on
// the non-dungeon generator (`0201198C`); the sim takes the table amount, and takes ids 1 to 8
// for the items `02008504` marks as stacking (that function is not read). A category 6 item
// starts at quantity 1 and any other at 0, as in `02008458`.
fn new_floor_item(tables: &ItemTables, id: u8, rng: &mut DungeonRng) -> Item {
    if id == 0 {
        return Item::default();
    }
    let mut item = Item::new(id, 0);
    if id < STACKABLE_BELOW && id != 0 {
        item.qty = tables
            .stack_amount
            .get(usize::from(id))
            .copied()
            .unwrap_or(1)
            .clamp(1, STACK_CAP);
    } else if tables.category(id) == 6 {
        item.qty = 1;
    }
    if can_be_sticky(id) && rng.int(100) < i32::from(tables.sticky_chance) {
        item.flags |= FLAG_STICKY;
    }
    if tables.category(id) == 6 {
        item.qty = money_quantity(tables, rng);
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
    /// The belly maximum of each team slot, in thousandths (Apples raise it).
    pub(crate) max_belly: [i32; TEAM_SLOTS],
    /// The last item use (`Dungeon::last_item_use`).
    pub(crate) last_use: Option<(u8, UseResult)>,
}

impl ItemState {
    pub(crate) fn new(bag: Bag, tables: ItemTables) -> Self {
        Self {
            bag,
            held: [Item::default(); TEAM_SLOTS],
            floor: vec![None; FLOOR_ITEM_SLOTS],
            num_items: 0,
            tables,
            max_belly: [START_MAX_BELLY; TEAM_SLOTS],
            last_use: None,
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
    pub(crate) fn runs_away(&self, member: usize) -> bool {
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

    /// Leader pickup on its tile (`021441D8`, the step-on pickup; the caller applies the
    /// `info+0x46 == 0` run gate). Order: the floor item, the shop-stock refusal, the
    /// run-away gate, money (category 6), stacking, the first free slot, bag full.
    ///
    /// The candidates are the member's held slot alone when `+0x64C == 0`, else the 20 bag
    /// slots alone. A stack needs an id below 9 and a floor item that is not shop stock; a
    /// candidate must exist, not be shop stock, share the id, not be at 99, and be strictly the
    /// fullest (the first wins a tie). The first pass also needs the same sticky bit; with no
    /// match the second pass drops that condition. The sum clamps to 99 and the slot becomes
    /// sticky when the floor item was.
    pub fn pickup(&mut self, member: usize) -> Pickup {
        self.pickup_from(member, false)
    }

    /// Ally pickup (`02184E50`, action 0x3F). It has no `+0x46` gate. The candidates are the 20
    /// bag slots and then the held slot (`info+0x5A`, index 0x15) when `+0x64C != 0` and
    /// `info+6 == 0` (every team member); with `+0x64C == 0` the held slot alone. Money runs
    /// only for `info+6 == 0`. Shop stock is refused. The free-slot search walks the same list,
    /// so a full bag puts the item in the held slot, and a stack can merge into the held slot.
    // shortcut: the ally guards on the species byte (`info+0x40` not 'J' or 'G') and on
    // `info+0x98 != 1`, `info+8 != 1` and `0214067C` are not modelled; no AI action calls this
    // yet (the pickup decision is in `ai.md`, not ported).
    pub fn ally_pickup(&mut self, member: usize) -> Pickup {
        self.pickup_from(member, true)
    }

    fn pickup_from(&mut self, member: usize, ally: bool) -> Pickup {
        let Some(m) = self.team.get(member) else {
            return Pickup::NoItem;
        };
        let Some(slot) = self.items.slot_at(m.pos) else {
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
        if st.tables.category(item.id) == 6 {
            let amount = st.tables.money_value(item.qty);
            st.bag.add_money(amount);
            st.remove(slot);
            return Pickup::Money(amount);
        }
        // The candidates: `None` is the held slot, `Some(i)` bag slot `i`.
        let candidates: Vec<Option<usize>> = if st.tables.has_inventory {
            let mut c: Vec<Option<usize>> = (0..BAG_SLOTS).map(Some).collect();
            if ally {
                c.push(None);
            }
            c
        } else {
            vec![None]
        };
        let get = |st: &ItemState, c: Option<usize>| match c {
            Some(i) => st.bag.slots[i],
            None => st.held.get(member).copied().unwrap_or_default(),
        };
        if item.id < STACKABLE_BELOW {
            let fullest = |same_sticky: bool| {
                let mut best: Option<(Option<usize>, u8)> = None;
                for &c in &candidates {
                    let it = get(st, c);
                    if it.exists()
                        && it.flags & FLAG_SHOP == 0
                        && it.id == item.id
                        && (!same_sticky || it.sticky() == item.sticky())
                        && it.qty != STACK_CAP
                        && best.is_none_or(|(_, q)| it.qty > q)
                    {
                        best = Some((c, it.qty));
                    }
                }
                best
            };
            if let Some((c, _)) = fullest(true).or_else(|| fullest(false)) {
                let mut merged = get(st, c);
                merged.qty = merged.qty.saturating_add(item.qty).min(STACK_CAP);
                merged.flags |= item.flags & FLAG_STICKY;
                match c {
                    Some(i) => st.bag.slots[i] = merged,
                    None => {
                        if let Some(h) = st.held.get_mut(member) {
                            *h = merged;
                        }
                    }
                }
                st.remove(slot);
                return Pickup::Stacked;
            }
        }
        match candidates.iter().find(|&&c| !get(st, c).exists()) {
            Some(Some(i)) => st.bag.slots[*i] = item,
            Some(None) => {
                if let Some(h) = st.held.get_mut(member) {
                    *h = item;
                }
            }
            None => return Pickup::Full,
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
        // shortcut: `0219037C` (the monster house wake-up: tile bit 0x40, flag `gDungeon+0x661`,
        // room `+0x34AC`) ends `02173CCC` and is not modelled.
    }

    /// `02173A48` (action 0x0A): swaps the floor item underfoot with `source`, a bag slot or the
    /// leader's held item. Refusals: no floor item underfoot (the game wants the `0x80`
    /// selector), an empty source, a bag item that is sticky and carries the marker, a held
    /// item that is sticky.
    // shortcut: a trap on the tile (the tile object not being an item) is not checked: the sim
    // has no traps. `021440B0` cannot fail here (the floor item's slot was just freed), so the
    // game's failure branch (a message, both items lost) is not modelled.
    pub(crate) fn swap_check(&self, source: SwapSource) -> Result<(), Blocked> {
        let leader = self.team.first().ok_or(Blocked::NoLeader)?;
        if self.items.slot_at(leader.pos).is_none() {
            return Err(Blocked::NoItem);
        }
        let item = match source {
            SwapSource::Bag(slot) => self.items.bag.slots.get(usize::from(slot)).copied(),
            SwapSource::Held => Some(self.items.held[0]),
        }
        .filter(|i| i.exists())
        .ok_or(Blocked::NoItem)?;
        let refused = match source {
            SwapSource::Bag(_) => item.flags & FLAG_MARK != 0 && item.sticky(),
            SwapSource::Held => item.sticky(),
        };
        if refused {
            return Err(Blocked::ItemSticky);
        }
        Ok(())
    }

    /// The effect of a checked swap (see [`Self::swap_check`]). Both items lose the marker. The
    /// floor item goes to the held slot (for a held source) or to the first free bag slot, and the
    /// bag is compacted.
    pub(crate) fn swap_apply(&mut self, source: SwapSource) {
        let Some(pos) = self.team.first().map(|l| l.pos) else {
            return;
        };
        let st = &mut self.items;
        let Some(slot) = st.slot_at(pos) else {
            return;
        };
        let Some(floor_item) = st.floor[slot] else {
            return;
        };
        let unmark = |i: Item| Item {
            flags: i.flags & !FLAG_MARK,
            ..i
        };
        let (down, up) = match source {
            SwapSource::Bag(s) => (st.bag.slots[usize::from(s)], unmark(floor_item.item)),
            SwapSource::Held => (st.held[0], unmark(floor_item.item)),
        };
        st.remove(slot);
        match source {
            SwapSource::Bag(s) => st.bag.slots[usize::from(s)] = Item::default(),
            SwapSource::Held => st.held[0] = Item::default(),
        }
        st.spawn(pos, unmark(down));
        match source {
            SwapSource::Held => st.held[0] = up,
            SwapSource::Bag(_) => {
                st.bag.add(up);
            }
        }
        st.bag.compact();
        // shortcut: `0219037C` (the monster house wake-up: tile bit 0x40, flag `gDungeon+0x661`,
        // room `+0x34AC`) ends `02173A48` and is not modelled.
    }
}

impl Dungeon {
    /// `HasHeldItem` for team member `member` (`021432E8`, see [`Item::holds`]). False for a
    /// member with no held slot.
    #[must_use]
    pub fn has_held_item(&self, member: usize, id: u8) -> bool {
        self.held_item(member).is_some_and(|h| h.holds(id))
    }

    /// The held item of a battle slot: a team member's held slot, or nothing for a wild monster
    /// (the sim keeps held items for the team only). Used by the damage and status tests.
    pub(crate) fn held_of(&self, slot: crate::targeting::Slot) -> Item {
        match slot {
            crate::targeting::Slot::Team(i) => self.held_item(i).unwrap_or_default(),
            crate::targeting::Slot::Wild(_) => Item::default(),
        }
    }

    /// `02174280` (action `0x36`, Give): the leader gives bag slot `slot` to the ally `target`.
    /// Refusals: no item in the slot; the item is sticky and marked (a bag item, not from the
    /// floor); `target` is not another team member; the target's held item is sticky (a message
    /// in the game). An empty held slot takes the item; else the held item goes to the bag and
    /// the given item to the held slot. Both lose the marker.
    // shortcut: the give from the floor (the item underfoot, `02141720` with the floor selector)
    // is not modelled; the messages and the ally's `0x4000` flag and the `02175F54` bad-tile fix
    // are not modelled.
    pub fn give(&mut self, slot: u8, target: usize) -> Result<(), HeldBlocked> {
        let item = self
            .items
            .bag
            .slots
            .get(usize::from(slot))
            .copied()
            .filter(|i| i.exists())
            .ok_or(HeldBlocked::NoItem)?;
        if item.sticky() && item.flags & FLAG_MARK != 0 {
            return Err(HeldBlocked::ItemSticky);
        }
        if target == 0 || target >= self.team.len() {
            return Err(HeldBlocked::NoAlly);
        }
        let old = self.items.held[target];
        if old.sticky() {
            return Err(HeldBlocked::ItemSticky);
        }
        let st = &mut self.items;
        st.bag.slots[usize::from(slot)] = Item::default();
        st.held[target] = Item {
            flags: item.flags & !FLAG_MARK,
            ..item
        };
        if old.exists() {
            // The slot just freed takes the old held item, so the bag cannot be full here.
            st.bag.add(old);
        }
        st.bag.compact();
        Ok(())
    }

    /// `021740EC` (action `0x37`, Take): the held item of `member` goes to the bag, marker
    /// cleared, and the held slot is zeroed. Refusals: no held item, a full bag, a sticky held
    /// item.
    // shortcut: the messages and the `02175F54` bad-tile fix after the move are not modelled.
    pub fn take(&mut self, member: usize) -> Result<(), HeldBlocked> {
        let held = self
            .held_item(member)
            .filter(|h| h.exists())
            .ok_or(HeldBlocked::NoItem)?;
        if self.items.bag.slots.iter().all(|s| s.exists()) {
            return Err(HeldBlocked::BagFull);
        }
        if held.sticky() {
            return Err(HeldBlocked::ItemSticky);
        }
        let st = &mut self.items;
        st.bag.add(Item {
            flags: held.flags & !FLAG_MARK,
            ..held
        });
        st.held[member] = Item::default();
        Ok(())
    }

    /// Refuses a marker change when a sticky bag item carries the marker (`02174680` and
    /// `02174594`, which both abort on it before clearing anything).
    fn marker_free(&self) -> Result<(), HeldBlocked> {
        let blocked = self
            .items
            .bag
            .slots
            .iter()
            .any(|s| s.sticky() && s.flags & FLAG_MARK != 0);
        if blocked {
            Err(HeldBlocked::ItemSticky)
        } else {
            Ok(())
        }
    }

    /// Clears the marker from every bag slot.
    fn clear_markers(&mut self) {
        for s in &mut self.items.bag.slots {
            s.flags &= !FLAG_MARK;
        }
    }

    /// `02174680` (action `0x3C`, Set marker): marks bag slot `slot` and clears every other
    /// marker, so at most one item carries it. Refusals: no item in the slot; a sticky marked item
    /// anywhere in the bag.
    // shortcut: the three messages (`msg != 0`) are not modelled.
    pub fn set_marker(&mut self, slot: u8) -> Result<(), HeldBlocked> {
        let i = usize::from(slot);
        self.items
            .bag
            .slots
            .get(i)
            .filter(|s| s.exists())
            .ok_or(HeldBlocked::NoItem)?;
        self.marker_free()?;
        self.clear_markers();
        self.items.bag.slots[i].flags |= FLAG_MARK;
        Ok(())
    }

    /// `02174594` (action `0x3D`, Unset marker): clears the marker from the bag. Refused when a
    /// sticky marked item is in the bag.
    // shortcut: the sound and the message (`msg != 0`) are not modelled.
    pub fn unset_marker(&mut self) -> Result<(), HeldBlocked> {
        self.marker_free()?;
        self.clear_markers();
        Ok(())
    }
}

/// The item that [`GameState::swap_floor_item`] puts down for the one underfoot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapSource {
    /// Bag slot `n` (0-based; the game's selector is `n + 1`).
    Bag(u8),
    /// The leader's held item (selector `0x81`).
    Held,
}

/// The item spawn mark in a tile's spawn word (`puVar7[1] & 2` in `02144714`).
const SPAWN_ITEM: u16 = 0x2;

/// `02144714`: the floor items of a floor just generated. `numItems` is set to 0, then
/// `DungeonRandInt(0x38)` (x0) and `DungeonRandInt(0x20)` (y0) are drawn. The scan runs row by
/// row, y from y0 + 1 wrapping through 0x20 rows, and in each row x from x0 + 1 wrapping through
/// 0x38 columns. A tile without the stairs bit (terrain `0x200`) and with the item mark takes its
/// picker row (the shop bit `0x20` first: row 1; else terrain type 0: row 3; else the monster
/// house bit `0x40`: row 2; else 0), the pick ([`pick_floor_item`]), then the item's own draws
/// ([`new_floor_item`]). A shop tile marks the item as shop stock (flag `0x02`) when `0200747C`
/// holds. At most [`FLOOR_ITEM_SLOTS`] items are laid; a mark past the 64th makes none (its
/// draws still happen).
// shortcut: `0200747C` also reads `0200813C` and `02008154`, which are not read: every id but
// 0, Poké and `0x77` to `0x7A` counts as stock.
pub(crate) fn spawn_floor_items(
    floor: &Floor,
    tables: &ItemTables,
    rng: &mut DungeonRng,
) -> ItemState {
    let mut state = ItemState::new(Bag::default(), tables.clone());
    let (w, h) = (floor::WIDTH as i32, floor::HEIGHT as i32);
    let x0 = rng.int(w);
    let y0 = rng.int(h);
    for dy in 1..=h {
        let y = (y0 + dy) % h;
        for dx in 1..=w {
            let x = (x0 + dx) % w;
            let Some(tile) = floor.tile(x, y) else {
                continue;
            };
            if tile.terrain & floor::TERRAIN_STAIRS != 0
                || tile.spawn_or_visibility & SPAWN_ITEM == 0
            {
                continue;
            }
            let shop = tile.terrain & floor::TERRAIN_SHOP != 0;
            let row = if shop {
                1
            } else if tile.terrain & floor::TERRAIN_TYPE_MASK == 0 {
                3
            } else if tile.terrain & floor::TERRAIN_MONSTER_HOUSE != 0 {
                2
            } else {
                0
            };
            let id = pick_floor_item(tables, row, rng);
            let mut item = new_floor_item(tables, id, rng);
            if shop && !matches!(id, 0 | POKE | 0x77..=0x7A) {
                item.flags |= FLAG_SHOP;
            }
            state.spawn((x as i16, y as i16), item);
        }
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
    fn picker_hits_when_the_roll_equals_a_weight_and_skips_zero_weights() {
        let mut probe = DungeonRng::new(21);
        let r1 = probe.int(0x2710);
        let r2 = probe.int(0x2710);
        let w = |r: i32| u16::try_from(r).unwrap();
        let mut cats = vec![0u16; PICKER_CATEGORIES];
        cats[2] = w(r1); // equal to the roll: a hit
        let mut items = vec![0u16; PICKER_ITEMS];
        items[0x20] = w(r2); // equal to the roll: a hit
        let mut tables = ItemTables {
            categories: vec![0; 256],
            ..ItemTables::default()
        };
        tables.categories[0x20] = 2;
        tables.rows.push(PickerRow {
            categories: cats.clone(),
            items: items.clone(),
        });
        assert_eq!(pick_floor_item(&tables, 0, &mut DungeonRng::new(21)), 0x20);
        // One below the roll misses (and draws no second roll).
        tables.rows[0].categories[2] = w(r1 - 1);
        let mut a = DungeonRng::new(21);
        let mut b = a;
        assert_eq!(pick_floor_item(&tables, 0, &mut a), POKE);
        b.int(0x2710);
        assert_eq!(a.state(), b.state());
        // A zero weight never hits, even with a roll of 0 or more.
        tables.rows[0].categories = vec![0; PICKER_CATEGORIES];
        assert_eq!(pick_floor_item(&tables, 0, &mut DungeonRng::new(21)), POKE);
        // An item weight below the roll misses: Poké, after both draws.
        tables.rows[0].categories = cats;
        tables.rows[0].items[0x20] = w(r2 - 1);
        let mut c = DungeonRng::new(21);
        assert_eq!(pick_floor_item(&tables, 0, &mut c), POKE);
        assert_eq!(c.state(), probe.state());
    }

    #[test]
    fn floor_items_come_from_the_marks_scanning_rows_from_the_two_start_draws() {
        let mut floor = open_floor();
        let marks = [(10, 3), (3, 8), (3, 2), (50, 2)];
        for (x, y) in marks {
            floor.tile_mut(x, y).unwrap().spawn_or_visibility |= 0x2;
        }
        // The stairs bit rules a marked tile out.
        floor.tile_mut(50, 2).unwrap().terrain |= floor::TERRAIN_STAIRS;
        let tables = ItemTables::default();
        let mut rng = DungeonRng::new(5);
        let mut by_hand = DungeonRng::new(5);
        let (x0, y0) = (by_hand.int(0x38), by_hand.int(0x20));
        let st = spawn_floor_items(&floor, &tables, &mut rng);
        // Slots follow the scan: y from y0 + 1 with wrap, x from x0 + 1 with wrap in each row.
        let mut expect = Vec::new();
        for dy in 1..=0x20 {
            for dx in 1..=0x38 {
                let p = ((x0 + dx) % 0x38, (y0 + dy) % 0x20);
                if marks[..3].contains(&p) {
                    expect.push((p.0 as i16, p.1 as i16));
                }
            }
        }
        let at: Vec<_> = st.floor.iter().flatten().map(|f| f.pos).collect();
        assert_eq!(at, expect);
        assert_eq!(at.len(), 3);
        assert_eq!(st.num_items, 3);
        // Default tables: a Poké item per mark, with the pick roll and the quantity roll.
        assert!(st.floor.iter().flatten().all(|f| f.item.id == POKE));
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
        floor.tile_mut(5, 1).unwrap().terrain = floor::TERRAIN_SHOP; // the shop bit before type 0
        for x in 1..=5 {
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
        let mut ids: Vec<_> = st
            .floor
            .iter()
            .flatten()
            .map(|f| (f.pos.0, f.item.id, f.item.flags & FLAG_SHOP != 0))
            .collect();
        ids.sort_unstable();
        // The shop tiles' stock carries the shop flag.
        assert_eq!(
            ids,
            vec![
                (1, 13, false),
                (2, 11, true),
                (3, 12, false),
                (4, 10, false),
                (5, 11, true)
            ]
        );
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

    #[test]
    fn a_floor_item_draws_sticky_before_the_quantity_and_only_for_stickable_ids() {
        let mut tables = ItemTables {
            sticky_chance: 100,
            quantity_table: vec![0; 100],
            floor_level: 1,
            categories: vec![0; 256],
            ..ItemTables::default()
        };
        tables.categories[usize::from(POKE)] = 6;
        // A plain item: one sticky draw, always sticky at 100 percent.
        let mut rng = DungeonRng::new(3);
        let mut by_hand = rng;
        let item = new_floor_item(&tables, 0x40, &mut rng);
        by_hand.int(100);
        assert!(item.sticky());
        assert_eq!(rng.state(), by_hand.state());
        // Poké and 0x77 to 0x7A: no sticky draw.
        for id in [POKE, 0x77, 0x78, 0x79, 0x7A] {
            let mut r = DungeonRng::new(3);
            let item = new_floor_item(&tables, id, &mut r);
            assert!(!item.sticky());
            let mut h = DungeonRng::new(3);
            if id == POKE {
                h.int(100);
            }
            assert_eq!(r.state(), h.state(), "id {id:#x}");
        }
        // A category 6 item that can be sticky draws the sticky roll first, then the quantity.
        tables.categories[0x50] = 6;
        let mut r = DungeonRng::new(3);
        let item = new_floor_item(&tables, 0x50, &mut r);
        let mut h = DungeonRng::new(3);
        h.int(100);
        h.int(100);
        assert!(item.sticky());
        assert_eq!(r.state(), h.state());
    }

    #[test]
    fn the_money_quantity_halves_the_index_until_the_table_fits_the_limit() {
        let mut table = vec![1000; 100];
        let roll = DungeonRng::new(9).int(100);
        // The table fits (<= level * 0x28 = 40) only at index roll / 4 (two halvings, when
        // roll >= 4).
        let want = roll / 4;
        table[usize::try_from(want).unwrap()] = 40;
        let tables = ItemTables {
            quantity_table: table,
            floor_level: 1,
            ..ItemTables::default()
        };
        let mut rng = DungeonRng::new(9);
        // The roll's own entry and its first halving are 1000, so the pick is `want` unless an
        // earlier step in the chain fits; the chain is roll, roll/2, roll/4.
        let q = money_quantity(&tables, &mut rng);
        let mut chain = roll;
        let mut expect = 1;
        for _ in 0..200 {
            if tables.quantity_table[usize::try_from(chain).unwrap()] <= 40 {
                expect = u8::try_from(chain).unwrap();
                break;
            }
            chain /= 2;
        }
        assert_eq!(q, expect);
        // No entry ever fits: 200 misses give 1, with only the one draw.
        let none = ItemTables {
            quantity_table: vec![1000; 100],
            floor_level: 1,
            ..ItemTables::default()
        };
        let mut r = DungeonRng::new(9);
        assert_eq!(money_quantity(&none, &mut r), 1);
        let mut h = DungeonRng::new(9);
        h.int(100);
        assert_eq!(r.state(), h.state());
        // An entry equal to the limit fits (the test is <=).
        let at_limit = ItemTables {
            quantity_table: vec![80; 100],
            floor_level: 2,
            ..ItemTables::default()
        };
        assert_eq!(
            i32::from(money_quantity(&at_limit, &mut DungeonRng::new(9))),
            roll
        );
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

    fn sticky(id: u8, qty: u8) -> Item {
        Item {
            flags: FLAG_EXISTS | FLAG_STICKY,
            qty,
            id,
        }
    }

    #[test]
    fn only_ids_below_9_stack() {
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(9, 1);
        lay(&mut g, (5, 5), Item::new(9, 1));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
        assert_eq!(g.dungeon().unwrap().bag().slots[1].id, 9);
    }

    #[test]
    fn a_shop_stock_floor_item_or_slot_never_stacks() {
        // The floor item is stock: the pickup stops at the shop refusal.
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(3, 5);
        lay(
            &mut g,
            (5, 5),
            Item {
                flags: FLAG_EXISTS | FLAG_SHOP,
                qty: 2,
                id: 3,
            },
        );
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::ShopStock);
        // A bag slot with the shop flag is not a candidate.
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_SHOP,
            qty: 5,
            id: 3,
        };
        lay(&mut g, (5, 5), Item::new(3, 2));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
        assert_eq!(g.dungeon().unwrap().bag().slots[0].qty, 5);
        assert_eq!(g.dungeon().unwrap().bag().slots[1], Item::new(3, 2));
    }

    #[test]
    fn stacking_prefers_the_same_sticky_bit_and_falls_back_to_any() {
        // Pass 1: the matching sticky bit wins over a fuller plain stack.
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(3, 50);
            d.items.bag.slots[1] = sticky(3, 10);
        }
        lay(&mut g, (5, 5), sticky(3, 4));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Stacked);
        let d = g.dungeon().unwrap();
        assert_eq!(d.bag().slots[1], sticky(3, 14));
        assert_eq!(d.bag().slots[0].qty, 50);
        // Pass 2: a sticky floor item stacks onto a plain slot, which turns sticky.
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(3, 5);
        lay(&mut g, (5, 5), sticky(3, 2));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Stacked);
        assert_eq!(g.dungeon().unwrap().bag().slots[0], sticky(3, 7));
        // And a plain floor item onto a sticky slot, which stays sticky.
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = sticky(3, 5);
        lay(&mut g, (5, 5), Item::new(3, 2));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Stacked);
        assert_eq!(g.dungeon().unwrap().bag().slots[0], sticky(3, 7));
    }

    #[test]
    fn a_tie_goes_to_the_first_slot() {
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(3, 20);
            d.items.bag.slots[1] = Item::new(3, 20);
        }
        lay(&mut g, (5, 5), Item::new(3, 1));
        dungeon(&mut g).pickup(0);
        let d = g.dungeon().unwrap();
        assert_eq!((d.bag().slots[0].qty, d.bag().slots[1].qty), (21, 20));
    }

    #[test]
    fn with_the_bag_in_use_the_held_slot_is_not_a_candidate() {
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.held[0] = Item::new(2, 50);
            d.items.bag.slots[0] = Item::new(2, 20);
        }
        lay(&mut g, (5, 5), Item::new(2, 3));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Stacked);
        assert_eq!(g.dungeon().unwrap().held_item(0).unwrap().qty, 50);
        assert_eq!(g.dungeon().unwrap().bag().slots[0].qty, 23);
        // With the bag full of other items the pickup does not fall back to the held slot.
        let mut g = run();
        for i in 0..BAG_SLOTS {
            dungeon(&mut g).items.bag.slots[i] = Item::new(0x50, 1);
        }
        dungeon(&mut g).items.held[0] = Item::default();
        lay(&mut g, (5, 5), Item::new(0x40, 1));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Full);
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
    fn an_ally_with_a_full_bag_picks_up_into_the_held_slot() {
        let mut g = run();
        lay(&mut g, (9, 9), Item::new(0x40, 1));
        for i in 0..BAG_SLOTS {
            dungeon(&mut g).items.bag.slots[i] = Item::new(0x50, 1);
        }
        assert_eq!(dungeon(&mut g).ally_pickup(1), Pickup::Placed);
        let d = g.dungeon().unwrap();
        assert_eq!(d.held_item(1), Some(Item::new(0x40, 1)));
    }

    #[test]
    fn an_ally_stack_merges_into_the_held_slot_when_it_is_fullest() {
        let mut g = run();
        lay(&mut g, (9, 9), Item::new(3, 2));
        {
            let d = dungeon(&mut g);
            d.items.bag.slots[0] = Item::new(3, 4);
            d.items.held[1] = Item::new(3, 9);
        }
        assert_eq!(dungeon(&mut g).ally_pickup(1), Pickup::Stacked);
        let d = g.dungeon().unwrap();
        assert_eq!(d.held_item(1), Some(Item::new(3, 11)));
        assert_eq!(d.bag().slots[0], Item::new(3, 4));
    }

    #[test]
    fn an_ally_without_an_inventory_has_the_held_slot_alone() {
        let mut g = run();
        lay(&mut g, (9, 9), Item::new(0x40, 1));
        dungeon(&mut g).items.tables.has_inventory = false;
        assert_eq!(dungeon(&mut g).ally_pickup(1), Pickup::Placed);
        let d = g.dungeon().unwrap();
        assert_eq!(d.held_item(1), Some(Item::new(0x40, 1)));
        assert!(!d.bag().slots[0].exists());
    }

    #[test]
    fn a_run_does_not_pick_up_but_a_move_does() {
        // `021441D8` tests `info+0x46 == 0` (a run) first and only shows a message.
        let mut g = run();
        lay(&mut g, (6, 5), Item::new(0x40, 1));
        g.step(crate::command::Command::Run(
            crate::command::Direction::Right,
        ));
        let d = g.dungeon().unwrap();
        assert!(!d.bag().slots[0].exists());
        assert!(d.floor_item_at((6, 5)).is_some());
        let mut g = run();
        lay(&mut g, (6, 5), Item::new(0x40, 1));
        g.step(crate::command::Command::Move(
            crate::command::Direction::Right,
        ));
        assert_eq!(g.dungeon().unwrap().bag().slots[0], Item::new(0x40, 1));
    }

    #[test]
    fn money_is_category_6_not_the_id() {
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.tables.categories = vec![0; 256];
            d.items.tables.categories[0x60] = 6;
        }
        lay(&mut g, (5, 5), Item::new(0x60, 12));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Money(12));
        assert!(!g.dungeon().unwrap().bag().slots[0].exists());
        // Poké with a table that says otherwise is an ordinary item.
        let mut g = run();
        {
            let d = dungeon(&mut g);
            d.items.tables.categories = vec![0; 256];
        }
        lay(&mut g, (5, 5), Item::new(POKE, 12));
        assert_eq!(dungeon(&mut g).pickup(0), Pickup::Placed);
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
        assert_eq!(dungeon(&mut g).ally_pickup(1), Pickup::RunAway);
        // At exactly half HP the ability does not trigger (strictly below).
        dungeon(&mut g).team[1].info.hp = 15;
        assert_eq!(dungeon(&mut g).ally_pickup(1), Pickup::Placed);
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
    fn swap_puts_the_bag_item_down_and_takes_the_floor_item_into_the_bag() {
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
            d.items.bag.slots[2] = Item::new(0x12, 1);
        }
        lay(&mut g, (5, 5), Item::new(0x30, 1));
        assert_eq!(g.swap_floor_item(SwapSource::Bag(1)), Outcome::Applied);
        let d = g.dungeon().unwrap();
        // The marker is gone from the item put down; the floor item took the freed slot.
        assert_eq!(d.floor_item_at((5, 5)), Some(Item::new(0x11, 1)));
        assert_eq!(d.bag().slots[1], Item::new(0x30, 1));
        assert_eq!(d.bag().slots[0].id, 0x10);
        assert_eq!(d.bag().slots[2].id, 0x12);
        assert_eq!(d.num_items(), 1);
        assert_eq!(d.turn(), 1);
    }

    #[test]
    fn swap_with_the_held_item_puts_the_floor_item_in_the_held_slot() {
        use crate::command::Outcome;
        let mut g = run();
        dungeon(&mut g).items.held[0] = Item::new(0x12, 1);
        dungeon(&mut g).items.bag.slots[0] = Item::new(0x10, 1);
        lay(
            &mut g,
            (5, 5),
            Item {
                flags: FLAG_EXISTS | FLAG_MARK,
                qty: 1,
                id: 0x30,
            },
        );
        assert_eq!(g.swap_floor_item(SwapSource::Held), Outcome::Applied);
        let d = g.dungeon().unwrap();
        assert_eq!(d.held_item(0), Some(Item::new(0x30, 1)));
        assert_eq!(d.floor_item_at((5, 5)), Some(Item::new(0x12, 1)));
        assert_eq!(d.bag().slots[0], Item::new(0x10, 1));
    }

    #[test]
    fn swap_refusals() {
        use crate::command::{Blocked, Outcome};
        let mut g = run();
        dungeon(&mut g).items.bag.slots[0] = Item::new(0x10, 1);
        // Nothing underfoot.
        assert_eq!(
            g.swap_floor_item(SwapSource::Bag(0)),
            Outcome::Blocked(Blocked::NoItem)
        );
        lay(&mut g, (5, 5), Item::new(0x30, 1));
        // An empty bag slot or an empty held slot.
        assert_eq!(
            g.swap_floor_item(SwapSource::Bag(1)),
            Outcome::Blocked(Blocked::NoItem)
        );
        assert_eq!(
            g.swap_floor_item(SwapSource::Held),
            Outcome::Blocked(Blocked::NoItem)
        );
        // A bag item that is sticky and marked.
        dungeon(&mut g).items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_STICKY | FLAG_MARK,
            qty: 1,
            id: 0x10,
        };
        assert_eq!(
            g.swap_floor_item(SwapSource::Bag(0)),
            Outcome::Blocked(Blocked::ItemSticky)
        );
        // A sticky bag item without the marker swaps.
        dungeon(&mut g).items.bag.slots[0].flags = FLAG_EXISTS | FLAG_STICKY;
        // A sticky held item is refused.
        dungeon(&mut g).items.held[0] = sticky(0x13, 1);
        assert_eq!(
            g.swap_floor_item(SwapSource::Held),
            Outcome::Blocked(Blocked::ItemSticky)
        );
        assert_eq!(g.dungeon().unwrap().turn(), 0);
        assert_eq!(g.swap_floor_item(SwapSource::Bag(0)), Outcome::Applied);
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

    #[test]
    fn has_held_item_counts_only_an_existing_non_sticky_item_of_the_id() {
        use crate::items::{FLAG_EXISTS, FLAG_STICKY};
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.held[1] = Item::new(0x15, 1);
        assert!(d.has_held_item(1, 0x15));
        assert!(!d.has_held_item(1, 0x16), "another id");
        assert!(!d.has_held_item(0, 0x15), "an empty slot");
        assert!(!d.has_held_item(9, 0x15), "no such member");
        d.items.held[1].flags = FLAG_EXISTS | FLAG_STICKY;
        assert!(!d.has_held_item(1, 0x15), "a sticky item does not count");
        d.items.held[1] = Item {
            flags: 0,
            qty: 1,
            id: 0x15,
        };
        assert!(!d.has_held_item(1, 0x15), "a slot without the exists bit");
    }

    #[test]
    fn give_into_an_empty_held_slot_clears_the_marker() {
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_MARK,
            qty: 1,
            id: 0x30,
        };
        d.items.bag.slots[1] = Item::new(0x31, 1);
        assert_eq!(d.give(0, 1), Ok(()));
        assert_eq!(d.held_item(1), Some(Item::new(0x30, 1)));
        assert_eq!(d.bag().slots[0], Item::new(0x31, 1), "the bag closes up");
        assert_eq!(d.bag().slots[1], Item::default());
    }

    #[test]
    fn give_swaps_with_a_held_item_and_the_old_one_goes_to_the_bag() {
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.held[1] = Item::new(0x40, 2);
        d.items.bag.slots[0] = Item::new(0x41, 1);
        assert_eq!(d.give(0, 1), Ok(()));
        assert_eq!(d.held_item(1), Some(Item::new(0x41, 1)));
        assert_eq!(d.bag().slots[0], Item::new(0x40, 2));
    }

    #[test]
    fn give_refusals_change_nothing() {
        let mut g = run();
        let d = dungeon(&mut g);
        // A sticky marked bag item.
        d.items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_STICKY | FLAG_MARK,
            qty: 1,
            id: 0x30,
        };
        assert_eq!(d.give(0, 1), Err(HeldBlocked::ItemSticky));
        assert_eq!(d.give(3, 1), Err(HeldBlocked::NoItem), "an empty slot");
        d.items.bag.slots[0] = Item::new(0x30, 1);
        assert_eq!(
            d.give(0, 0),
            Err(HeldBlocked::NoAlly),
            "the leader is not an ally"
        );
        assert_eq!(d.give(0, 9), Err(HeldBlocked::NoAlly), "no such member");
        // A sticky held item of the target: a message in the game, nothing moves.
        d.items.held[1] = sticky(0x50, 1);
        assert_eq!(d.give(0, 1), Err(HeldBlocked::ItemSticky));
        assert_eq!(d.held_item(1), Some(sticky(0x50, 1)));
        assert_eq!(d.bag().slots[0], Item::new(0x30, 1));
    }

    #[test]
    fn take_moves_the_held_item_to_the_bag_without_its_marker() {
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.held[0] = Item::new(0x60, 1);
        assert_eq!(d.take(0), Ok(()));
        assert!(!d.held_item(0).unwrap().exists());
        assert_eq!(d.bag().slots[0], Item::new(0x60, 1));
    }

    #[test]
    fn take_refusals_change_nothing() {
        let mut g = run();
        let d = dungeon(&mut g);
        assert_eq!(d.take(0), Err(HeldBlocked::NoItem), "no held item");
        d.items.held[0] = sticky(0x61, 1);
        assert_eq!(d.take(0), Err(HeldBlocked::ItemSticky));
        assert_eq!(d.held_item(0), Some(sticky(0x61, 1)));
        d.items.held[0] = Item::new(0x62, 1);
        for slot in &mut d.items.bag.slots {
            *slot = Item::new(0x63, 1);
        }
        assert_eq!(d.take(0), Err(HeldBlocked::BagFull));
        assert_eq!(d.held_item(0), Some(Item::new(0x62, 1)));
    }

    #[test]
    fn set_marker_marks_one_item_and_clears_the_others() {
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_MARK,
            qty: 1,
            id: 0x70,
        };
        d.items.bag.slots[2] = Item::new(0x71, 1);
        assert_eq!(d.set_marker(2), Ok(()));
        assert_eq!(d.bag().slots[0].flags & FLAG_MARK, 0);
        assert_eq!(d.bag().slots[2].flags & FLAG_MARK, FLAG_MARK);
        assert_eq!(d.set_marker(5), Err(HeldBlocked::NoItem), "an empty slot");
    }

    #[test]
    fn a_sticky_marked_bag_item_blocks_both_marker_changes() {
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_STICKY | FLAG_MARK,
            qty: 1,
            id: 0x72,
        };
        d.items.bag.slots[1] = Item::new(0x73, 1);
        assert_eq!(d.set_marker(1), Err(HeldBlocked::ItemSticky));
        assert_eq!(d.unset_marker(), Err(HeldBlocked::ItemSticky));
        // Nothing was cleared or set.
        assert_eq!(d.bag().slots[0].flags & FLAG_MARK, FLAG_MARK);
        assert_eq!(d.bag().slots[1].flags & FLAG_MARK, 0);
    }

    #[test]
    fn unset_marker_clears_every_marker() {
        let mut g = run();
        let d = dungeon(&mut g);
        d.items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_MARK,
            qty: 1,
            id: 0x74,
        };
        assert_eq!(d.unset_marker(), Ok(()));
        assert_eq!(d.bag().slots[0], Item::new(0x74, 1));
    }
}
