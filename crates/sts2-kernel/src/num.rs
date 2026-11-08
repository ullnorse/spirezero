//! Exact integer replacements for the `Decimal` arithmetic of damage and block.
//!
//! The engine computes a hit or a block gain as a C# `decimal`: an integer base, the additive
//! modifiers (integers), then each multiplicative factor in listener order (`Hook.ModifyDamage`,
//! `Hook.ModifyBlock`), then `max(0, x)`, and every reader of the result truncates it
//! ([`sts2_core::Decimal::to_int_truncating`]: block lost, HP lost, block gained). Every factor of
//! the supported content is a multiple of `0.01` (Vulnerable `1.5`, Weak and Frail `0.75`,
//! Shrink `0.7`, Slow `1 + 0.1n`), so the kernel carries it as an integer percentage.
//!
//! **When this equals `Decimal`.** A `decimal` product is exact when the exact product fits the
//! 96-bit mantissa at a scale of at most 28; it only rounds beyond that. The kernel's product of
//! `k` percentages is `base * p1 * .. * pk / 100^k`: exact in `i128`, at scale `2k`. So as long as
//! `|base * p1 * .. * pk| < 2^96` and `2k <= 28`, the `decimal` result is exact too and both
//! truncate to the same integer (truncation commutes with nothing else here: every later step reads
//! the truncated value, see `docs/kernel-prototype.md`). Outside that domain [`scaled_trunc`]
//! returns `None` and the kernel refuses the fight ([`crate::Status::Unsupported`]) instead of
//! guessing. With fights' magnitudes (bases below 10^4, at most five factors below 10^4) the bound
//! is never close; `tests` checks the claim against `Decimal` itself.

/// The largest mantissa a `decimal` holds: `2^96 - 1`.
const DECIMAL_MANTISSA: i128 = (1 << 96) - 1;
/// `decimal`'s largest scale.
const DECIMAL_SCALE: usize = 28;

/// At most this many factors per product (one per listener that can modify one value).
pub const MAX_FACTORS: usize = 8;

/// Percentages to multiply a base by, in listener order.
#[derive(Clone, Copy, Debug, Default)]
pub struct Factors {
    pcts: [u32; MAX_FACTORS],
    len: usize,
}

impl Factors {
    /// No factors.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pcts: [0; MAX_FACTORS],
            len: 0,
        }
    }

    /// Appends `pct` (the factor times 100). Returns `false` if the list is full.
    pub const fn push(&mut self, pct: u32) -> bool {
        if self.len == MAX_FACTORS {
            return false;
        }
        self.pcts[self.len] = pct;
        self.len += 1;
        true
    }

    /// The factors, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[u32] {
        &self.pcts[..self.len]
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// `trunc(max(0, base * f1 * .. * fk))` with each `fi = pct / 100`, exactly; `None` outside the
/// domain where it provably equals the engine's `Decimal` result (see the module docs).
#[must_use]
pub fn scaled_trunc(base: i64, factors: &Factors) -> Option<i32> {
    let pcts = factors.as_slice();
    if base <= 0 {
        // `max(0, x)` of a non-positive base times positive factors.
        return (2 * pcts.len() <= DECIMAL_SCALE).then_some(0);
    }
    if 2 * pcts.len() > DECIMAL_SCALE {
        return None;
    }
    let mut num = i128::from(base);
    let mut den: i128 = 1;
    for &p in pcts {
        num = num.checked_mul(i128::from(p))?;
        if num > DECIMAL_MANTISSA {
            return None;
        }
        den *= 100;
    }
    i32::try_from(num / den).ok()
}

#[cfg(test)]
mod tests {
    use super::{Factors, scaled_trunc};
    use sts2_core::Decimal;

    /// The engine's own computation: the base, then each factor as the `Decimal` the power uses,
    /// in order, then `max(0, x)` and truncation.
    fn decimal(base: i32, factors: &[Decimal]) -> i32 {
        let mut d = Decimal::from(base);
        for &f in factors {
            d *= f;
        }
        d.max(Decimal::ZERO).to_int_truncating()
    }

    /// The `Decimal` each supported factor is built as (`VulnerablePower`, `WeakPower`,
    /// `FrailPower`, `ShrinkPower`, `SlowPower`).
    fn factor(which: u32, slow: i32) -> (u32, Decimal) {
        match which % 4 {
            0 => (150, Decimal::new(15, 1)),
            // Weak and Frail.
            1 => (75, Decimal::new(75, 2)),
            2 => (70, Decimal::new(7, 1)),
            _ => {
                let pct = 100 + 10 * u32::try_from(slow).expect("non-negative");
                (pct, Decimal::ONE + Decimal::new(1, 1) * Decimal::from(slow))
            }
        }
    }

    #[test]
    fn fixed_point_matches_decimal_on_every_small_product() {
        // Every base in -20..2000 against every ordered pair and triple of factor kinds, with
        // Slow counts 0..12: the kernel's integer result equals the engine's `Decimal` one.
        let mut checked = 0_u32;
        for base in -20..2000 {
            for a in 0..5 {
                for b in 0..5 {
                    for slow in [0, 1, 2, 3, 7, 12] {
                        for len in 0..=3 {
                            let picks = [a, b, a + b + 1];
                            let mut f = Factors::new();
                            let mut d = Vec::new();
                            for &w in &picks[..len] {
                                let (pct, dec) = factor(w, slow);
                                assert!(f.push(pct));
                                d.push(dec);
                            }
                            assert_eq!(
                                scaled_trunc(i64::from(base), &f),
                                Some(decimal(base, &d)),
                                "base {base}, factors {:?}",
                                f.as_slice()
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 400_000);
    }

    #[test]
    fn refuses_outside_the_exact_domain() {
        let mut f = Factors::new();
        for _ in 0..8 {
            assert!(f.push(60_000));
        }
        assert_eq!(scaled_trunc(1_000_000_000, &f), None);
        assert!(!f.push(100));
    }
}
