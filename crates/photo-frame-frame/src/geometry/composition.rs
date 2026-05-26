//! Composing a photo into the golden geometry.
//!
//! ## Construction
//!
//! Everything derives from a single quantum,
//!
//! ```text
//! quantum = photo_short_edge / φ⁶
//! ```
//!
//! the φ-spiral residue six levels below the photo's short edge. The
//! quantum is the **primary caption row's font height** (the
//! typographic measure), and `2·quantum` is the **mat** — the uniform
//! breathing band that surrounds every interior boundary of the
//! layout. Font and mat are deliberately offset by one φ-power so the
//! mat reads as deliberate frame mass rather than thin padding.
//!
//! ## Margin uniformity
//!
//! Every two adjacent elements are separated by exactly one mat:
//!
//! ```text
//! ┌──────────────────┐
//! │       mat        │  ← canvas top → photo
//! │  ┌────────────┐  │
//! │m │            │ m│  ← photo's left/right margin = mat
//! │a │   PHOTO    │ a│
//! │t │            │ t│
//! │  └────────────┘  │
//! │       mat        │  ← photo → strip
//! │  Caption row 1   │  ← primary font (= quantum)
//! │       gap        │
//! │  Caption row 2   │  ← secondary font (= quantum / φ)
//! │       mat        │  ← strip → canvas bottom
//! └──────────────────┘
//! ```
//!
//! So `canvas.W = photo.W + 2·mat` and `canvas.H = photo.H + 3·mat +
//! strip_h`. The canvas is *not* itself constrained to be golden —
//! forcing a golden outer rectangle competes with the uniform-mat
//! requirement and always loses (excess space lands as asymmetric
//! letterbox or pillarbox). Instead the **golden ratio appears
//! internally**:
//!
//! - `quantum / photo_short = 1 / φ⁶`         (type quantum, depth-6 spiral)
//! - `mat = 2 · quantum`                       (mat = `strip_h`: every interior
//!   gap shares a single mass)
//! - `secondary_font / primary_font = 1 / φ`  (type hierarchy)
//! - `line_gap / secondary_font = 1 / φ`      (rhythm within the strip)
//! - `strip_h = 2 · quantum`                   (strip equals mat — caption text
//!   fills its own mat-sized lane)
//!
//! ## Typographic hierarchy
//!
//! The two caption rows have different sizes so the EXIF data is not
//! a uniform wall of text:
//!
//! - **Row 1** (primary): camera body + lens, `font = quantum`,
//!   `Weight::Medium`. The identity line.
//! - **Row 2** (secondary): exposure + date, `font = quantum/φ`,
//!   `Weight::Regular`. The context line.
//!
//! Renderers position both rows so left- and right-aligned text
//! shares the photo's horizontal edges; the caption column sits
//! directly under the photo column.
//!
//! ## `show_meta = false`
//!
//! When the caption strip is suppressed the same `2·quantum` mat
//! wraps the photo on all four sides — no strip, but every other
//! gap stays the same width. The print reads as the captioned
//! version with the caption removed, not as a different layout.
//!
//! ## Polaroid style
//!
//! The Polaroid variant top-anchors the photo and replaces the
//! standard "strip + bottom mat" pair with a single, much heavier
//! bottom band beneath it. Side and top mats stay at the standard
//! `2·quantum`. The canvas is **always portrait** (`canvas.H ≥
//! canvas.W`) regardless of the input photo's orientation — for
//! landscape input the bottom band expands until that constraint is
//! met; for portrait input the band stays at the minimum `4·mat`
//! (the classical Polaroid bottom : side ≈ 4 : 1 ratio).
//!
//! The `show_meta` flag does not change Polaroid geometry. When
//! `false` the caption text is simply not drawn; the bottom band
//! stays the same size so the print keeps the Polaroid silhouette.
//!
//! ## Pixel rounding
//!
//! All recursion happens in `f64`. Only the leaf values the renderer
//! consumes — `canvas`, `photo_origin`, and the `MetaLayout` baselines
//! — convert to `u32` / `f32` at the boundary via [`round_to_u32`].

