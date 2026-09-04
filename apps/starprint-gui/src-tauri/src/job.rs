//! Job requests, document preparation, printing and hex dumps.
//! Unlike the shared [`Job`], a picture request names a file to read.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use starprint::Document;
use starprint::transport::TcpTransport;
use starprint_workflows::{Job, Printer};

use crate::hexdump::{self, HexDump};
use crate::picture::{PicturePath, SourceCache};

#[derive(Debug, Clone, Deserialize)]
pub struct JobRequest {
    #[serde(flatten)]
    job: Job,
    #[serde(default)]
    path: String,
}

impl JobRequest {
    fn document(self, printer: &Printer, cache: &SourceCache) -> Result<Document, String> {
        let image = match self.job {
            Job::Picture(picture) => Some(
                PicturePath {
                    path: self.path,
                    picture,
                }
                .source(printer.head.kind(), printer.head.paper(), cache)?,
            ),
            _ => None,
        };
        printer.document(&self.job, image.as_ref())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintReport {
    pub bytes: usize,
}

#[tauri::command]
pub async fn print_job(
    job: JobRequest,
    printer: Printer,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<PrintReport, String> {
    let cache = Arc::clone(&cache);
    // Image preparation is CPU-bound and the transport sleeps between
    // chunks; neither belongs on the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        let document = job.document(&printer, &cache)?;
        let mut transport = TcpTransport::connect(&printer.address()).map_err(|e| e.to_string())?;
        transport.print(&document).map_err(|e| e.to_string())?;
        Ok(PrintReport {
            bytes: document.as_bytes().len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn job_hexdump(
    job: JobRequest,
    printer: Printer,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<HexDump, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || {
        Ok(hexdump::of(job.document(&printer, &cache)?.as_bytes()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::tests::{at, sample};

    fn printer() -> Printer {
        Printer::impact("printer.invalid".to_owned(), 9100)
    }

    #[test]
    fn a_picture_file_prints_the_same_as_the_shared_job() {
        let file = sample(64, 32);
        let picture = at(file.path().to_str().unwrap());
        let source = image::open(file.path()).unwrap();
        let expected = printer()
            .document(&Job::Picture(picture.picture), Some(&source))
            .unwrap();
        let actual = JobRequest {
            job: Job::Picture(picture.picture),
            path: picture.path,
        }
        .document(&printer(), &SourceCache::default())
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn other_jobs_print_the_same_as_the_shared_job() {
        for json in [
            r#"{"kind":"task-card","text":"Task","priority":true}"#,
            r#"{"kind":"text","text":"Text","bold":true}"#,
            r#"{"kind":"note"}"#,
            r#"{"kind":"qr","data":"https://example.com"}"#,
            r#"{"kind":"test-page"}"#,
        ] {
            let request: JobRequest = serde_json::from_str(json).unwrap();
            let job: Job = serde_json::from_str(json).unwrap();
            let actual = request
                .document(&printer(), &SourceCache::default())
                .unwrap();
            let expected = printer().document(&job, None).unwrap();
            assert_eq!(actual, expected, "{json}");
        }
    }

    /// The frontend sends the path inside the picture object, so it has
    /// to survive the tag and the flatten together.
    #[test]
    fn a_picture_job_carries_its_path_beside_the_settings() {
        let json = r#"{"kind":"picture","path":"C:\\photo.jpg","dither":"atkinson"}"#;
        let request: JobRequest = serde_json::from_str(json).unwrap();
        let Job::Picture(picture) = request.job else {
            panic!("picture")
        };
        assert_eq!(request.path, r"C:\photo.jpg");
        assert_eq!(
            picture.dither,
            starprint_workflows::picture::Dither::Atkinson
        );
        assert_eq!(picture.threshold, 128, "and the rest default");
    }

    #[test]
    fn a_picture_job_needs_a_path() {
        for json in [
            r#"{"kind":"picture"}"#,
            r#"{"kind":"picture","path":""}"#,
            r#"{"kind":"picture","path":"  "}"#,
        ] {
            let request: JobRequest = serde_json::from_str(json).unwrap();
            assert_eq!(
                request
                    .document(&printer(), &SourceCache::default())
                    .unwrap_err(),
                "No picture chosen.",
            );
        }
    }
}
