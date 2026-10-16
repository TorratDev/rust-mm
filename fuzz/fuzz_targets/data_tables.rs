//! `rescue-data` table parsers on arbitrary SIR0-wrapped input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_data::{dungeon, item, moves, species, types};

fuzz_target!(|data: &[u8]| {
    let s = [&b"SIR0"[..], data].concat();
    let _ = species::parse_species(&s);
    let _ = species::parse_level_data(&s);
    let _ = moves::parse_moves(&s);
    let _ = item::parse_items(&s);
    let _ = dungeon::parse_mapparam(&s, 240);
    let _ = types::type_chart(data);
});
