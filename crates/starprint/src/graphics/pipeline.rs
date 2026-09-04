//! Turns a picture into a print-ready [`BitImage`], with the tone mapping
//! dialled in on SP700 hardware:
//!
//! 1. flatten transparency onto white, convert to grayscale,
//! 2. auto-contrast,
//! 3. gamma (1.8 impact, 0.55 thermal),
//! 4. unsharp mask (radius 2.4, 150 %),
//! 5. histogram equalisation (impact only),
//! 6. at double density, brighten ×1.2 to offset dot overlap,
//! 7. user brightness and contrast,
//! 8. resize to the head width, correcting the SP700's anisotropic
//!    resolution (84.7 DPI across, 72 DPI along),
//! 9. dither.
//!
//! The LUT stages match the Python reference byte-for-byte and are tested
//! against its fixtures. Resampling and blurring are equivalent but not
//! bit-identical (Lanczos3 and a true Gaussian against Pillow's).
//!
//! ```no_run
//! use starprint::graphics::Density;
//! use starprint::graphics::ImagePipeline;
//!
//! let photo = std::fs::read("photo.jpg").unwrap();
//! let prepared = ImagePipeline::new()
//!     .density(Density::Double)
//!     .prepare_bytes(&photo)?;
//! let doc = starprint::impact().bit_image(&prepared.image).build();
//! # Ok::<(), starprint::Error>(())
//! ```

use image::DynamicImage;
use image::imageops::FilterType;

use super::{BitImage, Density, DeviceProfile, HeadKind};
use super::{Dithering, Grayscale};
use crate::error::{Error, Result};

/// The unsharp radius is 3× this.
const SHARPEN_SIGMA: f64 = 0.8;
const UNSHARP_PERCENT: i32 = 150;
/// Offsets the dot overlap of double density.
const DOUBLE_DENSITY_BRIGHTNESS: f64 = 1.2;

/// The stages that depend on the head. [`ImagePipeline::tone`] overrides
/// the head's default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneCurve {
    /// `> 1` darkens midtones, `< 1` lightens them.
    pub gamma: f64,
    /// Equalise the histogram after sharpening. Maximum contrast, but it
    /// crushes shadows on a head that already prints dark.
    pub equalize: bool,
}

impl ToneCurve {
    /// Dialled in on the SP700. Do not change; it is the reference.
    pub const IMPACT: Self = Self {
        gamma: 1.8,
        equalize: true,
    };

    /// Dialled in on a TSP800II at slow speed, density +3.
    pub const THERMAL: Self = Self {
        gamma: 0.55,
        equalize: false,
    };

    #[must_use]
    pub const fn for_head(head: HeadKind) -> Self {
        match head {
            HeadKind::Impact => Self::IMPACT,
            HeadKind::Thermal => Self::THERMAL,
        }
    }
}

/// Chain settings, then [`prepare`](Self::prepare) or
/// [`prepare_bytes`](Self::prepare_bytes). [`new`](Self::new) is the
/// reference: SP700, single density, Floyd–Steinberg at 128.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagePipeline {
    density: Density,
    dither: Dithering,
    brightness: f64,
    contrast: f64,
    profile: DeviceProfile,
    tone: Option<ToneCurve>,
}

