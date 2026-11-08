//! Exact emulation of C# `decimal` for the values game logic produces.
//!
//! A value is `mantissa / 10^scale` with a mantissa of at most 96 bits and a scale of at
//! most 28, like .NET. Values are kept normalized (no trailing zeros in the mantissa while
//! `scale > 0`); .NET keeps trailing zeros, but they only affect `ToString`, never results.
//!
//! .NET rounds when an exact result needs more than 96 bits or 28 fractional digits, and
//! throws on overflow. Game values never get there, so instead of emulating that rounding
//! these operations panic whenever the exact result is not representable.

#![expect(
    clippy::as_conversions,
    reason = "the 96-bit arithmetic's widening and narrowing casts, in const fns where `From` is not const"
)]

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};

/// Largest mantissa magnitude: 2^96 - 1.
const MAX_MANTISSA: i128 = (1 << 96) - 1;
/// Largest scale .NET allows.
const MAX_SCALE: u8 = 28;

/// A C# `decimal`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal {
    mantissa: i128,
    scale: u8,
}

impl Decimal {
    /// `0m`.
    pub const ZERO: Self = Self::new(0, 0);
    /// `1m`.
    pub const ONE: Self = Self::new(1, 0);
    /// `decimal.MaxValue` (2^96 - 1), used by the game as a "no cap" sentinel.
    pub const MAX: Self = Self::new(MAX_MANTISSA, 0);
    /// `decimal.MinValue`.
    pub const MIN: Self = Self::new(-MAX_MANTISSA, 0);

    /// `mantissa / 10^scale`, the value of the literal with those digits:
    /// `Decimal::new(75, 2)` is `0.75m`.
    ///
    /// # Panics
    /// If the mantissa needs more than 96 bits or `scale > 28`.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "const fn: the scale only shrinks from a `u8`"
    )]
    pub const fn new(mantissa: i128, scale: u8) -> Self {
        assert!(
            mantissa.unsigned_abs() <= MAX_MANTISSA.unsigned_abs() && scale <= MAX_SCALE,
            "Decimal::new: not a valid decimal"
        );
        let (mantissa, scale) = normalize(mantissa, scale as u32);
        Self {
            mantissa,
            scale: scale as u8,
        }
    }

    /// Builds the exact result of an operation, panicking if .NET would have to round or
    /// overflow.
    fn exact(mantissa: Option<i128>, scale: u32, op: &str) -> Self {
        let mantissa = mantissa.unwrap_or_else(|| panic!("Decimal {op}: result not representable"));
        let (mantissa, scale) = normalize(mantissa, scale);
        assert!(
            mantissa.unsigned_abs() <= MAX_MANTISSA.unsigned_abs() && scale <= u32::from(MAX_SCALE),
            "Decimal {op}: result not representable"
        );
        Self {
            mantissa,
            scale: u8::try_from(scale).expect("scale <= 28"),
        }
    }

    /// The `(int)` cast: truncates toward zero.
    ///
    /// # Panics
    /// If the truncated value does not fit `i32` (.NET throws `OverflowException`).
    #[must_use]
    #[inline]
    pub fn to_int_truncating(self) -> i32 {
        if self.scale == 0
            && let Ok(value) = i32::try_from(self.mantissa)
        {
            return value;
        }
        let truncated = self.truncate().mantissa;
        i32::try_from(truncated).expect("Decimal::to_int_truncating: OverflowException")
    }

    /// `decimal.Truncate`: the integral part, toward zero.
    #[must_use]
    pub fn truncate(self) -> Self {
        // Game values are nearly always integral; skip the `i128` division.
        if self.scale == 0 {
            return self;
        }
        // An `i64` division when the value allows it (an `i128` one is a library call).
        if let Ok(m) = i64::try_from(self.mantissa)
            && self.scale <= 18
        {
            let unit = 10_i64.pow(u32::from(self.scale));
            return Self::new(i128::from(m / unit), 0);
        }
        Self::new(self.mantissa / pow10(u32::from(self.scale)), 0)
    }

    /// `Math.Ceiling`: the smallest integral value not below `self`.
    #[must_use]
    pub fn ceiling(self) -> Self {
        let truncated = self.truncate();
        if self > truncated {
            truncated + Self::ONE
        } else {
            truncated
        }
    }

    /// Both mantissas at a common scale, or `None` if one does not fit `i128`.
    fn aligned(self, other: Self) -> (Option<i128>, Option<i128>, u8) {
        if self.scale == other.scale {
            return (Some(self.mantissa), Some(other.mantissa), self.scale);
        }
        let scale = self.scale.max(other.scale);
        let lift = |d: Self| d.mantissa.checked_mul(pow10(u32::from(scale - d.scale)));
        (lift(self), lift(other), scale)
    }
}

