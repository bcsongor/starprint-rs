//! Image preparation pipeline (requires the `image` cargo feature).
//!
//! Turns an ordinary picture into a print-ready [`BitImage`] using the
//! tone-mapping pipeline that was dialled in on real SP700 hardware (the
//! thermal [`DeviceProfile`]s reuse the same tuning; only the geometry
//! differs):
//!
//! 1. flatten transparency onto white and convert to 8-bit grayscale,
//! 2. auto-contrast (linear histogram stretch),
//! 3. gamma 1.8 (darken midtones),
//! 4. unsharp mask (radius 2.4, 150 %),
//! 5. histogram equalisation,
//! 6. at double density: brighten ×1.2 to offset physical dot overlap,
//! 7. optional user brightness/contrast adjustments,
//! 8. resize to the head width — compensating for the anisotropic
//!    resolution (≈ 84.7 DPI horizontally vs 72 DPI vertically, so a
//!    naive resize would print stretched),
//! 9. dither to 1-bit.
//!
//! The LUT-based stages (2, 3, 5, 6, 7) reproduce the reference
//! implementation byte-for-byte and are verified against fixtures it
//! generated. Resampling and the unsharp blur are mathematically
//! equivalent but not bit-identical (Lanczos3 and a true Gaussian in
//! place of Pillow's C resampler and box-blur approximation).
//!
//! # Examples
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

use super::{BitImage, Density, DeviceProfile};
use super::{Dithering, Grayscale};
use crate::error::{Error, Result};

/// Gamma applied after auto-contrast; > 1 darkens midtones so they
/// survive dithering on paper.
const GAMMA_MIDTONE_DARKENING: f64 = 1.8;
/// Base sharpening strength; the unsharp radius is `3 ×` this value.
const SHARPEN_SIGMA: f64 = 0.8;
/// Unsharp-mask amount in percent.
const UNSHARP_PERCENT: i32 = 150;
/// Post-equalise brightness lift for double density, compensating for
/// the physical dot overlap when dots are printed at half the normal
/// spacing.
const DOUBLE_DENSITY_BRIGHTNESS: f64 = 1.2;

/// A configured image-preparation pipeline, in the style of
/// [`std::fs::OpenOptions`]: chain settings, then call
/// [`prepare`](Self::prepare) or [`prepare_bytes`](Self::prepare_bytes).
///
/// [`ImagePipeline::new`] gives the hardware-tuned reference behaviour:
/// single density, Floyd–Steinberg at threshold 128, no user
/// adjustments, SP700 profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagePipeline {
    density: Density,
    dither: Dithering,
    brightness: f64,
    contrast: f64,
    profile: DeviceProfile,
}

impl Default for ImagePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ImagePipeline {
    /// A pipeline with the hardware-tuned defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            density: Density::Single,
            dither: Dithering::default(),
            brightness: 1.0,
            contrast: 1.0,
            profile: DeviceProfile::SP700,
        }
    }

    /// Sets the horizontal dot density to render for (default: single).
    #[must_use]
    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// Sets the dithering algorithm (default: Floyd–Steinberg at
    /// threshold 128).
    #[must_use]
    pub fn dither(mut self, dither: Dithering) -> Self {
        self.dither = dither;
        self
    }

    /// Sets a user brightness multiplier applied after the standard
    /// pipeline (default 1.0 = unchanged).
    #[must_use]
    pub fn brightness(mut self, factor: f64) -> Self {
        self.brightness = factor;
        self
    }

    /// Sets a user contrast factor applied after the standard pipeline
    /// (default 1.0 = unchanged).
    #[must_use]
    pub fn contrast(mut self, factor: f64) -> Self {
        self.contrast = factor;
        self
    }

    /// Sets the printer head geometry (default:
    /// [`DeviceProfile::SP700`]).
    #[must_use]
    pub fn profile(mut self, profile: DeviceProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Decodes an encoded image (PNG, JPEG, WebP, BMP) and runs
    /// [`prepare`](Self::prepare).
    pub fn prepare_bytes(&self, bytes: &[u8]) -> Result<PreparedImage> {
        let decoded = image::load_from_memory(bytes).map_err(|err| Error::InvalidData {
            reason: format!("image decode failed: {err}"),
        })?;
        self.prepare(&decoded)
    }

    /// Runs the full preparation pipeline on a decoded image.
    pub fn prepare(&self, source: &DynamicImage) -> Result<PreparedImage> {
        prepare(source, self)
    }
}

