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
use rescue_sim::command::Direction;
use rescue_sim::state::{EntityInfo, Monster};

use crate::walk::Walk;

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

#[cfg(test)]
impl Tables {
    /// Hand-made tables for the headless tests: neutral stages, a move table that holds the
    /// throw's move. The numbers are the tests' own, not the game's.
    pub fn for_tests() -> Self {
        use rescue_data::combat::STAGES;
        use rescue_data::types::TYPES;
        let neutral = [0x100i32; STAGES];
        let tables = CombatTables {
            type_chart: [[2i16; TYPES]; TYPES],
            effectiveness: [[2i32; 4]; 4],
            cell_multiplier: [0x4000, 0xC000, 0x10000, 0x20000],
            stab: 0x14000,
            physical: [0u8; TYPES],
            attack_stage: neutral,
            defence_stage: neutral,
            accuracy_stage: neutral,
            evasion_stage: neutral,
            regular_attack_weight: [9, 7, 5, 3, 1],
        };
        let mv = |id: u16| Move {
            id,
            name: String::new(),
            base_power: 2,
            kind: 0,
            targeting: [0, 0],
            base_pp: 10,
            ai_weight: 0,
            accuracy: [100; 3],
            chained_hits: 0,
            max_upgrade: 0,
            crit_chance: 0,
            magic_coat: false,
            snatchable: false,
            uses_mouth: false,
            cannot_hit_frozen: false,
            ignores_taunted: false,
            range: 0,
            description: String::new(),
            use_text: String::new(),
        };
        Tables {
            tables,
            moves: (0..=rescue_sim::throw::THROW_MOVE).map(mv).collect(),
            exp_yields: Vec::new(),
            weights: Vec::new(),
            levels: Vec::new(),
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

/// What a monster does on screen this turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActKind {
    /// Walks to the act's tile.
    Walk(Walk),
    /// Strikes in a direction.
    Attack(Direction),
    /// Is struck.
    Hit,
}

/// A monster's on-screen act: the monster is found by its species and its tile after the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act {
    pub species: u16,
    pub pos: (i16, i16),
    pub kind: ActKind,
}

/// The directions in the order of `EntityInfo::direction` (0 is south, as `Direction`).
const DIRECTIONS: [Direction; 8] = [
    Direction::Down,
    Direction::DownRight,
    Direction::Right,
    Direction::UpRight,
    Direction::Up,
    Direction::UpLeft,
    Direction::Left,
    Direction::DownLeft,
];

/// The farthest a wild monster is taken to have moved in one turn when pairing it.
const PAIR_RANGE: i32 = 4;

/// Each of the nine first-dungeon statuses: the message when it starts, when it ends.
const STATUS_TEXT: [(&str, &str); 9] = [
    ("is paralyzed!", "is no longer paralyzed."),
    ("is burned!", "is no longer burned."),
    ("fell asleep!", "woke up."),
    ("flinched!", "recovered from flinching."),
    ("is whiffing!", "is no longer whiffing."),
    ("is focused!", "lost its focus."),
    ("is enraged!", "calmed down."),
    ("was seeded!", "is no longer seeded."),
    ("is slowed down!", "is no longer slowed."),
];

/// Which of the nine statuses `info` has, in the order of [`STATUS_TEXT`].
fn statuses(info: &EntityInfo) -> [bool; 9] {
    let s = &info.status;
    [
        s.burn_class == rescue_sim::status::PARALYSIS,
        s.burn_class == rescue_sim::status::BURN,
        s.sleep != 0,
        s.cringe != 0,
        s.sure_shot == rescue_sim::status::WHIFFER,
        s.sure_shot == rescue_sim::status::FOCUS_ENERGY,
        s.enraged != 0,
        s.leech_seed != 0,
        info.speed_down.iter().any(|&c| c > 0),
    ]
}

/// Whether the monster has any of the nine statuses: the sprite then carries a marker.
pub fn has_status(info: &EntityInfo) -> bool {
    statuses(info).contains(&true)
}

/// The lines for the statuses that started or ended between `before` and `after`.
// shortcut: placeholder text; the ROM's status messages are patched out in the trace and their
// ids are open (docs/re/status.md), so they are not read.
fn status_messages(who: &str, before: &EntityInfo, after: &EntityInfo) -> Vec<String> {
    let (b, a) = (statuses(before), statuses(after));
    (0..9)
        .filter(|&i| b[i] != a[i])
        .map(|i| {
            let (start, end) = STATUS_TEXT[i];
            format!("{who} {}", if a[i] { start } else { end })
        })
        .collect()
}

/// Pairs each monster of `before` with its match in `after`: the nearest of the same species
/// within `PAIR_RANGE` tiles, not matched twice. `None` when it is gone.
fn pair<'a>(
    before: &'a [Monster],
    after: &'a [Monster],
) -> Vec<(&'a Monster, Option<&'a Monster>)> {
    let mut taken = vec![false; after.len()];
    let dist = |a: (i16, i16), b: (i16, i16)| {
        i32::from(a.0.abs_diff(b.0)).max(i32::from(a.1.abs_diff(b.1)))
    };
    before
        .iter()
        .map(|b| {
            let best = after
                .iter()
                .enumerate()
                .filter(|(j, a)| {
                    !taken[*j]
                        && a.info.species == b.info.species
                        && dist(a.pos, b.pos) <= PAIR_RANGE
                })
                .min_by_key(|(_, a)| dist(a.pos, b.pos));
            if let Some((j, _)) = best {
                taken[j] = true;
            }
            (b, best.map(|(j, _)| &after[j]))
        })
        .collect()
}

