//! The game state the platform loop drives. No SDL here, so the boot flow runs headless.
//!
//! In the dungeon the sim owns the run (`GameState`). This owns what the sim does not: the
//! floor view, and the leader's walk on screen. A walk in progress is advanced each tick; when
//! none is, the buttons held are read as a command (`input`) and the sim is stepped. What the
//! step did starts a walk (a move), moves on to the next floor (the stairs) or goes back to the
//! menu (the dungeon is cleared).

use std::collections::BTreeMap;
use std::path::PathBuf;

use rescue_data::dungeon::MonsterSpawn;
use rescue_platform::{ButtonState, Flow, Screen};
use rescue_render::anim::AnimPlayer;
use rescue_render::bg::Frame;
use rescue_render::minimap::{MonsterInfo, MonsterMark, classify};
use rescue_sim::command::{Command, Direction, Outcome};
use rescue_sim::floor::Floor;
use rescue_sim::rng::GlobalRng;
use rescue_sim::state::{EntityInfo, FloorRecord, GameState, Monster, MoveSlot};

use crate::boot::RomCheck;
use crate::combat::{self, Act, ActKind, LOG_LINES, Tables};
use crate::dungeon::{self, Dungeon, Figure, FloorData, IDLE_GROUP, MonsterArt, WALK_GROUP};
use crate::input::{self, MenuInput};
use crate::save;
use crate::scene::{MenuItem, Scene};
use crate::title::{self, MenuClock, MenuView, PanelView, TitleArt};
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

/// A monster's act playing on screen: the monster is the one of `species` on `pos`, the tile it
/// ends the turn on. A walk draws it moving there.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fx {
    species: u16,
    pos: (i16, i16),
    walk: Option<Walk>,
    anim: AnimPlayer,
    done: bool,
}

/// Where a run starts: its seed, the dungeon and floor of the options, and the leader's species.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunConfig {
    pub seed: u32,
    pub dungeon: u8,
    pub floor: u8,
    pub species: u16,
}

