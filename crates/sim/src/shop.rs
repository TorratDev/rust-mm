//! Kecleon shops (`docs/re/items.md`, P6; #554): the stock on the shop tiles and its totals, buy,
//! sell and pay, leaving with debt, and the hostile shopkeeper.
//!
//! The totals are counters on the item state (`+0x67C` stock, `+0x680` credit, `+0x684`
//! baseline). Floor-item spawn and removal on a shop tile move them ([`ItemState::spawn_on`]).
//! Debt is `baseline - stock`. The game's yes/no prompts are not interactive here: the answer
//! the leave sequence uses is a setting ([`GameState::set_shop_answer`]).
// shortcut: no shopkeeper is spawned at the shop's centre (the floor generator's `pair_c` call is
// a no-op); a caller places one with `GameState::place_shopkeeper`.
// shortcut: the leave sequence takes any shopkeeper on the floor as the one in the room, and the
// warp and push paths into and out of the shop are not modelled, only the leader's steps.
// shortcut: the shop state (keepers, stolen flag, counters) is not in `DungeonSnapshot`, as the
// floor items are not either.

use crate::command::{Blocked, Outcome};
use crate::floor::TERRAIN_SHOP;
use crate::items::{FLAG_SHOP, Item, ItemState, POKE, Pickup, STACKABLE_BELOW};
use crate::state::{Dungeon, GameState, Monster};
use crate::status::NO_END;

/// The Kecleon's species id (`0x17C`): the shopkeeper's, and the thief's after it turns hostile.
pub const KECLEON: u16 = 0x17C;
/// `PendingMenuChoice` for the ground menu of the leader's tile (`gDungeon+0x5B8 = 4`).
pub const GROUND_MENU: u8 = 4;

/// The shop totals (`+0x67C`, `+0x680`, `+0x684`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShopTotals {
    /// `+0x67C`: the buy prices of the stock on the shop tiles.
    pub stock: i32,
    /// `+0x680`: the sell prices of the player's own items on the shop tiles.
    pub credit: i32,
    /// `+0x684`: the stock the shop started with, grown by each sale.
    pub baseline: i32,
}

impl ShopTotals {
    /// The debt: what the player owes for stock it took (`+0x684 - +0x67C`).
    #[must_use]
    pub const fn debt(self) -> i32 {
        self.baseline - self.stock
    }
}

/// The answer of a settlement, as the game's result codes (`02195E18`, `02196084`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settle {
    /// Nothing was owed, or the deal was done (0).
    Ok,
    /// The player said no (1).
    Declined,
    /// The money is below the debt (2).
    Short,
}

/// The shop state apart from the totals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopState {
    /// `+0x65E`: the leader stole from the shop. `+0x65F`, set with it, is not kept apart.
    pub(crate) stolen: bool,
    /// The shopkeepers (`info+8 == 1`), by position.
    pub(crate) keepers: Vec<(i16, i16)>,
    /// `+0x688`: the leader stands in the shop room.
    pub(crate) leader_in: bool,
    /// `+0x689`: the leader stood in the shop room before its last move.
    pub(crate) leader_was_in: bool,
    /// `PendingMenuChoice` (`gDungeon+0x5B8`), `None` for -1. The next input clears it.
    pub(crate) pending_menu: Option<u8>,
    /// The answer to the leave-with-debt prompts.
    pub(crate) pay_answer: bool,
}

impl Default for ShopState {
    fn default() -> Self {
        Self {
            stolen: false,
            keepers: Vec::new(),
            leader_in: false,
            leader_was_in: false,
            pending_menu: None,
            pay_answer: true,
        }
    }
}

/// Whether an item id is shop stock when it lies on a shop tile (the test of `0200747C`).
const fn is_stock_id(id: u8) -> bool {
    !matches!(id, 0 | POKE | 0x77..=0x7A)
}