/// The straight direction from `from` to `to` and the tiles between them. `None` when they are
/// the same tile or not on a line of the eight directions.
fn straight(from: (i16, i16), to: (i16, i16)) -> Option<(Direction, u32)> {
    let (dx, dy) = (
        i32::from(to.0) - i32::from(from.0),
        i32::from(to.1) - i32::from(from.1),
    );
    if (dx, dy) == (0, 0) || (dx != 0 && dy != 0 && dx.abs() != dy.abs()) {
        return None;
    }
    let dir = DIRECTIONS
        .into_iter()
        .find(|d| d.delta() == (dx.signum(), dy.signum()))?;
    Some((dir, dx.abs().max(dy.abs()).unsigned_abs()))
}

/// The acts and log lines of the monsters in a turn that is not the leader's own attack: the
/// wild monsters and the team's partners (slots after the leader) walk or strike, whoever is
/// struck reacts, and the team's damage and the nine statuses are logged. A monster strikes
/// when it stood still facing a monster that was hurt or defeated.
// shortcut: the sim reports no per-hit events, so an attacker is a monster that stood still
// facing a hurt neighbour; a move that hurt a monster standing elsewhere is not shown.
pub fn monster_acts(
    team: (&[Monster], &[Monster]),
    wild: (&[Monster], &[Monster]),
) -> (Vec<Act>, Vec<String>) {
    let (mut acts, mut lines) = (Vec::new(), Vec::new());
    let mut hurt = Vec::new();
    // (name, before, after, is the team's, is drawn by `monster_acts`)
    let mut seen: Vec<(String, &Monster, &Monster, bool, bool)> = Vec::new();
    for (i, (b, a)) in team.0.iter().zip(team.1).enumerate() {
        let who = if i == 0 { "The leader" } else { "The partner" };
        seen.push((who.into(), b, a, true, i > 0));
    }
    for (b, a) in pair(wild.0, wild.1) {
        let who = format!("Monster {}", b.info.species);
        match a {
            Some(a) => seen.push((who, b, a, false, true)),
            None => hurt.push(b.pos),
        }
    }
    for (who, b, a, team_side, _) in &seen {
        if a.info.hp < b.info.hp {
            hurt.extend([b.pos, a.pos]);
            if *team_side {
                lines.push(format!("{who} took {} damage.", b.info.hp - a.info.hp));
            }
        }
        lines.extend(status_messages(who, &b.info, &a.info));
    }
    for (who, b, a, _, _) in seen.iter().filter(|s| s.4) {
        let (species, pos) = (a.info.species, a.pos);
        let hit = a.info.hp < b.info.hp;
        if hit {
            acts.push(Act {
                species,
                pos,
                kind: ActKind::Hit,
            });
        }
        if let Some((dir, legs)) = straight(b.pos, a.pos) {
            let from = (i32::from(b.pos.0), i32::from(b.pos.1));
            if let Some(walk) = Walk::new(from, dir, legs) {
                acts.push(Act {
                    species,
                    pos,
                    kind: ActKind::Walk(walk),
                });
            }
        } else if b.pos == a.pos && !hit {
            let dir = DIRECTIONS[usize::from(a.info.direction) % 8];
            let (dx, dy) = dir.delta();
            let target = (pos.0 + dx as i16, pos.1 + dy as i16);
            if hurt.contains(&target) {
                lines.push(format!("{who} attacked!"));
                acts.push(Act {
                    species,
                    pos,
                    kind: ActKind::Attack(dir),
                });
            }
        }
    }
    (acts, lines)
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
                status: Default::default(),
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

    #[test]
    fn a_wild_monster_walks_and_one_facing_a_hurt_leader_strikes() {
        let leader = mon((5, 5), 25, 30);
        let mut hurt = leader.clone();
        hurt.info.hp = 20;
        let walker = mon((8, 5), 7, 30);
        let mut moved = walker.clone();
        moved.pos = (7, 5);
        let mut biter = mon((5, 6), 9, 30);
        biter.info.direction = 4; // up, at the leader
        let (acts, lines) = monster_acts(
            (&[leader], &[hurt]),
            (&[walker, biter.clone()], &[moved, biter]),
        );
        assert!(
            acts.iter()
                .any(|a| a.species == 7 && matches!(a.kind, ActKind::Walk(_)))
        );
        assert!(
            acts.iter()
                .any(|a| a.species == 9 && a.kind == ActKind::Attack(Direction::Up))
        );
        assert_eq!(
            lines,
            vec!["The leader took 10 damage.", "Monster 9 attacked!"]
        );
    }

    #[test]
    fn a_status_that_starts_or_ends_is_logged_and_marked() {
        let calm = mon((3, 3), 7, 30);
        let mut burned = calm.clone();
        burned.info.status.burn_class = rescue_sim::status::BURN;
        assert!(has_status(&burned.info) && !has_status(&calm.info));
        let (_, lines) = monster_acts(
            (&[], &[]),
            (std::slice::from_ref(&calm), std::slice::from_ref(&burned)),
        );
        assert_eq!(lines, vec!["Monster 7 is burned!"]);
        let (_, lines) = monster_acts((&[], &[]), (&[burned], &[calm]));
        assert_eq!(lines, vec!["Monster 7 is no longer burned."]);
    }
}
