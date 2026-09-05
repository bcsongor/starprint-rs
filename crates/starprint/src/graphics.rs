//! Pictures: [`Bitmap`] and [`BitImage`] (1-bit), [`Grayscale`] and
//! [`Dithering`] (8-bit to 1-bit), and with the `image` feature
//! `ImagePipeline`, which prepares a photo for a given head.

mod dither;
#[cfg(feature = "image")]
mod pipeline;

pub use dither::{Dithering, Grayscale};
#[cfg(feature = "image")]
pub use pipeline::{ImagePipeline, ToneCurve};

use crate::error::{Error, Result};

/// Rows per `ESC ^` stripe.
pub(crate) const STRIPE_HEIGHT: u32 = 9;

/// `ESC 3` units (1/216″) per dot row (1/72″).
pub(crate) const LINE_FEED_UNITS_PER_DOT: u8 = 3;

/// Horizontal dot density of a bit image. Double prints twice the columns
/// across the same width, so the dots overlap and print darker; the
/// pipeline brightens to compensate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// 210 dots across the SP700's 63 mm.
    #[default]
    Single,
    /// 420 dots.
    Double,
}

/// Decides the default tone curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadKind {
    /// A ribbon prints light; images need darkening and contrast.
    Impact,
    /// Prints dark with blooming dots; images need lightening.
    Thermal,
}

/// Head and paper geometry, used to size and proportion images.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceProfile {
    pub name: &'static str,
    pub head: HeadKind,
    pub vertical_dpi: f64,
    /// At single density.
    pub horizontal_dpi: f64,
    pub width_dots_single: u32,
    pub width_dots_double: u32,
}

impl DeviceProfile {
    /// 63 mm print region, 210 dots (≈ 84.7 DPI), 72 DPI vertically.
    /// Measured on real hardware.
    pub const SP700: Self = Self {
        name: "Star SP700 series",
        head: HeadKind::Impact,
        vertical_dpi: 72.0,
        horizontal_dpi: (210.0 * 25.4) / 63.0,
        width_dots_single: 210,
        width_dots_double: 420,
    };

    /// 72 mm print width at 8 dots/mm, 576 dots. Thermal heads have no
    /// horizontal double density, so [`Density`] makes no difference.
    pub const THERMAL_80MM: Self = Self {
        name: "Star 80 mm thermal",
        head: HeadKind::Thermal,
        vertical_dpi: 203.2,
        horizontal_dpi: 203.2,
        width_dots_single: 576,
        width_dots_double: 576,
    };

    /// TSP800II on 112 mm paper: 104 mm print width, 832 dots.
    pub const THERMAL_112MM: Self = Self {
        name: "Star TSP800II 112 mm",
        width_dots_single: 832,
        width_dots_double: 832,
        ..Self::THERMAL_80MM
    };

    /// [`THERMAL_80MM`](Self::THERMAL_80MM) in
    /// [`PrintMode::DoubleResolution`](crate::PrintMode::DoubleResolution):
    /// twice the rows.
    pub const THERMAL_80MM_DOUBLE_RESOLUTION: Self = Self {
        name: "Star 80 mm thermal (double resolution)",
        vertical_dpi: 406.4,
        ..Self::THERMAL_80MM
    };

    /// [`THERMAL_112MM`](Self::THERMAL_112MM) in double resolution.
    pub const THERMAL_112MM_DOUBLE_RESOLUTION: Self = Self {
        name: "Star 112 mm thermal (double resolution)",
        vertical_dpi: 406.4,
        ..Self::THERMAL_112MM
    };

    #[must_use]
    pub fn max_width(&self, density: Density) -> u32 {
        match density {
            Density::Single => self.width_dots_single,
            Density::Double => self.width_dots_double,
        }
    }

    #[must_use]
    pub fn horizontal_dpi_at(&self, density: Density) -> f64 {
        match density {
            Density::Single => self.horizontal_dpi,
            Density::Double => {
                self.horizontal_dpi * f64::from(self.width_dots_double)
                    / f64::from(self.width_dots_single)
            }
        }
    }
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self::SP700
    }
}

/// A 1-bit raster; `true` is ink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    ink: Vec<bool>,
}

impl Bitmap {
    /// ```
    /// use starprint::graphics::Bitmap;
    ///
    /// let checkerboard = Bitmap::from_fn(16, 16, |x, y| (x + y) % 2 == 0);
    /// assert!(checkerboard.get(0, 0) && !checkerboard.get(1, 0));
    /// ```
    #[must_use]
    pub fn from_fn(width: u32, height: u32, mut ink: impl FnMut(u32, u32) -> bool) -> Self {
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.push(ink(x, y));
            }
        }
        Self {
            width,
            height,
            ink: pixels,
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Out-of-bounds reads are blank.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.ink[(y * self.width + x) as usize]
    }
}

