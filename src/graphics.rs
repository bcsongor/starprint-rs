//! Bit-image graphics for impact printers.
//!
//! This module is the dependency-free core of image printing: a 1-bit
//! [`Bitmap`], the [`BitImage`] wrapper that validates it against a
//! printer's head width, and the column packing for the SP700's 9-dot
//! bit-image command (`ESC ^`). Producing a bitmap from a real picture
//! (decode, tone mapping, dithering) lives in [`crate::dither`] and —
//! behind the `image` feature — `crate::pipeline`.

use crate::error::{Error, Result};

/// Vertical dot rows per 9-dot bit-image stripe (`ESC ^` prints one
/// stripe per command).
pub(crate) const STRIPE_HEIGHT: u32 = 9;

/// Line-feed units (1/216″) per vertical dot row: dot rows are 1/72″
/// apart, so one stripe is `9 × 3 = 27` units.
pub(crate) const LINE_FEED_UNITS_PER_DOT: u8 = 3;

/// Horizontal dot density of a bit image.
///
/// Double density prints twice as many columns across the same physical
/// width; the dots physically overlap, which darkens the output (the
/// `pipeline` module compensates by brightening).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// One dot per single-density column (210 dots across 63 mm on the
    /// SP700).
    #[default]
    Single,
    /// Two dots per single-density column (420 dots across 63 mm on the
    /// SP700).
    Double,
}

/// Physical characteristics of a printer's head and paper, used to size
/// and proportion images.
///
/// The values were measured on real hardware; [`DeviceProfile::SP700`]
/// covers the SP700 series (and most Star dot-impact printers).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceProfile {
    /// Human-readable name of the device or family.
    pub name: &'static str,
    /// Vertical paper-feed resolution in DPI (dot rows are 1/72″ apart).
    pub vertical_dpi: f64,
    /// Horizontal resolution in DPI at single density.
    pub horizontal_dpi: f64,
    /// Maximum print width in dots at single density.
    pub width_dots_single: u32,
    /// Maximum print width in dots at double density.
    pub width_dots_double: u32,
}

impl DeviceProfile {
    /// The SP700 series (SP712/SP742/SP717/SP747): a 63 mm print region,
    /// 210 dots wide at single density (≈ 84.7 DPI), 72 DPI vertically.
    pub const SP700: Self = Self {
        name: "Star SP700 series",
        vertical_dpi: 72.0,
        horizontal_dpi: (210.0 * 25.4) / 63.0,
        width_dots_single: 210,
        width_dots_double: 420,
    };

    /// The maximum printable width in dots at the given density.
    #[must_use]
    pub fn width_dots(&self, density: Density) -> u32 {
        match density {
            Density::Single => self.width_dots_single,
            Density::Double => self.width_dots_double,
        }
    }

    /// The effective horizontal DPI at the given density.
    #[must_use]
    pub fn horizontal_dpi(&self, density: Density) -> f64 {
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

/// A 1-bit raster: `true` pixels are inked (printed), `false` pixels are
/// blank paper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    ink: Vec<bool>,
}

impl Bitmap {
    /// Builds a bitmap by evaluating `ink` for every `(x, y)` position,
    /// row by row.
    ///
    /// # Examples
    ///
    /// ```
    /// use starprint::graphics::Bitmap;
    ///
    /// // A 16x16 checkerboard.
    /// let bmp = Bitmap::from_fn(16, 16, |x, y| (x + y) % 2 == 0);
    /// assert!(bmp.get(0, 0) && !bmp.get(1, 0));
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

    /// Width in dots.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in dot rows.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Whether the pixel at `(x, y)` is inked; out-of-bounds reads are
    /// blank.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.ink[(y * self.width + x) as usize]
    }
}

/// A [`Bitmap`] validated against a printer profile and bound to a
/// [`Density`], ready for [`bit_image`](crate::Builder::bit_image).
///
/// Constructing it up front (like [`Barcode`](crate::Barcode)) keeps
/// the builder infallible: a `BitImage` is printable by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitImage {
    pub(crate) bitmap: Bitmap,
    pub(crate) density: Density,
}

impl BitImage {
    /// Validates `bitmap` against the [`DeviceProfile::SP700`] head width
    /// for the chosen density.
    pub fn new(bitmap: Bitmap, density: Density) -> Result<Self> {
        Self::with_profile(bitmap, density, &DeviceProfile::SP700)
    }

    /// Validates `bitmap` against a specific device profile.
    pub fn with_profile(bitmap: Bitmap, density: Density, profile: &DeviceProfile) -> Result<Self> {
        if bitmap.width == 0 || bitmap.height == 0 {
            return Err(Error::InvalidData {
                reason: "bit image must not be empty".into(),
            });
        }
        let max = profile.width_dots(density);
        if bitmap.width > max {
            return Err(Error::DataTooLong {
                max: max as usize,
                actual: bitmap.width as usize,
            });
        }
        Ok(Self { bitmap, density })
    }
}

/// Packs one 9-row stripe starting at row `top` into `ESC ^` column data:
/// two bytes per column — rows `top..top+8` MSB-first in the first byte,
/// row `top+8` in the MSB of the second byte.
pub(crate) fn pack_stripe(bitmap: &Bitmap, top: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(bitmap.width as usize * 2);
    for x in 0..bitmap.width {
        let mut b0 = 0u8;
        for bit in 0..8 {
            if bitmap.get(x, top + bit) {
                b0 |= 1 << (7 - bit);
            }
        }
        let b1 = if bitmap.get(x, top + 8) { 0x80 } else { 0 };
        out.push(b0);
        out.push(b1);
    }
    out
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
    fn stripe_packing_layout() {
        // Column 0: rows 0 and 8 inked; column 1: row 7 inked.
        let bmp = Bitmap::from_fn(2, 9, |x, y| {
            (x == 0 && (y == 0 || y == 8)) || (x == 1 && y == 7)
        });
        assert_eq!(pack_stripe(&bmp, 0), [0b1000_0000, 0x80, 0b0000_0001, 0x00]);
        // A stripe past the bottom edge reads blank rows.
        let short = Bitmap::from_fn(1, 10, |_, y| y == 9);
        assert_eq!(pack_stripe(&short, 9), [0b1000_0000, 0x00]);
    }
}
