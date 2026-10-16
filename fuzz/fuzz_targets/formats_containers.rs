//! `.sbin` archive, SIR0 and AT4PX/PKDPX on arbitrary input, raw and with the magic
//! forced so the fuzzer reaches the bodies.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_formats::{archive::Archive, px, sir0::Sir0};

fuzz_target!(|data: &[u8]| {
    let _ = Archive::parse(data);
    for magic in [&b"SIR0"[..], b"AT4PX", b"PKDPX", b""] {
        let d = [magic, data].concat();
        let _ = Sir0::parse(&d);
        let _ = px::decompress(&d, 1 << 16);
    }
});
