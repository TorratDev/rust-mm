//! Simulation state of one run, and the one entry point that changes it.
//!
//! The state holds every random generator the sim owns (the global one, the floor-seed
//! preseed generator and the current floor's generator), the floor map, and the monsters.
//! Monsters live in two `Vec`s, the team and the wild monsters, in game slot order, so
//! nothing depends on hash order. Field names and offsets follow `docs/re/structs.md`
//! (the entity and entity-info tables, and the dungeon struct). Fields the notes do not
//! record are left out, not guessed: the item and trap entities, the statuses of
//! a monster, the weather and the floor properties. The turn-loop state is the fractional-turn
//! counter, the leader's speed stage and its pending end-of-turns ([`crate::turn`]). The `turn`
//! counter is the sim's own, not a game field.
//!
//! [`GameState::step`] is the only way a command changes the state. Integer math only.

use serde::{Deserialize, Serialize};

use rescue_data::dungeon::{FLOOR_PROPERTIES, MapParam};
use rescue_data::moves::Move;
use rescue_data::species::Species;

use crate::combat::CombatData;
use crate::command::{Blocked, Command, Direction, Outcome};
use crate::floor::{self, Floor, HEIGHT, Room, Tile, WIDTH};
use crate::floor_gen::{GenState, PortedSteps, UNSET, generate_floor, properties_from_record};
use crate::rng::{DungeonPreseed, DungeonRng, GlobalRng};
use crate::turn::{FRACTIONAL_TURNS, MAX_SPEED_STAGE};

/// Team slots: `teamPokemon[4]` (`+0x11314`).
pub const TEAM_SLOTS: usize = 4;
/// Wild slots: `wildPokemon[16]` (`+0x11324`).
pub const WILD_SLOTS: usize = 16;
/// The monster-house room of a floor with none (`+0x34AC`; the name is a red-hypothesis).
pub const NO_MONSTER_HOUSE: u8 = 0xFF;

/// Why a state could not be built or restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    /// More team members than [`TEAM_SLOTS`].
    TeamTooLarge(usize),
    /// More wild monsters than [`WILD_SLOTS`].
    WildTooLarge(usize),
    /// A snapshot's tile list is not `WIDTH * HEIGHT` tiles long.
    BadTileCount(usize),
    /// A snapshot records more floors than [`MAX_RESTORED_FLOORS`] to replay.
    TooManyFloors(u32),
}

/// The generator steps the game takes between a floor's seed and `GenerateFloor`'s entry
/// (`docs/re/rng.md`, "Per-floor seed"): 1 draw in `RunDungeon_Async` and 9 in the spawn list
/// `0217AA14`. The 9 are not ported, so the count is fixed; the trace harness uses the same 10.
// shortcut: the 10 draws are counted, not modelled; the spawn-list draws (9 on floors 1 to 3)
// need their own port before a floor's layout matches a retail floor past the first.
pub const FLOOR_ENTRY_STEPS: u32 = 10;

/// One floor's properties record as `mapparam` stores it (`FLOOR_PROPERTIES` bytes). Raw bytes
/// keep the state serializable; [`properties_from_record`] reads the fields.
pub type FloorRecord = [u8; FLOOR_PROPERTIES];

/// The floor records of dungeon `dungeon` in `map`, indexed by the floor number the game uses
/// (the `mapparam` floor index). Entry 0 is the placeholder Red keeps there, so the length is
/// the floor count the loop compares against (`docs/re/turns.md`, "Floor loop exit").
// shortcut: dungeon ids 0x4B to 0x61 read mapparam group 0x34 in the game, not `dungeons[id]`,
// and have 4 floors (`docs/re/turns.md`, "Floor index convention"); upgrade when that mapping is ported.
#[must_use]
pub fn floor_records(map: &MapParam, dungeon: u8) -> Option<Vec<FloorRecord>> {
    map.dungeons
        .get(usize::from(dungeon))?
        .iter()
        .map(|entry| {
            map.floor_properties
                .get(usize::from(entry.properties))
                .copied()
        })
        .collect()
}

/// The most floors a snapshot may record. Restoring the preseed generator replays one step
/// per floor, so the bound keeps a crafted snapshot from running long.
pub const MAX_RESTORED_FLOORS: u32 = 1 << 16;

/// The entity-info fields the notes record (`docs/re/structs.md`, "Entity info"). The
/// offset is the Blue offset from `entity + 0x70`. Left out: the held item, the status
/// records, the skill bits, the AI target and the decoy and curse state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityInfo {
    /// `+0x02`: species id.
    pub species: u16,
    /// `+0x04`: apparent species.
    pub apparent_species: u16,
    /// `+0x06`: non-zero for a monster that is not a team member (a wild one).
    pub not_team_member: bool,
    /// `+0x07`: non-zero for the team leader.
    pub is_team_leader: bool,
    /// `+0x09`: level.
    pub level: u8,
    /// `+0x0C`: IQ (s16).
    pub iq: i16,
    /// `+0x0E`: current HP.
    pub hp: i16,
    /// `+0x10`: maximum HP.
    pub max_hp: i16,
    /// `+0x14`: base attack for each split (u8 pair).
    pub attack: [u8; 2],
    /// `+0x16`: base defence for each split (u8 pair).
    pub defence: [u8; 2],
    /// `+0x1C`: offensive stages for each split (s16 pair). A stage is an index 0 to 20 into
    /// the stage table, clamped by the game; 10 is neutral.
    pub offensive_stages: [i16; 2],
    /// `+0x20`: defensive stages for each split (s16 pair), as `offensive_stages`.
    pub defensive_stages: [i16; 2],
    /// `+0x24`: the attacker's accuracy stage (s16), as `offensive_stages`.
    pub accuracy_stage: i16,
    /// `+0x26`: the target's evasion stage (s16), as `offensive_stages`.
    pub evasion_stage: i16,
    /// `+0x56` and `+0x57`: the species' two types (0 is none). Set from [`Species::types`].
    pub types: [u8; 2],
    /// `+0x58` and `+0x59`: the species' two abilities. Set from [`Species::abilities`].
    pub abilities: [u8; 2],
    /// `+0xF4`: the four move slots, six bytes each.
    pub moves: [MoveSlot; 4],
    /// `+0xE4` to `+0xE8`: the five speed-up counters (turns left, 0 when free). Each nonzero
    /// one adds 1 to the speed stage (`CalcSpeedStage`).
    // shortcut: the counters are set by the caller and never counted down (the gated-pass
    // tick, `TickStatusAndHealthRegen`, is not ported), so a speed stage keeps them until
    // the status slice ports the countdown.
    pub speed_up: [u8; 5],
    /// `+0xE9` to `+0xED`: the five speed-down counters, as `speed_up`. Each nonzero one
    /// takes 1 from the speed stage.
    pub speed_down: [u8; 5],
    /// `+0x44`: facing direction (the game's direction index).
    pub direction: u8,
    /// `+0x10E` (whole) and `+0x110` (thousandths) read as one value in thousandths: 100 000
    /// is 100.000, the belly a floor starts with. Only the leader's belly is decreased.
    pub belly: i32,
    /// `+0x118`: `bellyEmpty`. Set by the end-of-turn when the belly is 0, cleared at its start;
    /// while it is set, HP regeneration is skipped.
    pub belly_empty: bool,
    /// `+0x1C4`: the HP regeneration accumulator (s16).
    pub regen_acc: i16,
    /// The species regeneration speed: `Species::regen_speed` in `rescue-data` (already shifted
    /// left by one). Read for team members only; clamped to 30 to 500 when used.
    pub regen_speed: u8,
}

/// One move slot of a monster: the six bytes at `info + 0xF4 + 6 * slot` (`docs/re/combat.md`,
/// "PP", #419 P3). Field names are ours; the byte layout is the game's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveSlot {
    /// Byte 0. Bit 0 is set when the slot holds a move (the game's test in its use loop).
    pub flags: u8,
    /// Byte 1. Bit 3 (`0x08`) is the use mark: set when the move is used, and the PP is taken
    /// at the end of the action (see [`crate::combat`]).
    pub flags2: u8,
    /// Bytes 2 and 3: the move ID, a [`rescue_data::moves::Move`] index.
    pub id: u16,
    /// Byte 4: the current PP. The maximum is the move's base PP.
    pub pp: u8,
    /// Byte 5: the Ginseng upgrade level.
    pub ginseng: u8,
}

impl MoveSlot {
    /// Bit 0 of [`flags`](Self::flags): the slot holds a move.
    pub const HOLDS_MOVE: u8 = 1;
    /// Bit 3 of [`flags2`](Self::flags2): the move was used and its PP is still to be taken.
    pub const USE_MARK: u8 = 0x08;

    /// A slot holding `mv` at full PP.
    #[must_use]
    pub fn new(mv: &Move) -> Self {
        Self {
            flags: Self::HOLDS_MOVE,
            flags2: 0,
            id: mv.id,
            pp: mv.base_pp,
            ginseng: 0,
        }
    }
}

impl EntityInfo {
    /// Copies the species' types and abilities into the info (`+0x56` to `+0x59`), as the game
    /// does when it spawns a monster (`docs/re/combat.md`, "Abilities", P6).
    pub fn apply_species(&mut self, species: &Species) {
        self.types = species.types;
        self.abilities = species.abilities;
    }

    /// Whether the monster has ability `id` in either of its two slots (the game's
    /// `AbilityIsActive` test on the pair).
    #[must_use]
    pub fn has_ability(&self, id: u8) -> bool {
        self.abilities.contains(&id)
    }
}

/// One monster: the entity's position and visibility, and its info.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monster {
    /// `+0x04`, `+0x06`: map position (`x`, `y`).
    pub pos: (i16, i16),
    /// `+0x20`: visible on the map.
    pub visible: bool,
    /// `+0x70`: the info record.
    pub info: EntityInfo,
}

