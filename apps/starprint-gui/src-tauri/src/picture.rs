//! Picture printing: a photo from disk through the `starprint` image
//! pipeline, previewed as a PNG and printed as a bit image (impact) or a
//! raster (thermal).

use std::sync::Mutex;

use image::DynamicImage;
use image::imageops::FilterType;
use serde::Deserialize;
use starprint::graphics::{Density, DeviceProfile, Dithering, ImagePipeline, PreparedImage};
use starprint::{Alignment, Builder, Cut, Document, Impact, PrintMode, RasterQuality, StarLine};

use crate::{Paper, PrinterKind, preview};

/// Mirrors [`Dithering`] without its threshold, which is a separate field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Dither {
    FloydSteinberg,
    Atkinson,
    Threshold,
    Bayer,
}

impl Dither {
    fn dithering(self, threshold: u8) -> Dithering {
        match self {
            Self::FloydSteinberg => Dithering::FloydSteinberg { threshold },
            Self::Atkinson => Dithering::Atkinson { threshold },
            Self::Threshold => Dithering::Threshold { threshold },
            Self::Bayer => Dithering::Bayer8x8,
        }
    }
}

/// The picture and how to render it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Picture {
    /// Path of the image file on disk.
    pub path: String,
    /// Impact: double horizontal density (420 dots). Thermal: the
    /// double-resolution print mode (16 rows/mm).
    pub double: bool,
    pub dither: Dither,
    /// Black point for the dithers that take one, 1..=255.
    pub threshold: u8,
    /// 1.0 leaves the picture as is.
    pub brightness: f64,
    /// 1.0 leaves the picture as is.
    pub contrast: f64,
}

/// The pipeline sharpens and equalises at source resolution before it
/// resizes, so a camera photo costs seconds per preview. Feeding it a
/// copy at most this many times the head width keeps the sharpening
/// comparable while making the preview follow the sliders.
const SOURCE_OVERSAMPLE: u32 = 2;

/// The last decoded picture, shrunk for one head width, so that slider
/// changes do not decode and shrink the file again.
#[derive(Default)]
pub struct SourceCache(Mutex<Option<CachedSource>>);

struct CachedSource {
    path: String,
    max_width: u32,
    image: DynamicImage,
}

impl SourceCache {
    fn load(&self, path: &str, max_width: u32) -> Result<DynamicImage, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = slot
            .as_ref()
            .filter(|c| c.path == path && c.max_width == max_width)
        {
            return Ok(cached.image.clone());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let mut image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        if image.width() > max_width {
            let height = (u64::from(image.height()) * u64::from(max_width)
                / u64::from(image.width()))
            .max(1) as u32;
            image = image.resize_exact(max_width, height, FilterType::Triangle);
        }
        *slot = Some(CachedSource {
            path: path.to_owned(),
            max_width,
            image: image.clone(),
        });
        Ok(image)
    }
}

impl Picture {
    fn profile(&self, kind: PrinterKind, paper: Paper) -> DeviceProfile {
        match (kind, paper, self.double) {
            (PrinterKind::Impact, _, _) => DeviceProfile::SP700,
            (PrinterKind::Thermal, Paper::Mm80, false) => DeviceProfile::THERMAL_80MM,
            (PrinterKind::Thermal, Paper::Mm80, true) => {
                DeviceProfile::THERMAL_80MM_DOUBLE_RESOLUTION
            }
            (PrinterKind::Thermal, Paper::Mm112, false) => DeviceProfile::THERMAL_112MM,
            (PrinterKind::Thermal, Paper::Mm112, true) => {
                DeviceProfile::THERMAL_112MM_DOUBLE_RESOLUTION
            }
        }
    }

    /// Decodes and prepares the picture for this head.
    pub fn prepare(
        &self,
        kind: PrinterKind,
        paper: Paper,
        cache: &SourceCache,
    ) -> Result<PreparedImage, String> {
        if self.path.trim().is_empty() {
            return Err("No picture chosen.".to_owned());
        }
        let profile = self.profile(kind, paper);
        let density = match (kind, self.double) {
            (PrinterKind::Impact, true) => Density::Double,
            _ => Density::Single,
        };
        let source = cache.load(&self.path, profile.max_width(density) * SOURCE_OVERSAMPLE)?;
        ImagePipeline::new()
            .profile(profile)
            .density(density)
            .dither(self.dither.dithering(self.threshold))
            .brightness(self.brightness)
            .contrast(self.contrast)
            .prepare(&source)
            .map_err(|e| e.to_string())
    }

