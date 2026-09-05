//! Reading a picture off disk. The shared crate takes image data, so
//! choosing the file and decoding it fall to the app.

use std::sync::{Arc, Mutex};

use image::DynamicImage;
use serde::Deserialize;
use starprint_workflows::picture::{decode, fit_width};
use starprint_workflows::{Paper, Picture, PrinterKind};

/// A [`Picture`] and the file it prints.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PicturePath {
    /// Empty until one is chosen.
    pub path: String,
    #[serde(flatten)]
    pub picture: Picture,
}

impl PicturePath {
    /// The decoded file, shrunk to the width the pipeline would shrink
    /// it to anyway.
    pub fn source(
        &self,
        kind: PrinterKind,
        paper: Paper,
        cache: &SourceCache,
    ) -> Result<Arc<DynamicImage>, String> {
        if self.path.trim().is_empty() {
            return Err("No picture chosen.".to_owned());
        }
        cache.load(&self.path, self.picture.source_width(kind, paper))
    }
}

/// The last decoded picture, shrunk for one head width, so dragging a
/// slider does not read and decode the file again. Shared, not copied,
/// with each preview.
#[derive(Default)]
pub struct SourceCache(Mutex<Option<CachedSource>>);

struct CachedSource {
    path: String,
    max_width: u32,
    image: Arc<DynamicImage>,
}

impl SourceCache {
    fn load(&self, path: &str, max_width: u32) -> Result<Arc<DynamicImage>, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = slot
            .as_ref()
            .filter(|c| c.path == path && c.max_width == max_width)
        {
            return Ok(Arc::clone(&cached.image));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let image = decode(&bytes)?;
        let image = Arc::new(fit_width(&image, max_width).unwrap_or(image));
        *slot = Some(CachedSource {
            path: path.to_owned(),
            max_width,
            image: Arc::clone(&image),
        });
        Ok(image)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A PNG on disk for the tests that need one, here and in `job`.
    pub fn sample(width: u32, height: u32) -> tempfile::NamedTempFile {
        let mut image = image::GrayImage::new(width, height);
        image.put_pixel(0, 0, image::Luma([0]));
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        image.save(file.path()).unwrap();
        file
    }

    pub fn at(path: &str) -> PicturePath {
        PicturePath {
            path: path.to_owned(),
            picture: Picture::default(),
        }
    }

    #[test]
    fn a_missing_path_is_an_error() {
        assert_eq!(
            at("")
                .source(PrinterKind::Impact, Paper::Mm80, &SourceCache::default())
                .unwrap_err(),
            "No picture chosen."
        );
    }

    #[test]
    fn a_file_that_is_not_an_image_is_an_error() {
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(file.path(), b"not an image").unwrap();
        assert!(
            at(file.path().to_str().unwrap())
                .source(PrinterKind::Impact, Paper::Mm80, &SourceCache::default())
                .is_err()
        );
    }

    #[test]
    fn the_cache_shrinks_to_the_oversampled_width() {
        let file = sample(4000, 1000);
        let cache = SourceCache::default();
        let loaded = cache.load(file.path().to_str().unwrap(), 840).unwrap();
        assert_eq!((loaded.width(), loaded.height()), (840, 210));
        assert!(cache.0.lock().unwrap().is_some());
    }

    #[test]
    fn a_second_read_of_the_same_file_comes_from_the_cache() {
        let file = sample(64, 32);
        let path = file.path().to_str().unwrap().to_owned();
        let cache = SourceCache::default();
        cache.load(&path, 420).unwrap();
        // Deleting the file leaves the cache as the only way to load it.
        drop(file);
        assert!(cache.load(&path, 420).is_ok());
        assert!(
            cache.load(&path, 840).is_err(),
            "another width reads the file again"
        );
    }
}
