//! Test-only builders for synthesizing TIFF / JPEG inputs.
//!
//! Avoiding committed binary fixtures keeps every test self-explanatory:
//! the bytes the parser sees are written right next to the assertions
//! about how it interpreted them. The builder is intentionally minimal —
//! IFD0 / EXIF IFD with a handful of tag types (SHORT / RATIONAL /
//! SRATIONAL / ASCII), enough to drive every fallback chain the decoder
//! exposes.
//!
//! Each integration test binary that includes this file via `#[path]`
//! compiles its own copy and only references the subset of helpers it
//! needs. The module-wide `dead_code` allow exists so a binary that uses
//! only `jpeg_with_app1` doesn't trip on the still-defined `srational`
//! constructor that another binary depends on.

#![allow(
    dead_code,
    reason = "shared builder API; each consumer uses only a subset of helpers"
)]

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, RgbImage};

/// One IFD entry: tag, EXIF type code, count, raw big-endian value bytes.
pub(crate) struct Field {
    pub(crate) tag: u16,
    pub(crate) ty: u16,
    pub(crate) count: u32,
    pub(crate) data: Vec<u8>,
}

impl Field {
    pub(crate) fn short(tag: u16, value: u16) -> Self {
        Self {
            tag,
            ty: 3,
            count: 1,
            data: value.to_be_bytes().to_vec(),
        }
    }

    pub(crate) fn rational(tag: u16, num: u32, denom: u32) -> Self {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&num.to_be_bytes());
        data.extend_from_slice(&denom.to_be_bytes());
        Self {
            tag,
            ty: 5,
            count: 1,
            data,
        }
    }

    pub(crate) fn srational(tag: u16, num: i32, denom: i32) -> Self {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&num.to_be_bytes());
        data.extend_from_slice(&denom.to_be_bytes());
        Self {
            tag,
            ty: 10,
            count: 1,
            data,
        }
    }

    pub(crate) fn ascii(tag: u16, s: &str) -> Self {
        let mut data = s.as_bytes().to_vec();
        data.push(0);
        let count = u32::try_from(data.len()).expect("ascii field fits u32");
        Self {
            tag,
            ty: 2,
            count,
            data,
        }
    }
}

/// Convenience: a bare TIFF carrying only the `Orientation` tag in IFD0.
/// The returned bytes are EXIF body — wrap with `b"Exif\0\0"` to make an
/// APP1 segment payload.
#[must_use]
pub(crate) fn tiff_with_orientation(orientation: u16) -> Vec<u8> {
    let mut body = b"Exif\x00\x00".to_vec();
    body.extend_from_slice(&build_tiff(vec![Field::short(0x0112, orientation)], vec![]));
    body
}

