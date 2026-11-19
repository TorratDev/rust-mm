//! The combat replay (#424): the leader's Regular Attacks on a floor, with the target's HP and
//! the leader's HP compared on each row. The sim runs the attack with its combat tables (read
//! from the dump here, as `apps/game/src/combat.rs` reads them), and the other monsters' turns
//! are not run (#73). So the target and the leader are set from the trace before each row, and
//! a row's HP change that a monster caused is not compared.

use rescue_data::combat::CombatTables;
use rescue_data::moves::{Move, parse_moves};
use rescue_data::species::{LevelData, Species, parse_level_data, parse_species};
use rescue_formats::archive::Archive;
use rescue_nds::binary::{binary_bytes, overlay_data, read_overlay_tables};
use rescue_nds::blz;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_sim::combat::CombatData;
use rescue_sim::command::{Command, Direction, Outcome};
use rescue_sim::floor_gen::FloorProperties;
use rescue_sim::state::{GameState, MoveSlot, Snapshot};
use rescue_trace::{Entity, TurnRow};

use crate::actions::command_for;
use crate::start::{RowError, state_from_row, team_index};
use crate::{
    Divergence, FieldDiff, HarnessError, Replay, check, clock_check, compare_monster, push,
};

/// The target's species, as the first-floor capture attacks it (`wild[0]` of the first row).
// shortcut: the stats below are pinned to this species, and the target is taken from the first
// row; a capture with another target needs its own stats.
pub const TARGET_SPECIES: u16 = 290;

/// The leader's attack and defence at its level, read once from the emulator on the first-floor
/// save (`stats.py`, #424 u1). The trace does not carry them and the species table gives only
/// the base values.
// shortcut: the level-5 values are pinned from one emulator read; the replay fails if they are
// wrong, because the damage depends on them. Upgrade by reading the entity info from the save.
const LEADER_STATS: ([u8; 2], [u8; 2]) = ([10, 10], [9, 9]);
/// The target's attack and defence, as `LEADER_STATS` (only its defence is used by the attack).
const TARGET_STATS: ([u8; 2], [u8; 2]) = ([4, 6], [5, 5]);

/// The combat tables of the sim's attack and the species table, owned. Built from the dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatSetup {
    tables: CombatTables,
    moves: Vec<Move>,
    exp_yields: Vec<i32>,
    weights: Vec<u16>,
    levels: Vec<Vec<LevelData>>,
    species: Vec<Species>,
}

impl CombatSetup {
    /// Reads the tables from the dump `rom`. `leader` is the leader's species: its level table
    /// is the one loaded (as `apps/game/src/combat.rs`).
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
        Ok(Self {
            tables,
            moves,
            exp_yields,
            weights,
            levels,
            species,
        })
    }

    /// The sim's view of the tables.
    fn data(&self) -> CombatData<'_> {
        CombatData {
            tables: &self.tables,
            moves: &self.moves,
            exp_yields: &self.exp_yields,
            weights: &self.weights,
            levels: &self.levels,
        }
    }
}

/// Replays the trace's attacks with the sim's combat, from its first row. Each row is set from
/// the trace (the leader's HP when a monster hit it, the target's position), compared on the
/// leader (every field) and on the target (species, level, HP and max HP), then its actions are
/// applied. The clock rules are those of [`crate::replay`].
///
/// An attack with no monster on its tile is not modelled by the sim (`regular_attack`), so it is
/// applied as a wait: the game's turn passes either way.
// shortcut: the game's empty-tile attack is not read, so Wait stands in for it; the clock then
// shows whether the turn is the same.
pub fn replay_combat(
    rows: &[TurnRow],
    properties: Option<&FloorProperties>,
    setup: &CombatSetup,
) -> Result<Replay, HarnessError> {
    let Some(first) = rows.first() else {
        return Ok(Replay::Match { rows: 0 });
    };
    let mut state = state_from_row(first, properties).map_err(HarnessError::Start)?;
    prepare(&mut state, &setup.species)?;
    let target = first.wild.first().map(|e| e.species);
    let data = setup.data();
    let mut applied = false;
    for (index, row) in rows.iter().enumerate() {
        let number = index + 1;
        let previous = index.checked_sub(1).and_then(|p| rows.get(p));
        sync(&mut state, row, previous, target)?;
        let diffs = clock_check(&state, row, applied, || {
            compare_combat(row, &state.snapshot(), target)
        });
        if !diffs.is_empty() {
            return Ok(Replay::Divergence(Divergence {
                row: number,
                frame: row.frame,
                fractional_turn: row.fractional_turn,
                diffs,
            }));
        }
        for (position, action) in row.actions.iter().enumerate() {
            let command = command_for(action).map_err(|source| HarnessError::Action {
                row: number,
                index: position + 1,
                source,
            })?;
            applied = true;
            let command = match command {
                Command::Attack(dir) if !monster_at(&state, dir) => Command::Wait,
                other => other,
            };
            if state.step_with(command, &data) == Outcome::NotModelled {
                return Ok(Replay::SimGap {
                    row: number,
                    command: crate::actions::command_name(&command),
                });
            }
        }
    }
    Ok(Replay::Match { rows: rows.len() })
}

