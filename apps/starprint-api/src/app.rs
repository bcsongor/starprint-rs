//! The routes, and the JSON the client sees. `body` reads the request,
//! `job` turns it into printable bytes, `printers` writes them, `data`
//! keeps the profiles and schedules. This module wires them together
//! and does nothing else.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use starprint_workflows::Preview;

use crate::body;
use crate::config::{Profile, ProfileSpec};
use crate::job::JobRequest;
use crate::printers::{self, Printers};
use crate::problem::Problem;
use crate::schedule::{self, ScheduleSpec};

pub fn router(printers: Arc<Printers>) -> Router {
    let token = Arc::new(printers.data.token().to_owned());
    Router::new()
        .route("/v1/printers", get(list_printers))
        .route(
            "/v1/printers/{name}",
            axum::routing::put(put_printer).delete(delete_printer),
        )
        .route("/v1/printers/{name}/status", get(printer_status))
        .route(
            "/v1/printers/{name}/jobs",
            // Covers the multipart form, which is streamed rather than
            // collected; the JSON branch holds itself to its own limit.
            post(create_job).layer(DefaultBodyLimit::max(body::BINARY_LIMIT)),
        )
        .route(
            "/v1/printers/{name}/preview",
            post(preview_job).layer(DefaultBodyLimit::max(body::BINARY_LIMIT)),
        )
        .route("/v1/printers/{name}/raw", post(create_raw_job))
        .route("/v1/schedules", get(list_schedules).post(create_schedule))
        .route(
            "/v1/schedules/{id}",
            axum::routing::put(replace_schedule).delete(delete_schedule),
        )
        .fallback(|| async { Problem::not_found("No such endpoint.") })
        .method_not_allowed_fallback(|| async {
            Problem::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "Not a method this endpoint takes.",
            )
        })
        .with_state(printers)
        .layer(middleware::from_fn_with_state(token, require_token))
}

/// Every request carries the token as a bearer, or is a `401`.
///
/// This is also what keeps web pages out: a page cannot know the
/// token, and a browser will not send `Authorization` cross-origin
/// without a preflight, which this server never answers.
async fn require_token(
    State(token): State<Arc<String>>,
    request: Request,
    next: Next,
) -> Result<Response, Problem> {
    let given = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if given.is_none_or(|given| given.trim() != token.as_str()) {
        return Err(Problem::unauthorized(
            "Send the server's token as `Authorization: Bearer <token>`.",
        ));
    }
    Ok(next.run(request).await)
}

/// A profile as a client sees it: the name it is addressed by and the
/// settings a `PUT` takes.
#[derive(Debug, Serialize)]
struct PrinterView {
    name: String,
    #[serde(flatten)]
    spec: ProfileSpec,
}

impl From<&Profile> for PrinterView {
    fn from(profile: &Profile) -> Self {
        Self {
            name: profile.name.clone(),
            spec: profile.spec(),
        }
    }
}

/// The profiles, and the version of the server that holds them, so a
/// client can tell an older server from one it expects.
#[derive(Debug, Serialize)]
struct PrinterList {
    version: &'static str,
    printers: Vec<PrinterView>,
}

#[derive(Debug, Serialize)]
struct StatusView {
    /// Something accepted a connection at the printer's address.
    online: bool,
}

#[derive(Debug, Serialize)]
struct ScheduleView {
    id: String,
    #[serde(flatten)]
    spec: ScheduleSpec,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrintReport {
    /// A completed socket write, and nothing more.
    bytes_sent: usize,
}

async fn list_printers(State(printers): State<Arc<Printers>>) -> Json<PrinterList> {
    Json(PrinterList {
        version: env!("CARGO_PKG_VERSION"),
        printers: printers
            .data
            .profiles()
            .iter()
            .map(PrinterView::from)
            .collect(),
    })
}

/// Creates the profile, or replaces the one of that name in place.
async fn put_printer(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<(StatusCode, Json<PrinterView>), Problem> {
    let spec: ProfileSpec = body::json(request).await?;
    let profile = Profile::new(name, spec).map_err(|e| Problem::bad_request(format!("{e}.")))?;
    let view = PrinterView::from(&profile);
    let created = printers
        .data
        .put_profile(profile)
        .map_err(Problem::not_saved)?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(view)))
}

