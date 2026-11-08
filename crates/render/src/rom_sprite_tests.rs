//! ROM-gated check of the sprite renderer on a real species (docs/rom-tests.md). Skipped
//! without a ROM. Asserts only non-emptiness and determinism, never ROM bytes.

use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

rescue_tests::rom_test! {
    /// Draws a frame of species 25 (Pikachu) from its idle animation in its overworld palette
    /// and checks it is non-empty, deterministic and drawn with colours of that palette
    /// (a few pieces may keep their own palette bits, so this only requires some pixels
    /// from the species palette). Also checks the player walks a real group.
    fn species_sprite_draws(rom) {
        use rescue_data::species;
        use rescue_formats::graphics;
        use rescue_formats::sprite::Sprite;

        let bytes = rom.read();
        let header = Header::parse(&bytes).expect("header parses");
        let fs = Filesystem::parse(&bytes, &header).expect("FNT/FAT parse");
        let load = |path: &str| {
            let f = fs.get(path).unwrap_or_else(|| panic!("{path} missing"));
            fs.data(&bytes, f.id).expect("file bytes").to_vec()
        };
        let monster = load("monster.sbin");
        let marc = Archive::parse(&monster).unwrap();
        let sub = |a: &Archive, d: &[u8], n: &str| {
            a.data(d, a.find(n).unwrap_or_else(|| panic!("{n} missing"))).unwrap().to_vec()
        };
        let system = load("system.sbin");
        let sarc = Archive::parse(&system).unwrap();
        let species_palette = species::parse_species(&sub(&sarc, &system, "monspara")).unwrap()[25].palette;
        let palettes = graphics::rgbx_palettes16(&sub(&marc, &monster, "palet")).unwrap();
        let sprite = Sprite::parse(&sub(&marc, &monster, "ax025")).unwrap();

        let mut anim = crate::anim::AnimPlayer::new(&sprite, 7, 0, true).expect("idle group");
        anim.advance(3);
        let img = crate::sprite::render_sprite(&sprite, anim.frame(), species_palette, &palettes).unwrap();
        assert!(img.pixels.iter().any(Option::is_some), "empty sprite");
        let again = crate::sprite::render_sprite(&sprite, anim.frame(), species_palette, &palettes).unwrap();
        assert!(img == again, "sprite render is not deterministic");
        let bank = &palettes[usize::from(species_palette)];
        assert!(
            img.pixels.iter().flatten().any(|(c, _)| bank.contains(c)),
            "no pixel uses the species palette {species_palette}"
        );
        // On a background layer too.
        let frame = crate::sprite::compose_with_sprites(
            &[],
            &palettes,
            [0, 0, 0, 255],
            &[crate::sprite::PlacedSprite { image: &img, at: (128, 96) }],
        );
        assert!(frame.pixels.chunks_exact(4).any(|p| p != [0, 0, 0, 255]));
    }
}
