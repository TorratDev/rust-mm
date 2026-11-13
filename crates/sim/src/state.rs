//! Simulation state of one run, and the one entry point that changes it.
//!
//! The state holds every random generator the sim owns (the global one, the floor-seed
//! preseed generator and the current floor's generator), the floor map, and the monsters.
//! Monsters live in two `Vec`s, the team and the wild monsters, in game slot order, so
//! nothing depends on hash order. Field names and offsets follow `docs/re/structs.md`
//! (the entity and entity-info tables, and the dungeon struct). Fields the notes do not
//! record are left out, not guessed: the item and trap entities, the moves and statuses of
//! a monster, the weather and the floor properties. The turn-loop state is the fractional-turn
//! counter, the leader's speed stage and its pending end-of-turns ([`crate::turn`]). The `turn` counter is the sim's
//! own, not a game field.
//!
//! [`GameState::step`] is the only way a command changes the state. Integer math only.

use serde::{Deserialize, Serialize};

use crate::command::{Blocked, Command, Direction, Outcome};
use crate::floor::{self, Floor, HEIGHT, Tile, WIDTH};
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

/// The most floors a snapshot may record. Restoring the preseed generator replays one step
/// per floor, so the bound keeps a crafted snapshot from running long.
pub const MAX_RESTORED_FLOORS: u32 = 1 << 16;

/// The entity-info fields the notes record (`docs/re/structs.md`, "Entity info"). The
/// offset is the Blue offset from `entity + 0x70`. Left out: `types`, `abilities`, the held
/// item, the moves, the status records, the skill bits, the AI target and the decoy and
/// curse state.
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
    /// `+0x1C`: offensive stages for each split (s16 pair, clamped to 0 to 20 by the game).
    pub offensive_stages: [i16; 2],
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
    floor: Floor,
    /// The floor's generator (`gDungeonRngState`).
    pub(crate) rng: DungeonRng,
    /// The team, in slot order; slot 0 is the leader.
    pub(crate) team: Vec<Monster>,
    /// The wild monsters, in slot order.
    wild: Vec<Monster>,
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
        if self.occupied(target.0, target.1) {
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
        }
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

    /// Enters a floor (see [`GameState::in_dungeon`]). Private: a floor is entered only by
    /// the constructor or through the state's own rules.
    fn enter_dungeon(
        &mut self,
        floor: Floor,
        team: Vec<Monster>,
        leader_speed: i32,
    ) -> Result<(), StateError> {
        if team.len() > TEAM_SLOTS {
            return Err(StateError::TeamTooLarge(team.len()));
        }
        let floor_seed = self.preseed.next_floor_seed();
        self.floors_entered = self.floors_entered.saturating_add(1);
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
        });
        Ok(())
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
    /// A refused move changes nothing and takes no time. Attack, moves, items and menus are
    /// valid input and return [`Outcome::NotModelled`].
    pub fn step(&mut self, command: Command) -> Outcome {
        let Some(dungeon) = self.dungeon.as_mut() else {
            return Outcome::Blocked(Blocked::NoDungeon);
        };
        match command {
            Command::Move(dir) => match dungeon.leader_target(dir) {
                Ok(target) => {
                    dungeon.take_leader_turn(|d| {
                        if let Some(leader) = d.team.first_mut() {
                            leader.pos = target;
                        }
                    });
                    Outcome::Applied
                }
                Err(blocked) => Outcome::Blocked(blocked),
            },
            Command::Wait => {
                dungeon.take_leader_turn(|_| {});
                Outcome::Applied
            }
            Command::Attack(_)
            | Command::UseMove { .. }
            | Command::UseItem { .. }
            | Command::Menu(_) => Outcome::NotModelled,
        }
    }

    /// A serde snapshot of the whole state.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            global: self.global.state(),
            run_seed: self.run_seed,
            floors_entered: self.floors_entered,
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
                })
            }
        };
        Ok(Self {
            global: GlobalRng::from_state(snapshot.global),
            run_seed: snapshot.run_seed,
            floors_entered: snapshot.floors_entered,
            preseed,
            dungeon,
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

/// A floor as a snapshot: its tiles row by row, and the stairs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorSnapshot {
    pub tiles: Vec<TileSnapshot>,
    pub stairs: Option<(i16, i16)>,
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
}

/// A whole run as a snapshot: the generator states and the dungeon, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub global: u32,
    pub run_seed: u32,
    pub floors_entered: u32,
    pub dungeon: Option<DungeonSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::MenuAction;
    use crate::floor::TERRAIN_FLOOR;

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
            offensive_stages: [0, 0],
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

    fn leader_pos(g: &GameState) -> (i16, i16) {
        g.dungeon.as_ref().unwrap().team[0].pos
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
