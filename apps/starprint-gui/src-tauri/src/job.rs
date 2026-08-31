//! The job the frontend sends. It differs from the shared crate's
//! [`Job`] in one place: a picture names a file, which only this app
//! knows how to read.

use image::DynamicImage;
use serde::Deserialize;
use starprint_workflows::{Job, Note, Paper, PrinterKind, Qr, TaskCard, TestPage, Text};

use crate::picture::{PicturePath, SourceCache};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum JobRequest {
    TaskCard(TaskCard),
    Text(Text),
    Note(Note),
    Qr(Qr),
    TestPage(TestPage),
    Picture(PicturePath),
}

impl JobRequest {
    /// Reads the picture, if this is one, so the shared crate never has
    /// to. The other kinds print nothing from disk and yield no image.
    pub fn resolve(
        self,
        kind: PrinterKind,
        paper: Paper,
        cache: &SourceCache,
    ) -> Result<(Job, Option<DynamicImage>), String> {
        Ok(match self {
            Self::TaskCard(card) => (Job::TaskCard(card), None),
            Self::Text(text) => (Job::Text(text), None),
            Self::Note(note) => (Job::Note(note), None),
            Self::Qr(code) => (Job::Qr(code), None),
            Self::TestPage(page) => (Job::TestPage(page), None),
            Self::Picture(picture) => {
                let source = picture.source(kind, paper, cache)?;
                (Job::Picture(picture.picture), Some(source))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::tests::{at, sample};

    fn resolve(job: JobRequest) -> Result<(Job, Option<DynamicImage>), String> {
        job.resolve(PrinterKind::Impact, Paper::Mm80, &SourceCache::default())
    }

    #[test]
    fn a_picture_job_resolves_to_its_image() {
        let file = sample(64, 32);
        let (job, image) = resolve(JobRequest::Picture(at(file.path().to_str().unwrap()))).unwrap();
        assert!(job.needs_image());
        assert_eq!(image.expect("read from disk").width(), 64);
    }

    #[test]
    fn every_other_kind_resolves_without_touching_the_disk() {
        let jobs = [
            JobRequest::TaskCard(TaskCard {
                text: "Task".to_owned(),
                ..TaskCard::default()
            }),
            JobRequest::Text(Text {
                text: "Text".to_owned(),
                ..Text::default()
            }),
            JobRequest::Note(Note::default()),
            JobRequest::Qr(Qr {
                data: "https://example.com".to_owned(),
                ..Qr::default()
            }),
            JobRequest::TestPage(TestPage::default()),
        ];
        for job in jobs {
            let (job, image) = resolve(job).expect("resolves");
            assert!(!job.needs_image());
            assert!(image.is_none());
        }
    }

    /// The frontend sends the path inside the picture object, so it has
    /// to survive the tag and the flatten together.
    #[test]
    fn a_picture_job_carries_its_path_beside_the_settings() {
        let json = r#"{"kind":"picture","path":"C:\\photo.jpg","dither":"atkinson"}"#;
        let JobRequest::Picture(picture) = serde_json::from_str(json).unwrap() else {
            panic!("picture")
        };
        assert_eq!(picture.path, r"C:\photo.jpg");
        assert_eq!(
            picture.picture.dither,
            starprint_workflows::picture::Dither::Atkinson
        );
        assert_eq!(picture.picture.threshold, 128, "and the rest default");
    }
}
