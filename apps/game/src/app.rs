//! The game state the platform loop drives. No SDL here, so the boot flow runs headless.
//!
//! In the dungeon the sim owns the run (`GameState`). This owns what the sim does not: the
//! floor view, and the leader's walk on screen. A walk in progress is advanced each tick; when
//! none is, the buttons held are read as a command (`input`) and the sim is stepped. What the
//! step did starts a walk (a move), moves on to the next floor (the stairs) or goes back to the
//! menu (the dungeon is cleared).

use std::collections::BTreeMap;

use rescue_platform::{ButtonState, Flow};
use rescue_render::anim::AnimPlayer;
use rescue_sim::command::{Command, Direction, Outcome};
use rescue_sim::state::{EntityInfo, FloorRecord, GameState, Monster, MoveSlot};

use crate::boot::RomCheck;
use crate::combat::{self, LOG_LINES, Tables};
use crate::dungeon::{self, Dungeon, Figure, FloorData, IDLE_GROUP, MonsterArt, WALK_GROUP};
use crate::input::{self, MenuInput};
use crate::scene::Scene;
use crate::walk::Walk;

/// The leader's attack pose: group 2, the close-range strike (docs/formats/sprites.md).
// shortcut: Regular Attack's own group is `GetMoveAnimId`'s answer for move `0x163`, which is not
// read from the move table; the close strike stands in.
const ATTACK_GROUP: usize = 2;
/// A struck monster's pose: group 6, the hit reaction (docs/formats/sprites.md).
const HIT_GROUP: usize = 6;
/// The move the leader starts with in slot 0.
// shortcut: the leader's moves are not read from its learnset; Tackle (`0x09A`) stands in.
const START_MOVE: usize = 0x09A;

/// A hit reaction playing on the monster at `pos`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HitAnim {
    pos: (i16, i16),
    anim: AnimPlayer,
}

/// Where a run starts: its seed, the dungeon and floor of the options, and the leader's species.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunConfig {
    pub seed: u32,
    pub dungeon: u8,
    pub floor: u8,
    pub species: u16,
}

/// The running game: the current scene, last tick's held buttons, and while the dungeon scene
/// is up the run (`game`), its floor view (`dungeon`), the floor's ROM data (`data`, `None` for
/// the placeholder) and the leader's walk and animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub scene: Scene,
    held: ButtonState,
    config: RunConfig,
    /// The dump's bytes, for the floors' data. `None` without a ROM.
    rom: Option<Vec<u8>>,
    /// The ROM's number of the floor on screen.
    floor: u8,
    data: Option<FloorData>,
    game: Option<GameState>,
    dungeon: Option<Dungeon>,
    /// The walk on screen, while the leader is walking; inputs wait for it to end.
    walk: Option<Walk>,
    anim: Option<AnimPlayer>,
    /// The way the leader last moved, which it stands facing.
    facing: Direction,
    /// The sim's combat tables, once read from the dump.
    tables: Option<Tables>,
    /// The leader's attack pose, while it plays.
    strike: Option<AnimPlayer>,
    /// Hit reactions of struck monsters, while they play.
    hits: Vec<HitAnim>,
    /// Sprites of the wild species met on this floor.
    art: BTreeMap<u16, MonsterArt>,
    /// Logic ticks in the dungeon, which time the monsters' idle poses.
    ticks: u32,
    /// The move menu's cursor while it is open.
    menu: Option<usize>,
    /// The last lines of the message log.
    log: Vec<String>,
}

impl App {
    /// `open_dungeon` starts on the dungeon scene instead of the splash (with a ROM). `rom` is
    /// the dump's bytes, read when the ROM check passed.
    pub fn new(
        rom_check: &RomCheck,
        rom: Option<Vec<u8>>,
        config: RunConfig,
        open_dungeon: bool,
    ) -> App {
        let scene = match rom_check {
            RomCheck::Ready(_) if open_dungeon => Scene::Dungeon,
            RomCheck::Ready(_) => Scene::Splash { ticks: 0 },
            RomCheck::NotConfigured | RomCheck::Refused(_) => Scene::Refused,
        };
        let mut app = App {
            scene,
            held: ButtonState::default(),
            config,
            rom,
            floor: config.floor,
            data: None,
            game: None,
            dungeon: None,
            walk: None,
            anim: None,
            facing: Direction::Down,
            tables: None,
            strike: None,
            hits: Vec::new(),
            art: BTreeMap::new(),
            ticks: 0,
            menu: None,
            log: Vec::new(),
        };
        if app.scene == Scene::Dungeon {
            app.enter_dungeon();
        }
        app
    }

