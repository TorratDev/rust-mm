//! Species (`system.sbin` `monspara`, 0x48 bytes per entry, Red's `MonsterDataEntry`)
//! and level-up stat growth (`lvmpNNN`, 100 x 12-byte `LevelData`).

use rescue_formats::px;
use rescue_formats::read::{self as bytes, cstr_at};
use rescue_formats::sir0::Sir0;

use crate::{FormatError, Result, latin1, record_count};

const ENTRY: usize = 0x48;

/// One species.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Species {
    /// Index (0 is a placeholder entry).
    pub id: u16,
    /// Name.
    pub name: String,
    /// Category ("Seed", ...).
    pub category: String,
    /// Overworld palette number.
    pub palette: u8,
    /// Body size.
    pub body_size: u8,
    /// Movement speed.
    pub movement_speed: i32,
    /// Bit per emotion: portrait available.
    pub portrait_flags: u16,
    /// Types (0 = none).
    pub types: [u8; 2],
    /// Movement type.
    pub movement_type: u8,
    /// Friend area.
    pub friend_area: u8,
    /// Abilities.
    pub abilities: [u8; 2],
    /// Shadow size.
    pub shadow_size: u8,
    /// HP regeneration speed: the byte's low 7 bits, doubled (Blue `FUN_02002368`).
    pub regen_speed: u8,
    /// Bit 7 of the regeneration byte. Meaning unknown.
    pub regen_flag: bool,
    /// Moves unless leader.
    pub can_move: bool,
    /// Chance to be asleep on spawn.
    pub chance_asleep: u8,
    /// Base HP.
    pub base_hp: u16,
    /// Experience yield.
    pub exp_yield: i32,
    /// Base Attack, Special Attack.
    pub base_attack: [u16; 2],
    /// Base Defense, Special Defense.
    pub base_defense: [u16; 2],
    /// Weight.
    pub weight: u16,
    /// Size.
    pub size: u16,
    /// Evolves from (species, method).
    pub pre_evolution: (i16, u16),
    /// Evolution requirement (main, additional).
    pub evolution_requirement: (i16, u16),
    /// National dex number and internal number.
    pub dex: [i16; 2],
    /// Recruit rate, per mille (blue-verified: `FUN_ov_0001__0217f9b8`).
    pub recruit_rate: i16,
    /// Alphabetical order number and parent species.
    pub alphabet_parent: [i16; 2],
    /// Byte `+0x12`, read by overlay 1 (`FUN_02002288`). Meaning unknown.
    pub byte_12: u8,
    /// Byte `+0x33`, read by overlay 1 (`FUN_020022a4`). Meaning unknown.
    pub byte_33: u8,
}

/// Parses `monspara`.
pub fn parse_species(data: &[u8]) -> Result<Vec<Species>> {
    let s = Sir0::parse(data)?;
    let start = s.data_offset;
    (0..record_count(&s, start, ENTRY))
        .map(|i| {
            let e = bytes::slice(data, start + i * ENTRY, ENTRY, "species entry")?;
            let u8_ = |o: usize| e[o];
            let u16_ = |o: usize| u16::from_le_bytes([e[o], e[o + 1]]);
            let i16_ = |o: usize| i16::from_le_bytes([e[o], e[o + 1]]);
            let i32_ = |o: usize| i32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
            let at = start + i * ENTRY;
            Ok(Species {
                id: i as u16,
                name: latin1(cstr_at(data, s.ptr(at, "species name")?, "species name")?),
                category: latin1(cstr_at(
                    data,
                    s.ptr(at + 4, "species category")?,
                    "species category",
                )?),
                palette: u8_(8),
                body_size: u8_(9),
                movement_speed: i32_(0xC),
                portrait_flags: u16_(0x10),
                types: [u8_(0x13), u8_(0x14)],
                movement_type: u8_(0x15),
                friend_area: u8_(0x16),
                abilities: [u8_(0x17), u8_(0x18)],
                shadow_size: u8_(0x19),
                regen_speed: (u8_(0x1B) & 0x7F) << 1,
                regen_flag: u8_(0x1B) & 0x80 != 0,
                can_move: u8_(0x1C) != 0,
                chance_asleep: u8_(0x1D),
                base_hp: u16_(0x1E),
                exp_yield: i32_(0x20),
                base_attack: [u16_(0x24), u16_(0x26)],
                base_defense: [u16_(0x28), u16_(0x2A)],
                weight: u16_(0x2C),
                size: u16_(0x2E),
                pre_evolution: (i16_(0x34), u16_(0x36)),
                evolution_requirement: (i16_(0x38), u16_(0x3A)),
                dex: [i16_(0x3C), i16_(0x3E)],
                recruit_rate: i16_(0x40),
                alphabet_parent: [i16_(0x42), i16_(0x44)],
                byte_12: u8_(0x12),
                byte_33: u8_(0x33),
            })
        })
        .collect()
}

/// Stat growth for one level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelData {
    /// Total experience needed.
    pub exp_required: i32,
    /// HP gained.
    pub hp: u16,
    /// Attack, Special Attack gained.
    pub attack: [u8; 2],
    /// Defense, Special Defense gained.
    pub defense: [u8; 2],
}

/// Parses an `lvmpNNN` file: SIR0 wrapping an AT4PX of 12-byte records (levels 1..=100).
pub fn parse_level_data(data: &[u8]) -> Result<Vec<LevelData>> {
    let s = Sir0::parse(data)?;
    let raw = px::decompress(s.tail(s.data_offset, "level data")?, px::DEFAULT_LIMIT)?;
    if raw.len() % 12 != 0 {
        return Err(FormatError::invalid(
            "level data",
            format!("{} bytes", raw.len()),
        ));
    }
    Ok(raw
        .chunks_exact(12)
        .map(|c| LevelData {
            exp_required: i32::from_le_bytes([c[0], c[1], c[2], c[3]]),
            hp: u16::from_le_bytes([c[4], c[5]]),
            attack: [c[6], c[7]],
            defense: [c[8], c[9]],
        })
        .collect())
}
