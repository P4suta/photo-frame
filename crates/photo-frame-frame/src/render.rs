//! End-to-end rendering: downscale → compose canvas → draw caption →
//! return packed RGBA8 [`Pixels`].

use fast_image_resize as fir;
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use photo_frame_types::{Photograph, Pixels};
use rayon::prelude::*;

use crate::format::{caption_from, Caption};
use crate::geometry::{self, Composition, LayoutStyle, MetaLayout};
use crate::num::round_to_u32;
use crate::options::{to_image_rgba, CaptionLayout, FrameOptions, MetaPolicy};
use crate::text::{Renderer, Weight};

#[tracing::instrument(
    level = "info",
    name = "frame_render",
    skip(photo, opts),
    fields(
        photo_width = photo.pixels.width(),
        photo_height = photo.pixels.height(),
        theme = opts.theme.label(),
        layout = opts.layout.label(),
        meta_policy = ?opts.meta_policy,
        max_long_edge = ?opts.max_long_edge,
        canvas_width = tracing::field::Empty,
        canvas_height = tracing::field::Empty,
        caption_visible = tracing::field::Empty,
    ),
)]
pub(crate) fn render(photo: &Photograph, opts: &FrameOptions) -> Pixels {
    let upright = pixels_to_rgba_image(&photo.pixels);
    let upright = maybe_downscale(upright, opts.max_long_edge);

    let caption = match opts.meta_policy {
        MetaPolicy::Never => Caption::default(),
        MetaPolicy::Auto => caption_from(&photo.provenance),
    };
    let caption_visible = !caption.is_empty();

    let style = match opts.layout {
        CaptionLayout::Polaroid => LayoutStyle::Polaroid,
        _ => LayoutStyle::Standard,
    };
    let composition =
        geometry::compute((upright.width(), upright.height()), caption_visible, style);
    let span = tracing::Span::current();
    span.record("canvas_width", composition.canvas.0);
    span.record("canvas_height", composition.canvas.1);
    span.record("caption_visible", caption_visible);

    let canvas = compose_canvas(&composition, &upright, &caption, opts);
    let (w, h) = canvas.dimensions();
    Pixels::from_rgba8(w, h, canvas.into_raw())
        .expect("geometry guarantees a positive RGBA8 canvas")
}

/// Copy the borrowed `Pixels` payload into an `image::RgbaImage` so
/// the rest of the pipeline can drive `image`'s mutating APIs. The
/// copy is unavoidable as long as `ImageBuffer::from_raw` requires an
/// owned `Vec<u8>` — `Pixels` no longer implements `Clone`, so we
/// allocate one shared-ownership-safe buffer here instead of letting
/// callers do it implicitly.
fn pixels_to_rgba_image(pixels: &Pixels) -> RgbaImage {
    ImageBuffer::<Rgba<u8>, _>::from_raw(
        pixels.width(),
        pixels.height(),
        pixels.as_rgba8().to_vec(),
    )
    .expect("Pixels invariants guarantee width * height * 4 bytes")
}

fn maybe_downscale(img: RgbaImage, max_long_edge: Option<u32>) -> RgbaImage {
    let Some(max) = max_long_edge else {
        return img;
    };
    let long = img.width().max(img.height());
    if long <= max {
        return img;
    }
    let ratio = f64::from(max) / f64::from(long);
    let new_w = round_to_u32(f64::from(img.width()) * ratio).max(1);
    let new_h = round_to_u32(f64::from(img.height()) * ratio).max(1);
    tracing::debug!(
        from_w = img.width(),
        from_h = img.height(),
        to_w = new_w,
        to_h = new_h,
        "downscaled"
    );

    // `fast_image_resize` auto-selects SSE/AVX2 (or wasm-SIMD-128 when
    // built with `+simd128`) for the Lanczos3 convolution, and the
    // `rayon` feature splits the destination by rows across worker
    // threads.
    let src = DynamicImage::ImageRgba8(img);
    let mut dst = DynamicImage::ImageRgba8(ImageBuffer::<Rgba<u8>, _>::new(new_w, new_h));
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
    fir::Resizer::new()
        .resize(&src, &mut dst, &options)
        .expect("fir: matched pixel types (RGBA8↔RGBA8) and valid dimensions");
    dst.into_rgba8()
}

