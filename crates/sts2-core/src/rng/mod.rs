//! Bit-exact port of the game's `Rng` (`MegaCrit.Sts2.Core.Random/Rng.cs`) and its
//! xoshiro256** core (`MegaRandom.cs`), plus seed derivation and the run/player stream sets.
//!
//! This is the only module allowed to use floating point: every output goes through the
//! game's `NextDouble()`, and IEEE-754 `f64`/`f32` arithmetic and casts in Rust match .NET.
#![allow(
    clippy::float_arithmetic,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "ports the game's f64/f32 RNG arithmetic and its unchecked C# casts exactly"
)]
#![allow(
    clippy::suboptimal_flops,
    reason = "the game computes `a * b + c` unfused; `mul_add` would round differently"
)]

mod grab_bag;
mod seed;
mod sets;

pub use grab_bag::GrabBag;
pub use seed::{canonicalize_seed, snake_case, xxh64};
pub use sets::{PlayerRngSet, PlayerRngType, RunRngSet, RunRngType};

/// 2^-53: scales the top 53 bits of a raw draw into `[0, 1)` (`MegaRandom.NextDouble`).
const DOUBLE_UNIT: f64 = 1.110_223_024_625_156_5E-16;

/// Saved RNG state, mirroring the game's `SerializableRng`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RngState {
    /// Number of public calls made so far; bookkeeping only, never affects outputs.
    pub counter: i32,
    /// The four xoshiro256** state words `s0..s3`.
    pub s: [u64; 4],
}

/// The game's seeded random number generator: xoshiro256** plus a call counter.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Rng {
    counter: i32,
    s: [u64; 4],
}

impl_clone_by_fields!(Rng { counter, s });