/// `mantissa / 10^scale` without trailing zeros in the mantissa while `scale > 0`, dividing
/// in `i64` when the mantissa fits one (an `i128` remainder is a library call).
#[expect(
    clippy::cast_possible_truncation,
    reason = "const fn: the `i64` casts are range-checked first"
)]
const fn normalize(mantissa: i128, mut scale: u32) -> (i128, u32) {
    if scale == 0 {
        return (mantissa, scale);
    }
    if mantissa >= i64::MIN as i128 && mantissa <= i64::MAX as i128 {
        let mut m = mantissa as i64;
        while scale > 0 && m % 10 == 0 {
            m /= 10;
            scale -= 1;
        }
        return (m as i128, scale);
    }
    let mut m = mantissa;
    while scale > 0 && m % 10 == 0 {
        m /= 10;
        scale -= 1;
    }
    (m, scale)
}

/// `10^exp` for `exp <= 38`.
const fn pow10(exp: u32) -> i128 {
    10_i128.pow(exp)
}

/// Removes up to `limit` factors of `factor` from `value`; returns the new value and the count.
const fn strip_factor(mut value: i128, factor: i128, limit: u32) -> (i128, u32) {
    let mut count = 0;
    while count < limit && value != 0 && value % factor == 0 {
        value /= factor;
        count += 1;
    }
    (value, count)
}

impl Decimal {
    /// `value` as a decimal: [`From<i32>`](Decimal#impl-From<i32>-for-Decimal) in a const
    /// context.
    #[must_use]
    pub const fn from_i32(value: i32) -> Self {
        Self::new(value as i128, 0)
    }
}

impl From<i32> for Decimal {
    #[inline]
    fn from(value: i32) -> Self {
        Self::new(i128::from(value), 0)
    }
}

impl From<i64> for Decimal {
    fn from(value: i64) -> Self {
        Self::new(i128::from(value), 0)
    }
}

/// The integral fast paths of the operators below are inlined; the general cases (other scales,
/// overflow) are calls.
impl Decimal {
    /// `mantissa` at scale 0, if representable (else the general path decides: it panics).
    #[inline]
    const fn integral(mantissa: Option<i128>) -> Option<Self> {
        match mantissa {
            Some(mantissa) if mantissa.unsigned_abs() <= MAX_MANTISSA.unsigned_abs() => {
                Some(Self { mantissa, scale: 0 })
            }
            _ => None,
        }
    }

    #[inline(never)]
    fn cmp_general(self, other: Self) -> Ordering {
        match self.aligned(other) {
            (Some(a), Some(b), _) => a.cmp(&b),
            // A lifted mantissa beyond i128 dwarfs any 96-bit one, so its sign decides.
            (None, _, _) => self.mantissa.cmp(&0),
            (_, None, _) => 0.cmp(&other.mantissa),
        }
    }

    #[inline(never)]
    fn add_general(self, rhs: Self) -> Self {
        let (a, b, scale) = self.aligned(rhs);
        let sum = a.zip(b).and_then(|(a, b)| a.checked_add(b));
        Self::exact(sum, u32::from(scale), "add")
    }

