//! The save-file reader on arbitrary bytes: it must return an error, never panic.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = rescue_save::read(data);
    let _ = rescue_save::load(data);
});
