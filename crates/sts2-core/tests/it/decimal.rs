//! `Decimal`: exact arithmetic on the values the damage pipeline produces.

use sts2_core::Decimal;

const fn d(mantissa: i128, scale: u8) -> Decimal {
    Decimal::new(mantissa, scale)
}

#[test]
fn literals_and_ints() {
    assert_eq!(d(75, 2).to_string(), "0.75");
    assert_eq!(d(150, 2), d(15, 1));
    assert_eq!(Decimal::from(3), d(30, 1));
    assert_eq!(Decimal::from(-7_i64).to_string(), "-7");
    assert_eq!(Decimal::ZERO, d(0, 5));
}

#[test]
fn add_sub_mul_are_exact() {
    // Slow: 1 + 0.1 * 5 is exactly 1.5 (inexact in binary floating point).
    assert_eq!(Decimal::ONE + d(1, 1) * Decimal::from(5), d(15, 1));
    assert_eq!(d(1, 1) + d(2, 1), d(3, 1));
    assert_eq!(d(1, 1) - d(25, 2), d(-15, 2));
    // Weak then Vulnerable, one factor at a time.
    let damage = Decimal::from(7) * d(75, 2) * d(15, 1);
    assert_eq!(damage, d(7875, 3));
    assert_eq!(damage.to_int_truncating(), 7);
    let mut x = Decimal::from(10);
    x += d(5, 1);
    x -= Decimal::ONE;
    x *= d(2, 0);
    assert_eq!(x, Decimal::from(19));
}

#[test]
fn long_chains_stay_exact() {
    // A capped 999_999_999 hit through many two-decimal factors overflows a scaled i64.
    let mut value = Decimal::from(999_999_999);
    for _ in 0..8 {
        value *= d(75, 2);
    }
    assert_eq!(value.to_string(), "100112914.9389495849609375");
    assert_eq!(value.to_int_truncating(), 100_112_914);
}

#[test]
fn product_trailing_zeros_cancel_before_overflow() {
    // 5^41 * 2^55 / 10^41 = 2^14: the raw product exceeds i128 but the value fits.
    let fives = d(5_i128.pow(41), 28);
    let twos = d(2_i128.pow(55), 13);
    assert_eq!(fives * twos, Decimal::from(2_i32.pow(14)));
}

#[test]
fn comparison_is_numeric() {
    assert!(d(75, 2) < Decimal::ONE);
    assert!(d(-5, 1) < Decimal::ZERO);
    assert!(Decimal::MAX > d(1, 28));
    assert!(Decimal::MIN < d(-1, 28));
    assert_eq!(Decimal::MAX.min(Decimal::from(12)), Decimal::from(12));
    assert_eq!(d(10, 1).cmp(&Decimal::ONE), std::cmp::Ordering::Equal);
}

#[test]
fn truncation_goes_toward_zero() {
    assert_eq!(d(29, 1).to_int_truncating(), 2);
    assert_eq!(d(-29, 1).to_int_truncating(), -2);
    assert_eq!(d(-9, 1).to_int_truncating(), 0);
    assert_eq!(Decimal::from(i32::MIN).to_int_truncating(), i32::MIN);
}

#[test]
#[should_panic(expected = "OverflowException")]
fn truncation_overflow_panics() {
    let _ = (Decimal::from(i32::MAX) + Decimal::ONE).to_int_truncating();
}

#[test]
#[should_panic(expected = "not representable")]
fn overflow_panics() {
    let _ = Decimal::MAX + Decimal::ONE;
}

#[test]
#[should_panic(expected = "not representable")]
fn precision_loss_panics() {
    // 29 fractional digits: .NET would round here.
    let _ = d(1, 28) * d(1, 1);
}

#[test]
fn truncate_and_ceiling() {
    assert_eq!(d(275, 2).truncate(), Decimal::from(2));
    assert_eq!(d(-275, 2).truncate(), Decimal::from(-2));
    assert_eq!(d(201, 2).ceiling(), Decimal::from(3));
    assert_eq!(d(-201, 2).ceiling(), Decimal::from(-2));
    assert_eq!(Decimal::from(5).ceiling(), Decimal::from(5));
}
