//! The game's random number generators, bit-exact.
//!
//! Blue has two linear congruential generators that share one step,
//! `s = s * 0x5D58_8B65 + 1` (wrapping, 32 bits); both output the top 16 bits
//! of the new state. See `docs/re/rng.md` for the addresses, how each one is
//! seeded and which code uses it.
//!
//! - [`DungeonRng`] drives everything inside a dungeon floor. It is reseeded once
//!   per floor from [`DungeonPreseed`], which is seeded once per dungeon run.
//! - [`GlobalRng`] is used everywhere else, and also advances during dungeons
//!   (for example for cosmetic choices such as tile graphics variants).

/// Multiplier of the shared LCG step.
pub const LCG_MULTIPLIER: u32 = 0x5D58_8B65;

/// One step of the shared LCG.
#[must_use]
pub const fn lcg_step(state: u32) -> u32 {
    state.wrapping_mul(LCG_MULTIPLIER).wrapping_add(1)
}

/// Reduces a 16-bit random value to `0..cap` the way the game does: scale and
/// keep the top 16 bits of the product, masked to 16 bits.
const fn scale16(r16: u32, cap: i32) -> i32 {
    (((r16 & 0xFFFF) as i32).wrapping_mul(cap) >> 16) & 0xFFFF
}

/// Dungeon floor generator (`gDungeonRngState` in the game).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DungeonRng {
    state: u32,
}

impl DungeonRng {
    /// Seeds the generator for a floor. The state is forced odd.
    #[must_use]
    pub const fn new(floor_seed: u32) -> Self {
        Self {
            state: floor_seed | 1,
        }
    }

    /// Uses `state` as is (for resuming from a recorded state).
    #[must_use]
    pub const fn from_state(state: u32) -> Self {
        Self { state }
    }

    #[must_use]
    pub const fn state(&self) -> u32 {
        self.state
    }

    /// Next 16-bit output.
    pub fn next_u16(&mut self) -> u32 {
        self.state = lcg_step(self.state);
        self.state >> 16
    }

    /// Value in `0..cap` (0 when `cap` is 0).
    pub fn int(&mut self, cap: i32) -> i32 {
        scale16(self.next_u16(), cap)
    }

    /// Value between `low` and `high`, upper bound excluded; the bounds may be
    /// given in either order, and equal bounds return `low` without using the
    /// generator.
    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        if low == high {
            low
        } else if low < high {
            low + self.int(high - low)
        } else {
            high + self.int(low - high)
        }
    }

    /// True with `percent` % probability: `int(100) < percent`.
    pub fn outcome(&mut self, percent: i32) -> bool {
        self.int(100) < percent
    }
}

/// Once-per-run generator that produces each floor's seed
/// (`gDungeonRngPreseedState` in the game).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DungeonPreseed {
    state: u32,
}

impl DungeonPreseed {
    /// Seeds from the run's 24-bit preseed. The state is forced odd and kept to
    /// 24 bits.
    #[must_use]
    pub const fn new(run_preseed: u32) -> Self {
        Self {
            state: (run_preseed | 1) & 0x00FF_FFFF,
        }
    }

    #[must_use]
    pub const fn state(&self) -> u32 {
        self.state
    }

    /// Next floor seed: two steps, the top halves of both new states joined,
    /// cut to 24 bits and forced odd.
    pub fn next_floor_seed(&mut self) -> u32 {
        let first = lcg_step(self.state);
        let second = lcg_step(first);
        self.state = second;
        (((second & 0xFFFF_0000) | (first >> 16)) & 0x00FF_FFFF) | 1
    }
}

/// The global generator (`sPRNGState` in the game).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalRng {
    state: u32,
}

impl GlobalRng {
    /// Boot seed from six bytes (the console's MAC address in Blue):
    /// `54021 + b0*b1 + b2*b3 + b4*b5`.
    #[must_use]
    pub const fn from_seed_bytes(b: [u8; 6]) -> Self {
        let sum = 54021
            + b[0] as u32 * b[1] as u32
            + b[2] as u32 * b[3] as u32
            + b[4] as u32 * b[5] as u32;
        Self { state: sum }
    }

