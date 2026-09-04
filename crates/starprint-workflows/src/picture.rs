//! A photo, printed as a bit image (impact) or a raster (thermal).
//!
//! The image comes from the caller: the desktop app reads a file it was
//! pointed at, the API takes the bytes of a multipart part. Nothing here
//! opens one.

use image::DynamicImage;
use image::imageops::FilterType;
use serde::Deserialize;
use starprint::graphics::{Density, DeviceProfile, Dithering, ImagePipeline, PreparedImage};
use starprint::{Alignment, Builder, Document, Impact, PrintMode, RasterQuality, StarLine};

use crate::{Paper, PrinterKind, finish};

/// [`Dithering`] without its threshold.
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

/// Everything but the image itself. A caller that supplies none of it
/// gets the reference settings: Floyd-Steinberg at 128, untouched tone.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
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

/// Shrinks to at most `max_width`, keeping the aspect ratio. Anything
/// narrower comes back untouched.
#[must_use]
pub fn fit_width(image: DynamicImage, max_width: u32) -> DynamicImage {
    if image.width() <= max_width {
        return image;
    }
    let height =
        (u64::from(image.height()) * u64::from(max_width) / u64::from(image.width())).max(1) as u32;
    image.resize_exact(max_width, height, FilterType::Triangle)
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

    pub fn prepare(
        &self,
        kind: PrinterKind,
        paper: Paper,
        source: &DynamicImage,
    ) -> Result<PreparedImage, String> {
        let width = self.source_width(kind, paper);
        let shrunk;
        let source = if source.width() > width {
            shrunk = fit_width(source.clone(), width);
            &shrunk
        } else {
            source
        };
        ImagePipeline::new()
            .profile(self.profile(kind, paper))
            .density(self.density(kind))
            .dither(self.dither.dithering(self.threshold))
            .brightness(self.brightness)
            .contrast(self.contrast)
            .prepare(source)
            .map_err(|e| e.to_string())
    }

    /// Dot diameter as a percentage of the pitch, matched against a step
    /// wedge on a TSP700II at slow speed, density +3. Rows are half as far
    /// apart in double resolution, so the overlap is larger.
    pub fn thermal_dot_size(&self) -> u32 {
        if self.double { 200 } else { 150 }
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
) -> Result<Document, String> {
    let prepared = picture.prepare(PrinterKind::Thermal, paper, source)?;
    let mut doc = builder.align(Alignment::Center);
    if picture.double {
        doc = doc.print_mode(PrintMode::DoubleResolution);
    }
    doc = doc.raster(&prepared.image, RasterQuality::High);
    if picture.double {
        // The mode outlives ESC @.
        doc = doc.print_mode(PrintMode::SingleColor);
    }
    Ok(finish(margin(doc, cut), cut))
}

pub(crate) fn impact(
    builder: Builder<Impact>,
    picture: &Picture,
    cut: bool,
    source: &DynamicImage,
) -> Result<Document, String> {
    let prepared = picture.prepare(PrinterKind::Impact, Paper::Mm80, source)?;
    let doc = builder.align(Alignment::Center).bit_image(&prepared.image);
    Ok(finish(margin(doc, cut), cut))
}

/// Blank paper under the image before a cut. Without one, the feed that
/// ends the job is margin enough.
pub(crate) fn margin<P: starprint::Protocol>(doc: Builder<P>, cut: bool) -> Builder<P> {
    if cut { doc.feed(2) } else { doc }
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
        let shrunk = fit_width(source.clone(), 840);
        assert_eq!((shrunk.width(), shrunk.height()), (840, 210));

        // Shrinking to the same width first changes nothing, so a cache
        // that holds the shrunk source prints what the full one would.
        let direct = picture
            .prepare(PrinterKind::Impact, Paper::Mm80, &source)
            .unwrap();
        let cached = picture
            .prepare(
                PrinterKind::Impact,
                Paper::Mm80,
                &fit_width(
                    source,
                    picture.source_width(PrinterKind::Impact, Paper::Mm80),
                ),
            )
            .unwrap();
        assert_eq!(direct.preview.pixels(), cached.preview.pixels());
    }

    #[test]
    fn fit_width_leaves_a_narrow_image_alone() {
        let image = sample();
        assert_eq!(fit_width(image.clone(), 640).width(), 64);
        assert_eq!(fit_width(image, 64).width(), 64);
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
        let prepared = picture(true)
            .prepare(PrinterKind::Impact, Paper::Mm80, &sample())
            .unwrap();
        assert_eq!(prepared.preview.width(), 210);
    }
}