/// The result of [`ImagePipeline::prepare`]: the print-ready image plus
/// an on-screen preview.
#[derive(Debug, Clone)]
pub struct PreparedImage {
    /// Validated bit image, ready for
    /// [`bit_image`](crate::Builder::bit_image).
    pub image: BitImage,
    /// A dithered preview at single-density width with square pixels.
    /// At double density, horizontal pixel pairs are min-pooled to
    /// simulate the physical dot overlap, so the preview matches what
    /// the paper will show.
    pub preview: Grayscale,
}

fn prepare(source: &DynamicImage, options: &ImagePipeline) -> Result<PreparedImage> {
    let mut gray = to_grayscale_on_white(source);
    let (src_w, src_h) = (gray.width(), gray.height());
    if src_w == 0 || src_h == 0 {
        return Err(Error::InvalidData {
            reason: "source image has zero width or height".into(),
        });
    }

    // Tone-mapping pipeline.
    autocontrast(&mut gray);
    apply_lut(&mut gray, &gamma_lut(GAMMA_MIDTONE_DARKENING));
    gray = unsharp_mask(&gray, SHARPEN_SIGMA * 3.0, UNSHARP_PERCENT, 0);
    equalize(&mut gray);
    if options.density == Density::Double {
        brightness(&mut gray, DOUBLE_DENSITY_BRIGHTNESS);
    }

    // User adjustments (after the standard pipeline, before dithering).
    if options.brightness != 1.0 {
        brightness(&mut gray, options.brightness);
    }
    if options.contrast != 1.0 {
        contrast(&mut gray, options.contrast);
    }

    let width = options.profile.max_width(options.density);
    let h_dpi = options.profile.horizontal_dpi_at(options.density);

    // Print data: aspect-preserving width fit, then vertical compensation
    // for the anisotropic dot pitch.
    let aspect_h = ratio_round(src_h, width, src_w);
    let compensated_h =
        (f64::from(aspect_h) * options.profile.vertical_dpi / h_dpi).round_ties_even() as u32;
    if compensated_h == 0 {
        return Err(Error::InvalidData {
            reason: "image is too wide and short to print at this width".into(),
        });
    }
    // Hand the tone-mapped pixels to the `image` crate once (no copy) and
    // resample from it for both the print data and the preview.
    let full = image::GrayImage::from_raw(src_w, src_h, gray.into_pixels())
        .expect("buffer sized from dimensions");
    let print_gray = options.dither.apply(&resize(&full, width, compensated_h));
    let image = BitImage::with_profile(print_gray.to_bitmap(), options.density, &options.profile)?;

    // Preview: dither at print width and display height (square pixels),
    // then simulate double-density dot overlap by min-pooling pairs.
    let display_w = options.profile.width_dots_single;
    let display_h = ratio_round(src_h, display_w, src_w).max(1);
    let preview_gray = options.dither.apply(&resize(&full, width, display_h));
    let preview = if width == display_w {
        preview_gray
    } else {
        min_pool_pairs(&preview_gray)
    };

    Ok(PreparedImage { image, preview })
}

/// `round(a * b / c)` with Python's banker's rounding.
fn ratio_round(a: u32, b: u32, c: u32) -> u32 {
    (f64::from(a) * f64::from(b) / f64::from(c)).round_ties_even() as u32
}

/// Flattens transparency onto white and converts to grayscale with
/// Pillow's "L" formula (rounded ITU-R 601-2 luma).
///
/// The common 8-bit layouts are read in place; anything else goes
/// through an RGBA8 conversion first. All paths produce identical bytes.
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

/// Linear histogram stretch (Pillow `ImageOps.autocontrast`, cutoff 0).
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

