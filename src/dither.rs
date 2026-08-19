//! Grayscale buffers and dithering.
//!
//! Impact printers put ink down or they don't — there are no gray dots —
//! so continuous-tone images must be reduced to 1 bit per pixel. This
//! module holds the 8-bit [`Grayscale`] buffer and the [`Dither`]
//! algorithms that perform that reduction, ported byte-for-byte from the
//! reference implementation that was tuned on real SP700 hardware.
//!
//! Pixels follow the usual convention: `0` is black (ink), `255` is
//! white (paper).

use crate::error::{Error, Result};
use crate::graphics::Bitmap;

/// An 8-bit grayscale raster (`0` = black, `255` = white), stored row by
/// row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grayscale {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Grayscale {
    /// Wraps row-major 8-bit pixels; `pixels.len()` must equal
    /// `width * height`.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        if pixels.len() != (width as usize) * (height as usize) {
            return Err(Error::InvalidData {
                reason: format!(
                    "pixel buffer holds {} bytes but {width}x{height} needs {}",
                    pixels.len(),
                    (width as usize) * (height as usize),
                ),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The row-major pixel data.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Mutable access for in-place pipeline stages.
    #[cfg(feature = "image")]
    pub(crate) fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Converts to a 1-bit [`Bitmap`]: pixels darker than mid-gray
    /// (`< 128`) become ink.
    ///
    /// On dithered output (exactly `0` or `255` per pixel) this is a
    /// lossless reinterpretation.
    #[must_use]
    pub fn to_bitmap(&self) -> Bitmap {
        Bitmap::from_fn(self.width, self.height, |x, y| {
            self.pixels[(y * self.width + x) as usize] < 128
        })
    }
}

/// A dithering algorithm reducing 8-bit grayscale to pure black/white.
///
/// The `threshold` fields set the gray level below which a pixel leans
/// black (128 in the hardware-tuned reference); [`Bayer8x8`](Self::Bayer8x8)
/// uses its own matrix instead of a threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dither {
    /// Plain thresholding — crisp for text and line art, poor for photos.
    Threshold {
        /// Gray level below which a pixel becomes black.
        threshold: u8,
    },
    /// Floyd–Steinberg error diffusion — the all-round default for
    /// photographs.
    FloydSteinberg {
        /// Quantisation threshold.
        threshold: u8,
    },
    /// Atkinson error diffusion — diffuses only 6/8 of the error, giving
    /// a lighter, higher-contrast look.
    Atkinson {
        /// Quantisation threshold.
        threshold: u8,
    },
    /// Ordered dithering with an 8×8 Bayer matrix — regular patterning;
    /// not recommended for photos.
    Bayer8x8,
}

impl Default for Dither {
    /// Floyd–Steinberg at the reference threshold of 128.
    fn default() -> Self {
        Self::FloydSteinberg { threshold: 128 }
    }
}

/// `(dx, dy, weight)` error-diffusion taps.
type Kernel = &'static [(i64, i64, f64)];

/// Floyd–Steinberg distributes the full error over four neighbours.
const FLOYD_STEINBERG: Kernel = &[
    (1, 0, 7.0 / 16.0),
    (-1, 1, 3.0 / 16.0),
    (0, 1, 5.0 / 16.0),
    (1, 1, 1.0 / 16.0),
];

/// Atkinson distributes 1/8 to six neighbours (deliberately dropping the
/// remaining 2/8).
const ATKINSON: Kernel = &[
    (1, 0, 1.0 / 8.0),
    (2, 0, 1.0 / 8.0),
    (-1, 1, 1.0 / 8.0),
    (0, 1, 1.0 / 8.0),
    (1, 1, 1.0 / 8.0),
    (0, 2, 1.0 / 8.0),
];