/// The dungeon state of one floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dungeon {
    /// The floor map.
    pub(crate) floor: Floor,
    /// The floor's generator (`gDungeonRngState`).
    pub(crate) rng: DungeonRng,
    /// The team, in slot order; slot 0 is the leader.
    pub(crate) team: Vec<Monster>,
    /// The wild monsters, in slot order.
    pub(crate) wild: Vec<Monster>,
    /// The sim's turn counter: one per command that takes a turn.
    pub(crate) turn: u32,
    /// `fractionalTurn` (`dungeon+0x650`): the next fractional turn to run, 0 to 23.
    pub(crate) fractional_turn: u8,
    /// The leader's end-of-turns still waiting for a weather step (one per move or wait that
    /// has not reached its weather step yet). See [`crate::turn`].
    pub(crate) pending_end_of_turns: u8,
    /// The leader's species movement speed (`FUN_020023A8`, `monster_md` `+0xC`), the base term
    /// of `CalcSpeedStage`. Given at dungeon entry.
    pub(crate) leader_speed: i32,
    /// A speed stage the caller observed, 0 to 4, set by [`GameState::set_speed_stage`]. `None`
    /// computes the stage from `leader_speed` and the leader's counters.
    pub(crate) speed_stage: Option<u8>,
    /// `+0x34AA`: the Kecleon shop pass ran on this floor.
    shop_pass: bool,
    /// `+0x34AC`: the monster-house room, or [`NO_MONSTER_HOUSE`].
    monster_house_room: u8,
    /// `+0xC0B8`, `+0xC0BA`: the leader's start, `None` when the game stores -1.
    player_spawn: Option<(i16, i16)>,
    /// `+0xC0BC`, `+0xC0BE`: the stairs spawn, `None` when the game stores -1.
    stairs_spawn: Option<(i16, i16)>,
    /// `dungeon+1`: the leader stands on stairs and its next input takes them (see
    /// [`crate::turn`]).
    pub(crate) stairs_pending: bool,
    /// `+0x15FA1`: the darkness setting `D` of the floor (`props +0x16 & 3`), read by the
    /// corridor range (see [`crate::visibility`]). 0 is a lit floor.
    pub(crate) darkness: u8,
}

impl Dungeon {
    /// The floor map.
    #[must_use]
    pub const fn floor(&self) -> &Floor {
        &self.floor
    }

    /// The floor's generator.
    #[must_use]
    pub const fn rng(&self) -> &DungeonRng {
        &self.rng
    }

    /// The team, slot 0 first (the leader).
    #[must_use]
    pub fn team(&self) -> &[Monster] {
        &self.team
    }

    /// The wild monsters, in slot order.
    #[must_use]
    pub fn wild(&self) -> &[Monster] {
        &self.wild
    }

    /// The turn counter.
    #[must_use]
    pub const fn turn(&self) -> u32 {
        self.turn
    }

    /// The next fractional turn to run, 0 to 23 (`dungeon+0x650`).
    #[must_use]
    pub const fn fractional_turn(&self) -> u8 {
        self.fractional_turn
    }

    /// The speed stage the caller set, or `None` when it is computed (see
    /// [`GameState::set_speed_stage`]).
    #[must_use]
    pub const fn speed_stage(&self) -> Option<u8> {
        self.speed_stage
    }

    /// Whether the Kecleon shop pass ran on this floor (`+0x34AA`).
    #[must_use]
    pub const fn shop_pass(&self) -> bool {
        self.shop_pass
    }

    /// The monster-house room, or [`NO_MONSTER_HOUSE`] (`+0x34AC`).
    #[must_use]
    pub const fn monster_house_room(&self) -> u8 {
        self.monster_house_room
    }

    /// The leader's start (`+0xC0B8`), `None` when unset.
    #[must_use]
    pub const fn player_spawn(&self) -> Option<(i16, i16)> {
        self.player_spawn
    }

    /// The stairs spawn (`+0xC0BC`), `None` when unset.
    #[must_use]
    pub const fn stairs_spawn(&self) -> Option<(i16, i16)> {
        self.stairs_spawn
    }

    /// Whether the leader's next input takes the stairs it stands on (`dungeon+1`).
    #[must_use]
    pub const fn stairs_pending(&self) -> bool {
        self.stairs_pending
    }
}

impl Dungeon {
    /// The monster standing on (`x`, `y`), if any: team or wild.
    fn occupied(&self, x: i16, y: i16) -> bool {
        self.team
            .iter()
            .chain(self.wild.iter())
            .any(|m| m.pos == (x, y))
    }

    /// The tile the leader would step to in `dir`, or the reason it cannot. This is the regular
    /// mobility rule (`docs/re/turns.md`, `CanMoveInDirection` and the walkable-neighbour
    /// masks): the target must be NORMAL terrain, and a diagonal step needs both orthogonal
    /// neighbours to be open, so it never cuts a wall corner. Other mobility classes (water,
    /// lava, crevice) and the status and held-item overrides are not modelled, so the leader
    /// moves as a regular monster. Checks only; the caller moves the leader.
    fn leader_target(&self, dir: Direction) -> Result<(i16, i16), Blocked> {
        let Some(leader) = self.team.first() else {
            return Err(Blocked::NoLeader);
        };
        let (lx, ly) = (i32::from(leader.pos.0), i32::from(leader.pos.1));
        let (dx, dy) = dir.delta();
        let (x, y) = (lx + dx, ly + dy);
        let Some(tile) = self.floor.tile(x, y) else {
            return Err(Blocked::OutOfBounds);
        };
        if tile.terrain & floor::TERRAIN_IMPASSABLE_WALL != 0 {
            return Err(Blocked::Wall);
        }
        // The map is at most 56 by 32, so the tile test above keeps both coordinates small.
        let target = (x as i16, y as i16);
        // A team ally on the target is swapped with (see `move_leader`); any other monster blocks.
        if !self.team.iter().skip(1).any(|m| m.pos == target) && self.occupied(target.0, target.1) {
            return Err(Blocked::Occupied);
        }
        if tile.terrain & floor::TERRAIN_TYPE_MASK != TERRAIN_NORMAL {
            return Err(Blocked::Terrain);
        }
        if dx != 0 && dy != 0 {
            // A wall (or the map edge) on either side of the diagonal blocks the step.
            let open = |xx: i32, yy: i32| {
                self.floor.tile(xx, yy).is_some_and(|t| {
                    t.terrain & floor::TERRAIN_IMPASSABLE_WALL == 0
                        && t.terrain & floor::TERRAIN_TYPE_MASK != 0
                })
            };
            if !open(lx + dx, ly) || !open(lx, ly + dy) {
                return Err(Blocked::CornerCut);
            }
        }
        Ok(target)
    }
}

/// The eight directions in the game's table order (`Direction` declaration order).
pub(crate) const DIRECTIONS: [Direction; 8] = [
    Direction::Down,
    Direction::DownRight,
    Direction::Right,
    Direction::UpRight,
    Direction::Up,
    Direction::UpLeft,
    Direction::Left,
    Direction::DownLeft,
];

/// The game's index (0 to 7) of `dir` in [`DIRECTIONS`].
fn index_of(dir: Direction) -> usize {
    DIRECTIONS.iter().position(|&d| d == dir).unwrap_or(0)
}

/// The direction `steps` places round the table from `dir` (7 is one step back).
fn turned(dir: Direction, steps: usize) -> Direction {
    DIRECTIONS[(index_of(dir) + steps) % DIRECTIONS.len()]
}

/// The natural-junction terrain bit (`0x8`), a room tile the run stops on (`docs/re/turns.md`,
/// "Run continuation checks", check 5; the name is the note's).
const TERRAIN_NATURAL_JUNCTION: u16 = 0x8;

impl Dungeon {
    /// Moves the leader onto `target` in one leader turn. A team ally on `target` swaps places
    /// with the leader and faces back, `(dir + 4) & 7` (`docs/re/turns.md`, "Swap flags").
    // shortcut: the other preconditions of the swap (`FUN_02165C80`: the statuses, confusion,
    // `+0x99`, `0215A23C` and the walk check `02181264`) and the `1 << 5` normal-swap flag
    // (`docs/re/turns.md`, hypothesis) are not checked. The sim has no statuses or those fields,
    // so any team ally on a plain tile swaps. The Yes/No prompt is not modelled. Upgrade with
    // #415, once statuses and the flag's writer are in.
    fn move_leader(&mut self, dir: Direction, target: (i16, i16)) {
        self.take_leader_turn(|d| {
            let Some(leader) = d.team.first_mut() else {
                return;
            };
            let from = leader.pos;
            leader.pos = target;
            if let Some(ally) = d.team.iter_mut().skip(1).find(|m| m.pos == target) {
                ally.pos = from;
                ally.info.direction = u8::try_from((index_of(dir) + 4) % 8).unwrap_or(0);
            }
        });
    }

    /// Runs the leader from the command's first step `first`, then takes each further step the
    /// run checks allow. A belly of 0 at input makes the command one step (`docs/re/turns.md`,
    /// "Run flag and belly test").
    fn run_leader(&mut self, dir: Direction, first: (i16, i16)) {
        let running = self.team.first().is_some_and(|l| l.info.belly != 0);
        self.move_leader(dir, first);
        if !running {
            return;
        }
        while let Some(next) = self.run_ahead(dir) {
            self.move_leader(dir, next);
        }
    }

