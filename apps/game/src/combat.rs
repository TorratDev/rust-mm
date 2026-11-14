//! The combat the dungeon scene runs (#423): the tables the sim's attack needs, read from the
//! dump, and the message log lines an attack's result makes. The sim reports no per-hit
//! events, so the lines come from comparing the wild monsters before and after a command.

use rescue_data::combat::CombatTables;
use rescue_data::moves::{Move, parse_moves};
use rescue_data::species::{LevelData, parse_level_data, parse_species};
use rescue_formats::archive::Archive;
use rescue_nds::binary::{binary_bytes, overlay_data, read_overlay_tables};
use rescue_nds::blz;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::combat::CombatData;
use rescue_sim::state::Monster;

/// Lines the message log keeps.
pub const LOG_LINES: usize = 4;

/// The tables of the sim's damage path, owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    tables: CombatTables,
    pub moves: Vec<Move>,
    exp_yields: Vec<i32>,
    weights: Vec<u16>,
    levels: Vec<Vec<LevelData>>,
}

impl Tables {
    /// Reads the tables from the dump `rom`. `leader` is the leader's species: its level table
    /// is the one loaded.
    // shortcut: only the leader's `lvmpNNN` is read, so a wild monster's own level table is
    // empty; the sim's EXP award reads the defeated monster's yield and the leader's table.
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

/// The log lines of an attack, from the wild monsters before and after it: each one that lost
/// HP is hit, each one gone is defeated. Monsters stand still during an attack, so a monster is
/// matched by its tile and species. Empty when nothing changed.
// shortcut: the ROM's message strings are not read (their ids are not traced), so the lines
// are placeholders and a monster is named by its species number.
pub fn attack_messages(before: &[Monster], after: &[Monster]) -> Vec<String> {
    let mut lines = Vec::new();
    for m in before {
        let name = format!("Monster {}", m.info.species);
        let now = after
            .iter()
            .find(|a| a.pos == m.pos && a.info.species == m.info.species);
        match now {
            None => lines.push(format!("{name} was defeated!")),
            Some(a) if a.info.hp < m.info.hp => {
                lines.push(format!("{name} took {} damage.", m.info.hp - a.info.hp));
            }
            Some(_) => {}
        }
    }
    lines
}

/// The monsters of `before` that are still in `after` with less HP: the ones to play a hit on.
pub fn hit_tiles(before: &[Monster], after: &[Monster]) -> Vec<(i16, i16)> {
    before
        .iter()
        .filter(|m| {
            after.iter().any(|a| {
                a.pos == m.pos && a.info.species == m.info.species && a.info.hp < m.info.hp
            })
        })
        .map(|m| m.pos)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_sim::state::{EntityInfo, MoveSlot};

    fn mon(pos: (i16, i16), species: u16, hp: i16) -> Monster {
        Monster {
            pos,
            visible: true,
            info: EntityInfo {
                species,
                apparent_species: species,
                not_team_member: true,
                is_team_leader: false,
                level: 1,
                iq: 0,
                hp,
                max_hp: 50,
                attack: [0, 0],
                defence: [0, 0],
                offensive_stages: [10, 10],
                defensive_stages: [10, 10],
                accuracy_stage: 10,
                evasion_stage: 10,
                types: [0, 0],
                abilities: [0, 0],
                moves: [MoveSlot::default(); 4],
                speed_up: [0; 5],
                speed_down: [0; 5],
                direction: 0,
                belly: 0,
                belly_empty: false,
                regen_acc: 0,
                regen_speed: 0,
                exp: 0,
                exp_gained: 0,
            },
        }
    }

    #[test]
    fn a_hit_names_the_damage_and_a_defeat_names_the_monster() {
        let before = [mon((3, 3), 7, 30), mon((4, 3), 9, 5), mon((9, 9), 2, 8)];
        let after = [mon((3, 3), 7, 22), mon((9, 9), 2, 8)];
        assert_eq!(
            attack_messages(&before, &after),
            vec!["Monster 7 took 8 damage.", "Monster 9 was defeated!"]
        );
        assert_eq!(hit_tiles(&before, &after), vec![(3, 3)]);
    }

    #[test]
    fn nothing_changed_is_no_line() {
        let w = [mon((3, 3), 7, 30)];
        assert!(attack_messages(&w, &w).is_empty());
        assert!(hit_tiles(&w, &w).is_empty());
    }
}
