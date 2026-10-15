//! No-panic property for the parsers that `fuzz/` targets (#38). Random and mutated input
//! must produce `Ok` or `Err`, never a panic. This runs on every `cargo test`; the fuzz
//! targets go further with longer runs.

use proptest::prelude::*;
use rescue_formats::compress::decompress_limited;
use rescue_formats::gfx::{
    MapEntry, decode_4bpp_tiles, decode_palette, decode_tilemap, render_tile,
};
use rescue_nds::binary::{overlay_data, read_overlay_tables};
use rescue_nds::blz;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_nds::synth;

/// A valid synthetic ROM with one compressed-looking overlay, then bytes flipped at random.
fn mutate(base: &[u8], flips: &[(usize, u8)]) -> Vec<u8> {
    let mut v = base.to_vec();
    for &(at, val) in flips {
        if !v.is_empty() {
            let i = at % v.len();
            v[i] = val;
        }
    }
    v
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn random_bytes_never_panic_the_container(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        if let Ok(header) = Header::parse(&data)
            && let Ok(fs) = Filesystem::parse(&data, &header)
            && let Ok((arm9, arm7)) = read_overlay_tables(&data, &header)
        {
            for ov in arm9.iter().chain(arm7.iter()) {
                let _ = overlay_data(&data, &fs, ov);
            }
        }
    }

    #[test]
    fn mutated_rom_never_panics(flips in prop::collection::vec((any::<usize>(), any::<u8>()), 0..16)) {
        let base = synth::build_rom(&[("a.bin", b"hello"), ("b.bin", &[1, 2, 3])], &[vec![9; 40]]);
        let data = mutate(&base, &flips);
        if let Ok(header) = Header::parse(&data) {
            let _ = Filesystem::parse(&data, &header);
        }
    }

    #[test]
    fn random_blz_never_panics(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = blz::decompress_limited(&data, 1 << 16);
    }

    #[test]
    fn random_codec_input_never_panics(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = decompress_limited(&data, 1 << 16);
    }

    #[test]
    fn random_gfx_input_never_panics(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let palette = decode_palette(&data, true).unwrap_or_default();
        let _ = decode_tilemap(&data);
        if let Ok(tiles) = decode_4bpp_tiles(&data)
            && let Some(tile) = tiles.first()
        {
            let _ = render_tile(tile, &palette, MapEntry::from_raw(0xFFFF));
        }
    }
}