    /// The run continuation checks (`FUN_ov_0001__02165F78`, `docs/re/turns.md`, "Run
    /// continuation checks (#181 P1)"), in the game's order. Returns the tile ahead when the run
    /// goes on, and `None` when a stop condition holds.
    // shortcut: check 2 (an object on the leader's tile), check 7 (items in the 3 by 3 scan) and
    // check 8 (visible traps) need items and traps, which the sim does not model; the trap test
    // also needs visibility (#407). A run passes those tiles. Upgrade with #415, once items and
    // traps are in.
    // shortcut: the other StopRunning events (a monster's attack, a trap, a menu) are not modelled.
    fn run_ahead(&self, dir: Direction) -> Option<(i16, i16)> {
        let leader = self.team.first()?;
        let (lx, ly) = (i32::from(leader.pos.0), i32::from(leader.pos.1));
        let (dx, dy) = dir.delta();
        let at = |x: i32, y: i32| self.floor.tile(x, y);
        // The helper of `FUN_021821E4`: off the floor, a wall, or terrain that is not plain ground.
        let blocked = |x: i32, y: i32| {
            at(x, y).is_none_or(|t| {
                t.terrain & floor::TERRAIN_IMPASSABLE_WALL != 0
                    || t.terrain & floor::TERRAIN_TYPE_MASK != TERRAIN_NORMAL
            })
        };
        let monster_at = |x: i32, y: i32| {
            i16::try_from(x)
                .ok()
                .zip(i16::try_from(y).ok())
                .is_some_and(|(x, y)| self.occupied(x, y))
        };
        let (ax, ay) = (lx + dx, ly + dy);
        let own = at(lx, ly)?;
        let ahead = at(ax, ay)?;
        // The stairs action waits for the leader's next input, and a belly at 0 stops a run.
        if leader.info.belly_empty || self.stairs_pending {
            return None;
        }
        // Check 3: the leader's own tile is plain ground.
        if blocked(lx, ly) {
            return None;
        }
        // Check 4: the tile ahead has no monster and the regular step to it is allowed.
        if monster_at(ax, ay) || self.leader_target(dir).is_err() {
            return None;
        }
        // Check 5: from a corridor into a room, or on a natural junction in a room.
        if own.room == floor::NO_ROOM {
            if ahead.room != floor::NO_ROOM {
                return None;
            }
        } else if own.terrain & TERRAIN_NATURAL_JUNCTION != 0 {
            return None;
        }
        // Check 6: a monster or stairs on the three tiles ahead (dir - 1, dir, dir + 1).
        for steps in [7, 0, 1] {
            let (ex, ey) = {
                let (ddx, ddy) = turned(dir, steps).delta();
                (lx + ddx, ly + ddy)
            };
            if monster_at(ex, ey)
                || at(ex, ey).is_some_and(|t| t.terrain & floor::TERRAIN_STAIRS != 0)
            {
                return None;
            }
        }
        // Check 9: an open cardinal neighbour that is not behind the leader is in another room.
        for side in [
            Direction::Down,
            Direction::Right,
            Direction::Up,
            Direction::Left,
        ] {
            if side == turned(dir, 4) {
                continue;
            }
            let (sx, sy) = side.delta();
            let (nx, ny) = (lx + sx, ly + sy);
            if own.room != floor::NO_ROOM
                && !blocked(nx, ny)
                && at(nx, ny).is_some_and(|t| t.room != own.room)
            {
                return None;
            }
        }
        // Check 10: on a straight run, an open tile beside the leader with a blocked corner (ahead
        // or behind, on that side) is a side opening.
        if dx == 0 || dy == 0 {
            for sign in [1, -1] {
                let (px, py) = (dy * sign, dx * sign);
                if !blocked(lx + px, ly + py)
                    && (blocked(ax + px, ay + py) || blocked(lx - dx + px, ly - dy + py))
                {
                    return None;
                }
            }
        }
        // Check 11: the run goes on, onto the tile ahead.
        i16::try_from(ax).ok().zip(i16::try_from(ay).ok())
    }
}

impl Dungeon {
    /// The leader's speed stage for the gate: the caller's stage if one was set, else
    /// [`Self::calc_speed_stage`].
    pub(crate) fn leader_stage(&self) -> u8 {
        self.speed_stage.unwrap_or_else(|| self.calc_speed_stage())
    }

    /// `CalcSpeedStage` (`ov_0001::02181E58`) for the leader: the species movement speed, plus
    /// one per nonzero speed-up counter, minus one per nonzero speed-down counter, clamped to 0
    /// to 4.
    fn calc_speed_stage(&self) -> u8 {
        let (up, down) = match self.team.first() {
            Some(leader) => (
                leader
                    .info
                    .speed_up
                    .iter()
                    .map(|&c| i32::from(c != 0))
                    .sum::<i32>(),
                leader
                    .info
                    .speed_down
                    .iter()
                    .map(|&c| i32::from(c != 0))
                    .sum::<i32>(),
            ),
            None => (0, 0),
        };
        // shortcut: the terms that need state the sim does not have are left out. The -1 for
        // paralysis (`+0x9B` == 4) needs the status records; the +1 for snow on an ice type
        // needs the weather (`FUN_02195C48`); the +1 for Deoxys in Speed form (apparent species
        // 419) needs its constant, which the export does not resolve; the +1 for a Kecleon
        // that stole (species 380, not a team member, `gDungeon+0x65E`) needs the stolen-item
        // flag. Each goes in with its slice.
        let stage = self
            .leader_speed
            .saturating_add(up)
            .saturating_sub(down)
            .clamp(0, i32::from(MAX_SPEED_STAGE));
        u8::try_from(stage).unwrap_or(MAX_SPEED_STAGE)
    }
}

/// Terrain type of ordinary ground, the only type a regular monster may stand on.
const TERRAIN_NORMAL: u16 = floor::TERRAIN_FLOOR;

/// A spawn position as the game stores it: `None` for the unset `(-1, -1)`.
fn spawn_or_none(pos: (i16, i16)) -> Option<(i16, i16)> {
    (pos != UNSET).then_some(pos)
}

/// Places a team on a new floor: the leader on the floor's start (pair A), and each other member
/// on the first open tile in map order that no member stands on.
// shortcut: the game's placement of the other members is not ported, so the first free open tile
// stands in for it.
fn place_team(team: &mut [Monster], floor: &Floor, start: (i16, i16)) {
    let mut taken = Vec::with_capacity(team.len());
    if let Some(leader) = team.first_mut() {
        leader.pos = start;
        taken.push(start);
    }
    for member in team.iter_mut().skip(1) {
        let free = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x as i16, y as i16)))
            .find(|&(x, y)| {
                !taken.contains(&(x, y))
                    && floor.tile(i32::from(x), i32::from(y)).is_some_and(|t| {
                        t.terrain & floor::TERRAIN_IMPASSABLE_WALL == 0
                            && t.terrain & floor::TERRAIN_TYPE_MASK == TERRAIN_NORMAL
                    })
            });
        if let Some(spot) = free {
            member.pos = spot;
            taken.push(spot);
        }
    }
}

/// Everything the sim owns for one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    /// The global generator (`sPRNGState`).
    global: GlobalRng,
    /// The seed the preseed generator was made from.
    run_seed: u32,
    /// How many floors have been entered; each drew one floor seed from the preseed generator.
    floors_entered: u32,
    /// Produces each floor's seed (`gDungeonRngPreseedState`), made from [`Self::run_seed`]
    /// and advanced once per floor entered.
    preseed: DungeonPreseed,
    /// The current dungeon, if one is loaded.
    dungeon: Option<Dungeon>,
    /// The run's dungeon, one record per floor number, placeholder first (see
    /// [`floor_records`]). Set at construction with [`GameState::with_floors`]. Its length is the
    /// floor count: the stairs of the last floor clear the dungeon.
    // shortcut: the count is the length of the mapparam floor list, not the ARM9 count from
    // `FUN_020061D8` (`docs/re/turns.md`, hypothesis). The two are compared on the byte table at
    // `DAT_02006200` before the count is trusted.
    floors: Vec<FloorRecord>,
}