    /// The floor view while the dungeon scene is up.
    pub fn dungeon(&self) -> Option<&Dungeon> {
        self.dungeon.as_ref()
    }

    /// The message log's last lines, oldest first.
    // shortcut: no text is drawn yet (`draw`), so new lines are printed, not shown on screen.
    #[cfg(test)]
    fn log(&self) -> &[String] {
        &self.log
    }

    /// The move menu's cursor, while it is open.
    pub fn menu(&self) -> Option<usize> {
        self.menu
    }

    /// One logic tick with the buttons held now.
    pub fn tick(&mut self, buttons: ButtonState) -> Flow {
        let mut pressed = ButtonState::default();
        for b in buttons.held().filter(|b| !self.held.is_held(*b)) {
            pressed.set(b, true);
        }
        self.held = buttons;
        if self.scene == Scene::Refused {
            return if pressed.held().next().is_some() {
                Flow::Quit
            } else {
                Flow::Continue
            };
        }
        self.scene = self.scene.step(pressed);
        if self.scene == Scene::Dungeon {
            if self.game.is_none() {
                self.enter_dungeon();
            }
            if self.scene == Scene::Dungeon {
                self.step_dungeon(buttons, pressed);
            }
        } else {
            self.leave_dungeon();
        }
        Flow::Continue
    }

    /// The ROM's data for `floor` of the run's dungeon. `Err` without a ROM or if it does not
    /// load.
    fn load_floor(&self, floor: u8) -> Result<FloorData, String> {
        let rom = self.rom.as_deref().ok_or("no ROM")?;
        FloorData::from_rom(rom, self.config.dungeon, floor, self.config.species)
    }

    /// Starts the run on its first floor. Without the ROM's floor the placeholder is drawn (the
    /// error is printed). If the sim or the view cannot start, the menu stays up.
    fn enter_dungeon(&mut self) {
        let floor = self.config.floor;
        self.floor = floor;
        self.data = match self.load_floor(floor) {
            Ok(data) => Some(data),
            Err(e) => {
                if self.rom.is_some() {
                    eprintln!("rescue: dungeon placeholder, the floor did not load: {e}");
                }
                None
            }
        };
        let Some((map, leader)) = dungeon::first_floor(self.config.seed, self.data.as_ref()) else {
            return self.back_to_menu();
        };
        let (species, speed, records, wild) = match self.data.as_ref() {
            Some(d) => (
                self.config.species,
                d.leader_speed(),
                d.records(),
                d.wild_spawns(),
            ),
            None => (0, 0, &[][..], &[][..]),
        };
        let floors = sim_floors(records, floor);
        if self.tables.is_none() {
            self.tables = self.rom.as_deref().and_then(|rom| {
                Tables::from_rom(rom, species)
                    .map_err(|e| eprintln!("rescue: attacks are not modelled, no tables: {e}"))
                    .ok()
            });
        }
        let mut leader = leader_monster(leader, species);
        if let Some(mv) = self.tables.as_ref().and_then(|t| t.moves.get(START_MOVE)) {
            leader.info.moves[0] = MoveSlot::new(mv);
        }
        let team = vec![leader];
        let Ok(game) = GameState::in_dungeon_with_spawns(self.config.seed, map, team, speed, wild)
        else {
            return self.back_to_menu();
        };
        self.game = Some(game.with_floors(floors));
        if !self.show_floor() {
            self.back_to_menu();
        }
    }

    /// Builds the view of the sim's floor, with the leader standing. `false` if that fails.
    fn show_floor(&mut self) -> bool {
        let Some(map) = self
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .map(|d| d.floor())
        else {
            return false;
        };
        self.dungeon = Dungeon::new(map, self.data.as_ref());
        self.walk = None;
        self.strike = None;
        self.hits.clear();
        self.art.clear();
        self.anim = self
            .data
            .as_ref()
            .and_then(|d| d.leader_anim(IDLE_GROUP, self.facing));
        self.redraw();
        self.dungeon.is_some()
    }