use crate::num::round_to_u32;

use super::rectangle::{Axis, GoldenRectangle, PHI};
use super::spiral::GoldenSpiral;

/// Spiral depth that produces the layout quantum from the photo's
/// shorter edge. Calibrated so that the resulting quantum sits at
/// the visual scale where the primary caption row reads as
/// deliberate type rather than thin annotation; every other
/// measurement is a small-integer or `1/φ` multiple of it.
const QUANTUM_DEPTH: u32 = 6;

/// Frame-style switch consumed by [`compute`]. The standard style
/// keeps the photo centred in a uniform-mat frame; the Polaroid
/// style top-anchors the photo and replaces the strip + bottom mat
/// with a single heavier band beneath it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum LayoutStyle {
    /// Uniform `2·quantum` mat on every interior boundary, caption
    /// strip sits one mat below the photo. Used by `Edges` and
    /// `Centered` caption variants.
    Standard,
    /// Photo top-anchored, bottom area thickened to `4·mat = 8·quantum`.
    /// Used by the `Polaroid` caption variant; the renderer centres the
    /// two caption rows vertically inside the thick bottom band.
    Polaroid,
}

/// Complete spatial plan of a framed output.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Composition {
    /// Canvas dimensions in pixels, `(width, height)`.
    pub(crate) canvas: (u32, u32),
    /// Photo's top-left corner inside the canvas, `(x, y)`.
    pub(crate) photo_origin: (u32, u32),
    /// Photo dimensions in pixels, `(width, height)`. Identical to
    /// the input photo size — the geometry never scales the photo.
    /// The renderer reads this to bound caption text width during
    /// auto-fit.
    pub(crate) photo_size: (u32, u32),
    /// Caption strip placement; `None` when `show_meta = false`.
    pub(crate) meta: Option<MetaLayout>,
}

/// Sub-plan describing the metadata strip and the two caption rows.
#[derive(Copy, Clone, Debug)]
pub(crate) struct MetaLayout {
    /// Strip rectangle inside the canvas as `(x, y, w, h)`. The
    /// renderer uses `(y, h)` to centre the caption text vertically
    /// after the auto-fit step decides on the actual font height.
    pub(crate) region: (u32, u32, u32, u32),
    /// x of the photo's left edge in the canvas — the left-alignment
    /// anchor for caption text. Photo and caption share this column.
    pub(crate) photo_left_x: u32,
    /// x of the photo's right edge in the canvas — the right-alignment
    /// anchor for caption text.
    pub(crate) photo_right_x: u32,
    /// Primary caption font height (camera body + lens row).
    /// Renderer treats this as the *ideal* size; the auto-fit step
    /// may shrink the actual font to keep text inside the photo's
    /// horizontal column.
    pub(crate) primary_font_height: f32,
    /// Secondary caption font height (exposure + date row).
    /// Smaller by a factor of φ so the data reads as supporting
    /// information rather than competing with the identity line.
    pub(crate) secondary_font_height: f32,
    /// Pixel distance between the two caption rows (between primary's
    /// baseline-bottom and secondary's top). Scales with the font when
    /// the renderer auto-fits.
    pub(crate) line_gap: f32,
}

