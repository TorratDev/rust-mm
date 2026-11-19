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
//! | 16 | n | payload: JSON `{"snapshot": Snapshot, "resume": Resume}` |
//!
//! [`read`] never panics: every failure is a [`SaveError`].
//!
//! # Versioning rule
//!
//! [`VERSION`] names the payload's shape and meaning. Change it in the same commit as any
//! change to the payload: a field added to or removed from [`Snapshot`], [`Resume`] or what
//! they hold, a field read differently, or a new piece of sim state that a run needs to
//! continue. The reader accepts only the current version and returns
//! [`SaveError::UnsupportedVersion`] for any other; there is no migration, so an old save is
//! refused, never half-read. New sim state must go into [`Resume`] (or the snapshot) and into
//! the property test `save_and_load_continues_the_run`, which fails when a field is missed.
//!
//! Version 1: the snapshot alone. Version 2: the snapshot and [`Resume`] (spawn state, sport
//! timers, AI records, species sleep chances).

#![forbid(unsafe_code)]

use rescue_sim::state::{GameState, Resume, Snapshot, StateError};

pub const MAGIC: [u8; 4] = *b"RSAV";
pub const VERSION: u32 = 2;
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
    /// The snapshot decoded but [`GameState::restore`] refused it.
    Restore(StateError),
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
            Self::Restore(e) => write!(f, "save does not restore: {e:?}"),
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

/// Serializes a snapshot into a save file, with no [`Resume`]: a load rebuilds the floor's
/// spawn state and AI records as [`GameState::restore`] does. [`write_run`] keeps a run whole.
#[must_use]
pub fn write(snapshot: &Snapshot) -> Vec<u8> {
    encode(snapshot, &Resume::default())
}

/// Serializes a whole run: its snapshot and everything the snapshot leaves out.
#[must_use]
pub fn write_run(state: &GameState) -> Vec<u8> {
    encode(&state.snapshot(), &state.resume())
}

fn encode(snapshot: &Snapshot, resume: &Resume) -> Vec<u8> {
    // Plain integers, vectors and options, which JSON always accepts.
    let payload = serde_json::to_vec(&serde_json::json!({
        "snapshot": snapshot,
        "resume": resume,
    }))
    .unwrap_or_default();
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

/// Parses a save file. The snapshot still has to pass `GameState::restore`; see [`load`].
pub fn read(bytes: &[u8]) -> Result<Snapshot, SaveError> {
    read_run(bytes).map(|(snapshot, _)| snapshot)
}

/// Parses a save file into its snapshot and [`Resume`].
pub fn read_run(bytes: &[u8]) -> Result<(Snapshot, Resume), SaveError> {
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
    let mut value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|e| SaveError::Payload(e.to_string()))?;
    let part = |value: &mut serde_json::Value, key: &str| {
        value
            .get_mut(key)
            .map(serde_json::Value::take)
            .ok_or_else(|| SaveError::Payload(format!("missing `{key}`")))
    };
    let snapshot = serde_json::from_value(part(&mut value, "snapshot")?)
        .map_err(|e| SaveError::Payload(e.to_string()))?;
    let resume = serde_json::from_value(part(&mut value, "resume")?)
        .map_err(|e| SaveError::Payload(e.to_string()))?;
    Ok((snapshot, resume))
}

