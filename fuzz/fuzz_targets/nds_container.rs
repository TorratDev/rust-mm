//! Feeds arbitrary bytes through the whole container path: header, FNT/FAT, overlay
//! tables and overlay decompression. Every stage must return an error, never panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_nds::binary::{overlay_data, read_overlay_tables};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

fuzz_target!(|data: &[u8]| {
    let Ok(header) = Header::parse(data) else {
        return;
    };
    let Ok(fs) = Filesystem::parse(data, &header) else {
        return;
    };
    if let Ok((arm9, arm7)) = read_overlay_tables(data, &header) {
        for ov in arm9.iter().chain(arm7.iter()).take(256) {
            let _ = overlay_data(data, &fs, ov);
        }
    }
});
