//! A photo, printed as a bit image (impact) or a raster (thermal).
//!
//! The image comes from the caller: the desktop app reads a file it was
//! pointed at, the API takes the bytes of a multipart part. Nothing here
//! opens one.

use image::DynamicImage;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use starprint::graphics::{BitImage, Density, DeviceProfile, Dithering, Grayscale, ImagePipeline};
use starprint::{Alignment, Builder, Document, Impact, PrintMode, RasterQuality, StarLine};

use crate::{Paper, PrinterKind, finish_graphic};

/// [`Dithering`] without its threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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

/// Everything but the image itself. A caller that supplies none of it
/// gets the reference settings: Floyd-Steinberg at 128, untouched tone.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Picture {
    /// Impact: double density. Thermal: double-resolution mode.
    pub double: bool,
    pub dither: Dither,
    /// Ignored by Bayer.
    pub threshold: u8,
    pub brightness: f64,
    pub contrast: f64,
}

impl Default for Picture {
    fn default() -> Self {
        Self {
            double: false,
            dither: Dither::FloydSteinberg,
            threshold: 128,
            brightness: 1.0,
            contrast: 1.0,
        }
    }
}

/// The pipeline sharpens at source resolution, so a camera photo costs
/// seconds per preview. Shrinking it to this many times the head width
/// first keeps the sharpening comparable and the sliders responsive.
const SOURCE_OVERSAMPLE: u32 = 2;

/// Shrinks to at most `max_width`, keeping the aspect ratio, or `None`
/// when the image is already no wider.
#[must_use]
pub fn fit_width(image: &DynamicImage, max_width: u32) -> Option<DynamicImage> {
    if image.width() <= max_width {
        return None;
    }
    let height =
        (u64::from(image.height()) * u64::from(max_width) / u64::from(image.width())).max(1) as u32;
    Some(image.resize_exact(max_width, height, FilterType::Triangle))
}

impl Picture {
    fn profile(&self, kind: PrinterKind, paper: Paper) -> DeviceProfile {
        match (kind, paper, self.double) {
            (PrinterKind::Impact, _, _) => DeviceProfile::SP700,
            (PrinterKind::Thermal, _, false) => paper.profile().clone(),
            (PrinterKind::Thermal, Paper::Mm80, true) => {
                DeviceProfile::THERMAL_80MM_DOUBLE_RESOLUTION
            }
            (PrinterKind::Thermal, Paper::Mm112, true) => {
                DeviceProfile::THERMAL_112MM_DOUBLE_RESOLUTION
            }
        }
    }

    fn density(&self, kind: PrinterKind) -> Density {
        match (kind, self.double) {
            (PrinterKind::Impact, true) => Density::Double,
            _ => Density::Single,
        }
    }

    /// What [`prepare`](Self::prepare) shrinks its source to. A caller
    /// that caches decoded images shrinks to this width once, so the
    /// result is the same either way.
    pub fn source_width(&self, kind: PrinterKind, paper: Paper) -> u32 {
        self.profile(kind, paper).max_width(self.density(kind)) * SOURCE_OVERSAMPLE
    }

    /// The picture as it prints.
    pub fn prepare(
        &self,
        kind: PrinterKind,
        paper: Paper,
        source: &DynamicImage,
    ) -> Result<BitImage, String> {
        let shrunk = fit_width(source, self.source_width(kind, paper));
        self.pipeline(kind, paper)
            .prepare(shrunk.as_ref().unwrap_or(source))
            .map_err(|e| e.to_string())
    }

    /// The picture for the screen, one head wide.
    pub fn preview(
        &self,
        kind: PrinterKind,
        paper: Paper,
        source: &DynamicImage,
    ) -> Result<Grayscale, String> {
        let shrunk = fit_width(source, self.source_width(kind, paper));
        self.pipeline(kind, paper)
            .prepare_preview(shrunk.as_ref().unwrap_or(source))
            .map_err(|e| e.to_string())
    }

    fn pipeline(&self, kind: PrinterKind, paper: Paper) -> ImagePipeline {
        ImagePipeline::new()
            .profile(self.profile(kind, paper))
            .density(self.density(kind))
            .dither(self.dither.dithering(self.threshold))
            .brightness(self.brightness)
            .contrast(self.contrast)
    }
}

/// PNG, JPEG, WebP or BMP, whatever the caller called it. The message
/// is written for whoever sent the bytes, since both front ends pass it
/// straight back.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    image::load_from_memory(bytes).map_err(|e| format!("The image could not be decoded: {e}"))
}

