//! The dungeon scene's floor (#70): generated once from a fixed seed by `rescue_sim`,
//! autotiled and composed by `rescue_render` into a frame. The graphics are placeholders
//! (solid colours per terrain) until the ROM's tilesets are loaded (#60).

use rescue_render::autotile::{CEX_LEN, autotile};
use rescue_render::bg::{Frame, compose, solid_cells_layer};
use rescue_sim::floor::{Floor, HEIGHT, WIDTH};
use rescue_sim::floor_gen::{FloorProperties, GenState, PortedSteps, generate_floor};
use rescue_sim::rng::DungeonRng;

/// The floor's RNG state at `GenerateFloor`'s entry. One of the fixed entry states of the sim's
/// end-to-end floor test (`generate_floor_matches_the_retail_floors_end_to_end`).
pub const SEED: u32 = 0x1234_5678;

/// Palette of the placeholder graphics: index 0 is transparent (not drawn).
const PALETTE: [[u8; 4]; 4] = [
    [0, 0, 0, 0],
    [40, 40, 60, 255],    // wall (metatile 1)
    [120, 90, 60, 255],   // secondary terrain (metatile 2)
    [200, 190, 160, 255], // floor (metatile 3)
];
const BACKDROP: [u8; 4] = [16, 16, 24, 255];

/// A generated floor and its drawn frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dungeon {
    pub floor: Floor,
    pub frame: Frame,
}

/// The first floor of a new game, from the values the sim's recorded rows pin. shortcut: hard-coded
/// instead of read from the ROM's `mapparam` (the trace harness's `properties_for` does that),
/// upgrade when the game loads its ROM data.
fn first_floor() -> FloorProperties {
    FloorProperties {
        layout: 1,
        room_density: 5,
        byte_05: 15,
        byte_06: 4,
        byte_0f: 2,
        byte_13: 5,
        ..Default::default()
    }
}

/// Placeholder autotile table: a cell's metatile is its terrain block (wall 1, secondary 2,
/// floor 3), ignoring the neighbour mask and the variant. shortcut: the real table is the
/// ROM's `cex` (#60).
fn placeholder_cex() -> Vec<u8> {
    (0..CEX_LEN).map(|i| ((i / 3) >> 8) as u8 + 1).collect()
}

/// Generates the floor for `seed` and draws its top-left 32 x 24 cells. `None` if generation
/// or drawing fails (the generator returns an error, or a layout is not ported).
pub fn build(seed: u32) -> Option<Dungeon> {
    let mut floor = Floor::new();
    let mut rng = DungeonRng::from_state(seed);
    let mut g = GenState::default();
    generate_floor(
        &first_floor(),
        false,
        &mut floor,
        &mut rng,
        &mut g,
        &mut PortedSteps::default(),
        false,
    )
    .ok()?;

    let mut terrain = Vec::with_capacity(WIDTH * HEIGHT);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            terrain.push(floor.tile(x as i32, y as i32)?.terrain);
        }
    }
    // shortcut: variant draws are always 0; the real ones come from the global RNG (#60).
    let cells = autotile(&terrain, 0, &placeholder_cex(), &[], false, || 0).ok()?;
    let layer = solid_cells_layer(&cells, WIDTH, HEIGHT)?;
    let frame = compose(&[layer], &[PALETTE.to_vec()], BACKDROP);
    Some(Dungeon { floor, frame })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescue_render::{SCREEN_HEIGHT, SCREEN_WIDTH};

    #[test]
    fn the_seed_builds_the_same_floor_every_time() {
        let a = build(SEED).expect("the fixed seed generates");
        let b = build(SEED).expect("the fixed seed generates");
        assert_eq!(a, b);
    }

    #[test]
    fn the_frame_draws_each_visible_cell_in_its_terrain_colour() {
        let d = build(SEED).expect("the fixed seed generates");
        let mut floor_cells = 0;
        for cy in 0..SCREEN_HEIGHT / 8 {
            for cx in 0..SCREEN_WIDTH / 8 {
                let terrain = d
                    .floor
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
}