    /// One logic tick of the dungeon: the animations run, and a walk in progress is advanced.
    /// While the move menu is open it reads the presses. When no walk or attack is playing
    /// (or it ends on this tick), the buttons held are read.
    fn step_dungeon(&mut self, buttons: ButtonState, pressed: ButtonState) {
        self.ticks = self.ticks.wrapping_add(1);
        if let Some(anim) = self.anim.as_mut() {
            anim.advance(1);
        }
        if let Some(strike) = self.strike.as_mut() {
            strike.advance(1);
            if strike.finished() {
                self.strike = None;
            }
        }
        for hit in &mut self.hits {
            hit.anim.advance(1);
        }
        self.hits.retain(|h| !h.anim.finished());
        if self.walk.as_mut().is_some_and(Walk::tick) {
            self.walk = None;
            self.anim = self
                .data
                .as_ref()
                .and_then(|d| d.leader_anim(IDLE_GROUP, self.facing));
        }
        let idle = self.walk.is_none() && self.strike.is_none() && self.hits.is_empty();
        if let Some(cursor) = self.menu {
            self.menu_tick(cursor, pressed);
        } else if idle && input::opens_menu(pressed) {
            self.menu = Some(0);
        } else if idle && let Some(command) = input::command_for(buttons, self.facing) {
            self.take_turn(command);
        }
        self.redraw();
    }

    /// The move menu reads this tick's presses: the cursor moves, a slot is used, or it closes.
    fn menu_tick(&mut self, cursor: usize, pressed: ButtonState) {
        match input::menu_input(pressed, cursor) {
            MenuInput::Stay => {}
            MenuInput::Cursor(to) => self.menu = Some(to),
            MenuInput::Close => self.menu = None,
            MenuInput::Use(slot) => {
                self.menu = None;
                self.take_turn(Command::UseMove {
                    slot: u8::try_from(slot).unwrap_or(0),
                    direction: None,
                });
            }
        }
    }

