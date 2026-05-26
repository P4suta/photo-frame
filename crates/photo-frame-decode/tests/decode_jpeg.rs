//! End-to-end JPEG decode: bytes in, Photograph out.

use photo_frame_decode::from_bytes;
use photo_frame_types::{Fnumber, IsoSensitivity};

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{build_tiff, jpeg_solid, jpeg_with_app1, Field};

#[test]
fn tiny_jpeg_round_trips_to_photograph() {
    let bytes = jpeg_solid(8, 6);
    let photo = from_bytes(&bytes).expect("decode");
    assert_eq!(photo.pixels.width(), 8);
    assert_eq!(photo.pixels.height(), 6);
    assert_eq!(photo.pixels.as_rgba8().len(), 8 * 6 * 4);
    // No EXIF in the synthesized JPEG → default Provenance.
    assert!(photo.provenance.is_empty());
}

#[test]
fn jpeg_with_full_exif_populates_provenance() {
    let mut body = b"Exif\x00\x00".to_vec();
    body.extend_from_slice(&build_tiff(
        vec![
            Field::ascii(0x010F, "NIKON CORPORATION"),
            Field::ascii(0x0110, "NIKON Z 5"),
            Field::ascii(0x0132, "2026:05:24 10:00:00"),
        ],
        vec![
            Field::ascii(0xA434, "NIKKOR Z 50mm f/1.8 S"),
            Field::rational(0x920A, 50, 1),
            Field::rational(0x829D, 18, 10),
            Field::rational(0x829A, 1, 250),
            Field::short(0x8827, 200),
        ],
    ));
    let bytes = jpeg_with_app1(8, 6, &body);

    let photo = from_bytes(&bytes).expect("decode");
    let prov = &photo.provenance;

    let cam = prov.camera.as_ref().expect("camera");
    assert_eq!(cam.make.as_deref(), Some("NIKON CORPORATION"));
    assert_eq!(cam.model.as_deref(), Some("NIKON Z 5"));

    let lens = prov.lens.as_ref().expect("lens");
    assert_eq!(lens.model.as_deref(), Some("NIKKOR Z 50mm f/1.8 S"));

    let exp = prov.exposure.as_ref().expect("exposure");
    assert_eq!(exp.focal_length_mm, Some(50.0));
    assert_eq!(exp.aperture.map(Fnumber::get), Some(1.8));
    assert!(exp
        .shutter_seconds
        .is_some_and(|v| (v - 1.0 / 250.0).abs() < 1e-9));
    assert_eq!(exp.iso.map(IsoSensitivity::get), Some(200));

    let dt = prov.captured_at.as_ref().expect("datetime");
    assert_eq!((dt.year, dt.month, dt.day), (2026, 5, 24));
}
