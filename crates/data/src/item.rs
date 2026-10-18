//! Items (`system.sbin` `itempara`, `itemparf`, ... per language): Red's 0x20-byte
//! `ItemDataEntry`.

use rescue_formats::read::{self as bytes, cstr_at};
use rescue_formats::sir0::Sir0;

use crate::{Result, latin1, record_count};

const ENTRY: usize = 0x20;

/// One item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Index.
    pub id: u16,
    /// Name (may contain text control codes).
    pub name: String,
    /// Buy price.
    pub buy_price: u32,
    /// Sell price.
    pub sell_price: u32,
    /// Category.
    pub category: u8,
    /// Icon.
    pub icon: u8,
    /// Description.
    pub description: String,
    /// AI use flags: on self, thrown at allies, thrown at enemies.
    pub ai_flags: [bool; 3],
    /// Move taught (TMs/HMs, orbs).
    pub move_id: u16,
    /// Storage order.
    pub order: u8,
    /// Spawn amount range.
    pub spawn_amount: [u8; 2],
    /// Palette.
    pub palette: u8,
    /// Action type.
    pub action: u8,
}

/// Parses `itempara`.
pub fn parse_items(data: &[u8]) -> Result<Vec<Item>> {
    let s = Sir0::parse(data)?;
    let start = s.data_offset;
    (0..record_count(&s, start, ENTRY))
        .map(|i| {
            let at = start + i * ENTRY;
            let e = bytes::slice(data, at, ENTRY, "item entry")?;
            let u32_ = |o: usize| u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
            let text = |o: usize, what| -> Result<String> {
                Ok(latin1(cstr_at(data, s.ptr(at + o, what)?, what)?))
            };
            Ok(Item {
                id: i as u16,
                name: text(0, "item name")?,
                buy_price: u32_(4),
                sell_price: u32_(8),
                category: e[0xC],
                icon: e[0xD],
                description: text(0x10, "item description")?,
                ai_flags: [e[0x14] != 0, e[0x15] != 0, e[0x16] != 0],
                move_id: u16::from_le_bytes([e[0x18], e[0x19]]),
                order: e[0x1A],
                spawn_amount: [e[0x1B], e[0x1C]],
                palette: e[0x1D],
                action: e[0x1E],
            })
        })
        .collect()
}