fn compose_canvas(
    composition: &Composition,
    photo: &RgbaImage,
    caption: &Caption,
    opts: &FrameOptions,
) -> RgbaImage {
    let (canvas_w, canvas_h) = composition.canvas;
    let mut canvas = build_canvas_with_photo(canvas_w, canvas_h, photo, composition, opts);

    if let Some(ml) = composition.meta.as_ref() {
        let renderer = Renderer::new(to_image_rgba(opts.theme.ink()));
        let photo_w = composition.photo_size.0;
        let metrics = fit_caption(&renderer, caption, opts.layout, ml, photo_w);
        match opts.layout {
            CaptionLayout::Edges => {
                draw_caption_edges(&mut canvas, &renderer, ml, caption, &metrics);
            },
            CaptionLayout::Centered | CaptionLayout::Polaroid => {
                // Polaroid geometry already centres the strip in a
                // thick bottom band — same renderer used for both
                // centred-text layouts.
                draw_caption_centered(&mut canvas, &renderer, ml, caption, &metrics, canvas_w);
            },
        }
    }

    canvas
}

/// Single-pass row-parallel canvas build: each row is independent,
/// so the destination `Vec` is allocated up-front and `rayon`
/// dispatches rows to worker threads. Each row does at most three
/// `memcpy`/`memset` operations (left margin bg, photo bytes, right
/// margin bg); rows above and below the photo are a single
/// `memset`.
fn build_canvas_with_photo(
    canvas_w: u32,
    canvas_h: u32,
    photo: &RgbaImage,
    composition: &Composition,
    opts: &FrameOptions,
) -> RgbaImage {
    let bg_bytes = opts.theme.background().to_array();
    let canvas_stride = (canvas_w as usize) * 4;
    let total_bytes = (canvas_h as usize) * canvas_stride;

    let (photo_origin_x, photo_origin_y) = composition.photo_origin;
    let (photo_width, photo_height) = photo.dimensions();
    let photo_left = photo_origin_x as usize;
    let photo_top = photo_origin_y as usize;
    let photo_bottom = photo_top + (photo_height as usize);
    let photo_byte_stride = (photo_width as usize) * 4;
    let photo_data = photo.as_raw();
    let photo_byte_offset = photo_left * 4;

    // Layout invariant: the geometry layer (`crate::geometry`) only ever
    // produces a canvas big enough to contain the photo rectangle at
    // the given origin. Assert it here so an out-of-bounds origin is a
    // loud panic, not a silent slice OOB inside the parallel block.
    debug_assert!(photo_left + (photo_width as usize) <= canvas_w as usize);
    debug_assert!(photo_bottom <= canvas_h as usize);

    let mut canvas_data: Vec<u8> = vec![0_u8; total_bytes];

    canvas_data
        .par_chunks_mut(canvas_stride)
        .enumerate()
        .for_each(|(y, row)| {
            // Rows outside the photo's vertical extent: pure bg.
            if y < photo_top || y >= photo_bottom {
                fill_row_with_bg(row, bg_bytes);
                return;
            }
            // Inside the photo's vertical extent: left margin bg, photo
            // bytes, right margin bg. Empty margins (photo_x = 0 or
            // photo flush right) skip the corresponding fill.
            if photo_byte_offset > 0 {
                fill_row_with_bg(&mut row[..photo_byte_offset], bg_bytes);
            }
            let src_y = y - photo_top;
            let src_start = src_y * photo_byte_stride;
            row[photo_byte_offset..photo_byte_offset + photo_byte_stride]
                .copy_from_slice(&photo_data[src_start..src_start + photo_byte_stride]);
            let right_start = photo_byte_offset + photo_byte_stride;
            if right_start < row.len() {
                fill_row_with_bg(&mut row[right_start..], bg_bytes);
            }
        });

    RgbaImage::from_raw(canvas_w, canvas_h, canvas_data)
        .expect("canvas buffer length matches canvas_w * canvas_h * 4 by construction")
}

/// Fill `row` (which must be `n * 4` bytes long) with repeating `bg`
/// quartets. `slice::chunks_exact_mut(4)` codegens to a tight memset
/// when the optimiser recognises the constant-fill pattern.
fn fill_row_with_bg(row: &mut [u8], bg: [u8; 4]) {
    for px in row.chunks_exact_mut(4) {
        px.copy_from_slice(&bg);
    }
}