impl Default for ImagePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ImagePipeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            density: Density::Single,
            dither: Dithering::default(),
            brightness: 1.0,
            contrast: 1.0,
            profile: DeviceProfile::SP700,
            tone: None,
        }
    }

    /// Overrides the profile head's [`ToneCurve`].
    #[must_use]
    pub fn tone(mut self, tone: ToneCurve) -> Self {
        self.tone = Some(tone);
        self
    }

    #[must_use]
    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    #[must_use]
    pub fn dither(mut self, dither: Dithering) -> Self {
        self.dither = dither;
        self
    }

    /// Applied after the tone curve; 1.0 is unchanged.
    #[must_use]
    pub fn brightness(mut self, factor: f64) -> Self {
        self.brightness = factor;
        self
    }

    /// Applied after the tone curve; 1.0 is unchanged.
    #[must_use]
    pub fn contrast(mut self, factor: f64) -> Self {
        self.contrast = factor;
        self
    }

    #[must_use]
    pub fn profile(mut self, profile: DeviceProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Decodes PNG, JPEG, WebP or BMP, then [`prepare`](Self::prepare).
    pub fn prepare_bytes(&self, bytes: &[u8]) -> Result<PreparedImage> {
        let decoded = image::load_from_memory(bytes).map_err(|err| Error::InvalidData {
            reason: format!("image decode failed: {err}"),
        })?;
        self.prepare(&decoded)
    }

    pub fn prepare(&self, source: &DynamicImage) -> Result<PreparedImage> {
        let mut gray = to_grayscale_on_white(source);
        let (src_w, src_h) = (gray.width(), gray.height());
        if src_w == 0 || src_h == 0 {
            return Err(Error::InvalidData {
                reason: "source image has zero width or height".into(),
            });
        }

        let tone = self.tone.unwrap_or(ToneCurve::for_head(self.profile.head));
        autocontrast(&mut gray);
        if tone.gamma != 1.0 {
            apply_lut(&mut gray, &gamma_lut(tone.gamma));
        }
        gray = unsharp_mask(&gray, SHARPEN_SIGMA * 3.0);
        if tone.equalize {
            equalize(&mut gray);
        }
        if self.density == Density::Double {
            brightness(&mut gray, DOUBLE_DENSITY_BRIGHTNESS);
        }

        if self.brightness != 1.0 {
            brightness(&mut gray, self.brightness);
        }
        if self.contrast != 1.0 {
            contrast(&mut gray, self.contrast);
        }

        let width = self.profile.max_width(self.density);
        let h_dpi = self.profile.horizontal_dpi_at(self.density);

        // Fit the width, then stretch the height for the anisotropic pitch.
        let aspect_h = ratio_round(src_h, width, src_w);
        let compensated_h =
            (f64::from(aspect_h) * self.profile.vertical_dpi / h_dpi).round_ties_even() as u32;
        if compensated_h == 0 {
            return Err(Error::InvalidData {
                reason: "image is too wide and short to print at this width".into(),
            });
        }
        // One source for both resamples.
        let full = image::GrayImage::from_raw(src_w, src_h, gray.into_pixels())
            .expect("buffer sized from dimensions");
        let print_gray = self.dither.apply(&resize(&full, width, compensated_h));
        let image = BitImage::with_profile(print_gray.to_bitmap(), self.density, &self.profile)?;

        // Preview: print width at display height, then pool pairs at double
        // density.
        let display_w = self.profile.width_dots_single;
        let display_h = ratio_round(src_h, display_w, src_w).max(1);
        let preview_gray = if display_h == compensated_h {
            print_gray
        } else {
            self.dither.apply(&resize(&full, width, display_h))
        };
        let preview = if width == display_w {
            preview_gray
        } else {
            min_pool_pairs(&preview_gray)
        };

        Ok(PreparedImage { image, preview })
    }
}

#[derive(Debug, Clone)]
pub struct PreparedImage {
    pub image: BitImage,
    /// For the screen: single-density width, square pixels. At double
    /// density, pixel pairs are min-pooled to show the dot overlap.
    pub preview: Grayscale,
}

/// `round(a * b / c)`, ties to even like Python.
fn ratio_round(a: u32, b: u32, c: u32) -> u32 {
    (f64::from(a) * f64::from(b) / f64::from(c)).round_ties_even() as u32
}

