# Format specs

One spec per parser, with its verification status. Issue: [#41](https://github.com/TorratDev/rust-mm/issues/41).

| Spec | Parser | Status |
|------|--------|--------|
| [nds-container.md](nds-container.md) | `crates/nds` (header, FNT/FAT, overlays, BLZ) | implemented; ROM checks pending |
| [compression.md](compression.md) | `crates/formats` codecs | implemented; not yet checked against Blue |
| [graphics.md](graphics.md) | `crates/formats` palettes, tiles, tilemaps | implemented; not yet checked against Blue |
| [save.md](save.md) | save layout (#53) | research notes |

Not yet written: sprites (#33), portraits (#34), tilesets and maps (#35), text (#36), data tables (#37).
