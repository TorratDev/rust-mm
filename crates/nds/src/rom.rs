//! ROM location and identification.
//!
//! Every tool reads game data at fixed offsets, so it must know exactly which dump it is
//! looking at. A dump is identified by the SHA-1 of the whole file and accepted only if
//! that hash is on the supported list ([`SUPPORTED_DUMPS`], documented in
//! `docs/supported-dumps.md`). Anything else is refused with [`RomError::UnknownDump`],
//! whose message gives the computed hash and lists the supported dumps.
//!
//! The ROM path comes from the [`ROM_PATH_ENV`] environment variable or, failing that,
//! the `rom_path` entry of a `rescue.toml` config file (see [`resolve_rom_path`]).
//!
//! Nothing here embeds ROM data: the only facts recorded are whole-file sizes and hashes.

use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1 as Sha1Hasher};

/// Environment variable holding the path to the user's ROM dump.
pub const ROM_PATH_ENV: &str = "RESCUE_ROM";

/// Default config file name, looked up in the current directory by apps and in the
/// workspace root by tests.
pub const CONFIG_FILE_NAME: &str = "rescue.toml";

/// Config file key holding the ROM path.
pub const CONFIG_ROM_PATH_KEY: &str = "rom_path";

/// Where users find the list of supported dumps.
pub const SUPPORTED_DUMPS_DOC: &str = "docs/supported-dumps.md";

/// A SHA-1 digest (20 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sha1(pub [u8; 20]);

impl Sha1 {
    /// Parses 40 hex digits (either case). Returns `None` for anything else.
    pub const fn from_hex(hex: &str) -> Option<Self> {
        let bytes = hex.as_bytes();
        if bytes.len() != 40 {
            return None;
        }
        let mut out = [0u8; 20];
        let mut i = 0;
        while i < 20 {
            let (Some(hi), Some(lo)) = (hex_digit(bytes[2 * i]), hex_digit(bytes[2 * i + 1]))
            else {
                return None;
            };
            out[i] = (hi << 4) | lo;
            i += 1;
        }
        Some(Self(out))
    }

    /// SHA-1 of an in-memory buffer.
    pub fn of_bytes(data: &[u8]) -> Self {
        Self(Sha1Hasher::digest(data).into())
    }

    /// SHA-1 of everything `reader` yields, plus the number of bytes read.
    pub fn of_reader(mut reader: impl Read) -> io::Result<(Self, u64)> {
        let mut hasher = Sha1Hasher::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut len: u64 = 0;
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            let chunk = buf.get(..n).unwrap_or_default();
            hasher.update(chunk);
            len = len.saturating_add(chunk.len() as u64);
        }
        Ok((Self(hasher.finalize().into()), len))
    }
}

const fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Compile-time hex literal for the supported-dumps table.
const fn sha1_lit(hex: &str) -> Sha1 {
    match Sha1::from_hex(hex) {
        Some(h) => h,
        None => panic!("invalid SHA-1 literal"),
    }
}

impl fmt::Display for Sha1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Sha1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha1({self})")
    }
}

/// A dump the project supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RomId {
    /// Blue Rescue Team, Nintendo DS, Europe.
    BlueRescueTeamEur,
}

impl RomId {
    /// The supported-dumps entry for this ID.
    pub fn dump(self) -> &'static KnownDump {
        match self {
            RomId::BlueRescueTeamEur => &SUPPORTED_DUMPS[0],
        }
    }
}

impl fmt::Display for RomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.dump();
        write!(f, "{} ({}, {})", d.title, d.platform, d.region)
    }
}

/// One entry of the supported-dumps list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownDump {
    /// Typed identifier returned on a match.
    pub id: RomId,
    /// Game title, as used in documentation.
    pub title: &'static str,
    /// Platform.
    pub platform: &'static str,
    /// Region / revision.
    pub region: &'static str,
    /// Whole-file size in bytes.
    pub size: u64,
    /// Whole-file SHA-1.
    pub sha1: Sha1,
}

impl fmt::Display for KnownDump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {}), {} bytes, SHA-1 {}",
            self.title, self.platform, self.region, self.size, self.sha1
        )
    }
}

/// Dumps the tools accept. Keep in sync with `docs/supported-dumps.md`.
pub const SUPPORTED_DUMPS: &[KnownDump] = &[KnownDump {
    id: RomId::BlueRescueTeamEur,
    title: "Blue Rescue Team",
    platform: "NDS",
    region: "EUR",
    size: 33_554_432,
    sha1: sha1_lit("6f01887356268de39085df01490347bbcc3aa330"),
}];