/// Compute the [`Composition`] for the given photo dimensions.
#[allow(
    clippy::cast_possible_truncation,
    reason = "f64 → f32 narrowing is safe for font heights (well below 24-bit precision)"
)]
pub(crate) fn compute(photo: (u32, u32), show_meta: bool, style: LayoutStyle) -> Composition {
    let photo_width = f64::from(photo.0);
    let photo_height = f64::from(photo.1);
    let photo_short = photo_width.min(photo_height);

    // Layout quantum: φ-spiral residue six levels below the photo's
    // short edge. Drives mat, primary font, and secondary font through
    // their closure relations.
    let quantum_seed = GoldenRectangle::from_short(photo_short, Axis::Vertical);
    let quantum_spiral = GoldenSpiral::from_rectangle(quantum_seed, QUANTUM_DEPTH);
    let quantum = quantum_spiral.residue_at(QUANTUM_DEPTH - 1).short;

    let primary_font = quantum;
    let secondary_font = quantum / PHI;
    let line_gap = quantum / PHI.powi(2);
    // Strip closure: primary + gap + secondary = quantum·(1 + 1/φ² +
    // 1/φ) = quantum·2 (since 1 + 1/φ + 1/φ² = 2). So the strip is
    // exactly `2·quantum` tall — the rhythm is self-balancing.
    let strip_h = primary_font + line_gap + secondary_font;

    // Mat thickness is `2·quantum` across every layout. The mat and
    // the strip share the same `2·quantum` height — every interior
    // gap (canvas-edge ↔ photo, photo ↔ strip, strip ↔ canvas-edge)
    // is the same visible mass.
    let mat = 2.0 * quantum;

    match (show_meta, style) {
        (_, LayoutStyle::Polaroid) => compose_polaroid(
            photo,
            photo_width,
            photo_height,
            primary_font,
            secondary_font,
            line_gap,
            mat,
            show_meta,
        ),
        (true, LayoutStyle::Standard) => compose_standard(
            photo,
            photo_width,
            photo_height,
            primary_font,
            secondary_font,
            line_gap,
            mat,
            strip_h,
        ),
        (false, LayoutStyle::Standard) => compose_no_meta(photo, photo_width, photo_height, mat),
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "f64 → f32 narrowing is safe for font heights (well below 24-bit precision)"
)]
#[allow(
    clippy::too_many_arguments,
    reason = "internal helper called from one site; bundling the parameters into a struct adds boilerplate without making the call site clearer"
)]
fn compose_standard(
    photo: (u32, u32),
    photo_width: f64,
    photo_height: f64,
    primary_font: f64,
    secondary_font: f64,
    line_gap: f64,
    mat: f64,
    strip_h: f64,
) -> Composition {
    // Standard: photo centred in canvas, uniform mat + strip below.
    // canvas.W = photo + 2·mat
    // canvas.H = photo + 3·mat + strip (= photo + 4·mat since strip = mat)
    let canvas_width = 2.0_f64.mul_add(mat, photo_width);
    let canvas_height = 3.0_f64.mul_add(mat, photo_height + strip_h);
    let canvas = (round_to_u32(canvas_width), round_to_u32(canvas_height));

    let photo_origin_x = mat;
    let photo_origin_y = mat;
    let photo_origin = (round_to_u32(photo_origin_x), round_to_u32(photo_origin_y));

    let strip_top = photo_origin_y + photo_height + mat;
    let photo_left_f = photo_origin_x;
    let photo_right_f = photo_origin_x + photo_width;

    let meta = MetaLayout {
        region: (
            round_to_u32(photo_origin_x),
            round_to_u32(strip_top),
            round_to_u32(photo_width),
            round_to_u32(strip_h),
        ),
        photo_left_x: round_to_u32(photo_left_f),
        photo_right_x: round_to_u32(photo_right_f),
        primary_font_height: primary_font as f32,
        secondary_font_height: secondary_font as f32,
        line_gap: line_gap as f32,
    };

    Composition {
        canvas,
        photo_origin,
        photo_size: photo,
        meta: Some(meta),
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "f64 → f32 narrowing is safe for font heights (well below 24-bit precision)"
)]
#[allow(
    clippy::too_many_arguments,
    reason = "internal helper called from one site; bundling parameters into a struct adds boilerplate without making the call site clearer"
)]
fn compose_polaroid(
    photo: (u32, u32),
    photo_width: f64,
    photo_height: f64,
    primary_font: f64,
    secondary_font: f64,
    line_gap: f64,
    mat: f64,
    show_meta: bool,
) -> Composition {
    // Polaroid: photo top-anchored. canvas.W = photo + 2·mat (same
    // as Standard). The bottom band absorbs the role of both the
    // standard layout's "below-photo mat" + "strip" + "below-strip
    // mat" and inflates them into one substantial Polaroid-style
    // border.
    //
    // The canvas is always clearly portrait — for any input photo,
    // `canvas.H ≥ canvas.W + 2·mat`. The "+ 2·mat" overshoot keeps
    // landscape inputs visibly portrait rather than just square. The
    // bottom band has a floor of `4·mat` (the classical Polaroid
    // bottom : side ratio); for landscape inputs the band expands
    // further to meet the portrait constraint.
    let canvas_width = 2.0_f64.mul_add(mat, photo_width);
    let min_bottom = 4.0 * mat;
    let portrait_overshoot = 2.0 * mat;
    let needed_for_portrait = canvas_width + portrait_overshoot - photo_height - mat;
    let bottom_band = needed_for_portrait.max(min_bottom);
    let canvas_height = photo_height + mat + bottom_band;
    let canvas = (round_to_u32(canvas_width), round_to_u32(canvas_height));

    // Photo: top-anchored with `mat` on top and sides. No bottom mat —
    // the bottom_band sits directly below.
    let photo_origin_x = mat;
    let photo_origin_y = mat;
    let photo_origin = (round_to_u32(photo_origin_x), round_to_u32(photo_origin_y));

    if !show_meta {
        // No caption: keep the same Polaroid silhouette, but leave the
        // bottom band as empty mat.
        return Composition {
            canvas,
            photo_origin,
            photo_size: photo,
            meta: None,
        };
    }

    // Bottom band: y ∈ [photo_bottom, canvas_bottom]. The two caption
    // rows centre vertically inside it; primary on top, secondary
    // below with the standard `line_gap`. Text block height =
    // primary + gap + secondary = `2·quantum`, so the symmetric pad
    // above and below is `(bottom_band − 2·quantum) / 2`.
    let band_top = photo_origin_y + photo_height;

    let meta = MetaLayout {
        region: (
            0,
            round_to_u32(band_top),
            canvas.0,
            round_to_u32(bottom_band),
        ),
        photo_left_x: round_to_u32(photo_origin_x),
        photo_right_x: round_to_u32(photo_origin_x + photo_width),
        primary_font_height: primary_font as f32,
        secondary_font_height: secondary_font as f32,
        line_gap: line_gap as f32,
    };

    Composition {
        canvas,
        photo_origin,
        photo_size: photo,
        meta: Some(meta),
    }
}

