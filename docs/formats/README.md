# Format specs

One spec per parser, with its verification status. Issue: [#41](https://github.com/TorratDev/rust-mm/issues/41). Every spec uses the same parts: where the data lives and its resource IDs, a layout table, field meanings, compression, unknown fields, and verification status with links to the parser and its tests. Survey of every ROM entry: [`file-survey.md`](../file-survey.md).

| Spec | Parser | Status |
|------|--------|--------|
| [nds-container.md](nds-container.md) | `crates/nds` (header, FNT/FAT, overlays, BLZ) | verified (ROM-gated `tests/tests/nds_rom.rs`) |
| [archive.md](archive.md) | `crates/formats` `archive`, `sir0` | verified |
| [compression.md](compression.md) | `crates/formats` `compress` (Nitro), `px` (AT4PX/PKDPX) | PX verified; Nitro codecs unit-tested (unused by Blue) |
| [graphics.md](graphics.md) | `crates/formats` `gfx`, `graphics` | tiles and RGBX palettes verified |
| [portraits.md](portraits.md) | `crates/formats` `portrait` | verified |
| [sprites.md](sprites.md) | `crates/formats` `sprite` | verified; animation meanings open |
| [tilesets.md](tilesets.md) | `crates/formats` `tileset` | verified; autotile bit order a hypothesis |
| [maps.md](maps.md) | `crates/formats` `ground` | verified: maps render (BPA frame info, layer priority open) |
| [text.md](text.md) | `crates/formats` `text`, `font` | verified; some control codes open |
| [data.md](data.md) | `crates/data` | verified; equals Red for species, moves, items |
| [save.md](save.md) | save layout (#53) | research notes |

## Open items (follow-ups)

Collected from the "Unknown fields" sections:

- [x] Ground BPC tile compression: decode, then render ground maps ([maps.md](maps.md)).
- [ ] Ground collision and data layers, BPA frame info, layer priority, maps without their own tiles ([maps.md](maps.md)).
- [ ] Sprite animation group meanings and direction order; piece and step unknowns ([sprites.md](sprites.md)).
- [ ] Autotile neighbour bit order (E/W) and the 48 trailing bytes; `canm` slot targets ([tilesets.md](tilesets.md)).
- [ ] Text codes `#+`, `#>`, `#1`–`#9`, `$v`, `$n`; portrait-change code ([text.md](text.md)).
- [ ] `effect.sbin`, `ornament.sbin`, `titlemenu.sbin`, `sample.sbin`, halt-screen layout ([graphics.md](graphics.md)).
- [ ] Unnamed bytes in species, move and item entries; dungeon names and floor counts ([data.md](data.md)).
