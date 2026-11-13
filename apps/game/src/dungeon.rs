//! The dungeon scene's floor view (#70, #400, #334, #347). The sim owns the run: its floors,
//! the leader and the turns (`rescue_sim::state::GameState`, built by `app`). This module draws
//! one floor of it: the ROM's tiles in the tileset, the leader's sprite at a given pose
//! (`Dungeon::draw`), scrolled so the leader's drawn position is centred, and the minimap on the
//! lower screen. The first floor of a run is generated here (`first_floor`); the later floors
//! come from the sim. Without a ROM the floor is drawn as solid colours per terrain
//! (placeholder), with no sprite and no minimap.
//!
//! The game reads, from the dump: `system.sbin` (`itempara`, for the item count `mapparam`
//! needs; `monspara`, for the leader's palette and speed), `dungeon.sbin` (`mapparam`, `zmappat`
//! for the minimap, and tileset `b14*` through `rescue_formats`), and `monster.sbin` (`ax`, the
//! leader's sprite, and `palet`). Not yet: palette animation (the tileset's `canm` is not
//! applied) and the autotile variants' retail draws (#400 P3).

use rescue_data::dungeon::parse_mapparam;
use rescue_data::item::parse_items;
use rescue_data::species::parse_species;
use rescue_formats::archive::Archive;
use rescue_formats::graphics::{Depth, Palette, rgbx_palette, rgbx_palettes16, tile_sheet};
use rescue_formats::sprite::Sprite;
use rescue_formats::tileset::Tileset;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_render::SCREEN_WIDTH;
use rescue_render::anim::AnimPlayer;
use rescue_render::autotile::{GRID_H, GRID_W, autotile};
use rescue_render::bg::{BgLayer, Frame, compose, floor_layer, solid_cells_layer};
use rescue_render::camera::follow;
use rescue_render::minimap::{CANVAS_COLS, COLS, Cell, Patterns, ROWS, Settings, render};
use rescue_render::sprite::{
    PlacedSprite, SpriteImage, compose_with_sprites, render_sprite, sprite_origin,
};
use rescue_sim::command::Direction;
use rescue_sim::floor::{Floor, HEIGHT, WIDTH};
use rescue_sim::floor_gen::{
    FloorProperties, GenState, PortedSteps, generate_floor, properties_for,
};
use rescue_sim::rng::{DungeonRng, GlobalRng};
use rescue_sim::state::{FloorRecord, floor_records};

/// The floor's RNG state at `GenerateFloor`'s entry for a new game: the natural seed of the
/// sim's first-floor oracle rows (`ORACLE_FLOORS`, layout 1).
pub const SEED: u32 = 0xB280_4D01;

/// The options' defaults: dungeon 0, floor 1 is the first floor of a new game.
pub const DEFAULT_DUNGEON: u8 = 0;
pub const DEFAULT_FLOOR: u8 = 1;
/// shortcut: the leader's species is not modelled (no personality quiz), so every game draws
/// this one. Pikachu (25) is the species the ROM sprite test draws, so its sprite is known to
/// decode. Upgrade when the quiz picks the leader.
pub const DEFAULT_SPECIES: u16 = 25;

/// Byte of the floor record that names the tileset (`+0x02`).
const TILESET_BYTE: usize = 0x02;

/// Seed of the global RNG for the autotile variant draws. shortcut: the console seeds it from
/// its MAC address, and the retail variants depend on frame timing, so they are not
/// reproducible (#400 P3); zero bytes stand in for the seed.
const VARIANT_SEED: [u8; 6] = [0; 6];

/// The leader's pose groups, both verified in Blue (docs/formats/sprites.md): group 7 is the
/// idle pose, group 0 the walk. Direction 0 faces the viewer (south).
pub const IDLE_GROUP: usize = 7;
pub const WALK_GROUP: usize = 0;

/// `zmappat` offset of the 16 RGBX colours of BG palette 14, the minimap's palette
/// (docs/re/ui-screens.md, BG palette 14).
const MINIMAP_PALETTE_AT: usize = 0xCC10;

