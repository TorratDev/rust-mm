//! Nitro codecs (LZ10, LZ11, Huffman 4/8, RLE) on arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_formats::compress::decompress_limited;

fuzz_target!(|data: &[u8]| {
    let _ = decompress_limited(data, 1 << 20);
});