    /// The dithered preview as a PNG, at single-density width with square
    /// pixels: what the paper will show. On thermal heads the dots are
    /// widened as the head's own bloom widens them, see
    /// [`preview::dot_gain`].
    pub fn preview_png(
        &self,
        kind: PrinterKind,
        paper: Paper,
        cache: &SourceCache,
    ) -> Result<Vec<u8>, String> {
        let preview = self.prepare(kind, paper, cache)?.preview;
        let shown = match kind {
            PrinterKind::Thermal => preview::dot_gain(&preview, self.thermal_dot_size()),
            PrinterKind::Impact => preview,
        };
        preview::png(&shown)
    }

    /// Printed dot diameter as a percentage of the dot pitch, matched
    /// against a step wedge on a TSP700II at slow speed, density +3. In
    /// double-resolution mode the rows are half as far apart, so each dot
    /// overlaps its neighbours far more and the paper prints darker.
    fn thermal_dot_size(&self) -> u32 {
        if self.double { 200 } else { 150 }
    }
}

pub fn thermal(
    builder: Builder<StarLine>,
    picture: &Picture,
    paper: Paper,
    cut: bool,
    cache: &SourceCache,
) -> Result<Document, String> {
    let prepared = picture.prepare(PrinterKind::Thermal, paper, cache)?;
    let mut doc = builder.align(Alignment::Center);
    if picture.double {
        doc = doc.print_mode(PrintMode::DoubleResolution);
    }
    doc = doc.raster(&prepared.image, RasterQuality::High);
    if picture.double {
        // The mode outlives ESC @, so switch back explicitly.
        doc = doc.print_mode(PrintMode::SingleColor);
    }
    Ok(finish(doc, cut))
}

pub fn impact(
    builder: Builder<Impact>,
    picture: &Picture,
    cut: bool,
    cache: &SourceCache,
) -> Result<Document, String> {
    // The SP700 has one head width, so the roll does not come into it.
    let prepared = picture.prepare(PrinterKind::Impact, Paper::Mm80, cache)?;
    let doc = builder.align(Alignment::Center).bit_image(&prepared.image);
    Ok(finish(doc, cut))
}

fn finish<P: starprint::Protocol>(doc: Builder<P>, cut: bool) -> Document {
    if cut {
        doc.feed(2).cut(Cut::FeedThenPartial).build()
    } else {
        doc.feed(3).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(path: &str, double: bool) -> Picture {
        Picture {
            path: path.to_owned(),
            double,
            dither: Dither::FloydSteinberg,
            threshold: 128,
            brightness: 1.0,
            contrast: 1.0,
        }
    }

    fn sample() -> tempfile::NamedTempFile {
        let mut image = image::GrayImage::new(64, 32);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Luma([((x * 4) ^ (y * 8)) as u8]);
        }
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        image.save(file.path()).unwrap();
        file
    }

    #[test]
    fn missing_path_is_an_error() {
        assert_eq!(
            picture("", false)
                .prepare(PrinterKind::Impact, Paper::Mm80, &SourceCache::default())
                .unwrap_err(),
            "No picture chosen."
        );
    }

    #[test]
    fn cache_shrinks_to_the_oversampled_width() {
        let mut image = image::GrayImage::new(4000, 1000);
        image.put_pixel(0, 0, image::Luma([0]));
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        image.save(file.path()).unwrap();
        let cache = SourceCache::default();
        let loaded = cache.load(file.path().to_str().unwrap(), 840).unwrap();
        assert_eq!((loaded.width(), loaded.height()), (840, 210));
        assert!(cache.0.lock().unwrap().is_some());
    }

    #[test]
    fn impact_double_density_uses_esc_caret_1() {
        let file = sample();
        let path = file.path().to_str().unwrap();
        let cache = SourceCache::default();
        let doc = impact(starprint::impact(), &picture(path, true), true, &cache).unwrap();
        let bytes = doc.as_bytes();
        assert!(bytes.windows(3).any(|w| w == [0x1b, b'^', 1]));
        assert!(bytes.ends_with(&[0x1b, b'd', 3]));
    }

    #[test]
    fn thermal_double_resolution_switches_back() {
        let file = sample();
        let path = file.path().to_str().unwrap();
        let cache = SourceCache::default();
        let doc = thermal(
            starprint::starline(),
            &picture(path, true),
            Paper::Mm80,
            false,
            &cache,
        )
        .unwrap();
        let bytes = doc.as_bytes();
        let modes: Vec<u8> = bytes
            .windows(4)
            .filter(|w| w[..3] == [0x1b, 0x1e, b'C'])
            .map(|w| w[3])
            .collect();
        assert!(modes.len() >= 2, "sets the mode twice: {modes:?}");
        assert!(!bytes.ends_with(&[0x1b, b'd', 3]));
    }

    #[test]
    fn preview_is_a_png_at_single_width() {
        let file = sample();
        let path = file.path().to_str().unwrap();
        let png = picture(path, true)
            .preview_png(PrinterKind::Impact, Paper::Mm80, &SourceCache::default())
            .unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!(decoded.width(), 210);
    }
}