/// Where the menu goes when its fade out ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leave {
    Scene(Scene),
    /// Load the suspend save and resume.
    Continue,
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
    /// The monsters' walks and strikes of the last turn, while they play.
    fx: Vec<Fx>,
    /// Sprites of the wild species met on this floor.
    art: BTreeMap<u16, MonsterArt>,
    /// Logic ticks in the dungeon, which time the monsters' idle poses.
    ticks: u32,
    /// The move menu's cursor while it is open.
    menu: Option<usize>,
    /// The last lines of the message log.
    log: Vec<String>,
    /// The suspend save's file. `None`: no suspend, no Continue.
    save_path: Option<PathBuf>,
    /// Whether the save file exists, which adds Continue to the menu.
    has_save: bool,
    /// Why the last suspend or Continue failed, shown on the menu until the next try.
    notice: Option<String>,
    /// The title screens' art, once read from the dump. `None`: the placeholders are drawn.
    title: Option<TitleArt>,
    /// The menu screen's fade and blink, while the menu is up.
    clock: Option<MenuClock>,
    /// The adventure log's lines while that screen is up, read from the save.
    adventure_log: Option<Vec<String>>,
    /// Where the menu goes once it has faded out.
    leave: Option<Leave>,
    /// The menu backdrop of this visit, `FUN_02011968(3)` from the global generator.
    backdrop: usize,
    /// The global generator, which draws the backdrop.
    // shortcut: seeded from the run's seed; the game seeds it from the console's MAC address.
    rng: GlobalRng,
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
            fx: Vec::new(),
            art: BTreeMap::new(),
            ticks: 0,
            menu: None,
            log: Vec::new(),
            save_path: None,
            has_save: false,
            adventure_log: None,
            notice: None,
            title: None,
            clock: None,
            leave: None,
            backdrop: 0,
            rng: GlobalRng::from_state(config.seed),
        };
        app.title = app.rom.as_deref().and_then(|rom| {
            TitleArt::from_rom(rom)
                .map_err(|e| eprintln!("rescue: no title art, placeholders are drawn: {e}"))
                .ok()
        });
        if app.scene == Scene::Dungeon {
            app.enter_dungeon();
        }
        app
    }

    /// Sets the suspend save's file; Continue is offered while it exists.
    #[must_use]
    pub fn with_save_path(mut self, path: Option<PathBuf>) -> App {
        self.has_save = path.as_deref().is_some_and(|p| p.is_file());
        self.save_path = path;
        self
    }

    /// The number of entries on the main menu.
    pub fn menu_items(&self) -> usize {
        MenuItem::list(self.has_save).len()
    }

    /// Why the last suspend or Continue failed, if it did.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
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
        if self.menu_busy(&mut pressed) {
            return Flow::Continue;
        }
        if self.continue_pressed(pressed) {
            self.start_leave(Leave::Continue);
        } else {
            let mut next = self.scene.step_with(pressed, self.has_save);
            if matches!(self.scene, Scene::ConfirmDelete { yes: true })
                && matches!(next, Scene::Menu { .. })
                && (pressed.is_held(rescue_platform::DsButton::A)
                    || pressed.is_held(rescue_platform::DsButton::Start))
                && !self.delete_save()
            {
                // The save could not be deleted: back to the Delete row, which still shows it.
                next = Scene::Menu { cursor: 1 };
            }
            if matches!(self.scene, Scene::Menu { .. }) && !matches!(next, Scene::Menu { .. }) {
                self.start_leave(Leave::Scene(next));
            } else {
                self.scene = next;
            }
        }
        self.track_menu();
        self.track_log();
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
        self.track_menu();
        Flow::Continue
    }

    /// Runs the menu screen's fade for this tick. `true` while it is fading: input is not read.
    /// The tick the fade out ends, the menu's choice happens and the tick goes on.
    fn menu_busy(&mut self, pressed: &mut ButtonState) -> bool {
        if !matches!(self.scene, Scene::Menu { .. }) {
            return false;
        }
        let Some(clock) = self.clock.as_mut() else {
            return false;
        };
        let was_ready = clock.ready();
        let done = clock.tick();
        if done {
            self.clock = None;
            match self.leave.take() {
                Some(Leave::Scene(s)) => self.scene = s,
                Some(Leave::Continue) => self.continue_run(),
                None => {}
            }
            self.track_menu();
            *pressed = ButtonState::default();
            return false;
        }
        !was_ready || self.leave.is_some()
    }

    /// Starts the menu's fade out towards `leave`.
    fn start_leave(&mut self, leave: Leave) {
        match self.clock.as_mut() {
            Some(clock) => {
                clock.leave();
                self.leave = Some(leave);
            }
            // No clock (the scene was set from outside): leave at once.
            None => match leave {
                Leave::Scene(s) => self.scene = s,
                Leave::Continue => self.continue_run(),
            },
        }
    }

    /// Keeps the menu clock in step with the scene: a menu that is up without one starts a fade
    /// in on a new backdrop, and the clock goes when the scene does.
    fn track_menu(&mut self) {
        if matches!(self.scene, Scene::Menu { .. }) {
            if self.clock.is_none() {
                self.backdrop = title::backdrop_from(self.rng.next_u32());
                self.clock = Some(MenuClock::new());
                self.leave = None;
            }
        } else {
            self.clock = None;
        }
    }

    /// Reads the adventure log's lines when its screen opens and drops them when it closes.
    fn track_log(&mut self) {
        if self.scene != Scene::AdventureLog {
            self.adventure_log = None;
        } else if self.adventure_log.is_none() {
            self.adventure_log = Some(self.read_log());
        }
    }

    /// The adventure log: what the save holds. A fresh boot has no save, so no adventure.
    // shortcut: the port's save holds one suspended run, so the log shows that run's floors
    // and team; Blue's career record (rank, dungeons cleared) is not kept yet.
    fn read_log(&self) -> Vec<String> {
        let mut lines = vec![String::from("Adventure Log")];
        let save = self.save_path.as_deref().filter(|_| self.has_save);
        match save.map(save::read) {
            None => lines.push(String::from("No adventure has been saved yet.")),
            Some(Err(_)) => lines.push(String::from("The save could not be read.")),
            Some(Ok(game)) => {
                lines.push(format!("Floors entered: {}", game.floors_entered()));
                lines.push(format!(
                    "Team members: {}",
                    game.dungeon().map_or(0, |d| d.team().len())
                ));
            }
        }
        lines.push(String::from("Press B to go back."));
        lines
    }

    /// Deletes the save file. `false` (and a notice) if it could not be removed.
    fn delete_save(&mut self) -> bool {
        self.notice = None;
        if let Some(path) = self.save_path.as_deref() {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    self.refuse(format!("the save was not deleted: {}: {e}", path.display()));
                    return false;
                }
            }
        }
        self.has_save = false;
        true
    }

    /// The title screens' frame for `screen`: the logo screen on the title, the menu screen on
    /// the menu. `None` when another scene is up or the art is not loaded.
    pub fn title_frame(&self, screen: Screen) -> Option<Frame> {
        let art = self.title.as_ref()?;
        match (self.scene, screen) {
            (Scene::Title, Screen::Bottom) => Some(art.logo_frame()),
            (Scene::Title, Screen::Top) => Some(TitleArt::black()),
            (Scene::Menu { cursor }, _) => {
                let clock = self.clock?;
                if screen == Screen::Top {
                    return Some(art.menu_top(clock.counter));
                }
                Some(art.menu_bottom(&MenuView {
                    backdrop: self.backdrop,
                    fade: clock.counter,
                    windows: clock.windows(),
                    items: MenuItem::list(self.has_save),
                    cursor,
                    blink: clock.blink,
                }))
            }
            (Scene::ConfirmDelete { yes }, _) => {
                if screen == Screen::Top {
                    return Some(art.menu_top(title::FADE_FRAMES));
                }
                let lines = [
                    String::from("Delete all of your save data?"),
                    String::from("It cannot be restored."),
                ];
                Some(art.panel_bottom(&PanelView {
                    backdrop: self.backdrop,
                    lines: &lines,
                    choices: &["No", "Yes"],
                    cursor: usize::from(yes),
                }))
            }
            (Scene::Unavailable { cursor }, _) => {
                if screen == Screen::Top {
                    return Some(art.menu_top(title::FADE_FRAMES));
                }
                let name = MenuItem::list(self.has_save).get(cursor)?.name();
                let lines = [
                    format!("{name} is not available"),
                    String::from("in this version."),
                    String::from("It needs the DS link or the"),
                    String::from("rescue password system."),
                    String::from("Press A or B."),
                ];
                Some(art.panel_bottom(&PanelView {
                    backdrop: self.backdrop,
                    lines: &lines,
                    choices: &[],
                    cursor: 0,
                }))
            }
            (Scene::AdventureLog, _) => {
                if screen == Screen::Top {
                    return Some(art.menu_top(title::FADE_FRAMES));
                }
                Some(art.panel_bottom(&PanelView {
                    backdrop: self.backdrop,
                    lines: self.adventure_log.as_deref()?,
                    choices: &[],
                    cursor: 0,
                }))
            }
            _ => None,
        }
    }

    /// Whether this tick confirms Continue on the menu.
    fn continue_pressed(&self, pressed: ButtonState) -> bool {
        let confirm = pressed.is_held(rescue_platform::DsButton::A)
            || pressed.is_held(rescue_platform::DsButton::Start);
        confirm
            && matches!(self.scene, Scene::Menu { cursor }
                if MenuItem::list(self.has_save).get(cursor) == Some(&MenuItem::Continue))
    }

    /// Loads the suspend save and resumes the run on the turn it was suspended on. A save that
    /// does not read is reported and not loaded; the menu stays up.
    fn continue_run(&mut self) {
        self.notice = None;
        let Some(path) = self.save_path.clone() else {
            return;
        };
        let game = match save::read(&path) {
            Ok(game) => game,
            Err(e) => return self.refuse(format!("the save was not loaded: {e}")),
        };
        // The run's floor on the ROM: the start floor, then one per set of stairs taken.
        let climbed = game.floors_entered().saturating_sub(1);
        let floor = u8::try_from(u32::from(self.config.floor) + climbed).unwrap_or(u8::MAX);
        let data = if self.rom.is_some() {
            match self.load_floor(floor) {
                Ok(data) => Some(data),
                Err(e) => return self.refuse(format!("the saved floor did not load: {e}")),
            }
        } else {
            None
        };
        self.leave_dungeon();
        self.floor = floor;
        self.data = data;
        self.facing = Direction::Down;
        if self.tables.is_none() {
            self.tables = self.rom.as_deref().and_then(|rom| {
                Tables::from_rom(rom, self.config.species)
                    .map_err(|e| eprintln!("rescue: attacks are not modelled, no tables: {e}"))
                    .ok()
            });
        }
        self.game = Some(game);
        if self.show_floor() {
            self.scene = Scene::Dungeon;
        } else {
            self.leave_dungeon();
            self.refuse(String::from("the saved run has no floor to show"));
        }
    }

    /// Writes the run to the suspend save and goes back to the title. If it cannot be written
    /// the run goes on and the reason is reported.
    fn suspend(&mut self) {
        self.notice = None;
        let (Some(path), Some(game)) = (self.save_path.clone(), self.game.as_ref()) else {
            return self.refuse(String::from("there is nowhere to save the run"));
        };
        match save::write(&path, game) {
            Ok(()) => {
                self.has_save = true;
                self.leave_dungeon();
                self.scene = Scene::Title;
            }
            Err(e) => self.refuse(format!("the run was not saved: {e}")),
        }
    }

    fn refuse(&mut self, why: String) {
        eprintln!("rescue: {why}");
        self.notice = Some(why);
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
        let (species, speed, records, wild, sleep) = match self.data.as_ref() {
            Some(d) => (
                self.config.species,
                d.leader_speed(),
                d.records(),
                d.wild_spawns(),
                d.sleep_chances(),
            ),
            None => (0, 0, &[][..], &[][..], &[][..]),
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
        let seed = self.config.seed;
        let Some(game) = start_run(seed, (map, team, speed), (wild, sleep), floors) else {
            return self.back_to_menu();
        };
        self.game = Some(game);
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
        self.fx.clear();
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
        for fx in &mut self.fx {
            fx.anim.advance(1);
            // A walk ends with its legs; a strike or a hit ends with its animation.
            fx.done = match fx.walk.as_mut() {
                Some(walk) => walk.tick(),
                None => fx.anim.finished(),
            };
        }
        self.fx.retain(|fx| !fx.done);
        if self.walk.as_mut().is_some_and(Walk::tick) {
            self.walk = None;
            self.anim = self
                .data
                .as_ref()
                .and_then(|d| d.leader_anim(IDLE_GROUP, self.facing));
        }
        let idle = self.walk.is_none()
            && self.strike.is_none()
            && self.hits.is_empty()
            && self.fx.is_empty();
        if let Some(cursor) = self.menu {
            self.menu_tick(cursor, pressed);
        } else if idle && input::opens_menu(pressed) {
            self.menu = Some(0);
        } else if idle
            && let Some(command) = input::command_for_press(buttons, pressed, self.facing)
        {
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
            MenuInput::Suspend => {
                self.menu = None;
                self.suspend();
            }
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
        let team_before = game
            .dungeon()
            .map(|d| d.team().to_vec())
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
        let team_after = game
            .dungeon()
            .map(|d| d.team().to_vec())
            .unwrap_or_default();
        match outcome {
            Outcome::Applied => {
                if let (Some(from), Some(to), Some(dir)) = (before, after, moved_in(command)) {
                    self.start_walk(from, to, dir);
                    // A partner's attack on a wild monster shows in the log.
                    let lines = combat::attack_messages(&wild_before, &wild_after);
                    self.log_lines(lines);
                } else if let Some(dir) = attacked_in(command, self.facing) {
                    self.show_attack(dir, &wild_before, &wild_after);
                }
                self.show_monsters((&team_before, &team_after), (&wild_before, &wild_after));
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
        self.log_lines(lines);
    }

    /// Prints `lines` and keeps the last of them in the message log.
    fn log_lines(&mut self, lines: Vec<String>) {
        for line in &lines {
            println!("rescue: {line}");
        }
        self.log.extend(lines);
        let extra = self.log.len().saturating_sub(LOG_LINES);
        self.log.drain(..extra);
    }

    /// Shows what the monsters did in the turn: their walks, strikes and reactions, and the log
    /// lines of their attacks and statuses (`combat::monster_acts`).
    fn show_monsters(&mut self, team: (&[Monster], &[Monster]), wild: (&[Monster], &[Monster])) {
        let (acts, lines) = combat::monster_acts(team, wild);
        for Act { species, pos, kind } in acts {
            let (group, dir, looping, walk) = match kind {
                ActKind::Walk(w) => (WALK_GROUP, w.dir() as usize, true, Some(w)),
                ActKind::Attack(d) => (ATTACK_GROUP, d as usize, false, None),
                ActKind::Hit => (HIT_GROUP, 0, false, None),
            };
            let anim = self
                .art_of(species)
                .and_then(|a| a.anim(group, dir, looping));
            if let Some(anim) = anim {
                self.fx.push(Fx {
                    species,
                    pos,
                    walk,
                    anim,
                    done: false,
                });
            }
        }
        self.log_lines(lines);
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
        // The partners (team slots after the leader), then the wild monsters.
        let monsters: Vec<Monster> = self
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .map(|d| d.team().iter().skip(1).chain(d.wild()).cloned().collect())
            .unwrap_or_default();
        let mut out = Vec::new();
        for m in monsters.iter().filter(|m| m.visible) {
            let fx = self
                .fx
                .iter()
                .find(|f| f.pos == m.pos && f.species == m.info.species)
                .map(|f| (f.anim.clone(), f.walk));
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
            let (fx_anim, walk) = fx.unzip();
            let walk = walk.flatten();
            let anim = hit.or(fx_anim).or_else(|| {
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
            // A walking monster is drawn from the tile its leg starts on.
            let (cell, sub) = walk
                .map_or(((i32::from(m.pos.0), i32::from(m.pos.1)), (0, 0)), |w| {
                    w.pose()
                });
            out.push(Figure {
                cell,
                offset: (i32::from(step.0) + sub.0, i32::from(step.1) + sub.1),
                image,
                mark: combat::has_status(&m.info).then(|| mark_of(&m.info)),
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
        let leader_mark = self
            .game
            .as_ref()
            .and_then(|g| g.dungeon())
            .and_then(|d| d.team().first())
            .filter(|l| combat::has_status(&l.info))
            .map(|l| mark_of(&l.info));
        view.draw(
            data,
            (anim, data.leader_image(anim)),
            cell,
            sub,
            &figures,
            leader_mark,
        );
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
        self.fx.clear();
        self.art.clear();
        self.menu = None;
        self.log.clear();
    }
}

/// The marker class of a monster: red for a wild one, yellow for a partner, white for the leader
/// (`render::minimap::classify`). The species and status bytes it also reads are left 0.
fn mark_of(info: &EntityInfo) -> MonsterMark {
    classify(MonsterInfo {
        species: 0,
        status: 0,
        hostile: u8::from(info.not_team_member),
        leader: u8::from(info.is_team_leader),
    })
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

/// The run on its first floor: the spawn list `wild`, the species sleep chances `sleep` (so a
/// natural spawn can start asleep) and the floor records `floors`. `None` if the team does not fit.
fn start_run(
    seed: u32,
    (map, team, speed): (Floor, Vec<Monster>, i32),
    (wild, sleep): (&[MonsterSpawn], &[u8]),
    floors: Vec<FloorRecord>,
) -> Option<GameState> {
    let game = GameState::in_dungeon_with_floors(seed, floors, map, team, speed, wild).ok()?;
    Some(game.with_species_sleep_chances(sleep.to_vec()))
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
            status: Default::default(),
            regen_speed: 0,
            exp: 0,
            exp_gained: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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

    /// Waits out the menu's fade in or out.
    fn fade(app: &mut App) {
        for _ in 0..crate::title::FADE_FRAMES {
            app.tick(ButtonState::default());
        }
    }

    /// A tap on the menu: waits for the fade in, taps, then waits for the fade out.
    fn choose(app: &mut App, b: DsButton) {
        fade(app);
        tap(app, b);
        fade(app);
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
        choose(&mut app, DsButton::Down);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
        choose(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::AdventureLog);
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
        choose(&mut app, DsButton::Up);
        choose(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.dungeon().is_some(), "entering builds the floor");
        // B is run, not back: the dungeon stays up until the stairs clear it.
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Dungeon);
    }

    #[test]
    fn the_menu_fades_in_and_ignores_input_until_it_is_up() {
        let mut app = App::new(&ready(), None, RUN, false);
        app.scene = Scene::Title;
        tap(&mut app, DsButton::Start);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        for _ in 0..crate::title::FADE_FRAMES {
            app.tick(held(DsButton::Down));
            app.tick(ButtonState::default());
        }
        assert_eq!(
            app.scene,
            Scene::Menu { cursor: 0 },
            "the fade in reads no input"
        );
        tap(&mut app, DsButton::Down);
        assert_eq!(app.scene, Scene::Menu { cursor: 1 });
    }

    #[test]
    fn a_choice_leaves_after_a_32_frame_fade_out() {
        let mut app = App::new(&ready(), None, RUN, false);
        app.scene = Scene::Title;
        tap(&mut app, DsButton::Start);
        fade(&mut app);
        app.tick(ButtonState::default());
        app.tick(held(DsButton::Start));
        assert_eq!(app.scene, Scene::Menu { cursor: 0 }, "the fade out starts");
        app.tick(held(DsButton::Down));
        for _ in 0..crate::title::FADE_FRAMES - 3 {
            app.tick(ButtonState::default());
            assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        }
        app.tick(ButtonState::default());
        app.tick(ButtonState::default());
        assert_eq!(app.scene, Scene::Dungeon);
    }

    #[test]
    fn each_visit_to_the_menu_draws_a_backdrop_from_the_global_generator() {
        let mut app = App::new(&ready(), None, RUN, false);
        let mut rng = GlobalRng::from_state(RUN.seed);
        app.scene = Scene::Title;
        tap(&mut app, DsButton::Start);
        assert_eq!(app.backdrop, title::backdrop_from(rng.next_u32()));
        choose(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Dungeon);
        app.back_to_menu();
        app.tick(ButtonState::default());
        assert_eq!(app.backdrop, title::backdrop_from(rng.next_u32()));
        assert!(app.backdrop < title::BACKDROPS);
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
    fn a_natural_spawn_can_start_asleep_in_the_game_path() {
        let (mut map, tile) = dungeon::first_floor(dungeon::SEED, None).expect("the floor builds");
        // No spawn marks: the floor starts with no wild monsters, so the 10-monster cap leaves
        // room for the natural spawns.
        for (x, y) in rescue_sim::floor::enemy_spawn_spots(&map) {
            if let Some(t) = map.tile_mut(i32::from(x), i32::from(y)) {
                t.spawn_or_visibility = 0;
            }
        }
        let wild = [MonsterSpawn {
            species: 7,
            level: 4,
            weights: [10_000, 0],
        }];
        // Species 7 has a sleep chance of 100: every roll passes.
        let sleep = [0, 0, 0, 0, 0, 0, 0, 100];
        // Floor records for numbers 0 and 1 (index 0 is the placeholder), enemy density 4.
        let floors: Vec<FloorRecord> = (0..2u8)
            .map(|_| {
                let mut r = [0; FLOOR_PROPERTIES];
                r[0x06] = 4;
                r
            })
            .collect();
        let asleep_after_waiting = |sleep: &[u8]| {
            let team = vec![leader_monster(tile, 0)];
            let mut game = start_run(1, (map.clone(), team, 2), (&wild, sleep), floors.clone())
                .expect("the team fits");
            assert_eq!(game.floors_entered(), 1);
            let initial = game.dungeon().map_or(0, |d| d.wild().len());
            for _ in 0..200 {
                game.step(Command::Wait);
                if let Some(m) = game.dungeon().and_then(|d| d.wild().get(initial)) {
                    return Some(m.info.is_asleep());
                }
            }
            None
        };
        assert_eq!(asleep_after_waiting(&sleep), Some(true));
        assert_eq!(asleep_after_waiting(&[]), Some(false));
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

    /// A fresh path for a save file, in the temp dir.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rescue-game-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("suspend.sav")
    }

    fn saving_app(path: &Path) -> App {
        App::new(&ready(), None, RUN, true).with_save_path(Some(path.to_path_buf()))
    }

    /// Taps `button`, then waits out any walk.
    fn step(app: &mut App, button: DsButton) {
        tap(app, button);
        for _ in 0..TILE_FRAMES * 2 {
            app.tick(ButtonState::default());
        }
    }

    const MOVES: [DsButton; 4] = [
        DsButton::Right,
        DsButton::Down,
        DsButton::Left,
        DsButton::Down,
    ];

    /// Opens the move menu and picks its last row, Suspend.
    fn suspend_from_menu(app: &mut App) {
        tap(app, DsButton::X);
        assert!(app.menu().is_some());
        tap(app, DsButton::Up);
        assert_eq!(app.menu(), Some(crate::input::MENU_SLOTS));
        tap(app, DsButton::A);
    }

    fn continue_from_title(app: &mut App) {
        tap(app, DsButton::Start);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        choose(app, DsButton::A);
    }

    #[test]
    fn suspend_then_continue_resumes_the_run_and_matches_one_that_never_stopped() {
        let path = scratch("resume");
        let mut app = saving_app(&path);
        let mut control = App::new(&ready(), None, RUN, true);
        for b in MOVES {
            step(&mut app, b);
            step(&mut control, b);
        }
        let before = app.game.clone();
        assert!(before.is_some());
        suspend_from_menu(&mut app);
        assert_eq!(app.scene, Scene::Title);
        assert!(app.game.is_none(), "suspend drops the run");
        assert!(path.is_file());
        assert_eq!(app.menu_items(), 6, "the save menu is offered");

        continue_from_title(&mut app);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.dungeon().is_some());
        assert_eq!(app.game, before, "the run is as it was");
        assert_eq!(app.game, control.game);
        for b in MOVES.into_iter().rev() {
            step(&mut app, b);
            step(&mut control, b);
            assert_eq!(app.game, control.game, "the next steps match");
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));
    }

    #[test]
    fn a_fresh_boot_has_no_continue_and_a_new_app_sees_an_existing_save() {
        let path = scratch("exists");
        let mut app = saving_app(&path);
        assert_eq!(app.menu_items(), 2);
        suspend_from_menu(&mut app);
        let again = App::new(&ready(), None, RUN, false).with_save_path(Some(path.clone()));
        assert_eq!(again.menu_items(), 6);
        let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));
    }

    /// An app on the save menu: a run was suspended, then the title was passed.
    fn save_menu(name: &str) -> (App, PathBuf) {
        let path = scratch(name);
        let mut app = saving_app(&path);
        suspend_from_menu(&mut app);
        assert_eq!(app.scene, Scene::Title);
        tap(&mut app, DsButton::Start);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        fade(&mut app);
        (app, path)
    }

    #[test]
    fn delete_save_data_confirms_then_shows_the_fresh_boot_menu() {
        let (mut app, path) = save_menu("delete-yes");
        tap(&mut app, DsButton::Down);
        choose(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::ConfirmDelete { yes: false });
        assert!(path.is_file(), "nothing is deleted before the answer");
        tap(&mut app, DsButton::Down);
        assert_eq!(app.scene, Scene::ConfirmDelete { yes: true });
        assert!(path.is_file());
        tap(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Menu { cursor: 0 });
        assert!(!path.exists(), "the save is gone");
        assert_eq!(app.menu_items(), 2, "the fresh-boot menu");
        fade(&mut app);
        choose(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::Dungeon, "row 0 is New Game again");
        let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));
    }

    #[test]
    fn declining_the_deletion_keeps_the_save_and_the_menu() {
        for answer in [DsButton::A, DsButton::B] {
            let (mut app, path) = save_menu("delete-no");
            tap(&mut app, DsButton::Down);
            choose(&mut app, DsButton::A);
            assert_eq!(app.scene, Scene::ConfirmDelete { yes: false });
            tap(&mut app, answer);
            assert_eq!(app.scene, Scene::Menu { cursor: 1 });
            assert!(path.is_file());
            assert_eq!(app.menu_items(), 6);
            let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));
        }
    }

    #[test]
    fn the_ds_entries_say_they_are_not_available_and_return() {
        let (mut app, path) = save_menu("ds-entries");
        for row in 3..6 {
            tap(&mut app, DsButton::Down);
            if row == 3 {
                tap(&mut app, DsButton::Down);
                tap(&mut app, DsButton::Down);
            }
            assert_eq!(app.scene, Scene::Menu { cursor: row });
            choose(&mut app, DsButton::A);
            assert_eq!(app.scene, Scene::Unavailable { cursor: row });
            fade(&mut app);
            tap(&mut app, DsButton::B);
            assert_eq!(app.scene, Scene::Menu { cursor: row });
            fade(&mut app);
        }
        assert!(path.is_file(), "nothing was changed");
        let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));
    }

    #[test]
    fn the_adventure_log_reads_the_save_or_says_there_is_none() {
        let (mut app, path) = save_menu("log-save");
        tap(&mut app, DsButton::Down);
        tap(&mut app, DsButton::Down);
        choose(&mut app, DsButton::A);
        assert_eq!(app.scene, Scene::AdventureLog);
        let lines = app.adventure_log.clone().expect("the log is read");
        assert!(lines.iter().any(|l| l.starts_with("Floors entered: ")));
        tap(&mut app, DsButton::B);
        assert_eq!(app.scene, Scene::Menu { cursor: 2 });
        assert!(app.adventure_log.is_none());
        let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));

        let mut fresh = App::new(&ready(), None, RUN, false);
        fresh.scene = Scene::Title;
        tap(&mut fresh, DsButton::Start);
        fade(&mut fresh);
        tap(&mut fresh, DsButton::Down);
        choose(&mut fresh, DsButton::A);
        assert_eq!(fresh.scene, Scene::AdventureLog);
        let lines = fresh.adventure_log.clone().expect("the log");
        assert!(lines.iter().any(|l| l.contains("No adventure")));
    }

    #[test]
    fn a_corrupted_save_is_reported_and_not_loaded() {
        let path = scratch("corrupt");
        let mut app = saving_app(&path);
        suspend_from_menu(&mut app);
        let good = std::fs::read(&path).expect("the save");
        let mut flipped = good.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x40;
        let bad_files = [
            flipped,
            good[..good.len() / 2].to_vec(),
            Vec::new(),
            b"RSAV".to_vec(),
        ];
        for bad in bad_files {
            std::fs::write(&path, bad).expect("write");
            let mut app = saving_app(&path);
            app.scene = Scene::Title;
            continue_from_title(&mut app);
            assert_eq!(app.scene, Scene::Menu { cursor: 0 }, "the menu stays up");
            assert!(app.game.is_none(), "nothing is loaded");
            assert!(app.notice().is_some(), "the refusal is reported");
        }
        // A missing file is refused too, with no crash.
        std::fs::remove_file(&path).expect("remove");
        let mut app = saving_app(&path);
        app.has_save = true;
        app.scene = Scene::Title;
        continue_from_title(&mut app);
        assert!(app.game.is_none() && app.notice().is_some());
        let _ = std::fs::remove_dir_all(path.parent().expect("a dir"));
    }

    #[test]
    fn suspend_with_nowhere_to_save_keeps_the_run() {
        let mut app = App::new(&ready(), None, RUN, true);
        suspend_from_menu(&mut app);
        assert_eq!(app.scene, Scene::Dungeon);
        assert!(app.game.is_some());
        assert!(app.notice().is_some());
    }
}