/// The price of `item` from a price table: the unit price, times the quantity for a stacking id.
/// An id past the table costs 0.
fn price(prices: &[u32], item: Item) -> i32 {
    let unit = i64::from(prices.get(usize::from(item.id)).copied().unwrap_or(0));
    let count = if item.id < STACKABLE_BELOW {
        i64::from(item.qty)
    } else {
        1
    };
    i32::try_from(unit * count).unwrap_or(i32::MAX)
}

impl ItemState {
    /// Adds (`sign` 1) or takes away (`-1`) the shop total of an item on a shop tile: stock
    /// (buy price) when the item is stock, else own credit (sell price). `02143F98`, `021440B0`.
    pub(crate) fn shop_account(&mut self, item: Item, sign: i32) {
        if item.flags & FLAG_SHOP != 0 {
            self.shop.stock += sign * price(&self.tables.buy_prices, item);
        } else {
            self.shop.credit += sign * price(&self.tables.sell_prices, item);
        }
    }
}

impl Dungeon {
    /// Whether `pos` is a shop tile (terrain bit `0x20`).
    #[must_use]
    pub(crate) fn tile_is_shop(&self, pos: (i16, i16)) -> bool {
        self.floor
            .tile(i32::from(pos.0), i32::from(pos.1))
            .is_some_and(|t| t.terrain & TERRAIN_SHOP != 0)
    }

    /// The shop totals.
    #[must_use]
    pub fn shop_totals(&self) -> ShopTotals {
        self.items.shop
    }

    /// `PendingMenuChoice`: 4 after the leader stepped on shop stock (the ground menu).
    #[must_use]
    pub fn pending_menu_choice(&self) -> Option<u8> {
        self.shop.pending_menu
    }

    /// Whether the leader stole from the shop (`+0x65E`).
    #[must_use]
    pub fn shop_stolen(&self) -> bool {
        self.shop.stolen
    }

    /// Whether `pos` holds a shopkeeper that still passes its turn (`+0x65E == 0`).
    pub(crate) fn keepers_at(&self, pos: (i16, i16)) -> bool {
        !self.shop.stolen && self.shop.keepers.contains(&pos)
    }

    /// The step onto a shop item's tile (`02165B00`): stock opens the ground menu. Returns
    /// whether it did.
    pub(crate) fn shop_stepped_on(&mut self, pos: (i16, i16)) -> bool {
        let stock = self
            .floor_item_at(pos)
            .is_some_and(|f| f.flags & FLAG_SHOP != 0);
        if stock {
            self.shop.pending_menu = Some(GROUND_MENU);
        }
        stock
    }

    /// Whether the buy can take the stock under the leader: the ground menu is open, the tile
    /// holds stock, and the bag takes it.
    pub(crate) fn shop_buy_check(&self, menu: Option<u8>) -> Result<(), Blocked> {
        if menu != Some(GROUND_MENU) {
            return Err(Blocked::NoStock);
        }
        let leader = self.team.first().ok_or(Blocked::NoLeader)?;
        let stock = self
            .items
            .slot_at(leader.pos)
            .and_then(|s| self.items.floor[s])
            .is_some_and(|f| f.item.flags & FLAG_SHOP != 0);
        if !stock {
            return Err(Blocked::NoStock);
        }
        let mut probe = self.clone();
        if probe.pickup_from(0, false, true) == Pickup::Full {
            return Err(Blocked::BagFull);
        }
        Ok(())
    }

    /// The buy: the leader takes the stock from its tile (the stock comes off the totals and
    /// keeps its flag, so it stays owed until paid).
    pub(crate) fn shop_buy(&mut self) {
        self.pickup_from(0, false, true);
    }

