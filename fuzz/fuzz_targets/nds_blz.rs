//! BLZ decompression on arbitrary input, with a small output limit so the target stays
//! fast and the limit check itself is exercised.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_nds::blz::decompress_limited;

fuzz_target!(|data: &[u8]| {
    let _ = decompress_limited(data, 1 << 20);
});
