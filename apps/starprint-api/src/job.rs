//! A job request, and the bytes it prints. This is where a request stops
//! being HTTP and becomes a `starprint-workflows` job.

use axum::body::Bytes;
use axum::http::StatusCode;
use serde::Deserialize;
use starprint_workflows::{Head, Job, Printer, Speed, check_density};

use crate::problem::Problem;

/// Everything about a job but the image. Omitted options come from the
/// profile. A schedule builds one of these each time it runs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobRequest {
    pub(crate) job: Job,
    pub(crate) cut: Option<bool>,
    pub(crate) density: Option<i8>,
    pub(crate) speed: Option<Speed>,
}

impl JobRequest {
    pub fn parse(body: &[u8]) -> Result<Self, Problem> {
        crate::body::parse_json(body)
    }

    /// The bytes this job prints on `profile`, with `image` for the one
    /// kind that needs it.
    ///
    /// Preparing a picture is CPU-bound, so it runs off the async
    /// runtime. The caller does this before taking the printer's turn,
    /// so a slow photo does not hold the printer up.
    pub async fn document(self, profile: &Printer, image: Option<Bytes>) -> Result<Bytes, Problem> {
        if self.job.needs_image() && image.is_none() {
            return Err(Problem::bad_request(
                "A picture job needs an `image` part, so it must be sent as multipart/form-data.",
            ));
        }
        let printer = self.printer(profile)?;
        let job = self.job;
        tokio::task::spawn_blocking(move || {
            let image = image
                .map(|bytes| starprint_workflows::picture::decode(&bytes))
                .transpose()?;
            printer
                .document(&job, image.as_ref())
                .map(|document| Bytes::from(document.into_bytes()))
        })
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("The job could not be prepared: {e}."),
            )
        })?
        .map_err(Problem::bad_request)
    }

    /// The profile with this request's overrides applied. `host`,
    /// `port`, `kind` and `paper` are the profile's alone.
    fn printer(&self, profile: &Printer) -> Result<Printer, Problem> {
        let head = match profile.head {
            Head::Thermal {
                paper,
                density,
                speed,
            } => {
                let density = self.density.unwrap_or(density);
                check_density(density).map_err(|e| Problem::bad_request(format!("{e}.")))?;
                Head::Thermal {
                    paper,
                    density,
                    speed: self.speed.unwrap_or(speed),
                }
            }
            // An SP700 has neither setting, so naming one is a mistake
            // rather than something to quietly drop.
            Head::Impact => {
                for (field, given) in [
                    ("density", self.density.is_some()),
                    ("speed", self.speed.is_some()),
                ] {
                    if given {
                        return Err(Problem::bad_request(format!(
                            "`{field}` is not a setting on an impact printer."
                        )));
                    }
                }
                Head::Impact
            }
        };
        Ok(Printer {
            head,
            cut: self.cut.unwrap_or(profile.cut),
            ..profile.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starprint_workflows::Paper;

    fn thermal() -> Printer {
        Printer::thermal(
            "printer.invalid".to_owned(),
            9100,
            Paper::Mm80,
            3,
            Speed::Slow,
        )
    }

    fn impact() -> Printer {
        Printer::impact("printer.invalid".to_owned(), 9100)
    }

    fn request(json: &str) -> JobRequest {
        JobRequest::parse(json.as_bytes()).expect("parses")
    }

    fn detail(problem: Problem) -> String {
        serde_json::to_value(&problem).unwrap()["detail"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn omitted_options_come_from_the_profile() {
        let printer = request(r#"{"job":{"kind":"test-page"}}"#)
            .printer(&thermal())
            .unwrap();
        assert_eq!(printer.head, thermal().head);
        assert!(printer.cut);
    }

    #[test]
    fn given_options_replace_the_profile_defaults() {
        let printer = request(
            r#"{"cut":false,"density":-1,"speed":"high","job":{"kind":"text","text":"Hi"}}"#,
        )
        .printer(&thermal())
        .unwrap();
        assert_eq!(
            printer.head,
            Head::Thermal {
                paper: Paper::Mm80,
                density: -1,
                speed: Speed::High
            },
            "the paper is still the profile's"
        );
        assert!(!printer.cut);
    }

    /// The profile alone decides where the job goes and how wide it is.
    #[test]
    fn an_override_cannot_reach_the_address_or_the_paper() {
        assert!(JobRequest::parse(br#"{"host":"10.0.0.1","job":{"kind":"test-page"}}"#).is_err());
        assert!(JobRequest::parse(br#"{"paper":112,"job":{"kind":"test-page"}}"#).is_err());
        assert!(JobRequest::parse(br#"{"kind":"impact","job":{"kind":"test-page"}}"#).is_err());
    }

    #[test]
    fn an_impact_printer_has_no_density_or_speed_to_set() {
        for field in ["\"density\":1", "\"speed\":\"slow\""] {
            let request = request(&format!("{{{field},\"job\":{{\"kind\":\"test-page\"}}}}"));
            assert!(
                detail(request.printer(&impact()).unwrap_err())
                    .ends_with("is not a setting on an impact printer.")
            );
        }
        // The same request is fine on a thermal printer.
        assert!(
            request(r#"{"density":1,"job":{"kind":"test-page"}}"#)
                .printer(&thermal())
                .is_ok()
        );
    }

    #[test]
    fn a_density_off_the_scale_is_refused() {
        let request = request(r#"{"density":7,"job":{"kind":"test-page"}}"#);
        assert_eq!(
            detail(request.printer(&thermal()).unwrap_err()),
            "`density` is 7, outside -3 to 4."
        );
    }

    #[tokio::test]
    async fn a_picture_without_an_image_is_refused_before_any_work() {
        let request = request(r#"{"job":{"kind":"picture"}}"#);
        assert!(
            detail(request.document(&thermal(), None).await.unwrap_err())
                .contains("multipart/form-data")
        );
    }

    #[tokio::test]
    async fn an_image_that_will_not_decode_is_a_bad_request() {
        let request = request(r#"{"job":{"kind":"picture"}}"#);
        let problem = request
            .document(&thermal(), Some(Bytes::from_static(b"not an image")))
            .await
            .unwrap_err();
        assert!(detail(problem).starts_with("The image could not be decoded"));
    }

    #[tokio::test]
    async fn a_job_that_cannot_be_built_is_a_bad_request() {
        let request = request(r#"{"job":{"kind":"task-card","text":"   "}}"#);
        assert_eq!(
            detail(request.document(&thermal(), None).await.unwrap_err()),
            "The task text is empty."
        );
    }

    #[tokio::test]
    async fn a_built_job_carries_the_overrides_into_its_bytes() {
        let request = request(r#"{"density":1,"job":{"kind":"text","text":"Hi"}}"#);
        let bytes = request.document(&thermal(), None).await.unwrap();
        // `ESC RS d n` counts the other way: n = 3 - level.
        assert!(bytes.windows(4).any(|w| w == [0x1b, 0x1e, b'd', 2]));
        assert!(bytes.windows(2).any(|w| w == b"Hi"));
    }
}
