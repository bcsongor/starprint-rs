//! The routes, and the JSON the client sees. `body` reads the request,
//! `job` turns it into printable bytes, `printers` writes them. This
//! module wires the three together and does nothing else.

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use starprint_workflows::{Head, PrinterKind, Speed};

use crate::body;
use crate::config::Profile;
use crate::job::JobRequest;
use crate::printers::Printers;
use crate::problem::Problem;

pub fn router(printers: Arc<Printers>) -> Router {
    Router::new()
        .route("/v1/printers", get(list_printers))
        .route(
            "/v1/printers/{name}/jobs",
            // Covers the multipart form, which is streamed rather than
            // collected; the JSON branch holds itself to its own limit.
            post(create_job).layer(DefaultBodyLimit::max(body::BINARY_LIMIT)),
        )
        .route("/v1/printers/{name}/raw", post(create_raw_job))
        .fallback(|| async { Problem::not_found("No such endpoint.") })
        .method_not_allowed_fallback(|| async {
            Problem::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "Not a method this endpoint takes.",
            )
        })
        .with_state(printers)
}

/// A read-only view of the profile file, so a client can pick a printer
/// and see which options apply to it. The address is deliberately not
/// here: a client names a printer, never a host.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrinterView {
    name: String,
    kind: PrinterKind,
    /// Roll width in millimetres. Thermal only.
    #[serde(skip_serializing_if = "Option::is_none")]
    paper: Option<u16>,
    cut: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    density: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<Speed>,
}

