//! Golden-ratio layout calculation.
//!
//! See [`crate::num::round_to_u32`] for the rounding policy used throughout.
//!
//! Every output dimension is expressed as `side · φⁿ` for some integer `n`, so
//! the framing reads as a single coherent scale rather than a bag of magic
//! numbers. `side` is the only quantization point; downstream dimensions are
//! derived from it via the formulas below, then the metadata strip absorbs
//! its rounding slack symmetrically above and below the text block.
//!
//! ```text
//! min_dim     = side · φ⁶          (input constraint — defines side)
//! pad_x       = side · φ⁰          (text aligns flush with photo edges)
//! pad_top     = side · φ⁻¹         (minimum text inset inside the strip)
//! bottom      = side · φ²          (≈ 2.618 · side; height of meta strip)
//! font_height = side · φ⁻²         (≈ 0.382 · side; caption cap height)
//! line_gap    = side · φ⁻³         (≈ 0.236 · side; gap between the two lines)
//! ```
//!
//! Closure: by the identity `φ² = 2φ + 1` and `φ⁻ⁿ = φ⁻ⁿ⁺¹ − φ⁻ⁿ⁻¹` (the
//! Fibonacci recurrence on negative powers), `bottom − 2·pad_top − 2·font −
//! line_gap` simplifies to `side · φ⁻²`. That residual is distributed
//! symmetrically above and below the text block, so the strip stays
//! visually balanced without breaking the φⁿ chain on any *named* quantity.
//!
//! Exception: for photos whose short edge is below `MIN_SIDE_PX · φ⁶ ≈ 144 px`,
//! `side` clamps at `MIN_SIDE_PX`. The φⁿ chain is broken locally at that
//! point; framing such inputs is degenerate anyway.

use crate::num::round_to_u32;

/// Golden ratio.
pub(crate) const PHI: f64 = 1.618_033_988_749_895;

/// Minimum pixel thickness for `side` — below this, framing becomes invisible
/// on any reasonable display.
const MIN_SIDE_PX: u32 = 8;

/// Outer side-frame thickness, derived from the photo's shorter edge.
pub(crate) fn side_for(min_dim: u32) -> u32 {
    round_to_u32(f64::from(min_dim) / PHI.powi(6)).max(MIN_SIDE_PX)
}

/// Bottom strip height when the metadata caption is rendered.
pub(crate) fn bottom_for(side: u32) -> u32 {
    round_to_u32(f64::from(side) * PHI.powi(2))
}

/// Vertical inset between the strip edges and the text block — `side / φ`.
pub(crate) fn pad_top_for(side: u32) -> u32 {
    round_to_u32(f64::from(side) / PHI)
}

/// Caption font height in pixels — `side / φ²`. One step larger than the
/// v1 value (`side / φ³`); justified by the user observing that the v1
/// caption was perceptually too small on full-resolution Nikon Z output.
#[allow(
    clippy::cast_possible_truncation,
    reason = "f32 precision is fine for caption sizing (< 24-bit range)"
)]
pub(crate) fn font_height_for(side: u32) -> f32 {
    (f64::from(side) / PHI.powi(2)) as f32
}

/// Vertical gap between the two caption rows — `side / φ³`. Sits exactly
/// `font_height_for(side) / φ` below the font height, so single-line spacing
/// reads as a typographic third stop below cap height.
pub(crate) fn line_gap_for(side: u32) -> u32 {
    round_to_u32(f64::from(side) / PHI.powi(3))
}

/// Complete spatial layout of the framed output.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Layout {
    pub(crate) canvas_size: (u32, u32),
    pub(crate) photo_origin: (u32, u32),
    pub(crate) meta: Option<MetaLayout>,
}

/// Sub-layout describing the metadata strip and the two text rows inside it.
#[derive(Copy, Clone, Debug)]
pub(crate) struct MetaLayout {
    pub(crate) pad_x: u32,
    pub(crate) top_line_y: u32,
    pub(crate) bottom_line_y: u32,
    pub(crate) font_height: f32,
}