    /// The sell prompt (`02196084`): with credit, a yes pays the credit to the money, and each
    /// own item on a shop tile becomes stock. Without credit nothing is asked.
    pub(crate) fn shop_sell(&mut self, yes: bool) -> Settle {
        let credit = self.items.shop.credit;
        if credit == 0 {
            return Settle::Ok;
        }
        if !yes {
            return Settle::Declined;
        }
        self.items.bag.add_money(credit);
        let before = self.items.shop.stock;
        let own: Vec<usize> = (0..self.items.floor.len())
            .filter(|&s| {
                self.items.floor[s]
                    .is_some_and(|f| f.item.flags & FLAG_SHOP == 0 && self.tile_is_shop(f.pos))
            })
            .collect();
        for slot in own {
            let Some(f) = self.items.floor[slot] else {
                continue;
            };
            self.items.remove_on(slot, true);
            let mut item = f.item;
            if is_stock_id(item.id) {
                item.flags |= FLAG_SHOP;
            }
            self.items.spawn_on(f.pos, item, true);
        }
        self.items.shop.baseline += self.items.shop.stock - before;
        Settle::Ok
    }

    /// The pay prompt (`02195E18`): with debt, a yes needs the money to cover it. Then the money
    /// goes, the stock flags of the items outside the shop, the bag and the held items clear,
    /// and the counters settle (credit 0, the baseline equal to the stock still on the shop).
    pub(crate) fn shop_pay(&mut self, yes: bool) -> Settle {
        let debt = self.items.shop.debt();
        if debt <= 0 {
            return Settle::Ok;
        }
        if !yes {
            return Settle::Declined;
        }
        if self.items.bag.money < debt {
            return Settle::Short;
        }
        self.items.bag.add_money(-debt);
        for slot in 0..self.items.floor.len() {
            let outside = self.items.floor[slot].is_some_and(|f| !self.tile_is_shop(f.pos));
            if let (true, Some(f)) = (outside, self.items.floor[slot].as_mut()) {
                f.item.flags &= !FLAG_SHOP;
            }
        }
        for it in self
            .items
            .bag
            .slots
            .iter_mut()
            .chain(self.items.held.iter_mut())
        {
            it.flags &= !FLAG_SHOP;
        }
        self.items.shop.credit = 0;
        self.items.shop.baseline = self.items.shop.stock;
        Settle::Ok
    }

    /// `0219635C`, after the leader's move: the leader leaves the shop room with debt or
    /// credit. Sell, then pay; a declined or short result makes the shopkeeper hostile.
    pub(crate) fn shop_leader_moved(&mut self) {
        let now = self.team.first().is_some_and(|l| self.tile_is_shop(l.pos));
        self.shop.leader_was_in = std::mem::replace(&mut self.shop.leader_in, now);
        let was = self.shop.leader_was_in;
        let totals = self.items.shop;
        if self.shop.stolen || (totals.baseline == 0 && totals.credit == 0) {
            return;
        }
        if !(was && !now) || self.shop.keepers.is_empty() {
            return;
        }
        let yes = self.shop.pay_answer;
        if self.shop_sell(yes) != Settle::Ok || self.shop_pay(yes) != Settle::Ok {
            self.make_hostile();
        }
    }

    /// `MakeShopHostile` (`02195C88`): the leader stole. The stock flags clear on the floor, in
    /// the bag and on the held items, every shopkeeper stops being one, every Kecleon gets a
    /// speed stage up for good, and the totals reset.
    pub(crate) fn make_hostile(&mut self) {
        self.shop.stolen = true;
        self.shop.keepers.clear();
        for f in self.items.floor.iter_mut().flatten() {
            f.item.flags &= !FLAG_SHOP;
        }
        for it in self
            .items
            .bag
            .slots
            .iter_mut()
            .chain(self.items.held.iter_mut())
        {
            it.flags &= !FLAG_SHOP;
        }
        for m in self.wild.iter_mut().filter(|m| m.info.species == KECLEON) {
            // A speed-up counter that never ticks down: the `+0x65E` stage of `02181E58`.
            if let Some(c) = m.info.speed_up.iter_mut().find(|c| **c == 0) {
                *c = NO_END;
            }
        }
        self.items.shop = ShopTotals::default();
    }

