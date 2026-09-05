//! Grayscale buffers and the dithering that reduces them to 1 bit, ported
//! byte-for-byte from the Python reference. `0` is black, `255` white.

use crate::error::{Error, Result};
use crate::graphics::Bitmap;

/// 8-bit grayscale, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grayscale {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Grayscale {
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

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    #[must_use]
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    #[cfg(feature = "image")]
    pub(crate) fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Pixels below 128 become ink.
    #[must_use]
    pub fn to_bitmap(&self) -> Bitmap {
        Bitmap::from_fn(self.width, self.height, |x, y| {
            self.pixels[(y * self.width + x) as usize] < 128
        })
    }
}

/// Reduces 8-bit grayscale to black and white. `threshold` is the level
/// below which a pixel leans black; the reference uses 128.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dithering {
    /// Crisp for line art, poor for photos.
    Threshold { threshold: u8 },
    /// The default for photos.
    FloydSteinberg { threshold: u8 },
    /// Spreads only 6/8 of the error: lighter, more contrast.
    Atkinson { threshold: u8 },
    /// Ordered; the pattern shows, so not for photos.
    Bayer8x8,
}

impl Default for Dithering {
    fn default() -> Self {
        Self::FloydSteinberg { threshold: 128 }
    }
}

/// `(dx, dy, weight)` taps.
type Kernel = &'static [(i64, i64, f64)];

const FLOYD_STEINBERG: Kernel = &[
    (1, 0, 7.0 / 16.0),
    (-1, 1, 3.0 / 16.0),
    (0, 1, 5.0 / 16.0),
    (1, 1, 1.0 / 16.0),
];

/// Six taps of 1/8; the remaining 2/8 is dropped on purpose.
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

impl Dithering {
    /// Every output pixel is exactly `0` or `255`.
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

/// In f64 like the reference, so rounding matches. The kernels reach at
/// most two rows down, so three rows of error are live at a time.
fn error_diffuse(image: &Grayscale, threshold: u8, kernel: Kernel) -> Vec<u8> {
    const ROWS: usize = 3;
    debug_assert!(
        kernel
            .iter()
            .all(|&(_, dy, _)| (0..ROWS as i64).contains(&dy))
    );
    let (w, h) = (image.width as usize, image.height as usize);
    let threshold = f64::from(threshold);
    let mut out = vec![0u8; w * h];
    let mut ring = vec![0.0f64; ROWS * w];
    let load = |ring: &mut [f64], y: usize| {
        if y < h {
            let row = &image.pixels[y * w..][..w];
            for (err, &p) in ring[(y % ROWS) * w..][..w].iter_mut().zip(row) {
                *err = f64::from(p);
            }
        }
    };
    for y in 0..ROWS {
        load(&mut ring, y);
    }

    for y in 0..h {
        for x in 0..w {
            let old = ring[(y % ROWS) * w + x];
            let new = if old < threshold { 0.0 } else { 255.0 };
            out[y * w + x] = new as u8;
            let err = old - new;
            for &(dx, dy, weight) in kernel {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx >= 0 && nx < w as i64 && ny < h as i64 {
                    ring[(ny as usize % ROWS) * w + nx as usize] += err * weight;
                }
            }
        }
        // Row `y` is done, so its slot takes the row entering the window.
        load(&mut ring, y + ROWS);
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
        let out = Dithering::Threshold { threshold: 128 }.apply(&img);
        assert_eq!(out.pixels(), [0, 0, 255, 255]);
    }

    #[test]
    fn floyd_steinberg_pushes_error_right() {
        // 192 quantises to white (error -63), pushing 7/16 * -63 ≈ -27.6
        // onto the next pixel: 160 - 27.6 = 132.4 → white; then error
        // -122.6, pushing -53.6 onto 160 → 106.4 → black.
        let img = gray(3, 1, &[192, 160, 160]);
        let out = Dithering::FloydSteinberg { threshold: 128 }.apply(&img);
        assert_eq!(out.pixels(), [255, 255, 0]);
    }

    #[test]
    fn output_is_strictly_binary() {
        let img = gray(8, 8, &(0..64).map(|i| (i * 4) as u8).collect::<Vec<_>>());
        for dither in [
            Dithering::Threshold { threshold: 128 },
            Dithering::FloydSteinberg { threshold: 128 },
            Dithering::Atkinson { threshold: 128 },
            Dithering::Bayer8x8,
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
