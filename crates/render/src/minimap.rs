//! The dungeon minimap (BG0 cols 1 to 28, rows 1 to 16, tiles `0x0B8` to `0x277`): one 4x4
//! pixel cell per floor cell, each drawn with a pattern from `dungeon.sbin` `zmappat`.
//!
//! The cell rule is `FUN_0213cd5c` (ov_0001, rescue-arm9-a decompile; findings for #313, P1),
//! transcribed in the order the game runs it. The inputs are the cell word (terrain and
//! flags), its monster and object entries, the layout byte and the dungeon switches; the
//! switches and predicates are passed in [`Settings`], so the render crate reads game state
//! and never writes it. Addresses in the comments are ov_0001 unless stated.
//!
//! Known gaps, not in the rule's inputs: the monster info fields and trap ids are not in the
//! capture's cell list, so [`Monster`] and [`Object`] are supplied by the caller. The pattern
//! table is read as 768 contiguous 64-byte patterns from file offset 0x10: the pointer table at
//! 0xC010 is not used, and the ROM-gated test checks that layout against the captured canvas.
//!
//! Untested cases (no capture has them): terrain 2 and 3, flags 1 and 2 alone during play
//! (the unit tests use synthetic cells), trap ids other than the observed 17 and the default
//! pattern 4, and the layout options 4 and 6 with a non-zero object.

use rescue_formats::read::{Error, Result};

/// Floor cells: 56 across, 32 down (`x < 0x38`, `y < 0x20`).
pub const COLS: usize = 56;
pub const ROWS: usize = 32;
/// Canvas tiles: 28 across, 16 down. One tile holds 2 by 2 cells.
pub const CANVAS_COLS: usize = 28;
pub const CANVAS_ROWS: usize = 16;
/// Canvas bytes: 4bpp tiles of 32 bytes, row-major (row stride `0x380` = 28 tiles).
pub const CANVAS_BYTES: usize = CANVAS_COLS * CANVAS_ROWS * 32;
/// Patterns: 192 cell patterns of 4 quadrants, 768 in all, 64 bytes each.
pub const PATTERNS: usize = 768;
const PATTERN_BYTES: usize = 64;
const PATTERN_BASE: usize = 0x10;

/// Monster classes the rule draws (`FUN_0213cd5c`, the monster branch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonsterMark {
    /// Pattern 0x0A (yellow diamond): an ally, or a special monster.
    Yellow,
    /// Pattern 0x02 (red diamond): a hostile monster.
    Red,
    /// The leader: pattern 0x08 (white) when `leader_white` is set, else terrain.
    Leader,
}

/// The monster-info bytes the rule reads (`info = *(entity + 0x70)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterInfo {
    /// `info + 0x40`: `'J'` or `'G'` make a monster yellow.
    pub species: u8,
    /// `info + 0x98`: 1 or 4 makes a monster yellow.
    pub status: u8,
    /// `info + 6`: non-zero is hostile.
    pub hostile: u8,
    /// `info + 7`: non-zero is the leader (when `hostile` is 0).
    pub leader: u8,
}

/// A monster entry of a cell (`W + 0x0C`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monster {
    /// The entity type (`FUN_02142adc`); 1 is a monster.
    pub kind: u8,
    /// `FUN_02141e58`: the monster is visible on the map.
    pub visible: bool,
    pub info: MonsterInfo,
}

/// An object entry of a cell (`W + 0x10`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Object {
    /// The entity type: 1 monster, 2 trap, 3 item.
    pub kind: u8,
    /// The trap id (`FUN_02142acc`), read for traps only.
    pub trap_id: u8,
    /// `object + 0x20` is non-zero.
    pub flag20: bool,
}

/// One floor cell: its word and its entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// `W[0]`: terrain. Bits 0 and 1 give the terrain class (1 is floor); bit 9 is stairs.
    pub terrain: u16,
    /// `W[1]`: flags. Bit 0 is "known", bit 1 is the seen variant.
    pub flags: u16,
    pub monster: Option<Monster>,
    pub object: Option<Object>,
}

