//! The tables of the damage and hit formulas (`docs/re/combat.md`, "Tables read by rescue-data",
//! #419). They sit in decoded ARM9 overlay 1 (type multipliers, label table, stage and
//! accuracy tables) and in the ARM9 binary (the physical-type bytes). Nothing is copied into the
//! repo: the values are read from the dump at run time, as [`crate::types::type_chart`] does.

use rescue_formats::read as bytes;

use crate::{FormatError, Result, types};

/// RAM address of decoded ARM9 overlay 1 in the Blue EUR dump (`ov_0001`, `docs/re/combat.md`).
pub const OVERLAY1_RAM: u32 = 0x0213_8860;
/// RAM address of the ARM9 binary.
pub const ARM9_RAM: u32 = 0x0200_0000;

/// Entries in each stage table: stages 0 to 20.
pub const STAGES: usize = 21;

/// Pointer array of the four type-effectiveness multipliers (`ov_0001::021AE248`).
const TYPE_MULTIPLIER_PTRS: u32 = 0x021A_E248;
/// The 8-byte object of the STAB and critical multiplier (1.5), `ov_0001::021AE238`.
const STAB_OBJECT: u32 = 0x021A_E238;
/// The 4 by 4 `s32` label table (`gEffectivenessChart`, `ov_0001::021AA588`).
const EFFECTIVENESS_CHART: u32 = 0x021A_A588;
/// Attack and defence stage multipliers, 21 `s24.8` each (`ov_0001::021AA660`, `021AA6B4`).
const ATTACK_STAGES: u32 = 0x021A_A660;
const DEFENCE_STAGES: u32 = 0x021A_A6B4;
/// Accuracy and evasion stage multipliers, 21 integers each (`ov_0001::021AA8C4`, `021AA918`).
const ACCURACY_STAGES: u32 = 0x021A_A8C4;
const EVASION_STAGES: u32 = 0x021A_A918;
/// Physical-type flag per type, one byte each, in the ARM9 binary (`0x02091E78`).
const PHYSICAL_TYPES: u32 = 0x0209_1E78;

/// Everything the damage and hit formulas read from the dump, except the type chart and the
/// move table (`rescue_data::types::type_chart`, `rescue_data::moves`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatTables {
    /// The type chart: attacker row, defender column, values 0 to 3 (see
    /// [`cell_multiplier`](Self::cell_multiplier)).
    pub type_chart: [[i16; types::TYPES]; types::TYPES],
    /// `[cell of the first defender type][cell of the second]`: the label. Label 3 is super
    /// effective.
    pub effectiveness: [[i32; 4]; 4],
    /// The multiplier (16.16) of each chart cell value 0 to 3. Cell 2 is neutral and is never
    /// multiplied, so its entry is read but not used.
    pub cell_multiplier: [u32; 4],
    /// The multiplier (16.16) of STAB, and of a critical hit.
    pub stab: u32,
    /// Per type, 1 for a physical type and 0 for a special one (`FUN_02008C64`).
    pub physical: [u8; types::TYPES],
    /// Attack stage multiplier per stage (24.8). Index 10 is neutral.
    pub attack_stage: [i32; STAGES],
    /// Defence stage multiplier per stage (24.8).
    pub defence_stage: [i32; STAGES],
    /// Accuracy stage multiplier per stage (integer, 256 is neutral).
    pub accuracy_stage: [i32; STAGES],
    /// Evasion stage multiplier per stage (integer, 256 is neutral).
    pub evasion_stage: [i32; STAGES],
}

/// The file offset in `overlay1` of an overlay 1 RAM address.
fn overlay_offset(addr: u32, what: &'static str) -> Result<usize> {
    addr.checked_sub(OVERLAY1_RAM)
        .map(|offset| offset as usize)
        .ok_or_else(|| FormatError::invalid(what, "address below overlay 1"))
}

/// The RAM address a pointer in overlay 1 holds.
fn pointer_at(overlay1: &[u8], addr: u32, what: &'static str) -> Result<u32> {
    bytes::u32_at(overlay1, overlay_offset(addr, what)?, what)
}