/// Parses a save file and rebuilds the state: [`read_run`], then [`GameState::restore_with`].
pub fn load(bytes: &[u8]) -> Result<GameState, SaveError> {
    let (snapshot, resume) = read_run(bytes)?;
    GameState::restore_with(&snapshot, &resume).map_err(SaveError::Restore)
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

    /// A run on open ground with a leader, stepped `steps` times, with floor records.
    fn real_run(seed: u32, steps: u8) -> GameState {
        use rescue_sim::command::{Command, Direction};
        use rescue_sim::floor::{Floor, HEIGHT, TERRAIN_FLOOR, WIDTH};
        use rescue_sim::state::{EntityInfo, Monster, MoveSlot};
        let mut floor = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                floor.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        let leader = Monster {
            pos: (5, 5),
            visible: true,
            info: EntityInfo {
                species: 1,
                apparent_species: 1,
                not_team_member: false,
                is_team_leader: true,
                level: 8,
                iq: 0,
                hp: 30,
                max_hp: 30,
                attack: [10, 10],
                defence: [5, 5],
                offensive_stages: [10, 10],
                defensive_stages: [10, 10],
                accuracy_stage: 10,
                evasion_stage: 10,
                types: [0, 0],
                abilities: [0, 0],
                moves: [MoveSlot::default(); 4],
                speed_up: [0; 5],
                speed_down: [0; 5],
                direction: 0,
                belly: 100_000,
                belly_empty: false,
                regen_acc: 0,
                status: Default::default(),
                regen_speed: 0,
                exp: 0,
                exp_gained: 0,
            },
        };
        let mut g = GameState::in_dungeon(seed, floor, vec![leader], 0)
            .unwrap()
            .with_floors(vec![std::array::from_fn(|i| i as u8); 2]);
        let dirs = [
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
        ];
        for i in 0..steps {
            g.step(Command::Move(dirs[usize::from(i) % 4]));
        }
        g
    }

    proptest! {
        #[test]
        fn real_run_round_trips(seed: u32, steps in 0u8..12) {
            let g = real_run(seed, steps);
            let snapshot = g.snapshot();
            prop_assert!(!snapshot.floors.is_empty() && snapshot.dungeon.is_some());
            let file = write_run(&g);
            prop_assert_eq!(&read(&file)?, &snapshot);
            let back = load(&file).unwrap();
            prop_assert_eq!(back.snapshot(), snapshot);
            prop_assert_eq!(back, g);
        }

        #[test]
        fn real_file_cut_anywhere_is_rejected(seed: u32, cut: usize) {
            let file = write(&real_run(seed, 4).snapshot());
            prop_assert!(read(&file[..cut % file.len()]).is_err());
        }

        #[test]
        fn real_file_flipped_bit_is_rejected(seed: u32, idx: usize, bit in 0u8..8) {
            let mut file = write(&real_run(seed, 4).snapshot());
            let i = idx % file.len();
            file[i] ^= 1 << bit;
            prop_assert!(read(&file).is_err());
        }
    }

    /// A run with a leader and a partner, wild monsters, a spawn list, a monster density,
    /// species sleep chances and statuses on the monsters. The sim's constructors for these
    /// take types this crate does not link, so the run is built through a snapshot and a
    /// [`Resume`], the two things a save holds.
    fn busy_run(seed: u32) -> GameState {
        use rescue_sim::floor::{Floor, HEIGHT, TERRAIN_FLOOR, WIDTH};
        use rescue_sim::status::{BURN, FOCUS_ENERGY, LEECH_SEED, PARALYSIS, SLEEP};
        let mut floor = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                floor.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        let leader = real_run(seed, 0).snapshot().dungeon.unwrap().team[0].clone();
        let mut partner = leader.clone();
        partner.pos = (4, 5);
        partner.info.is_team_leader = false;
        partner.info.species = 2;
        let mut leader = leader;
        leader.info.status.burn_class = BURN;
        leader.info.status.burn_turns = 6;
        leader.info.status.sure_shot = FOCUS_ENERGY;
        leader.info.status.sure_shot_turns = 9;
        let g = GameState::in_dungeon(seed, floor, vec![leader, partner], 4)
            .unwrap()
            .with_floors(vec![[0u8; 0x1C]; 3]);
        let mut snapshot = g.snapshot();
        let d = snapshot.dungeon.as_mut().unwrap();
        for (i, pos) in [(9, 5), (14, 12), (20, 8), (6, 15)].into_iter().enumerate() {
            let mut m = d.team[0].clone();
            m.pos = pos;
            m.info.is_team_leader = false;
            m.info.not_team_member = true;
            m.info.species = 3;
            m.info.belly = 0;
            match i % 3 {
                0 => {
                    m.info.status.sleep = SLEEP;
                    m.info.status.sleep_turns = 20;
                }
                1 => {
                    m.info.status.burn_class = PARALYSIS;
                    m.info.status.burn_turns = 12;
                }
                _ => {
                    m.info.status.leech_seed = LEECH_SEED;
                    m.info.status.leech_turns = 10;
                }
            }
            d.wild.push(m);
        }
        let mut resume = g.resume();
        resume.chance_asleep = vec![0, 0, 0, 50, 50];
        let dr = resume.dungeon.as_mut().unwrap();
        dr.spawns = vec![(3, 5, [10, 10])];
        dr.enemy_density = 12;
        GameState::restore_with(&snapshot, &resume).unwrap()
    }

    fn command(code: u8) -> rescue_sim::command::Command {
        use rescue_sim::command::{Command, Direction};
        let dir = [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ][usize::from(code >> 4) % 4];
        match code % 4 {
            0 => Command::Move(dir),
            1 => Command::Run(dir),
            2 => Command::Attack(dir),
            _ => Command::Wait,
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        /// Saving and loading after N steps, then M more, equals N + M steps without the save.
        /// The run has wild monsters, a partner, statuses and per-turn spawns; the check is
        /// the whole state and every step's outcome.
        #[test]
        fn save_and_load_continues_the_run(
            seed: u32,
            codes in proptest::collection::vec(any::<u8>(), 2..48),
            cut in any::<usize>(),
        ) {
            let n = cut % codes.len();
            let mut straight = busy_run(seed);
            let mut saved = busy_run(seed);
            for &c in &codes[..n] {
                let a = straight.step(command(c));
                let b = saved.step(command(c));
                prop_assert_eq!(a, b);
            }
            let mut resumed = load(&write_run(&saved)).unwrap();
            prop_assert_eq!(&resumed, &saved);
            for &c in &codes[n..] {
                let a = straight.step(command(c));
                let b = resumed.step(command(c));
                prop_assert_eq!(a, b);
                prop_assert_eq!(&straight, &resumed);
            }
            prop_assert_eq!(straight.snapshot(), resumed.snapshot());
        }
    }

    #[test]
    fn busy_run_has_wild_monsters_and_a_partner() {
        let d = busy_run(7).snapshot().dungeon.unwrap();
        assert_eq!(d.team.len(), 2);
        assert!(d.wild.len() >= 3);
    }

    #[test]
    fn a_v1_file_is_refused() {
        let mut file = write(&snap(1, 2, 3));
        file[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(read(&file), Err(SaveError::UnsupportedVersion(1)));
    }

    #[test]
    fn load_reports_a_payload_that_does_not_restore() {
        let mut snapshot = real_run(1, 2).snapshot();
        let file = write(&snapshot);
        assert!(load(&file).is_ok());
        snapshot.floors_entered = rescue_sim::state::MAX_RESTORED_FLOORS + 1;
        assert_eq!(
            load(&write(&snapshot)).err(),
            Some(SaveError::Restore(StateError::TooManyFloors(
                rescue_sim::state::MAX_RESTORED_FLOORS + 1
            )))
        );
        // A file that does not decode keeps its own error.
        assert!(matches!(load(&file[..4]), Err(SaveError::TooShort)));
    }
}