fn compose_no_meta(
    photo: (u32, u32),
    photo_width: f64,
    photo_height: f64,
    mat: f64,
) -> Composition {
    // No caption: uniform mat on all four sides.
    let canvas_width = 2.0_f64.mul_add(mat, photo_width);
    let canvas_height = 2.0_f64.mul_add(mat, photo_height);
    let canvas = (round_to_u32(canvas_width), round_to_u32(canvas_height));
    let photo_origin = (round_to_u32(mat), round_to_u32(mat));
    Composition {
        canvas,
        photo_origin,
        photo_size: photo,
        meta: None,
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "tests cast bounded float values to u32 for pixel comparisons; bounds are asserted by the surrounding assertions"
)]
mod tests {
    use super::{compute, PHI, QUANTUM_DEPTH};
    use crate::num::round_to_u32;

    fn quantum_for(pw: u32, ph: u32) -> u32 {
        let short = f64::from(pw.min(ph));
        round_to_u32(short / PHI.powi(i32::try_from(QUANTUM_DEPTH).unwrap()))
    }

    fn mat_for(pw: u32, ph: u32) -> u32 {
        // mat = 2·quantum, unified across with-meta and no-meta.
        2 * quantum_for(pw, ph)
    }

    #[test]
    fn canvas_width_is_photo_width_plus_two_mats() {
        for &(pw, ph) in &[
            (3000_u32, 2000_u32),
            (2000, 3000),
            (4000, 1500),
            (1500, 4000),
        ] {
            let c = compute((pw, ph), true, super::LayoutStyle::Standard);
            let mat = mat_for(pw, ph);
            assert!(
                c.canvas.0.abs_diff(pw + 2 * mat) <= 2,
                "({pw},{ph}) canvas.w={}, expected≈{}",
                c.canvas.0,
                pw + 2 * mat,
            );
        }
    }