/// Caption metrics after the auto-fit step has decided how big the
/// text actually needs to render. The geometry layer hands us the
/// ideal font heights for the photo's size; here we measure the
/// actual caption strings against the photo's horizontal column and
/// shrink the font proportionally if the ideal sizes would overflow.
/// The vertical positions move with the font so the (now smaller)
/// text block stays centred in the strip.
struct CaptionMetrics {
    primary_font: f32,
    secondary_font: f32,
    primary_y: u32,
    secondary_y: u32,
}

/// Measure the caption against the photo's horizontal column and
/// derive the actual render-time font sizes and y-positions. When
/// the ideal font already fits, returns the geometry layer's values
/// verbatim; when not, scales the font down proportionally so the
/// widest row equals the photo width, and re-centres the smaller
/// text block in the strip.
#[allow(
    clippy::cast_precision_loss,
    reason = "photo widths are small enough that f32 conversion is exact"
)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "positions and font heights are clamped non-negative before casting"
)]
fn fit_caption(
    renderer: &Renderer,
    caption: &Caption,
    layout: CaptionLayout,
    ml: &MetaLayout,
    photo_w: u32,
) -> CaptionMetrics {
    let photo_w_f = photo_w as f32;
    // Minimum visible separator between left- and right-aligned
    // strings in the Edges layout — half the primary font's height,
    // which reads as roughly one character's worth of space at any
    // scale.
    let edges_min_gap = ml.primary_font_height * 0.5;

    let (top_req, bot_req) = match layout {
        CaptionLayout::Edges => {
            let top_l = caption.top_left.as_deref().map_or(0.0, |t| {
                renderer.measure(t, ml.primary_font_height, Weight::Medium)
            });
            let top_r = caption.top_right.as_deref().map_or(0.0, |t| {
                renderer.measure(t, ml.primary_font_height, Weight::Medium)
            });
            let bot_l = caption.bottom_left.as_deref().map_or(0.0, |t| {
                renderer.measure(t, ml.secondary_font_height, Weight::Regular)
            });
            let bot_r = caption.bottom_right.as_deref().map_or(0.0, |t| {
                renderer.measure(t, ml.secondary_font_height, Weight::Regular)
            });
            let top = if top_l > 0.0 && top_r > 0.0 {
                top_l + top_r + edges_min_gap
            } else {
                top_l.max(top_r)
            };
            let bot = if bot_l > 0.0 && bot_r > 0.0 {
                bot_l + bot_r + edges_min_gap
            } else {
                bot_l.max(bot_r)
            };
            (top, bot)
        },
        CaptionLayout::Centered | CaptionLayout::Polaroid => {
            let top = caption.top_combined().as_deref().map_or(0.0, |t| {
                renderer.measure(t, ml.primary_font_height, Weight::Medium)
            });
            let bot = caption.bottom_combined().as_deref().map_or(0.0, |t| {
                renderer.measure(t, ml.secondary_font_height, Weight::Regular)
            });
            (top, bot)
        },
    };

    let max_req = top_req.max(bot_req);
    let scale = if max_req > photo_w_f && max_req > 0.0 {
        photo_w_f / max_req
    } else {
        1.0
    };

    let primary_font = ml.primary_font_height * scale;
    let secondary_font = ml.secondary_font_height * scale;
    let line_gap = ml.line_gap * scale;

    // Recompute vertical text block placement: centre the (possibly
    // smaller) block within the strip rectangle the geometry layer
    // allocated.
    let strip_top = ml.region.1 as f32;
    let strip_h = ml.region.3 as f32;
    let text_block_h = primary_font + line_gap + secondary_font;
    let pad = ((strip_h - text_block_h) * 0.5).max(0.0);
    let primary_y_f = strip_top + pad;
    let secondary_y_f = primary_y_f + primary_font + line_gap;

    CaptionMetrics {
        primary_font,
        secondary_font,
        primary_y: primary_y_f.round() as u32,
        secondary_y: secondary_y_f.round() as u32,
    }
}