impl GameState {
    /// A run seeded from one value: the global generator takes `seed` as its state, and the
    /// preseed generator takes the same value. Seeding both generators from one value is a
    /// sim choice, not the game's seeding (the game seeds the global generator from the
    /// console's six bytes and the preseed from the run). No dungeon is loaded.
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            global: GlobalRng::from_state(seed),
            run_seed: seed,
            floors_entered: 0,
            preseed: DungeonPreseed::new(seed),
            dungeon: None,
            floors: Vec::new(),
        }
    }

    /// The run with its dungeon's floor records (see [`floor_records`]), given before any
    /// command. Without them the floor count is 0, so the first stairs clear the dungeon.
    #[must_use]
    pub fn with_floors(mut self, floors: Vec<FloorRecord>) -> Self {
        self.floors = floors;
        self
    }

    /// The floor records of the run's dungeon.
    #[must_use]
    pub fn floors(&self) -> &[FloorRecord] {
        &self.floors
    }

    /// A run with a floor loaded: [`GameState::new`] with `seed`, then the floor. The floor's
    /// seed comes from the preseed generator, and the floor gets its own generator. Fails if
    /// the team is too large. `leader_speed` is the leader's species movement speed
    /// (`rescue_data::Species::movement_speed`), the base of the leader's speed stage.
    pub fn in_dungeon(
        seed: u32,
        floor: Floor,
        team: Vec<Monster>,
        leader_speed: i32,
    ) -> Result<Self, StateError> {
        let mut state = Self::new(seed);
        state.enter_dungeon(floor, team, leader_speed)?;
        Ok(state)
    }

    /// The global generator.
    #[must_use]
    pub const fn global(&self) -> &GlobalRng {
        &self.global
    }

    /// The seed the preseed generator was made from.
    #[must_use]
    pub const fn run_seed(&self) -> u32 {
        self.run_seed
    }

    /// How many floors have been entered.
    #[must_use]
    pub const fn floors_entered(&self) -> u32 {
        self.floors_entered
    }

    /// The current dungeon, if one is loaded.
    #[must_use]
    pub const fn dungeon(&self) -> Option<&Dungeon> {
        self.dungeon.as_ref()
    }

    /// The current dungeon, mutably. Crate-internal: the tests place wild monsters with it.
    #[cfg(test)]
    pub(crate) fn dungeon_mut(&mut self) -> Option<&mut Dungeon> {
        self.dungeon.as_mut()
    }

    /// Enters a floor (see [`GameState::in_dungeon`]). Crate-private: a floor is entered only by
    /// the constructor or through the state's own rules. The floor's darkness comes from its
    /// record (floor number `floors_entered`, see [`floor_records`]); with no record it is lit.
    pub(crate) fn enter_dungeon(
        &mut self,
        mut floor: Floor,
        team: Vec<Monster>,
        leader_speed: i32,
    ) -> Result<(), StateError> {
        if team.len() > TEAM_SLOTS {
            return Err(StateError::TeamTooLarge(team.len()));
        }
        crate::visibility::enter_floor(&mut floor);
        let floor_seed = self.preseed.next_floor_seed();
        self.floors_entered = self.floors_entered.saturating_add(1);
        let darkness = self.darkness_of_floor(self.floors_entered);
        self.dungeon = Some(Dungeon {
            floor,
            rng: DungeonRng::new(floor_seed),
            team,
            wild: Vec::new(),
            turn: 0,
            fractional_turn: 0,
            pending_end_of_turns: 0,
            leader_speed,
            speed_stage: None,
            shop_pass: false,
            monster_house_room: NO_MONSTER_HOUSE,
            player_spawn: None,
            stairs_spawn: None,
            stairs_pending: false,
            darkness,
        });
        // The arrival reveal (`docs/re/visibility.md`, P2): the leader's start is seen at once.
        if let Some(dungeon) = self.dungeon.as_mut() {
            dungeon.reveal_from_leader();
        }
        Ok(())
    }

    /// The darkness setting of floor number `floor` in the run's records (0 when there is no
    /// record for it, which is a lit floor).
    fn darkness_of_floor(&self, floor: u32) -> u8 {
        usize::try_from(floor)
            .ok()
            .and_then(|index| self.floors.get(index))
            .map_or(0, crate::visibility::darkness)
    }

    /// Sets the leader's speed stage to one the caller observed (clamped to 0 to 4). The
    /// trace harness sets each row's stage this way. Without it, the stage is computed from the
    /// species speed and the counters, as `CalcSpeedStage` does. Does nothing when no dungeon
    /// is loaded.
    pub fn set_speed_stage(&mut self, stage: u8) {
        if let Some(dungeon) = self.dungeon.as_mut() {
            dungeon.speed_stage = Some(stage.min(MAX_SPEED_STAGE));
        }
    }

    /// Applies one input command. The only entry point that changes the state.
    ///
    /// Movement and waiting take the leader's turn: the clock runs fractional turns until the
    /// leader's speed stage gives it one, then the command takes effect (see [`crate::turn`]).
    /// A refused move changes nothing and takes no time. While the leader's stairs flag is set,
    /// the command is the answer to the stairs menu: the floor changes ([`Outcome::NextFloor`],
    /// or [`Outcome::DungeonCleared`] on the last floor). Otherwise attack, items and menus are
    /// valid input and return [`Outcome::NotModelled`]. A move command (`UseMove`) runs only
    /// through [`GameState::step_with`], with the combat tables.
    pub fn step(&mut self, command: Command) -> Outcome {
        self.apply(command, None)
    }

    /// [`step`](Self::step) with the combat tables and the move table. A move command
    /// (`UseMove`) runs its damage here; without the tables it is not modelled (see
    /// [`crate::combat`]).
    pub fn step_with(&mut self, command: Command, data: &CombatData<'_>) -> Outcome {
        self.apply(command, Some(data))
    }

    fn apply(&mut self, command: Command, data: Option<&CombatData<'_>>) -> Outcome {
        // The stairs menu answers the leader's next input: confirm is given, so any command
        // takes the stairs (menus are not modelled).
        if self.dungeon.as_ref().is_some_and(|d| d.stairs_pending) {
            return self.take_stairs();
        }
        let Some(dungeon) = self.dungeon.as_mut() else {
            return Outcome::Blocked(Blocked::NoDungeon);
        };
        match command {
            Command::Move(dir) => match dungeon.leader_target(dir) {
                Ok(target) => {
                    dungeon.move_leader(dir, target);
                    Outcome::Applied
                }
                Err(blocked) => Outcome::Blocked(blocked),
            },
            Command::Run(dir) => match dungeon.leader_target(dir) {
                Ok(target) => {
                    dungeon.run_leader(dir, target);
                    Outcome::Applied
                }
                Err(blocked) => Outcome::Blocked(blocked),
            },
            Command::Wait => {
                dungeon.take_leader_turn(|_| {});
                Outcome::Applied
            }
            Command::UseMove { slot, direction } => match data {
                Some(data) => dungeon.use_move(slot, direction, data),
                None => Outcome::NotModelled,
            },
            Command::Attack(_) | Command::UseItem { .. } | Command::Menu(_) => Outcome::NotModelled,
        }
    }

    /// The stairs action: the next floor is loaded, or the dungeon is cleared when the next floor
    /// number is not below the floor count. Nothing changes unless the next floor is placed.
    fn take_stairs(&mut self) -> Outcome {
        let Some(mut leaving) = self.dungeon.clone() else {
            return Outcome::Blocked(Blocked::NoDungeon);
        };
        // Floor numbers are 1-based and the list holds the placeholder at index 0, so floor
        // `floors_entered + 1` is the next one (`docs/re/turns.md`, "Floor loop exit").
        let next = self.floors_entered.saturating_add(1);
        let Some(record) = usize::try_from(next)
            .ok()
            .and_then(|index| self.floors.get(index))
        else {
            self.dungeon = None;
            return Outcome::DungeonCleared;
        };
        // The leaving floor's end effects come first: they are part of the stairs turn, and the
        // team carries their result.
        leaving.leave_floor_by_stairs();

        // The next floor's seed is the next step of the run's preseed chain. Its generator takes
        // the fixed draws before `GenerateFloor` (`FLOOR_ENTRY_STEPS`).
        let mut preseed = self.preseed;
        let mut rng = DungeonRng::new(preseed.next_floor_seed());
        for _ in 0..FLOOR_ENTRY_STEPS {
            rng.next_u16();
        }
        let mut floor = Floor::new();
        let mut g = GenState::default();
        // shortcut: fixed rooms need the fixed-room file, which is not loaded, so the floor is
        // generated without one (as the trace harness does).
        // stop_at_standard is off, so generate_floor cannot return its Err.
        let _ = generate_floor(
            &properties_from_record(record),
            false,
            &mut floor,
            &mut rng,
            &mut g,
            &mut PortedSteps::default(),
            false,
        );
        if g.stairs_a == UNSET {
            return Outcome::NotModelled;
        }
        crate::visibility::enter_floor(&mut floor);
        place_team(&mut leaving.team, &floor, g.stairs_a);
        self.preseed = preseed;
        self.floors_entered = next;
        let darkness = self.darkness_of_floor(next);
        self.dungeon = Some(Dungeon {
            floor,
            rng,
            team: leaving.team,
            wild: Vec::new(),
            turn: 0,
            fractional_turn: 0,
            pending_end_of_turns: 0,
            leader_speed: leaving.leader_speed,
            speed_stage: leaving.speed_stage,
            shop_pass: false,
            monster_house_room: NO_MONSTER_HOUSE,
            player_spawn: spawn_or_none(g.stairs_a),
            stairs_spawn: spawn_or_none(g.stairs_b),
            stairs_pending: false,
            darkness,
        });
        if let Some(dungeon) = self.dungeon.as_mut() {
            dungeon.reveal_from_leader();
        }
        Outcome::NextFloor
    }

    /// A serde snapshot of the whole state.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            global: self.global.state(),
            run_seed: self.run_seed,
            floors_entered: self.floors_entered,
            floors: self.floors.clone(),
            dungeon: self.dungeon.as_ref().map(|d| DungeonSnapshot {
                rng: d.rng.state(),
                floor: FloorSnapshot::capture(&d.floor),
                team: d.team.clone(),
                wild: d.wild.clone(),
                turn: d.turn,
                fractional_turn: d.fractional_turn,
                pending_end_of_turns: d.pending_end_of_turns,
                leader_speed: d.leader_speed,
                speed_stage: d.speed_stage,
                shop_pass: d.shop_pass,
                monster_house_room: d.monster_house_room,
                player_spawn: d.player_spawn,
                stairs_spawn: d.stairs_spawn,
                stairs_pending: d.stairs_pending,
                darkness: d.darkness,
            }),
        }
    }

    /// Rebuilds a state from a snapshot. Checks the sizes and the generator states, and
    /// fails rather than guess.
    pub fn restore(snapshot: &Snapshot) -> Result<Self, StateError> {
        if snapshot.floors_entered > MAX_RESTORED_FLOORS {
            return Err(StateError::TooManyFloors(snapshot.floors_entered));
        }
        // The preseed generator's state is not always a value `new` accepts (its step does not
        // keep the 24-bit, odd form), so it is rebuilt by replaying one step per floor.
        let mut preseed = DungeonPreseed::new(snapshot.run_seed);
        for _ in 0..snapshot.floors_entered {
            preseed.next_floor_seed();
        }
        let dungeon = match &snapshot.dungeon {
            None => None,
            Some(d) => {
                if d.team.len() > TEAM_SLOTS {
                    return Err(StateError::TeamTooLarge(d.team.len()));
                }
                if d.wild.len() > WILD_SLOTS {
                    return Err(StateError::WildTooLarge(d.wild.len()));
                }
                Some(Dungeon {
                    floor: d.floor.restore()?,
                    rng: DungeonRng::from_state(d.rng),
                    team: d.team.clone(),
                    wild: d.wild.clone(),
                    turn: d.turn,
                    // Clamped as the sim keeps them, so a crafted snapshot cannot index past
                    // the speed table or the 24 fractional turns.
                    fractional_turn: d.fractional_turn % FRACTIONAL_TURNS,
                    pending_end_of_turns: d.pending_end_of_turns,
                    leader_speed: d.leader_speed,
                    speed_stage: d.speed_stage.map(|s| s.min(MAX_SPEED_STAGE)),
                    shop_pass: d.shop_pass,
                    monster_house_room: d.monster_house_room,
                    player_spawn: d.player_spawn,
                    stairs_spawn: d.stairs_spawn,
                    stairs_pending: d.stairs_pending,
                    // Clamped to the two bits the setting uses.
                    darkness: d.darkness & 3,
                })
            }
        };
        Ok(Self {
            global: GlobalRng::from_state(snapshot.global),
            run_seed: snapshot.run_seed,
            floors_entered: snapshot.floors_entered,
            preseed,
            dungeon,
            floors: snapshot.floors.clone(),
        })
    }
}