pub(crate) fn thermal(
    builder: Builder<StarLine>,
    picture: &Picture,
    paper: Paper,
    cut: bool,
    source: &DynamicImage,
    mode: PrintMode,
) -> Result<Document, String> {
    let image = picture.prepare(PrinterKind::Thermal, paper, source)?;
    let mut doc = builder.align(Alignment::Center);
    if picture.double {
        doc = doc.print_mode(PrintMode::DoubleResolution);
        if mode == PrintMode::TwoColor {
            doc = doc.print_density(3);
        }
    }
    doc = doc.raster(&image, RasterQuality::High);
    if picture.double {
        // The mode outlives ESC @, so go back to the job's own.
        doc = doc.print_mode(mode);
    }
    Ok(finish_graphic(doc, cut))
}

pub(crate) fn impact(
    builder: Builder<Impact>,
    picture: &Picture,
    cut: bool,
    source: &DynamicImage,
) -> Result<Document, String> {
    let image = picture.prepare(PrinterKind::Impact, Paper::Mm80, source)?;
    let doc = builder.align(Alignment::Center).bit_image(&image);
    Ok(finish_graphic(doc, cut))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(double: bool) -> Picture {
        Picture {
            double,
            ..Picture::default()
        }
    }

    fn sample() -> DynamicImage {
        let mut image = image::GrayImage::new(64, 32);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Luma([((x * 4) ^ (y * 8)) as u8]);
        }
        DynamicImage::ImageLuma8(image)
    }

    #[test]
    fn undecodable_bytes_are_an_error() {
        assert!(decode(b"not an image").is_err());
    }

    #[test]
    fn preparing_shrinks_to_the_oversampled_width() {
        let mut image = image::GrayImage::new(4000, 1000);
        image.put_pixel(0, 0, image::Luma([0]));
        let source = DynamicImage::ImageLuma8(image);

        let picture = picture(false);
        assert_eq!(
            picture.source_width(PrinterKind::Impact, Paper::Mm80),
            420,
            "210 dots of head, twice over"
        );
        let shrunk = fit_width(&source, 840).unwrap();
        assert_eq!((shrunk.width(), shrunk.height()), (840, 210));

        // Shrinking to the same width first changes nothing, so a cache
        // that holds the shrunk source prints what the full one would.
        let direct = picture
            .preview(PrinterKind::Impact, Paper::Mm80, &source)
            .unwrap();
        let cached = picture
            .preview(
                PrinterKind::Impact,
                Paper::Mm80,
                &fit_width(
                    &source,
                    picture.source_width(PrinterKind::Impact, Paper::Mm80),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(direct.pixels(), cached.pixels());
    }

    #[test]
    fn fit_width_leaves_a_narrow_image_alone() {
        assert!(fit_width(&sample(), 640).is_none());
        assert!(
            fit_width(&sample(), 64).is_none(),
            "nor one exactly as wide"
        );
    }

    #[test]
    fn impact_double_density_uses_esc_caret_1() {
        let doc = impact(starprint::impact(), &picture(true), true, &sample()).unwrap();
        let bytes = doc.as_bytes();
        assert!(bytes.windows(3).any(|w| w == [0x1b, b'^', 1]));
        assert!(bytes.ends_with(&[0x1b, b'd', 3]));
    }

    #[test]
    fn thermal_double_resolution_switches_back() {
        let doc = thermal(
            starprint::starline(),
            &picture(true),
            Paper::Mm80,
            false,
            &sample(),
            PrintMode::SingleColor,
        )
        .unwrap();
        let bytes = doc.as_bytes();
        let modes: Vec<u8> = bytes
            .windows(4)
            .filter(|w| w[..3] == [0x1b, 0x1e, b'C'])
            .map(|w| w[3])
            .collect();
        assert!(modes.contains(&32), "selects double resolution: {modes:?}");
        assert_eq!(
            modes.last(),
            Some(&0),
            "and leaves the printer in single colour: {modes:?}"
        );
        assert!(!bytes.ends_with(&[0x1b, b'd', 3]));
    }

    #[test]
    fn the_preview_is_one_head_wide() {
        let preview = picture(true)
            .preview(PrinterKind::Impact, Paper::Mm80, &sample())
            .unwrap();
        assert_eq!(preview.width(), 210);
    }
}
