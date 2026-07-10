//! Small numeric operations whose Java semantics are easy to lose in Rust.
//!
//! Shattered Pixel Dungeon v3.3.8 performs its generation in Java.  In
//! particular, Java floating-point-to-integer casts saturate (and map NaN to
//! zero), `Math.round` breaks half-way ties toward positive infinity, and all
//! intermediate `int` arithmetic wraps.  Keeping those rules named at the
//! compatibility boundary makes the geometry port auditable.
//!
//! Reference: Java SE 21 JLS 5.1.3/5.1.4 and `java.lang.Math.round`.

/// Java's `(int) value` narrowing conversion for a `float`.
#[must_use]
pub fn f32_to_i32(value: f32) -> i32 {
    // Since Rust 1.45, `as` deliberately has the same saturation and NaN rules
    // as Java's floating-point narrowing conversion.
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = value as i32;
    narrowed
}

/// Java's `(int) value` narrowing conversion for a `double`.
#[must_use]
pub fn f64_to_i32(value: f64) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = value as i32;
    narrowed
}

/// Java's `(long) value` narrowing conversion for a `double`.
#[must_use]
pub fn f64_to_i64(value: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = value as i64;
    narrowed
}

/// Exact equivalent of `Math.round(float)`.
///
/// A Java `float` is first widened to `f64` so the half-unit comparison is
/// exact.  Computing `(value + 0.5_f32).floor()` directly is subtly wrong for
/// `nextDown(0.5f)`, whose same-width addition rounds up to `1.0` even though
/// `Math.round` correctly returns zero.  Rust's `f32::round` is also incorrect
/// for Java's negative half-way rule.
#[must_use]
pub fn round_f32(value: f32) -> i32 {
    let value = f64::from(value);
    let floor = value.floor();
    let rounded = if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    f64_to_i32(rounded)
}

/// Exact equivalent of `Math.round(double)`.
#[must_use]
pub fn round_f64(value: f64) -> i64 {
    let floor = value.floor();
    let rounded = if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    f64_to_i64(rounded)
}

/// Java's `(int)Math.floor(value)`.
#[must_use]
pub fn floor_f64_to_i32(value: f64) -> i32 {
    f64_to_i32(value.floor())
}

/// Java's `(int)Math.ceil(value)`.
#[must_use]
pub fn ceil_f64_to_i32(value: f64) -> i32 {
    f64_to_i32(value.ceil())
}

/// Java `int` division, including the specified `MIN_VALUE / -1` wraparound.
///
/// # Panics
///
/// Panics on division by zero, just like Java throws `ArithmeticException`.
#[must_use]
pub fn div_i32(dividend: i32, divisor: i32) -> i32 {
    assert_ne!(divisor, 0, "integer division by zero");
    dividend.wrapping_div(divisor)
}

/// Java `int` remainder, including `MIN_VALUE % -1 == 0`.
///
/// # Panics
///
/// Panics on division by zero, just like Java throws `ArithmeticException`.
#[must_use]
pub fn rem_i32(dividend: i32, divisor: i32) -> i32 {
    assert_ne!(divisor, 0, "integer remainder by zero");
    dividend.wrapping_rem(divisor)
}

#[cfg(test)]
mod tests {
    use super::{
        ceil_f64_to_i32, div_i32, f32_to_i32, f64_to_i32, floor_f64_to_i32, rem_i32, round_f32,
        round_f64,
    };

    /// Values captured from `OpenJDK` 21 `Math.round` and casts.  The half-way
    /// cases guard against accidentally replacing Java rounding with
    /// `f32::round`, which rounds `-1.5` in the opposite direction.
    #[test]
    fn java_rounding_and_narrowing_reference_vector() {
        assert_eq!(round_f32(1.5), 2);
        assert_eq!(round_f32(-1.5), -1);
        assert_eq!(round_f32(-1.6), -2);
        assert_eq!(round_f32(f32::from_bits(0x3eff_ffff)), 0); // nextDown(0.5f)
        assert_eq!(round_f64(2.5), 3);
        assert_eq!(round_f64(-2.5), -2);
        assert_eq!(round_f64(f64::from_bits(0x3fdf_ffff_ffff_ffff)), 0);

        assert_eq!(f32_to_i32(f32::NAN), 0);
        assert_eq!(f32_to_i32(f32::INFINITY), i32::MAX);
        assert_eq!(f32_to_i32(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(f64_to_i32(3.9), 3);
        assert_eq!(f64_to_i32(-3.9), -3);
        assert_eq!(floor_f64_to_i32(-3.1), -4);
        assert_eq!(ceil_f64_to_i32(-3.9), -3);
    }

    #[test]
    fn java_signed_division_wraps_only_overflowing_pair() {
        assert_eq!(div_i32(-7, 3), -2);
        assert_eq!(rem_i32(-7, 3), -1);
        assert_eq!(div_i32(i32::MIN, -1), i32::MIN);
        assert_eq!(rem_i32(i32::MIN, -1), 0);
    }
}