/// Errors from locating or identifying a ROM.
#[derive(Debug)]
#[non_exhaustive]
pub enum RomError {
    /// The ROM file (or config file) could not be read.
    Io {
        /// File that failed.
        path: PathBuf,
        /// Underlying error.
        source: io::Error,
    },
    /// The config file is malformed.
    Config {
        /// Config file.
        path: PathBuf,
        /// 1-based line number.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// The file's SHA-1 is not on the supported list.
    UnknownDump {
        /// Computed SHA-1.
        sha1: Sha1,
        /// File size in bytes.
        size: u64,
        /// The list it was checked against.
        supported: Vec<KnownDump>,
    },
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::Io { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            RomError::Config {
                path,
                line,
                message,
            } => write!(f, "{}:{line}: {message}", path.display()),
            RomError::UnknownDump {
                sha1,
                size,
                supported,
            } => {
                writeln!(
                    f,
                    "unsupported ROM dump: SHA-1 {sha1} ({size} bytes) is not a supported dump."
                )?;
                writeln!(f, "Supported dumps:")?;
                for d in supported {
                    writeln!(f, "  - {d}")?;
                }
                write!(
                    f,
                    "Other regions, revisions, trimmed, patched or bad dumps are refused. \
                     See {SUPPORTED_DUMPS_DOC}."
                )
            }
        }
    }
}

impl std::error::Error for RomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RomError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Matches a hash against a supported-dumps table.
pub fn identify_hash(sha1: Sha1, size: u64, table: &[KnownDump]) -> Result<RomId, RomError> {
    table
        .iter()
        .find(|d| d.sha1 == sha1 && d.size == size)
        .map(|d| d.id)
        .ok_or_else(|| RomError::UnknownDump {
            sha1,
            size,
            supported: table.to_vec(),
        })
}

/// Identifies an in-memory dump against [`SUPPORTED_DUMPS`].
pub fn identify_bytes(data: &[u8]) -> Result<RomId, RomError> {
    identify_bytes_with(data, SUPPORTED_DUMPS)
}

/// Identifies an in-memory dump against `table`.
pub fn identify_bytes_with(data: &[u8], table: &[KnownDump]) -> Result<RomId, RomError> {
    identify_hash(Sha1::of_bytes(data), data.len() as u64, table)
}

/// Identifies the dump at `path` against [`SUPPORTED_DUMPS`].
pub fn identify_file(path: &Path) -> Result<RomId, RomError> {
    identify_file_with(path, SUPPORTED_DUMPS)
}

/// Identifies the dump at `path` against `table`, hashing it without loading it whole.
pub fn identify_file_with(path: &Path, table: &[KnownDump]) -> Result<RomId, RomError> {
    let io_err = |source| RomError::Io {
        path: path.to_path_buf(),
        source,
    };
    let file = File::open(path).map_err(io_err)?;
    let (sha1, size) = Sha1::of_reader(file).map_err(io_err)?;
    identify_hash(sha1, size, table)
}

/// Reads `rom_path` from config file text. `Ok(None)` if the key is absent.
///
/// The format is the TOML subset `rom_path = "..."` (basic string, `\\` and `\"`
/// escapes) or `rom_path = '...'` (literal string, handy for Windows paths), at top
/// level. Comments (`#`) and blank lines are skipped; other keys and tables are ignored.
/// A relative path is resolved against `config_dir`.
pub fn parse_config_rom_path(
    text: &str,
    config_path: &Path,
    config_dir: &Path,
) -> Result<Option<PathBuf>, RomError> {
    let err = |line: usize, message: &str| RomError::Config {
        path: config_path.to_path_buf(),
        line,
        message: message.to_owned(),
    };
    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            // Top-level keys come before the first table.
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != CONFIG_ROM_PATH_KEY {
            continue;
        }
        let value = parse_string_value(value.trim())
            .ok_or_else(|| err(line_no, "rom_path must be a quoted string"))?;
        if value.is_empty() {
            return Err(err(line_no, "rom_path is empty"));
        }
        let p = PathBuf::from(value);
        return Ok(Some(if p.is_absolute() {
            p
        } else {
            config_dir.join(p)
        }));
    }
    Ok(None)
}

/// Parses a TOML basic (`"..."`) or literal (`'...'`) string, allowing a trailing comment.
fn parse_string_value(v: &str) -> Option<String> {
    let mut chars = v.chars();
    let quote = chars.next()?;
    let mut out = String::new();
    match quote {
        '\'' => loop {
            match chars.next()? {
                '\'' => break,
                c => out.push(c),
            }
        },
        '"' => loop {
            match chars.next()? {
                '"' => break,
                '\\' => match chars.next()? {
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    _ => return None,
                },
                c => out.push(c),
            }
        },
        _ => return None,
    }
    let rest = chars.as_str().trim();
    if rest.is_empty() || rest.starts_with('#') {
        Some(out)
    } else {
        None
    }
}