/// `out = (in / 255) ^ gamma * 255`, truncated like the reference LUT.
fn gamma_lut(gamma: f64) -> [u8; 256] {
    let mut lut = [0u8; 256];
    for (i, out) in lut.iter_mut().enumerate() {
        *out = (((i as f64 / 255.0).powf(gamma) * 255.0) as i32).min(255) as u8;
    }
    lut
}

/// Histogram equalisation (Pillow `ImageOps.equalize`).
fn equalize(gray: &mut Grayscale) {
    let h = histogram(gray.pixels());
    let nonzero: Vec<u64> = h
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| u64::from(c))
        .collect();
    if nonzero.len() <= 1 {
        return;
    }
    let step = (nonzero.iter().sum::<u64>() - nonzero.last().unwrap()) / 255;
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

/// Pillow `ImageEnhance.Brightness`: blend towards black, i.e.
/// `out = px * factor`, truncated and clipped like Pillow's C blend.
fn brightness(gray: &mut Grayscale, factor: f64) {
    for p in gray.pixels_mut() {
        *p = blend_channel(0.0, f64::from(*p), factor);
    }
}

/// Pillow `ImageEnhance.Contrast`: blend away from the rounded image
/// mean, i.e. `out = mean + factor * (px - mean)`.
fn contrast(gray: &mut Grayscale, factor: f64) {
    let h = histogram(gray.pixels());
    let total: u64 = h.iter().map(|&c| u64::from(c)).sum();
    let weighted: u64 = h
        .iter()
        .enumerate()
        .map(|(i, &c)| i as u64 * u64::from(c))
        .sum();
    let mean = (weighted as f64 / total as f64 + 0.5) as i32; // round half up
    for p in gray.pixels_mut() {
        *p = blend_channel(f64::from(mean), f64::from(*p), factor);
    }
}

/// Pillow's C `ImagingBlend` per-channel arithmetic:
/// `out = a + factor * (b - a)`, clipped to 0–255 and truncated.
fn blend_channel(a: f64, b: f64, factor: f64) -> u8 {
    let out = a + factor * (b - a);
    if out <= 0.0 {
        0
    } else if out >= 255.0 {
        255
    } else {
        out as u8
    }
}

/// Unsharp mask matching Pillow's integer arithmetic, but over a true
/// Gaussian blur (Pillow approximates one with box blurs).
fn unsharp_mask(gray: &Grayscale, radius: f64, percent: i32, threshold: i32) -> Grayscale {
    let blurred = gaussian_blur(gray, radius);
    let pixels = gray
        .pixels()
        .iter()
        .zip(blurred.pixels())
        .map(|(&px, &blur)| {
            let diff = i32::from(px) - i32::from(blur);
            if diff.abs() > threshold {
                (i32::from(px) + diff * percent / 100).clamp(0, 255) as u8
            } else {
                px
            }
        })
        .collect();
    Grayscale::new(gray.width(), gray.height(), pixels).expect("same dimensions")
}

/// Separable Gaussian blur with `sigma = radius` and clamped edges.
///
/// Both passes run row by row over contiguous memory (the horizontal pass
/// over an edge-padded copy of each row, the vertical pass by accumulating
/// whole source rows into the output row), which keeps the inner loops
/// cache-friendly and auto-vectorisable.
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

    // Horizontal pass: each row is edge-padded by `taps` so every output
    // pixel is a plain dot product with the kernel.
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

    // Vertical pass: accumulate weighted (edge-clamped) source rows.
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

/// Lanczos3 resize via the `image` crate (the counterpart of Pillow's
/// `LANCZOS` resampling).
fn resize(source: &image::GrayImage, width: u32, height: u32) -> Grayscale {
    let resized = image::imageops::resize(source, width, height, FilterType::Lanczos3);
    Grayscale::new(width, height, resized.into_raw()).expect("buffer sized from dimensions")
}

/// Collapses horizontal pixel pairs via MIN, simulating physical dot
/// overlap in double density: if either dot in a pair is inked, the
/// wider physical dot covers both positions.
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