    #[inline(never)]
    fn sub_general(self, rhs: Self) -> Self {
        let (a, b, scale) = self.aligned(rhs);
        let difference = a.zip(b).and_then(|(a, b)| a.checked_sub(b));
        Self::exact(difference, u32::from(scale), "sub")
    }

    #[inline(never)]
    fn mul_general(self, rhs: Self) -> Self {
        if self.scale == 0 && rhs.scale == 0 {
            return Self::exact(self.mantissa.checked_mul(rhs.mantissa), 0, "mul");
        }
        // Small operands (game values): the exact product fits an `i64`, and normalizing it
        // gives the same value as the general path below.
        if let (Ok(a), Ok(b)) = (i32::try_from(self.mantissa), i32::try_from(rhs.mantissa)) {
            let product = i128::from(i64::from(a) * i64::from(b));
            let scale = u32::from(self.scale) + u32::from(rhs.scale);
            return Self::exact(Some(product), scale, "mul");
        }
        self.mul_cancelling(rhs)
    }
}

impl Ord for Decimal {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        if self.scale == other.scale {
            self.mantissa.cmp(&other.mantissa)
        } else {
            self.cmp_general(*other)
        }
    }
}

impl PartialOrd for Decimal {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for Decimal {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        if self.scale == 0
            && rhs.scale == 0
            && let Some(sum) = Self::integral(self.mantissa.checked_add(rhs.mantissa))
        {
            return sum;
        }
        self.add_general(rhs)
    }
}

impl Sub for Decimal {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        if self.scale == 0
            && rhs.scale == 0
            && let Some(difference) = Self::integral(self.mantissa.checked_sub(rhs.mantissa))
        {
            return difference;
        }
        self.sub_general(rhs)
    }
}

impl Mul for Decimal {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        if self.scale == 0
            && rhs.scale == 0
            && let Some(product) = Self::integral(self.mantissa.checked_mul(rhs.mantissa))
        {
            return product;
        }
        self.mul_general(rhs)
    }
}

impl Decimal {
    /// The general product ([`Mul`]): exact for any operands whose product is representable.
    fn mul_cancelling(self, rhs: Self) -> Self {
        // Cancel the product's trailing zeros (pairs of 2s and 5s, as far as the scale
        // allows) before multiplying, so an exact result that fits is never lost to i128
        // overflow.
        let scale = u32::from(self.scale) + u32::from(rhs.scale);
        let (a, a_twos) = strip_factor(self.mantissa, 2, scale);
        let (b, b_twos) = strip_factor(rhs.mantissa, 2, scale - a_twos);
        let twos = a_twos + b_twos;
        let (a, a_fives) = strip_factor(a, 5, twos);
        let (b, b_fives) = strip_factor(b, 5, twos - a_fives);
        let tens = a_fives + b_fives;
        // Put back the 2s that found no matching 5.
        let product = a
            .checked_mul(b)
            .zip(2_i128.checked_pow(twos - tens))
            .and_then(|(product, twos)| product.checked_mul(twos));
        Self::exact(product, scale - tens, "mul")
    }
}

impl AddAssign for Decimal {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Decimal {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Decimal {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

/// A [`Decimal`] packed into 16 bytes at 4-byte alignment, for state that is copied often.
///
/// It uses the C# layout (96-bit magnitude, then sign and scale). A [`Decimal`] takes 32 bytes
/// at 16-byte alignment (its `i128`), which pads every struct holding one. The conversions are
/// exact both ways; compute on the [`Decimal`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedDecimal {
    /// The magnitude's bits 0-31, 32-63 and 64-95.
    magnitude: [u32; 3],
    /// The scale, plus the sign in bit 31.
    flags: u32,
}

impl PackedDecimal {
    /// `0m`.
    pub const ZERO: Self = Self::pack(Decimal::ZERO);

