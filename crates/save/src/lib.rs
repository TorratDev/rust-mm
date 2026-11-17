//! Save file serialization.
//!
//! Layout, all integers little-endian:
//!
//! | offset | size | field |
//! |-------:|-----:|-------|
//! | 0 | 4 | magic `RSAV` |
//! | 4 | 4 | format version |
//! | 8 | 4 | payload length |
//! | 12 | 4 | CRC-32 (IEEE) of the payload |
//! | 16 | n | payload: the [`Snapshot`] as JSON |
//!
//! [`read`] never panics: every failure is a [`SaveError`].

#![forbid(unsafe_code)]

use rescue_sim::state::Snapshot;

pub const MAGIC: [u8; 4] = *b"RSAV";
pub const VERSION: u32 = 1;
const HEADER_LEN: usize = 16;
/// Largest payload the reader accepts.
pub const MAX_PAYLOAD: usize = 64 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveError {
    /// Fewer bytes than a header.
    TooShort,
    BadMagic,
    UnsupportedVersion(u32),
    /// The header's payload length is over [`MAX_PAYLOAD`].
    PayloadTooLarge(u32),
    /// The file is shorter or longer than the header says.
    LengthMismatch {
        expected: usize,
        actual: usize,
    },
    BadChecksum {
        expected: u32,
        actual: u32,
    },
    /// The checksum matched but the payload is not a snapshot.
    Payload(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "save file is too short"),
            Self::BadMagic => write!(f, "not a save file (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported save version {v}"),
            Self::PayloadTooLarge(n) => write!(f, "save payload of {n} bytes is too large"),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "save file length {actual}, header says {expected}")
            }
            Self::BadChecksum { expected, actual } => {
                write!(
                    f,
                    "save checksum {actual:#010x}, header says {expected:#010x}"
                )
            }
            Self::Payload(e) => write!(f, "save payload is invalid: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// CRC-32 (IEEE 802.3, reflected, as in zlib).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// Serializes a snapshot into a save file.
#[must_use]
pub fn write(snapshot: &Snapshot) -> Vec<u8> {
    // A snapshot is plain integers, vectors and options, which JSON always accepts.
    let payload = serde_json::to_vec(snapshot).unwrap_or_default();
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc32(&payload).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parses a save file. The snapshot still has to pass `GameState::restore`.
pub fn read(bytes: &[u8]) -> Result<Snapshot, SaveError> {
    if bytes.len() < HEADER_LEN {
        return Err(SaveError::TooShort);
    }
    if bytes[..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let version = word(bytes, 4).ok_or(SaveError::TooShort)?;
    if version != VERSION {
        return Err(SaveError::UnsupportedVersion(version));
    }
    let len = word(bytes, 8).ok_or(SaveError::TooShort)?;
    if len as usize > MAX_PAYLOAD {
        return Err(SaveError::PayloadTooLarge(len));
    }
    let expected = HEADER_LEN + len as usize;
    if bytes.len() != expected {
        return Err(SaveError::LengthMismatch {
            expected,
            actual: bytes.len(),
        });
    }
    let want = word(bytes, 12).ok_or(SaveError::TooShort)?;
    let payload = &bytes[HEADER_LEN..];
    let got = crc32(payload);
    if got != want {
        return Err(SaveError::BadChecksum {
            expected: want,
            actual: got,
        });
    }
    serde_json::from_slice(payload).map_err(|e| SaveError::Payload(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn snap(global: u32, run_seed: u32, floors_entered: u32) -> Snapshot {
        Snapshot {
            global,
            run_seed,
            floors_entered,
            dungeon: None,
            floors: Vec::new(),
        }
    }

    #[test]
    fn crc_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    proptest! {
        #[test]
        fn round_trip(g: u32, s: u32, f: u32) {
            let snapshot = snap(g, s, f);
            prop_assert_eq!(read(&write(&snapshot)), Ok(snapshot));
        }

        #[test]
        fn any_bytes_do_not_panic(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = read(&bytes);
        }

        #[test]
        fn flipped_bit_is_rejected(g: u32, idx: usize, bit in 0u8..8) {
            let mut file = write(&snap(g, 1, 0));
            let i = idx % file.len();
            file[i] ^= 1 << bit;
            prop_assert!(read(&file).is_err());
        }
    }

    #[test]
    fn distinct_errors() {
        let file = write(&snap(1, 2, 3));
        assert_eq!(read(&file[..8]), Err(SaveError::TooShort));
        let mut bad = file.clone();
        bad[0] = b'X';
        assert_eq!(read(&bad), Err(SaveError::BadMagic));
        let mut bad = file.clone();
        bad[4] = 9;
        assert_eq!(read(&bad), Err(SaveError::UnsupportedVersion(9)));
        let mut bad = file.clone();
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(read(&bad), Err(SaveError::PayloadTooLarge(u32::MAX)));
        assert!(matches!(
            read(&file[..file.len() - 1]),
            Err(SaveError::LengthMismatch { .. })
        ));
        let mut bad = file.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(matches!(read(&bad), Err(SaveError::BadChecksum { .. })));
        // A well-checksummed payload that is not a snapshot.
        let payload = b"{}";
        let mut bad = file[..8].to_vec();
        bad.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bad.extend_from_slice(&crc32(payload).to_le_bytes());
        bad.extend_from_slice(payload);
        assert!(matches!(read(&bad), Err(SaveError::Payload(_))));
    }

    #[test]
    fn restores_a_state() {
        use rescue_sim::state::GameState;
        let snapshot = snap(0, 0, 0);
        let back = read(&write(&snapshot)).unwrap();
        assert!(GameState::restore(&back).is_ok());
    }
}