/// A [`Bitmap`] that fits the head at its [`Density`], so
/// [`bit_image`](crate::Builder::bit_image) cannot fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitImage {
    pub(crate) bitmap: Bitmap,
    pub(crate) density: Density,
}

impl BitImage {
    /// Validates against [`DeviceProfile::SP700`].
    pub fn new(bitmap: Bitmap, density: Density) -> Result<Self> {
        Self::with_profile(bitmap, density, &DeviceProfile::SP700)
    }

    pub fn with_profile(bitmap: Bitmap, density: Density, profile: &DeviceProfile) -> Result<Self> {
        if bitmap.width == 0 || bitmap.height == 0 {
            return Err(Error::InvalidData {
                reason: "bit image must not be empty".into(),
            });
        }
        let max = profile.max_width(density);
        if bitmap.width > max {
            return Err(Error::DataTooLong {
                max: max as usize,
                actual: bitmap.width as usize,
            });
        }
        Ok(Self { bitmap, density })
    }

    #[must_use]
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    #[must_use]
    pub fn density(&self) -> Density {
        self.density
    }
}

impl AsRef<Bitmap> for Bitmap {
    fn as_ref(&self) -> &Bitmap {
        self
    }
}

impl AsRef<Bitmap> for BitImage {
    fn as_ref(&self) -> &Bitmap {
        &self.bitmap
    }
}

/// Appends one raster row: 8 dots per byte, MSB leftmost, padded with
/// blanks in the last byte.
pub(crate) fn pack_row(bitmap: &Bitmap, y: u32, out: &mut Vec<u8>) {
    let width = bitmap.width as usize;
    let start = y as usize * width;
    out.extend(bitmap.ink[start..start + width].chunks(8).map(|dots| {
        dots.iter()
            .enumerate()
            .fold(0u8, |byte, (bit, &ink)| byte | (u8::from(ink) << (7 - bit)))
    }));
}

/// Appends one `ESC ^` stripe from row `top`: two bytes per column, rows
/// `top..top+8` in the first, row `top+8` in the MSB of the second.
pub(crate) fn pack_stripe(bitmap: &Bitmap, top: u32, out: &mut Vec<u8>) {
    for x in 0..bitmap.width {
        let mut b0 = 0u8;
        for bit in 0..8 {
            if bitmap.get(x, top + bit) {
                b0 |= 1 << (7 - bit);
            }
        }
        let b1 = if bitmap.get(x, top + 8) { 0x80 } else { 0 };
        out.extend_from_slice(&[b0, b1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_from_fn_and_get() {
        let bmp = Bitmap::from_fn(3, 2, |x, y| x == y);
        assert!(bmp.get(0, 0) && bmp.get(1, 1));
        assert!(!bmp.get(2, 0) && !bmp.get(0, 1));
        assert!(!bmp.get(99, 99)); // out of bounds reads blank
    }

    #[test]
    fn bit_image_validates_width() {
        let too_wide = Bitmap::from_fn(211, 1, |_, _| true);
        assert!(BitImage::new(too_wide.clone(), Density::Single).is_err());
        assert!(BitImage::new(too_wide, Density::Double).is_ok());
        let empty = Bitmap::from_fn(0, 5, |_, _| true);
        assert!(BitImage::new(empty, Density::Single).is_err());
    }

    #[test]
    fn row_packing_is_msb_first_and_padded() {
        // 10 dots wide: ink at x = 0, 7, 8 → 0b1000_0001, 0b1000_0000.
        let bmp = Bitmap::from_fn(10, 1, |x, _| matches!(x, 0 | 7 | 8));
        let mut row = Vec::new();
        pack_row(&bmp, 0, &mut row);
        assert_eq!(row, [0b1000_0001, 0b1000_0000]);
    }

    #[test]
    fn stripe_packing_layout() {
        // Column 0: rows 0 and 8 inked; column 1: row 7 inked.
        let bmp = Bitmap::from_fn(2, 9, |x, y| {
            (x == 0 && (y == 0 || y == 8)) || (x == 1 && y == 7)
        });
        let mut stripe = Vec::new();
        pack_stripe(&bmp, 0, &mut stripe);
        assert_eq!(stripe, [0b1000_0000, 0x80, 0b0000_0001, 0x00]);
        // A stripe past the bottom edge reads blank rows.
        let short = Bitmap::from_fn(1, 10, |_, y| y == 9);
        stripe.clear();
        pack_stripe(&short, 9, &mut stripe);
        assert_eq!(stripe, [0b1000_0000, 0x00]);
    }
}