/// The layout option `t` the minimap rule reads. 1 is the value of the dng1 capture
/// (docs/re/ui-screens.md, Minimap); the options screen that sets it is not ported.
const LAYOUT: u8 = 1;

/// Top-left of the minimap canvas on the lower screen: BG0 column 1, row 1.
const MINIMAP_AT: (usize, usize) = (8, 8);

/// Palette of the placeholder graphics: index 0 is transparent (not drawn).
const PALETTE: [[u8; 4]; 4] = [
    [0, 0, 0, 0],
    [40, 40, 60, 255],    // wall
    [120, 90, 60, 255],   // secondary terrain
    [200, 190, 160, 255], // floor
];
const BACKDROP: [u8; 4] = [16, 16, 24, 255];

/// What the scene takes from the ROM for one floor, and for the run's dungeon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorData {
    props: FloorProperties,
    /// The dungeon's floor records, placeholder first (`floor_records`).
    records: Vec<FloorRecord>,
    tileset_no: u8,
    tileset: Tileset,
    patterns: Patterns,
    minimap_palette: Palette,
    /// The leader species' movement speed (`monster_md` `+0xC`), for the sim's speed stage.
    leader_speed: i32,
    /// The leader's sprite and its palette, drawn at any pose by `leader_image`.
    sprite: Sprite,
    species_palette: u8,
    palettes: Vec<Palette>,
}