    #[test]
    fn canvas_height_with_meta_is_photo_plus_three_mats_plus_strip() {
        for &(pw, ph) in &[(3000_u32, 2000_u32), (2000, 3000)] {
            let c = compute((pw, ph), true, super::LayoutStyle::Standard);
            let q = quantum_for(pw, ph);
            // mat = 2q, strip_h = 2q. Vertical: ph + 3·mat + strip_h
            // = ph + 6q + 2q = ph + 8q (±4 px: 8 × round-half-error).
            assert!(
                c.canvas.1.abs_diff(ph + 8 * q) <= 4,
                "({pw},{ph}) canvas.h={}, expected≈{}",
                c.canvas.1,
                ph + 8 * q,
            );
        }
    }

    #[test]
    fn photo_sits_at_mat_offset_top_left() {
        let c = compute((3000, 2000), true, super::LayoutStyle::Standard);
        let mat = mat_for(3000, 2000);
        assert!(c.photo_origin.0.abs_diff(mat) <= 1);
        assert!(c.photo_origin.1.abs_diff(mat) <= 1);
    }

    #[test]
    fn uniform_mat_on_every_interior_boundary() {
        // canvas-left ↔ photo-left, photo-right ↔ canvas-right,
        // canvas-top ↔ photo-top, photo-bottom ↔ strip-top, strip-bottom
        // ↔ canvas-bottom — all should equal mat within 1 px.
        for &(pw, ph) in &[(3000_u32, 2000_u32), (2000, 3000)] {
            let c = compute((pw, ph), true, super::LayoutStyle::Standard);
            let m = c.meta.expect("meta visible");
            let mat = mat_for(pw, ph);

            let left = c.photo_origin.0;
            let right = c.canvas.0 - (c.photo_origin.0 + pw);
            let top = c.photo_origin.1;
            let photo_strip = m.region.1 - (c.photo_origin.1 + ph);
            let strip_bottom = c.canvas.1 - (m.region.1 + m.region.3);

            for (label, gap) in [
                ("left", left),
                ("right", right),
                ("top", top),
                ("photo→strip", photo_strip),
                ("strip→bottom", strip_bottom),
            ] {
                assert!(
                    gap.abs_diff(mat) <= 1,
                    "({pw},{ph}) {label} gap={gap}, expected mat={mat}",
                );
            }
        }
    }

    #[test]
    fn caption_anchors_align_with_photo_edges() {
        let c = compute((3000, 2000), true, super::LayoutStyle::Standard);
        let m = c.meta.expect("meta visible");
        assert_eq!(m.photo_left_x, c.photo_origin.0);
        assert_eq!(m.photo_right_x, c.photo_origin.0 + 3000);
    }

    #[test]
    fn secondary_font_is_primary_over_phi() {
        let c = compute((3000, 2000), true, super::LayoutStyle::Standard);
        let m = c.meta.expect("meta visible");
        let ratio = f64::from(m.primary_font_height) / f64::from(m.secondary_font_height);
        assert!(
            (ratio - PHI).abs() < 0.01,
            "expected primary/secondary = φ, got {ratio}",
        );
    }

    #[test]
    fn primary_font_equals_mat_quantum() {
        let c = compute((3000, 2000), true, super::LayoutStyle::Standard);
        let m = c.meta.expect("meta visible");
        let quantum = quantum_for(3000, 2000);
        let primary = m.primary_font_height.round() as u32;
        assert!(primary.abs_diff(quantum) <= 1);
    }

    #[test]
    fn strip_height_equals_two_mats() {
        for &(pw, ph) in &[(3000_u32, 2000_u32), (2000, 3000)] {
            let c = compute((pw, ph), true, super::LayoutStyle::Standard);
            let m = c.meta.expect("meta visible");
            let mat = quantum_for(pw, ph);
            assert!(
                m.region.3.abs_diff(2 * mat) <= 2,
                "({pw},{ph}) strip_h={}, expected 2·mat={}",
                m.region.3,
                2 * mat,
            );
        }
    }

