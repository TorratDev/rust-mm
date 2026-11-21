//! Input commands: the only way the outside world changes the simulation.
//!
//! The directions are the game's eight, in the order of its direction table (`Down` is 0,
//! then clockwise: `DownRight`, `Right`, `UpRight`, `Up`, `UpLeft`, `Left`, `DownLeft`;
//! `docs/re/io.md`, "Dungeon directions"). Map `y` grows downward, as the floor grid does.
//!
//! A front end turns buttons, touch and menu gestures into these commands. Nothing else
//! reads raw input.

use serde::{Deserialize, Serialize};

use crate::items::SwapSource;

/// One of the eight directions of movement and attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Down,
    DownRight,
    Right,
    UpRight,
    Up,
    UpLeft,
    Left,
    DownLeft,
}

impl Direction {
    /// Step in map tiles, `(dx, dy)` with `y` growing downward.
    #[must_use]
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::Down => (0, 1),
            Direction::DownRight => (1, 1),
            Direction::Right => (1, 0),
            Direction::UpRight => (1, -1),
            Direction::Up => (0, -1),
            Direction::UpLeft => (-1, -1),
            Direction::Left => (-1, 0),
            Direction::DownLeft => (-1, 1),
        }
    }
}

/// A menu gesture. Menus are not modelled yet, so these only exist as input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
}

/// One input command for the dungeon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Move the leader one tile.
    Move(Direction),
    /// Run in a direction: the first step is a move, then the leader keeps going while the run
    /// checks hold (`docs/re/turns.md`, "Running and swapping"). With an empty belly it is one
    /// step.
    Run(Direction),
    /// Regular Attack in a direction (action `0x32`, move `0x163`): it takes no PP. Its targets
    /// are the monsters its range picks. Only the modelled areas run, through
    /// [`crate::state::GameState::step_with`].
    Attack(Direction),
    /// Use move slot `slot` (0 to 3), at a direction for moves that need one. Only the
    /// damaging moves of [`crate::combat`] are modelled, and only through
    /// [`crate::state::GameState::step_with`].
    UseMove {
        slot: u8,
        direction: Option<Direction>,
    },
    /// Use inventory item `slot` (actions `0x0C`/`0x0D`; an orb is action `0x31`, which needs
    /// [`crate::state::GameState::step_with`]). See [`crate::item_use`].
    UseItem { slot: u8 },
    /// Drop bag slot `slot` on the leader's tile (action 8). The same as
    /// [`crate::state::GameState::drop_item`].
    Drop { slot: u8 },
    /// Swap the floor item underfoot with a bag slot or the held item (action `0x0A`). The same
    /// as [`crate::state::GameState::swap_floor_item`].
    Swap(SwapSource),
    /// Pass a turn in place.
    Wait,
    /// A menu gesture. Menus are not modelled yet.
    Menu(MenuAction),
}

/// Why a command did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blocked {
    /// No dungeon is loaded.
    NoDungeon,
    /// The team is empty, so there is no leader to move.
    NoLeader,
    /// The target tile is off the floor.
    OutOfBounds,
    /// The target tile is an impassable wall (`0x10`).
    Wall,
    /// A wild monster stands on the target tile. A team ally there is swapped with instead.
    Occupied,
    /// The target is not ordinary ground (water, lava or a plain wall: `terrain & 3` is not 1).
    Terrain,
    /// A diagonal step would cut a wall corner: one of its orthogonal neighbours is a wall.
    CornerCut,
    /// The move slot is empty, or the slot is not one of the four.
    NoMove,
    /// The move has no PP left, and another move still has PP, so the game does not use it
    /// (`docs/re/combat.md`, P3). When every move has no PP the use is Struggle instead.
    NoPp,
    /// The bag slot is empty or not one of the 20; or a swap has nothing underfoot or an empty
    /// source.
    NoItem,
    /// The item is sticky (a drop or swap of a sticky marked item, or a swap of a sticky held
    /// item).
    ItemSticky,
    /// The leader's tile cannot take a dropped item: the stairs, not plain floor, or it already
    /// holds an item.
    DropTile,
    /// All 64 floor item slots are used.
    FloorItemsFull,
    /// The buy needs the ground menu of a shop stock tile, and the leader's tile holds stock.
    NoStock,
    /// The bag has no room for the bought item (`docs/re/items.md`, P6).
    BagFull,
}

/// What a command did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// The command took effect; the turn advanced when the command takes a turn.
    Applied,
    /// The command was refused and the state did not change.
    Blocked(Blocked),
    /// The command is valid input, but its rule is not modelled yet. The state did not change.
    NotModelled,
    /// The leader took the stairs and the next floor is loaded (see [`crate::state`]).
    NextFloor,
    /// The leader took the stairs of the last floor: the dungeon is cleared and no dungeon is
    /// loaded any more.
    DungeonCleared,
    /// The leader's HP reached 0 (`docs/re/combat.md`, "Defeat in the sim (#422)"). The run ends:
    /// no dungeon is loaded any more, as with [`Outcome::DungeonCleared`].
    LeaderDefeated,
}