    /// The leader switch is refused while the player owes stock or is owed for its own items
    /// (`021657A8`, `02196320`).
    // shortcut: the leader switch (`021657A8`) is not modelled, so nothing calls this yet.
    #[allow(dead_code)]
    pub(crate) fn shop_blocks_leader_switch(&self) -> bool {
        self.items.shop.debt() > 0 || self.items.shop.credit != 0
    }
}

impl GameState {
    /// Buys the stock under the leader from the ground menu (the menu opened by stepping on it).
    /// It takes the leader's turn. Refusals change nothing (see [`Blocked`]).
    pub fn buy_shop_stock(&mut self) -> Outcome {
        self.apply_input(crate::state::Input::Buy, None)
    }

    /// Answers the pay prompt: `Some` with the result, `None` with no dungeon.
    pub fn pay_shop_debt(&mut self, yes: bool) -> Option<Settle> {
        self.dungeon_mut().map(|d| d.shop_pay(yes))
    }

    /// Answers the sell prompt for the credit: `Some` with the result, `None` with no dungeon.
    pub fn sell_to_shop(&mut self, yes: bool) -> Option<Settle> {
        self.dungeon_mut().map(|d| d.shop_sell(yes))
    }

    /// The answer the leave-with-debt sequence gives to its prompts (the sim has no prompts).
    /// It starts as yes.
    pub fn set_shop_answer(&mut self, yes: bool) {
        if let Some(d) = self.dungeon_mut() {
            d.shop.pay_answer = yes;
        }
    }

