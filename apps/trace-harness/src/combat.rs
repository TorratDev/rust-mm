//! The tables the sim's attack path reads, loaded from the trace's dump for the harness (#424).
//! The same reads as the game's loader (`apps/game/src/combat.rs`, `Tables::from_rom`), which
//! the harness cannot depend on.
// shortcut: this copies the game's loader. Move both to one crate once the loader's home is
// decided, so the two cannot drift.

use rescue_data::combat::CombatTables;
use rescue_data::moves::{Move, parse_moves};
use rescue_data::species::{LevelData, parse_level_data, parse_species};
use rescue_formats::archive::Archive;
use rescue_nds::binary::{binary_bytes, overlay_data, read_overlay_tables};
use rescue_nds::blz;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::combat::CombatData;

/// The tables of the sim's attack path, owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    tables: CombatTables,
    moves: Vec<Move>,
    exp_yields: Vec<i32>,
    weights: Vec<u16>,
    levels: Vec<Vec<LevelData>>,
}

impl Tables {
    /// Reads the tables from the dump `rom`. `leader` is the trace's leader species: only its
    /// level table is loaded (the sim's EXP award reads the leader's table).
    pub fn from_rom(rom: &[u8], leader: u16) -> Result<Self, String> {
        let header = Header::parse(rom).map_err(|e| e.to_string())?;
        let fs = Filesystem::parse(rom, &header).map_err(|e| e.to_string())?;
        let raw = binary_bytes(rom, &header.arm9).map_err(|e| e.to_string())?;
        // The ARM9 may be stored compressed; the tables read its RAM image.
        let arm9 = blz::decompress(raw).unwrap_or_else(|_| raw.to_vec());
        let (overlays, _) = read_overlay_tables(rom, &header).map_err(|e| e.to_string())?;
        let ov1 = overlays.iter().find(|o| o.id == 1).ok_or("no overlay 1")?;
        let ov1 = overlay_data(rom, &fs, ov1).map_err(|e| e.to_string())?;
        let tables = CombatTables::load(&ov1, &arm9).map_err(|e| e.to_string())?;

        let id = fs.get("system.sbin").ok_or("no system.sbin")?.id;
        let sys = fs.data(rom, id).map_err(|e| e.to_string())?;
        let arc = Archive::parse(sys).map_err(|e| e.to_string())?;
        let file = |name: &str| arc.get(sys, name).map_err(|e| e.to_string());
        let moves = parse_moves(file("wazapara")?)
            .map_err(|e| e.to_string())?
            .moves;
        let species = parse_species(file("monspara")?).map_err(|e| e.to_string())?;
        let exp_yields = species.iter().map(|s| s.exp_yield).collect();
        let weights = species.iter().map(|s| s.weight).collect();
        let mut levels = vec![Vec::new(); species.len()];
        let own =
            parse_level_data(file(&format!("lvmp{leader:03}"))?).map_err(|e| e.to_string())?;
        if let Some(slot) = levels.get_mut(usize::from(leader)) {
            *slot = own;
        }
        Ok(Tables {
            tables,
            moves,
            exp_yields,
            weights,
            levels,
        })
    }

    /// The sim's view of the tables.
    #[must_use]
    pub fn data(&self) -> CombatData<'_> {
        CombatData {
            tables: &self.tables,
            moves: &self.moves,
            exp_yields: &self.exp_yields,
            weights: &self.weights,
            levels: &self.levels,
        }
    }
}
