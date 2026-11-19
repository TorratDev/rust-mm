//! Title-menu screen and palette parsers, and the fade formula, on arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_formats::titlemenu;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = titlemenu::parse_blob(data) {
        let _ = titlemenu::check_tiles(&s);
    }
    let _ = titlemenu::parse(data);
    let n = u32::from(data.first().copied().unwrap_or(0));
    if let Ok(p) = titlemenu::parse_palette(data) {
        let _ = titlemenu::fade_palette(&p, n);
    }
});
