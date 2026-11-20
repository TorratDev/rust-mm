//! Items in the dungeon scene (#555): what the sim's item API needs from the ROM (the item
//! tables, the throw numbers), the floor item and money sprites, and the log lines of picking up,
//! using, throwing and placing items.
// shortcut: the ROM's message strings are not read (their ids are not traced), so the lines are
// English placeholders and an item is named by its `itempara` name.

use rescue_data::dungeon::{Floor as FloorEntry, MapParam};
use rescue_data::item::Item as ItemData;
use rescue_data::species::Species;
use rescue_formats::graphics::Palette;
use rescue_render::camera::CELL_ANCHOR;
use rescue_render::sprite::{SpriteImage, render_obj};
use rescue_sim::command::Blocked;
use rescue_sim::item_use::UseResult;
use rescue_sim::items::{FLAG_SHOP, FloorItem, ItemTables, PICKER_CATEGORIES, POKE, PickerRow};
use rescue_sim::throw::{ThrowParams, ThrowResult};

/// Bytes before the first icon in `dungeon.sbin` `itempat` (the SIR0 header).
const ICON_BASE: usize = 0x14;
/// Bytes of one 16x16 4bpp icon (4 tiles).
const ICON_BYTES: usize = 128;
/// The icon the captured floor item used: 0x314 bytes into `itempat` (`docs/formats/sprites.md`).
const FALLBACK_ICON: usize = 6;
/// A floor item's top-left from its cell's corner (`docs/formats/sprites.md`, "Floor item").
const ITEM_CORNER: (i32, i32) = (4, 4);
/// The hit roll bound of a throw: 90 on floor 1 (`docs/re/items.md`, the s16 at `021AA268`).
const THROW_HIT_CHANCE: i32 = 90;
/// The damage of the arc items' fixed hit, which the sim wants from the caller.
// shortcut: the real third argument of `02146880` is not traced (`ThrowParams::arc_damage`).
const ARC_DAMAGE: i32 = 10;

/// The item graphics and text of the ROM: `itempara` and the icons of `itempat`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemArt {
    items: Vec<ItemData>,
    sheet: Vec<u8>,
}

impl ItemArt {
    pub fn new(items: Vec<ItemData>, sheet: Vec<u8>) -> Self {
        Self { items, sheet }
    }

    fn get(&self, id: u8) -> Option<&ItemData> {
        self.items.get(usize::from(id))
    }

    /// The item's name, without control bytes. Without the ROM: `Item <id>`.
    pub fn name(&self, id: u8) -> String {
        match self.get(id) {
            Some(i) => i.name.chars().filter(|c| !c.is_control()).collect(),
            None => format!("Item {id}"),
        }
    }

    /// The item's description, split at its line breaks, control bytes dropped.
    pub fn description(&self, id: u8) -> Vec<String> {
        let Some(item) = self.get(id) else {
            return vec![format!("There is nothing to tell about item {id}.")];
        };
        item.description
            .split(['\n', '\r'])
            .map(|l| l.chars().filter(|c| !c.is_control()).collect::<String>())
            .filter(|l| !l.is_empty())
            .collect()
    }

    /// The floor sprite of item `id`: its 16x16 icon, top-left a few pixels in from the cell's
    /// corner (relative to the entity anchor, as `Figure` wants). `None` with no icon sheet.
    // shortcut: the icon byte (`itempara +0xD`) is taken as the index of the 128-byte image in
    // `itempat`, and the item's palette byte as an index into the monster palettes. Only the
    // captured floor item's image (index 6) is verified (`docs/formats/sprites.md`), not which
    // item it was, nor the palette bank (OBJ bank 3 in the capture; its source is not traced).
    pub fn image(&self, id: u8, palettes: &[Palette]) -> Option<SpriteImage> {
        let item = self.get(id)?;
        let count = self.sheet.len().saturating_sub(ICON_BASE) / ICON_BYTES;
        let icon = match usize::from(item.icon) {
            i if i < count => i,
            _ => FALLBACK_ICON.min(count.saturating_sub(1)),
        };
        let palette = palettes
            .get(usize::from(item.palette))
            .or_else(|| palettes.first())?;
        let tiles = self.sheet.get(ICON_BASE + icon * ICON_BYTES..)?;
        Some(render_obj(
            tiles,
            0,
            (16, 16),
            palette,
            3,
            (ITEM_CORNER.0 - CELL_ANCHOR.0, ITEM_CORNER.1 - CELL_ANCHOR.1),
        ))
    }
}