/// Its schedules stay, and do not run until a printer of that name is
/// back.
async fn delete_printer(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
) -> Result<StatusCode, Problem> {
    if !printers
        .data
        .remove_profile(&name)
        .map_err(Problem::not_saved)?
    {
        return Err(Problem::not_found(format!("No printer named `{name}`.")));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn printer_status(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
) -> Result<Json<StatusView>, Problem> {
    let printer = printers.find(&name)?.printer;
    Ok(Json(StatusView {
        online: printers::reachable(&printer.host, printer.port, &printers.queue).await,
    }))
}

/// A job request as JSON, or as a form with the picture beside it.
async fn job_request(request: Request) -> Result<(JobRequest, Option<Bytes>), Problem> {
    match body::media_type(request.headers()).as_deref() {
        Some("application/json") => Ok((body::json(request).await?, None)),
        Some("multipart/form-data") => {
            let (job, image) = body::form(request).await?;
            Ok((JobRequest::parse(&job)?, image))
        }
        other => Err(body::unsupported(
            other,
            "application/json or multipart/form-data",
        )),
    }
}

async fn create_job(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Json<PrintReport>, Problem> {
    let printer = printers.find(&name)?.printer;
    let (job, image) = job_request(request).await?;
    let payload = job.document(&printer, image).await?;
    Ok(Json(PrintReport {
        bytes_sent: printers::send(&printer, payload, &printers.queue).await?,
    }))
}

/// The same request as a job, answered with how it would look instead
/// of sending it.
async fn preview_job(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Json<Preview>, Problem> {
    let printer = printers.find(&name)?.printer;
    let (job, image) = job_request(request).await?;
    Ok(Json(job.preview(&printer, image).await?))
}

async fn create_raw_job(
    State(printers): State<Arc<Printers>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Json<PrintReport>, Problem> {
    let printer = printers.find(&name)?.printer;
    match body::media_type(request.headers()).as_deref() {
        Some("application/octet-stream") => {}
        other => return Err(body::unsupported(other, "application/octet-stream")),
    }
    // The bytes already contain every printer command, so the profile's
    // cut, density and speed have nothing to add. Hex and other text
    // encodings are not supported.
    let payload = body::collect(request, body::BINARY_LIMIT).await?;
    Ok(Json(PrintReport {
        bytes_sent: printers::send(&printer, payload, &printers.queue).await?,
    }))
}

async fn list_schedules(State(printers): State<Arc<Printers>>) -> Json<Vec<ScheduleView>> {
    Json(
        printers
            .data
            .schedules()
            .into_iter()
            .map(|(id, spec)| ScheduleView { id, spec })
            .collect(),
    )
}

async fn create_schedule(
    State(printers): State<Arc<Printers>>,
    request: Request,
) -> Result<(StatusCode, Json<ScheduleView>), Problem> {
    let spec: ScheduleSpec = body::json(request).await?;
    schedule::admit(&spec, &printers.data).await?;
    let id = printers
        .data
        .add_schedule(spec.clone())
        .map_err(Problem::not_saved)?;
    Ok((StatusCode::CREATED, Json(ScheduleView { id, spec })))
}

async fn replace_schedule(
    State(printers): State<Arc<Printers>>,
    Path(id): Path<String>,
    request: Request,
) -> Result<Json<ScheduleView>, Problem> {
    let spec: ScheduleSpec = body::json(request).await?;
    schedule::admit(&spec, &printers.data).await?;
    if !printers
        .data
        .put_schedule(&id, spec.clone())
        .map_err(Problem::not_saved)?
    {
        return Err(Problem::not_found(format!("No schedule with id `{id}`.")));
    }
    Ok(Json(ScheduleView { id, spec }))
}

async fn delete_schedule(
    State(printers): State<Arc<Printers>>,
    Path(id): Path<String>,
) -> Result<StatusCode, Problem> {
    if !printers
        .data
        .remove_schedule(&id)
        .map_err(Problem::not_saved)?
    {
        return Err(Problem::not_found(format!("No schedule with id `{id}`.")));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Data;
    use axum::body::Body;
    use axum::http::{Method, header};
    use serde_json::{Value, json};
    use starprint_workflows::{Paper, Printer, Speed};
    use tokio::io::AsyncReadExt as _;
    use tokio::net::TcpListener;
    use tower::ServiceExt as _;

    const BOUNDARY: &str = "starprintboundary";
    const TOKEN: &str = "test-token";

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
        let data = Data::ephemeral(
            vec![
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
            ],
            TOKEN.to_owned(),
        );
        Arc::new(Printers::new(Arc::new(data), Arc::default()))
    }

    struct Reply {
        status: StatusCode,
        content_type: String,
        json: Value,
    }

    async fn call(printers: Arc<Printers>, mut request: Request) -> Reply {
        request
            .headers_mut()
            .entry(header::AUTHORIZATION)
            .or_insert_with(|| format!("Bearer {TOKEN}").parse().unwrap());
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

    fn get(path: &str) -> Request {
        Request::builder().uri(path).body(Body::empty()).unwrap()
    }

    fn delete(path: &str) -> Request {
        Request::builder()
            .method(Method::DELETE)
            .uri(path)
            .body(Body::empty())
            .unwrap()
    }

    fn send(method: Method, path: &str, content_type: &str, body: impl Into<Body>) -> Request {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, content_type)
            .body(body.into())
            .unwrap()
    }

    fn post(path: &str, content_type: &str, body: impl Into<Body>) -> Request {
        send(Method::POST, path, content_type, body)
    }

    fn post_json(path: &str, body: Value) -> Request {
        post(path, "application/json", body.to_string())
    }

    fn put_json(path: &str, body: Value) -> Request {
        send(Method::PUT, path, "application/json", body.to_string())
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

    fn schedule(printer: &str) -> Value {
        json!({
            "printer": printer,
            "cron": "0 9 * * 1-5",
            "due": "run-day",
            "job": { "kind": "task-card", "text": "Standup" },
        })
    }

    #[tokio::test]
    async fn printers_are_listed_in_file_order_with_the_server_version() {
        let reply = call(printers(9100), get("/v1/printers")).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            reply.json,
            json!({
                "version": env!("CARGO_PKG_VERSION"),
                "printers": [
                    { "name": "tsp800ii", "host": "127.0.0.1", "port": 9100, "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
                    { "name": "sp743", "host": "127.0.0.1", "port": 9100, "kind": "impact", "cut": true },
                ],
            })
        );
    }

    #[tokio::test]
    async fn a_profile_is_created_replaced_and_deleted_by_name() {
        let printers = printers(9100);
        let spec = json!({ "host": "10.0.0.7", "port": 9100, "kind": "impact", "cut": false });
        let reply = call(
            Arc::clone(&printers),
            put_json("/v1/printers/desk", spec.clone()),
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.json);
        assert_eq!(reply.json["name"], "desk");
        assert_eq!(reply.json["host"], "10.0.0.7");

        let replaced = json!({ "host": "10.0.0.8", "port": 9100, "kind": "impact", "cut": true });
        let reply = call(
            Arc::clone(&printers),
            put_json("/v1/printers/desk", replaced),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);

        let reply = call(Arc::clone(&printers), get("/v1/printers")).await;
        let names: Vec<&str> = reply.json["printers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["tsp800ii", "sp743", "desk"], "a new one goes last");
        assert_eq!(reply.json["printers"][2]["host"], "10.0.0.8");

        let reply = call(Arc::clone(&printers), delete("/v1/printers/desk")).await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        let reply = call(Arc::clone(&printers), delete("/v1/printers/desk")).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert_eq!(printers.data.profiles().len(), 2);
    }

    /// The file's checks answer the request, worded the same.
    #[tokio::test]
    async fn a_profile_that_would_not_pass_the_file_is_a_bad_request() {
        let printers = printers(9100);
        let cases = [
            (
                json!({ "host": "h", "port": 9100, "kind": "thermal", "cut": true }),
                "p: `paper` is required for a thermal printer.",
            ),
            (
                json!({ "host": "h", "port": 9100, "kind": "impact", "cut": true, "density": 3 }),
                "p: `density` is not a setting on an impact printer.",
            ),
            (
                json!({ "host": " ", "port": 9100, "kind": "impact", "cut": true }),
                "p: `host` is empty.",
            ),
        ];
        for (spec, detail) in cases {
            let reply = call(Arc::clone(&printers), put_json("/v1/printers/p", spec)).await;
            assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{detail}");
            assert_eq!(reply.json["detail"], detail);
        }
        let unknown =
            json!({ "host": "h", "port": 9100, "kind": "impact", "cut": true, "colour": 1 });
        let reply = call(Arc::clone(&printers), put_json("/v1/printers/p", unknown)).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        let blank = json!({ "host": "h", "port": 9100, "kind": "impact", "cut": true });
        let reply = call(Arc::clone(&printers), put_json("/v1/printers/%20", blank)).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(printers.data.profiles().len(), 2, "nothing was kept");
    }

    #[tokio::test]
    async fn a_printer_that_answers_is_online_and_one_that_does_not_is_not() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let reply = call(printers(port), get("/v1/printers/sp743/status")).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.json, json!({ "online": true }));

        let reply = call(
            printers(closed_port().await),
            get("/v1/printers/sp743/status"),
        )
        .await;
        assert_eq!(reply.json, json!({ "online": false }));

        let reply = call(printers(port), get("/v1/printers/nope/status")).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_schedule_is_created_listed_replaced_and_deleted() {
        let printers = printers(9100);
        let reply = call(
            Arc::clone(&printers),
            post_json("/v1/schedules", schedule("sp743")),
        )
        .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.json);
        let id = reply.json["id"].as_str().unwrap().to_owned();
        assert_eq!(reply.json["enabled"], true);
        assert_eq!(reply.json["due"], "run-day");
        assert_eq!(reply.json["job"]["kind"], "task-card");
        assert!(reply.json.get("density").is_none());

        let reply = call(Arc::clone(&printers), get("/v1/schedules")).await;
        assert_eq!(reply.json.as_array().unwrap().len(), 1);
        assert_eq!(reply.json[0]["id"], id);

        let mut changed = schedule("tsp800ii");
        changed["enabled"] = json!(false);
        changed["density"] = json!(4);
        let reply = call(
            Arc::clone(&printers),
            put_json(&format!("/v1/schedules/{id}"), changed.clone()),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
        assert_eq!(reply.json["printer"], "tsp800ii");
        assert_eq!(reply.json["enabled"], false);
        assert_eq!(reply.json["density"], 4);

        let reply = call(
            Arc::clone(&printers),
            put_json("/v1/schedules/nope", changed),
        )
        .await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);

        let reply = call(
            Arc::clone(&printers),
            delete(&format!("/v1/schedules/{id}")),
        )
        .await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        let reply = call(Arc::clone(&printers), delete("/v1/schedules/nope")).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert!(printers.data.schedules().is_empty());
    }

    #[tokio::test]
    async fn a_schedule_that_cannot_run_is_refused() {
        let printers = printers(9100);
        let mut bad_cron = schedule("sp743");
        bad_cron["cron"] = json!("0 9 * *");
        let mut picture = schedule("sp743");
        picture["job"] = json!({ "kind": "picture" });
        let mut with_id = schedule("sp743");
        with_id["id"] = json!("mine");
        let mut density = schedule("sp743");
        density["density"] = json!(3);
        let mut blank = schedule("sp743");
        blank["job"] = json!({ "kind": "task-card", "text": "  " });
        for (what, body) in [
            ("four fields", bad_cron),
            ("a picture", picture),
            ("an unknown printer", schedule("nope")),
            ("an id of its own", with_id),
            ("density on an impact printer", density),
            ("a card with no text", blank),
        ] {
            let reply = call(Arc::clone(&printers), post_json("/v1/schedules", body)).await;
            assert_eq!(
                reply.status,
                StatusCode::BAD_REQUEST,
                "{what}: {:?}",
                reply.json
            );
        }
        assert!(printers.data.schedules().is_empty());
    }

    #[tokio::test]
    async fn a_json_job_is_written_to_the_printer() {
        let (port, received) = fake_printer().await;
        let reply = call(
            printers(port),
            post_json(
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
            post_json(
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

    /// A preview takes the same body as a job and touches no printer,
    /// so it answers for a printer that is off.
    #[tokio::test]
    async fn a_preview_shows_the_job_without_printing_it() {
        let port = closed_port().await;
        let reply = call(
            printers(port),
            post_json(
                "/v1/printers/tsp800ii/preview",
                json!({ "job": { "kind": "task-card", "text": "Standup" }, "density": 4 }),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
        assert_eq!(reply.json["kind"], "task-card");
        assert_eq!(reply.json["lines"][0], "Standup");

        let reply = call(
            printers(port),
            multipart(
                "/v1/printers/sp743/preview",
                form(json!({ "job": { "kind": "picture" } }), Some(&png())),
            ),
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
        assert_eq!(reply.json["kind"], "picture");
        assert!(
            reply.json["image"]
                .as_str()
                .is_some_and(|url| url.starts_with("data:image/png;base64,")),
            "{:?}",
            reply.json
        );

        let reply = call(
            printers(port),
            post_json(
                "/v1/printers/sp743/preview",
                json!({ "job": { "kind": "picture" } }),
            ),
        )
        .await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "a picture needs its image"
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
    /// live, in `body`, `job`, `config` and `schedule`.
    #[tokio::test]
    async fn errors_are_problem_details() {
        let cases: Vec<(StatusCode, &str, Request)> = vec![
            (
                StatusCode::NOT_FOUND,
                "an unknown printer",
                post_json(
                    "/v1/printers/nope/jobs",
                    json!({ "job": { "kind": "test-page" } }),
                ),
            ),
            (
                StatusCode::NOT_FOUND,
                "an unknown endpoint",
                post_json("/v2/printers", json!({})),
            ),
            (
                StatusCode::METHOD_NOT_ALLOWED,
                "the wrong method",
                post_json("/v1/printers", json!({})),
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
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "a profile that is not JSON",
                send(Method::PUT, "/v1/printers/p", "text/plain", "host = h"),
            ),
            (
                StatusCode::BAD_REQUEST,
                "malformed JSON",
                post("/v1/printers/tsp800ii/jobs", "application/json", "{"),
            ),
            (
                StatusCode::BAD_REQUEST,
                "a malformed schedule",
                post("/v1/schedules", "application/json", "{"),
            ),
            (
                StatusCode::BAD_REQUEST,
                "a picture sent as plain JSON",
                post_json(
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

    /// Wrong, malformed or missing, the answer is the same `401`, and
    /// no printer is looked up first. Bypasses `call`, which fills the
    /// header in.
    #[tokio::test]
    async fn a_request_without_the_token_is_unauthorized() {
        for given in [
            None,
            Some("Bearer wrong"),
            Some(TOKEN),
            Some("Basic dGVzdA=="),
        ] {
            let mut request = Request::builder().uri("/v1/printers");
            if let Some(given) = given {
                request = request.header(header::AUTHORIZATION, given);
            }
            let response = router(printers(9100))
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{given:?}");
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json",
                "{given:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_printer_that_cannot_be_reached_is_a_bad_gateway() {
        let reply = call(
            printers(closed_port().await),
            post_json(
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
}
