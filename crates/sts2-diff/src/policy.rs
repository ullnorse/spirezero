//! The random play policy. It has its own tiny PRNG so it never touches the engine's streams.

/// `SplitMix64`: a 64-bit generator that is fine for choosing among a handful of actions.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    state: u64,
}

impl Policy {
    /// A policy seeded with `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniformly random index in `0..len`.
    ///
    /// # Panics
    /// If `len` is 0.
    pub fn pick(&mut self, len: usize) -> usize {
        assert!(len > 0, "nothing to pick from");
        let len = u64::try_from(len).expect("usize fits u64");
        usize::try_from(self.next_u64() % len).expect("below len")
    }
}