    /// Steps the sim with `command`. What the step did is shown: a move starts its walk, an
    /// attack its poses and log lines, the stairs take the next floor, and a cleared dungeon
    /// goes back to the menu.
    fn take_turn(&mut self, command: Command) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let before = game.dungeon().and_then(leader_tile);
        let wild_before = game
            .dungeon()
            .map(|d| d.wild().to_vec())
            .unwrap_or_default();
        let outcome = match self.tables.as_ref() {
            Some(t) => game.step_with(command, &t.data()),
            None => game.step(command),
        };
        let after = game.dungeon().and_then(leader_tile);
        let wild_after = game
            .dungeon()
            .map(|d| d.wild().to_vec())
            .unwrap_or_default();
        match outcome {
            Outcome::Applied => {
                if let (Some(from), Some(to), Some(dir)) = (before, after, moved_in(command)) {
                    self.start_walk(from, to, dir);
                } else if let Some(dir) = attacked_in(command, self.facing) {
                    self.show_attack(dir, &wild_before, &wild_after);
                }
            }
            Outcome::NextFloor => self.next_floor(),
            Outcome::DungeonCleared | Outcome::LeaderDefeated => self.back_to_menu(),
            Outcome::Blocked(_) | Outcome::NotModelled => {}
        }
    }

    /// Shows an attack facing `dir`: the leader's strike, a hit reaction on each monster that
    /// lost HP, and the log lines. A defeated monster is gone from the sim's list, so it is no
    /// longer drawn. An attack that changed nothing missed.
    fn show_attack(&mut self, dir: Direction, before: &[Monster], after: &[Monster]) {
        self.facing = dir;
        self.strike = self
            .data
            .as_ref()
            .and_then(|d| d.leader_strike(ATTACK_GROUP, dir));
        let mut lines = combat::attack_messages(before, after);
        if lines.is_empty() {
            // shortcut: a placeholder line, as the ROM's strings are not read (`combat`).
            lines.push(String::from("The attack missed."));
        }
        for pos in combat::hit_tiles(before, after) {
            let species = before.iter().find(|m| m.pos == pos).map(|m| m.info.species);
            let anim = species
                .and_then(|sp| self.art_of(sp))
                .and_then(|a| a.anim(HIT_GROUP, 0, false));
            if let Some(anim) = anim {
                self.hits.push(HitAnim { pos, anim });
            }
        }
        for line in &lines {
            println!("rescue: {line}");
        }
        self.log.extend(lines);
        let extra = self.log.len().saturating_sub(LOG_LINES);
        self.log.drain(..extra);
    }

    /// The sprite of wild species `species`, read from the dump the first time it is asked for.
    fn art_of(&mut self, species: u16) -> Option<&MonsterArt> {
        if !self.art.contains_key(&species) {
            let rom = self.rom.as_deref()?;
            match MonsterArt::from_rom(rom, species) {
                Ok(art) => {
                    self.art.insert(species, art);
                }
                Err(e) => {
                    eprintln!("rescue: species {species} is not drawn: {e}");
                    return None;
                }
            }
        }
        self.art.get(&species)
    }

    /// The wild monsters in view, each at its idle pose or its hit reaction.
    fn figures(&mut self) -> Vec<Figure> {
        let wild: Vec<Monster> = self
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .map(|d| d.wild().to_vec())
            .unwrap_or_default();
        let mut out = Vec::new();
        for m in wild.iter().filter(|m| m.visible) {
            let hit = self
                .hits
                .iter()
                .find(|h| h.pos == m.pos)
                .map(|h| h.anim.clone());
            let ticks = self.ticks;
            let dir = usize::from(m.info.direction) % 8;
            let Some(art) = self.art_of(m.info.species) else {
                continue;
            };
            let anim = hit.or_else(|| {
                let mut idle = art.anim(IDLE_GROUP, dir, true)?;
                idle.advance(ticks);
                Some(idle)
            });
            let Some(anim) = anim else {
                continue;
            };
            let Some(image) = art.image(&anim) else {
                continue;
            };
            let step = anim.step().offset;
            out.push(Figure {
                cell: (i32::from(m.pos.0), i32::from(m.pos.1)),
                offset: (i32::from(step.0), i32::from(step.1)),
                image,
            });
        }
        out
    }

    /// Starts the walk of the leader from `from` to `to`, one leg a tile, facing `dir`. The walk
    /// holds the leader's place on screen until it ends (`walk::Walk`).
    fn start_walk(&mut self, from: (i32, i32), to: (i32, i32), dir: Direction) {
        let legs = to.0.abs_diff(from.0).max(to.1.abs_diff(from.1));
        let Some(walk) = Walk::new(from, dir, legs) else {
            return;
        };
        self.facing = dir;
        self.walk = Some(walk);
        self.anim = self
            .data
            .as_ref()
            .and_then(|d| d.leader_anim(WALK_GROUP, dir));
    }

    /// The stairs took the leader to the next floor: the sim has built it, and the view and the
    /// ROM's floor follow. Without the ROM's floor the menu stays up.
    fn next_floor(&mut self) {
        let floor = self.floor.saturating_add(1);
        let data = if self.rom.is_some() {
            match self.load_floor(floor) {
                Ok(data) => Some(data),
                Err(e) => {
                    eprintln!("rescue: the next floor did not load: {e}");
                    return self.back_to_menu();
                }
            }
        } else {
            None
        };
        self.floor = floor;
        self.data = data;
        if !self.show_floor() {
            self.back_to_menu();
        }
    }

    /// Draws the top screen with the leader at its pose (the walk's, or standing on the sim's
    /// tile; the attack pose over either) and the wild monsters.
    fn redraw(&mut self) {
        let figures = self.figures();
        let (Some(view), Some(data)) = (self.dungeon.as_mut(), self.data.as_ref()) else {
            return;
        };
        let Some(anim) = self.strike.as_ref().or(self.anim.as_ref()) else {
            return;
        };
        let (cell, sub) = match &self.walk {
            Some(walk) => walk.pose(),
            None => {
                let tile = self
                    .game
                    .as_ref()
                    .and_then(|g| g.dungeon())
                    .and_then(leader_tile);
                (tile.unwrap_or((0, 0)), (0, 0))
            }
        };
        view.draw(data, (anim, data.leader_image(anim)), cell, sub, &figures);
    }

    /// Drops the run and goes back to the menu's first entry.
    fn back_to_menu(&mut self) {
        self.leave_dungeon();
        self.scene = Scene::Menu { cursor: 0 };
    }

    /// Drops the run and its view, as leaving the dungeon does.
    fn leave_dungeon(&mut self) {
        self.game = None;
        self.dungeon = None;
        self.data = None;
        self.walk = None;
        self.anim = None;
        self.strike = None;
        self.hits.clear();
        self.art.clear();
        self.menu = None;
        self.log.clear();
    }
}

