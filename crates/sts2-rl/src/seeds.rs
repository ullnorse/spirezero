//! The run env's episode seeds: a list, a range, or random from a master seed. Run seed `s`
//! is the game seed string `RUN<s>` (as the run bench's), played by [`RunConfig`]'s character.
//!
//! [`RunConfig`]: crate::run_envs::RunConfig

/// `SplitMix64`: the env's own RNG (never one of the engine's streams).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitMix(u64);

impl SplitMix {
    /// A generator whose first output is `SplitMix64(seed)`'s.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next output.
    pub const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform-ish index below `len` (the output modulo `len`).
    ///
    /// # Panics
    /// If `len` is 0.
    pub fn below(&mut self, len: usize) -> usize {
        let len = u64::try_from(len).expect("usize fits u64");
        usize::try_from(self.next() % len).expect("below len")
    }
}

/// Where the run env's episodes come from: episode `k` (in the order the batch hands them out)
/// plays the seed [`SeedSource::seed`] gives for `k`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedSource {
    /// These seeds in order; once through unless `cycle`.
    List { seeds: Vec<u64>, cycle: bool },
    /// `start..end` in order; once through unless `cycle`.
    Range { start: u64, end: u64, cycle: bool },
    /// Endless: episode `k` plays `SplitMix64(master + k)`'s first output.
    Random { master: u64 },
}

impl SeedSource {
    /// The seed of episode `k`; `None` once a finite source is used up.
    #[must_use]
    pub fn seed(&self, k: u64) -> Option<u64> {
        match self {
            Self::List { seeds, cycle } => {
                let len = u64::try_from(seeds.len()).unwrap_or(u64::MAX);
                if len == 0 || (!cycle && k >= len) {
                    return None;
                }
                seeds.get(usize::try_from(k % len).ok()?).copied()
            }
            Self::Range { start, end, cycle } => {
                let len = end.saturating_sub(*start);
                if len == 0 || (!cycle && k >= len) {
                    return None;
                }
                Some(start + k % len)
            }
            Self::Random { master } => Some(SplitMix::new(master.wrapping_add(k)).next()),
        }
    }

    /// Whether the source has at least one seed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seed(0).is_none()
    }
}