/// The wild monsters of the first-floor capture as the game holds them at the first row: the
/// species, its attack and defence, and its first move (id, PP) read from the emulator
/// (`~/rescue-re/scratch/505/probe2.py`, frame 455, #505). Slot flags `05`: holds a move, AI on.
// shortcut: pinned from one emulator read of the first-floor save, as `TARGET_STATS`; a capture
// with other wild species needs its own entries. Upgrade by reading the spawn's learnset.
const WILD_PINS: [WildPin; 2] = [
    WildPin {
        species: 290,
        stats: ([4, 6], [5, 5]),
        mv: (0x9A, 22),
    },
    WildPin {
        species: 191,
        stats: ([5, 5], [4, 4]),
        mv: (0x87, 18),
    },
];

/// One entry of [`WILD_PINS`].
struct WildPin {
    species: u16,
    stats: ([u8; 2], [u8; 2]),
    mv: (u16, u8),
}

/// Replays the enemies trace with the wild turns on the combat path (#505): the start state's
/// wild monsters hold their real stats and moves ([`WILD_PINS`]), the partner and leader their
/// measured stats, and every command runs through `step_with`, so a wild attack on the team
/// deals its damage. Compared as [`crate::replay`] with [`Scope::All`].
pub fn replay_enemies(
    rows: &[TurnRow],
    properties: Option<&FloorProperties>,
    setup: &CombatSetup,
) -> Result<Replay, HarnessError> {
    let Some(first) = rows.first() else {
        return Ok(Replay::Match { rows: 0 });
    };
    let mut state = state_from_row(first, properties).map_err(HarnessError::Start)?;
    prepare(&mut state, &setup.species)?;
    pin_wild(&mut state)?;
    crate::replay_from(state, rows, crate::Scope::All, Some(&setup.data()))
}

/// The partner's attack and defence at level 5 (species 4), read from the emulator with the wild
/// pins (frame 455). The trace does not carry them.
// shortcut: pinned from one emulator read, as `LEADER_STATS`.
const PARTNER_STATS: ([u8; 2], [u8; 2]) = ([11, 11], [8, 8]);

/// The partner's moves, Growl (pp 20) and Scratch (pp 22), flags `05`, read from the emulator
/// with the wild pins (#507). The trace does not carry them.
// shortcut: pinned from one emulator read, as `PARTNER_STATS`.
const PARTNER_MOVES: [(u16, u8); 2] = [(0x0D9, 20), (0x104, 22)];

/// Gives the partner its stats and moves and each wild monster of [`WILD_PINS`] its stats and its move.
fn pin_wild(state: &mut GameState) -> Result<(), HarnessError> {
    let mut snapshot = state.snapshot();
    let Some(dungeon) = snapshot.dungeon.as_mut() else {
        return Ok(());
    };
    for monster in dungeon.team.iter_mut().filter(|m| !m.info.is_team_leader) {
        (monster.info.attack, monster.info.defence) = PARTNER_STATS;
        for (slot, (id, pp)) in monster.info.moves.iter_mut().zip(PARTNER_MOVES) {
            *slot = MoveSlot {
                flags: MoveSlot::HOLDS_MOVE | 0x04,
                flags2: 0,
                id,
                pp,
                ginseng: 0,
            };
        }
    }
    for monster in dungeon.wild.iter_mut() {
        let info = &mut monster.info;
        let Some(pin) = WILD_PINS.iter().find(|p| p.species == info.species) else {
            continue;
        };
        (info.attack, info.defence) = pin.stats;
        info.moves[0] = MoveSlot {
            flags: MoveSlot::HOLDS_MOVE | 0x04,
            flags2: 0,
            id: pin.mv.0,
            pp: pin.mv.1,
            ginseng: 0,
        };
    }
    *state = restore(&snapshot)?;
    Ok(())
}

