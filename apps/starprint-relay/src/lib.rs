//! The relay: a public server that hands out line records and holds
//! sealed faxes until their recipient polls for them, since printers sit
//! behind home routers and cannot be dialled directly. Anyone can run
//! one, because nothing it serves has to be trusted: records check
//! themselves and faxes are sealed. A relay has a name, such as
//! `LONRELAY01`, which a server learns when a relay is added to it.
//!
//! Lines and waiting faxes are kept in one SQLite file (see `store`), so
//! a restart loses nothing. A fax is deleted once its line confirms it,
//! or after the hold, and every fax taken and delivered is logged: who
//! to whom and how big, never what it says. Each client address
//! may leave only so many faxes and new lines (see `limit`).
//!
//! What a relay and a server say to each other is `starprint-fax`'s;
//! this is only the keeping.

mod limit;
mod store;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde::de::DeserializeOwned;
use starprint_fax::{
    Action, Collect, Fax, Held, HeldFaxes, Line, LineRecord, Number, RelayInfo, now,
};

use crate::limit::Limit;
use crate::store::Store;

/// Records and requests are text; anything larger is a client gone wrong.
const JSON_LIMIT: usize = 1 << 20;
/// A sealed fax with a picture in it, encoded twice over.
const FAX_LIMIT: usize = 32 << 20;
/// What a relay holds for one line before it turns faxes away.
const HELD_FAXES: usize = 100;
const HELD_BYTES: usize = 64 << 20;
/// What a relay holds for every line together, so that new lines, each
/// with an allowance of its own, cannot fill the disk. The file is about
/// a third larger, since faxes are kept as base64.
const STORED_BYTES: u64 = 8 << 30;
/// Faxes a client address may leave: a burst, then so many an hour.
const FAXES: (u32, u32) = (10, 30);
/// Lines a client address may publish: each is a new identity to fax
/// from, so they are held to fewer.
const NEW_LINES: (u32, u32) = (5, 10);
/// Lines a relay holds in all. Every one is kept in memory and none is
/// ever dropped, so this is what stops new identities filling both.
const LINES: usize = 10_000;
/// A fax nobody polls for is deleted after thirty days.
const HOLD_SECONDS: u64 = 30 * 24 * 60 * 60;
/// How far a poll's clock may be from the relay's.
const CLOCK_SKEW: u64 = 5 * 60;
struct Relay {
    name: String,
    clients: Clients,
    /// Every line in the store, with its keys decoded.
    lines: RwLock<HashMap<Number, (LineRecord, Line)>>,
    store: Store,
    faxes: Limit,
    new_lines: Limit,
}

/// Where a request's client address comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clients {
    /// The connection's own address.
    Direct,
    /// The last address in `X-Forwarded-For`, the one the TLS proxy in
    /// front of the relay added. Trusting it without a proxy would let
    /// anyone claim any address.
    BehindProxy,
}

impl Relay {
    /// Who sent `request`, if it can be told: a router called without
    /// connection details, as in tests, limits no one.
    fn client(&self, request: &Request) -> Option<IpAddr> {
        let forwarded = || {
            let header = request
                .headers()
                .get_all("x-forwarded-for")
                .iter()
                .next_back()?;
            header
                .to_str()
                .ok()?
                .rsplit(',')
                .next()?
                .trim()
                .parse()
                .ok()
        };
        let direct = || {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0.ip())
        };
        match self.clients {
            Clients::BehindProxy => forwarded().or_else(direct),
            Clients::Direct => direct(),
        }
    }
}

/// A token from `limit` for `client`, or the `429` saying when to try
/// again.
fn take(limit: &Limit, client: Option<IpAddr>, what: &str) -> Result<(), Problem> {
    let Some(client) = client else {
        return Ok(());
    };
    limit
        .take(client, Instant::now())
        .map_err(|wait: Duration| {
            let seconds = wait.as_secs() + 1;
            Problem::new(
                StatusCode::TOO_MANY_REQUESTS,
                format!("Too many {what} from this address; try again in {seconds} seconds."),
            )
            .retry_after(seconds)
        })
}

