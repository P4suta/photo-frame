//! End-to-end rendering: downscale → compose canvas → draw caption →
//! return packed RGBA8 [`Pixels`].

use image::{imageops, imageops::FilterType, DynamicImage, ImageBuffer, Rgba, RgbaImage};
use photo_frame_types::{Photograph, Pixels};

use crate::format::{caption_from, Caption};
use crate::geometry::{self, Layout, MetaLayout};
use crate::num::round_to_u32;
use crate::options::{CaptionLayout, FrameOptions, MetaPolicy};
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

    let layout = geometry::compute((upright.width(), upright.height()), caption_visible);
    let span = tracing::Span::current();
    span.record("canvas_width", layout.canvas_size.0);
    span.record("canvas_height", layout.canvas_size.1);
    span.record("caption_visible", caption_visible);

    let canvas = compose_canvas(&layout, &upright, &caption, opts);
    let (w, h) = canvas.dimensions();
    Pixels::from_rgba8(w, h, canvas.into_raw())
        .expect("geometry guarantees a positive RGBA8 canvas")
}

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
    DynamicImage::ImageRgba8(img)
        .resize(new_w, new_h, FilterType::Lanczos3)
        .to_rgba8()
}

fn compose_canvas(
    layout: &Layout,
    photo: &RgbaImage,
    caption: &Caption,
    opts: &FrameOptions,
) -> RgbaImage {
    let (canvas_w, canvas_h) = layout.canvas_size;
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, opts.theme.background());

    imageops::replace(
        &mut canvas,
        photo,
        i64::from(layout.photo_origin.0),
        i64::from(layout.photo_origin.1),
    );

    if let Some(ml) = layout.meta.as_ref() {
        let renderer = Renderer::new(opts.theme.ink());
        match opts.layout {
            CaptionLayout::Edges => draw_caption_edges(&mut canvas, &renderer, ml, caption, canvas_w),
            CaptionLayout::Centered => draw_caption_centered(&mut canvas, &renderer, ml, caption, canvas_w),
        }
    }

    canvas
}

fn draw_caption_edges(
    canvas: &mut RgbaImage,
    renderer: &Renderer,
    ml: &MetaLayout,
    caption: &Caption,
    canvas_w: u32,
) {
    let right_x = canvas_w.saturating_sub(ml.pad_x);

    if let Some(text) = caption.top_left.as_deref() {
        renderer.draw_left(
            canvas,
            ml.pad_x,
            ml.top_line_y,
            ml.font_height,
            Weight::Medium,
            text,
        );
    }
    if let Some(text) = caption.top_right.as_deref() {
        renderer.draw_right(
            canvas,
            right_x,
            ml.top_line_y,
            ml.font_height,
            Weight::Medium,
            text,
        );
    }
    if let Some(text) = caption.bottom_left.as_deref() {
        renderer.draw_left(
            canvas,
            ml.pad_x,
            ml.bottom_line_y,
            ml.font_height,
            Weight::Regular,
            text,
        );
    }
    if let Some(text) = caption.bottom_right.as_deref() {
        renderer.draw_right(
            canvas,
            right_x,
            ml.bottom_line_y,
            ml.font_height,
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
    canvas_w: u32,
) {
    let cx = canvas_w / 2;
    if let Some(text) = caption.top_combined() {
        renderer.draw_center(
            canvas,
            cx,
            ml.top_line_y,
            ml.font_height,
            Weight::Medium,
            &text,
        );
    }
    if let Some(text) = caption.bottom_combined() {
        renderer.draw_center(
            canvas,
            cx,
            ml.bottom_line_y,
            ml.font_height,
            Weight::Regular,
            &text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::options::{FrameOptions, MetaPolicy};
    use photo_frame_types::{Camera, DateTime, Exposure, Photograph, Pixels, Provenance};

    fn solid_photo(w: u32, h: u32, provenance: Provenance) -> Photograph {
        let buf = vec![200_u8; (w as usize) * (h as usize) * 4];
        let pixels = Pixels::from_rgba8(w, h, buf).expect("pixels");
        Photograph::new(pixels, provenance)
    }

    #[test]
    fn render_without_caption_uses_symmetric_thin_border() {
        let photo = solid_photo(200, 100, Provenance::default());
        let opts = FrameOptions::default();
        let out = render(&photo, &opts);
        // side = side_for(min(200,100)) = side_for(100) = max(round(100/φ⁶), 8) = 8
        // bottom collapses to side when caption empty → canvas = (200+16, 100+16) = (216, 116)
        assert_eq!(out.width(), 216);
        assert_eq!(out.height(), 116);
    }

    #[test]
    fn render_with_caption_grows_bottom_strip() {
        let prov = Provenance {
            camera: Some(Camera {
                make: None,
                model: Some("NIKON Z 5".into()),
            }),
            ..Default::default()
        };
        let photo = solid_photo(200, 100, prov);
        let opts = FrameOptions::default();
        let out = render(&photo, &opts);
        // side = 8, bottom = round(8·φ²) = 21 (instead of 8 collapse) → h = 100+8+21 = 129
        assert_eq!(out.width(), 216);
        assert_eq!(out.height(), 129);
    }

    #[test]
    fn render_meta_policy_never_collapses_strip_even_with_provenance() {
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
        assert_eq!(out.height(), 116, "bottom strip must collapse with Never");
    }

    #[test]
    fn render_downscales_when_max_long_edge_is_set() {
        let photo = solid_photo(400, 200, Provenance::default());
        let opts = FrameOptions {
            max_long_edge: Some(100),
            ..Default::default()
        };
        let out = render(&photo, &opts);
        // After downscale: long edge clamped to 100, short edge halved → 100×50.
        // side_for(50) = max(round(50/φ⁶), 8) = 8 → canvas = (100+16, 50+16) = (116, 66)
        assert_eq!(out.width(), 116);
        assert_eq!(out.height(), 66);
    }

    #[test]
    fn render_no_downscale_when_within_budget() {
        let photo = solid_photo(80, 40, Provenance::default());
        let opts = FrameOptions {
            max_long_edge: Some(100),
            ..Default::default()
        };
        let out = render(&photo, &opts);
        // 80x40 already <= 100 long edge: pass through.
        // side_for(40) = max(round(40/φ⁶), 8) = 8 → canvas = (80+16, 40+16) = (96, 56)
        assert_eq!(out.width(), 96);
        assert_eq!(out.height(), 56);
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
            Rgba([0x1A, 0x1A, 0x1A, 255]).0.as_slice(),
            "ink-theme border must paint soft-black at the canvas corner",
        );
        let last_pixel = (out.width() as usize) * (out.height() as usize) * 4 - 4;
        let bottom_right = &buf[last_pixel..last_pixel + 4];
        assert_eq!(bottom_right, Rgba([0x1A, 0x1A, 0x1A, 255]).0.as_slice());
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
    fn caption_with_full_provenance_renders_larger_strip() {
        let prov = Provenance {
            camera: Some(Camera {
                make: Some("NIKON CORPORATION".into()),
                model: Some("NIKON Z 5".into()),
            }),
            exposure: Some(Exposure {
                focal_length_mm: Some(50.0),
                aperture: Some(1.8),
                shutter_seconds: Some(1.0 / 250.0),
                iso: Some(200),
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
        // We don't pin exact dimensions here (geometry tests cover that);
        // we just verify the output is sane and the strip expanded.
        assert!(out.width() > 800);
        assert!(out.height() > 600 + 16);
    }
}