/// Resolves the configured ROM path.
///
/// `env_value` is the value of [`ROM_PATH_ENV`] (pass `std::env::var_os(ROM_PATH_ENV)`);
/// a non-empty value wins. Otherwise `config_file` is read if it exists and its
/// `rom_path` used. `Ok(None)` means no ROM is configured.
pub fn resolve_rom_path(
    env_value: Option<OsString>,
    config_file: Option<&Path>,
) -> Result<Option<PathBuf>, RomError> {
    if let Some(v) = env_value.filter(|v| !v.is_empty()) {
        return Ok(Some(PathBuf::from(v)));
    }
    let Some(cfg) = config_file else {
        return Ok(None);
    };
    let text = match std::fs::read_to_string(cfg) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(RomError::Io {
                path: cfg.to_path_buf(),
                source,
            });
        }
    };
    let dir = cfg.parent().unwrap_or_else(|| Path::new("."));
    parse_config_rom_path(&text, cfg, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_table(data: &[u8]) -> [KnownDump; 1] {
        [KnownDump {
            sha1: Sha1::of_bytes(data),
            size: data.len() as u64,
            ..SUPPORTED_DUMPS[0]
        }]
    }

    #[test]
    fn reference_dump_is_recorded() {
        let d = RomId::BlueRescueTeamEur.dump();
        assert_eq!(d.size, 33_554_432);
        assert_eq!(
            d.sha1.to_string(),
            "6f01887356268de39085df01490347bbcc3aa330"
        );
        assert_eq!(SUPPORTED_DUMPS.len(), 1);
    }

    #[test]
    fn sha1_known_vector() {
        assert_eq!(
            Sha1::of_bytes(b"abc").to_string(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        let (h, n) = Sha1::of_reader(&b"abc"[..]).unwrap();
        assert_eq!(h, Sha1::of_bytes(b"abc"));
        assert_eq!(n, 3);
    }

    #[test]
    fn hex_round_trip_and_rejects_garbage() {
        let h = Sha1::from_hex("6F01887356268DE39085DF01490347BBCC3AA330").unwrap();
        assert_eq!(h, RomId::BlueRescueTeamEur.dump().sha1);
        assert!(Sha1::from_hex("").is_none());
        assert!(Sha1::from_hex("6f01").is_none());
        assert!(Sha1::from_hex("zz01887356268de39085df01490347bbcc3aa330").is_none());
        assert!(Sha1::from_hex("6f01887356268de39085df01490347bbcc3aa330ff").is_none());
    }

    #[test]
    fn synthetic_data_is_refused_by_real_table() {
        let data = vec![0xA5u8; 4096];
        let err = identify_bytes(&data).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&Sha1::of_bytes(&data).to_string()), "{msg}");
        assert!(msg.contains("4096 bytes"), "{msg}");
        assert!(
            msg.contains("6f01887356268de39085df01490347bbcc3aa330"),
            "{msg}"
        );
        assert!(msg.contains(SUPPORTED_DUMPS_DOC), "{msg}");
        assert!(identify_bytes(&[]).is_err());
    }

    #[test]
    fn synthetic_data_matches_fake_table() {
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let table = fake_table(&data);
        assert_eq!(
            identify_bytes_with(&data, &table).unwrap(),
            RomId::BlueRescueTeamEur
        );
        // One flipped byte (a bad dump) is refused.
        let mut bad = data.clone();
        bad[1234] ^= 1;
        assert!(matches!(
            identify_bytes_with(&bad, &table),
            Err(RomError::UnknownDump { .. })
        ));
        // Same hash but wrong size is refused too.
        assert!(identify_hash(table[0].sha1, 1, &table).is_err());
    }

    #[test]
    fn identify_file_reports_io_errors() {
        let path = Path::new("definitely/not/a/rom.nds");
        let err = identify_file(path).unwrap_err();
        assert!(matches!(err, RomError::Io { .. }));
        assert!(err.to_string().contains("rom.nds"));
    }

    #[test]
    fn config_parsing() {
        let cfg = Path::new("/cfg/rescue.toml");
        let dir = Path::new("/cfg");
        let parse = |t: &str| parse_config_rom_path(t, cfg, dir);

        assert_eq!(parse("").unwrap(), None);
        assert_eq!(parse("# nothing\nother = 1\n").unwrap(), None);
        assert_eq!(
            parse("rom_path = \"roms/game.nds\" # mine\n").unwrap(),
            Some(dir.join("roms/game.nds"))
        );
        assert_eq!(
            parse("  rom_path='C:\\roms\\x.nds'").unwrap(),
            Some(dir.join("C:\\roms\\x.nds"))
        );
        assert_eq!(
            parse("rom_path = \"a\\\\b\\\"c\"").unwrap(),
            Some(dir.join("a\\b\"c"))
        );
        // Keys inside tables are not top-level.
        assert_eq!(parse("[other]\nrom_path = \"x\"").unwrap(), None);

        for bad in [
            "rom_path = x",
            "rom_path = \"unterminated",
            "rom_path = \"\"",
            "rom_path = \"a\" trailing",
            "rom_path = \"\\n\"",
            "rom_path =",
        ] {
            let err = parse(bad).unwrap_err();
            assert!(matches!(err, RomError::Config { line: 1, .. }), "{bad}");
        }
    }

    #[test]
    fn env_wins_and_missing_config_is_none() {
        let missing = Path::new("definitely/not/here/rescue.toml");
        assert_eq!(resolve_rom_path(None, None).unwrap(), None);
        assert_eq!(resolve_rom_path(None, Some(missing)).unwrap(), None);
        assert_eq!(
            resolve_rom_path(Some(OsString::new()), Some(missing)).unwrap(),
            None
        );
        assert_eq!(
            resolve_rom_path(Some("x.nds".into()), Some(missing)).unwrap(),
            Some(PathBuf::from("x.nds"))
        );
    }
}
