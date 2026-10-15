//! The type-effectiveness chart (`gTypeEffectivenessChart`), 18×18 `s16` cells.
//!
//! Layout as found in Blue (docs/symbols.md, V6/V8): row = attacking type, column =
//! defending type, `0x24` bytes per row. The table is in `arm9 overlay 1` after BLZ
//! decompression, at offset `0x72688` (see `docs/verification.md`).

use std::fmt;

/// Number of types in the chart.
pub const TYPE_COUNT: usize = 18;

/// Size of the chart in bytes (18 × 18 × 2).
pub const TYPE_CHART_LEN: usize = TYPE_COUNT * TYPE_COUNT * 2;

/// Errors from parsing typed tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataError {
    /// The input is shorter than the table.
    Truncated { need: usize, have: usize },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Truncated { need, have } => {
                write!(f, "type chart needs {need} bytes, have {have}")
            }
        }
    }
}

impl std::error::Error for DataError {}

/// The 18×18 effectiveness chart, indexed `[attacker][defender]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeChart(pub [[i16; TYPE_COUNT]; TYPE_COUNT]);

impl TypeChart {
    /// Effectiveness of an attacking type against a defending type.
    pub fn get(&self, attacker: usize, defender: usize) -> Option<i16> {
        self.0.get(attacker)?.get(defender).copied()
    }
}

/// Parses the chart from the start of `bytes` (the bytes at the table's offset).
pub fn parse_type_chart(bytes: &[u8]) -> Result<TypeChart, DataError> {
    let table = bytes.get(..TYPE_CHART_LEN).ok_or(DataError::Truncated {
        need: TYPE_CHART_LEN,
        have: bytes.len(),
    })?;
    let mut out = [[0i16; TYPE_COUNT]; TYPE_COUNT];
    for (i, cell) in table.chunks_exact(2).enumerate() {
        let row = i / TYPE_COUNT;
        let col = i % TYPE_COUNT;
        let v = i16::from_le_bytes([cell[0], cell[1]]);
        if let Some(slot) = out.get_mut(row).and_then(|r| r.get_mut(col)) {
            *slot = v;
        }
    }
    Ok(TypeChart(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows_and_columns() {
        let mut bytes = vec![0u8; TYPE_CHART_LEN];
        // Attacker 2, defender 5 => cell index 2*18 + 5.
        let at = (2 * TYPE_COUNT + 5) * 2;
        bytes[at..at + 2].copy_from_slice(&(-7i16).to_le_bytes());
        let chart = parse_type_chart(&bytes).unwrap();
        assert_eq!(chart.get(2, 5), Some(-7));
        assert_eq!(chart.get(5, 2), Some(0));
        assert_eq!(chart.get(TYPE_COUNT, 0), None);
    }

    #[test]
    fn truncated_input_is_an_error() {
        assert_eq!(
            parse_type_chart(&[0u8; 10]),
            Err(DataError::Truncated {
                need: TYPE_CHART_LEN,
                have: 10
            })
        );
    }
}