/// The layout byte and the switches the rule reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `*(*0x020DED14 + 4)`, the layout option `t` (0 to 6 in use).
    pub layout: u8,
    /// `FUN_0206db90() != 0`: the dungeon's special state (`D[0x668]`, `D[0x63d]`, `D[0x641]`,
    /// `D+0x34b4`; their meanings are not named).
    pub dungeon_special: bool,
    /// `*0x021B4210 != 0`: monsters are shown.
    pub show_monsters: bool,
    /// `D[0x15FAC]`: the leader is drawn white.
    pub leader_white: bool,
    /// `D[0x15FA2]`: no entity branch at all.
    pub no_entities: bool,
    /// `D[0x15FA3]`: every cell is known (when `no_entities` is clear).
    pub known_all: bool,
    /// `D[0x15FA6]`: items show without the seen flag.
    pub items_visible: bool,
    /// `D[0x15FA7]`: traps show without `object + 0x20`.
    pub traps_visible: bool,
    /// `D[0x15FA8]`: any drawn entity or stairs cell becomes green (0x0B).
    pub marked: bool,
    /// `D[0x15FA9]`: stairs show in unknown cells (when `dungeon_special` is clear).
    pub stairs_unknown: bool,
    /// `D[0x11308] != 0`: a scripted event blanks the whole minimap.
    pub blank: bool,
    /// The draw gate: the pattern table is loaded, `*0x021B4214 != 0`, and `FUN_02023bf4()`
    /// returns 0. Not traced further; the caller supplies it.
    pub enabled: bool,
}

/// The minimap canvas and the leader position the rule stored (`0x021B3EB8`, for the OBJ dot).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Minimap {
    /// 4bpp tile bytes in game order (28 tiles per row, 32 bytes per tile).
    pub canvas: Vec<u8>,
    /// Cell position of the leader, when the leader branch ran.
    pub leader: Option<(i32, i32)>,
}

/// The 768 patterns of `zmappat`: each is 8 pairs of u32 `(AND, OR)`, one pair per 8-pixel tile
/// row, applied as `row = (row & AND) | OR`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patterns {
    words: Vec<[u32; 16]>,
}

impl Patterns {
    /// Decodes `zmappat` (the sub-file bytes): 768 patterns of 64 bytes from offset 0x10.
    pub fn decode(zmappat: &[u8]) -> Result<Self> {
        let end = PATTERN_BASE + PATTERNS * PATTERN_BYTES;
        let body = zmappat
            .get(PATTERN_BASE..end)
            .ok_or_else(|| Error::invalid("zmappat", "shorter than 768 patterns"))?;
        let words = body
            .chunks_exact(PATTERN_BYTES)
            .map(|p| {
                let mut w = [0u32; 16];
                for (i, c) in p.chunks_exact(4).enumerate() {
                    w[i] = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                }
                w
            })
            .collect();
        Ok(Self { words })
    }

    /// Builds patterns from already-decoded words (tests, or another decoder).
    pub fn from_words(words: Vec<[u32; 16]>) -> Self {
        Self { words }
    }

    fn get(&self, index: usize) -> Option<&[u32; 16]> {
        self.words.get(index)
    }
}

/// Pattern value of trap `id`: `byte[0x021ABA30 + id]` (`FUN_0213cd5c`). Observed as 4 for every
/// id except 17, which gives 7 (dng1 runtime). Other ids are `[hypothesis]`: 4.
pub fn trap_u(id: u8) -> u16 {
    if id == 17 { 7 } else { 4 }
}

/// Classifies a monster by its info bytes (`FUN_0213cd5c`, the monster branch): yellow for
/// `'J'`, `'G'`, status 1 or 4, and for an ally; red for a hostile; the leader otherwise.
pub fn classify(info: MonsterInfo) -> MonsterMark {
    if info.species == b'J' || info.species == b'G' || info.status == 1 || info.status == 4 {
        MonsterMark::Yellow
    } else if info.hostile == 0 {
        if info.leader == 0 {
            MonsterMark::Yellow
        } else {
            MonsterMark::Leader
        }
    } else {
        MonsterMark::Red
    }
}

/// The cell at (`x`, `y`) of the floor grid, row-major.
fn cell_at(grid: &[Cell], x: i32, y: i32) -> Option<&Cell> {
    if (0..COLS as i32).contains(&x) && (0..ROWS as i32).contains(&y) {
        grid.get(y as usize * COLS + x as usize)
    } else {
        None
    }
}

