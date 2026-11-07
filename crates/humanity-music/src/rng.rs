//! The composer's and the voices' randomness: a seeded generator, so the same seed composes the
//! same music note for note (tests, rigs and `HUMANITY_MUSIC_SEED`), and nothing allocates.
//!
//! SplitMix64 (Steele, Lea and Flood, "Fast splittable pseudorandom number generators", OOPSLA
//! 2014): one add and three multiply-xorshift steps per number, every 64-bit seed a full-period
//! stream. Not cryptographic, and never used for anything but music.

/// A seeded random stream.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A stream from `seed` (any value, zero included).
    pub fn new(seed: u64) -> Self {
        Rng { state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    /// The next 64 random bits.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in [0, 1).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number in [-1, 1): white noise.
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        self.f32() * 2.0 - 1.0
    }

    /// A number in [lo, hi).
    #[inline]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// An integer in [lo, hi] (both included; `hi < lo` gives `lo`).
    #[inline]
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as i32
    }

    /// True with probability `p`.
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// An index chosen by weight (weights at or under zero are never chosen; all of them so gives 0).
    pub fn weighted(&mut self, weights: impl Iterator<Item = f32> + Clone) -> usize {
        let total: f32 = weights.clone().map(|w| w.max(0.0)).sum();
        if total <= 0.0 {
            return 0;
        }
        let mut x = self.f32() * total;
        let mut last = 0;
        for (i, w) in weights.enumerate() {
            let w = w.max(0.0);
            if w <= 0.0 {
                continue;
            }
            last = i;
            if x < w {
                return i;
            }
            x -= w;
        }
        last
    }

    /// A new independent stream derived from this one (each voice gets its own noise).
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }
}
