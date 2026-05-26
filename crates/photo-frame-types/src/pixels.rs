use miette::Diagnostic;
use thiserror::Error;

use crate::{Categorize, Category};

/// Constructor-time invariants for [`Pixels`].
///
/// Both variants are caller-side programming errors (the producer
/// handed us an inconsistent buffer / dimension pair). They surface as
/// `Category::Internal` so the CLI exits with code 1 by convention —
/// the user can't directly fix a producer bug, but the diagnostic
/// `code` and `help` make it actionable for the developer.
#[derive(Debug, Error, Diagnostic)]
pub enum PixelError {
    #[error("pixel buffer length {got} does not match {width}x{height}x4 = {expected}")]
    #[diagnostic(
        code(photo_frame::types::pixels::size_mismatch),
        help(
            "The decoder must hand back exactly width*height*4 bytes of RGBA8 data. \
             Verify the producer (decode crate, custom Pixels::from_rgba8 call) \
             is computing buffer length consistently with the declared dimensions."
        )
    )]
    /// The buffer is the wrong size for the declared `width × height × 4`
    /// RGBA8 layout. Reported when the producer (decoder, raw constructor)
    /// hands back a buffer whose length doesn't match the dimensions.
    DataSizeMismatch {
        /// Declared image width in pixels.
        width: u32,
        /// Declared image height in pixels.
        height: u32,
        /// Actual byte length of the data buffer the caller supplied.
        got: usize,
        /// Byte length the buffer was required to have: `width * height * 4`.
        expected: usize,
    },

    #[error("dimensions must be non-zero (got {width}x{height})")]
    #[diagnostic(
        code(photo_frame::types::pixels::zero_dimension),
        help(
            "Pixels with zero width or height are meaningless. \
             A 0×0 buffer usually indicates a decode bug or an upstream \
             validation slip — check the producer."
        )
    )]
    /// Either dimension is zero. A 0×0 (or 0×N, N×0) pixel grid is
    /// meaningless; this typically signals a producer bug.
    ZeroDimension {
        /// Declared image width in pixels (one of `width` / `height` is zero).
        width: u32,
        /// Declared image height in pixels (one of `width` / `height` is zero).
        height: u32,
    },
}

impl Categorize for PixelError {
    fn category(&self) -> Category {
        // PixelError always represents a producer-side invariant
        // violation — neither variant is the user's fault.
        Category::Internal
    }
}

/// Owned RGBA8 pixel grid, row-major.
///
/// Invariants enforced at construction:
/// - `width > 0`, `height > 0`
/// - `data.len() == width * height * 4`
///
/// The grid is the canonical pixel container used everywhere downstream of
/// decode. Orientation is always already applied — the producer (decoder)
/// is responsible for handing back a `Pixels` whose `(0, 0)` is the
/// scene's top-left as a viewer would see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Pixels {
    /// Construct from an RGBA8 buffer. Takes ownership.
    ///
    /// # Errors
    /// [`PixelError::ZeroDimension`] for zero-sized inputs, or
    /// [`PixelError::DataSizeMismatch`] when `data.len()` disagrees with
    /// the declared dimensions.
    pub fn from_rgba8(width: u32, height: u32, data: Vec<u8>) -> Result<Self, PixelError> {
        Self::ensure_nonzero(width, height)?;
        let expected = expected_rgba8_len(width, height);
        if data.len() != expected {
            return Err(PixelError::DataSizeMismatch {
                width,
                height,
                got: data.len(),
                expected,
            });
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Construct from an RGB8 buffer by allocating a new RGBA8 buffer with
    /// alpha = 0xFF for every pixel.
    ///
    /// # Errors
    /// Same shape as [`Self::from_rgba8`], but the size check is against
    /// `width * height * 3`.
    pub fn from_rgb8(width: u32, height: u32, rgb: &[u8]) -> Result<Self, PixelError> {
        Self::ensure_nonzero(width, height)?;
        let expected_rgb = (width as usize) * (height as usize) * 3;
        if rgb.len() != expected_rgb {
            return Err(PixelError::DataSizeMismatch {
                width,
                height,
                got: rgb.len(),
                expected: expected_rgb,
            });
        }
        let pixel_count = (width as usize) * (height as usize);
        let mut data = Vec::with_capacity(pixel_count * 4);
        for chunk in rgb.chunks_exact(3) {
            data.extend_from_slice(chunk);
            data.push(0xFF);
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// `(width, height)` tuple, often nicer at call sites than two
    /// individual reads.
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// RGBA8 buffer view, row-major.
    #[must_use]
    pub fn as_rgba8(&self) -> &[u8] {
        &self.data
    }

    /// Consume `self` and return the underlying RGBA8 buffer along with the
    /// dimensions. Useful when downstream code wants to construct another
    /// image library's type without an extra allocation.
    #[must_use]
    pub fn into_parts(self) -> (u32, u32, Vec<u8>) {
        (self.width, self.height, self.data)
    }

    const fn ensure_nonzero(width: u32, height: u32) -> Result<(), PixelError> {
        if width == 0 || height == 0 {
            Err(PixelError::ZeroDimension { width, height })
        } else {
            Ok(())
        }
    }
}

const fn expected_rgba8_len(width: u32, height: u32) -> usize {
    (width as usize) * (height as usize) * 4
}

#[cfg(test)]
mod tests {
    use super::{PixelError, Pixels};

    #[test]
    fn rgba8_roundtrip_through_constructor() {
        let data = vec![0_u8; 2 * 3 * 4];
        let p = Pixels::from_rgba8(2, 3, data.clone()).unwrap();
        assert_eq!(p.dimensions(), (2, 3));
        assert_eq!(p.as_rgba8(), &data[..]);
    }

    #[test]
    fn rgb8_constructor_pads_alpha() {
        let rgb = vec![10, 20, 30, 40, 50, 60]; // 2 pixels
        let p = Pixels::from_rgb8(2, 1, &rgb).unwrap();
        assert_eq!(p.as_rgba8(), &[10, 20, 30, 0xFF, 40, 50, 60, 0xFF]);
    }

    #[test]
    fn zero_dimension_rejected() {
        assert!(matches!(
            Pixels::from_rgba8(0, 1, vec![]),
            Err(PixelError::ZeroDimension { .. })
        ));
        assert!(matches!(
            Pixels::from_rgb8(1, 0, &[]),
            Err(PixelError::ZeroDimension { .. })
        ));
    }

    #[test]
    fn rgba8_size_mismatch_rejected() {
        let too_small = vec![0_u8; 5]; // 2x2x4 = 16 expected
        assert!(matches!(
            Pixels::from_rgba8(2, 2, too_small),
            Err(PixelError::DataSizeMismatch {
                expected: 16,
                got: 5,
                ..
            })
        ));
    }

    #[test]
    fn rgb8_size_mismatch_rejected() {
        let too_big = vec![0_u8; 100];
        assert!(matches!(
            Pixels::from_rgb8(2, 2, &too_big),
            Err(PixelError::DataSizeMismatch {
                expected: 12,
                got: 100,
                ..
            })
        ));
    }

    #[test]
    fn into_parts_round_trip() {
        let data = vec![0xAB_u8; 4];
        let p = Pixels::from_rgba8(1, 1, data.clone()).unwrap();
        let (w, h, out) = p.into_parts();
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, data);
    }
}