/// One tile, as a snapshot: the fields of [`Tile`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileSnapshot {
    pub terrain: u16,
    pub spawn_or_visibility: u16,
    pub graphics_variant: u8,
    pub room: u8,
    pub walkable_neighbors: [u8; 4],
    pub unk_e: u16,
}

impl From<Tile> for TileSnapshot {
    fn from(t: Tile) -> Self {
        Self {
            terrain: t.terrain,
            spawn_or_visibility: t.spawn_or_visibility,
            graphics_variant: t.graphics_variant,
            room: t.room,
            walkable_neighbors: t.walkable_neighbors,
            unk_e: t.unk_e,
        }
    }
}

impl From<TileSnapshot> for Tile {
    fn from(t: TileSnapshot) -> Self {
        Self {
            terrain: t.terrain,
            spawn_or_visibility: t.spawn_or_visibility,
            graphics_variant: t.graphics_variant,
            room: t.room,
            walkable_neighbors: t.walkable_neighbors,
            unk_e: t.unk_e,
        }
    }
}

/// A floor as a snapshot: its tiles row by row, the stairs and the room table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorSnapshot {
    pub tiles: Vec<TileSnapshot>,
    pub stairs: Option<(i16, i16)>,
    /// The room table (see [`Room`]). Absent in older snapshots: none.
    #[serde(default)]
    pub rooms: Vec<Room>,
}

impl FloorSnapshot {
    fn capture(floor: &Floor) -> Self {
        let mut tiles = Vec::with_capacity(WIDTH * HEIGHT);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if let Some(t) = floor.tile(x as i32, y as i32) {
                    tiles.push(TileSnapshot::from(*t));
                }
            }
        }
        Self {
            tiles,
            stairs: floor.stairs,
            rooms: floor.rooms().to_vec(),
        }
    }

    fn restore(&self) -> Result<Floor, StateError> {
        if self.tiles.len() != WIDTH * HEIGHT {
            return Err(StateError::BadTileCount(self.tiles.len()));
        }
        let mut floor = Floor::new();
        let mut tiles = self.tiles.iter().copied();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if let (Some(slot), Some(t)) = (floor.tile_mut(x as i32, y as i32), tiles.next()) {
                    *slot = Tile::from(t);
                }
            }
        }
        floor.stairs = self.stairs;
        floor.rooms.clone_from(&self.rooms);
        Ok(floor)
    }
}

/// A dungeon as a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DungeonSnapshot {
    pub rng: u32,
    pub floor: FloorSnapshot,
    pub team: Vec<Monster>,
    pub wild: Vec<Monster>,
    pub turn: u32,
    /// `fractionalTurn`, 0 to 23. Restore takes it modulo 24.
    pub fractional_turn: u8,
    /// The leader's end-of-turns waiting for a weather step.
    pub pending_end_of_turns: u8,
    /// The leader's species movement speed, the base of its speed stage.
    pub leader_speed: i32,
    /// The stage the caller set, or `None` to compute it. Restore clamps it to 0 to 4.
    pub speed_stage: Option<u8>,
    pub shop_pass: bool,
    pub monster_house_room: u8,
    pub player_spawn: Option<(i16, i16)>,
    pub stairs_spawn: Option<(i16, i16)>,
    /// `dungeon+1`: the leader's next input takes the stairs. Absent in older snapshots: false.
    #[serde(default)]
    pub stairs_pending: bool,
    /// The darkness setting, 0 to 3 (see [`crate::visibility`]). Absent in older snapshots: 0.
    #[serde(default)]
    pub darkness: u8,
}