/// Compute the [`Layout`] for a photo of the given pixel dimensions.
///
/// When `show_meta` is `false`, the bottom strip collapses to the same
/// thickness as the side frame, producing a symmetric thin border.
pub(crate) fn compute(photo: (u32, u32), show_meta: bool) -> Layout {
    let (w, h) = photo;
    let side = side_for(w.min(h));
    let bottom = if show_meta { bottom_for(side) } else { side };

    let canvas = (w + 2 * side, h + side + bottom);
    let photo_origin = (side, side);
    let meta = show_meta.then(|| meta_layout(h + side, bottom, side));

    Layout {
        canvas_size: canvas,
        photo_origin,
        meta,
    }
}

/// Derive a [`MetaLayout`] from a strip placed at `strip_y` with the given
/// `bottom` (strip height) and `side` (base φⁿ unit).
///
/// The strip layout is:
///
/// ```text
///   strip_y ┬─────────────────────────────────────┐
///           │  pad_top + slack/2                  │
///           ├─────────────────────────────────────┤  ← top_line_y
///           │  font_height                        │
///           ├─────────────────────────────────────┤
///           │  line_gap                           │
///           ├─────────────────────────────────────┤  ← bottom_line_y
///           │  font_height                        │
///           ├─────────────────────────────────────┤
///           │  pad_top + slack/2                  │
///           └─────────────────────────────────────┘
/// ```
///
/// where `slack = bottom − 2·pad_top − 2·font_h − line_gap` (provably
/// `side · φ⁻²` in the limit of zero rounding error). Splitting `slack`
/// equally above and below preserves vertical symmetry within 1 px.
fn meta_layout(strip_y: u32, bottom: u32, side: u32) -> MetaLayout {
    let font_height = font_height_for(side);
    let line_gap = line_gap_for(side);
    let pad_top = pad_top_for(side);

    let font_h_rounded = crate::num::round_to_u32_f32(font_height);
    let text_block_px = 2 * font_h_rounded + line_gap;
    let used = 2 * pad_top + text_block_px;
    let slack = bottom.saturating_sub(used);
    let half_slack = slack / 2;

    let top_line_y = strip_y + pad_top + half_slack;
    let bottom_line_y = top_line_y + font_h_rounded + line_gap;

    MetaLayout {
        pad_x: side,
        top_line_y,
        bottom_line_y,
        font_height,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bottom_for, compute, font_height_for, line_gap_for, pad_top_for, side_for, MIN_SIDE_PX, PHI,
    };
    use crate::num::{round_to_u32, round_to_u32_f32};

    /// Concrete-value table: locking in the exact pixel sizes for a 3000×2000
    /// image keeps the test honest — if any formula drifts, this fails loudly.
    const REF_W: u32 = 3000;
    const REF_H: u32 = 2000;
    const REF_SIDE: u32 = 111; // round(2000 / φ⁶)
    const REF_BOTTOM: u32 = 291; // round(111 · φ²)
    const REF_PAD_TOP: u32 = 69; // round(111 / φ)
    const REF_LINE_GAP: u32 = 26; // round(111 / φ³)
                                  // round(111 / φ²) = round(42.39) = 42 — but font_h is f32, so the
                                  // rounded integer used in layout math is 42.
    const REF_FONT_H_ROUNDED: u32 = 42;

    #[test]
    fn side_locked_table_value() {
        assert_eq!(side_for(REF_H), REF_SIDE);
    }

    #[test]
    fn bottom_locked_table_value() {
        assert_eq!(bottom_for(REF_SIDE), REF_BOTTOM);
    }

    #[test]
    fn pad_top_is_side_over_phi() {
        assert_eq!(pad_top_for(REF_SIDE), REF_PAD_TOP);
    }

    #[test]
    fn font_height_is_side_over_phi_squared() {
        assert_eq!(
            round_to_u32_f32(font_height_for(REF_SIDE)),
            REF_FONT_H_ROUNDED
        );
    }

    #[test]
    fn line_gap_is_side_over_phi_cubed() {
        assert_eq!(line_gap_for(REF_SIDE), REF_LINE_GAP);
    }

    #[test]
    fn side_recomputes_from_formula() {
        let expected = round_to_u32(f64::from(REF_H) / PHI.powi(6)).max(MIN_SIDE_PX);
        assert_eq!(side_for(REF_H), expected);
    }

    #[test]
    fn min_side_kicks_in_for_tiny_images() {
        assert_eq!(side_for(40), MIN_SIDE_PX);
    }

    #[test]
    fn canvas_includes_all_four_borders() {
        let layout = compute((REF_W, REF_H), true);
        assert_eq!(
            layout.canvas_size,
            (REF_W + 2 * REF_SIDE, REF_H + REF_SIDE + REF_BOTTOM)
        );
    }

    #[test]
    fn photo_sits_at_inner_top_left() {
        let layout = compute((REF_W, REF_H), true);
        assert_eq!(layout.photo_origin, (REF_SIDE, REF_SIDE));
    }

    #[test]
    fn bottom_collapses_to_side_when_meta_hidden() {
        let layout = compute((REF_W, REF_H), false);
        assert_eq!(
            layout.canvas_size,
            (REF_W + 2 * REF_SIDE, REF_H + 2 * REF_SIDE)
        );
        assert!(layout.meta.is_none());
    }

    #[test]
    fn meta_lines_fit_within_strip() {
        let layout = compute((REF_W, REF_H), true);
        let meta = layout.meta.expect("meta visible");
        let strip_bottom_y = REF_H + REF_SIDE + REF_BOTTOM;
        let last_glyph_y = meta.bottom_line_y + round_to_u32_f32(meta.font_height);
        assert!(last_glyph_y <= strip_bottom_y);
    }

    #[test]
    fn meta_lines_are_vertically_balanced() {
        let layout = compute((REF_W, REF_H), true);
        let meta = layout.meta.expect("meta visible");
        let strip_top = REF_H + REF_SIDE;
        let strip_bottom = strip_top + REF_BOTTOM;
        let font_h = round_to_u32_f32(meta.font_height);
        let top_pad = meta.top_line_y - strip_top;
        let bottom_pad = strip_bottom - (meta.bottom_line_y + font_h);
        assert!(
            top_pad.abs_diff(bottom_pad) <= 1,
            "top_pad={top_pad}, bottom_pad={bottom_pad}",
        );
    }

    /// The strip is `bottom = 2·pad_top + text_block + slack` where slack ≈
    /// `side · φ⁻²` exactly (within rounding). Locking this identity here
    /// is what makes the design *provably* φⁿ-closed rather than
    /// approximately so.
    #[test]
    fn strip_closure_is_phi_minus_two() {
        let side = REF_SIDE;
        let bottom = bottom_for(side);
        let pad_top = pad_top_for(side);
        let font_h = round_to_u32_f32(font_height_for(side));
        let line_gap = line_gap_for(side);
        let used = 2 * pad_top + 2 * font_h + line_gap;
        let slack = bottom - used;
        let expected_slack = round_to_u32(f64::from(side) / PHI.powi(2));
        assert!(
            slack.abs_diff(expected_slack) <= 1,
            "slack={slack}, expected ≈ side/φ² = {expected_slack}",
        );
    }

    /// Sanity scan across a range of input sizes: the closure identity must
    /// hold (within ±1 px) for every reasonable photo dimension.
    #[test]
    fn strip_closure_holds_across_input_range() {
        for h in (200..=8000).step_by(137) {
            let side = side_for(h);
            let bottom = bottom_for(side);
            let pad_top = pad_top_for(side);
            let font_h = round_to_u32_f32(font_height_for(side));
            let line_gap = line_gap_for(side);
            let used = 2 * pad_top + 2 * font_h + line_gap;
            let slack = bottom.saturating_sub(used);
            let expected_slack = round_to_u32(f64::from(side) / PHI.powi(2));
            assert!(
                slack.abs_diff(expected_slack) <= 2,
                "h={h}, side={side}, bottom={bottom}, slack={slack}, expected={expected_slack}",
            );
        }
    }

    /// The user's headline requirement: the v1.1 font is 1.618× bigger than
    /// v1's `side / φ³`. Locking this catches accidental regressions to the
    /// older formula.
    #[test]
    fn font_height_is_phi_times_v1_value() {
        let side = REF_SIDE;
        let v1 = f64::from(side) / PHI.powi(3);
        let v1_1 = f64::from(font_height_for(side));
        assert!(
            (v1_1 / v1 - PHI).abs() < 1e-6,
            "v1.1/v1 = {} ; want φ ≈ {PHI}",
            v1_1 / v1,
        );
    }
}