/// The leader's tile on the sim's floor.
fn leader_tile(run: &rescue_sim::state::Dungeon) -> Option<(i32, i32)> {
    let leader = run.team().first()?;
    Some((i32::from(leader.pos.0), i32::from(leader.pos.1)))
}

/// The direction a command moves the leader in, if it moves it.
fn moved_in(command: Command) -> Option<Direction> {
    match command {
        Command::Move(dir) | Command::Run(dir) => Some(dir),
        _ => None,
    }
}

/// The direction an attack command strikes in, if `command` is one.
fn attacked_in(command: Command, facing: Direction) -> Option<Direction> {
    match command {
        Command::Attack(dir) => Some(dir),
        Command::UseMove { direction, .. } => Some(direction.unwrap_or(facing)),
        _ => None,
    }
}

/// The floors the sim counts for a run that starts on floor `start`. The sim numbers its floors
/// from 1 (`GameState::floors_entered`) and takes the next floor from index `floors_entered + 1`,
/// so its list is the placeholder (index 0), then the records from the start floor on: the start
/// floor is at index 1.
fn sim_floors(records: &[FloorRecord], start: u8) -> Vec<FloorRecord> {
    let mut floors: Vec<FloorRecord> = records.first().copied().into_iter().collect();
    floors.extend(records.iter().skip(usize::from(start)).copied());
    floors
}

