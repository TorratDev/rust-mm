//! Simulation state of one run, and the one entry point that changes it.
//!
//! The state holds every random generator the sim owns (the global one, the floor-seed
//! preseed generator and the current floor's generator), the floor map, and the monsters.
//! Monsters live in two `Vec`s, the team and the wild monsters, in game slot order, so
//! nothing depends on hash order. Field names and offsets follow `docs/re/structs.md`
//! (the entity and entity-info tables, and the dungeon struct). Fields the notes do not
//! record are left out, not guessed: the item and trap entities, the moves and statuses of
//! a monster, the weather, the floor properties, and the turn-loop state. The `turn` counter
//! is the sim's own, not a game field.
//!
//! [`GameState::step`] is the only way a command changes the state. Integer math only.

use serde::{Deserialize, Serialize};

use crate::command::{Blocked, Command, Direction, Outcome};
use crate::floor::{self, Floor, HEIGHT, Tile, WIDTH};
use crate::rng::{DungeonPreseed, DungeonRng, GlobalRng};

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
/// item, the moves, the status records, the speed stages and counters, the skill bits, the
/// AI target and the decoy and curse state.
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
    /// `+0x44`: facing direction (the game's direction index).
    pub direction: u8,
    /// `+0x10E`: belly, a 4-byte fixed-point value (`probable` in the notes; its scale is
    /// not named).
    pub belly: i32,
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
    rng: DungeonRng,
    /// The team, in slot order; slot 0 is the leader.
    team: Vec<Monster>,
    /// The wild monsters, in slot order.
    wild: Vec<Monster>,
    /// The sim's turn counter: one per command that takes a turn.
    turn: u32,
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

    /// Moves the leader one tile in `dir`, or refuses with the reason. This is the regular
    /// mobility rule (`docs/re/turns.md`, `CanMoveInDirection` and the walkable-neighbour
    /// masks): the target must be NORMAL terrain, and a diagonal step needs both orthogonal
    /// neighbours to be open, so it never cuts a wall corner. Other mobility classes (water,
    /// lava, crevice) and the status and held-item overrides are not modelled, so the leader
    /// moves as a regular monster.
    fn move_leader(&mut self, dir: Direction) -> Outcome {
        let Some(leader) = self.team.first() else {
            return Outcome::Blocked(Blocked::NoLeader);
        };
        let (lx, ly) = (i32::from(leader.pos.0), i32::from(leader.pos.1));
        let (dx, dy) = dir.delta();
        let (x, y) = (lx + dx, ly + dy);
        let Some(tile) = self.floor.tile(x, y) else {
            return Outcome::Blocked(Blocked::OutOfBounds);
        };
        if tile.terrain & floor::TERRAIN_IMPASSABLE_WALL != 0 {
            return Outcome::Blocked(Blocked::Wall);
        }
        // The map is at most 56 by 32, so the tile test above keeps both coordinates small.
        let target = (x as i16, y as i16);
        if self.occupied(target.0, target.1) {
            return Outcome::Blocked(Blocked::Occupied);
        }
        if tile.terrain & floor::TERRAIN_TYPE_MASK != TERRAIN_NORMAL {
            return Outcome::Blocked(Blocked::Terrain);
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
                return Outcome::Blocked(Blocked::CornerCut);
            }
        }
        if let Some(leader) = self.team.first_mut() {
            leader.pos = target;
        }
        self.turn = self.turn.saturating_add(1);
        Outcome::Applied
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
    /// the team is too large.
    pub fn in_dungeon(seed: u32, floor: Floor, team: Vec<Monster>) -> Result<Self, StateError> {
        let mut state = Self::new(seed);
        state.enter_dungeon(floor, team)?;
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
    fn enter_dungeon(&mut self, floor: Floor, team: Vec<Monster>) -> Result<(), StateError> {
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
            shop_pass: false,
            monster_house_room: NO_MONSTER_HOUSE,
            player_spawn: None,
            stairs_spawn: None,
        });
        Ok(())
    }

    /// Applies one input command. The only entry point that changes the state.
    ///
    /// Only movement and waiting have their rules here. Attack, moves, items and menus are
    /// valid input and return [`Outcome::NotModelled`] without a change; a refused command
    /// returns [`Outcome::Blocked`] without a change.
    pub fn step(&mut self, command: Command) -> Outcome {
        let Some(dungeon) = self.dungeon.as_mut() else {
            return Outcome::Blocked(Blocked::NoDungeon);
        };
        match command {
            Command::Move(dir) => dungeon.move_leader(dir),
            Command::Wait => {
                dungeon.turn = dungeon.turn.saturating_add(1);
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
            direction: 0,
            belly: 0,
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

    /// A run with the leader at (5, 5) on open ground.
    fn run_with_leader(seed: u32) -> GameState {
        let mut g = GameState::new(seed);
        let mut leader = monster(5, 5, 8);
        leader.info.is_team_leader = true;
        g.enter_dungeon(open_floor(), vec![leader]).unwrap();
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
        g.enter_dungeon(open_floor(), vec![leader]).unwrap();
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
        g.enter_dungeon(open_floor(), Vec::new()).unwrap();
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
            g.enter_dungeon(open_floor(), team),
            Err(StateError::TeamTooLarge(5))
        );
        assert!(g.dungeon.is_none());
    }

    #[test]
    fn the_floor_seed_comes_from_the_preseed() {
        let mut a = GameState::new(0x12_3457);
        let mut b = GameState::new(0x12_3457);
        a.enter_dungeon(open_floor(), Vec::new()).unwrap();
        b.enter_dungeon(open_floor(), Vec::new()).unwrap();
        assert_eq!(a, b);
        // A second floor takes the next seed, so its generator differs.
        let first = a.dungeon.as_ref().unwrap().rng.state();
        a.enter_dungeon(open_floor(), Vec::new()).unwrap();
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
        let mut g = GameState::in_dungeon(1, open_floor(), {
            let mut l = monster(0, 1, 8);
            l.info.is_team_leader = true;
            vec![l]
        })
        .unwrap();
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
        g.enter_dungeon(open_floor(), team).unwrap();
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
