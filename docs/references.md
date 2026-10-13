# External references

Sources for Blue Rescue Team (NDS) research, with scope and trust level. Ground truth is Blue's own ROM and runtime behavior (see [`roadmap.md`](roadmap.md#reference-sources)). Nothing here is authoritative for Blue until confirmed in Blue's ARM9/overlays or by an emulator trace.

Collected 2026-10-13 (#22).

## How this was collected

- From this session only `github.com` was reachable (web search worked; `git clone` worked).
- These sites were **not reachable** (proxy refused the connection or DNS failed): `projectpokemon.org`, `datacrystal.tcrf.net`, `tcrf.net`, `problemkaputt.de` (GBATEK), `romhacking.net`, `pmdcollab.org`. What this doc says about them comes from search-result summaries only. Their content was not read and is marked **unverified**.
- Sources cloned and read: `pret/pmd-red`, `SBDWolf/PMD-RescueTeam-Cutsceneless-Hack`, `marius851000/pmd_wan`.

## Legend

**Applies to:**
- **Blue** — describes Blue Rescue Team (NDS).
- **Red** — describes Red Rescue Team (GBA) only. Logic hypotheses for Blue at best.
- **Explorers** — describes Explorers of Time/Darkness/Sky. Out of scope unless a format is verified identical in Blue.
- **DS** — generic Nintendo DS hardware or SDK formats.

**Trust:**
- **verified** — checked by us against Blue (cite the `verification.md` entry).
- **read** — we read the source itself (code or doc); not yet checked against Blue.
- **unverified** — known only from search summaries or second-hand claims.

## Red Rescue Team decompilation (logic reference)

| Source | Applies to | Trust | Notes |
|--------|-----------|-------|-------|
| [pret/pmd-red](https://github.com/pret/pmd-red) | Red | read; partly verified | GBA decompilation. Builds Red 1.0 (US), SHA-1 `9f4cfc5b5f4859d17169a485462e977c7aac2b89`. Coverage per subsystem: [V5](verification.md#v5--pmd-red-decomp-coverage). Type chart byte-identical in Blue: [V6](verification.md#v6--blue-reuses-reds-logic). **No license file**: read-only, never vendored or copied (see [`legal.md`](legal.md)). |

**Commit used:** `013475aa04f5be3191e5527c186d9bfceae7cae0` (committed 2026-10-09). Fetch it with [`tools/fetch_pmd_red.sh`](../tools/fetch_pmd_red.sh), which shallow-fetches that commit into `reference/pmd-red/` (ignored by git). Pass `latest` to take the current tip, then update the commit here.

Leads from reading pmd-red (all `[red-hypothesis]` for Blue):
- Red's GBA ROM holds file archives named after the same `.sbin` files Blue has in its NitroFS: `dungeon`, `effect`, `monster`, `ornament`, `system`, `titlemenu` (`include/def_filearchives.h`; Blue's list is in [`file-survey.md`](file-survey.md)). Red's `FileArchive` is an 8-byte magic (`pksdir0`), an entry count and a table of (name pointer, data pointer) pairs (`include/structs/str_file_system.h`). The GBA version uses absolute ROM pointers, so Blue's on-disk layout must differ, but the archive split and entry names may carry over. Blue's `.sbin` files start with short ASCII strings (`b00c`, `efbg`, `A01P`, `ax00`, `flag`, `anim`, `font`, `clmk`), which may be entry names. Unchecked.
- `dump_effect_sbin.py` in pmd-red parses `SIRO`/`SIR0` headers inside Red's `effect` archive, and names entry kinds `efbg` and `efob`. Blue's `effect.sbin` starts with `efbg`, which makes this archive the best first candidate for a Red-to-Blue format comparison. SIR0 is the wrapper Explorers uses for many files. Whether Blue's archive entries are SIR0-wrapped is unchecked.
- RNG: `Random32MersenneTwister` in `src/random_mersenne_twister.c` (V5). Blue's RNG is not identified yet (#47).

## Blue Rescue Team work

| Source | Applies to | Trust | Notes |
|--------|-----------|-------|-------|
| [SBDWolf/PMD-RescueTeam-Cutsceneless-Hack](https://github.com/SBDWolf/PMD-RescueTeam-Cutsceneless-Hack) (commit `b06a27d`, 2024-02-15) | Blue | read | Cutscene-removal hack for Blue. Python scripts extract and reinsert cutscene scripts at fixed ROM offsets (`script_list.json`, e.g. `0x29ED30`). Script commands are 16 bytes, first byte an opcode; `script_extract.py` names some opcodes (`WARPTO_MAP`, `MSG_SPEECH`, `PLAY_SONG`, ...). Some commands embed ARM9 RAM pointers (`0x0216xxxx`). README claims most scripts are byte-identical between Red and Blue and finds Blue scripts by hex-searching Red's. ROM region of their dump not stated, so offsets are unverified for our EUR dump. No license file: read-only. |
| [TASVideos user files for Blue](https://tasvideos.org/UserFiles/Game/1268), ThunderAxe31's dungeon bruteforce Lua script ([forum](https://TASVideos.org/Forum/Posts/511520)) | Blue | unverified | Per search summary: dungeon RNG seed at `0x02115414`, floor RNG at `0x021C2790`. Region unknown. Useful as a starting point for RNG RE (#47) and V7 triggers; confirm in our dump. |
| [Mystery Dungeon Wiki: Rescue Team Dungeon RNG Manipulation](https://mobile.mysterydungeonwiki.com/wiki/Rescue_Team:Dungeon_RNG_Manipulation) | Blue (and Red?) | unverified | Per search summary: after a quicksave-continue, a 4-byte RAM value (the dungeon seed for the next floor) is set to 1, making floor layouts, spawns and items predictable. Relevant to golden traces (Phase 6). |
| [Project Pokémon wiki, NDS Mystery Dungeon category](https://projectpokemon.org/wiki/Category:NDS_Mystery_Dungeon) | Blue + Explorers | unverified | Not reachable. Search summaries list a "Red and Blue Rescue Team" page and a "Rescue Team Save Structure" page next to Explorers format pages (SIR0, WAN/WAT, PKDPX, AT4PX, `text_*.str`). Each page must be checked for which game it was written against. |
| [psy_commando's tools and research notes thread](https://projectpokemon.org/home/forums/topic/31407-pokemon-mystery-dungeon-2-psy_commandos-tools-and-research-notes/) | Explorers (Blue mentioned) | unverified | Not reachable. Per search summary, mentions that its sbin unpacking and sprite approach "might also apply" to Blue. |
| [Data Crystal: Blue Rescue Team](https://datacrystal.tcrf.net/wiki/Pok%C3%A9mon_Mystery_Dungeon:_Blue_Rescue_Team) | Blue | unverified | Not reachable. Per search summary, links the AT4PX format page. |
| [TCRF: Red/Blue Rescue Team](https://new.tcrf.net/Pok%C3%A9mon_Mystery_Dungeon:_Red_Rescue_Team) | Red + Blue | unverified | Not reachable. Unused-content notes; may point at debug features. |
| [Senexis/PMD-1-Tools](https://github.com/Senexis/PMD-1-Tools) (archived), [Dreaker75/PMD-Rescue-Team-IQ-Calculator](https://github.com/Dreaker75/PMD-Rescue-Team-IQ-Calculator) | Red + Blue | unverified | Web tools built on gameplay data (IQ from Gummis, etc.). Not RE sources; possible cross-checks for data tables. Not read. |

No Blue decompilation or disassembly project was found (GitHub repository search and web search, 2026-10-13).

Not used: a 2005 Blue prototype listed on Hidden Palace. It is a leaked build; [`legal.md`](legal.md) excludes leaked and prototype builds.

## Explorers tooling (out of scope unless verified in Blue)

Same series, later engine. Listed because Blue shares some magics with it; each format must be verified against Blue before use.

| Source | Applies to | Trust | Notes |
|--------|-----------|-------|-------|
| Project Pokémon: [AT4PX](https://projectpokemon.org/wiki/Pmd2_AT4PX), [PKDPX](https://projectpokemon.org/wiki/Pmd2_PKDPX), [PX compression](https://projectpokemon.org/wiki/Pmd2_PX_Compression) | Explorers | unverified | Not reachable. Per search summary: AT4PX is an 18-byte header + PX-compressed image data; PKDPX a 20-byte header for any data; PX is an LZ-like scheme driven by a command byte. Blue's `nitro_haltscreen*.sbin` files start with `AT4P` ([file survey](file-survey.md)), so AT4PX is the first candidate for those (#28–#41). |
| Project Pokémon: [WAN/WAT](https://projectpokemon.org/wiki/Pmd2_WAN) | Explorers | unverified | Not reachable. Sprite format, wrapped in SIR0. |
| [marius851000/pmd_wan](https://github.com/marius851000/pmd_wan) (crate `pmd_wan` 6.0.0) | Explorers (claims Rescue Team) | read | Rust WAN reader. README: "used in pokemon mystery dungeon: Explorers and Rescue Team (only tested with Explorers of Sky)"; fuzzed. License CC0-1.0 (in `Cargo.toml`). Whether Blue uses WAN is unchecked. |
| [PsyCommando/ppmdu_2](https://github.com/PsyCommando/ppmdu_2) | Explorers | unverified | C++ tools for Explorers formats (PX compression, archives). Not read. |
| [SkyTemple](https://github.com/SkyTemple) (incl. `skytemple-files`) | Explorers | unverified | Explorers of Sky editor and format library. Not read. |
| pret `pmd-sky` (listed on [pret](https://github.com/pret)), pmdsky-debug | Explorers | unverified | Explorers of Sky decompilation and symbol maps. Different binary; not read. |

## Generic DS documentation

| Source | Applies to | Trust | Notes |
|--------|-----------|-------|-------|
| [GBATEK](https://problemkaputt.de/gbatek.htm) (Martin Korth) | DS | unverified here | Not reachable from this session. Standard hardware reference: cartridge header, NitroROM file system (FNT at header `0x40`, FAT at `0x48`), overlay tables, memory map, LZ/Huffman BIOS decompression. Our header/FNT/FAT/overlay parsing in [`tools/nds_survey.py`](../tools/nds_survey.py) already works on Blue (V4). |
| libnds `sNDSHeader` ([BlocksDS](https://blocksds.skylyrac.net/libnds/structsNDSHeader.html), [devkitPro](https://libnds.devkitpro.org/structsNDSHeader.html)) | DS | unverified | Header field names and offsets as a C struct. Cross-check for `rescue-nds`. |
| [ndspy docs](https://ndspy.readthedocs.io/en/latest/) | DS | unverified | Python DS library. [SDAT structure appendix](https://ndspy.readthedocs.io/en/latest/appendices/sdat-structure.html); [`codeCompression`](https://ndspy.readthedocs.io/en/latest/api/codeCompression.html) documents BLZ ("backwards LZ", LZ10 decoded from the end; ARM9 handled slightly differently from overlays). Blue's overlays are BLZ-flagged (V4) and decoded in V6. |
| Nitro Composer (SDAT) specification ([Retro CDN copy](https://retrocdn.net/Nitro_Composer_File_Specification)) | DS | unverified | Unofficial 2007 spec (originally kiwi.ds) for SSEQ, SSAR, SBNK, SWAV, SWAR, STRM; the author says it is incomplete. Blue's `sound.sbin` is an SDAT (V3). |
| RomHacking.net "NDS formats" document (#469, copy not linked) | DS | unverified | Per search summary: NCLR, NCGR, NSCR, NCER, NANR, NARC, SDAT; generic Nitro header and sections; BGR555 palettes. Original page not reachable. |
| [NitroPaint](https://git.randommeaninglesscharacters.com/Garhoogin/NitroPaint), [nitrogfx-py](https://pypi.org/project/nitrogfx-py) | DS | unverified | Reference implementations for Nitro graphics formats (NCLR/NCGR/NSCR/NCER/NANR). None of Blue's top-level files carry Nitro graphics magics (file survey); they may appear inside `.sbin` archives. |
| [ds-decomp](https://docs.rs/crate/ds-decomp/latest) | DS | unverified | Rust DS decompilation toolkit. Possible help for V8 (overlay mapping) if Ghidra stays blocked. |

## Open questions from this survey

- Do Blue's `.sbin` archives match Red's `FileArchive` organisation (names, entry order) or an Explorers-style container?
- Are Blue's archive entries SIR0-wrapped, and do AT4PX/PKDPX/PX match the Explorers descriptions?
- Is Blue's cutscene script bytecode identical to Red's, as the cutscene hack claims? If so, pmd-red's script data and commands are a direct map.
- Do the TASVideos RNG addresses hold in our EUR dump?