/// A whole run as a snapshot: the generator states and the dungeon, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub global: u32,
    pub run_seed: u32,
    pub floors_entered: u32,
    pub dungeon: Option<DungeonSnapshot>,
    /// The run's floor records (see [`GameState::with_floors`]). Absent in older snapshots: none.
    #[serde(default)]
    pub floors: Vec<FloorRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::MenuAction;
    use crate::floor::TERRAIN_FLOOR;
    use crate::visibility::{LIT_BIT, REVEALED_BIT};

    fn info(level: u8) -> EntityInfo {
        EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: false,
            level,
            iq: 0,
            hp: 30,
            max_hp: 30,
            attack: [10, 10],
            defence: [5, 5],
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
        }
    }

    fn monster(x: i16, y: i16, level: u8) -> Monster {
        Monster {
            pos: (x, y),
            visible: true,
            info: info(level),
        }
    }

    /// A floor of open ground.
    fn open_floor() -> Floor {
        let mut f = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                if let Some(t) = f.tile_mut(x, y) {
                    t.terrain = TERRAIN_FLOOR;
                }
            }
        }
        f
    }

    /// A run with the leader at (5, 5) on open ground. The leader is the fastest stage, so
    /// every command takes its turn in the fractional turn it is given.
    fn run_with_leader(seed: u32) -> GameState {
        let mut g = GameState::new(seed);
        let mut leader = monster(5, 5, 8);
        leader.info.is_team_leader = true;
        g.enter_dungeon(open_floor(), vec![leader], 0).unwrap();
        g.set_speed_stage(MAX_SPEED_STAGE);
        g
    }

    /// [`run_with_leader`] with a full belly: the info helper leaves it at 0, and a run needs a
    /// belly that is not 0 (`docs/re/turns.md`, "Run flag and belly test").
    fn fed_run(seed: u32) -> GameState {
        let mut g = run_with_leader(seed);
        g.dungeon.as_mut().unwrap().team[0].info.belly = 100_000;
        g
    }

    fn leader_pos(g: &GameState) -> (i16, i16) {
        g.dungeon.as_ref().unwrap().team[0].pos
    }

    fn set_room(g: &mut GameState, x: i32, y: i32, room: u8) {
        let d = g.dungeon.as_mut().unwrap();
        d.floor.tile_mut(x, y).unwrap().room = room;
    }

    #[test]
    fn no_dungeon_blocks_every_command() {
        let mut g = GameState::new(1);
        assert_eq!(g.step(Command::Wait), Outcome::Blocked(Blocked::NoDungeon));
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::NoDungeon)
        );
        assert!(g.dungeon.is_none());
    }

    #[test]
    fn move_steps_the_leader_and_takes_a_turn() {
        let mut g = run_with_leader(1);
        assert_eq!(g.step(Command::Move(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
        assert_eq!(g.dungeon.as_ref().unwrap().turn, 1);
        assert_eq!(g.step(Command::Move(Direction::DownLeft)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (5, 6));
    }

    #[test]
    fn a_wall_blocks_the_move_and_changes_nothing() {
        let mut g = run_with_leader(1);
        let before = g.clone();
        let d = g.dungeon.as_mut().unwrap();
        d.floor.tile_mut(6, 5).unwrap().terrain = floor::TERRAIN_IMPASSABLE_WALL;
        let before_wall = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::Wall)
        );
        assert_eq!(g, before_wall);
        assert_ne!(g, before);
    }

    #[test]
    fn the_map_edge_blocks_the_move() {
        let mut g = GameState::new(1);
        let mut leader = monster(0, 0, 8);
        leader.info.is_team_leader = true;
        g.enter_dungeon(open_floor(), vec![leader], 0).unwrap();
        g.set_speed_stage(MAX_SPEED_STAGE);
        let before = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::UpLeft)),
            Outcome::Blocked(Blocked::OutOfBounds)
        );
        assert_eq!(g, before);
    }

    #[test]
    fn a_monster_blocks_the_move() {
        let mut g = run_with_leader(1);
        g.dungeon.as_mut().unwrap().wild.push(monster(6, 5, 3));
        let before = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::Occupied)
        );
        assert_eq!(g, before);
    }

    #[test]
    fn a_run_goes_on_until_a_wall_is_ahead() {
        let mut g = fed_run(1);
        set_terrain(&mut g, 9, 5, floor::TERRAIN_IMPASSABLE_WALL);
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (8, 5));
        assert_eq!(g.dungeon().unwrap().turn(), 3);
    }

    #[test]
    fn a_run_stops_at_a_monster_ahead() {
        let mut g = fed_run(1);
        g.dungeon.as_mut().unwrap().wild.push(monster(8, 5, 3));
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (7, 5));
    }

    #[test]
    fn a_run_stops_for_a_monster_beside_the_next_tile() {
        // (7, 4) is ahead and to the side of the leader at (6, 5): check 6.
        let mut g = fed_run(1);
        g.dungeon.as_mut().unwrap().wild.push(monster(7, 4, 3));
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
        assert_eq!(g.dungeon().unwrap().turn(), 1);
    }

    #[test]
    fn a_run_stops_before_stairs_in_reach() {
        let mut g = fed_run(1);
        set_terrain(&mut g, 8, 5, floor::TERRAIN_FLOOR | floor::TERRAIN_STAIRS);
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (7, 5));
    }

    #[test]
    fn a_run_stops_at_the_edge_of_a_room_from_a_corridor() {
        let mut g = fed_run(1);
        g.dungeon
            .as_mut()
            .unwrap()
            .floor
            .tile_mut(8, 5)
            .unwrap()
            .room = 0;
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (7, 5));
    }

    #[test]
    fn a_run_stops_on_a_natural_junction_in_a_room() {
        // Check 5: the room tile at (6, 5) carries the natural-junction bit. The room block keeps
        // check 9 from firing there.
        let mut g = fed_run(1);
        for y in 4..=6 {
            for x in 4..=7 {
                set_room(&mut g, x, y, 0);
            }
        }
        set_terrain(
            &mut g,
            6,
            5,
            floor::TERRAIN_FLOOR | TERRAIN_NATURAL_JUNCTION,
        );
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
        assert_eq!(g.dungeon().unwrap().turn(), 1);
    }

    #[test]
    fn a_room_run_stops_beside_a_tile_of_another_room() {
        // Check 9: the leader in room 0 has an open cardinal neighbour below it in room 1, not
        // behind it, so the run stops. With that tile in room 0 the run goes on.
        let mut g = fed_run(1);
        for y in 4..=6 {
            for x in 4..=7 {
                set_room(&mut g, x, y, 0);
            }
        }
        set_room(&mut g, 5, 6, 1);
        assert_eq!(g.dungeon().unwrap().run_ahead(Direction::Right), None);
        set_room(&mut g, 5, 6, 0);
        assert_eq!(
            g.dungeon().unwrap().run_ahead(Direction::Right),
            Some((6, 5))
        );
    }

    #[test]
    fn a_corridor_run_stops_beside_a_side_opening() {
        // Check 10: the corridor on row 5 has a gap in its lower wall at (6, 6), with a wall at
        // the corner ahead, (7, 6). The first step lands on (6, 5), beside the gap, and the run
        // stops there. Checks 5 and 9 do not fire: the tiles are not in a room.
        let mut g = fed_run(1);
        for x in 5..=8 {
            set_terrain(&mut g, x, 4, floor::TERRAIN_IMPASSABLE_WALL);
        }
        for x in [5, 7, 8] {
            set_terrain(&mut g, x, 6, floor::TERRAIN_IMPASSABLE_WALL);
        }
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
        assert_eq!(g.dungeon().unwrap().turn(), 1);
    }

    #[test]
    fn a_run_does_not_continue_from_water_under_the_leader() {
        // Check 3: the leader's own tile must be plain ground. A step cannot reach water, so
        // `run_ahead` is called directly. With plain ground under the leader the run goes on.
        let mut g = fed_run(1);
        set_terrain(&mut g, 5, 5, 2);
        assert_eq!(g.dungeon().unwrap().run_ahead(Direction::Right), None);
        set_terrain(&mut g, 5, 5, TERRAIN_FLOOR);
        assert_eq!(
            g.dungeon().unwrap().run_ahead(Direction::Right),
            Some((6, 5))
        );
    }

    #[test]
    fn a_run_is_one_step_with_an_empty_belly() {
        let mut g = run_with_leader(1);
        g.dungeon.as_mut().unwrap().team[0].info.belly = 0;
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
        assert_eq!(g.dungeon().unwrap().turn(), 1);
    }

    #[test]
    fn a_run_stops_when_the_belly_runs_out() {
        // 0.2 of belly: the first step leaves 0.1, the second empties it and stops the run.
        let mut g = fed_run(1);
        g.set_speed_stage(1);
        g.dungeon.as_mut().unwrap().team[0].info.belly = 200;
        assert_eq!(g.step(Command::Run(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (7, 5));
        assert!(g.dungeon().unwrap().team()[0].info.belly_empty);
    }

    #[test]
    fn a_move_into_a_team_ally_swaps_places_and_the_ally_faces_back() {
        let mut g = run_with_leader(1);
        g.dungeon.as_mut().unwrap().team.push(monster(6, 5, 8));
        assert_eq!(g.step(Command::Move(Direction::Right)), Outcome::Applied);
        let team = g.dungeon().unwrap().team();
        assert_eq!((team[0].pos, team[1].pos), ((6, 5), (5, 5)));
        // Right is 2; (2 + 4) & 7 is 6, Left.
        assert_eq!(team[1].info.direction, 6);
        assert_eq!(g.dungeon().unwrap().turn(), 1);
    }

    #[test]
    fn a_team_ally_on_water_is_not_swapped_with() {
        let mut g = run_with_leader(1);
        g.dungeon.as_mut().unwrap().team.push(monster(6, 5, 8));
        set_terrain(&mut g, 6, 5, 2);
        let before = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::Terrain)
        );
        assert_eq!(g, before);
    }

    #[test]
    fn an_empty_team_has_no_leader_to_move() {
        let mut g = GameState::new(1);
        g.enter_dungeon(open_floor(), Vec::new(), 0).unwrap();
        g.set_speed_stage(MAX_SPEED_STAGE);
        assert_eq!(
            g.step(Command::Move(Direction::Up)),
            Outcome::Blocked(Blocked::NoLeader)
        );
        // Waiting does not need a leader.
        assert_eq!(g.step(Command::Wait), Outcome::Applied);
    }

    #[test]
    fn wait_only_advances_the_turn() {
        let mut g = run_with_leader(1);
        let before_pos = leader_pos(&g);
        assert_eq!(g.step(Command::Wait), Outcome::Applied);
        assert_eq!(leader_pos(&g), before_pos);
        assert_eq!(g.dungeon.as_ref().unwrap().turn, 1);
    }

    #[test]
    fn a_move_takes_the_leaders_turn_in_its_gated_fractional_turn() {
        // Stage 1 acts in fractional turns 3, 7, ...: a move from turn 0 runs turns 0 to 3,
        // steps the leader, runs its end-of-turn at the weather step of turn 6, and leaves the
        // counter on 7, the leader's next gate.
        let mut g = run_with_leader(1);
        g.set_speed_stage(1);
        assert_eq!(g.step(Command::Move(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
        let d = g.dungeon.as_ref().unwrap();
        assert_eq!((d.fractional_turn(), d.turn()), (7, 1));
    }

    #[test]
    fn a_refused_move_takes_no_time() {
        let mut g = run_with_leader(1);
        g.set_speed_stage(1);
        set_terrain(&mut g, 6, 5, floor::TERRAIN_IMPASSABLE_WALL);
        let before = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::Wall)
        );
        assert_eq!(g, before);
        assert_eq!(g.dungeon().unwrap().fractional_turn(), 0);
    }

    /// A run whose leader has no counters and no stage set, with the given species speed.
    fn fresh_leader(leader_speed: i32) -> GameState {
        let mut l = monster(5, 5, 8);
        l.info.is_team_leader = true;
        GameState::in_dungeon(1, open_floor(), vec![l], leader_speed).unwrap()
    }

    #[test]
    fn a_fresh_leaders_stage_is_its_species_speed_clamped() {
        for (speed, expected) in [(0, 0), (1, 1), (2, 2), (5, MAX_SPEED_STAGE), (-3, 0)] {
            let g = fresh_leader(speed);
            assert_eq!(
                g.dungeon().unwrap().leader_stage(),
                expected,
                "speed {speed}"
            );
        }
    }

    #[test]
    fn the_speed_counters_raise_and_lower_the_stage() {
        // `CalcSpeedStage`: species speed 2, plus one per nonzero speed-up counter, minus one
        // per nonzero speed-down counter, clamped to 0 to 4.
        let stage = |up: [u8; 5], down: [u8; 5]| {
            let mut l = monster(5, 5, 8);
            l.info.is_team_leader = true;
            l.info.speed_up = up;
            l.info.speed_down = down;
            GameState::in_dungeon(1, open_floor(), vec![l], 2)
                .unwrap()
                .dungeon()
                .unwrap()
                .leader_stage()
        };
        assert_eq!(stage([0; 5], [0; 5]), 2);
        assert_eq!(stage([7, 0, 0, 0, 0], [0; 5]), 3);
        assert_eq!(stage([1, 1, 0, 0, 0], [0; 5]), 4);
        assert_eq!(stage([1, 1, 1, 1, 1], [0; 5]), MAX_SPEED_STAGE);
        assert_eq!(stage([0; 5], [1, 0, 0, 0, 0]), 1);
        assert_eq!(stage([0; 5], [1; 5]), 0);
        assert_eq!(stage([1, 1, 0, 0, 0], [1, 0, 0, 0, 0]), 3);
    }

    #[test]
    fn a_fresh_leader_acts_without_a_caller_setting_its_stage() {
        let mut g = fresh_leader(1);
        // Stage 1 acts in fractional turn 3, so the first wait leaves the counter on 7, the next gate.
        assert_eq!(g.step(Command::Wait), Outcome::Applied);
        assert_eq!(g.dungeon().unwrap().fractional_turn(), 7);
        assert_eq!(g.step(Command::Move(Direction::Right)), Outcome::Applied);
        assert_eq!(leader_pos(&g), (6, 5));
    }

    #[test]
    fn a_stage_the_caller_sets_overrides_the_computed_one() {
        let mut g = fresh_leader(2);
        g.set_speed_stage(0);
        assert_eq!(g.dungeon().unwrap().speed_stage(), Some(0));
        assert_eq!(g.dungeon().unwrap().leader_stage(), 0);
    }

    #[test]
    fn the_leader_speed_and_counters_survive_a_snapshot() {
        let mut l = monster(5, 5, 8);
        l.info.is_team_leader = true;
        l.info.speed_up = [2, 0, 0, 0, 0];
        let g = GameState::in_dungeon(1, open_floor(), vec![l], 2).unwrap();
        let back = GameState::restore(&g.snapshot()).unwrap();
        assert_eq!(back, g);
        assert_eq!(back.dungeon().unwrap().leader_stage(), 3);
    }

    #[test]
    fn the_speed_stage_is_clamped_to_the_fastest() {
        let mut g = run_with_leader(1);
        g.set_speed_stage(9);
        assert_eq!(g.dungeon().unwrap().speed_stage(), Some(MAX_SPEED_STAGE));
    }

    #[test]
    fn unmodelled_commands_are_valid_and_change_nothing() {
        let mut g = run_with_leader(1);
        let before = g.clone();
        for c in [
            Command::Attack(Direction::Down),
            Command::UseMove {
                slot: 2,
                direction: Some(Direction::Up),
            },
            Command::UseItem { slot: 0 },
            Command::Menu(MenuAction::Confirm),
        ] {
            assert_eq!(g.step(c), Outcome::NotModelled, "{c:?}");
        }
        assert_eq!(g, before);
    }

    #[test]
    fn oversized_teams_are_refused() {
        let mut g = GameState::new(1);
        let team = (0..5).map(|i| monster(i, 5, 1)).collect();
        assert_eq!(
            g.enter_dungeon(open_floor(), team, 0),
            Err(StateError::TeamTooLarge(5))
        );
        assert!(g.dungeon.is_none());
    }

    #[test]
    fn the_floor_seed_comes_from_the_preseed() {
        let mut a = GameState::new(0x12_3457);
        let mut b = GameState::new(0x12_3457);
        a.enter_dungeon(open_floor(), Vec::new(), 0).unwrap();
        b.enter_dungeon(open_floor(), Vec::new(), 0).unwrap();
        assert_eq!(a, b);
        // A second floor takes the next seed, so its generator differs.
        let first = a.dungeon.as_ref().unwrap().rng.state();
        a.enter_dungeon(open_floor(), Vec::new(), 0).unwrap();
        assert_ne!(a.dungeon.as_ref().unwrap().rng.state(), first);
    }

    /// `n` floors of layout 1 with no items or monsters, after the placeholder at index 0: the
    /// floor count is `n + 1`. Floor `i` has room density `i`, so the floors differ.
    fn records(n: usize) -> Vec<FloorRecord> {
        let mut out = vec![[0u8; FLOOR_PROPERTIES]];
        for i in 1..=n {
            let mut floor = [0u8; FLOOR_PROPERTIES];
            floor[0] = 1;
            floor[1] = u8::try_from(i).expect("few floors");
            out.push(floor);
        }
        out
    }

    /// A run's floor for the layout comparisons: the arrival reveal is undone. Every tile must
    /// carry no marker outside the reveal bits (a stairs or trap marker left by generation would
    /// fail here); the reveal bits (0 and 1) are then cleared, and the rooms' revealed flags too.
    // The reveal bits also carry the item marker (bit 1); the item case is checked on its own in
    // `visibility::entry_clears_the_markers_and_builds_the_room_table_from_the_tiles`.
    fn layout_of(floor: &Floor) -> Floor {
        let mut out = floor.clone();
        for room in &mut out.rooms {
            room.revealed = false;
        }
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if let Some(t) = out.tile_mut(x as i32, y as i32) {
                    assert_eq!(
                        t.spawn_or_visibility & !(REVEALED_BIT | LIT_BIT),
                        0,
                        "a generation marker survived the entry at ({x}, {y})"
                    );
                    t.spawn_or_visibility = 0;
                }
            }
        }
        out
    }

    /// A generated floor after the entry step, in the form [`layout_of`] compares: no markers, and
    /// the room table built from its tiles.
    fn expected_layout(floor: &Floor) -> Floor {
        let mut out = floor.clone();
        crate::visibility::enter_floor(&mut out);
        layout_of(&out)
    }

    /// Floor `n` of the run `seed`, built the way the stairs build it: the chain's `n`th seed,
    /// the fixed draws, then `GenerateFloor`. Returns the floor, the generator after it, and the
    /// generator's stairs.
    fn chained_floor(seed: u32, recs: &[FloorRecord], n: usize) -> (Floor, DungeonRng, GenState) {
        chained_floor_with(seed, &recs[n], n)
    }

    /// [`chained_floor`] with the floor's record given directly, so a test can build floor `n`
    /// from another floor's record.
    fn chained_floor_with(
        seed: u32,
        record: &FloorRecord,
        n: usize,
    ) -> (Floor, DungeonRng, GenState) {
        let mut preseed = DungeonPreseed::new(seed);
        let mut floor_seed = 0;
        for _ in 0..n {
            floor_seed = preseed.next_floor_seed();
        }
        let mut rng = DungeonRng::new(floor_seed);
        for _ in 0..FLOOR_ENTRY_STEPS {
            rng.next_u16();
        }
        let mut floor = Floor::new();
        let mut g = GenState::default();
        generate_floor(
            &properties_from_record(record),
            false,
            &mut floor,
            &mut rng,
            &mut g,
            &mut PortedSteps::default(),
            false,
        )
        .unwrap();
        (floor, rng, g)
    }

    /// A run on floor 1 of `seed`, with the leader on the floor's start.
    fn first_floor_run(seed: u32, recs: Vec<FloorRecord>) -> GameState {
        let (floor, _, g) = chained_floor(seed, &recs, 1);
        let mut leader = monster(g.stairs_a.0, g.stairs_a.1, 8);
        leader.info.is_team_leader = true;
        let mut st = GameState::new(seed).with_floors(recs);
        st.enter_dungeon(floor, vec![leader], 0).unwrap();
        st.set_speed_stage(MAX_SPEED_STAGE);
        st
    }

    /// Makes the leader's next input take the stairs it is standing on. The walk test below
    /// does this by walking; the others skip the walk.
    fn stand_on_stairs(st: &mut GameState) {
        st.dungeon.as_mut().unwrap().stairs_pending = true;
    }

    /// The moves from `from` to `to` over ordinary ground, by breadth-first search with the
    /// corner rule. Test-only: the moves are then checked by `step`.
    fn walk_path(floor: &Floor, from: (i32, i32), to: (i32, i32)) -> Vec<Direction> {
        const DIRS: [Direction; 8] = [
            Direction::Down,
            Direction::DownRight,
            Direction::Right,
            Direction::UpRight,
            Direction::Up,
            Direction::UpLeft,
            Direction::Left,
            Direction::DownLeft,
        ];
        let open = |x: i32, y: i32| {
            floor.tile(x, y).is_some_and(|t| {
                t.terrain & floor::TERRAIN_IMPASSABLE_WALL == 0
                    && t.terrain & floor::TERRAIN_TYPE_MASK == TERRAIN_NORMAL
            })
        };
        let mut came_from: std::collections::BTreeMap<(i32, i32), ((i32, i32), Direction)> =
            std::collections::BTreeMap::new();
        let mut seen = std::collections::BTreeSet::from([from]);
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(p) = queue.pop_front() {
            for dir in DIRS {
                let (dx, dy) = dir.delta();
                let q = (p.0 + dx, p.1 + dy);
                let corner_ok = dx == 0 || dy == 0 || (open(p.0 + dx, p.1) && open(p.0, p.1 + dy));
                if open(q.0, q.1) && corner_ok && seen.insert(q) {
                    came_from.insert(q, (p, dir));
                    queue.push_back(q);
                }
            }
        }
        let mut path = Vec::new();
        let mut at = to;
        while at != from {
            let (prev, dir) = *came_from.get(&at).expect("the stairs are reachable");
            path.push(dir);
            at = prev;
        }
        path.reverse();
        path
    }

    #[test]
    fn a_scripted_walk_to_the_stairs_takes_them_to_floor_2() {
        let seed = 0x0055_5557;
        let recs = records(3);
        let (_, _, g1) = chained_floor(seed, &recs, 1);
        let mut st = first_floor_run(seed, recs.clone());
        let from = g1.stairs_a;
        let to = g1.stairs_b;
        let path = walk_path(
            st.dungeon().unwrap().floor(),
            (from.0.into(), from.1.into()),
            (to.0.into(), to.1.into()),
        );
        assert!(!path.is_empty(), "the stairs are not on the leader's start");
        for dir in path {
            assert_eq!(st.step(Command::Move(dir)), Outcome::Applied);
        }
        let d = st.dungeon().unwrap();
        assert_eq!(d.team()[0].pos, to, "the leader stands on the stairs");
        assert!(d.stairs_pending(), "the step set the stairs flag");
        assert_eq!(
            d.floor()
                .tile(i32::from(to.0), i32::from(to.1))
                .map(|t| t.terrain & floor::TERRAIN_STAIRS),
            Some(floor::TERRAIN_STAIRS)
        );

        // The leader's next input is the stairs action.
        assert_eq!(st.step(Command::Wait), Outcome::NextFloor);
        assert_eq!(st.floors_entered(), 2);
        let (floor2, rng2, g2) = chained_floor(seed, &recs, 2);
        let d = st.dungeon().unwrap();
        assert_eq!(
            layout_of(d.floor()),
            expected_layout(&floor2),
            "floor 2 is generated from the chain's second seed"
        );
        // Floor 2 takes record 2, not record 1 (an off-by-one in `take_stairs` would pass the
        // check above only if the two records made the same floor).
        let (floor2_from_record_1, _, _) = chained_floor_with(seed, &recs[1], 2);
        assert_ne!(
            layout_of(d.floor()),
            expected_layout(&floor2_from_record_1),
            "floor 2 is built from record 2, not record 1"
        );
        assert_eq!(
            d.rng(),
            &rng2,
            "the floor generator is left where generation left it"
        );
        assert_eq!(
            d.team()[0].pos,
            g2.stairs_a,
            "the leader starts on floor 2's start"
        );
        assert_eq!(
            (d.turn(), d.fractional_turn(), d.stairs_pending()),
            (0, 0, false)
        );
    }

    #[test]
    fn two_floors_in_a_row_match_separate_generation_and_the_last_floor_clears() {
        let seed = 0x0031_4159;
        let recs = records(3);
        let mut st = first_floor_run(seed, recs.clone());
        for n in [2, 3] {
            stand_on_stairs(&mut st);
            assert_eq!(st.step(Command::Wait), Outcome::NextFloor, "to floor {n}");
            let (floor, rng, _) = chained_floor(seed, &recs, n);
            let d = st.dungeon().unwrap();
            assert_eq!(layout_of(d.floor()), expected_layout(&floor), "floor {n}");
            assert_eq!(d.rng(), &rng, "floor {n} generator");
        }
        // Floor 3 is the last: its stairs clear the dungeon.
        stand_on_stairs(&mut st);
        assert_eq!(st.step(Command::Wait), Outcome::DungeonCleared);
        assert!(st.dungeon().is_none());
        assert_eq!(st.floors_entered(), 3);
        assert_eq!(st.step(Command::Wait), Outcome::Blocked(Blocked::NoDungeon));
    }

    #[test]
    fn the_leader_and_the_team_carry_their_stats_to_the_next_floor() {
        let seed = 0x0042_4243;
        let recs = records(2);
        let mut st = first_floor_run(seed, recs);
        let mut ally = monster(1, 1, 6);
        ally.info.hp = 9;
        st.dungeon.as_mut().unwrap().team.push(ally);
        {
            let leader = &mut st.dungeon.as_mut().unwrap().team[0].info;
            leader.hp = 17;
            leader.belly = 42_000;
            leader.level = 9;
            leader.regen_speed = 200;
        }
        stand_on_stairs(&mut st);
        assert_eq!(st.step(Command::Wait), Outcome::NextFloor);
        let team = st.dungeon().unwrap().team();
        assert_eq!(team.len(), 2);
        // No step is pending (the flag was set by hand), so the stairs turn ticks no end-of-turn
        // and the regeneration accumulator stays under its speed: HP and belly carry unchanged.
        assert_eq!((team[0].info.hp, team[0].info.level), (17, 9));
        assert_eq!(team[0].info.belly, 42_000);
        assert_eq!((team[1].info.hp, team[1].info.level), (9, 6));
        assert_ne!(team[1].pos, team[0].pos, "the members do not share a tile");
    }

    #[test]
    fn a_run_with_no_floor_data_clears_at_its_first_stairs() {
        let mut leader = monster(5, 5, 8);
        leader.info.is_team_leader = true;
        let mut st = GameState::new(0x0007_0007);
        st.enter_dungeon(open_floor(), vec![leader], 0).unwrap();
        stand_on_stairs(&mut st);
        assert_eq!(st.step(Command::Wait), Outcome::DungeonCleared);
    }

    #[test]
    fn the_floors_and_the_stairs_flag_survive_a_snapshot() {
        let seed = 0x0008_1111;
        let mut st = first_floor_run(seed, records(2));
        stand_on_stairs(&mut st);
        let text = serde_json::to_string(&st.snapshot()).unwrap();
        let back: Snapshot = serde_json::from_str(&text).unwrap();
        let mut restored = GameState::restore(&back).unwrap();
        assert_eq!(restored, st);
        assert_eq!(restored.step(Command::Wait), st.step(Command::Wait));
        assert_eq!(restored, st);
    }

    fn set_terrain(g: &mut GameState, x: i32, y: i32, terrain: u16) {
        let d = g.dungeon.as_mut().unwrap();
        d.floor.tile_mut(x, y).unwrap().terrain = terrain;
    }

    #[test]
    fn a_plain_wall_blocks_ordinary_ground_rules() {
        // Terrain 0 is an ordinary wall without the 0x10 flag: regular monsters may not enter.
        let mut g = run_with_leader(1);
        set_terrain(&mut g, 6, 5, 0);
        let before = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::Terrain)
        );
        assert_eq!(g, before);
    }

    #[test]
    fn water_and_lava_block_a_regular_monster() {
        // Terrain type 2 (secondary, water or lava) is not NORMAL.
        let mut g = run_with_leader(1);
        set_terrain(&mut g, 6, 5, 2);
        assert_eq!(
            g.step(Command::Move(Direction::Right)),
            Outcome::Blocked(Blocked::Terrain)
        );
    }

    #[test]
    fn a_diagonal_cannot_cut_a_wall_corner() {
        // Leader at (5, 5); the diagonal target (6, 6) is open, but (6, 5) is a wall.
        let mut g = run_with_leader(1);
        set_terrain(&mut g, 6, 5, 0);
        let before = g.clone();
        assert_eq!(
            g.step(Command::Move(Direction::DownRight)),
            Outcome::Blocked(Blocked::CornerCut)
        );
        assert_eq!(g, before);
        // The other orthogonal neighbour blocks too.
        let mut g = run_with_leader(1);
        set_terrain(&mut g, 5, 6, 0);
        assert_eq!(
            g.step(Command::Move(Direction::DownRight)),
            Outcome::Blocked(Blocked::CornerCut)
        );
    }

    #[test]
    fn a_diagonal_is_allowed_when_both_sides_are_open() {
        let mut g = run_with_leader(1);
        assert_eq!(
            g.step(Command::Move(Direction::DownRight)),
            Outcome::Applied
        );
        assert_eq!(leader_pos(&g), (6, 6));
    }

    #[test]
    fn a_wall_on_the_far_side_blocks_a_diagonal_too() {
        // Leader at (0, 1) moving up-right to (1, 0): the orthogonal (1, 1) is a wall.
        let mut g = GameState::in_dungeon(
            1,
            open_floor(),
            {
                let mut l = monster(0, 1, 8);
                l.info.is_team_leader = true;
                vec![l]
            },
            0,
        )
        .unwrap();
        g.set_speed_stage(MAX_SPEED_STAGE);
        set_terrain(&mut g, 1, 1, 0);
        assert_eq!(
            g.step(Command::Move(Direction::UpRight)),
            Outcome::Blocked(Blocked::CornerCut)
        );
    }

    #[test]
    fn directions_follow_the_game_table() {
        let order = [
            Direction::Down,
            Direction::DownRight,
            Direction::Right,
            Direction::UpRight,
            Direction::Up,
            Direction::UpLeft,
            Direction::Left,
            Direction::DownLeft,
        ];
        let deltas: Vec<(i32, i32)> = order.iter().map(|d| d.delta()).collect();
        assert_eq!(
            deltas,
            vec![
                (0, 1),
                (1, 1),
                (1, 0),
                (1, -1),
                (0, -1),
                (-1, -1),
                (-1, 0),
                (-1, 1)
            ]
        );
    }

    /// Plays `count` commands drawn from a generator seeded with `seed`, on a fixed setup.
    fn play(seed: u32, count: usize) -> (Snapshot, Vec<Outcome>) {
        let mut g = GameState::new(seed);
        let mut leader = monster(10, 10, 8);
        leader.info.is_team_leader = true;
        let mut team = vec![leader];
        team.push(monster(4, 4, 6));
        g.enter_dungeon(open_floor(), team, 0).unwrap();
        g.set_speed_stage(MAX_SPEED_STAGE);
        if let Some(d) = g.dungeon.as_mut() {
            d.wild.push(monster(20, 12, 3));
            d.wild.push(monster(30, 20, 4));
        }
        let mut pick = GlobalRng::from_state(seed ^ 0x9E37_79B9);
        let mut outcomes = Vec::with_capacity(count);
        for _ in 0..count {
            let dir = [
                Direction::Down,
                Direction::DownRight,
                Direction::Right,
                Direction::UpRight,
                Direction::Up,
                Direction::UpLeft,
                Direction::Left,
                Direction::DownLeft,
            ][pick.int(8) as usize];
            let command = match pick.int(10) {
                0..=5 => Command::Move(dir),
                6 => Command::Wait,
                7 => Command::Attack(dir),
                8 => Command::UseItem { slot: 1 },
                _ => Command::Menu(MenuAction::Down),
            };
            outcomes.push(g.step(command));
        }
        (g.snapshot(), outcomes)
    }

    #[test]
    fn same_seed_and_commands_give_the_same_state() {
        let (a, out_a) = play(0xC0FF_EE01, 400);
        let (b, out_b) = play(0xC0FF_EE01, 400);
        assert_eq!(a, b);
        assert_eq!(out_a, out_b);
        // Not vacuous: the moves did something, and another seed plays differently.
        assert!(out_a.contains(&Outcome::Applied));
        let (c, _) = play(0xC0FF_EE02, 400);
        assert_ne!(a, c);
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let mut g = run_with_leader(0x1234_5679);
        for _ in 0..25 {
            g.step(Command::Move(Direction::Right));
            g.step(Command::Move(Direction::Down));
        }
        let snap = g.snapshot();
        let text = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&text).unwrap();
        assert_eq!(back, snap);
        let restored = GameState::restore(&back).unwrap();
        assert_eq!(restored, g);
        assert_eq!(restored.snapshot(), snap);
    }

    #[test]
    fn the_counter_and_the_stage_survive_a_snapshot() {
        let mut g = run_with_leader(0x1234_5679);
        g.set_speed_stage(2);
        // Stage 2 acts in fractional turns 1 and 3: two waits leave the counter on 5, the next gate.
        g.step(Command::Wait);
        g.step(Command::Wait);
        assert_eq!(g.dungeon().unwrap().fractional_turn(), 5);
        let snap = g.snapshot();
        let text = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&text).unwrap();
        let mut restored = GameState::restore(&back).unwrap();
        assert_eq!(restored.dungeon().unwrap().fractional_turn(), 5);
        assert_eq!(restored.dungeon().unwrap().speed_stage(), Some(2));
        // The restored run keeps the same schedule.
        for _ in 0..6 {
            assert_eq!(
                restored.step(Command::Wait),
                g.step(Command::Wait),
                "turn outcome"
            );
        }
        assert_eq!(restored, g);

        // A crafted snapshot cannot put the counter or the stage out of range.
        let mut crafted = snap.clone();
        let d = crafted.dungeon.as_mut().unwrap();
        d.fractional_turn = 30;
        d.speed_stage = Some(9);
        let r = GameState::restore(&crafted).unwrap();
        assert_eq!(r.dungeon().unwrap().fractional_turn(), 6);
        assert_eq!(r.dungeon().unwrap().speed_stage(), Some(MAX_SPEED_STAGE));
    }

    #[test]
    fn restore_refuses_bad_snapshots() {
        let g = run_with_leader(7);
        let good = g.snapshot();

        let mut short = good.clone();
        short.dungeon.as_mut().unwrap().floor.tiles.pop();
        assert_eq!(
            GameState::restore(&short),
            Err(StateError::BadTileCount(WIDTH * HEIGHT - 1))
        );

        let mut big = good.clone();
        let extra = monster(1, 1, 1);
        big.dungeon.as_mut().unwrap().team = vec![extra.clone(); 5];
        assert_eq!(GameState::restore(&big), Err(StateError::TeamTooLarge(5)));

        let mut many = good.clone();
        many.floors_entered = MAX_RESTORED_FLOORS + 1;
        assert_eq!(
            GameState::restore(&many),
            Err(StateError::TooManyFloors(MAX_RESTORED_FLOORS + 1))
        );
    }
}
