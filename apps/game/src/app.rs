//! The game state the platform loop drives. No SDL here, so the boot flow runs headless.
//!
//! In the dungeon the sim owns the run (`GameState`). This owns what the sim does not: the
//! floor view, and the leader's walk on screen. A walk in progress is advanced each tick; when
//! none is, the buttons held are read as a command (`input`) and the sim is stepped. What the
//! step did starts a walk (a move), moves on to the next floor (the stairs) or goes back to the
//! menu (the dungeon is cleared).

use rescue_platform::{ButtonState, Flow};
use rescue_render::anim::AnimPlayer;
use rescue_sim::command::{Command, Direction, Outcome};
use rescue_sim::state::{EntityInfo, FloorRecord, GameState, Monster};

use crate::boot::RomCheck;
use crate::dungeon::{self, Dungeon, FloorData, IDLE_GROUP, WALK_GROUP};
use crate::input;
use crate::scene::Scene;
use crate::walk::Walk;

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
                self.step_dungeon(buttons);
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
        let (species, speed, records) = match self.data.as_ref() {
            Some(d) => (self.config.species, d.leader_speed(), d.records()),
            None => (0, 0, &[][..]),
        };
        let floors = sim_floors(records, floor);
        let team = vec![leader_monster(leader, species)];
        let Ok(game) = GameState::in_dungeon(self.config.seed, map, team, speed) else {
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
        self.anim = self
            .data
            .as_ref()
            .and_then(|d| d.leader_anim(IDLE_GROUP, self.facing));
        self.redraw();
        self.dungeon.is_some()
    }

    /// One logic tick of the dungeon: the animation runs, and a walk in progress is advanced.
    /// When no walk is left after that (or it ends on this tick), the buttons held are read.
    fn step_dungeon(&mut self, buttons: ButtonState) {
        if let Some(anim) = self.anim.as_mut() {
            anim.advance(1);
        }
        if self.walk.as_mut().is_some_and(Walk::tick) {
            self.walk = None;
            self.anim = self
                .data
                .as_ref()
                .and_then(|d| d.leader_anim(IDLE_GROUP, self.facing));
        }
        if self.walk.is_none() {
            self.take_turn(buttons);
        }
        self.redraw();
    }

    /// Reads the buttons held as a command and steps the sim with it. What the step did is
    /// shown: a move starts its walk, the stairs take the next floor, and a cleared dungeon goes
    /// back to the menu.
    fn take_turn(&mut self, buttons: ButtonState) {
        let Some(command) = input::command_for(buttons) else {
            return;
        };
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let before = game.dungeon().and_then(leader_tile);
        let outcome = game.step(command);
        let after = game.dungeon().and_then(leader_tile);
        match outcome {
            Outcome::Applied => {
                if let (Some(from), Some(to), Some(dir)) = (before, after, moved_in(command)) {
                    self.start_walk(from, to, dir);
                }
            }
            Outcome::NextFloor => self.next_floor(),
            Outcome::DungeonCleared => self.back_to_menu(),
            Outcome::Blocked(_) | Outcome::NotModelled => {}
        }
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

    /// Draws the top screen with the leader at its pose: the walk's, or standing on the sim's
    /// tile.
    fn redraw(&mut self) {
        let (Some(view), Some(data), Some(anim)) = (
            self.dungeon.as_mut(),
            self.data.as_ref(),
            self.anim.as_ref(),
        ) else {
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
        view.draw(data, anim, cell, sub);
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
            offensive_stages: [0, 0],
            speed_up: [0; 5],
            speed_down: [0; 5],
            // Direction 0, south: the facing of a new run.
            direction: 0,
            belly: 100_000,
            belly_empty: false,
            regen_acc: 0,
            regen_speed: 0,
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

    /// The first tile of the run's floor that has the stairs flag.
    fn stairs_tile(app: &App) -> (i16, i16) {
        let map = app
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .expect("a floor")
            .floor();
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| {
                map.tile(x as i32, y as i32)
                    .is_some_and(|t| t.terrain & TERRAIN_STAIRS != 0)
            })
            .map(|(x, y)| (x as i16, y as i16))
            .expect("the floor has stairs")
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
            let tile = stairs_tile(&app);
            place_leader(&mut app, tile);
            app.tick(buttons(&[DsButton::A, DsButton::B]));
            app.tick(buttons(&[DsButton::A, DsButton::B]));
            assert_eq!(app.scene, Scene::Dungeon);
            assert_eq!(app.floor, 2, "the view is on floor 2");
            assert_eq!(app.game.as_ref().map(|g| g.floors_entered()), Some(2));
            assert!(app.dungeon().is_some(), "the floor view is rebuilt");
        }
    }
}
