//! Recruit rate by dungeon level (`gRecruitRateByLevel`, Red's `dungeon_config.c`). In Blue
//! it is 101 `s16` values in decoded ARM9 overlay 1 at [`RECRUIT_RATE_OFFSET`]; the values
//! are identical to Red's (240 at levels 90 to 100). Read by `FUN_ov_0001__0217f9b8`.

use rescue_formats::read as bytes;

use crate::Result;

/// Levels with a rate: 0 to 100.
pub const LEVELS: usize = 101;
/// Offset of the table in decoded ARM9 overlay 1 of the Blue EUR dump.
pub const RECRUIT_RATE_OFFSET: usize = 0x7210C;

/// Parses the table from decoded overlay 1 bytes.
pub fn recruit_rate_by_level(overlay1: &[u8]) -> Result<[i16; LEVELS]> {
    let mut rates = [0i16; LEVELS];
    for (level, rate) in rates.iter_mut().enumerate() {
        *rate = bytes::i16_at(
            overlay1,
            RECRUIT_RATE_OFFSET + level * 2,
            "recruit rate table",
        )?;
    }
    Ok(rates)
}
