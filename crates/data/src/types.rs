//! The type effectiveness chart: 18 x 18 `s16`, attacker row, defender column. In Blue it
//! sits in decoded ARM9 overlay 1 at [`TYPE_CHART_OFFSET`] (V6, `docs/verification.md`),
//! byte-identical to Red's `gTypeEffectivenessChart`.

use rescue_formats::read as bytes;

use crate::Result;

/// Number of types (including "none").
pub const TYPES: usize = 18;
/// Offset of the chart in decoded ARM9 overlay 1 of the Blue EUR dump.
pub const TYPE_CHART_OFFSET: usize = 0x72688;

/// Parses the chart from decoded overlay 1 bytes.
pub fn type_chart(overlay1: &[u8]) -> Result<[[i16; TYPES]; TYPES]> {
    let mut chart = [[0i16; TYPES]; TYPES];
    for (a, row) in chart.iter_mut().enumerate() {
        for (d, cell) in row.iter_mut().enumerate() {
            *cell = bytes::i16_at(
                overlay1,
                TYPE_CHART_OFFSET + (a * TYPES + d) * 2,
                "type chart",
            )?;
        }
    }
    Ok(chart)
}
