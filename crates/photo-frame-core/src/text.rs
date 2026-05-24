//! Caption text rasterization.
//!
//! The font is statically embedded so the core has no filesystem dependency at
//! runtime — important for the WASM target, but also helpful in the CLI case
//! (single-binary distribution).

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;

use crate::num::round_to_u32_f32;

// Geist Sans (Vercel, OFL 1.1) — bundled verbatim from the upstream release.
// Filenames, internal name table, and accompanying license / authorship files
// are all unchanged from the v1.7.1 source distribution at
// https://github.com/vercel/geist-font.
const REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist/Geist-Regular.otf");
const MEDIUM_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist/Geist-Medium.otf");

/// Soft ink — pure black floats above paper-white, this sits flush.
const INK: Rgb<u8> = Rgb([60, 60, 60]);

/// Text weight selector. Top caption line uses [`Weight::Medium`], the
/// exposure line uses [`Weight::Regular`].
#[derive(Copy, Clone, Debug)]
pub(crate) enum Weight {
    Regular,
    Medium,
}

/// Holds parsed font handles. Cheap to construct; designed to be created once
/// per `frame_image` invocation rather than per glyph.
#[derive(Debug)]
pub(crate) struct Renderer {
    regular: FontRef<'static>,
    medium: FontRef<'static>,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        Self {
            regular: FontRef::try_from_slice(REGULAR_BYTES)
                .expect("embedded Geist-Regular.otf must parse"),
            medium: FontRef::try_from_slice(MEDIUM_BYTES)
                .expect("embedded Geist-Medium.otf must parse"),
        }
    }

    /// Draw `text` left-aligned, with its top edge at `top_y` and its left edge
    /// at `x`.
    pub(crate) fn draw_left(
        &self,
        canvas: &mut RgbImage,
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
        draw_text_mut(canvas, INK, to_i32(x), to_i32(top_y), scale, font, text);
    }

    /// Draw `text` right-aligned so that its right edge lands at `right_x`. If
    /// the text would overflow the left edge of the canvas, it clips at `0`
    /// rather than panicking.
    pub(crate) fn draw_right(
        &self,
        canvas: &mut RgbImage,
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
        draw_text_mut(canvas, INK, x, to_i32(top_y), scale, font, text);
    }

    const fn font_for(&self, weight: Weight) -> &FontRef<'static> {
        match weight {
            Weight::Regular => &self.regular,
            Weight::Medium => &self.medium,
        }
    }
}

/// Saturating `u32` → `i32` for canvas coordinates. Canvas dimensions are
/// bounded by available memory, so wraparound past `i32::MAX` is purely
/// theoretical, but clamping is cheap.
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
    use super::Renderer;

    #[test]
    fn renderer_constructs_without_panicking() {
        let _ = Renderer::new();
    }
}
