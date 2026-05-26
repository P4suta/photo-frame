//! Golden rectangle as a value type, with one operation: `subdivide`.
//!
//! A golden rectangle is the unique rectangle whose long edge stands
//! to its short edge as `φ : 1`. Its defining property is closure
//! under removing a square: a golden rectangle equals a square of
//! the short edge plus a smaller golden rectangle whose short edge is
//! the original's `long − short = short / φ`.
//!
//! [`GoldenRectangle::subdivide`] returns that smaller rectangle (the
//! *residue*). The peeled square is implicit — its edge is the
//! source rectangle's short edge.

/// Golden ratio.
pub(crate) const PHI: f64 = 1.618_033_988_749_895;

/// Which way the long edge of a [`GoldenRectangle`] points.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    /// Long edge runs horizontally; the rectangle is wider than it
    /// is tall.
    Horizontal,
    /// Long edge runs vertically; the rectangle is taller than it
    /// is wide.
    Vertical,
}

/// A rectangle whose `long / short` ratio is [`PHI`] by construction.
#[derive(Copy, Clone, Debug)]
pub(crate) struct GoldenRectangle {
    pub(crate) short: f64,
    pub(crate) long: f64,
    pub(crate) orientation: Axis,
}

impl GoldenRectangle {
    /// Construct a golden rectangle whose short edge is `short`.
    pub(crate) fn from_short(short: f64, orientation: Axis) -> Self {
        Self {
            short,
            long: short * PHI,
            orientation,
        }
    }

    /// Subdivide: return the residue golden rectangle that remains
    /// after peeling a square of edge `self.short` off the long-edge
    /// end. The residue's short edge equals `self.long − self.short`
    /// (= `self.short / φ`); the orientation flips so the residue's
    /// long axis is perpendicular to `self`'s.
    pub(crate) fn subdivide(&self) -> Self {
        let residue_short = self.long - self.short;
        let residue_orientation = match self.orientation {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        };
        Self::from_short(residue_short, residue_orientation)
    }
}

#[cfg(test)]
mod tests {
    use super::{Axis, GoldenRectangle, PHI};

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn from_short_sets_long_to_phi_times_short() {
        let g = GoldenRectangle::from_short(100.0, Axis::Vertical);
        assert!(approx(g.long, 100.0 * PHI, 1e-12));
        assert!(approx(g.short, 100.0, 1e-12));
        assert_eq!(g.orientation, Axis::Vertical);
    }

    #[test]
    fn subdivide_residue_is_golden() {
        let g = GoldenRectangle::from_short(42.0, Axis::Vertical);
        let residue = g.subdivide();
        assert!(approx(residue.long / residue.short, PHI, 1e-12));
    }

    #[test]
    fn subdivide_closure_holds() {
        // Implicit square edge + residue short edge = original long edge,
        // which is the defining closure of the golden rectangle.
        for short in [1.0_f64, 13.0, 100.0, 1234.5, 99_999.0] {
            let g = GoldenRectangle::from_short(short, Axis::Vertical);
            let residue = g.subdivide();
            assert!(
                approx(g.short + residue.short, g.long, 1e-9),
                "short={short}"
            );
        }
    }

    #[test]
    fn subdivide_orientation_flips() {
        let v = GoldenRectangle::from_short(10.0, Axis::Vertical);
        assert_eq!(v.subdivide().orientation, Axis::Horizontal);
        let h = GoldenRectangle::from_short(10.0, Axis::Horizontal);
        assert_eq!(h.subdivide().orientation, Axis::Vertical);
    }

    #[test]
    fn nested_subdivision_short_edges_form_phi_chain() {
        // After `n` subdivisions the residue's short edge is
        // `start.short / φⁿ`.
        let g0 = GoldenRectangle::from_short(1000.0, Axis::Vertical);
        let g1 = g0.subdivide();
        let g2 = g1.subdivide();
        let g3 = g2.subdivide();
        assert!(approx(g1.short, 1000.0 / PHI, 1e-9));
        assert!(approx(g2.short, 1000.0 / PHI.powi(2), 1e-9));
        assert!(approx(g3.short, 1000.0 / PHI.powi(3), 1e-9));
    }
}
