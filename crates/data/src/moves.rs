//! Moves and learnsets (`system.sbin` `wazapara`, `wazaparf`, ... per language).
//!
//! Content header: `{move table pointer, learnset table pointer}`. Moves are Red's
//! 0x24-byte `MoveDataEntry`; learnsets are `{level-up stream pointer, TM/HM stream
//! pointer}` per species. A stream is move IDs (1 byte, or 2 when bit 7 of the first is
//! set: `(b1 & 0x7F) << 7 | (b0 & 0x7F)`), each followed by a level byte for level-up
//! streams; 0 ends it.

use rescue_formats::read::{self as bytes, cstr_at};
use rescue_formats::sir0::Sir0;

use crate::{FormatError, Result, latin1};

const MOVE: usize = 0x24;

/// One move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    /// Index.
    pub id: u16,
    /// Name.
    pub name: String,
    /// Base power.
    pub base_power: i16,
    /// Type.
    pub kind: u8,
    /// Targeting flags (player, AI).
    pub targeting: [u16; 2],
    /// Base PP.
    pub base_pp: u8,
    /// AI weight.
    pub ai_weight: u8,
    /// Accuracy values.
    pub accuracy: [u8; 3],
    /// Maximum chained hits.
    pub chained_hits: u8,
    /// Maximum Ginseng upgrade level.
    pub max_upgrade: u8,
    /// Critical-hit chance.
    pub crit_chance: u8,
    /// Reflected by Magic Coat.
    pub magic_coat: bool,
    /// Snatchable.
    pub snatchable: bool,
    /// Uses the mouth (blocked by Muzzled).
    pub uses_mouth: bool,
    /// Cannot hit frozen targets.
    pub cannot_hit_frozen: bool,
    /// Ignores Taunted.
    pub ignores_taunted: bool,
    /// Range ID.
    pub range: u8,
    /// Description.
    pub description: String,
    /// Text shown when used.
    pub use_text: String,
}

/// A species' learnset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Learnset {
    /// `(move, level)` pairs in stream order.
    pub level_up: Vec<(u16, u8)>,
    /// TM/HM move IDs.
    pub tm_hm: Vec<u16>,
}

/// Parsed `wazapara`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveData {
    /// Moves.
    pub moves: Vec<Move>,
    /// Learnsets by species index.
    pub learnsets: Vec<Learnset>,
}

fn move_id(data: &[u8], at: &mut usize) -> Result<u16> {
    let b0 = bytes::u8_at(data, *at, "move stream")?;
    *at += 1;
    if b0 & 0x80 == 0 {
        return Ok(u16::from(b0));
    }
    let b1 = bytes::u8_at(data, *at, "move stream")?;
    *at += 1;
    // Red's DecompressMoveID: the first byte holds the high 7 bits.
    Ok(u16::from(b0 & 0x7F) << 7 | u16::from(b1 & 0x7F))
}

fn stream(data: &[u8], mut at: usize, with_level: bool) -> Result<Vec<(u16, u8)>> {
    let mut out = Vec::new();
    if at == 0 {
        return Ok(out); // null pointer: species 0 has no learnset
    }
    while bytes::u8_at(data, at, "move stream")? != 0 {
        let id = move_id(data, &mut at)?;
        let level = if with_level {
            at += 1;
            bytes::u8_at(data, at - 1, "move stream")?
        } else {
            0
        };
        out.push((id, level));
        if out.len() > 512 {
            return Err(FormatError::invalid("move stream", "no terminator"));
        }
    }
    Ok(out)
}

/// Parses `wazapara` (or a language variant).
pub fn parse_moves(data: &[u8]) -> Result<MoveData> {
    let s = Sir0::parse(data)?;
    let moves_at = s.ptr(s.data_offset, "move table pointer")?;
    let learn_at = s.ptr(s.data_offset + 4, "learnset table pointer")?;
    if learn_at < moves_at || s.data_offset < learn_at {
        return Err(FormatError::invalid("wazapara", "tables out of order"));
    }
    let text =
        |at: usize, what| -> Result<String> { Ok(latin1(cstr_at(data, s.ptr(at, what)?, what)?)) };
    let moves = (0..(learn_at - moves_at) / MOVE)
        .map(|i| {
            let at = moves_at + i * MOVE;
            let e = bytes::slice(data, at, MOVE, "move entry")?;
            let u16_ = |o: usize| u16::from_le_bytes([e[o], e[o + 1]]);
            Ok(Move {
                id: i as u16,
                name: text(at, "move name")?,
                base_power: i16::from_le_bytes([e[4], e[5]]),
                kind: e[6],
                targeting: [u16_(8), u16_(0xA)],
                base_pp: e[0xC],
                ai_weight: e[0xD],
                accuracy: [e[0xE], e[0xF], e[0x10]],
                chained_hits: e[0x11],
                max_upgrade: e[0x12],
                crit_chance: e[0x13],
                magic_coat: e[0x14] != 0,
                snatchable: e[0x15] != 0,
                uses_mouth: e[0x16] != 0,
                cannot_hit_frozen: e[0x17] != 0,
                ignores_taunted: e[0x18] != 0,
                range: e[0x19],
                description: text(at + 0x1C, "move description")?,
                use_text: text(at + 0x20, "move use text")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let learnsets = (0..(s.data_offset - learn_at) / 8)
        .map(|i| {
            let at = learn_at + i * 8;
            Ok(Learnset {
                level_up: stream(data, s.ptr(at, "level-up moves")?, true)?,
                tm_hm: stream(data, s.ptr(at + 4, "TM/HM moves")?, false)?
                    .into_iter()
                    .map(|(m, _)| m)
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(MoveData { moves, learnsets })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_streams() {
        let d = [0x21, 5, 0x81, 0x02, 9, 0];
        assert_eq!(stream(&d, 0, true).unwrap(), [(0x21, 5), (0x82, 9)]);
        assert_eq!(stream(&[3, 4, 0], 0, false).unwrap(), [(3, 0), (4, 0)]);
        assert!(stream(&[3, 4], 0, false).is_err());
    }
}