    /// Marks the wild Kecleon at `pos` as a shopkeeper (`info+8 == 1`). It passes its turn
    /// until the leader steals. Returns false when no wild Kecleon stands there.
    pub fn place_shopkeeper(&mut self, pos: (i16, i16)) -> bool {
        let Some(d) = self.dungeon_mut() else {
            return false;
        };
        let is_kecleon = |m: &Monster| m.pos == pos && m.info.species == KECLEON;
        if !d.wild.iter().any(is_kecleon) {
            return false;
        }
        if !d.shop.keepers.contains(&pos) {
            d.shop.keepers.push(pos);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Direction};
    use crate::floor::{Floor, HEIGHT, TERRAIN_FLOOR, WIDTH};
    use crate::items::FLAG_EXISTS;
    use crate::items::ItemTables;
    use crate::state::{EntityInfo, Monster, MoveSlot};

    /// The item tables with buy and sell prices for the given ids.
    fn priced_tables(buy: &[(u8, u32)], sell: &[(u8, u32)]) -> ItemTables {
        let table = |prices: &[(u8, u32)]| {
            let mut v = vec![0u32; 0x80];
            for &(id, p) in prices {
                v[usize::from(id)] = p;
            }
            v
        };
        ItemTables {
            buy_prices: table(buy),
            sell_prices: table(sell),
            ..ItemTables::default()
        }
    }

    const STOCK_ID: u8 = 0x40;
    const BUY: u32 = 300;
    const SELL: u32 = 120;

    fn info(leader: bool, species: u16) -> EntityInfo {
        EntityInfo {
            species,
            apparent_species: species,
            not_team_member: !leader,
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

    /// Open floor with the shop tiles at x 5 and 6 on row 5, and the stock tile at (6, 5).
    fn shop_floor() -> Floor {
        let mut f = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                f.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        for x in [5, 6] {
            f.tile_mut(x, 5).unwrap().terrain = TERRAIN_FLOOR | TERRAIN_SHOP;
        }
        f
    }

    /// A run with the leader on (5, 5), a shopkeeper at (9, 9), and the stock priced.
    fn run() -> GameState {
        let mut g = GameState::new(7);
        let team = vec![Monster {
            pos: (5, 5),
            visible: true,
            info: info(true, 1),
        }];
        g.enter_dungeon(shop_floor(), team, 0, &[]).unwrap();
        g.set_speed_stage(crate::turn::MAX_SPEED_STAGE);
        let kecleon = Monster {
            pos: (9, 9),
            visible: true,
            info: info(false, KECLEON),
        };
        let dg = g.dungeon_mut().unwrap();
        dg.wild.push(kecleon);
        dg.items.tables = priced_tables(&[(STOCK_ID, BUY)], &[(0x41, SELL)]);
        assert!(g.place_shopkeeper((9, 9)));
        g
    }

    fn d(g: &mut GameState) -> &mut Dungeon {
        g.dungeon_mut().unwrap()
    }

    fn stock(id: u8) -> Item {
        Item {
            flags: FLAG_EXISTS | FLAG_SHOP,
            qty: 1,
            id,
        }
    }

    /// Lays an item. Stock laid here is the baseline too, as the floor setup makes it.
    fn lay(g: &mut GameState, pos: (i16, i16), item: Item, shop: bool) {
        let dg = d(g);
        dg.items.spawn_on(pos, item, shop).unwrap();
        if shop && item.flags & FLAG_SHOP != 0 {
            dg.items.shop.baseline = dg.items.shop.stock;
        }
    }

    #[test]
    fn stock_on_a_shop_tile_moves_the_totals_and_removing_it_undoes_them() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        assert_eq!(d(&mut g).shop_totals().stock, BUY as i32);
        // Own items on a shop tile count as credit; off the shop, nothing counts.
        lay(&mut g, (5, 5), Item::new(0x41, 1), true);
        lay(&mut g, (7, 7), stock(STOCK_ID), false);
        let t = d(&mut g).shop_totals();
        assert_eq!((t.stock, t.credit), (BUY as i32, SELL as i32));
        let slot = d(&mut g).items.slot_at((5, 5)).unwrap();
        d(&mut g).items.remove_on(slot, true);
        assert_eq!(d(&mut g).shop_totals().credit, 0);
        assert_eq!(d(&mut g).shop_totals().stock, BUY as i32);
    }

    #[test]
    fn stepping_on_stock_opens_the_ground_menu_and_keeps_the_item() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        g.step(Command::Move(Direction::Right));
        assert_eq!(
            g.dungeon().unwrap().pending_menu_choice(),
            Some(GROUND_MENU)
        );
        assert!(g.bag().slots.iter().all(|s| !s.exists()));
        assert!(g.dungeon().unwrap().floor_item_at((6, 5)).is_some());
        // Any other input closes the menu.
        g.step(Command::Wait);
        assert_eq!(g.dungeon().unwrap().pending_menu_choice(), None);
    }

    #[test]
    fn buy_and_pay_settle_the_debt_and_clear_the_flags() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        g.step(Command::Move(Direction::Right));
        // Buy from the ground menu: the item is in the bag, still stock, and the debt is owed.
        assert_eq!(g.buy_shop_stock(), Outcome::Applied);
        let debt = d(&mut g).shop_totals().debt();
        assert_eq!(debt, BUY as i32);
        assert_eq!(g.bag().slots[0], stock(STOCK_ID));
        assert!(g.dungeon().unwrap().floor_item_at((6, 5)).is_none());
        // Short of money: result 2, nothing changes.
        d(&mut g).items.bag.money = BUY as i32 - 1;
        assert_eq!(g.pay_shop_debt(true), Some(Settle::Short));
        assert_eq!(g.dungeon().unwrap().shop_totals().debt(), debt);
        // Enough money and a yes: the money goes, the bag flag clears, the debt is settled.
        d(&mut g).items.bag.money = 1_000;
        assert_eq!(g.pay_shop_debt(true), Some(Settle::Ok));
        assert_eq!(g.bag().money, 1_000 - BUY as i32);
        assert_eq!(g.bag().slots[0], Item::new(STOCK_ID, 1));
        assert_eq!(g.dungeon().unwrap().shop_totals().debt(), 0);
    }

