//! The chain of golden-rectangle residues produced by repeated
//! subdivision.
//!
//! A [`GoldenRectangle`] subdivides into a square plus a smaller
//! golden rectangle (its *residue*). Tracing the residue at every
//! step gives the classical φ-spiral: each rectangle's short edge is
//! the previous one's `short / φ`. Reading the residue at depth `n`
//! is the same number as `start.short / φⁿ`, but the chain form
//! makes "the geometry's `n`-th iteration" the source of the number
//! rather than an `n`-th-power multiplier.

use super::rectangle::GoldenRectangle;

/// A finite chain of subdivision residues starting from a seed
/// rectangle.
#[derive(Clone, Debug)]
pub(crate) struct GoldenSpiral {
    residues: Vec<GoldenRectangle>,
}

impl GoldenSpiral {
    /// Trace the spiral by subdividing `start` exactly `depth` times,
    /// recursing on the residue each step. The resulting spiral
    /// exposes the residue at every depth from `0` to `depth − 1`.
    pub(crate) fn from_rectangle(start: GoldenRectangle, depth: u32) -> Self {
        let mut residues = Vec::with_capacity(depth as usize);
        let mut current = start;
        for _ in 0..depth {
            let residue = current.subdivide();
            residues.push(residue);
            current = residue;
        }
        Self { residues }
    }

    /// Borrow the residue at the given depth (0-based). Depth `0` is
    /// the first subdivision's residue (`start.short / φ`); depth `n`
    /// is `start.short / φ^(n+1)`.
    ///
    /// # Panics
    /// Panics if `depth >= depth_passed_to_from_rectangle`. Callers in
    /// this crate always know the depth at construction time, so the
    /// index is a static fact rather than user input.
    pub(crate) fn residue_at(&self, depth: u32) -> &GoldenRectangle {
        &self.residues[depth as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::super::rectangle::{Axis, GoldenRectangle, PHI};
    use super::GoldenSpiral;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn each_residue_is_phi_times_smaller_than_previous() {
        const DEPTH: u32 = 5;
        let r = GoldenRectangle::from_short(1000.0, Axis::Vertical);
        let s = GoldenSpiral::from_rectangle(r, DEPTH);
        let mut previous = 1000.0_f64;
        for n in 0..DEPTH {
            assert!(approx(s.residue_at(n).short, previous / PHI, 1e-6));
            previous = s.residue_at(n).short;
        }
    }

    #[test]
    fn residue_at_depth_n_matches_phi_power() {
        let r = GoldenRectangle::from_short(1000.0, Axis::Vertical);
        let s = GoldenSpiral::from_rectangle(r, 4);
        for n in 0_u32..4 {
            let expected = 1000.0 / PHI.powi(i32::try_from(n).unwrap() + 1);
            assert!(approx(s.residue_at(n).short, expected, 1e-6));
        }
    }
}
