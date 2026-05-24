//! Property-based fuzz tests for `photo_frame_decode::from_bytes`.
//!
//! Goal: no matter what we throw at the decoder, it returns either
//! `Ok(Photograph)` or a typed `DecodeError`. Panics, aborts, or any
//! other process-killing behaviour are bugs.
//!
//! These tests are not exhaustive — they're a defence against the
//! corruption-style bugs that unit tests rarely cover. Each `proptest!`
//! block runs 256 cases by default; the slowest (PNG/TIFF round-trip
//! through full decode) finishes in well under a second.

use photo_frame_decode::from_bytes;
use proptest::prelude::*;

proptest! {
    // Random byte slices of varied length should never panic. The
    // decoder will reject most of them (UnknownFormat / Decode error)
    // — that's the expected, well-behaved path.
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    #[test]
    fn from_bytes_never_panics_on_random_input(
        bytes in proptest::collection::vec(any::<u8>(), 0..=1024)
    ) {
        // We don't care what the result is — only that it didn't
        // panic. `from_bytes` returning Err is fine; that's exactly
        // what UnknownFormat is for.
        let _ = from_bytes(&bytes);
    }

    #[test]
    fn from_bytes_never_panics_on_truncated_jpeg(
        keep in 0usize..200
    ) {
        // A real JPEG SOI marker followed by random length of zero
        // bytes — a common shape for truncated downloads. The
        // decoder should reject with Decode(_), never abort.
        let mut bytes = vec![0xFF, 0xD8, 0xFF];
        bytes.extend(std::iter::repeat(0u8).take(keep));
        let _ = from_bytes(&bytes);
    }

    #[test]
    fn from_bytes_never_panics_on_truncated_png(
        keep in 0usize..200
    ) {
        // PNG 8-byte signature + random tail.
        let mut bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend(std::iter::repeat(0u8).take(keep));
        let _ = from_bytes(&bytes);
    }

    #[test]
    fn from_bytes_never_panics_on_heic_like_ftyp(
        brand_idx in 0usize..12,
        tail in proptest::collection::vec(any::<u8>(), 0..=128),
    ) {
        // Random ftyp box payload — the format detector will route
        // these to the HEIC path, which will return
        // HeifFeatureDisabled (default features) or HeifDecode
        // (--features heif). Either is fine; neither should panic.
        const BRANDS: [&[u8; 4]; 12] = [
            b"heic", b"heix", b"hevc", b"hevx", b"heim", b"heis",
            b"hevm", b"hevs", b"mif1", b"msf1", b"avif", b"abcd",
        ];
        let mut bytes = vec![0, 0, 0, 0x20];
        bytes.extend_from_slice(b"ftyp");
        bytes.extend_from_slice(BRANDS[brand_idx]);
        bytes.extend(tail);
        let _ = from_bytes(&bytes);
    }

    #[test]
    fn from_bytes_empty_always_returns_empty_input(
        // Generate "almost empty" cases too — single zero byte,
        // single FF byte, etc — to exercise the EmptyInput / Unknown
        // boundary.
        prefix_len in 0usize..=3,
    ) {
        use photo_frame_decode::DecodeError;
        let bytes = vec![0u8; prefix_len];
        let result = from_bytes(&bytes);
        match (prefix_len, result) {
            (0, Err(DecodeError::EmptyInput)) => {}
            (0, other) => prop_assert!(false, "0-byte input must be EmptyInput, got {other:?}"),
            (_, Err(_)) => {}  // any error is fine for nonzero junk
            (_, Ok(_)) => prop_assert!(false, "3-byte zero blob should not decode"),
        }
    }
}