fn draw_caption_edges(
    canvas: &mut RgbaImage,
    renderer: &Renderer,
    ml: &MetaLayout,
    caption: &Caption,
    metrics: &CaptionMetrics,
) {
    // Four-corner layout anchored to the photo's left and right edges.
    // The primary row (camera / lens) uses the larger font and
    // medium weight; the secondary row (exposure / date) uses the
    // smaller `primary / φ` font and regular weight. The caption and
    // the photo share a single horizontal column.
    if let Some(text) = caption.top_left.as_deref() {
        renderer.draw_left(
            canvas,
            ml.photo_left_x,
            metrics.primary_y,
            metrics.primary_font,
            Weight::Medium,
            text,
        );
    }
    if let Some(text) = caption.top_right.as_deref() {
        renderer.draw_right(
            canvas,
            ml.photo_right_x,
            metrics.primary_y,
            metrics.primary_font,
            Weight::Medium,
            text,
        );
    }
    if let Some(text) = caption.bottom_left.as_deref() {
        renderer.draw_left(
            canvas,
            ml.photo_left_x,
            metrics.secondary_y,
            metrics.secondary_font,
            Weight::Regular,
            text,
        );
    }
    if let Some(text) = caption.bottom_right.as_deref() {
        renderer.draw_right(
            canvas,
            ml.photo_right_x,
            metrics.secondary_y,
            metrics.secondary_font,
            Weight::Regular,
            text,
        );
    }
}

