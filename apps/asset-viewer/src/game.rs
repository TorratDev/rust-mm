//! The loaded ROM: identity check, filesystem, `.sbin` archives and the name tables the
//! viewer and the index label things with.

use std::path::{Path, PathBuf};

use rescue_data::{item, moves, species};
use rescue_formats::archive::Archive;
use rescue_formats::graphics::{self, Palette};
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;
use rescue_nds::rom::{self, RomId};

/// A ROM file that is a `.sbin` pack archive.
pub struct Pack {
    /// ROM path, e.g. `monster.sbin`.
    pub path: String,
    /// File bytes.
    pub data: Vec<u8>,
    /// Entry table.
    pub archive: Archive,
}

/// Everything loaded from one supported dump.
pub struct Game {
    /// Identified dump.
    pub id: RomId,
    /// Whole ROM.
    pub rom: Vec<u8>,
    /// Header.
    pub header: Header,
    /// Filesystem.
    pub fs: Filesystem,
    /// Pack archives by ROM path.
    pub packs: Vec<Pack>,
    /// Species names by index (empty if `monspara` failed to parse).
    pub species: Vec<String>,
    /// Move names.
    pub moves: Vec<String>,
    /// Item names.
    pub items: Vec<String>,
    /// Shared sprite palettes (`monster.sbin` `palet`).
    pub sprite_palettes: Vec<Palette>,
}

/// Where the ROM path comes from: the argument, else `RESCUE_ROM`, else `rescue.toml`
/// in the current directory.
pub fn rom_path(arg: Option<&str>) -> Result<PathBuf, String> {
    if let Some(a) = arg {
        return Ok(PathBuf::from(a));
    }
    let cfg = Path::new(rom::CONFIG_FILE_NAME);
    rom::resolve_rom_path(std::env::var_os(rom::ROM_PATH_ENV), Some(cfg))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "no ROM given: pass a path, set {}, or put rom_path in {}",
                rom::ROM_PATH_ENV,
                rom::CONFIG_FILE_NAME
            )
        })
}

impl Game {
    /// Reads, identifies (SHA-1 against the supported list) and parses the ROM.
    pub fn load(path: &Path) -> Result<Self, String> {
        let rom =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let id = rom::identify_bytes(&rom).map_err(|e| e.to_string())?;
        let header = Header::parse(&rom).map_err(|e| e.to_string())?;
        let fs = Filesystem::parse(&rom, &header).map_err(|e| e.to_string())?;
        let mut packs = Vec::new();
        for f in fs.files() {
            let data = fs.data(&rom, f.id).map_err(|e| e.to_string())?;
            // Only `.sbin` pack files: message `.str` tables also parse as archives.
            if !f.path.ends_with(".sbin") {
                continue;
            }
            if let Ok(archive) = Archive::parse(data) {
                packs.push(Pack {
                    path: f.path.clone(),
                    data: data.to_vec(),
                    archive,
                });
            }
        }
        let mut g = Game {
            id,
            rom,
            header,
            fs,
            packs,
            species: Vec::new(),
            moves: Vec::new(),
            items: Vec::new(),
            sprite_palettes: Vec::new(),
        };
        if let Some(d) = g.sub("system.sbin", "monspara") {
            g.species = species::parse_species(d)
                .map(|v| v.into_iter().map(|s| s.name).collect())
                .unwrap_or_default();
        }
        if let Some(d) = g.sub("system.sbin", "wazapara") {
            g.moves = moves::parse_moves(d)
                .map(|m| m.moves.into_iter().map(|m| m.name).collect())
                .unwrap_or_default();
        }
        if let Some(d) = g.sub("system.sbin", "itempara") {
            g.items = item::parse_items(d)
                .map(|v| v.into_iter().map(|i| i.name).collect())
                .unwrap_or_default();
        }
        if let Some(d) = g.sub("monster.sbin", "palet") {
            g.sprite_palettes = graphics::rgbx_palettes16(d).unwrap_or_default();
        }
        Ok(g)
    }

    /// The pack at ROM path `path`.
    pub fn pack(&self, path: &str) -> Option<&Pack> {
        self.packs.iter().find(|p| p.path == path)
    }

    /// Bytes of sub-file `name` of pack `pack`.
    pub fn sub(&self, pack: &str, name: &str) -> Option<&[u8]> {
        let p = self.pack(pack)?;
        p.archive.get(&p.data, name).ok()
    }

    /// Bytes of a named ROM file.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.fs.data(&self.rom, self.fs.get(path)?.id).ok()
    }

    /// Species name for `id`, or `#id`.
    pub fn species_name(&self, id: u16) -> String {
        self.species
            .get(usize::from(id))
            .cloned()
            .unwrap_or_else(|| format!("#{id}"))
    }
}
