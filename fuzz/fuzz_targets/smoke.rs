//! Placeholder target that proves the cargo-fuzz toolchain works in CI and links the
//! parser crates. Replace with real parser targets (#38) once `rescue-nds` and
//! `rescue-formats` have parsers.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = data.iter().fold(0u8, |acc, b| acc ^ b);
});