impl CombatTables {
    /// Reads the tables from decoded overlay 1 and the decompressed ARM9 binary (its RAM image from
    /// [`ARM9_RAM`] on).
    pub fn load(overlay1: &[u8], arm9: &[u8]) -> Result<Self> {
        let type_chart = types::type_chart(overlay1)?;

        let mut effectiveness = [[0i32; 4]; 4];
        for (i, cell) in effectiveness.iter_mut().flatten().enumerate() {
            let at = overlay_offset(EFFECTIVENESS_CHART, "effectiveness chart")? + i * 4;
            *cell = bytes::u32_at(overlay1, at, "effectiveness chart")? as i32;
        }

        let mut cell_multiplier = [0u32; 4];
        for (cell, multiplier) in cell_multiplier.iter_mut().enumerate() {
            let slot = TYPE_MULTIPLIER_PTRS + 4 * cell as u32;
            let object = pointer_at(overlay1, slot, "type multiplier pointer")?;
            // The object is 8 bytes; its second word is the 16.16 multiplier.
            let at = overlay_offset(object, "type multiplier")? + 4;
            *multiplier = bytes::u32_at(overlay1, at, "type multiplier")?;
        }
        let stab_at = overlay_offset(STAB_OBJECT, "STAB multiplier")? + 4;
        let stab = bytes::u32_at(overlay1, stab_at, "STAB multiplier")?;

        let mut physical = [0u8; types::TYPES];
        let base = usize::try_from(PHYSICAL_TYPES - ARM9_RAM)
            .map_err(|_| FormatError::invalid("physical types", "address below ARM9"))?;
        for (i, flag) in physical.iter_mut().enumerate() {
            *flag = bytes::u8_at(arm9, base + i, "physical types")?;
        }

        Ok(Self {
            type_chart,
            effectiveness,
            cell_multiplier,
            stab,
            physical,
            attack_stage: stage_table(overlay1, ATTACK_STAGES, "attack stage table")?,
            defence_stage: stage_table(overlay1, DEFENCE_STAGES, "defence stage table")?,
            accuracy_stage: stage_table(overlay1, ACCURACY_STAGES, "accuracy stage table")?,
            evasion_stage: stage_table(overlay1, EVASION_STAGES, "evasion stage table")?,
        })
    }
}

/// Reads one table of [`STAGES`] words at the overlay 1 RAM address `addr`.
fn stage_table(overlay1: &[u8], addr: u32, what: &'static str) -> Result<[i32; STAGES]> {
    let base = overlay_offset(addr, what)?;
    let mut table = [0i32; STAGES];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = bytes::u32_at(overlay1, base + i * 4, what)? as i32;
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(buf: &mut [u8], at: u32, value: u32) {
        let at = at as usize;
        buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// Overlay 1 and ARM9 bytes with the table values written at their addresses, all distinct.
    fn images() -> (Vec<u8>, Vec<u8>) {
        let mut ov = vec![0u8; 0x8_0000];
        let mut arm9 = vec![0u8; 0x9_2000];
        let ov_at = |addr: u32| addr - OVERLAY1_RAM;
        // Multiplier objects live after the pointer array; the pointers point at them.
        let objects = [0x021A_F000u32, 0x021A_F010, 0x021A_F020, 0x021A_F030];
        for (cell, object) in objects.iter().enumerate() {
            put(
                &mut ov,
                ov_at(TYPE_MULTIPLIER_PTRS) + 4 * cell as u32,
                *object,
            );
            put(&mut ov, ov_at(*object) + 4, 0x1000 + cell as u32);
        }
        put(&mut ov, ov_at(STAB_OBJECT) + 4, 0x2000);
        for (i, value) in (0..16u32).enumerate() {
            put(
                &mut ov,
                ov_at(EFFECTIVENESS_CHART) + 4 * i as u32,
                0x3000 + value,
            );
        }
        for i in 0..STAGES as u32 {
            put(&mut ov, ov_at(ATTACK_STAGES) + 4 * i, 0x4000 + i);
            put(&mut ov, ov_at(DEFENCE_STAGES) + 4 * i, 0x5000 + i);
            put(&mut ov, ov_at(ACCURACY_STAGES) + 4 * i, 0x6000 + i);
            put(&mut ov, ov_at(EVASION_STAGES) + 4 * i, 0x7000 + i);
        }
        // Type chart: cell (a, d) = (a * 18 + d) % 4, so every value is 0 to 3.
        for a in 0..types::TYPES {
            for d in 0..types::TYPES {
                let at = types::TYPE_CHART_OFFSET + (a * types::TYPES + d) * 2;
                ov[at..at + 2].copy_from_slice(&(((a * 18 + d) % 4) as i16).to_le_bytes());
            }
        }
        for t in 0..types::TYPES {
            arm9[(PHYSICAL_TYPES - ARM9_RAM) as usize + t] = (t % 2) as u8;
        }
        (ov, arm9)
    }

    #[test]
    fn tables_are_read_through_the_pointers() {
        let (ov, arm9) = images();
        let t = CombatTables::load(&ov, &arm9).unwrap();
        assert_eq!(t.cell_multiplier, [0x1000, 0x1001, 0x1002, 0x1003]);
        assert_eq!(t.stab, 0x2000);
        assert_eq!(t.effectiveness[0], [0x3000, 0x3001, 0x3002, 0x3003]);
        assert_eq!(t.effectiveness[3][3], 0x300F);
        assert_eq!(t.attack_stage[20], 0x4014);
        assert_eq!(t.defence_stage[0], 0x5000);
        assert_eq!(t.accuracy_stage[10], 0x600A);
        assert_eq!(t.evasion_stage[1], 0x7001);
        assert_eq!(t.physical[0..4], [0, 1, 0, 1]);
        assert_eq!(t.type_chart[1][2], 0);
    }

    #[test]
    fn a_short_dump_is_an_error_not_a_panic() {
        let (ov, arm9) = images();
        assert!(CombatTables::load(&ov[..0x7000], &arm9).is_err());
        assert!(CombatTables::load(&ov, &arm9[..0x1000]).is_err());
    }
}
