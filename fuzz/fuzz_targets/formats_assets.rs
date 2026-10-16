//! Blue's asset parsers (portraits, sprites, tilesets, ground maps, text, fonts) on
//! arbitrary input; SIR0-based ones get the magic prepended.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rescue_formats::{effect, font, ground, portrait, sprite::Sprite, text, tileset};

fuzz_target!(|data: &[u8]| {
    let s = [&b"SIR0"[..], data].concat();
    let _ = portrait::parse(&s);
    if let Ok(sp) = Sprite::parse(&s) {
        for i in 0..sp.frames.len().min(64) {
            let _ = sp.render_frame(i);
        }
    }
    if let Ok(o) = effect::parse_object(&s) {
        for i in 0..o.sprite.frames.len().min(64) {
            let _ = o.sprite.render_frame(i);
        }
    }
    let _ = effect::parse_background(&s);
    let _ = tileset::parse_autotile(&s);
    let _ = tileset::parse_palette_anim(&s);
    let _ = tileset::parse_metatiles(data);
    let _ = font::parse_font(&s);
    let _ = font::parse_fixed_font(data);
    let _ = ground::parse_bpl(data);
    let _ = ground::parse_bpc(data);
    let _ = ground::parse_bma(data);
    let _ = text::parse_bin(data);
    if let Ok(strs) = text::parse_str(data) {
        for st in strs.iter().take(64) {
            let _ = text::to_plain(&text::decode(st));
        }
    }
    let _ = text::decode(data);
});
