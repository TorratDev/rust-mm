//! Dungeon floor data (`dungeon.sbin` `mapparam`), Red's `DungeonMapParam2`.
//!
//! Content header, five pointers: per-dungeon floor tables, the `FloorProperties`
//! array, item-spawn lists, monster-spawn lists, trap-chance lists. A dungeon's floor
//! table is an array of 16-byte records `{s16 floor properties, s16 monster list,
//! s16 trap list, s16 item lists[4], s16 unused}`; its length runs to the next table.

use rescue_formats::read as bytes;
use rescue_formats::sir0::Sir0;

use crate::{FormatError, Result};

/// Bytes per `FloorProperties`.
pub const FLOOR_PROPERTIES: usize = 0x1C;
/// Item spawn categories (Red `NUM_ITEM_CATEGORIES`).
pub const ITEM_CATEGORIES: usize = 12;
/// Values at or above this in an item list are "skip `v - 30000` zeros" (Red
/// `ITEM_SETS_SKIP_NUMBER`).
pub const ITEM_SKIP: u16 = 30000;
/// Traps per trap-chance list.
pub const TRAPS: usize = 20;

/// One floor of a dungeon: indices into the shared lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Floor {
    /// Index into [`MapParam::floor_properties`].
    pub properties: u16,
    /// Index into [`MapParam::monster_spawns`].
    pub monsters: u16,
    /// Index into [`MapParam::trap_chances`].
    pub traps: u16,
    /// Indices into [`MapParam::item_spawns`]: normal, shop, monster house, buried.
    pub items: [u16; 4],
}

/// One monster spawn entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterSpawn {
    /// Species (low 9 bits of the packed word).
    pub species: u16,
    /// Level (high 7 bits).
    pub level: u8,
    /// Spawn weights (Red `randNum[2]`).
    pub weights: [i16; 2],
}

/// Parsed `mapparam`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapParam {
    /// Floors per dungeon, by dungeon index (floor 0 is a placeholder in Red).
    pub dungeons: Vec<Vec<Floor>>,
    /// Raw 28-byte `FloorProperties` records (field layout: Red's struct, see the
    /// floor generator in `rescue-sim`).
    pub floor_properties: Vec<[u8; FLOOR_PROPERTIES]>,
    /// Monster spawn lists.
    pub monster_spawns: Vec<Vec<MonsterSpawn>>,
    /// Trap chances.
    pub trap_chances: Vec<[u16; TRAPS]>,
    /// Item spawn weights, expanded: categories, then one per item ID.
    pub item_spawns: Vec<Vec<u16>>,
}

/// Reads consecutive pointers starting at `at` (entries the SIR0 list marks).
fn pointer_table(s: &Sir0<'_>, mut at: usize) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    while s.pointers.binary_search(&at).is_ok() {
        out.push(s.ptr(at, "mapparam table")?);
        at += 4;
    }
    Ok(out)
}

/// Parses `mapparam`. `item_ids` is the number of item IDs (the `itempara` length), which
/// fixes the expanded length of item lists.
pub fn parse_mapparam(data: &[u8], item_ids: usize) -> Result<MapParam> {
    let s = Sir0::parse(data)?;
    let h: Vec<usize> = (0..5)
        .map(|i| s.ptr(s.data_offset + i * 4, "mapparam header"))
        .collect::<Result<_>>()?;
    let i16_ = |o: usize| bytes::i16_at(data, o, "mapparam");
    let idx = |v: i16| {
        u16::try_from(v).map_err(|_| FormatError::invalid("mapparam floor", format!("index {v}")))
    };

    let dungeon_tables = pointer_table(&s, h[0])?;
    let mut dungeons = Vec::with_capacity(dungeon_tables.len());
    for (i, &start) in dungeon_tables.iter().enumerate() {
        // Each table runs to the next one; the last to the table of pointers itself.
        let end = dungeon_tables.get(i + 1).copied().unwrap_or(h[0]);
        let floors = (start..end.max(start))
            .step_by(16)
            .take_while(|o| o + 16 <= end)
            .map(|o| {
                Ok(Floor {
                    properties: idx(i16_(o)?)?,
                    monsters: idx(i16_(o + 2)?)?,
                    traps: idx(i16_(o + 4)?)?,
                    items: [
                        idx(i16_(o + 6)?)?,
                        idx(i16_(o + 8)?)?,
                        idx(i16_(o + 10)?)?,
                        idx(i16_(o + 12)?)?,
                    ],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        dungeons.push(floors);
    }

    let monster_lists = pointer_table(&s, h[3])?;
    let props_end = monster_lists.iter().copied().min().unwrap_or(h[1]);
    let floor_properties = (0..props_end.saturating_sub(h[1]) / FLOOR_PROPERTIES)
        .map(|i| {
            bytes::array::<FLOOR_PROPERTIES>(data, h[1] + i * FLOOR_PROPERTIES, "floor properties")
        })
        .collect::<Result<Vec<_>>>()?;

    let monster_spawns = monster_lists
        .iter()
        .map(|&start| {
            let mut out = Vec::new();
            let mut o = start;
            loop {
                let packed = bytes::u16_at(data, o, "monster spawn")?;
                if packed & 0x1FF == 0 {
                    break;
                }
                out.push(MonsterSpawn {
                    species: packed & 0x1FF,
                    level: (packed >> 9) as u8,
                    weights: [i16_(o + 2)?, i16_(o + 4)?],
                });
                o += 6;
                if out.len() > 64 {
                    return Err(FormatError::invalid("monster spawn list", "no terminator"));
                }
            }
            Ok(out)
        })
        .collect::<Result<Vec<_>>>()?;

    let trap_chances = pointer_table(&s, h[4])?
        .into_iter()
        .map(|start| {
            let mut t = [0u16; TRAPS];
            for (k, v) in t.iter_mut().enumerate() {
                *v = bytes::u16_at(data, start + k * 2, "trap chances")?;
            }
            Ok(t)
        })
        .collect::<Result<Vec<_>>>()?;

    let want = ITEM_CATEGORIES + item_ids;
    let item_spawns = pointer_table(&s, h[2])?
        .into_iter()
        .map(|start| {
            let mut out = Vec::with_capacity(want);
            let mut o = start;
            while out.len() < want {
                let v = bytes::u16_at(data, o, "item spawns")?;
                o += 2;
                if v >= ITEM_SKIP {
                    out.resize(out.len() + usize::from(v - ITEM_SKIP), 0);
                } else {
                    out.push(v);
                }
            }
            out.truncate(want);
            Ok(out)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(MapParam {
        dungeons,
        floor_properties,
        monster_spawns,
        trap_chances,
        item_spawns,
    })
}