/// The start state's monsters get the fields the row does not carry: the species' types and
/// abilities, its regeneration speed for team members, and the measured attack and defence.
fn prepare(state: &mut GameState, species: &[Species]) -> Result<(), HarnessError> {
    let mut snapshot = state.snapshot();
    let Some(dungeon) = snapshot.dungeon.as_mut() else {
        return Ok(());
    };
    for monster in dungeon.team.iter_mut().chain(dungeon.wild.iter_mut()) {
        let info = &mut monster.info;
        let Some(sp) = species.get(usize::from(info.species)) else {
            continue;
        };
        info.apply_species(sp);
        if !info.not_team_member {
            info.regen_speed = sp.regen_speed;
        }
        if info.is_team_leader {
            (info.attack, info.defence) = LEADER_STATS;
        } else if info.species == TARGET_SPECIES {
            (info.attack, info.defence) = TARGET_STATS;
        }
    }
    *state = restore(&snapshot)?;
    Ok(())
}

/// Sets the sim from the trace before the row is compared: the leader's HP when the trace's HP
/// changed since the previous row (a monster's hit, #73, which the sim does not run), and the
/// target's position (wild moves are not run, #73). Nothing else is copied from the trace.
fn sync(
    state: &mut GameState,
    row: &TurnRow,
    previous: Option<&TurnRow>,
    target: Option<i16>,
) -> Result<(), HarnessError> {
    let leader_hp = |r: &TurnRow| r.team[0].map(|e| e.hp);
    let hp_changed = previous.is_some_and(|p| leader_hp(p) != leader_hp(row));
    let target_pos = trace_target(row, target).map(|e| (e.x, e.y));
    if !hp_changed && target_pos.is_none() {
        return Ok(());
    }
    let mut snapshot = state.snapshot();
    let Some(dungeon) = snapshot.dungeon.as_mut() else {
        return Ok(());
    };
    if let (true, Some(hp), Some(leader)) = (hp_changed, leader_hp(row), dungeon.team.first_mut()) {
        leader.info.hp = hp;
    }
    if let (Some(pos), Some(monster)) = (target_pos, dungeon.wild.first_mut()) {
        monster.pos = pos;
    }
    *state = restore(&snapshot)?;
    Ok(())
}

/// The target as the trace shows it on this row: `wild[0]` when it has the target's species.
fn trace_target(row: &TurnRow, target: Option<i16>) -> Option<Entity> {
    row.wild
        .first()
        .copied()
        .filter(|e| Some(e.species) == target)
}

/// Compares the leader (every field, as [`crate::compare_leader`]) and the target (its species,
/// level, HP and max HP: its position is set from the trace, so it is not a check). Absence of the
/// target on either side is a difference, so the kill is checked from the row after it.
fn compare_combat(row: &TurnRow, state: &Snapshot, target: Option<i16>) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    let Some(dungeon) = &state.dungeon else {
        push(&mut diffs, "dungeon", "present", "absent");
        return diffs;
    };
    let index = team_index(&row.team);
    let leader = index[0].and_then(|i| dungeon.team.get(i));
    compare_monster(&mut diffs, "team[0]", row.team[0], leader);
    let expected = trace_target(row, target);
    let actual = dungeon
        .wild
        .first()
        .filter(|m| Some(i16::try_from(m.info.species).unwrap_or(-1)) == target);
    match (expected, actual) {
        (None, None) => {}
        (Some(_), None) => push(&mut diffs, "target", "present", "absent"),
        (None, Some(_)) => push(&mut diffs, "target", "absent", "present"),
        (Some(e), Some(m)) => {
            check(
                &mut diffs,
                "target.level",
                i64::from(e.level),
                i64::from(m.info.level),
            );
            check(
                &mut diffs,
                "target.hp",
                i64::from(e.hp),
                i64::from(m.info.hp),
            );
            check(
                &mut diffs,
                "target.maxHp",
                i64::from(e.max_hp),
                i64::from(m.info.max_hp),
            );
        }
    }
    diffs
}

/// Whether a wild monster stands on the tile next to the leader in `dir`.
fn monster_at(state: &GameState, dir: Direction) -> bool {
    let Some(dungeon) = state.dungeon() else {
        return false;
    };
    let Some(leader) = dungeon.team().first() else {
        return false;
    };
    let (dx, dy) = dir.delta();
    let (x, y) = (i32::from(leader.pos.0) + dx, i32::from(leader.pos.1) + dy);
    dungeon
        .wild()
        .iter()
        .any(|m| i32::from(m.pos.0) == x && i32::from(m.pos.1) == y)
}

/// [`GameState::restore`], with its error as a [`HarnessError`].
fn restore(snapshot: &Snapshot) -> Result<GameState, HarnessError> {
    GameState::restore(snapshot).map_err(|e| HarnessError::Start(RowError::State(e)))
}