    /// Uses `state` as is (for resuming from a recorded state).
    #[must_use]
    pub const fn from_state(state: u32) -> Self {
        Self { state }
    }

    /// Sets the state, then advances it once through [`Self::next_u32`], as the
    /// game does when it restores the state from a save.
    #[must_use]
    pub fn reseeded(state: u32) -> Self {
        let mut rng = Self { state };
        rng.state = rng.next_u32();
        rng
    }

    #[must_use]
    pub const fn state(&self) -> u32 {
        self.state
    }

    /// Next 16-bit output, sign-extended like the game's `s16` return value.
    pub fn next_i16(&mut self) -> i32 {
        self.state = lcg_step(self.state);
        i32::from((self.state >> 16) as u16 as i16)
    }

    /// Two 16-bit outputs joined: `(first << 16) | second`. The second output is
    /// sign-extended before the OR, so when its top bit is set the result's top
    /// half is all ones. The game does this too.
    pub fn next_u32(&mut self) -> u32 {
        let first = self.next_i16();
        let second = self.next_i16();
        ((first << 16) | second) as u32
    }

    /// Value in `0..cap`, from the low half of [`Self::next_u32`].
    pub fn int(&mut self, cap: i32) -> i32 {
        scale16(self.next_u32(), cap)
    }

    /// Like [`DungeonRng::range`], with this generator.
    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        if low == high {
            low
        } else if low < high {
            low + self.int(high - low)
        } else {
            high + self.int(low - high)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vectors measured on the retail ROM by verification/v7-trace/retail_trace.py
    // (V7 in docs/verification.md): the first dungeon floor of a new game.
    const RUN_PRESEED: u32 = 0x00FF_80BA;
    const FLOOR_SEED: u32 = 0x00DA_4EFB;
    const PRESEED_AFTER_FLOOR_1: u32 = 0xBBDA_40E9;
    const DUNGEON_STATE_AT_FIRST_TURN: u32 = 2_384_006_743;
    const STEPS_FROM_FLOOR_SEED_TO_FIRST_TURN: usize = 1812;

    #[test]
    fn floor_seed_from_run_preseed() {
        let mut pre = DungeonPreseed::new(RUN_PRESEED);
        assert_eq!(pre.state(), 0x00FF_80BB);
        assert_eq!(pre.next_floor_seed(), FLOOR_SEED);
        assert_eq!(pre.state(), PRESEED_AFTER_FLOOR_1);
    }

    #[test]
    fn dungeon_rng_reaches_recorded_state() {
        let mut rng = DungeonRng::new(FLOOR_SEED);
        for _ in 0..STEPS_FROM_FLOOR_SEED_TO_FIRST_TURN {
            rng.next_u16();
        }
        assert_eq!(rng.state(), DUNGEON_STATE_AT_FIRST_TURN);
    }

    #[test]
    fn int_and_range() {
        let mut a = DungeonRng::from_state(0x1234_5678);
        let mut b = a;
        let r = b.next_u16();
        assert_eq!(a.int(100), (((r as i32) * 100) >> 16) & 0xFFFF);
        assert_eq!(a.state(), b.state());
        let mut c = DungeonRng::from_state(7);
        let before = c;
        assert_eq!(c.range(5, 5), 5);
        assert_eq!(c, before, "equal bounds do not use the generator");
        for _ in 0..1000 {
            let v = c.range(2, 9);
            assert!((2..9).contains(&v));
            let w = c.range(9, 2);
            assert!((2..9).contains(&w));
        }
    }

    #[test]
    fn global_rng_sign_extends_the_low_half() {
        // Find a state whose second output has its top bit set.
        let mut probe = GlobalRng::from_state(1);
        loop {
            let mut copy = probe;
            copy.next_i16();
            if copy.next_i16() < 0 {
                break;
            }
            probe.next_i16();
        }
        assert_eq!(probe.next_u32() >> 16, 0xFFFF);
    }

    #[test]
    fn global_seed_from_bytes() {
        let rng = GlobalRng::from_seed_bytes([0x00, 0x09, 0xBF, 0x12, 0x34, 0x56]);
        assert_eq!(rng.state(), 54021 + 0xBF * 0x12 + 0x34 * 0x56);
    }
}
