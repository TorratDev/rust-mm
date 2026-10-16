//! ROM-gated checks of the asset viewer's render layer (#39) and the resource index
//! (#40): every ROM file and archive sub-file produces a view without a decode error,
//! the identified formats render an image, and the index finds things by name, ID and
//! type. Nothing from the ROM is written to the repository.

use rescue_asset_viewer::game::Game;
use rescue_asset_viewer::{index, png, view};

rescue_tests::rom_test! {
    fn every_resource_views_and_indexes(rom) {
        let g = Game::load(&rom.path).unwrap();
        let mut images = 0;
        for p in &g.packs {
            for e in p.archive.entries() {
                let v = view::view(&g, &p.path, Some(&e.name), view::Options::default());
                assert!(!v.lines.iter().any(|l| l.starts_with("decode error")), "{}/{}: {:?}", p.path, e.name, v.lines.last());
                let must_draw = ["portrait", "sprite", "effect", "tileset", "font", "palettes", "ground map"]
                    .iter()
                    .any(|k| v.kind.starts_with(k));
                if must_draw {
                    let img = v.image.as_ref().unwrap_or_else(|| panic!("{}/{} ({}) has no image", p.path, e.name, v.kind));
                    assert_eq!(img.pixels.len(), img.width * img.height * 4);
                    images += 1;
                }
            }
        }
        for f in g.fs.files() {
            let v = view::view(&g, &f.path, None, view::Options::default());
            assert!(!v.lines.iter().any(|l| l.starts_with("decode error")), "{}", f.path);
        }
        assert!(images > 900, "{images} images");
        let v = view::view(&g, "monster.sbin", Some("ax025"), view::Options { group: 0, direction: 2, tick: 7, palette: 0 });
        let img = v.image.unwrap();
        assert!(png::encode(img.width as u32, img.height as u32, &img.pixels).len() > 100);

        let idx = index::build(&g);
        let back = index::from_tsv(&index::to_tsv(&idx));
        assert_eq!(back.len(), idx.len());
        let hits = index::search(&back, "pikachu");
        assert!(hits.iter().any(|r| r.format == "portrait") && hits.iter().any(|r| r.format == "sprite"));
        assert!(index::search(&back, "kao025").iter().any(|r| r.format == "portrait"));
        assert!(!index::search(&back, "tileset").is_empty());
        assert!(index::search(&back, "Oran Berry").iter().any(|r| r.format == "item"));
        // Statue (species palette 5) is not drawn all black: its pieces use the overworld palette.
        let statue = view::view(&g, "monster.sbin", Some("ax422"), view::Options::default());
        let img = statue.image.unwrap();
        assert!(img.pixels.chunks_exact(4).any(|p| p[3] == 255 && p[..3] != [0, 0, 0]));
        assert_eq!(index::search(&back, "effect object").len(), 139);
        assert_eq!(index::search(&back, "effect background").len(), 8);
        assert!(index::search(&back, "level growth").iter().any(|r| r.name == "Bulbasaur"));
        assert!(index::search(&back, "dungeon 63").iter().any(|r| r.format == "dungeon"));
    }
}