fn draw_caption_centered(
    canvas: &mut RgbaImage,
    renderer: &Renderer,
    ml: &MetaLayout,
    caption: &Caption,
    metrics: &CaptionMetrics,
    canvas_w: u32,
) {
    let _ = ml; // currently only the metrics drive centred layout
    let cx = canvas_w / 2;
    if let Some(text) = caption.top_combined() {
        renderer.draw_center(
            canvas,
            cx,
            metrics.primary_y,
            metrics.primary_font,
            Weight::Medium,
            &text,
        );
    }
    if let Some(text) = caption.bottom_combined() {
        renderer.draw_center(
            canvas,
            cx,
            metrics.secondary_y,
            metrics.secondary_font,
            Weight::Regular,
            &text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::options::{FrameOptions, MetaPolicy};
    use photo_frame_types::{
        Camera, DateTime, ExifString, Exposure, Fnumber, FocalLengthMm, IsoSensitivity, Photograph,
        Pixels, Provenance, ShutterSeconds,
    };

    fn exif(s: &str) -> ExifString {
        ExifString::new(s.to_owned()).expect("non-empty fixture string")
    }

    fn solid_photo(w: u32, h: u32, provenance: Provenance) -> Photograph {
        let buf = vec![200_u8; (w as usize) * (h as usize) * 4];
        let pixels = Pixels::from_rgba8(w, h, buf).expect("pixels");
        Photograph::new(pixels, provenance)
    }

    const PHI_F64: f64 = 1.618_033_988_749_895;

    fn quantum(pw: u32, ph: u32) -> u32 {
        let short = f64::from(pw.min(ph));
        crate::num::round_to_u32(short / PHI_F64.powi(6))
    }

    #[test]
    fn render_without_caption_canvas_uses_heavier_no_meta_mat() {
        let photo = solid_photo(200, 100, Provenance::default());
        let out = render(&photo, &FrameOptions::default());
        // No caption → mat doubles to 2·quantum, so canvas widens and
        // heightens by 4·quantum per axis (±2 px rounding tolerance).
        let q = quantum(200, 100);
        assert!(out.width().abs_diff(200 + 4 * q) <= 2);
        assert!(out.height().abs_diff(100 + 4 * q) <= 2);
    }

    #[test]
    fn render_with_caption_canvas_includes_strip_and_3_mats() {
        let prov = Provenance {
            camera: Some(Camera {
                make: None,
                model: Some(exif("NIKON Z 5")),
            }),
            ..Default::default()
        };
        let photo = solid_photo(200, 100, prov);
        let out = render(&photo, &FrameOptions::default());
        let q = quantum(200, 100);
        // mat = 2·quantum (uniform). canvas.W = photo + 2·mat = +4·q;
        // canvas.H = photo + 3·mat + strip(= 2·q) = +8·q. ±4 px on H.
        assert!(out.width().abs_diff(200 + 4 * q) <= 2);
        assert!(out.height().abs_diff(100 + 8 * q) <= 4);
    }

    #[test]
    fn render_meta_policy_never_uses_no_meta_canvas_even_with_provenance() {
        let prov = Provenance {
            captured_at: Some(DateTime {
                year: 2026,
                month: 5,
                day: 24,
                ..Default::default()
            }),
            ..Default::default()
        };
        let photo = solid_photo(200, 100, prov);
        let opts = FrameOptions {
            meta_policy: MetaPolicy::Never,
            ..Default::default()
        };
        let out = render(&photo, &opts);
        // Never forces show_meta=false → no strip, heavier mat
        // (2·quantum) replaces the strip's mass: +4·quantum total
        // (±2 px rounding tolerance).
        let q = quantum(200, 100);
        assert!(out.width().abs_diff(200 + 4 * q) <= 2);
        assert!(out.height().abs_diff(100 + 4 * q) <= 2);
    }

    #[test]
    fn render_downscales_when_max_long_edge_is_set() {
        let photo = solid_photo(400, 200, Provenance::default());
        let opts = FrameOptions {
            max_long_edge: Some(100),
            ..Default::default()
        };
        let out = render(&photo, &opts);
        // After downscale 400×200 → 100×50. No caption → 4·quantum
        // per axis (heavier no-meta mat, ±2 px rounding tolerance).
        let q = quantum(100, 50);
        assert!(out.width().abs_diff(100 + 4 * q) <= 2);
        assert!(out.height().abs_diff(50 + 4 * q) <= 2);
    }

    #[test]
    fn render_no_downscale_when_within_budget() {
        let photo = solid_photo(80, 40, Provenance::default());
        let opts = FrameOptions {
            max_long_edge: Some(100),
            ..Default::default()
        };
        let out = render(&photo, &opts);
        // 80×40 already ≤ 100: no downscale. No caption → heavier mat.
        let q = quantum(80, 40);
        assert!(out.width().abs_diff(80 + 4 * q) <= 2);
        assert!(out.height().abs_diff(40 + 4 * q) <= 2);
    }

    #[test]
    fn rendered_buffer_length_matches_dimensions() {
        let photo = solid_photo(80, 60, Provenance::default());
        let out = render(&photo, &FrameOptions::default());
        assert_eq!(
            out.as_rgba8().len(),
            (out.width() * out.height() * 4) as usize
        );
    }

    #[test]
    fn ink_theme_paints_corner_pixels_with_soft_black() {
        use crate::options::FrameTheme;
        use image::Rgba;
        let photo = solid_photo(80, 60, Provenance::default());
        let out = render(
            &photo,
            &FrameOptions {
                theme: FrameTheme::Ink,
                ..Default::default()
            },
        );
        let buf = out.as_rgba8();
        // Top-left and bottom-right of the framed canvas sit on the
        // border (not on the photo), so they must show the Ink fill.
        let top_left = &buf[0..4];
        assert_eq!(
            top_left,
            Rgba([0, 0, 0, 255]).0.as_slice(),
            "ink-theme border must paint black at the canvas corner",
        );
        let last_pixel = (out.width() as usize) * (out.height() as usize) * 4 - 4;
        let bottom_right = &buf[last_pixel..last_pixel + 4];
        assert_eq!(bottom_right, Rgba([0, 0, 0, 255]).0.as_slice());
    }

    #[test]
    fn paper_theme_paints_corner_pixels_with_white() {
        use image::Rgba;
        let photo = solid_photo(80, 60, Provenance::default());
        let out = render(&photo, &FrameOptions::default());
        let buf = out.as_rgba8();
        assert_eq!(&buf[0..4], Rgba([255, 255, 255, 255]).0.as_slice());
    }

    #[test]
    fn caption_with_full_provenance_widens_and_heightens_canvas() {
        let prov = Provenance {
            camera: Some(Camera {
                make: Some(exif("NIKON CORPORATION")),
                model: Some(exif("NIKON Z 5")),
            }),
            exposure: Some(Exposure {
                focal_length_mm: FocalLengthMm::new(50.0),
                aperture: Fnumber::new(1.8),
                shutter_seconds: ShutterSeconds::new(1.0 / 250.0),
                iso: IsoSensitivity::new(200),
            }),
            captured_at: Some(DateTime {
                year: 2026,
                month: 5,
                day: 24,
                ..Default::default()
            }),
            ..Default::default()
        };
        let photo = solid_photo(800, 600, prov);
        let out = render(&photo, &FrameOptions::default());
        let q = quantum(800, 600);
        assert!(out.width().abs_diff(800 + 4 * q) <= 2);
        assert!(out.height().abs_diff(600 + 8 * q) <= 4);
    }
}