    #[test]
    fn no_meta_uses_same_uniform_mat() {
        // Mat is `2·quantum` for every layout, so no-meta is just the
        // captioned canvas with the strip + middle mat lifted out.
        // canvas = photo + 2·mat on every side ⇒ +4·quantum per axis.
        for &(pw, ph) in &[(3000_u32, 2000_u32), (2000, 3000)] {
            let c = compute((pw, ph), false, super::LayoutStyle::Standard);
            let q = quantum_for(pw, ph);
            assert!(c.canvas.0.abs_diff(pw + 4 * q) <= 2);
            assert!(c.canvas.1.abs_diff(ph + 4 * q) <= 2);
            assert!(c.meta.is_none());
        }
    }

    #[test]
    fn photo_dominates_canvas() {
        // Anti-米粒 invariant: photo must dominate the framed output.
        // With the unified-mat layout the photo still occupies more
        // than half the canvas for any reasonable input — the mat is
        // intentionally heavy (`2·quantum`) but the photo's own area
        // dwarfs the surrounding frame.
        for &(pw, ph) in &[
            (3000_u32, 2000_u32),
            (2000, 3000),
            (4000, 1500),
            (1500, 4000),
        ] {
            let c = compute((pw, ph), true, super::LayoutStyle::Standard);
            let photo_area = f64::from(pw) * f64::from(ph);
            let canvas_area = f64::from(c.canvas.0) * f64::from(c.canvas.1);
            let ratio = photo_area / canvas_area;
            assert!(
                ratio > 0.5,
                "({pw},{ph}) photo only {:.1}% of canvas",
                ratio * 100.0,
            );
        }
    }

    #[test]
    fn invariants_hold_across_input_sweep() {
        for pw in (100_u32..=4000).step_by(83) {
            for ph in (100_u32..=4000).step_by(73) {
                check_invariants(pw, ph, true);
                check_invariants(pw, ph, false);
            }
        }
    }