/// Pillow's "L" conversion (ITU-R 601-2 luma) over white.
fn to_grayscale_on_white(source: &DynamicImage) -> Grayscale {
    fn over_white(c: u8, a: u8) -> u32 {
        (u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255
    }
    fn luma(r: u32, g: u32, b: u32) -> u8 {
        ((r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16) as u8
    }
    fn rgba_to_luma(raw: &[u8]) -> Vec<u8> {
        raw.chunks_exact(4)
            .map(|p| {
                luma(
                    over_white(p[0], p[3]),
                    over_white(p[1], p[3]),
                    over_white(p[2], p[3]),
                )
            })
            .collect()
    }

    let (width, height) = (source.width(), source.height());
    let pixels = match source {
        DynamicImage::ImageLuma8(img) => img.as_raw().clone(),
        DynamicImage::ImageLumaA8(img) => img
            .as_raw()
            .chunks_exact(2)
            .map(|p| over_white(p[0], p[1]) as u8)
            .collect(),
        DynamicImage::ImageRgb8(img) => img
            .as_raw()
            .chunks_exact(3)
            .map(|p| luma(u32::from(p[0]), u32::from(p[1]), u32::from(p[2])))
            .collect(),
        DynamicImage::ImageRgba8(img) => rgba_to_luma(img.as_raw()),
        other => rgba_to_luma(other.to_rgba8().as_raw()),
    };
    Grayscale::new(width, height, pixels).expect("buffer sized from dimensions")
}

fn histogram(pixels: &[u8]) -> [u32; 256] {
    let mut h = [0u32; 256];
    for &p in pixels {
        h[p as usize] += 1;
    }
    h
}

fn apply_lut(gray: &mut Grayscale, lut: &[u8; 256]) {
    for p in gray.pixels_mut() {
        *p = lut[*p as usize];
    }
}

/// Pillow `ImageOps.autocontrast`, cutoff 0.
fn autocontrast(gray: &mut Grayscale) {
    let h = histogram(gray.pixels());
    let Some(lo) = h.iter().position(|&c| c > 0) else {
        return;
    };
    let hi = h.iter().rposition(|&c| c > 0).expect("nonzero bin exists");
    if hi <= lo {
        return;
    }
    let scale = 255.0 / (hi - lo) as f64;
    let offset = -(lo as f64) * scale;
    let mut lut = [0u8; 256];
    for (i, out) in lut.iter_mut().enumerate() {
        // Pillow truncates toward zero, then clamps.
        *out = ((i as f64 * scale + offset) as i32).clamp(0, 255) as u8;
    }
    apply_lut(gray, &lut);
}

/// `(in / 255) ^ gamma * 255`, truncated like the reference.
fn gamma_lut(gamma: f64) -> [u8; 256] {
    let mut lut = [0u8; 256];
    for (i, out) in lut.iter_mut().enumerate() {
        *out = (((i as f64 / 255.0).powf(gamma) * 255.0) as i32).min(255) as u8;
    }
    lut
}

/// Pillow `ImageOps.equalize`.
fn equalize(gray: &mut Grayscale) {
    let h = histogram(gray.pixels());
    let mut nonzero = h.iter().copied().filter(|&c| c > 0).map(u64::from);
    // Pillow excludes the highest occupied bin when calculating the step.
    if nonzero.next_back().is_none() {
        return;
    }
    let step = nonzero.sum::<u64>() / 255;
    if step == 0 {
        return;
    }
    let mut lut = [0u8; 256];
    let mut n = step / 2;
    for (i, out) in lut.iter_mut().enumerate() {
        *out = (n / step).min(255) as u8;
        n += u64::from(h[i]);
    }
    apply_lut(gray, &lut);
}

/// Pillow `ImageEnhance.Brightness`: `px * factor`.
fn brightness(gray: &mut Grayscale, factor: f64) {
    for p in gray.pixels_mut() {
        *p = (factor * f64::from(*p)) as u8;
    }
}

/// Pillow `ImageEnhance.Contrast`: `mean + factor * (px - mean)`.
fn contrast(gray: &mut Grayscale, factor: f64) {
    let sum: u64 = gray.pixels().iter().map(|&p| u64::from(p)).sum();
    let mean = (sum as f64 / gray.pixels().len() as f64 + 0.5) as i32; // round half up
    for p in gray.pixels_mut() {
        *p = (f64::from(mean) + factor * (f64::from(*p) - f64::from(mean))) as u8;
    }
}

/// Pillow's unsharp arithmetic at [`UNSHARP_PERCENT`] and no threshold,
/// over a true Gaussian (Pillow uses box blurs).
fn unsharp_mask(gray: &Grayscale, radius: f64) -> Grayscale {
    let blurred = gaussian_blur(gray, radius);
    let pixels = gray
        .pixels()
        .iter()
        .zip(blurred.pixels())
        .map(|(&px, &blur)| {
            let diff = i32::from(px) - i32::from(blur);
            (i32::from(px) + diff * UNSHARP_PERCENT / 100).clamp(0, 255) as u8
        })
        .collect();
    Grayscale::new(gray.width(), gray.height(), pixels).expect("same dimensions")
}

/// Separable Gaussian blur with clamped edges. Both passes walk rows of
/// contiguous memory so the inner loops vectorise.
fn gaussian_blur(gray: &Grayscale, sigma: f64) -> Grayscale {
    let taps = std::cmp::max(1, (sigma * 3.0).ceil() as usize);
    let kernel: Vec<f32> = {
        let raw: Vec<f64> = (0..=2 * taps)
            .map(|i| {
                let d = i as f64 - taps as f64;
                (-(d * d) / (2.0 * sigma * sigma)).exp()
            })
            .collect();
        let sum: f64 = raw.iter().sum();
        raw.iter().map(|k| (k / sum) as f32).collect()
    };

    let (w, h) = (gray.width() as usize, gray.height() as usize);
    let src = gray.pixels();

    // Horizontal: pad each row by `taps` so every output is a dot product.
    let mut mid = vec![0f32; w * h];
    let mut padded = vec![0f32; w + 2 * taps];
    for (row, out_row) in src.chunks_exact(w).zip(mid.chunks_exact_mut(w)) {
        padded[..taps].fill(f32::from(row[0]));
        padded[taps + w..].fill(f32::from(row[w - 1]));
        for (dst, &p) in padded[taps..taps + w].iter_mut().zip(row) {
            *dst = f32::from(p);
        }
        for (x, out) in out_row.iter_mut().enumerate() {
            *out = padded[x..x + kernel.len()]
                .iter()
                .zip(&kernel)
                .map(|(v, k)| v * k)
                .sum();
        }
    }

    // Vertical: accumulate weighted rows.
    let mut out = vec![0u8; w * h];
    let mut acc = vec![0f32; w];
    for (y, out_row) in out.chunks_exact_mut(w).enumerate() {
        acc.fill(0.0);
        for (k, &weight) in kernel.iter().enumerate() {
            let sy = (y + k).saturating_sub(taps).min(h - 1);
            let row = &mid[sy * w..(sy + 1) * w];
            for (a, &v) in acc.iter_mut().zip(row) {
                *a += weight * v;
            }
        }
        for (o, &a) in out_row.iter_mut().zip(&acc) {
            *o = a.round().clamp(0.0, 255.0) as u8;
        }
    }
    Grayscale::new(gray.width(), gray.height(), out).expect("same dimensions")
}

fn resize(source: &image::GrayImage, width: u32, height: u32) -> Grayscale {
    let resized = image::imageops::resize(source, width, height, FilterType::Lanczos3);
    Grayscale::new(width, height, resized.into_raw()).expect("buffer sized from dimensions")
}

/// Halves the width, taking the darker of each pair: an inked dot at
/// double density covers its neighbour's position too.
fn min_pool_pairs(gray: &Grayscale) -> Grayscale {
    let (w, h) = (gray.width(), gray.height());
    let half_w = w / 2;
    let src = gray.pixels();
    let mut out = Vec::with_capacity((half_w * h) as usize);
    for y in 0..h {
        let row = (y * w) as usize;
        for x in 0..half_w as usize {
            out.push(src[row + x * 2].min(src[row + x * 2 + 1]));
        }
    }
    Grayscale::new(half_w, h, out).expect("buffer sized from dimensions")
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 37;
    const H: u32 = 23;

    fn fixture(bytes: &[u8]) -> Grayscale {
        Grayscale::new(W, H, bytes.to_vec()).unwrap()
    }

    fn src() -> Grayscale {
        fixture(include_bytes!("../../tests/fixtures/src.bin"))
    }

    #[test]
    fn autocontrast_matches_reference() {
        let mut gray = fixture(include_bytes!("../../tests/fixtures/src_low.bin"));
        autocontrast(&mut gray);
        assert_eq!(
            gray.pixels(),
            include_bytes!("../../tests/fixtures/autocontrast.bin")
        );
    }

    #[test]
    fn gamma_matches_reference() {
        let mut gray = src();
        apply_lut(&mut gray, &gamma_lut(1.8));
        assert_eq!(
            gray.pixels(),
            include_bytes!("../../tests/fixtures/gamma_1_8.bin")
        );
    }

    #[test]
    fn equalize_matches_reference() {
        let mut gray = src();
        equalize(&mut gray);
        assert_eq!(
            gray.pixels(),
            include_bytes!("../../tests/fixtures/equalize.bin")
        );
    }

    #[test]
    fn equalize_leaves_empty_and_uniform_images_unchanged() {
        for pixels in [vec![], vec![0; 256], vec![128; 256], vec![255; 256]] {
            let mut gray = Grayscale::new(pixels.len() as u32, 1, pixels.clone()).unwrap();
            equalize(&mut gray);
            assert_eq!(gray.pixels(), pixels);
        }
    }

    #[test]
    fn brightness_matches_reference() {
        let mut lifted = src();
        brightness(&mut lifted, 1.2);
        assert_eq!(
            lifted.pixels(),
            include_bytes!("../../tests/fixtures/brightness_1_2.bin")
        );
        let mut dimmed = src();
        brightness(&mut dimmed, 0.8);
        assert_eq!(
            dimmed.pixels(),
            include_bytes!("../../tests/fixtures/brightness_0_8.bin")
        );
    }

    #[test]
    fn contrast_matches_reference() {
        let mut gray = src();
        contrast(&mut gray, 1.3);
        assert_eq!(
            gray.pixels(),
            include_bytes!("../../tests/fixtures/contrast_1_3.bin")
        );
    }

    #[test]
    fn prepare_produces_compensated_dimensions() {
        // A 400x300 source at single density: width 210, aspect height
        // round(300 * 210 / 400) = 158, compensated round(158 * 72 / 84.7) = 134.
        let source = DynamicImage::new_luma8(400, 300);
        let prepared = ImagePipeline::new().prepare(&source).unwrap();
        let expected_aspect = (300.0f64 * 210.0 / 400.0).round_ties_even();
        let expected_h =
            (expected_aspect * 72.0 / DeviceProfile::SP700.horizontal_dpi).round_ties_even() as u32;
        assert_eq!(prepared.image.bitmap.width(), 210);
        assert_eq!(prepared.image.bitmap.height(), expected_h);
        // Preview keeps square pixels at display width.
        assert_eq!(prepared.preview.width(), 210);
        assert_eq!(prepared.preview.height(), 158);
    }

    #[test]
    fn thermal_double_resolution_doubles_the_rows() {
        // 400x300 at 576 dots wide: aspect height round(300 * 576 / 400) = 432;
        // at 406.4 DPI vertical vs 203.2 horizontal that becomes 864 rows.
        let source = DynamicImage::new_luma8(400, 300);
        let prepared = ImagePipeline::new()
            .profile(DeviceProfile::THERMAL_80MM_DOUBLE_RESOLUTION)
            .prepare(&source)
            .unwrap();
        assert_eq!(prepared.image.bitmap().width(), 576);
        assert_eq!(prepared.image.bitmap().height(), 864);
        // Single resolution keeps the aspect ratio as-is.
        let prepared = ImagePipeline::new()
            .profile(DeviceProfile::THERMAL_80MM)
            .prepare(&source)
            .unwrap();
        assert_eq!(prepared.image.bitmap().height(), 432);
    }

    #[test]
    fn normal_thermal_preview_matches_the_printed_dots() {
        let source = image::GrayImage::from_raw(W, H, src().into_pixels()).unwrap();
        let source = DynamicImage::ImageLuma8(source);
        for dither in [
            Dithering::FloydSteinberg { threshold: 128 },
            Dithering::Atkinson { threshold: 128 },
            Dithering::Threshold { threshold: 128 },
            Dithering::Bayer8x8,
        ] {
            let prepared = ImagePipeline::new()
                .profile(DeviceProfile::THERMAL_80MM)
                .dither(dither)
                .prepare(&source)
                .unwrap();
            assert_eq!(&prepared.preview.to_bitmap(), prepared.image.bitmap());
        }
    }

    #[test]
    fn double_density_preview_is_min_pooled_to_display_width() {
        let source = DynamicImage::new_luma8(400, 300);
        let prepared = ImagePipeline::new()
            .density(Density::Double)
            .prepare(&source)
            .unwrap();
        assert_eq!(prepared.image.bitmap.width(), 420);
        assert_eq!(prepared.preview.width(), 210);
    }
}
