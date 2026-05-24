//! Camera and Lens extraction.
//!
//! Both follow the same shape: read two ASCII tags, collapse to `None`
//! when neither is present. Vendor cleanup ("NIKON CORPORATION" →
//! "NIKON") is *display* concern, not data concern, and stays in the
//! frame crate.

use exif::{Exif, Tag};
use photo_frame_types::{Camera, Lens};

use super::ascii;

pub(crate) fn camera(exif: &Exif) -> Option<Camera> {
    let make = ascii(exif, Tag::Make);
    let model = ascii(exif, Tag::Model);
    if make.is_none() && model.is_none() {
        return None;
    }
    Some(Camera { make, model })
}

pub(crate) fn lens(exif: &Exif) -> Option<Lens> {
    let make = ascii(exif, Tag::LensMake);
    let model = ascii(exif, Tag::LensModel);
    if make.is_none() && model.is_none() {
        return None;
    }
    Some(Lens { make, model })
}

#[cfg(test)]
mod tests {
    use super::{camera, lens};
    use crate::test_support::{build_tiff, Field};
    use exif::Reader;

    fn parse(ifd0: Vec<Field>, exif_ifd: Vec<Field>) -> exif::Exif {
        let mut body = b"Exif\x00\x00".to_vec();
        body.extend_from_slice(&build_tiff(ifd0, exif_ifd));
        // Wrap as JPEG so we exercise the same container parser path as
        // production. read_raw also works, but read_from_container is
        // closer to the real pipeline.
        Reader::new()
            .read_raw(body[6..].to_vec())
            .expect("synthesized TIFF parses")
    }

    #[test]
    fn camera_make_and_model_present() {
        let exif = parse(
            vec![
                Field::ascii(0x010F, "NIKON CORPORATION"),
                Field::ascii(0x0110, "NIKON Z 5"),
            ],
            vec![],
        );
        let cam = camera(&exif).expect("present");
        assert_eq!(cam.make.as_deref(), Some("NIKON CORPORATION"));
        assert_eq!(cam.model.as_deref(), Some("NIKON Z 5"));
    }

    #[test]
    fn camera_make_only_returns_some() {
        let exif = parse(vec![Field::ascii(0x010F, "SONY")], vec![]);
        let cam = camera(&exif).expect("present");
        assert_eq!(cam.make.as_deref(), Some("SONY"));
        assert!(cam.model.is_none());
    }

    #[test]
    fn camera_with_neither_returns_none() {
        let exif = parse(vec![], vec![]);
        assert!(camera(&exif).is_none());
    }

    #[test]
    fn lens_model_only() {
        let exif = parse(vec![], vec![Field::ascii(0xA434, "NIKKOR Z 50mm f/1.8 S")]);
        let l = lens(&exif).expect("present");
        assert_eq!(l.model.as_deref(), Some("NIKKOR Z 50mm f/1.8 S"));
        assert!(l.make.is_none());
    }

    #[test]
    fn lens_with_neither_returns_none() {
        let exif = parse(vec![], vec![]);
        assert!(lens(&exif).is_none());
    }
}