/// The pattern value `u` of cell (`x`, `y`) and the leader position it stored, or `None` when
/// nothing is drawn there. Transcribed from `FUN_0213cd5c`; the step numbers are the order in
/// the decompile.
pub fn cell_u(grid: &[Cell], x: i32, y: i32, s: &Settings) -> Option<(u16, Option<(i32, i32)>)> {
    let local = i32::from(s.layout == 6);
    // Bounds: x in 0..0x37, y in 2..0x1F, or y in 1..0x1F for layout 6.
    if !(0..COLS as i32).contains(&x) || !(0..ROWS as i32).contains(&y) || y < 2 - local {
        return None;
    }
    let cell = cell_at(grid, x, y)?;
    let t = s.layout;
    let terrain = cell.terrain;
    let flags = cell.flags;

    // Step 1: known, and the stairs switch.
    let known = if !s.no_entities && s.known_all {
        1
    } else {
        flags & 1
    };
    let show_stairs = !s.no_entities && !s.dungeon_special && s.stairs_unknown;

    // Step 2: blank for layouts 0 and 3 (`FUN_0200c60c` is 0) and while `D[0x11308]` is set.
    // The rest, including the layout offsets, is skipped.
    let c60c = t != 0 && t != 3;
    if !c60c || s.blank {
        return Some((1, None));
    }

    // Step 3: the entity branches, until one decides the cell.
    let mut u: u16 = 0;
    let mut done = false;
    let mut leader = None;
    let monster = if s.no_entities {
        None
    } else {
        cell.monster
            .filter(|m| m.kind == 1 && s.show_monsters && m.visible)
    };
    if let Some(m) = monster {
        done = true;
        match classify(m.info) {
            MonsterMark::Yellow => u = 0x0A,
            MonsterMark::Red => u = 0x02,
            MonsterMark::Leader => {
                leader = Some((x, y));
                if s.leader_white {
                    u = 0x08;
                }
            }
        }
    }
    if !done && !s.no_entities {
        match cell.object {
            None => {
                let stairs = terrain & 0x200 != 0;
                if (known == 0 && show_stairs && stairs) || (known != 0 && stairs) {
                    u = if s.dungeon_special { 5 } else { 6 };
                    done = true;
                }
            }
            Some(o) => {
                if known == 0 {
                    u = if o.kind == 3 && (s.items_visible || flags & 2 != 0) && terrain & 3 != 0 {
                        3
                    } else {
                        0
                    };
                    done = true;
                } else {
                    if o.kind == 2 && (o.flag20 || s.traps_visible) {
                        u = trap_u(o.trap_id);
                        done = true;
                    }
                    if !done
                        && o.kind == 3
                        && (s.items_visible || flags & 2 != 0)
                        && terrain & 3 != 0
                    {
                        u = 3;
                        done = true;
                    }
                    if !done && terrain & 3 != 1 {
                        u = 0;
                        done = true;
                    }
                }
            }
        }
    }

    // Step 4: the reveal tint (`D[0x15FA8]`): an entity or stairs cell becomes green.
    if done && s.marked && u != 0 {
        u = 0x0B;
    }

    // Step 5: terrain, when the entity branches drew nothing.
    if u == 0 {
        if known == 0 {
            u = 1;
        } else if terrain & 3 == 1 {
            // Walls: clear the bits of the sides that touch floor. The bottom neighbour sets
            // the mask to 0xE, as the decompile does.
            let floor =
                |xx: i32, yy: i32| cell_at(grid, xx, yy).is_some_and(|c| c.terrain & 3 == 1);
            let mut m: u16 = 0xF;
            if y < 0x1F && floor(x, y + 1) {
                m = 0xE;
            }
            if y > 0 && floor(x, y - 1) {
                m &= !4;
            }
            if x < 0x37 && floor(x + 1, y) {
                m &= !2;
            }
            if x > 0 && floor(x - 1, y) {
                m &= !8;
            }
            u = m + if flags & 2 == 0 { 0x20 } else { 0x10 };
        } else {
            u = 1;
        }
    }

    // Step 6: layout offsets (only on the non-blank path): 0x80 for layout 6, else 0x40 when
    // `FUN_0200c5dc` is 1 (layout not 2 or 5).
    if t == 6 {
        u += 0x80;
    } else if t != 2 && t != 5 {
        u += 0x40;
    }
    Some((u, leader))
}