impl Relay {
    fn line(&self, number: Number) -> Result<Line, Problem> {
        self.lines
            .read()
            .unwrap()
            .get(&number)
            .map(|(_, line)| line.clone())
            .ok_or_else(|| Problem::not_found(format!("No line {number} on this relay.")))
    }
}

/// The relay's routes, answering to `name`, telling clients apart as
/// `clients` says, and keeping everything in the SQLite file at `db`.
/// Serve it with connection details, so it knows its clients.
pub fn router(name: String, clients: Clients, db: &std::path::Path) -> Result<Router, String> {
    let store = Store::open(db)?;
    let records = store
        .lines()
        .map_err(|e| format!("{} could not be read: {e}", db.display()))?;
    let lines: HashMap<_, _> = records
        .into_iter()
        .filter_map(|record| {
            let line = record.check().ok()?;
            Some((record.number, (record, line)))
        })
        .collect();
    let waiting = store
        .expire_and_count(now().saturating_sub(HOLD_SECONDS))
        .map_err(|e| format!("{} could not be read: {e}", db.display()))?;
    log(format!(
        "{} lines and {waiting} faxes waiting in {}",
        lines.len(),
        db.display()
    ));
    let relay = Arc::new(Relay {
        name,
        clients,
        lines: RwLock::new(lines),
        store,
        faxes: Limit::new(FAXES.0, FAXES.1),
        new_lines: Limit::new(NEW_LINES.0, NEW_LINES.1),
    });
    Ok(Router::new()
        .route("/v1/relay", get(info))
        .route("/v1/lines/{number}", get(get_line).put(put_line))
        .route(
            "/v1/lines/{number}/faxes",
            post(hold_fax).layer(DefaultBodyLimit::max(FAX_LIMIT)),
        )
        .route("/v1/lines/{number}/poll", post(poll))
        .route("/v1/lines/{number}/confirm", post(confirm))
        .fallback(|| async { Problem::not_found("No such endpoint.") })
        .with_state(relay))
}

