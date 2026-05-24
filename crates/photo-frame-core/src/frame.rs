//! Canvas composition: combine background, photo, and caption into a single
//! RGB image ready for JPEG encoding.

use image::{imageops, DynamicImage, Rgb, RgbImage};

use crate::exif::Meta;
use crate::geometry::{Layout, MetaLayout};
use crate::options::Background;
use crate::text::{Renderer, Weight};

#[must_use]
pub(crate) fn render(
    layout: &Layout,
    photo: &DynamicImage,
    meta: Option<&Meta>,
    bg: Background,
) -> RgbImage {
    let (canvas_w, canvas_h) = layout.canvas_size;
    let mut canvas = RgbImage::from_pixel(canvas_w, canvas_h, Rgb(bg.rgb()));

    let photo_rgb = photo.to_rgb8();
    imageops::replace(
        &mut canvas,
        &photo_rgb,
        i64::from(layout.photo_origin.0),
        i64::from(layout.photo_origin.1),
    );

    if let (Some(ml), Some(meta)) = (layout.meta.as_ref(), meta) {
        let renderer = Renderer::new();
        draw_caption(&mut canvas, &renderer, ml, meta, canvas_w);
    }

    canvas
}

fn draw_caption(
    canvas: &mut RgbImage,
    renderer: &Renderer,
    ml: &MetaLayout,
    meta: &Meta,
    canvas_w: u32,
) {
    let right_x = canvas_w.saturating_sub(ml.pad_x);

    // Top row: camera body (left, medium weight) and lens (right).
    if let Some(camera) = meta.camera.as_deref() {
        renderer.draw_left(
            canvas,
            ml.pad_x,
            ml.top_line_y,
            ml.font_height,
            Weight::Medium,
            camera,
        );
    }
    if let Some(lens) = meta.lens.as_deref() {
        renderer.draw_right(
            canvas,
            right_x,
            ml.top_line_y,
            ml.font_height,
            Weight::Medium,
            lens,
        );
    }

    // Bottom row: exposure facts (left, regular) and capture date (right).
    if let Some(expo) = meta.exposure_line() {
        renderer.draw_left(
            canvas,
            ml.pad_x,
            ml.bottom_line_y,
            ml.font_height,
            Weight::Regular,
            &expo,
        );
    }
    if let Some(date) = meta.date.as_deref() {
        renderer.draw_right(
            canvas,
            right_x,
            ml.bottom_line_y,
            ml.font_height,
            Weight::Regular,
            date,
        );
    }
}