impl FloorData {
    /// Reads floor `floor` of dungeon `dungeon` from the dump `rom`, with the leader drawn as
    /// species `species`: its properties through `mapparam`, its tileset number from the
    /// record's byte `+0x02`, that tileset, the dungeon's records, the minimap patterns and
    /// palette, the leader's movement speed and sprite. `Err` if the dump lacks a file, a record
    /// or a part, or a part does not parse.
    pub fn from_rom(rom: &[u8], dungeon: u8, floor: u8, species: u16) -> Result<Self, String> {
        let header = Header::parse(rom).map_err(|e| e.to_string())?;
        let fs = Filesystem::parse(rom, &header).map_err(|e| e.to_string())?;
        let file = |name: &str| -> Result<&[u8], String> {
            let id = fs
                .get(name)
                .ok_or_else(|| format!("{name} is not in the ROM"))?
                .id;
            fs.data(rom, id).map_err(|e| e.to_string())
        };

        let sys = file("system.sbin")?;
        let sys_arc = Archive::parse(sys).map_err(|e| e.to_string())?;
        let itempara = sys_arc.get(sys, "itempara").map_err(|e| e.to_string())?;
        let items = parse_items(itempara).map_err(|e| e.to_string())?;
        let monspara = sys_arc.get(sys, "monspara").map_err(|e| e.to_string())?;
        let species_table = parse_species(monspara).map_err(|e| e.to_string())?;
        let entry = species_table
            .get(usize::from(species))
            .ok_or_else(|| format!("species {species} is not in the ROM"))?;

        let dun = file("dungeon.sbin")?;
        let arc = Archive::parse(dun).map_err(|e| e.to_string())?;
        let mapparam = arc.get(dun, "mapparam").map_err(|e| e.to_string())?;
        let map = parse_mapparam(mapparam, items.len()).map_err(|e| e.to_string())?;
        let props = properties_for(&map, dungeon, floor).map_err(|e| e.to_string())?;
        let records = floor_records(&map, dungeon).ok_or("the dungeon has no floor list")?;
        let record = map
            .dungeons
            .get(usize::from(dungeon))
            .and_then(|floors| floors.get(usize::from(floor)))
            .and_then(|entry| map.floor_properties.get(usize::from(entry.properties)))
            .ok_or("the floor has no record")?;
        let tileset_no = record[TILESET_BYTE];
        let tileset = Tileset::load(&arc, dun, tileset_no).map_err(|e| e.to_string())?;
        let zmappat = arc.get(dun, "zmappat").map_err(|e| e.to_string())?;
        let patterns = Patterns::decode(zmappat).map_err(|e| e.to_string())?;
        let minimap_palette = rgbx_palette(
            zmappat
                .get(MINIMAP_PALETTE_AT..)
                .ok_or("zmappat has no minimap palette")?,
            16,
        )
        .map_err(|e| e.to_string())?;

        let mon = file("monster.sbin")?;
        let marc = Archive::parse(mon).map_err(|e| e.to_string())?;
        let palettes = rgbx_palettes16(marc.get(mon, "palet").map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let sprite = Sprite::parse(
            marc.get(mon, &format!("ax{species:03}"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if sprite.groups.get(IDLE_GROUP).is_none() {
            return Err(String::from("the leader sprite has no idle pose"));
        }

        Ok(FloorData {
            props,
            records,
            tileset_no,
            tileset,
            patterns,
            minimap_palette,
            leader_speed: entry.movement_speed,
            sprite,
            species_palette: entry.palette,
            palettes,
        })
    }

    /// The dungeon's floor records (see [`FloorData::from_rom`]).
    pub fn records(&self) -> &[FloorRecord] {
        &self.records
    }

    /// The leader species' movement speed.
    pub fn leader_speed(&self) -> i32 {
        self.leader_speed
    }

    /// An animation of the leader's `group` facing `dir`, looping. `None` if the sprite has no
    /// such pose.
    pub fn leader_anim(&self, group: usize, dir: Direction) -> Option<AnimPlayer> {
        AnimPlayer::new(&self.sprite, group, dir as usize, true)
    }

    /// The leader's sprite on the frame `anim` is on.
    pub fn leader_image(&self, anim: &AnimPlayer) -> Option<SpriteImage> {
        render_sprite(
            &self.sprite,
            anim.frame(),
            self.species_palette,
            &self.palettes,
        )
        .ok()
    }
}

/// The two screens of one floor. The top is redrawn by [`Dungeon::draw`]; the lower is fixed.
/// `layer` and `minimap` are `None` for the placeholder, whose 8-pixel cells do not scroll and
/// have no minimap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dungeon {
    /// The top screen: the floor with the leader's sprite, as last drawn.
    pub frame: Frame,
    /// The lower screen: the minimap.
    pub minimap: Option<Frame>,
    /// The ROM floor's layer; `draw` scrolls it.
    layer: Option<BgLayer>,
}

/// Properties of the placeholder floor, used when the ROM's are not loaded.
fn placeholder_props() -> FloorProperties {
    FloorProperties::default()
}

/// The leader's cell from `stairs_a`, the position the generator leaves at -1 when unset.
fn leader_cell((x, y): (i16, i16)) -> Option<(i16, i16)> {
    let (xu, yu) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
    (xu < WIDTH && yu < HEIGHT).then_some((x, y))
}

/// The first floor of a run, generated from `seed` with the ROM's properties (or the
/// placeholder's), and the leader's cell: the generator's stairs A. The sim takes the floor
/// from here (`GameState::in_dungeon`). `None` if generation fails or the cell is unset.
pub fn first_floor(seed: u32, data: Option<&FloorData>) -> Option<(Floor, (i16, i16))> {
    let props = data.map_or_else(placeholder_props, |d| d.props);
    let mut floor = Floor::new();
    let mut rng = DungeonRng::from_state(seed);
    let mut g = GenState::default();
    generate_floor(
        &props,
        false,
        &mut floor,
        &mut rng,
        &mut g,
        &mut PortedSteps::default(),
        false,
    )
    .ok()?;
    let leader = leader_cell(g.stairs_a)?;
    Some((floor, leader))
}

/// The minimap canvas of `floor` on a backdrop frame, with every cell revealed.
// shortcut: all cells are known (`known_all`), since visibility is not ported (#407); no
// monsters, items or traps are drawn, and the stairs bit is not set on the grid. Upgrade with
// #407 for the reveal and when the sim exposes the stairs in the grid for the bit.
fn minimap_frame(floor: &Floor, patterns: &Patterns, palette: &Palette) -> Option<Frame> {
    let mut grid = Vec::with_capacity(COLS * ROWS);
    for y in 0..ROWS {
        for x in 0..COLS {
            let terrain = floor.tile(x as i32, y as i32)?.terrain;
            grid.push(Cell {
                terrain,
                flags: 1,
                monster: None,
                object: None,
            });
        }
    }
    let settings = Settings {
        layout: LAYOUT,
        dungeon_special: false,
        show_monsters: false,
        leader_white: false,
        no_entities: false,
        known_all: true,
        items_visible: false,
        traps_visible: false,
        marked: false,
        stairs_unknown: false,
        blank: false,
        enabled: true,
    };
    let canvas = render(patterns, &grid, &settings).canvas;
    let image = tile_sheet(&canvas, Depth::Bpp4, CANVAS_COLS).ok()?;
    let mut frame = Frame::filled(BACKDROP);
    for y in 0..image.height {
        for x in 0..image.width {
            let Some((_, c)) = image.pixels[y * image.width + x] else {
                continue;
            };
            if c == 0 {
                continue; // index 0 is transparent on a BG
            }
            let colour = palette.get(usize::from(c))?;
            let at = ((MINIMAP_AT.1 + y) * SCREEN_WIDTH as usize + MINIMAP_AT.0 + x) * 4;
            frame.pixels[at..at + 4].copy_from_slice(colour);
        }
    }
    Some(frame)
}

impl Dungeon {
    /// The view of `floor`, the sim's floor of the run: the ROM's tiles and minimap with `data`,
    /// or the placeholder without it. The top screen is blank until the first `draw`. `None` if
    /// drawing fails.
    pub fn new(floor: &Floor, data: Option<&FloorData>) -> Option<Dungeon> {
        let mut terrain = Vec::with_capacity(GRID_W * GRID_H);
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                terrain.push(floor.tile(x as i32, y as i32)?.terrain);
            }
        }
        match data {
            Some(d) => {
                let cex = d.tileset.autotile.as_deref()?;
                let mut variants = GlobalRng::from_seed_bytes(VARIANT_SEED);
                // Retail variant draws depend on frame timing, so these are not reproducible
                // (#400 P3); the global RNG only stands in for them.
                let cells = autotile(&terrain, u16::from(d.tileset_no), cex, &[], false, || {
                    variants.int(4).cast_unsigned()
                })
                .ok()?;
                let mut layer = floor_layer(&d.tileset, &cells).ok()?;
                // BG3 at priority 3, as in the retail dungeon: sprites at priority 3 are drawn
                // over it (docs/re/ui-screens.md, section 1).
                layer.priority = 3;
                let minimap = minimap_frame(floor, &d.patterns, &d.minimap_palette)?;
                Some(Dungeon {
                    frame: Frame::filled(BACKDROP),
                    minimap: Some(minimap),
                    layer: Some(layer),
                })
            }
            None => {
                // One solid 8-pixel cell per terrain: wall 1, floor 3, secondary 2.
                let cells: Vec<u8> = terrain
                    .iter()
                    .map(|t| match t & 3 {
                        0 => 1,
                        1 => 3,
                        _ => 2,
                    })
                    .collect();
                let layer = solid_cells_layer(&cells, WIDTH, HEIGHT)?;
                Some(Dungeon {
                    frame: compose(&[layer], &[PALETTE.to_vec()], BACKDROP),
                    minimap: None,
                    layer: None,
                })
            }
        }
    }

    /// Draws the top screen with the leader's sprite `anim` at tile `cell`, `sub` pixels off
    /// the tile's centre (a walk's offset, `walk::Walk::pose`). The floor scrolls so that point
    /// is at the leader's place on screen. Does nothing for the placeholder.
    pub fn draw(&mut self, data: &FloorData, anim: &AnimPlayer, cell: (i32, i32), sub: (i32, i32)) {
        let Some(layer) = self.layer.as_mut() else {
            return;
        };
        let camera = follow(cell, sub);
        // The layer wraps, so a scroll past its edge shows the far side of the floor.
        (layer.scroll_x, layer.scroll_y) = camera.scroll;
        let image = data.leader_image(anim);
        let at = sprite_origin(camera.sprite_origin, anim.step());
        let sprites: Vec<PlacedSprite<'_>> = image
            .iter()
            .map(|image| PlacedSprite { image, at })
            .collect();
        self.frame = compose_with_sprites(
            std::slice::from_ref(&*layer),
            &data.tileset.palettes,
            BACKDROP,
            &sprites,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};

    #[test]
    fn the_seed_builds_the_same_floor_every_time() {
        let a = first_floor(SEED, None).expect("the fixed seed generates");
        let b = first_floor(SEED, None).expect("the fixed seed generates");
        assert_eq!(a, b);
    }

    #[test]
    fn the_frame_draws_each_visible_cell_in_its_terrain_colour() {
        let (floor, _) = first_floor(SEED, None).expect("the fixed seed generates");
        let d = Dungeon::new(&floor, None).expect("the placeholder draws");
        let mut floor_cells = 0;
        for cy in 0..SCREEN_HEIGHT / 8 {
            for cx in 0..SCREEN_WIDTH / 8 {
                let terrain = floor
                    .tile(cx as i32, cy as i32)
                    .expect("in the map")
                    .terrain;
                let expect = PALETTE[match terrain & 3 {
                    0 => 1,
                    1 => 3,
                    _ => 2,
                }];
                floor_cells += usize::from(expect == PALETTE[3]);
                let got = d.frame.get(cx * 8 + 4, cy * 8 + 4);
                assert_eq!(got, expect, "cell ({cx}, {cy})");
            }
        }
        assert!(floor_cells > 0, "the visible part has open ground");
    }

    #[test]
    fn an_unset_leader_is_an_error_and_the_placeholder_has_no_minimap() {
        assert_eq!(leader_cell((-1, -1)), None);
        assert_eq!(leader_cell((-1, 5)), None);
        assert_eq!(leader_cell((WIDTH as i16, 0)), None);
        assert_eq!(leader_cell((20, 25)), Some((20, 25)));
        let (floor, _) = first_floor(SEED, None).expect("generates");
        let placeholder = Dungeon::new(&floor, None).expect("draws");
        assert!(placeholder.layer.is_none());
        assert!(
            placeholder.minimap.is_none(),
            "no minimap without the ROM's patterns"
        );
    }
}

#[cfg(test)]
mod rom_tests {
    use super::*;
    use rescue_render::camera::LEADER_SCREEN;
    use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};
    use rescue_sim::floor::layout_fnv1a;

    /// The natural floor drawn with the leader idle on `cell`.
    fn idle_at(data: &FloorData, cell: (i16, i16)) -> (Dungeon, AnimPlayer) {
        let (floor, _) = first_floor(SEED, Some(data)).expect("the natural floor builds");
        let mut view = Dungeon::new(&floor, Some(data)).expect("the natural floor draws");
        let anim = data
            .leader_anim(IDLE_GROUP, Direction::Down)
            .expect("the idle pose");
        view.draw(data, &anim, (i32::from(cell.0), i32::from(cell.1)), (0, 0));
        (view, anim)
    }

    rescue_tests::rom_test! {
        /// The first floor from the ROM: layout 1 on the natural seed and on the boot seed
        /// `0x1234_5678`, with the layout hashes of `ORACLE_FLOORS` (`floor.rs`, the layout 1
        /// rows of those seeds). The leader is the natural run's stairs A, (20, 25), and the
        /// camera centres it (#400 P4).
        fn first_floor_matches_the_oracle_and_centres_the_leader(rom) {
            let data = FloorData::from_rom(&rom.read(), DEFAULT_DUNGEON, DEFAULT_FLOOR, DEFAULT_SPECIES)
                .expect("the first floor loads");
            assert_eq!(data.tileset_no, 14, "tileset of the first floor");
            assert_eq!(data.props.layout, 1, "layout of the first floor");

            let (natural, leader) = first_floor(SEED, Some(&data)).expect("the natural floor builds");
            assert_eq!(layout_fnv1a(&natural), 0xf56d_01ac_d18e_338a, "natural seed");
            let (boot, _) = first_floor(0x1234_5678, Some(&data)).expect("the boot-seed floor builds");
            assert_eq!(layout_fnv1a(&boot), 0xeb41_617f_5f54_1eec, "seed 0x12345678");

            assert_eq!(leader, (20, 25), "stairs A of the natural floor");
            let (view, _) = idle_at(&data, leader);
            let layer = view.layer.as_ref().expect("the ROM floor has a layer");
            assert_eq!((layer.scroll_x, layer.scroll_y), (364, 508));
        }
    }

    rescue_tests::rom_test! {
        /// A walk's pixel offset moves the scroll by the same amount (#347 P2): the camera
        /// follows the drawn sprite.
        fn the_camera_follows_the_drawn_sprite(rom) {
            let data = FloorData::from_rom(&rom.read(), DEFAULT_DUNGEON, DEFAULT_FLOOR, DEFAULT_SPECIES)
                .expect("the first floor loads");
            let (mut view, anim) = idle_at(&data, (20, 25));
            let rest = view.layer.as_ref().map(|l| (l.scroll_x, l.scroll_y));
            view.draw(&data, &anim, (20, 25), (5, -3));
            let moved = view.layer.as_ref().map(|l| (l.scroll_x, l.scroll_y));
            assert_eq!(moved, rest.map(|(x, y)| (x + 5, y - 3)));
        }
    }

    rescue_tests::rom_test! {
        /// The natural-seed frame has no backdrop or black pixel: the floor is drawn in its
        /// tile colours. Black dots came from the PX control-index 1/5 bug (#403 P1). The
        /// leader's own pixels are left out: its palette has black (its sprite is checked in
        /// `leader_sprite_and_minimap_are_drawn`).
        fn natural_frame_has_no_backdrop_or_black_pixel(rom) {
            let data = FloorData::from_rom(&rom.read(), DEFAULT_DUNGEON, DEFAULT_FLOOR, DEFAULT_SPECIES)
                .expect("the first floor loads");
            let (natural, anim) = idle_at(&data, (20, 25));
            let sprite = data.leader_image(&anim).expect("the idle frame");
            let at = sprite_origin(LEADER_SCREEN, anim.step());
            for y in 0..SCREEN_HEIGHT {
                for x in 0..SCREEN_WIDTH {
                    let (lx, ly) = (x as i32 - at.0 - sprite.left, y as i32 - at.1 - sprite.top);
                    let on_leader = (0..sprite.width as i32).contains(&lx)
                        && (0..sprite.height as i32).contains(&ly)
                        && sprite.pixels[ly as usize * sprite.width + lx as usize].is_some();
                    if on_leader {
                        continue;
                    }
                    let p = natural.frame.get(x, y);
                    assert!(p != BACKDROP && p != [0, 0, 0, 255], "pixel ({x}, {y}) is {p:?}");
                }
            }
        }
    }

    rescue_tests::rom_test! {
        /// One frame from the ROM (#334): the leader's sprite is on the top screen at its
        /// place, each opaque pixel in its own colour where the floor's layer does not cover
        /// it, and the lower screen has the minimap drawn on the backdrop.
        fn leader_sprite_and_minimap_are_drawn(rom) {
            let data = FloorData::from_rom(&rom.read(), DEFAULT_DUNGEON, DEFAULT_FLOOR, DEFAULT_SPECIES)
                .expect("the first floor loads");
            let (d, anim) = idle_at(&data, (20, 25));
            let sprite = data.leader_image(&anim).expect("the idle frame");
            let at = sprite_origin(LEADER_SCREEN, anim.step());
            let mut opaque = 0;
            for y in 0..sprite.height {
                for x in 0..sprite.width {
                    let Some((colour, _)) = sprite.pixels[y * sprite.width + x] else {
                        continue;
                    };
                    let px = at.0 + sprite.left + x as i32;
                    let py = at.1 + sprite.top + y as i32;
                    if !(0..256).contains(&px) || !(0..192).contains(&py) {
                        continue;
                    }
                    opaque += 1;
                    assert_eq!(d.frame.get(px as u32, py as u32), colour, "sprite pixel ({px}, {py})");
                }
            }
            assert!(opaque > 0, "the leader sprite has pixels on screen");

            let lower = d.minimap.expect("the ROM floor has a minimap");
            let drawn = lower.pixels.chunks_exact(4).filter(|p| *p != BACKDROP).count();
            assert!(drawn > 0, "the minimap is drawn on the lower screen");
        }
    }
}
