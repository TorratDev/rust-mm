//! The dungeon's Items menu and the ground menu (#555): which entries each offers, the cursor,
//! and the action a choice asks for. Pure state: the app reads the sim through `Dungeon` and
//! carries the action out through the sim's item API (`app::item_action`).
//!
//! Entries follow `docs/re/items.md` (#465 runtime check: an item's submenu is Give, Place,
//! Throw, Info; Use or Eat comes first for an item that has a use), filtered as Blue's entry
//! builder `ov_0001::021693DC` does (see [`entries_for`]), and the ground menu of
//! `gDungeon+0x5B8 == 4` (`docs/re/items.md`, P6: the item sub-menu of the leader's tile).
// shortcut: no text is drawn on screen yet (as for the move menu), so the labels are for the log
// and the tests. The labels are English placeholders: the ROM's strings are not read.

use rescue_platform::{ButtonState, DsButton};
use rescue_sim::command::Direction;
use rescue_sim::items::{FLAG_SHOP, Item};
use rescue_sim::state::Dungeon;

use crate::input;

/// An entry of an item's submenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Use,
    Eat,
    Give,
    Place,
    Throw,
    Info,
}

impl Entry {
    /// The entry's text.
    pub fn label(self) -> &'static str {
        match self {
            Entry::Use => "Use",
            Entry::Eat => "Eat",
            Entry::Give => "Give",
            Entry::Place => "Place",
            Entry::Throw => "Throw",
            Entry::Info => "Info",
        }
    }
}

/// An entry of the ground menu of the tile underfoot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundEntry {
    /// Swap the item underfoot with a bag item.
    Swap,
    /// Buy shop stock.
    Buy,
    Info,
}

impl GroundEntry {
    pub fn label(self) -> &'static str {
        match self {
            GroundEntry::Swap => "Swap",
            GroundEntry::Buy => "Buy",
            GroundEntry::Info => "Info",
        }
    }
}

/// What the bag list is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFor {
    /// Choose an item to act on.
    Browse,
    /// Choose the item to swap with the one underfoot.
    Swap,
}

/// The menu on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemMenu {
    /// The bag, one row per item; `cursor` is a row, not a bag slot.
    List { cursor: usize, purpose: ListFor },
    /// The submenu of the bag slot `slot`.
    Actions {
        slot: u8,
        cursor: usize,
        entries: Vec<Entry>,
    },
    /// Waiting for the direction to throw the item of bag slot `slot` in.
    Aim { slot: u8 },
    /// The ground menu of the leader's tile.
    Ground {
        cursor: usize,
        entries: Vec<GroundEntry>,
    },
    /// Yes/No before the swap of the item underfoot with bag slot `slot`. It opens on No.
    Confirm { slot: u8, yes: bool },
}

/// What a choice asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Use or eat bag slot `n`.
    Use(u8),
    Throw(u8, Direction),
    /// Place (drop) bag slot `n` on the floor.
    Place(u8),
    /// Swap the item underfoot with bag slot `n`.
    Swap(u8),
    /// Show the description of an item id.
    Info(u8),
    /// Give bag slot `n` to a partner.
    // shortcut: the sim has no Give action yet (#552), so the app only says it is unavailable.
    Give(u8),
    /// Buy the shop stock underfoot.
    // shortcut: the shop is not in the sim yet (#554), so the app only says it is unavailable.
    Buy,
}

/// The bag slots that hold an item, in order: the rows of the list.
pub fn rows(run: &Dungeon) -> Vec<u8> {
    run.bag()
        .slots
        .iter()
        .enumerate()
        .filter(|(_, i)| i.exists())
        .map(|(n, _)| u8::try_from(n).unwrap_or(0))
        .collect()
}

/// The submenu of `item`: Use or Eat first for an item with a use, then Give, Place, Throw, Info.
/// Categories (`docs/re/items.md`, P1): 2 and 3 are eaten (berry, seed, vitamin; food, gummi);
/// 0, 1 (thrown) and 4 (held) have no use; the rest (TM, orb, link box, other) are used.
///
/// The builder `ov_0001::021693DC` drops entries: Give (0x36) when no team member other than the
/// leader can take it, Place (action 8) when the leader's tile already holds an object, and the
/// throw actions when `021803B8` says the leader's species cannot throw (`can_throw`).
// hypothesis: where the tile holds an item the builder adds 0x3A, probably a swap; it is not
// offered here (the ground menu's Swap covers it).
pub fn entries_for(run: &Dungeon, item: Item, can_throw: bool) -> Vec<Entry> {
    let mut entries = Vec::new();
    match run.item_tables().category(item.id) {
        2 | 3 => entries.push(Entry::Eat),
        0 | 1 | 4 => {}
        _ => entries.push(Entry::Use),
    }
    if run.team().len() > 1 {
        entries.push(Entry::Give);
    }
    let occupied = run
        .team()
        .first()
        .is_some_and(|l| run.floor_item_at(l.pos).is_some());
    if !occupied {
        entries.push(Entry::Place);
    }
    if can_throw {
        entries.push(Entry::Throw);
    }
    entries.push(Entry::Info);
    entries
}