impl From<&Profile> for PrinterView {
    fn from(profile: &Profile) -> Self {
        let head = profile.printer.head;
        let (paper, density, speed) = match head {
            Head::Thermal {
                paper,
                density,
                speed,
            } => (Some(paper.mm()), Some(density), Some(speed)),
            Head::Impact => (None, None, None),
        };
        Self {
            name: profile.name.clone(),
            kind: head.kind(),
            paper,
            cut: profile.printer.cut,
            density,
            speed,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrintReport {
    /// A completed socket write, and nothing more.
    bytes_sent: usize,
}

async fn list_printers(State(printers): State<Arc<Printers>>) -> Json<Vec<PrinterView>> {
    Json(printers.profiles().map(PrinterView::from).collect())
}

async fn create_job(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Json<PrintReport>, Problem> {
    let printer = printers.find(&name)?;

    let (job, image) = match body::media_type(request.headers()).as_deref() {
        Some("application/json") => (body::collect(request, body::JSON_LIMIT).await?, None),
        Some("multipart/form-data") => body::form(request).await?,
        other => {
            return Err(body::unsupported(
                other,
                "application/json or multipart/form-data",
            ));
        }
    };

    let payload = JobRequest::parse(&job)?
        .document(&printer.profile().printer, image)
        .await?;
    Ok(Json(PrintReport {
        bytes_sent: printer.send(payload).await?,
    }))
}

async fn create_raw_job(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Json<PrintReport>, Problem> {
    let printer = printers.find(&name)?;
    match body::media_type(request.headers()).as_deref() {
        Some("application/octet-stream") => {}
        other => return Err(body::unsupported(other, "application/octet-stream")),
    }
    // The bytes already contain every printer command, so the profile's
    // cut, density and speed have nothing to add. Hex and other text
    // encodings are not supported.
    let payload = body::collect(request, body::BINARY_LIMIT).await?;
    Ok(Json(PrintReport {
        bytes_sent: printer.send(payload).await?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, header};
    use serde_json::{Value, json};
    use starprint_workflows::{Paper, Printer};
    use tokio::io::AsyncReadExt as _;
    use tokio::net::TcpListener;
    use tower::ServiceExt as _;

    const BOUNDARY: &str = "starprintboundary";

    /// Accepts one job and hands back the bytes it was sent.
    async fn fake_printer() -> (u16, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a free port");
        let port = listener.local_addr().unwrap().port();
        let received = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        (port, received)
    }

    /// A port nothing is listening on.
    async fn closed_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn printers(port: u16) -> Arc<Printers> {
        Arc::new(Printers::new(vec![
            Profile {
                name: "tsp800ii".to_owned(),
                printer: Printer::thermal(
                    "127.0.0.1".to_owned(),
                    port,
                    Paper::Mm80,
                    3,
                    Speed::Slow,
                ),
            },
            Profile {
                name: "sp743".to_owned(),
                printer: Printer::impact("127.0.0.1".to_owned(), port),
            },
        ]))
    }

    struct Reply {
        status: StatusCode,
        content_type: String,
        json: Value,
    }

    async fn call(printers: Arc<Printers>, request: Request) -> Reply {
        let response = router(printers).oneshot(request).await.expect("infallible");
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        Reply {
            status,
            content_type,
            json: serde_json::from_slice(&body).unwrap_or(Value::Null),
        }
    }

    fn post(path: &str, content_type: &str, body: impl Into<Body>) -> Request {
        Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, content_type)
            .body(body.into())
            .unwrap()
    }

    fn json_job(path: &str, body: Value) -> Request {
        post(path, "application/json", body.to_string())
    }

    fn multipart(path: &str, body: impl Into<Body>) -> Request {
        post(
            path,
            &format!("multipart/form-data; boundary={BOUNDARY}"),
            body,
        )
    }

    /// One `job` part, and an `image` part when there are bytes for one.
    fn form(job: Value, image: Option<&[u8]>) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"job\"\r\n\r\n{job}\r\n"
            )
            .into_bytes(),
        );
        if let Some(image) = image {
            body.extend(
                format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"image\"; \
                     filename=\"p.png\"\r\n\r\n"
                )
                .into_bytes(),
            );
            body.extend(image);
            body.extend(b"\r\n");
        }
        body.extend(format!("--{BOUNDARY}--\r\n").into_bytes());
        body
    }

    fn png() -> Vec<u8> {
        let mut image = image::GrayImage::new(64, 32);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Luma([((x * 4) ^ (y * 8)) as u8]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[tokio::test]
    async fn printers_are_listed_in_file_order_without_their_address() {
        let reply = call(
            printers(9100),
            Request::builder()
                .uri("/v1/printers")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            reply.json,
            json!([
                { "name": "tsp800ii", "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
                { "name": "sp743", "kind": "impact", "cut": true },
            ])
        );
    }

    #[tokio::test]
    async fn a_json_job_is_written_to_the_printer() {
        let (port, received) = fake_printer().await;
        let reply = call(
            printers(port),
            json_job(
                "/v1/printers/tsp800ii/jobs",
                json!({
                    "job": { "kind": "task-card", "text": "Renew passport", "priority": true, "due": "2026-09-15" },
                }),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);

        let bytes = received.await.unwrap();
        assert_eq!(reply.json["bytesSent"], bytes.len());
        assert!(
            bytes.windows(14).any(|w| w == b"Renew passport"),
            "the card's text reached the printer"
        );
        assert!(bytes.ends_with(&[0x1b, b'd', 3]), "and the profile's cut");
    }

    /// The impact printers have no QR command, so the symbol is encoded
    /// here and sent as a bit image like any other picture.
    #[tokio::test]
    async fn a_qr_job_prints_on_the_impact_printer_too() {
        let (port, received) = fake_printer().await;
        let reply = call(
            printers(port),
            json_job(
                "/v1/printers/sp743/jobs",
                json!({ "job": { "kind": "qr", "data": "https://example.com/r/42", "caption": "Order 42" } }),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);

        let bytes = received.await.unwrap();
        assert!(bytes.windows(8).any(|w| w == b"Order 42"), "the caption");
        assert!(
            bytes.windows(3).any(|w| w == [0x1b, b'^', 1]),
            "and the symbol as a double-density bit image"
        );
    }

    #[tokio::test]
    async fn raw_bytes_go_through_untouched() {
        let (port, received) = fake_printer().await;
        let payload = b"\x1b@Hello\n\x1bd\x03".to_vec();
        let reply = call(
            printers(port),
            post(
                "/v1/printers/sp743/raw",
                "application/octet-stream",
                payload.clone(),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
        assert_eq!(reply.json["bytesSent"], payload.len());
        assert_eq!(received.await.unwrap(), payload);
    }

    #[tokio::test]
    async fn a_picture_is_sent_as_a_form() {
        let (port, received) = fake_printer().await;
        let reply = call(
            printers(port),
            multipart(
                "/v1/printers/sp743/jobs",
                form(
                    json!({ "job": { "kind": "picture", "double": true } }),
                    Some(&png()),
                ),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
        let bytes = received.await.unwrap();
        assert!(
            bytes.windows(3).any(|w| w == [0x1b, b'^', 1]),
            "a double-density bit image"
        );
    }

    #[tokio::test]
    async fn any_job_kind_may_be_sent_as_a_form() {
        let (port, received) = fake_printer().await;
        let reply = call(
            printers(port),
            multipart(
                "/v1/printers/sp743/jobs",
                form(json!({ "job": { "kind": "text", "text": "Hi" } }), None),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
        assert!(received.await.unwrap().windows(2).any(|w| w == b"Hi"));
    }

    /// One case per status the API can return, to prove the shape and
    /// the mapping. The rules behind each `400` are tested where they
    /// live, in `body` and `job`.
    #[tokio::test]
    async fn errors_are_problem_details() {
        let cases: Vec<(StatusCode, &str, Request)> = vec![
            (
                StatusCode::NOT_FOUND,
                "an unknown printer",
                json_job(
                    "/v1/printers/nope/jobs",
                    json!({ "job": { "kind": "test-page" } }),
                ),
            ),
            (
                StatusCode::NOT_FOUND,
                "an unknown endpoint",
                json_job("/v2/printers", json!({})),
            ),
            (
                StatusCode::METHOD_NOT_ALLOWED,
                "the wrong method",
                json_job("/v1/printers", json!({})),
            ),
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "a body that is neither JSON nor a form",
                post("/v1/printers/tsp800ii/jobs", "text/plain", "print this"),
            ),
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "raw bytes called something else",
                post("/v1/printers/tsp800ii/raw", "text/plain", "1b40"),
            ),
            (
                StatusCode::BAD_REQUEST,
                "malformed JSON",
                post("/v1/printers/tsp800ii/jobs", "application/json", "{"),
            ),
            (
                StatusCode::BAD_REQUEST,
                "a picture sent as plain JSON",
                json_job(
                    "/v1/printers/tsp800ii/jobs",
                    json!({ "job": { "kind": "picture" } }),
                ),
            ),
            (
                StatusCode::BAD_REQUEST,
                "a form with no job part",
                multipart("/v1/printers/sp743/jobs", format!("--{BOUNDARY}--\r\n")),
            ),
            (
                StatusCode::BAD_REQUEST,
                "a form part the endpoint does not take",
                multipart(
                    "/v1/printers/sp743/jobs",
                    format!(
                        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"printer\"\r\n\r\n\
                         sp743\r\n--{BOUNDARY}--\r\n"
                    ),
                ),
            ),
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                "a job description larger than a job description",
                post(
                    "/v1/printers/tsp800ii/jobs",
                    "application/json",
                    vec![b' '; body::JSON_LIMIT + 1],
                ),
            ),
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                "raw bytes past the limit",
                post(
                    "/v1/printers/tsp800ii/raw",
                    "application/octet-stream",
                    vec![0u8; body::BINARY_LIMIT + 1],
                ),
            ),
        ];

        for (status, what, request) in cases {
            let reply = call(printers(9100), request).await;
            assert_eq!(reply.status, status, "{what}: {:?}", reply.json);
            assert_eq!(reply.content_type, "application/problem+json", "{what}");
            assert_eq!(reply.json["status"], status.as_u16(), "{what}");
            assert!(
                reply.json["detail"].as_str().is_some_and(|d| !d.is_empty()),
                "{what} says what went wrong: {:?}",
                reply.json
            );
        }
    }

    #[tokio::test]
    async fn a_printer_that_cannot_be_reached_is_a_bad_gateway() {
        let reply = call(
            printers(closed_port().await),
            json_job(
                "/v1/printers/tsp800ii/jobs",
                json!({ "job": { "kind": "text", "text": "Hi" } }),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_GATEWAY);
        assert_eq!(reply.content_type, "application/problem+json");
        assert!(
            reply.json["detail"]
                .as_str()
                .unwrap()
                .contains("none, some or all"),
            "the client is told not to assume nothing printed: {:?}",
            reply.json
        );
    }

    /// Overlapping requests each write a whole job.
    #[tokio::test(flavor = "multi_thread")]
    async fn overlapping_requests_each_write_a_whole_job() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        // Reads each job to its end before accepting the next, so an
        // overlapping write would show up as an interleaved read.
        let jobs = tokio::spawn(async move {
            let mut lengths = Vec::new();
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                socket.read_to_end(&mut bytes).await.unwrap();
                lengths.push(bytes.len());
            }
            lengths
        });

        let printers = printers(port);
        let job = |text: &str| {
            json_job(
                "/v1/printers/tsp800ii/jobs",
                json!({ "job": { "kind": "text", "text": text } }),
            )
        };
        let (first, second) = tokio::join!(
            call(Arc::clone(&printers), job("first")),
            call(Arc::clone(&printers), job("second")),
        );
        assert_eq!(first.status, StatusCode::OK, "{:?}", first.json);
        assert_eq!(second.status, StatusCode::OK, "{:?}", second.json);
        assert_eq!(jobs.await.unwrap().len(), 2, "two whole jobs arrived");
    }
}