/// The sim's item tables for a floor, read from the dump: the picker rows of its item lists, the
/// category of each item, the thrown stacks' size, and the orbs' moves.
// shortcut: the rest of `ItemTables` is left at its default: the sticky chance and floor level
// (`gDungeon+0x1A300`, `+0x1A30D`, not traced to the floor record), the money tables (`02091FA8`,
// `DAT_021432E4`: a Poké item is worth its quantity) and the handlers' numbers (heal amounts,
// seed turns, gummi tables: a heal is 0). The sim keeps one set for the run, so a later floor
// keeps the first floor's rows.
pub fn item_tables(items: &[ItemData], map: &MapParam, floor: &FloorEntry) -> ItemTables {
    let mut tables = ItemTables::new();
    tables.rows = floor
        .items
        .iter()
        .map(|&list| {
            let weights = map
                .item_spawns
                .get(usize::from(list))
                .map_or(&[][..], Vec::as_slice);
            let split = weights.len().min(PICKER_CATEGORIES);
            PickerRow {
                categories: weights[..split].to_vec(),
                items: weights[split..].to_vec(),
            }
        })
        .collect();
    tables.categories = items.iter().map(|i| i.category).collect();
    tables.stack_amount = items.iter().map(|i| i.spawn_amount[0]).collect();
    tables.use_params.orb_moves = items
        .iter()
        .map(|i| if i.category == 9 { i.move_id } else { 0 })
        .collect();
    tables
}

/// The sim's throw numbers: the species that may throw (`021803B8`, species byte `+0x33`) and the
/// hit roll, with the neighbour ring as the landing offsets.
// shortcut: the ring (`02143BDC`) and the arc damage are stand-ins (`ThrowParams`), and the hit
// chance is floor 1's value for every floor.
pub fn throw_params(species: &[Species]) -> ThrowParams {
    ThrowParams {
        hit_chance: THROW_HIT_CHANCE,
        arc_damage: ARC_DAMAGE,
        can_throw: species.iter().map(|s| s.byte_33 != 0).collect(),
        landing_ring: ThrowParams::neighbour_ring(),
        ..ThrowParams::new()
    }
}

/// The throw numbers with no ROM: the leader's species is 0 there, so only it may throw.
pub fn placeholder_throw_params() -> ThrowParams {
    ThrowParams {
        hit_chance: THROW_HIT_CHANCE,
        arc_damage: ARC_DAMAGE,
        can_throw: vec![true],
        landing_ring: ThrowParams::neighbour_ring(),
        ..ThrowParams::new()
    }
}

/// How the item is named in a line: the name, with a quantity for a stack.
fn label(art: &ItemArt, item: rescue_sim::items::Item) -> String {
    if item.id < rescue_sim::items::STACKABLE_BELOW && item.qty > 1 {
        format!("{} ({})", art.name(item.id), item.qty)
    } else {
        art.name(item.id)
    }
}