#[rustfmt::skip]
const BAYER_8X8: [[u8; 8]; 8] = [
    [ 0, 32,  8, 40,  2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44,  4, 36, 14, 46,  6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [ 3, 35, 11, 43,  1, 33,  9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47,  7, 39, 13, 45,  5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

impl Dither {
    /// Applies the algorithm, producing an image whose pixels are all
    /// exactly `0` or `255`.
    #[must_use]
    pub fn apply(self, image: &Grayscale) -> Grayscale {
        let pixels = match self {
            Self::Threshold { threshold } => image
                .pixels
                .iter()
                .map(|&p| if p < threshold { 0 } else { 255 })
                .collect(),
            Self::FloydSteinberg { threshold } => error_diffuse(image, threshold, FLOYD_STEINBERG),
            Self::Atkinson { threshold } => error_diffuse(image, threshold, ATKINSON),
            Self::Bayer8x8 => {
                let w = image.width as usize;
                image
                    .pixels
                    .iter()
                    .enumerate()
                    .map(|(i, &p)| {
                        let t = BAYER_8X8[(i / w) & 7][(i % w) & 7];
                        if f64::from(p) > f64::from(t) * 255.0 / 63.0 {
                            255
                        } else {
                            0
                        }
                    })
                    .collect()
            }
        };
        Grayscale {
            width: image.width,
            height: image.height,
            pixels,
        }
    }
}

/// Generic error diffusion over a `(dx, dy, weight)` kernel, in f64 like
/// the reference implementation so rounding behaviour matches exactly.
fn error_diffuse(image: &Grayscale, threshold: u8, kernel: Kernel) -> Vec<u8> {
    let (w, h) = (image.width as i64, image.height as i64);
    let mut buf: Vec<f64> = image.pixels.iter().map(|&p| f64::from(p)).collect();
    let mut out = vec![0u8; buf.len()];

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let old = buf[i];
            let new = if old < f64::from(threshold) {
                0.0
            } else {
                255.0
            };
            out[i] = new as u8;
            let err = old - new;
            for &(dx, dy, weight) in kernel {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && nx < w && ny >= 0 && ny < h {
                    buf[(ny * w + nx) as usize] += err * weight;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(width: u32, height: u32, pixels: &[u8]) -> Grayscale {
        Grayscale::new(width, height, pixels.to_vec()).unwrap()
    }

    #[test]
    fn buffer_length_is_validated() {
        assert!(Grayscale::new(3, 2, vec![0; 6]).is_ok());
        assert!(Grayscale::new(3, 2, vec![0; 5]).is_err());
    }

    #[test]
    fn threshold_splits_at_the_given_level() {
        let img = gray(4, 1, &[0, 127, 128, 255]);
        let out = Dither::Threshold { threshold: 128 }.apply(&img);
        assert_eq!(out.pixels(), [0, 0, 255, 255]);
    }

    #[test]
    fn floyd_steinberg_pushes_error_right() {
        // 192 quantises to white (error -63), pushing 7/16 * -63 ≈ -27.6
        // onto the next pixel: 160 - 27.6 = 132.4 → white; then error
        // -122.6, pushing -53.6 onto 160 → 106.4 → black.
        let img = gray(3, 1, &[192, 160, 160]);
        let out = Dither::FloydSteinberg { threshold: 128 }.apply(&img);
        assert_eq!(out.pixels(), [255, 255, 0]);
    }

    #[test]
    fn output_is_strictly_binary() {
        let img = gray(8, 8, &(0..64).map(|i| (i * 4) as u8).collect::<Vec<_>>());
        for dither in [
            Dither::Threshold { threshold: 128 },
            Dither::FloydSteinberg { threshold: 128 },
            Dither::Atkinson { threshold: 128 },
            Dither::Bayer8x8,
        ] {
            let out = dither.apply(&img);
            assert!(
                out.pixels().iter().all(|&p| p == 0 || p == 255),
                "{dither:?}"
            );
        }
    }

    #[test]
    fn to_bitmap_inks_dark_pixels() {
        let img = gray(2, 1, &[0, 255]);
        let bmp = img.to_bitmap();
        assert!(bmp.get(0, 0) && !bmp.get(1, 0));
    }
}