    #[test]
    fn every_layout_renders_for_extreme_aspect_ratios() {
        // The realistic edge cases users will throw at the renderer:
        // square, 3:2, 16:9, 3:1 panorama, and their portrait mirrors.
        // Every (aspect × layout × meta) combination must produce a
        // composition that fits the photo within the canvas with
        // non-zero mat on every side and no panic.
        let aspects = [
            ("square", 3000_u32, 3000_u32),
            ("3:2_landscape", 3000, 2000),
            ("2:3_portrait", 2000, 3000),
            ("16:9_widescreen", 3840, 2160),
            ("9:16_vertical", 2160, 3840),
            ("3:1_panorama", 4500, 1500),
            ("1:3_tall_portrait", 1200, 3600),
            ("8:1_extreme_panorama", 4000, 500),
            ("1:8_extreme_tall", 500, 4000),
        ];
        for &(label, pw, ph) in &aspects {
            for &style in &[super::LayoutStyle::Standard, super::LayoutStyle::Polaroid] {
                for show_meta in [true, false] {
                    let c = compute((pw, ph), show_meta, style);
                    assert!(
                        c.canvas.0 > pw,
                        "{label} {style:?} meta={show_meta} canvas.W too small"
                    );
                    assert!(
                        c.canvas.1 > ph,
                        "{label} {style:?} meta={show_meta} canvas.H too small"
                    );
                    assert!(
                        c.photo_origin.0 > 0,
                        "{label} {style:?} meta={show_meta} zero left mat"
                    );
                    assert!(
                        c.photo_origin.1 > 0,
                        "{label} {style:?} meta={show_meta} zero top mat"
                    );
                    assert!(c.photo_origin.0 + pw < c.canvas.0);
                    assert!(c.photo_origin.1 + ph <= c.canvas.1);
                    if style == super::LayoutStyle::Polaroid {
                        assert!(
                            c.canvas.1 > c.canvas.0,
                            "{label} polaroid not portrait: {}×{}",
                            c.canvas.0,
                            c.canvas.1,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn polaroid_canvas_is_always_clearly_portrait() {
        // For any input orientation, Polaroid canvas.H ≥ canvas.W + 2·mat.
        for &(pw, ph) in &[
            (3000_u32, 2000_u32), // landscape
            (2000, 3000),         // portrait
            (4000, 1500),         // panoramic
            (1500, 4000),         // tall portrait
            (2000, 2000),         // square
        ] {
            let c = compute((pw, ph), true, super::LayoutStyle::Polaroid);
            let mat = mat_for(pw, ph);
            assert!(
                c.canvas.1 >= c.canvas.0 + 2 * mat - 2,
                "({pw},{ph}) expected canvas.H ≥ canvas.W + 2·mat, got {}×{}",
                c.canvas.0,
                c.canvas.1,
            );
        }
    }

    #[test]
    fn polaroid_keeps_minimum_bottom_band_for_portrait_input() {
        // Portrait input already produces canvas.H > canvas.W with
        // the default 4·mat bottom; the band shouldn't expand beyond
        // that floor.
        let c = compute((1500, 4000), true, super::LayoutStyle::Polaroid);
        let mat = mat_for(1500, 4000);
        let m = c.meta.expect("polaroid caption");
        assert!(m.region.3.abs_diff(4 * mat) <= 4);
    }

    #[test]
    fn polaroid_expands_bottom_band_for_landscape_input() {
        // Landscape input needs the band to expand to reach
        // canvas.H ≥ canvas.W + 2·mat. The expanded bottom band is
        // larger than the 4·mat floor.
        let c = compute((3000, 2000), true, super::LayoutStyle::Polaroid);
        let mat = mat_for(3000, 2000);
        let m = c.meta.expect("polaroid caption");
        assert!(m.region.3 > 4 * mat, "expected band > 4·mat for landscape");
    }

    #[test]
    fn polaroid_no_meta_keeps_geometry_but_drops_caption() {
        // With or without caption, Polaroid geometry is identical;
        // the bottom band is just empty mat when show_meta = false.
        let with = compute((3000, 2000), true, super::LayoutStyle::Polaroid);
        let without = compute((3000, 2000), false, super::LayoutStyle::Polaroid);
        assert_eq!(with.canvas, without.canvas);
        assert_eq!(with.photo_origin, without.photo_origin);
        assert!(with.meta.is_some());
        assert!(without.meta.is_none());
    }

    fn check_invariants(pw: u32, ph: u32, show_meta: bool) {
        let c = compute((pw, ph), show_meta, super::LayoutStyle::Standard);
        let q = quantum_for(pw, ph);
        // Mat is uniform `2·quantum` across both layouts.
        let mat = 2 * q;

        // canvas.W = photo.W + 2·mat (±2 px rounding tolerance)
        assert!(c.canvas.0.abs_diff(pw + 2 * mat) <= 2);
        // canvas.H = photo.H + (strip + 3 mats) | 2 mats. With caption
        // total is 8·q (8 × round-half ⇒ ±4 px tolerance); no-meta is
        // 4·q (±2 px).
        let (expected_h, tol_h) = if show_meta {
            (ph + 8 * q, 4)
        } else {
            (ph + 2 * mat, 2)
        };
        assert!(c.canvas.1.abs_diff(expected_h) <= tol_h);
        // Photo at (mat, mat) ±1 px
        assert!(c.photo_origin.0.abs_diff(mat) <= 1);
        assert!(c.photo_origin.1.abs_diff(mat) <= 1);
        // Photo fully inside canvas
        assert!(c.photo_origin.0 + pw <= c.canvas.0);
        assert!(c.photo_origin.1 + ph <= c.canvas.1);
        // Meta strip placement
        if let Some(m) = c.meta {
            assert!(m.region.1 > c.photo_origin.1 + ph);
            assert!(m.region.1 + m.region.3 <= c.canvas.1 + 1);
            assert_eq!(m.photo_left_x, c.photo_origin.0);
            assert_eq!(m.photo_right_x, c.photo_origin.0 + pw);
            // Hierarchy
            assert!(m.primary_font_height > m.secondary_font_height);
        }
    }
}