/// Build a minimal big-endian TIFF carrying the given IFD0 and EXIF IFD
/// entries. Output is directly parseable by `exif::Reader::read_raw`.
#[must_use]
pub(crate) fn build_tiff(mut ifd0: Vec<Field>, mut exif: Vec<Field>) -> Vec<u8> {
    ifd0.sort_by_key(|f| f.tag);
    exif.sort_by_key(|f| f.tag);

    let has_exif_ifd = !exif.is_empty();
    let ifd0_entry_count = ifd0.len() + usize::from(has_exif_ifd);
    let ifd0_size = 2 + 12 * ifd0_entry_count + 4;
    let ifd0_offset: u32 = 8;
    let ifd0_ext_start = ifd0_offset + u32::try_from(ifd0_size).unwrap();

    let (ifd0_offsets, ifd0_ext_size) = allocate_externals(&ifd0, ifd0_ext_start);
    let exif_ifd_offset = ifd0_ext_start + ifd0_ext_size;
    let exif_ifd_size = if has_exif_ifd {
        2 + 12 * exif.len() + 4
    } else {
        0
    };
    let exif_ext_start = exif_ifd_offset + u32::try_from(exif_ifd_size).unwrap();
    let (exif_offsets, _) = allocate_externals(&exif, exif_ext_start);

    let mut out = Vec::new();
    out.extend_from_slice(b"MM");
    out.extend_from_slice(&0x002A_u16.to_be_bytes());
    out.extend_from_slice(&ifd0_offset.to_be_bytes());

    let mut ifd0_rendered: Vec<(u16, u16, u32, [u8; 4])> = ifd0
        .iter()
        .zip(&ifd0_offsets)
        .map(|(f, off)| (f.tag, f.ty, f.count, value_field(f, *off)))
        .collect();
    if has_exif_ifd {
        ifd0_rendered.push((0x8769, 4, 1, exif_ifd_offset.to_be_bytes()));
        ifd0_rendered.sort_by_key(|&(tag, _, _, _)| tag);
    }
    write_ifd(&mut out, &ifd0_rendered);
    for (f, off) in ifd0.iter().zip(&ifd0_offsets) {
        if off.is_some() {
            out.extend_from_slice(&f.data);
        }
    }

    if has_exif_ifd {
        let exif_rendered: Vec<(u16, u16, u32, [u8; 4])> = exif
            .iter()
            .zip(&exif_offsets)
            .map(|(f, off)| (f.tag, f.ty, f.count, value_field(f, *off)))
            .collect();
        write_ifd(&mut out, &exif_rendered);
        for (f, off) in exif.iter().zip(&exif_offsets) {
            if off.is_some() {
                out.extend_from_slice(&f.data);
            }
        }
    }
    out
}

fn allocate_externals(fields: &[Field], start: u32) -> (Vec<Option<u32>>, u32) {
    let mut cursor = start;
    let mut offsets = Vec::with_capacity(fields.len());
    for f in fields {
        if f.data.len() <= 4 {
            offsets.push(None);
        } else {
            offsets.push(Some(cursor));
            cursor += u32::try_from(f.data.len()).unwrap();
        }
    }
    (offsets, cursor - start)
}

fn value_field(f: &Field, external_offset: Option<u32>) -> [u8; 4] {
    external_offset.map_or_else(
        || {
            let mut v = [0_u8; 4];
            let n = f.data.len().min(4);
            v[..n].copy_from_slice(&f.data[..n]);
            v
        },
        u32::to_be_bytes,
    )
}

fn write_ifd(out: &mut Vec<u8>, entries: &[(u16, u16, u32, [u8; 4])]) {
    let count = u16::try_from(entries.len()).expect("IFD entry count fits u16");
    out.extend_from_slice(&count.to_be_bytes());
    for (tag, ty, ct, val) in entries {
        out.extend_from_slice(&tag.to_be_bytes());
        out.extend_from_slice(&ty.to_be_bytes());
        out.extend_from_slice(&ct.to_be_bytes());
        out.extend_from_slice(val);
    }
    out.extend_from_slice(&0_u32.to_be_bytes());
}

/// A minimal valid JPEG of size `w × h` filled with a solid colour.
#[must_use]
pub(crate) fn jpeg_solid(w: u32, h: u32) -> Vec<u8> {
    let solid = RgbImage::from_pixel(w, h, image::Rgb([200, 60, 60]));
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .write_image(&solid, w, h, ExtendedColorType::Rgb8)
        .expect("jpeg encode");
    out
}

/// A JPEG of size `w × h` carrying an APP1 segment with the given EXIF
/// body (everything that lives between the `FF E1 <length>` header and
/// the next marker — i.e. the `Exif\0\0` identifier plus the TIFF blob).
#[must_use]
pub(crate) fn jpeg_with_app1(w: u32, h: u32, exif_body: &[u8]) -> Vec<u8> {
    let jpeg = jpeg_solid(w, h);
    let mut out = Vec::with_capacity(jpeg.len() + exif_body.len() + 4);
    out.extend_from_slice(&jpeg[..2]); // SOI
    out.push(0xFF);
    out.push(0xE1); // APP1
    let segment_len = u16::try_from(exif_body.len() + 2).expect("APP1 length fits u16");
    out.extend_from_slice(&segment_len.to_be_bytes());
    out.extend_from_slice(exif_body);
    out.extend_from_slice(&jpeg[2..]);
    out
}
