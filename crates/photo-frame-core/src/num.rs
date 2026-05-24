//! Numeric conversion helpers.
//!
//! Floating-point → unsigned integer rounding is unavoidably a lossy cast.
//! Centralising it behind a single named function lets the rest of the
//! geometry code read in *domain* terms (`round_to_u32(x)` vs `x as u32`),
//! and keeps the one place where a checked-cast lint is silenced explicit
//! and audited.

/// Round `v` to the nearest `u32`.
///
/// Non-finite or strictly-negative inputs return `0`; values larger than
/// [`u32::MAX`] saturate at the top. The explicit bounds check above the
/// cast is what makes the cast safe — clippy can't see through it, hence
/// the targeted allow on this single function.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "explicit finiteness and range guards above"
)]
pub(crate) fn round_to_u32(v: f64) -> u32 {
    let r = v.round();
    if !r.is_finite() || r < 0.0 {
        0
    } else if r >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        r as u32
    }
}

/// `f32` variant of [`round_to_u32`].
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "explicit finiteness and range guards above"
)]
pub(crate) fn round_to_u32_f32(v: f32) -> u32 {
    // Widen to f64 for the comparison so we don't need an f32→u32 endpoint.
    round_to_u32(f64::from(v))
}

#[cfg(test)]
mod tests {
    use super::{round_to_u32, round_to_u32_f32};

    #[test]
    fn rounds_to_nearest() {
        assert_eq!(round_to_u32(2.4), 2);
        assert_eq!(round_to_u32(2.6), 3);
        assert_eq!(round_to_u32(-0.0), 0);
    }

    #[test]
    fn clamps_negative_to_zero() {
        assert_eq!(round_to_u32(-1.0), 0);
        assert_eq!(round_to_u32(-1e9), 0);
    }

    #[test]
    fn saturates_at_max() {
        assert_eq!(round_to_u32(1e20), u32::MAX);
        assert_eq!(round_to_u32(f64::INFINITY), 0);
    }

    #[test]
    fn handles_nan() {
        assert_eq!(round_to_u32(f64::NAN), 0);
    }

    #[test]
    fn f32_delegates_correctly() {
        assert_eq!(round_to_u32_f32(2.4), 2);
        assert_eq!(round_to_u32_f32(2.6), 3);
        assert_eq!(round_to_u32_f32(-1.0), 0);
    }
}