/// One line on standard output, under the local time.
fn log(message: String) {
    println!(
        "{} {message}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
}

fn number(text: &str) -> Result<Number, Problem> {
    text.parse()
        .map_err(|e: String| Problem::not_found(format!("{e}.")))
}

async fn info(State(relay): State<Arc<Relay>>) -> Json<RelayInfo> {
    Json(RelayInfo {
        name: relay.name.clone(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

async fn get_line(
    State(relay): State<Arc<Relay>>,
    Path(text): Path<String>,
) -> Result<Json<LineRecord>, Problem> {
    let number = number(&text)?;
    relay
        .lines
        .read()
        .unwrap()
        .get(&number)
        .map(|(record, _)| Json(record.clone()))
        .ok_or_else(|| Problem::not_found(format!("No line {number} on this relay.")))
}

/// Keeps a record that checks, unless a newer one is already held. A
/// number is its identity's address, so no other identity can claim it.
async fn put_line(
    State(relay): State<Arc<Relay>>,
    Path(text): Path<String>,
    request: Request,
) -> Result<StatusCode, Problem> {
    let number = number(&text)?;
    let client = relay.client(&request);
    let record: LineRecord = json(request, JSON_LIMIT).await?;
    if record.number != number {
        return Err(Problem::bad_request(format!(
            "The record is for {}, not {number}.",
            record.number
        )));
    }
    let line = record
        .check()
        .map_err(|e| Problem::bad_request(format!("The record does not check: {e}.")))?;
    let mut lines = relay.lines.write().unwrap();
    match lines.get(&number) {
        Some((held, _)) if held.updated > record.updated => {
            return Err(Problem::new(
                StatusCode::CONFLICT,
                format!("A newer record for {number} is held."),
            ));
        }
        Some((held, _)) if *held == record => return Ok(StatusCode::NO_CONTENT),
        Some(_) => log(format!("line {number} updated")),
        None => {
            if lines.len() >= LINES {
                return Err(Problem::new(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "This relay holds as many lines as it can.",
                ));
            }
            take(&relay.new_lines, client, "new lines")?;
            log(format!("line {number} published"));
        }
    }
    stored(relay.store.put_line(&record))?;
    lines.insert(number, (record, line));
    Ok(StatusCode::NO_CONTENT)
}

/// Holds a fax signed by a line published here, for a line published
/// here.
async fn hold_fax(
    State(relay): State<Arc<Relay>>,
    Path(text): Path<String>,
    request: Request,
) -> Result<(StatusCode, Json<Held>), Problem> {
    let number = number(&text)?;
    // Before the body, so a flood costs the relay no more than a refusal.
    take(&relay.faxes, relay.client(&request), "faxes")?;
    let fax: Fax = json(request, FAX_LIMIT).await?;
    if fax.to != number {
        return Err(Problem::bad_request(format!(
            "The fax is for {}, not {number}.",
            fax.to
        )));
    }
    relay.line(number)?;
    let sender = relay.line(fax.from).map_err(|_| {
        Problem::bad_request(format!(
            "Publish {} on this relay before faxing from it.",
            fax.from
        ))
    })?;
    if !fax.signed_by(&sender) {
        return Err(Problem::new(
            StatusCode::UNAUTHORIZED,
            format!("The fax is not signed by {}.", fax.from),
        ));
    }
    let held = Held {
        id: uuid::Uuid::new_v4().to_string(),
        fax,
    };
    let size = held.fax.sealed.len() + held.fax.encapsulated.len() + held.fax.signature.len();
    let now = now();
    let taken = stored(relay.store.hold(
        &held,
        size,
        now,
        now.saturating_sub(HOLD_SECONDS),
        |waiting| {
            waiting.faxes >= HELD_FAXES
                || waiting.bytes + size > HELD_BYTES
                || waiting.stored + size as u64 > STORED_BYTES
        },
    ))?;
    if !taken {
        return Err(Problem::new(
            StatusCode::INSUFFICIENT_STORAGE,
            format!("Too many faxes are waiting for {number} or on this relay; try again later."),
        ));
    }
    log(format!(
        "fax {} from {} to {number}, {:.1} KB",
        short(&held.id),
        held.fax.from,
        size as f64 / 1024.0
    ));
    Ok((StatusCode::ACCEPTED, Json(held)))
}

/// Enough of an id to tell faxes apart in the log.
fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

async fn poll(
    State(relay): State<Arc<Relay>>,
    Path(text): Path<String>,
    request: Request,
) -> Result<Json<HeldFaxes>, Problem> {
    let number = number(&text)?;
    let collect: Collect = json(request, JSON_LIMIT).await?;
    check(&collect, Action::Poll, &relay.name, &relay.line(number)?)?;
    let faxes = stored(
        relay
            .store
            .waiting_for(number, now().saturating_sub(HOLD_SECONDS)),
    )?;
    Ok(Json(HeldFaxes { faxes }))
}

async fn confirm(
    State(relay): State<Arc<Relay>>,
    Path(text): Path<String>,
    request: Request,
) -> Result<StatusCode, Problem> {
    let number = number(&text)?;
    let collect: Collect = json(request, JSON_LIMIT).await?;
    check(&collect, Action::Confirm, &relay.name, &relay.line(number)?)?;
    for (id, sender) in stored(relay.store.deliver(number, &collect.ids))? {
        log(format!(
            "fax {} from {sender} delivered to {number}",
            short(&id)
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Checks `collect` for `line`: its clock, then its signature, which
/// must name this relay.
fn check(collect: &Collect, action: Action, relay: &str, line: &Line) -> Result<(), Problem> {
    if now().abs_diff(collect.at) > CLOCK_SKEW {
        return Err(Problem::bad_request(
            "`at` is more than five minutes from the relay's clock.",
        ));
    }
    if !collect.signed_by(action, relay, line) {
        return Err(Problem::new(
            StatusCode::UNAUTHORIZED,
            format!("The request is not signed by {} for {relay}.", line.number),
        ));
    }
    Ok(())
}

/// A `500` for a store that failed, which is the relay's own fault.
fn stored<T>(result: rusqlite::Result<T>) -> Result<T, Problem> {
    result.map_err(|e| {
        log(format!("the store failed: {e}"));
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The relay could not store that.",
        )
    })
}

/// A JSON body up to `limit` bytes, read as a `T`.
async fn json<T: DeserializeOwned>(request: Request, limit: usize) -> Result<T, Problem> {
    let bytes = axum::body::to_bytes(request.into_body(), limit)
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("The body is larger than {limit} bytes: {e}."),
            )
        })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Problem::bad_request(format!("The body could not be read as JSON: {e}.")))
}

/// An RFC 9457 problem, as the API answers with, so a server reads a
/// relay's refusals the same way.
#[derive(Debug, Serialize)]
struct Problem {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
    /// Seconds, for a `429`.
    #[serde(skip)]
    retry_after: Option<u64>,
}

impl Problem {
    fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            kind: "about:blank",
            title: status.canonical_reason().unwrap_or("Error"),
            status: status.as_u16(),
            detail: detail.into(),
            retry_after: None,
        }
    }

    fn retry_after(mut self, seconds: u64) -> Self {
        self.retry_after = Some(seconds);
        self
    }

    fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }

    fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, detail)
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).expect("made from a StatusCode");
        let body = serde_json::to_vec(&self).expect("problem details serialise");
        let mut response = (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            body,
        )
            .into_response();
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, seconds.into());
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, header};
    use serde_json::Value;
    use starprint_fax::{FaxKey, Identity};
    use tower::ServiceExt as _;

    async fn call(
        router: &Router,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = axum::http::Request::builder().method(method).uri(path);
        let body = match body {
            Some(json) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let response = router
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn test_router(name: &str) -> Router {
        router(
            name.to_owned(),
            Clients::BehindProxy,
            std::path::Path::new(":memory:"),
        )
        .unwrap()
    }

    /// A new line with its record.
    fn test_line() -> (Identity, LineRecord) {
        let identity = Identity::generate();
        let record = identity.record(&FaxKey::generate(), 1);
        (identity, record)
    }

    fn path(number: Number, rest: &str) -> String {
        format!("/v1/lines/{}{rest}", number.slug())
    }

    fn json<T: Serialize>(value: &T) -> Option<Value> {
        Some(serde_json::to_value(value).unwrap())
    }

    #[tokio::test]
    async fn a_relay_says_who_it_is() {
        let (status, body) = call(&test_router("LONRELAY01"), Method::GET, "/v1/relay", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "LONRELAY01");
    }

    #[tokio::test]
    async fn a_fax_waits_for_its_line_to_poll_and_confirm() {
        let router = test_router("LONRELAY01");
        let (anna, anna_record) = test_line();
        let (ben, ben_record) = test_line();
        let (a, b) = (anna_record.number, ben_record.number);
        let ben_line = ben_record.check().unwrap();
        let fax = Fax::seal(&anna, a, &ben_line, b"Hello Ben", now());

        let (status, _) = call(&router, Method::POST, &path(b, "/faxes"), json(&fax)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "Ben is not here yet");
        let (status, _) = call(&router, Method::PUT, &path(b, ""), json(&ben_record)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, body) = call(&router, Method::POST, &path(b, "/faxes"), json(&fax)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "Anna is not here yet");
        assert!(body["detail"].as_str().unwrap().contains("Publish"));
        call(&router, Method::PUT, &path(a, ""), json(&anna_record)).await;
        let (status, _) = call(&router, Method::POST, &path(b, "/faxes"), json(&fax)).await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let (status, body) = call(&router, Method::GET, &path(b, ""), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_value::<LineRecord>(body).unwrap(),
            ben_record
        );

        let (status, _) = call(
            &router,
            Method::POST,
            &path(b, "/poll"),
            json(&Collect::poll(&anna, "LONRELAY01", b)),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "only Ben polls Ben's line"
        );
        let (status, _) = call(
            &router,
            Method::POST,
            &path(b, "/poll"),
            json(&Collect::poll(&ben, "NYCRELAY01", b)),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a poll of another relay is not one of this relay"
        );
        let (_, body) = call(
            &router,
            Method::POST,
            &path(b, "/poll"),
            json(&Collect::poll(&ben, "LONRELAY01", b)),
        )
        .await;
        let held: HeldFaxes = serde_json::from_value(body).unwrap();
        assert_eq!(held.faxes.len(), 1);
        assert_eq!(held.faxes[0].fax, fax);

        let ids = vec![held.faxes[0].id.clone()];
        let (status, _) = call(
            &router,
            Method::POST,
            &path(b, "/confirm"),
            json(&Collect::confirm(&ben, "LONRELAY01", b, ids)),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, body) = call(
            &router,
            Method::POST,
            &path(b, "/poll"),
            json(&Collect::poll(&ben, "LONRELAY01", b)),
        )
        .await;
        assert_eq!(body["faxes"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn a_restart_loses_nothing_and_a_delivered_fax_is_deleted() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let open = || router("LONRELAY01".to_owned(), Clients::Direct, file.path()).unwrap();
        let (anna, anna_record) = test_line();
        let (ben, ben_record) = test_line();
        let (a, b) = (anna_record.number, ben_record.number);
        let ben_line = ben_record.check().unwrap();

        let before = open();
        call(&before, Method::PUT, &path(a, ""), json(&anna_record)).await;
        call(&before, Method::PUT, &path(b, ""), json(&ben_record)).await;
        let fax = Fax::seal(&anna, a, &ben_line, b"Hello Ben", now());
        let (status, _) = call(&before, Method::POST, &path(b, "/faxes"), json(&fax)).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        drop(before);

        let after = open();
        let (_, body) = call(&after, Method::GET, &path(b, ""), None).await;
        assert_eq!(
            serde_json::from_value::<LineRecord>(body).unwrap(),
            ben_record
        );
        let poll = || json(&Collect::poll(&ben, "LONRELAY01", b));
        let (_, body) = call(&after, Method::POST, &path(b, "/poll"), poll()).await;
        let held: HeldFaxes = serde_json::from_value(body).unwrap();
        assert_eq!(held.faxes.len(), 1, "still waiting");
        let ids = vec![held.faxes[0].id.clone()];
        let confirm = json(&Collect::confirm(&ben, "LONRELAY01", b, ids));
        call(&after, Method::POST, &path(b, "/confirm"), confirm).await;
        let (_, body) = call(&after, Method::POST, &path(b, "/poll"), poll()).await;
        assert_eq!(body["faxes"], serde_json::json!([]), "confirmed");

        let left: u64 = rusqlite::Connection::open(file.path())
            .unwrap()
            .query_row("SELECT count(*) FROM faxes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(left, 0, "deleted, not just hidden");
    }

    #[tokio::test]
    async fn an_address_publishes_only_so_many_new_lines() {
        let router = test_router("LONRELAY01");
        let publish = |from: &'static str| {
            let router = router.clone();
            async move {
                let (_, record) = test_line();
                let request = axum::http::Request::builder()
                    .method(Method::PUT)
                    .uri(path(record.number, ""))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("x-forwarded-for", format!("198.51.100.9, {from}"))
                    .body(Body::from(serde_json::to_string(&record).unwrap()))
                    .unwrap();
                router.oneshot(request).await.unwrap()
            }
        };
        for _ in 0..NEW_LINES.0 {
            assert_eq!(publish("192.0.2.1").await.status(), StatusCode::NO_CONTENT);
        }
        let refused = publish("192.0.2.1").await;
        assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(refused.headers().contains_key(header::RETRY_AFTER));
        assert_eq!(
            publish("192.0.2.2").await.status(),
            StatusCode::NO_CONTENT,
            "the last address is the proxy's word for the client"
        );
    }

    #[tokio::test]
    async fn a_newer_record_replaces_an_older_one() {
        let router = test_router("LONRELAY01");
        let (anna, record) = test_line();
        call(
            &router,
            Method::PUT,
            &path(record.number, ""),
            json(&record),
        )
        .await;

        let newer = anna.record(&FaxKey::generate(), 2);
        let (status, _) = call(&router, Method::PUT, &path(record.number, ""), json(&newer)).await;
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "a newer record replaces the old"
        );
        let (status, _) = call(
            &router,
            Method::PUT,
            &path(record.number, ""),
            json(&record),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "an older one does not");

        let mut tampered = newer.clone();
        tampered.updated = 3;
        let (status, _) = call(
            &router,
            Method::PUT,
            &path(record.number, ""),
            json(&tampered),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