/// Renders the minimap: each cell's pattern goes into its quadrant of its canvas tile. Cells
/// are read from `grid` (56 by 32, row-major). A disabled gate, or a grid of the wrong size,
/// gives an empty canvas.
pub fn render(patterns: &Patterns, grid: &[Cell], s: &Settings) -> Minimap {
    let mut out = Minimap {
        canvas: vec![0u8; CANVAS_BYTES],
        leader: None,
    };
    if !s.enabled || grid.len() != COLS * ROWS {
        return out;
    }
    let local = i32::from(s.layout == 6);
    for y in 0..ROWS as i32 {
        for x in 0..COLS as i32 {
            let Some((u, leader)) = cell_u(grid, x, y, s) else {
                continue;
            };
            if leader.is_some() {
                out.leader = leader;
            }
            let ry = y - 2 + local;
            let (tile_col, tile_row) = ((x / 2) as usize, (ry / 2) as usize);
            let q = (x & 1) + 2 * (ry & 1);
            let Some(pattern) = patterns.get(usize::from(u) * 4 + q as usize) else {
                continue;
            };
            let base = (tile_row * CANVAS_COLS + tile_col) * 32;
            for r in 0..8 {
                let at = base + r * 4;
                let Some(bytes) = out.canvas.get_mut(at..at + 4) else {
                    continue;
                };
                let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                let new = pattern[2 * r + 1] | (word & pattern[2 * r]);
                bytes.copy_from_slice(&new.to_le_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_INFO: MonsterInfo = MonsterInfo {
        species: 0,
        status: 0,
        hostile: 0,
        leader: 0,
    };

    fn settings() -> Settings {
        Settings {
            layout: 1,
            dungeon_special: false,
            show_monsters: true,
            leader_white: false,
            no_entities: false,
            known_all: false,
            items_visible: false,
            traps_visible: false,
            marked: false,
            stairs_unknown: false,
            blank: false,
            enabled: true,
        }
    }

    /// A grid of unknown blank cells, with the given cells placed at (x, y).
    fn grid(placed: &[((usize, usize), Cell)]) -> Vec<Cell> {
        let blank = Cell {
            terrain: 0,
            flags: 0,
            monster: None,
            object: None,
        };
        let mut g = vec![blank; COLS * ROWS];
        for &((x, y), c) in placed {
            g[y * COLS + x] = c;
        }
        g
    }

    fn cell(terrain: u16, flags: u16) -> Cell {
        Cell {
            terrain,
            flags,
            monster: None,
            object: None,
        }
    }

    fn u_at(g: &[Cell], x: i32, y: i32, s: &Settings) -> Option<u16> {
        cell_u(g, x, y, s).map(|(u, _)| u)
    }

    #[test]
    fn unknown_cell_is_transparent_with_the_layout_offset() {
        // Not known: terrain blank (u = 1), then +0x40 for layout 1 (0x41, not the bare 1).
        let g = grid(&[]);
        assert_eq!(u_at(&g, 10, 10, &settings()), Some(0x41));
        // Layout 2: no +0x40.
        let s = Settings {
            layout: 2,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s), Some(1));
    }

    #[test]
    fn known_floor_families_follow_flag_bit_1() {
        // Seen (flags 3) on layout 1: 0x10 + m with m = 0xF, then +0x40: 0x5F (the 0x50 family).
        let g = grid(&[((10, 10), cell(1, 3))]);
        assert_eq!(u_at(&g, 10, 10, &settings()), Some(0x5F));
        // Known only (flags 1): 0x20 + m, then +0x40: 0x6F (the 0x60 family).
        let g = grid(&[((10, 10), cell(1, 1))]);
        assert_eq!(u_at(&g, 10, 10, &settings()), Some(0x6F));
        // Layout 2 has no +0x40: the unshifted 0x20 family.
        let s = Settings {
            layout: 2,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s), Some(0x2F));
    }

    #[test]
    fn walls_clear_the_sides_that_touch_floor() {
        // Floor at (10, 10) with floor to its right (x+1): bit 1 clears, m = 0xD.
        let g = grid(&[((10, 10), cell(1, 3)), ((11, 10), cell(1, 3))]);
        assert_eq!(u_at(&g, 10, 10, &settings()), Some(0x5D));
        // Floor below (y+1) sets m to 0xE, clearing bit 0 (as the decompile does).
        let g = grid(&[((10, 10), cell(1, 3)), ((10, 11), cell(1, 3))]);
        assert_eq!(u_at(&g, 10, 10, &settings()), Some(0x5E));
    }

    #[test]
    fn layout_offsets_and_blank_layouts() {
        let g = grid(&[((10, 10), cell(1, 3))]);
        // Layout 6: +0x80 and rows start at y = 1, so y = 1 is drawn.
        let s6 = Settings {
            layout: 6,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s6), Some(0x9F));
        // An unknown cell on row 1 is drawn in layout 6: 1 + 0x80.
        assert_eq!(u_at(&g, 10, 1, &s6), Some(0x81));
        // Layouts 0 and 3 blank everything to pattern 1, with no offset.
        for t in [0u8, 3] {
            let s = Settings {
                layout: t,
                ..settings()
            };
            assert_eq!(u_at(&g, 10, 10, &s), Some(1), "layout {t}");
        }
        // Rows below y = 2 are not drawn for layouts other than 6.
        assert_eq!(u_at(&g, 10, 1, &settings()), None);
    }

    #[test]
    fn out_of_grid_cells_are_not_drawn() {
        let g = grid(&[]);
        assert_eq!(u_at(&g, 56, 10, &settings()), None);
        assert_eq!(u_at(&g, 10, 32, &settings()), None);
    }

    #[test]
    fn scripted_blank_overrides_everything() {
        let g = grid(&[((10, 10), cell(1, 3))]);
        let s = Settings {
            blank: true,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s), Some(1));
    }

    #[test]
    fn monsters_are_classified_and_leader_depends_on_the_switch() {
        let monster = |info: MonsterInfo| Monster {
            kind: 1,
            visible: true,
            info,
        };
        let with = |m: Monster| {
            let mut c = cell(1, 3);
            c.monster = Some(m);
            grid(&[((10, 10), c)])
        };
        // Hostile: red 0x02, then +0x40.
        let hostile = MonsterInfo {
            hostile: 1,
            ..EMPTY_INFO
        };
        assert_eq!(
            u_at(&with(monster(hostile)), 10, 10, &settings()),
            Some(0x42)
        );
        // Ally (not leader): yellow 0x0A.
        assert_eq!(
            u_at(&with(monster(EMPTY_INFO)), 10, 10, &settings()),
            Some(0x4A)
        );
        // Special species makes it yellow even when hostile.
        let g_species = MonsterInfo {
            species: b'G',
            hostile: 1,
            ..EMPTY_INFO
        };
        assert_eq!(classify(g_species), MonsterMark::Yellow);
        // Leader: terrain unless the white switch is set.
        let leader = MonsterInfo {
            leader: 1,
            ..EMPTY_INFO
        };
        assert_eq!(classify(leader), MonsterMark::Leader);
        let g = with(monster(leader));
        assert_eq!(
            cell_u(&g, 10, 10, &settings()),
            Some((0x5F, Some((10, 10))))
        );
        let white = Settings {
            leader_white: true,
            ..settings()
        };
        assert_eq!(cell_u(&g, 10, 10, &white), Some((0x48, Some((10, 10)))));
        // A monster that is not visible, or not type 1, draws nothing special.
        let hidden = Monster {
            visible: false,
            ..monster(hostile)
        };
        assert_eq!(u_at(&with(hidden), 10, 10, &settings()), Some(0x5F));
    }

    #[test]
    fn objects_items_traps_and_stairs() {
        let object = |kind, trap_id, flag20| Object {
            kind,
            trap_id,
            flag20,
        };
        let at = |c: Cell| grid(&[((10, 10), c)]);
        // Item on a seen cell (flags & 2): pattern 3, then +0x40 = 0x43.
        let mut c = cell(1, 3);
        c.object = Some(object(3, 0, false));
        assert_eq!(u_at(&at(c), 10, 10, &settings()), Some(0x43));
        // Item on a known cell without the seen flag, and no item switch: terrain.
        let mut c = cell(1, 1);
        c.object = Some(object(3, 0, false));
        assert_eq!(u_at(&at(c), 10, 10, &settings()), Some(0x6F));
        // The item switch shows it.
        let s = Settings {
            items_visible: true,
            ..settings()
        };
        assert_eq!(u_at(&at(c), 10, 10, &s), Some(0x43));
        // Trap with the flag at +0x20: trap id 17 gives 7; another id gives 4.
        let mut c = cell(1, 3);
        c.object = Some(object(2, 17, true));
        assert_eq!(u_at(&at(c), 10, 10, &settings()), Some(0x47));
        let mut c = cell(1, 3);
        c.object = Some(object(2, 5, true));
        assert_eq!(u_at(&at(c), 10, 10, &settings()), Some(0x44));
        // Stairs (terrain 0x200) on a known cell: 6, or 5 with the special state.
        let c = cell(0x201, 3);
        assert_eq!(u_at(&at(c), 10, 10, &settings()), Some(0x46));
        let s = Settings {
            dungeon_special: true,
            ..settings()
        };
        assert_eq!(u_at(&at(c), 10, 10, &s), Some(0x45));
        // Stairs in an unknown cell need the switch.
        let c = cell(0x201, 0);
        assert_eq!(u_at(&at(c), 10, 10, &settings()), Some(0x41));
        let s = Settings {
            stairs_unknown: true,
            ..settings()
        };
        assert_eq!(u_at(&at(c), 10, 10, &s), Some(0x46));
    }

    #[test]
    fn switches_suppress_entities_and_mark_them() {
        let mut c = cell(1, 3);
        c.object = Some(Object {
            kind: 3,
            trap_id: 0,
            flag20: false,
        });
        let g = grid(&[((10, 10), c)]);
        // No-entities switch: the item is not drawn; terrain is.
        let s = Settings {
            no_entities: true,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s), Some(0x5F));
        // Known-all switch: an unknown cell becomes known floor.
        let g = grid(&[((10, 10), cell(1, 0))]);
        let s = Settings {
            known_all: true,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s), Some(0x6F));
        // Marked switch: a drawn item becomes green (0x0B, then +0x40).
        let mut c = cell(1, 3);
        c.object = Some(Object {
            kind: 3,
            trap_id: 0,
            flag20: false,
        });
        let g = grid(&[((10, 10), c)]);
        let s = Settings {
            marked: true,
            ..settings()
        };
        assert_eq!(u_at(&g, 10, 10, &s), Some(0x4B));
    }

    #[test]
    fn patterns_decode_and_reject_short_input() {
        let short = vec![0u8; PATTERN_BASE + PATTERNS * PATTERN_BYTES - 1];
        assert!(Patterns::decode(&short).is_err());
        let mut full = vec![0u8; PATTERN_BASE + PATTERNS * PATTERN_BYTES];
        // Pattern 1, word 3 (the OR of row 1): 0x12345678.
        let at = PATTERN_BASE + PATTERN_BYTES + 12;
        full[at..at + 4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        let p = Patterns::decode(&full).unwrap();
        assert_eq!(p.get(1).unwrap()[3], 0x1234_5678);
    }

    #[test]
    fn a_cell_writes_only_its_quadrant_of_its_tile() {
        // Pattern u = 1, quadrant 1 (index 5): a synthetic pattern that keeps the left four
        // pixels and writes 5 to the right four, on all eight rows (real patterns use their
        // masks for one quadrant). Cell (1, 2) is tile (0, 0), quadrant 1. The other cells use
        // the empty pattern (AND all ones).
        // The empty pattern keeps every pixel: AND all ones, OR zero.
        let empty = {
            let mut e = [0u32; 16];
            for r in 0..8 {
                e[2 * r] = 0xFFFF_FFFF;
            }
            e
        };
        let mut words = vec![empty; PATTERNS];
        let mut w = [0u32; 16];
        for r in 0..8 {
            w[2 * r] = 0x0000_FFFF;
            w[2 * r + 1] = 0x5555_0000;
        }
        words[5] = w;
        let patterns = Patterns::from_words(words);
        // Cell (1, 2) is unknown with terrain 0 (u = 1); layout 2 adds no offset.
        let g = grid(&[]);
        let s = Settings {
            layout: 2,
            ..settings()
        };
        let m = render(&patterns, &g, &s);
        let word0 = u32::from_le_bytes([m.canvas[0], m.canvas[1], m.canvas[2], m.canvas[3]]);
        assert_eq!(word0, 0x5555_0000, "right half of tile row 0 is written");
        // The synthetic pattern covers row 3 too.
        let row3 = u32::from_le_bytes([m.canvas[12], m.canvas[13], m.canvas[14], m.canvas[15]]);
        assert_eq!(row3, 0x5555_0000);
        // Quadrant 1 of tile (1, 0) (cell (3, 2)) gets the same write; the left half of row 0
        // of tile (1, 0) is not touched.
        let tile10 = u32::from_le_bytes([m.canvas[32], m.canvas[33], m.canvas[34], m.canvas[35]]);
        assert_eq!(tile10, 0x5555_0000);
        assert_eq!(m.leader, None);
    }

    #[test]
    fn disabled_gate_draws_nothing() {
        let patterns = Patterns::from_words(vec![[0xFFFF_FFFF; 16]; PATTERNS]);
        let g = grid(&[((10, 10), cell(1, 3))]);
        let s = Settings {
            enabled: false,
            ..settings()
        };
        let m = render(&patterns, &g, &s);
        assert!(m.canvas.iter().all(|&b| b == 0));
    }
}