    /// `value`, packed.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "each word takes 32 bits of a magnitude of at most 96"
    )]
    pub const fn pack(value: Decimal) -> Self {
        let m = value.mantissa.unsigned_abs();
        debug_assert!(
            m <= MAX_MANTISSA.unsigned_abs(),
            "PackedDecimal::pack: magnitude over 96 bits"
        );
        let sign = if value.mantissa < 0 { 1 << 31 } else { 0 };
        Self {
            magnitude: [m as u32, (m >> 32) as u32, (m >> 64) as u32],
            flags: value.scale as u32 | sign,
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> Decimal {
        let [lo, mid, hi] = self.magnitude;
        let magnitude = (lo as i128) | ((mid as i128) << 32) | ((hi as i128) << 64);
        Decimal {
            mantissa: if self.flags >> 31 == 1 {
                -magnitude
            } else {
                magnitude
            },
            scale: (self.flags & 0xFF) as u8,
        }
    }
}

impl From<Decimal> for PackedDecimal {
    fn from(value: Decimal) -> Self {
        Self::pack(value)
    }
}

impl From<PackedDecimal> for Decimal {
    fn from(value: PackedDecimal) -> Self {
        value.get()
    }
}

impl fmt::Debug for PackedDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.get(), f)
    }
}

impl fmt::Debug for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Plain decimal notation of the normalized value (no trailing zeros).
impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = pow10(u32::from(self.scale));
        let sign = if self.mantissa < 0 { "-" } else { "" };
        let magnitude = self.mantissa.unsigned_abs();
        let unit = unit.unsigned_abs();
        write!(f, "{sign}{}", magnitude / unit)?;
        if self.scale > 0 {
            let width = usize::from(self.scale);
            write!(f, ".{:0width$}", magnitude % unit)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Decimal, MAX_MANTISSA, PackedDecimal};

    /// The small-operand product and the `i64` normalization agree with the general path.
    #[test]
    fn fast_paths_match_the_general_ones() {
        let values: Vec<Decimal> = (0..=4)
            .flat_map(|scale| {
                (-1500..=1500)
                    .step_by(7)
                    .chain([i32::MAX.into(), i32::MIN.into(), 1 << 40])
                    .map(move |m: i128| Decimal::new(m, scale))
            })
            .collect();
        for &a in &values {
            for &b in values.iter().step_by(13) {
                assert_eq!(a * b, a.mul_cancelling(b), "{a} * {b}");
            }
            let reference = {
                let (mut m, mut scale) = (a.mantissa, a.scale);
                while scale > 0 && m % 10 == 0 {
                    m /= 10;
                    scale -= 1;
                }
                (m, scale)
            };
            assert_eq!((a.mantissa, a.scale), reference);
            let unit = 10_i128.pow(u32::from(a.scale));
            assert_eq!(a.truncate(), Decimal::new(a.mantissa / unit, 0), "{a}");
        }
    }

    #[test]
    fn packing_is_exact() {
        let values = [
            Decimal::ZERO,
            Decimal::ONE,
            Decimal::new(-1, 0),
            Decimal::new(75, 2),
            Decimal::new(-12_345, 3),
            Decimal::MAX,
            Decimal::MIN,
            Decimal::new(1, 28),
            Decimal::new(-(1 << 70), 5),
            Decimal::new(MAX_MANTISSA, 28),
            Decimal::new(-MAX_MANTISSA, 28),
            Decimal::new(1 << 95, 0),
            Decimal::new(-(1 << 95), 1),
            Decimal::new(i128::from(u64::MAX), 0),
        ];
        for v in values {
            assert_eq!(PackedDecimal::pack(v).get(), v);
            assert_eq!(
                PackedDecimal::pack(v) == PackedDecimal::pack(Decimal::ONE),
                v == Decimal::ONE
            );
        }
        assert_eq!(size_of::<PackedDecimal>(), 16);
        assert_eq!(align_of::<PackedDecimal>(), 4);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "magnitude over 96 bits")]
    fn packing_more_than_96_bits_asserts() {
        // Only the private fields can build one; every constructor checks the range.
        let _ = PackedDecimal::pack(Decimal {
            mantissa: -(1 << 96),
            scale: 0,
        });
    }
}