/// The leader as the sim's team: on `pos`, of species `species`.
// shortcut: the leader's level, stats, IQ and regeneration are not read from the ROM yet. Level
// 1 with 100 HP stands in, and the belly starts at a floor's full 100.000. Upgrade when the
// species' stats and the save set the leader.
fn leader_monster(pos: (i16, i16), species: u16) -> Monster {
    Monster {
        pos,
        visible: true,
        info: EntityInfo {
            species,
            apparent_species: species,
            not_team_member: false,
            is_team_leader: true,
            level: 1,
            iq: 0,
            hp: 100,
            max_hp: 100,
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
            // Direction 0, south: the facing of a new run.
            direction: 0,
            belly: 100_000,
            belly_empty: false,
            regen_acc: 0,
            regen_speed: 0,
            exp: 0,
            exp_gained: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_data::dungeon::FLOOR_PROPERTIES;
    use rescue_nds::rom::RomId;
    use rescue_platform::DsButton;
    use rescue_sim::floor::{HEIGHT, TERRAIN_STAIRS, WIDTH};

    use crate::walk::TILE_FRAMES;

    const RUN: RunConfig = RunConfig {
        seed: dungeon::SEED,
        dungeon: dungeon::DEFAULT_DUNGEON,
        floor: dungeon::DEFAULT_FLOOR,
        species: dungeon::DEFAULT_SPECIES,
    };

    fn held(b: DsButton) -> ButtonState {
        buttons(&[b])
    }

    fn buttons(list: &[DsButton]) -> ButtonState {
        let mut s = ButtonState::default();
        for b in list {
            s.set(*b, true);
        }
        s
    }

    fn tap(app: &mut App, b: DsButton) {
        app.tick(held(b));
        app.tick(ButtonState::default());
    }

    fn ready() -> RomCheck {
        RomCheck::Ready(RomId::BlueRescueTeamEur)
    }

    /// The leader's tile on the run's floor.
    fn leader(app: &App) -> Option<(i16, i16)> {
        let run = app.game.as_ref()?.dungeon()?;
        run.team().first().map(|m| m.pos)
    }

    /// Puts the leader on `tile` of the run's floor: a test shortcut, so the walk to it is not
    /// run here. The run keeps its floor records.
    fn place_leader(app: &mut App, tile: (i16, i16)) {
        let game = app.game.take().expect("a run is up");
        let map = game.dungeon().expect("a floor").floor().clone();
        let seed = game.run_seed();
        let floors = game.floors().to_vec();
        let team = vec![leader_monster(tile, 0)];
        let placed = GameState::in_dungeon(seed, map, team, 0).expect("the team fits");
        app.game = Some(placed.with_floors(floors));
    }

    /// The tiles of the run's floor that have the stairs flag.
    fn stairs_tiles(app: &App) -> Vec<(i16, i16)> {
        let map = app
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .expect("a floor")
            .floor();
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.tile(x as i32, y as i32)
                    .is_some_and(|t| t.terrain & TERRAIN_STAIRS != 0)
            })
            .map(|(x, y)| (x as i16, y as i16))
            .collect()
    }

    /// The first stairs tile of the run's floor.
    fn stairs_tile(app: &App) -> (i16, i16) {
        *stairs_tiles(app).first().expect("the floor has stairs")
    }

    /// A tile next to a stairs tile, and the D-pad button that steps from it onto the stairs. The
    /// sim decides which steps are valid, so each candidate is tried on a copy of the run.
    fn approach_to_stairs(app: &App) -> ((i16, i16), DsButton) {
        let steps = [
            (DsButton::Up, (0, -1)),
            (DsButton::Down, (0, 1)),
            (DsButton::Left, (-1, 0)),
            (DsButton::Right, (1, 0)),
        ];
        for stairs in stairs_tiles(app) {
            for (button, (dx, dy)) in steps {
                let start = (stairs.0 - dx, stairs.1 - dy);
                let mut trial = app.clone();
                place_leader(&mut trial, start);
                trial.tick(held(button));
                if leader(&trial) == Some(stairs) {
                    return (start, button);
                }
            }
        }
        panic!("no tile next to the stairs steps onto them");
    }

    /// Walks the leader from a tile next to the stairs onto them, holding the direction the whole
    /// way as a player does. Checks the walk on the way; the last tick ends the walk and the
    /// stairs take the leader, so the caller checks the outcome.
    fn walk_onto_the_stairs(app: &mut App) {
        let (start, button) = approach_to_stairs(app);
        place_leader(app, start);
        let stairs = stairs_tile(app);
        app.tick(held(button));
        assert_eq!(
            leader(app),
            Some(stairs),
            "the move puts the leader on the stairs"
        );
        assert!(app.walk.is_some(), "the move starts a walk");
        for frame in 1..TILE_FRAMES {
            app.tick(held(button));
            assert_eq!(
                app.scene,
                Scene::Dungeon,
                "the walk holds the scene, frame {frame}"
            );
            assert!(app.walk.is_some(), "the walk runs, frame {frame}");
        }
        app.tick(held(button));
    }

    #[test]
    fn opens_on_the_dungeon_when_asked() {
        let app = App::new(&ready(), None, RUN, true);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.dungeon().is_some(), "the floor is built at start");
        let refused = App::new(&RomCheck::NotConfigured, None, RUN, true);
        assert_eq!(refused.scene, Scene::Refused);
    }

    #[test]
    fn boot_to_the_dungeon_headless_and_b_does_not_leave_it() {
        let mut app = App::new(&ready(), None, RUN, false);
        assert_eq!(app.scene, Scene::Splash { ticks: 0 });
        for _ in 0..crate::scene::SPLASH_TICKS {
            app.tick(ButtonState::default());
        }
        assert_eq!(app.scene, Scene::Title);
        tap(&mut app, DsButton::Start);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        tap(&mut app, DsButton::Down);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::AdventureLog);
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
        tap(&mut app, DsButton::Up);
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.dungeon().is_some(), "entering builds the floor");
        // B is run, not back: the dungeon stays up until the stairs clear it.
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Dungeon);
    }

    #[test]
    fn a_held_button_counts_once() {
        let mut app = App::new(&ready(), None, RUN, false);
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Title);
        // Still held: must not also fire the title's press on the next tick.
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Title);
    }

    #[test]
    fn refused_rom_quits_on_press() {
        let mut app = App::new(&RomCheck::NotConfigured, None, RUN, false);
        assert_eq!(app.scene, Scene::Refused);
        assert_eq!(app.tick(ButtonState::default()), Flow::Continue);
        assert_eq!(app.tick(held(DsButton::A)), Flow::Quit);
    }

    #[test]
    fn a_move_walks_a_tile_and_inputs_wait_for_the_walk() {
        let mut app = App::new(&ready(), None, RUN, true);
        // Two ordinary tiles side by side: the terrain class is 1 and no wall is set.
        let map = app
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .expect("a floor")
            .floor()
            .clone();
        let ground = |x: usize, y: usize| {
            map.tile(x as i32, y as i32)
                .is_some_and(|t| (t.terrain & 0x13) == 1)
        };
        let start = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH - 1).map(move |x| (x, y)))
            .find(|&(x, y)| ground(x, y) && ground(x + 1, y))
            .map(|(x, y)| (x as i16, y as i16))
            .expect("two ground tiles side by side");
        place_leader(&mut app, start);

        app.tick(held(DsButton::Right));
        let ahead = (start.0 + 1, start.1);
        assert_eq!(leader(&app), Some(ahead), "the sim is on the new tile");
        let walk = app.walk.expect("the move starts a walk");
        assert_eq!(
            walk.pose(),
            ((i32::from(start.0), i32::from(start.1)), (0, 0))
        );
        for frame in 1..24 {
            app.tick(held(DsButton::Right));
            assert_eq!(
                leader(&app),
                Some(ahead),
                "no input is read during the walk, frame {frame}"
            );
        }
    }

    #[test]
    fn the_stairs_clear_a_dungeon_with_no_next_floor() {
        // The placeholder has no floor records, so its stairs clear the dungeon.
        let mut app = App::new(&ready(), None, RUN, true);
        let tile = stairs_tile(&app);
        place_leader(&mut app, tile);
        app.tick(buttons(&[DsButton::A, DsButton::B]));
        assert_eq!(
            app.scene,
            Scene::Dungeon,
            "standing on the stairs waits for the next input"
        );
        app.tick(buttons(&[DsButton::A, DsButton::B]));
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        assert!(app.dungeon().is_none(), "the dungeon is dropped");
    }

    #[test]
    fn a_held_move_onto_the_stairs_takes_them_after_the_walk() {
        // The placeholder has no floor records, so its stairs clear the dungeon.
        let mut app = App::new(&ready(), None, RUN, true);
        walk_onto_the_stairs(&mut app);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        assert!(app.dungeon().is_none(), "the dungeon is dropped");
    }

    #[test]
    fn the_sim_floors_start_at_the_start_floor() {
        let records: Vec<FloorRecord> = (0..4u8).map(|i| [i; FLOOR_PROPERTIES]).collect();
        assert_eq!(sim_floors(&records, 1), records);
        assert_eq!(
            sim_floors(&records, 2),
            vec![records[0], records[2], records[3]]
        );
        assert!(sim_floors(&[], 1).is_empty());
    }

    rescue_tests::rom_test! {
        /// The stairs of floor 1 from the ROM take the leader to floor 2: the sim builds the next
        /// floor, and the ROM's floor and view follow (#347 P3).
        fn the_stairs_take_the_rom_floor_to_the_next(rom) {
            let mut app = App::new(&ready(), Some(rom.read()), RUN, true);
            walk_onto_the_stairs(&mut app);
            assert_eq!(app.scene, Scene::Dungeon);
            assert!(app.walk.is_none(), "the walk has ended");
            assert_eq!(app.floor, 2, "the view is on floor 2");
            assert_eq!(app.game.as_ref().map(|g| g.floors_entered()), Some(2));
            assert!(app.dungeon().is_some(), "the floor view is rebuilt");
        }
    }

    #[test]
    fn attack_with_no_target_changes_nothing_and_the_menu_opens_and_closes() {
        let mut app = App::new(&ready(), None, RUN, true);
        // No ROM: no tables, so an attack is not modelled; nothing is logged.
        app.tick(held(DsButton::A));
        assert!(app.log().is_empty());
        app.tick(ButtonState::default());
        tap(&mut app, DsButton::X);
        assert_eq!(app.menu(), Some(0));
        tap(&mut app, DsButton::Down);
        assert_eq!(app.menu(), Some(1));
        tap(&mut app, DsButton::B);
        assert_eq!(app.menu(), None);
    }

    #[test]
    fn an_attack_logs_hits_and_defeats_and_keeps_four_lines() {
        use crate::combat::attack_messages;
        let mut app = App::new(&ready(), None, RUN, true);
        let mut a = leader_monster((1, 1), 5);
        let mut b = a.clone();
        a.info.hp = 20;
        b.info.hp = 12;
        app.show_attack(Direction::Right, &[a.clone()], &[b.clone()]);
        assert_eq!(app.log(), attack_messages(&[a.clone()], &[b]).as_slice());
        app.show_attack(Direction::Right, &[a.clone()], &[]);
        assert_eq!(
            app.log().last().map(String::as_str),
            Some("Monster 5 was defeated!")
        );
        app.show_attack(Direction::Right, &[a.clone()], &[a.clone()]);
        assert_eq!(
            app.log().last().map(String::as_str),
            Some("The attack missed.")
        );
        for _ in 0..5 {
            app.show_attack(Direction::Left, &[], &[]);
        }
        assert_eq!(app.log().len(), 4);
    }
}