    #[test]
    fn a_pay_declined_is_result_1_and_costs_nothing() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        g.step(Command::Move(Direction::Right));
        g.buy_shop_stock();
        d(&mut g).items.bag.money = 1_000;
        assert_eq!(g.pay_shop_debt(false), Some(Settle::Declined));
        assert_eq!(g.bag().money, 1_000);
        assert_eq!(g.dungeon().unwrap().shop_totals().debt(), BUY as i32);
    }

    #[test]
    fn leave_without_paying_makes_the_shopkeeper_hostile() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        g.step(Command::Move(Direction::Right));
        g.buy_shop_stock();
        // The leader walks out of the shop still owing: the answer is no.
        g.set_shop_answer(false);
        g.step(Command::Move(Direction::Right));
        let dg = g.dungeon().unwrap();
        assert!(dg.shop_stolen());
        assert_eq!(dg.shop_totals(), ShopTotals::default());
        // The bag's stock flag clears, and the shopkeeper stops passing its turn.
        assert_eq!(g.bag().slots[0], Item::new(STOCK_ID, 1));
        let keeper = g.dungeon().unwrap().wild().first().unwrap().clone();
        assert!(!g.dungeon().unwrap().keepers_at(keeper.pos));
        assert!(keeper.info.speed_up.contains(&NO_END));
        // Stock stepped on after that is picked up normally.
        lay(&mut g, (6, 5), Item::new(0x42, 1), false);
        g.step(Command::Move(Direction::Left));
        assert_eq!(g.dungeon().unwrap().pending_menu_choice(), None);
        assert!(g.bag().slots.iter().any(|s| s.id == 0x42));
    }

    #[test]
    fn leaving_with_money_pays_and_the_shopkeeper_stays_a_shopkeeper() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        g.step(Command::Move(Direction::Right));
        g.buy_shop_stock();
        d(&mut g).items.bag.money = 1_000;
        g.step(Command::Move(Direction::Right));
        let dg = g.dungeon().unwrap();
        assert!(!dg.shop_stolen());
        assert_eq!(g.bag().money, 1_000 - BUY as i32);
        assert_eq!(dg.shop_totals().debt(), 0);
    }

    #[test]
    fn a_shopkeeper_passes_its_turn_while_nothing_is_stolen() {
        let mut g = run();
        for _ in 0..12 {
            g.step(Command::Wait);
        }
        let keeper = g.dungeon().unwrap().wild().first().unwrap().pos;
        assert_eq!(keeper, (9, 9));
    }

    #[test]
    fn the_leader_switch_is_refused_with_debt_or_credit() {
        let mut g = run();
        lay(&mut g, (6, 5), stock(STOCK_ID), true);
        assert!(!d(&mut g).shop_blocks_leader_switch());
        // Taking the stock off the shop makes it debt.
        let slot = d(&mut g).items.slot_at((6, 5)).unwrap();
        d(&mut g).items.remove_on(slot, true);
        assert!(d(&mut g).shop_blocks_leader_switch());
        // Own items on a shop tile are credit.
        d(&mut g).items.shop = ShopTotals::default();
        lay(&mut g, (5, 5), Item::new(0x41, 1), true);
        assert!(d(&mut g).shop_blocks_leader_switch());
    }

    #[test]
    fn selling_turns_own_items_into_stock_and_pays_the_credit() {
        let mut g = run();
        lay(&mut g, (6, 5), Item::new(0x41, 1), true);
        let before = d(&mut g).shop_totals();
        assert_eq!(before.credit, SELL as i32);
        d(&mut g).items.bag.money = 0;
        assert_eq!(g.sell_to_shop(true), Some(Settle::Ok));
        let after = d(&mut g).shop_totals();
        assert_eq!(after.credit, 0);
        // The item is stock now; id 0x41 has no buy price here, so the stock stays 0 and the
        // debt does not move.
        assert_eq!(after.stock, 0);
        assert_eq!(after.debt(), before.debt());
        assert_eq!(g.bag().money, SELL as i32);
        assert!(
            d(&mut g)
                .floor_item_at((6, 5))
                .is_some_and(|i| i.flags & FLAG_SHOP != 0)
        );
    }
}
