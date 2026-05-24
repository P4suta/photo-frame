//! Caption text rasterization on an RGBA8 canvas.
//!
//! The font is statically embedded so the renderer has no filesystem
//! dependency at runtime — important for the WASM target and convenient
//! for single-binary CLI distribution.
//!
//! Ink colour is decided per-renderer instance (not a `const`) so a
//! given canvas can pair the right contrast with its frame theme — see
//! [`crate::options::FrameTheme`].

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;

use crate::num::round_to_u32_f32;

// Geist Sans (Vercel, OFL 1.1) — bundled verbatim from the upstream
// release. Filenames, internal name table, and accompanying license /
// authorship files are all unchanged from the v1.7.1 source distribution
// at https://github.com/vercel/geist-font.
const REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist/Geist-Regular.otf");
const MEDIUM_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist/Geist-Medium.otf");

#[derive(Copy, Clone, Debug)]
pub(crate) enum Weight {
    Regular,
    Medium,
}

#[derive(Debug)]
pub(crate) struct Renderer {
    regular: FontRef<'static>,
    medium: FontRef<'static>,
    ink: Rgba<u8>,
}

impl Renderer {
    pub(crate) fn new(ink: Rgba<u8>) -> Self {
        Self {
            regular: FontRef::try_from_slice(REGULAR_BYTES)
                .expect("embedded Geist-Regular.otf must parse"),
            medium: FontRef::try_from_slice(MEDIUM_BYTES)
                .expect("embedded Geist-Medium.otf must parse"),
            ink,
        }
    }

    /// Draw `text` left-aligned, with its top edge at `top_y` and its
    /// left edge at `x`.
    pub(crate) fn draw_left(
        &self,
        canvas: &mut RgbaImage,
        x: u32,
        top_y: u32,
        font_height: f32,
        weight: Weight,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        let font = self.font_for(weight);
        let scale = PxScale::from(font_height);
        draw_text_mut(
            canvas,
            self.ink,
            to_i32(x),
            to_i32(top_y),
            scale,
            font,
            text,
        );
    }

    /// Draw `text` right-aligned so that its right edge lands at
    /// `right_x`. Overflow on the left clamps at `0` rather than
    /// panicking.
    pub(crate) fn draw_right(
        &self,
        canvas: &mut RgbaImage,
        right_x: u32,
        top_y: u32,
        font_height: f32,
        weight: Weight,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        let font = self.font_for(weight);
        let scale = PxScale::from(font_height);
        let width = round_to_u32_f32(text_width(font, scale, text));
        let x = to_i32(right_x).saturating_sub(to_i32(width)).max(0);
        draw_text_mut(canvas, self.ink, x, to_i32(top_y), scale, font, text);
    }

    /// Draw `text` horizontally centred on `center_x`. Overflow on the
    /// left clamps at `0` (matches [`Self::draw_right`]).
    pub(crate) fn draw_center(
        &self,
        canvas: &mut RgbaImage,
        center_x: u32,
        top_y: u32,
        font_height: f32,
        weight: Weight,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        let font = self.font_for(weight);
        let scale = PxScale::from(font_height);
        let width = round_to_u32_f32(text_width(font, scale, text));
        let half = i32::try_from(width / 2).unwrap_or(i32::MAX);
        let x = to_i32(center_x).saturating_sub(half).max(0);
        draw_text_mut(canvas, self.ink, x, to_i32(top_y), scale, font, text);
    }

    const fn font_for(&self, weight: Weight) -> &FontRef<'static> {
        match weight {
            Weight::Regular => &self.regular,
            Weight::Medium => &self.medium,
        }
    }
}

/// Saturating `u32` → `i32`. Canvas dimensions are bounded by available
/// memory, so wraparound past `i32::MAX` is purely theoretical.
fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// Sum of glyph advances — close enough to layout width for the caption's
/// short, predominantly Latin strings. Kerning is not applied.
fn text_width<F: Font>(font: &F, scale: PxScale, text: &str) -> f32 {
    let scaled = font.as_scaled(scale);
    text.chars()
        .map(|c| scaled.h_advance(font.glyph_id(c)))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{Renderer, Weight};
    use image::{Rgba, RgbaImage};

    #[test]
    fn renderer_constructs_without_panicking() {
        let _ = Renderer::new(Rgba([60, 60, 60, 255]));
    }

    /// Pixel-true symmetry check: rendering the same text centred at
    /// `cx = w/2` on a left-edge canvas and on a right-edge canvas should
    /// produce mirror images. Picking a fairly tight canvas keeps the
    /// test cheap.
    #[test]
    fn draw_center_is_horizontally_symmetric() {
        let ink = Rgba([0, 0, 0, 255]);
        let renderer = Renderer::new(ink);
        let mut canvas = RgbaImage::from_pixel(400, 32, Rgba([255, 255, 255, 255]));
        renderer.draw_center(&mut canvas, 200, 4, 20.0, Weight::Regular, "abc");
        // Find the leftmost and rightmost columns that contain ink.
        let mut left = None;
        let mut right = None;
        for x in 0..canvas.width() {
            for y in 0..canvas.height() {
                if canvas.get_pixel(x, y)[0] < 250 {
                    left.get_or_insert(x);
                    right = Some(x);
                    break;
                }
            }
        }
        let (l, r) = (left.expect("ink present"), right.expect("ink present"));
        let gap_l = l;
        let gap_r = canvas.width() - 1 - r;
        // Allow 1 px of slop for sub-pixel rounding inside the glyph
        // rasterizer — kerning is not applied so true symmetry is
        // already approximate.
        assert!(
            gap_l.abs_diff(gap_r) <= 1,
            "expected symmetric gutters around centre 200, got left={gap_l} right={gap_r}",
        );
    }
}