/// One splitmix64 step, used to expand a seed into the xoshiro state.
const fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// `new Rng(seed)`: seeds the state with four splitmix64 steps.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        let mut x = seed;
        let s = [
            splitmix64(&mut x),
            splitmix64(&mut x),
            splitmix64(&mut x),
            splitmix64(&mut x),
        ];
        Self { counter: 0, s }
    }

    /// `new Rng(seed, name)`: a named stream seeded with `seed + xxh64(name)`.
    #[must_use]
    pub fn named(seed: u64, name: &str) -> Self {
        Self::new(seed.wrapping_add(xxh64(name)))
    }

    /// Restores a saved state verbatim (`new Rng(SerializableRng)`).
    #[must_use]
    pub const fn from_state(state: RngState) -> Self {
        Self {
            counter: state.counter,
            s: state.s,
        }
    }

    /// The current state, as the game would save it (`ToSerializable`).
    #[must_use]
    pub const fn state(&self) -> RngState {
        RngState {
            counter: self.counter,
            s: self.s,
        }
    }

    /// Number of public calls made so far (wraps like a C# `int`).
    #[must_use]
    pub const fn counter(&self) -> i32 {
        self.counter
    }

    const fn bump(&mut self) {
        self.counter = self.counter.wrapping_add(1);
    }

    /// One raw xoshiro256** output.
    const fn raw_u64(&mut self) -> u64 {
        let [s0, s1, s2, s3] = &mut self.s;
        let result = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = *s1 << 17;
        *s2 ^= *s0;
        *s3 ^= *s1;
        *s1 ^= *s2;
        *s0 ^= *s3;
        *s2 ^= t;
        *s3 = s3.rotate_left(45);
        result
    }

    /// One raw draw scaled to `[0, 1)`, without touching the counter.
    fn raw_double(&mut self) -> f64 {
        (self.raw_u64() >> 11) as f64 * DOUBLE_UNIT
    }

    /// `NextBool()`: `true` with probability 1/2 (`NextInt(2) == 0`).
    pub fn next_bool(&mut self) -> bool {
        self.next_int(2) == 0
    }

    /// `NextInt(max)`: an integer in `[0, max)`. The game's default `max` is `i32::MAX`.
    ///
    /// # Panics
    /// If `max < 1`, like the game's `ArgumentOutOfRangeException`; the counter is still
    /// incremented first, matching the game.
    pub fn next_int(&mut self, max: i32) -> i32 {
        self.bump();
        assert!(max >= 1, "Rng::next_int: max must be > 0, got {max}");
        (self.raw_double() * f64::from(max)) as i32
    }

    /// `NextInt(min, max)`: an integer in `[min, max)`.
    ///
    /// # Panics
    /// If `min >= max` (before the counter is touched, like the game).
    pub fn next_int_range(&mut self, min: i32, max: i32) -> i32 {
        assert!(min < max, "Rng::next_int_range: min {min} >= max {max}");
        self.bump();
        let span = i64::from(max) - i64::from(min);
        if span <= i64::from(i32::MAX) {
            (self.raw_double() * span as f64) as i32 + min
        } else {
            ((self.raw_double() * span as f64) as i64 + i64::from(min)) as i32
        }
    }

    /// `NextUnsignedInt(max)`: an integer in `[0, max)`. The game's default `max` is `u32::MAX`.
    ///
    /// # Panics
    /// If `max == 0`.
    pub fn next_unsigned_int(&mut self, max: u32) -> u32 {
        self.next_unsigned_int_range(0, max)
    }

    /// `NextUnsignedInt(min, max)`: an integer in `[min, max)`.
    ///
    /// # Panics
    /// If `min >= max` (before the counter is touched).
    pub fn next_unsigned_int_range(&mut self, min: u32, max: u32) -> u32 {
        assert!(
            min < max,
            "Rng::next_unsigned_int_range: min {min} >= max {max}"
        );
        self.bump();
        min + (self.raw_double() * f64::from(max - min)) as u32
    }

    /// `NextUnsignedLong()`: one raw 64-bit output.
    pub const fn next_unsigned_long(&mut self) -> u64 {
        self.bump();
        self.raw_u64()
    }

    /// `NextUnsignedLong(max)`: an integer in `[0, max)`, or a raw output if `max == u64::MAX`.
    ///
    /// # Panics
    /// If `max == 0`.
    pub fn next_unsigned_long_max(&mut self, max: u64) -> u64 {
        if max == u64::MAX {
            self.next_unsigned_long()
        } else {
            self.next_unsigned_long_range(0, max)
        }
    }

    /// `NextUnsignedLong(min, max)`: an integer in `[min, max)` (through `f64`, so imprecise
    /// above 2^53, exactly like the game).
    ///
    /// # Panics
    /// If `min >= max` (before the counter is touched).
    pub fn next_unsigned_long_range(&mut self, min: u64, max: u64) -> u64 {
        assert!(
            min < max,
            "Rng::next_unsigned_long_range: min {min} >= max {max}"
        );
        self.bump();
        min + (self.raw_double() * (max - min) as f64) as u64
    }

    /// `NextFloat(max)`: a float in `[0, max)`. The game's default `max` is `1.0`.
    ///
    /// # Panics
    /// If `max < 0`.
    pub fn next_float(&mut self, max: f32) -> f32 {
        self.next_float_range(0.0, max)
    }

    /// `NextFloat(min, max)`: `(float)(NextDouble() * (double)(max - min) + (double)min)`,
    /// where `max - min` is an `f32` subtraction.
    ///
    /// # Panics
    /// If `min > max` (before the counter is touched).
    pub fn next_float_range(&mut self, min: f32, max: f32) -> f32 {
        assert!(min <= max, "Rng::next_float_range: min {min} > max {max}");
        self.bump();
        (self.raw_double() * f64::from(max - min) + f64::from(min)) as f32
    }

    /// `NextDouble()`: a double in `[0, 1)`.
    pub fn next_double(&mut self) -> f64 {
        self.bump();
        self.raw_double()
    }

    /// `NextDouble(min, max)`: `NextDouble() * (max - min) + min`.
    ///
    /// # Panics
    /// If `min > max` (before the counter is touched).
    pub fn next_double_range(&mut self, min: f64, max: f64) -> f64 {
        assert!(min <= max, "Rng::next_double_range: min {min} > max {max}");
        self.bump();
        self.raw_double() * (max - min) + min
    }

    /// `NextGaussianInt(mean, std_dev, min, max)` (`Rng.cs:260-281`): Box-Muller from two
    /// `NextDouble()` draws, rounded half to even, re-drawn until the result is in `[min, max]`.
    /// `Math.Sqrt/Log/Sin` are the host libm's, so the last bit of the intermediate may differ
    /// across platforms (R2); the rounded integer practically never does.
    ///
    /// # Panics
    /// If `min > max` or `mean` is outside `[min, max]`, like the game's exceptions.
    pub fn next_gaussian_int(&mut self, mean: i32, std_dev: i32, min: i32, max: i32) -> i32 {
        assert!(min <= max, "Rng::next_gaussian_int: min {min} > max {max}");
        assert!(
            (min..=max).contains(&mean),
            "Rng::next_gaussian_int: mean {mean} outside [{min}, {max}]"
        );
        loop {
            let d = 1.0 - self.next_double();
            let u = 1.0 - self.next_double();
            let z = (-2.0 * d.ln()).sqrt() * (std::f64::consts::TAU * u).sin();
            let n = (f64::from(mean) + f64::from(std_dev) * z).round_ties_even() as i32;
            if (min..=max).contains(&n) {
                return n;
            }
        }
    }

    /// `NextItem(items)`: a uniformly chosen item, or `None` (with no draw) if `items` is empty.
    ///
    /// # Panics
    /// If `items` has more than `i32::MAX` elements.
    pub fn next_item<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let len = i32::try_from(items.len()).expect("Rng::next_item: more than i32::MAX items");
        let index = self.next_int_range(0, len);
        items.get(index as usize)
    }

    /// `WeightedNextItem(items, weight)`: draws `NextFloat()` first, then picks with
    /// [`weighted_pick`]. Returns `None` where the game returns `default`.
    pub fn weighted_next_item<'a, T>(
        &mut self,
        items: &'a [T],
        weight: impl Fn(&T) -> f32,
    ) -> Option<&'a T> {
        let roll = self.next_float(1.0);
        weighted_pick(roll, items, weight)
    }

    /// `RandomBranchState.GetNextState` (`RandomBranchState.cs:115-127`): `max` is the LINQ
    /// `Sum` of the weights (accumulated in `f64`, see [`weighted_pick`]), `num = NextFloat(max)`,
    /// then subtract each weight in `f32` and pick the first index where `num <= 0`. Zero
    /// weights take part, so a zero-weight branch can still be picked when `num` is already 0.
    /// Returns `None` where the game throws.
    pub fn random_branch(&mut self, weights: &[f32]) -> Option<usize> {
        let max = weights.iter().fold(0.0, |sum, &w| sum + f64::from(w)) as f32;
        let mut num = self.next_float(max);
        weights.iter().position(|&w| {
            num -= w;
            num <= 0.0
        })
    }

    /// `Rng.Shuffle(list)`: in-place Fisher-Yates from the end; identical to
    /// [`crate::shuffle::unstable_shuffle`].
    pub fn shuffle<T>(&mut self, list: &mut [T]) {
        crate::shuffle::unstable_shuffle(list, self);
    }
}

/// Static `Rng.WeightedNextItem(randInput, items, weight, fallback)`.
///
/// Scales `roll` by the total weight, then subtracts weights in order until the remainder
/// is `<= 0`. Returns `None` where the game returns `fallback`.
///
/// The total uses LINQ's `Sum(Func<T, float>)`, which accumulates in `f64` and rounds
/// to `f32` once (UNVERIFIED: `rng.json` gives the same picks with an `f32` sum); the rest
/// is `f32` arithmetic.
pub fn weighted_pick<T>(roll: f32, items: &[T], weight: impl Fn(&T) -> f32) -> Option<&T> {
    let total = items
        .iter()
        .fold(0.0, |sum, item| sum + f64::from(weight(item))) as f32;
    let mut remaining = roll * total;
    items.iter().find(|item| {
        remaining -= weight(item);
        remaining <= 0.0
    })
}
