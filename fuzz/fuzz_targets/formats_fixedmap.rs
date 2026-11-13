//! The fixed-room file (`fixedmap`) on arbitrary input, raw and with the SIR0 magic forced
//! so the fuzzer reaches the table; every entry the parser accepts is decoded in full.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_formats::fixedmap::{ENTRY_COUNT, FixedMap, TileStream};

fuzz_target!(|data: &[u8]| {
    let mut tiles = TileStream::new(data);
    for _ in 0..256 {
        tiles.next_code();
    }
    let forced = [b"SIR0", data].concat();
    for file in [data, &forced[..]] {
        let Ok(map) = FixedMap::parse(file) else {
            continue;
        };
        for n in -1..=i16::try_from(ENTRY_COUNT).unwrap_or(i16::MAX) {
            if let Some(e) = map.entry(n) {
                let mut tiles = TileStream::new(e.stream);
                for _ in 0..usize::from(e.width) * usize::from(e.height) {
                    let _ = tiles.try_next();
                }
            }
        }
    }
});