/// The lines of a leader's step: what it picked up, or that the bag is full. `before` and `after`
/// are the floor items around the step, `money` the team money before and after. `stepped` is the
/// tile the leader moved onto, if it moved.
pub fn pickup_lines(
    art: &ItemArt,
    floor: (&[Option<FloorItem>], &[Option<FloorItem>]),
    money: (i32, i32),
    stepped: Option<(i16, i16)>,
) -> Vec<String> {
    let (before, after) = floor;
    let mut lines = Vec::new();
    for fi in before.iter().flatten() {
        let still = after.iter().flatten().any(|a| a == fi);
        if !still {
            if fi.item.id == POKE {
                lines.push(format!("Picked up {} Poke.", money.1 - money.0));
            } else {
                lines.push(format!("Picked up the {}.", label(art, fi.item)));
            }
        } else if Some(fi.pos) == stepped && fi.item.flags & FLAG_SHOP == 0 {
            lines.push(String::from("The bag is full."));
        }
    }
    lines
}

/// The line of using or eating an item: `eat` for the Eat entry, by the sim's result.
pub fn use_line(art: &ItemArt, id: u8, eat: bool, result: Option<UseResult>) -> String {
    let name = art.name(id);
    match result {
        Some(UseResult::NothingHappens) => format!("Nothing happens with the {name}."),
        Some(UseResult::Unported) => format!("The {name} was used up; its effect is not modelled."),
        _ if eat => format!("Ate the {name}."),
        _ => format!("Used the {name}."),
    }
}

/// The line of a refused item action.
pub fn blocked_line(art: &ItemArt, name_of: u8, why: Blocked) -> String {
    let name = art.name(name_of);
    match why {
        Blocked::ItemSticky => format!("The {name} is sticky."),
        Blocked::DropTile => String::from("It can't be placed here."),
        Blocked::FloorItemsFull => String::from("There is no room on the floor."),
        Blocked::NoItem => String::from("There is no such item."),
        _ => String::from("Nothing happens."),
    }
}

/// The line of a throw, by the sim's result.
pub fn throw_line(art: &ItemArt, id: u8, result: &ThrowResult) -> String {
    let name = art.name(id);
    match result {
        ThrowResult::Thrown(r) if r.hits > 0 => {
            format!("The {name} hit for {} damage.", r.damage)
        }
        ThrowResult::Thrown(r) if r.misses > 0 => format!("The {name} missed."),
        ThrowResult::Thrown(_) => format!("Threw the {name}."),
        ThrowResult::Blocked(b) => blocked_line(art, id, *b),
        ThrowResult::SpeciesGate => String::from("It can't throw items."),
        ThrowResult::RunsAway => String::from("It is too scared to throw."),
        ThrowResult::Unmodelled => String::from("Nothing happens."),
        ThrowResult::Skipped => String::from("It is asleep and threw nothing."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_sim::items::Item;

    fn art() -> ItemArt {
        ItemArt::default()
    }

    #[test]
    fn a_missing_table_names_items_by_number() {
        assert_eq!(art().name(7), "Item 7");
        assert!(art().image(7, &[]).is_none());
    }

    #[test]
    fn a_step_that_takes_an_item_says_so_and_a_full_bag_keeps_it() {
        let item = FloorItem {
            pos: (3, 3),
            item: Item::new(0x52, 0),
        };
        let gone = pickup_lines(&art(), (&[Some(item)], &[None]), (0, 0), Some((3, 3)));
        assert_eq!(gone, vec![String::from("Picked up the Item 82.")]);
        let stays = pickup_lines(&art(), (&[Some(item)], &[Some(item)]), (0, 0), Some((3, 3)));
        assert_eq!(stays, vec![String::from("The bag is full.")]);
        // Not stepped on this turn: nothing is said.
        let quiet = pickup_lines(&art(), (&[Some(item)], &[Some(item)]), (0, 0), None);
        assert!(quiet.is_empty());
    }

    #[test]
    fn money_says_how_much() {
        let money = FloorItem {
            pos: (1, 1),
            item: Item::new(POKE, 5),
        };
        let lines = pickup_lines(&art(), (&[Some(money)], &[None]), (10, 60), Some((1, 1)));
        assert_eq!(lines, vec![String::from("Picked up 50 Poke.")]);
    }
}