/// The ground menu of the leader's tile: `None` with nothing on it. Shop stock offers Buy.
pub fn ground_entries(run: &Dungeon) -> Option<Vec<GroundEntry>> {
    let pos = run.team().first()?.pos;
    let item = run.floor_item_at(pos)?;
    Some(if item.flags & FLAG_SHOP != 0 {
        vec![GroundEntry::Buy, GroundEntry::Info]
    } else {
        vec![GroundEntry::Swap, GroundEntry::Info]
    })
}

/// A cursor moved one row up or down, wrapping, in `len` rows.
fn step(cursor: usize, len: usize, pressed: ButtonState) -> usize {
    if len == 0 {
        0
    } else if pressed.is_held(DsButton::Down) {
        (cursor + 1) % len
    } else if pressed.is_held(DsButton::Up) {
        (cursor + len - 1) % len
    } else {
        cursor.min(len - 1)
    }
}

/// `labels` in one line with the row under `cursor` in brackets.
fn row(labels: &[String], cursor: usize) -> String {
    labels
        .iter()
        .enumerate()
        .map(|(n, l)| {
            if n == cursor {
                format!("[{l}]")
            } else {
                l.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl ItemMenu {
    /// The menu as one line of text, for the log: the rows with the cursor's in brackets.
    /// `name` names an item id.
    pub fn describe(&self, run: &Dungeon, name: &dyn Fn(u8) -> String) -> String {
        let item_names = |slots: &[u8]| -> Vec<String> {
            slots
                .iter()
                .map(|&s| name(run.bag().slots[usize::from(s)].id))
                .collect()
        };
        match self {
            ItemMenu::List { cursor, purpose } => {
                let head = match purpose {
                    ListFor::Browse => "Items",
                    ListFor::Swap => "Swap with",
                };
                format!("{head}: {}", row(&item_names(&rows(run)), *cursor))
            }
            ItemMenu::Actions {
                slot,
                cursor,
                entries,
            } => {
                let labels: Vec<String> = entries.iter().map(|e| e.label().to_string()).collect();
                let id = run.bag().slots[usize::from(*slot)].id;
                format!("{}: {}", name(id), row(&labels, *cursor))
            }
            ItemMenu::Aim { slot } => {
                let id = run.bag().slots[usize::from(*slot)].id;
                format!("Throw the {} which way?", name(id))
            }
            ItemMenu::Ground { cursor, entries } => {
                let labels: Vec<String> = entries.iter().map(|e| e.label().to_string()).collect();
                format!("Ground: {}", row(&labels, *cursor))
            }
            ItemMenu::Confirm { slot, yes } => {
                let id = run.bag().slots[usize::from(*slot)].id;
                let labels = [String::from("Yes"), String::from("No")];
                format!(
                    "Swap with the {}? {}",
                    name(id),
                    row(&labels, usize::from(!*yes))
                )
            }
        }
    }

    /// The bag list, if the bag holds anything and the run has a bag.
    pub fn open_list(run: &Dungeon) -> Option<ItemMenu> {
        (run.item_tables().has_inventory && !rows(run).is_empty()).then_some(ItemMenu::List {
            cursor: 0,
            purpose: ListFor::Browse,
        })
    }

    /// The ground menu of the leader's tile, if there is an item on it.
    pub fn open_ground(run: &Dungeon) -> Option<ItemMenu> {
        ground_entries(run).map(|entries| ItemMenu::Ground { cursor: 0, entries })
    }

    /// The menu after this frame's presses, and the action a choice asks for. The menu is
    /// `None` once it closes.
    pub fn input(
        self,
        pressed: ButtonState,
        run: &Dungeon,
        facing: Direction,
        can_throw: bool,
    ) -> (Option<ItemMenu>, Option<Action>) {
        let a = pressed.is_held(DsButton::A);
        let b = pressed.is_held(DsButton::B);
        match self {
            ItemMenu::List { cursor, purpose } => {
                let list = rows(run);
                if b {
                    return (None, None);
                }
                let cursor = step(cursor, list.len(), pressed);
                let same = ItemMenu::List { cursor, purpose };
                let Some(&slot) = list.get(cursor).filter(|_| a) else {
                    return (Some(same), None);
                };
                match purpose {
                    ListFor::Swap => (Some(ItemMenu::Confirm { slot, yes: false }), None),
                    ListFor::Browse => {
                        let item = run.bag().slots[usize::from(slot)];
                        let entries = entries_for(run, item, can_throw);
                        (
                            Some(ItemMenu::Actions {
                                slot,
                                cursor: 0,
                                entries,
                            }),
                            None,
                        )
                    }
                }
            }
            ItemMenu::Actions {
                slot,
                cursor,
                entries,
            } => {
                let list = rows(run);
                let Some(row) = list.iter().position(|&s| s == slot) else {
                    return (None, None);
                };
                if b {
                    return (
                        Some(ItemMenu::List {
                            cursor: row,
                            purpose: ListFor::Browse,
                        }),
                        None,
                    );
                }
                let cursor = step(cursor, entries.len(), pressed);
                let Some(&entry) = entries.get(cursor).filter(|_| a) else {
                    return (
                        Some(ItemMenu::Actions {
                            slot,
                            cursor,
                            entries,
                        }),
                        None,
                    );
                };
                let id = run.bag().slots[usize::from(slot)].id;
                let stay = Some(ItemMenu::Actions {
                    slot,
                    cursor,
                    entries,
                });
                match entry {
                    Entry::Use | Entry::Eat => (None, Some(Action::Use(slot))),
                    Entry::Place => (None, Some(Action::Place(slot))),
                    Entry::Throw => (Some(ItemMenu::Aim { slot }), None),
                    Entry::Give => (stay, Some(Action::Give(slot))),
                    Entry::Info => (stay, Some(Action::Info(id))),
                }
            }
            ItemMenu::Aim { slot } => {
                if b {
                    return (
                        Some(ItemMenu::Actions {
                            slot,
                            cursor: 0,
                            entries: run
                                .bag()
                                .slots
                                .get(usize::from(slot))
                                .map(|&i| entries_for(run, i, can_throw))
                                .unwrap_or_default(),
                        }),
                        None,
                    );
                }
                // A direction throws that way; A alone throws the way the leader faces.
                let dir = input::direction(pressed).or(a.then_some(facing));
                match dir {
                    Some(d) => (None, Some(Action::Throw(slot, d))),
                    None => (Some(ItemMenu::Aim { slot }), None),
                }
            }
            ItemMenu::Ground { cursor, entries } => {
                if b {
                    return (None, None);
                }
                let cursor = step(cursor, entries.len(), pressed);
                let Some(&entry) = entries.get(cursor).filter(|_| a) else {
                    return (Some(ItemMenu::Ground { cursor, entries }), None);
                };
                let stay = Some(ItemMenu::Ground { cursor, entries });
                match entry {
                    GroundEntry::Swap => (
                        // The bag may be empty: then there is nothing to swap with.
                        (!rows(run).is_empty()).then_some(ItemMenu::List {
                            cursor: 0,
                            purpose: ListFor::Swap,
                        }),
                        None,
                    ),
                    GroundEntry::Buy => (stay, Some(Action::Buy)),
                    GroundEntry::Info => {
                        let id = run
                            .team()
                            .first()
                            .and_then(|l| run.floor_item_at(l.pos))
                            .map(|i| i.id);
                        (stay, id.map(Action::Info))
                    }
                }
            }
            ItemMenu::Confirm { slot, yes } => {
                if b {
                    return (
                        Some(ItemMenu::List {
                            cursor: rows(run).iter().position(|&s| s == slot).unwrap_or(0),
                            purpose: ListFor::Swap,
                        }),
                        None,
                    );
                }
                if pressed.is_held(DsButton::Up) || pressed.is_held(DsButton::Down) {
                    return (Some(ItemMenu::Confirm { slot, yes: !yes }), None);
                }
                match (a, yes) {
                    (true, true) => (None, Some(Action::Swap(slot))),
                    (true, false) => (None, None),
                    _ => (Some(ItemMenu::Confirm { slot, yes }), None),
                }
            }
        }
    }
}
