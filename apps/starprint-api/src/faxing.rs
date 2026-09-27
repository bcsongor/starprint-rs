//! The server's fax line: its settings, activating it, sending faxes
//! through relays, and the loop that polls the relays and prints what
//! arrives. The protocol is `starprint-fax`'s; this is the part that
//! talks to relays and printers, and what a fax carries.
//!
//! Every fax prints, under a header saying who sent it and when: there
//! is no approving strangers yet. Numbers are
//! pinned to the identity they had the first time a fax went either
//! way, so a relay cannot swap one later.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Bytes;
use axum::http::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use starprint_fax::{
    Collect, Difficulty, Fax, Held, HeldFaxes, Identity, Line, LineRecord, NAME_LIMIT, Number,
    RelayInfo, base64_bytes, check_relay_name, now,
};
use starprint_workflows::{FaxHeader, Job};

use crate::config::Profile;
use crate::data::Data;
use crate::job::JobRequest;
use crate::printers::{self, Printers};
use crate::problem::Problem;

/// How often the relays are polled.
const POLL_EVERY: Duration = Duration::from_secs(10);

/// What a fax carries, sealed: a job as `/jobs` takes it, and the
/// picture a picture job needs, as JSON. The recipient prints it with its
/// own profile, so nothing about the sender's printer comes along. Every
/// server must read what every other writes, so its fields are protocol
/// too.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Contents {
    job: Job,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "base64_bytes::option"
    )]
    image: Option<Vec<u8>>,
}

/// `fax.json`: the line, the relays it uses and the numbers it has met.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FaxSettings {
    /// `None` until the line is activated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineSecret>,
    /// Who answers, shown to whoever faxes this line.
    #[serde(default)]
    pub name: String,
    /// The profile faxes print on; the first profile when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
    #[serde(default)]
    pub relays: Vec<RelayEntry>,
    /// Identity keys by number, from the first fax either way.
    #[serde(default)]
    pub pins: BTreeMap<Number, Pin>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pin(#[serde(with = "base64_bytes")] pub Vec<u8>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineSecret {
    pub number: Number,
    pub counter: u64,
    #[serde(with = "base64_bytes")]
    pub seed: Vec<u8>,
    /// When the record last changed, so relays keep the newest.
    pub updated: u64,
}

impl LineSecret {
    fn identity(&self) -> Identity {
        Identity::from_seed(self.seed.as_slice().try_into().expect("a 32-byte seed"))
    }
}

/// A relay as the server knows it: the name it gave, where it answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelayEntry {
    pub name: String,
    pub url: String,
}

impl FaxSettings {
    /// Where faxes print, if there is anywhere.
    fn printer(&self, data: &Data) -> Option<Profile> {
        match &self.printer {
            Some(name) => data.profile(name),
            None => data.profiles().into_iter().next(),
        }
    }

    fn pin(&self, number: Number) -> Option<Vec<u8>> {
        self.pins.get(&number).map(|pin| pin.0.clone())
    }
}

/// What the routes and the poll loop share for as long as a server runs.
pub struct Faxing {
    difficulty: Difficulty,
    client: reqwest::Client,
    /// Each relay's last poll by name: `None` when it went through.
    polled: Mutex<HashMap<String, Option<String>>>,
}

impl Faxing {
    pub fn new(difficulty: Difficulty) -> Self {
        Self {
            difficulty,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .build()
                .expect("an HTTP client"),
            polled: Mutex::default(),
        }
    }
}

/// `GET /v1/fax`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaxView {
    number: Option<Number>,
    name: String,
    printer: Option<String>,
    activation: Option<Activation>,
    relays: Vec<RelayView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Activation {
    tried: u64,
    /// The tries mining takes on average; it may take more or fewer.
    expected: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayView {
    name: String,
    url: String,
    /// `None` until it has been polled.
    online: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    problem: Option<String>,
}

pub fn view(printers: &Printers) -> FaxView {
    let settings = printers.data.fax();
    let polled = printers.fax.polled.lock().unwrap();
    FaxView {
        number: settings.line.as_ref().map(|line| line.number),
        name: settings.name,
        printer: settings.printer,
        activation: printers
            .data
            .activating
            .lock()
            .unwrap()
            .as_ref()
            .map(|tried| Activation {
                tried: tried.load(Ordering::Relaxed),
                expected: printers.fax.difficulty.expected_tries(),
            }),
        relays: settings
            .relays
            .into_iter()
            .map(|relay| {
                let last = polled.get(&relay.name);
                RelayView {
                    online: last.map(Option::is_none),
                    problem: last.cloned().flatten(),
                    name: relay.name,
                    url: relay.url,
                }
            })
            .collect(),
    }
}

/// `PUT /v1/fax`: who answers and where faxes print.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsRequest {
    name: String,
    printer: Option<String>,
}

pub fn configure(printers: &Printers, request: SettingsRequest) -> Result<(), Problem> {
    let name = request.name.trim().to_owned();
    if name.chars().count() > NAME_LIMIT {
        return Err(Problem::bad_request(format!(
            "`name` is longer than {NAME_LIMIT} characters."
        )));
    }
    if let Some(printer) = &request.printer {
        printers.find(printer).map_err(|_| {
            Problem::bad_request(format!("No printer named `{printer}` to print faxes on."))
        })?;
    }
    printers
        .data
        .change_fax(|settings| {
            if settings.name != name
                && let Some(line) = &mut settings.line
            {
                line.updated = now().max(line.updated + 1);
            }
            settings.name = name;
            settings.printer = request.printer;
            Ok(())
        })
        .map_err(Problem::not_saved)
}

/// Starts mining a number on a thread of its own, which outlives the
/// server that started it. A `409` if the line is already active.
pub fn activate(printers: &Printers) -> Result<(), Problem> {
    if printers.data.fax().line.is_some() {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "The fax line is already active.",
        ));
    }
    let mut activating = printers.data.activating.lock().unwrap();
    if activating.is_some() {
        return Ok(());
    }
    let tried = Arc::new(AtomicU64::new(0));
    *activating = Some(Arc::clone(&tried));
    let data = Arc::clone(&printers.data);
    let difficulty = printers.fax.difficulty;
    std::thread::spawn(move || {
        let identity = Identity::generate();
        let mined = difficulty.mine(&identity.key().to_bytes(), &tried, &AtomicBool::new(false));
        if let Some((counter, number)) = mined {
            let saved = data.change_fax(|settings| {
                settings.line = Some(LineSecret {
                    number,
                    counter,
                    seed: identity.seed().to_vec(),
                    updated: now(),
                });
                Ok(())
            });
            if let Err(e) = saved {
                eprintln!("starprint-api: the fax line could not be saved: {e}");
            }
        }
        *data.activating.lock().unwrap() = None;
    });
    Ok(())
}

/// `POST /v1/fax/relays`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddRelay {
    url: String,
}

/// Asks the relay at `url` for its name and keeps it under that name,
/// replacing one already called that.
pub async fn add_relay(printers: &Printers, request: AddRelay) -> Result<RelayEntry, Problem> {
    let url = request.url.trim().trim_end_matches('/').to_owned();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Problem::bad_request(format!(
            "`{url}` is not a relay's address; one starts with https://."
        )));
    }
    let info: RelayInfo = relay_json(printers.fax.client.get(format!("{url}/v1/relay")))
        .await
        .map_err(|e| Problem::bad_gateway(format!("No relay answers at {url}: {e}.")))?;
    check_relay_name(&info.name).map_err(|e| Problem::bad_gateway(format!("{url}: {e}.")))?;
    let entry = RelayEntry {
        name: info.name,
        url,
    };
    let added = entry.clone();
    printers
        .data
        .change_fax(|settings| {
            match settings.relays.iter_mut().find(|r| r.name == added.name) {
                Some(existing) => *existing = added,
                None => settings.relays.push(added),
            }
            Ok(())
        })
        .map_err(Problem::not_saved)?;
    Ok(entry)
}

/// `false` when there was none of that name.
pub fn remove_relay(printers: &Printers, name: &str) -> Result<bool, Problem> {
    printers
        .data
        .change_fax(|settings| {
            let before = settings.relays.len();
            settings.relays.retain(|relay| relay.name != name);
            Ok(settings.relays.len() != before)
        })
        .map_err(Problem::not_saved)
}

/// `POST /v1/fax/send`: a number and a job as `/jobs` takes it, without
/// overrides, since the recipient's profile decides.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendRequest {
    pub(crate) to: Number,
    pub(crate) job: Job,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    to: Number,
    /// The recipient's name, as its line gives it.
    name: String,
    relay: String,
}

/// Seals the job to the number and leaves it at the first relay that
/// has the line.
pub async fn send(
    printers: &Printers,
    request: SendRequest,
    image: Option<Bytes>,
) -> Result<Sent, Problem> {
    let settings = printers.data.fax();
    let Some(secret) = settings.line.clone() else {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "Activate the fax line before sending.",
        ));
    };
    if settings.relays.is_empty() {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "Add a relay before sending.",
        ));
    }
    if request.job.needs_image() && image.is_none() {
        return Err(Problem::bad_request(
            "A picture job needs an `image` part, so it must be sent as multipart/form-data.",
        ));
    }
    // Built once here so a job that cannot print is turned away now,
    // rather than failing on the other side where nobody sees it.
    if let Some(profile) = settings.printer(&printers.data) {
        request_for(request.job.clone())
            .document(&profile.printer, image.clone())
            .await?;
    }

    let identity = secret.identity();
    let contents = serde_json::to_vec(&Contents {
        job: request.job,
        image: image.map(|bytes| bytes.to_vec()),
    })
    .expect("contents serialise");
    let client = &printers.fax.client;
    let mut problems = Vec::new();
    for relay in &settings.relays {
        let record = match get_line(client, &relay.url, request.to).await {
            Ok(Some(record)) => record,
            Ok(None) => continue,
            Err(e) => {
                problems.push(format!("{}: {e}", relay.name));
                continue;
            }
        };
        let line = check(printers, record, settings.pin(request.to))
            .await
            .map_err(|e| Problem::bad_gateway(format!("{e}.")))?;
        // The relay holds faxes only from lines it has, so publish ours
        // there first. It is cheap for a relay that already has it.
        let own = identity.record(
            secret.number,
            secret.counter,
            &settings.name,
            secret.updated,
        );
        put_line(client, &relay.url, &own)
            .await
            .map_err(|e| Problem::bad_gateway(format!("{}: {e}.", relay.name)))?;
        let fax = Fax::seal(&identity, secret.number, &line, &contents, now());
        relay_ok(
            client
                .post(line_url(&relay.url, request.to, "/faxes"))
                .json(&fax),
        )
        .await
        .map_err(|e| Problem::bad_gateway(format!("{}: {e}.", relay.name)))?;
        pin(printers, &line)?;
        return Ok(Sent {
            to: line.number,
            name: line.name,
            relay: relay.name.clone(),
        });
    }
    Err(if problems.is_empty() {
        Problem::not_found(format!("No relay has {}.", request.to))
    } else {
        Problem::bad_gateway(format!(
            "{} was not found, and some relays did not answer: {}.",
            request.to,
            problems.join("; ")
        ))
    })
}

fn request_for(job: Job) -> JobRequest {
    JobRequest {
        job,
        cut: None,
        density: None,
        speed: None,
    }
}

/// Checks a record off the async runtime, since a number not seen
/// before costs one Argon2 hash.
async fn check(
    printers: &Printers,
    record: LineRecord,
    pinned: Option<Vec<u8>>,
) -> Result<Line, String> {
    let difficulty = printers.fax.difficulty;
    tokio::task::spawn_blocking(move || record.check(difficulty, pinned.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

fn pin(printers: &Printers, line: &Line) -> Result<(), Problem> {
    let identity = line.identity.to_bytes();
    printers
        .data
        .change_fax(|settings| {
            settings.pins.entry(line.number).or_insert(Pin(identity));
            Ok(())
        })
        .map_err(Problem::not_saved)
}

/// Polls every relay each [`POLL_EVERY`] for as long as the future is
/// polled. Dropping it stops between faxes; a blocking write already
/// under way finishes under its queue guard.
pub async fn serve(printers: Arc<Printers>) {
    // The record each relay last took, by URL, so it is sent again only
    // when it changes or the relay has lost it.
    let mut published = HashMap::new();
    loop {
        poll(&printers, &mut published).await;
        tokio::time::sleep(POLL_EVERY).await;
    }
}

/// One round of every relay: publish the line if need be, print what
/// is waiting and confirm what printed.
pub async fn poll(printers: &Printers, published: &mut HashMap<String, u64>) {
    let settings = printers.data.fax();
    let Some(secret) = settings.line.clone() else {
        return;
    };
    let identity = secret.identity();
    for relay in &settings.relays {
        let result = poll_relay(printers, &settings, &secret, &identity, relay, published).await;
        if let Err(problem) = &result {
            eprintln!("starprint-api: fax relay {}: {problem}", relay.name);
        }
        printers
            .fax
            .polled
            .lock()
            .unwrap()
            .insert(relay.name.clone(), result.err());
    }
}

async fn poll_relay(
    printers: &Printers,
    settings: &FaxSettings,
    secret: &LineSecret,
    identity: &Identity,
    relay: &RelayEntry,
    published: &mut HashMap<String, u64>,
) -> Result<(), String> {
    let client = &printers.fax.client;
    if published.get(&relay.url) != Some(&secret.updated) {
        let record = identity.record(
            secret.number,
            secret.counter,
            &settings.name,
            secret.updated,
        );
        put_line(client, &relay.url, &record).await?;
        published.insert(relay.url.clone(), secret.updated);
    }
    let response = client
        .post(line_url(&relay.url, secret.number, "/poll"))
        .json(&Collect::poll(identity, secret.number))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status() == StatusCode::NOT_FOUND {
        // The relay has lost the line, its file gone or a new relay at
        // the address; publish it next round.
        published.remove(&relay.url);
        return Err("the relay has lost this line; publishing it again".to_owned());
    }
    let held: HeldFaxes = read(response).await?;
    let mut done = Vec::new();
    for Held { id, fax } in held.faxes {
        match receive(printers, settings, relay, identity, secret.number, &fax).await {
            Ok(()) => done.push(id),
            Err(Receive::Refused(problem)) => {
                eprintln!("starprint-api: fax from {} refused: {problem}", fax.from);
                done.push(id);
            }
            Err(Receive::Later(problem)) => {
                eprintln!(
                    "starprint-api: fax from {} will print later: {problem}",
                    fax.from
                );
            }
        }
    }
    if !done.is_empty() {
        relay_ok(
            client
                .post(line_url(&relay.url, secret.number, "/confirm"))
                .json(&Collect::confirm(identity, secret.number, done)),
        )
        .await?;
    }
    Ok(())
}

enum Receive {
    /// Never going to print; the relay may drop it.
    Refused(String),
    /// Kept at the relay and tried again next round.
    Later(String),
}

async fn receive(
    printers: &Printers,
    settings: &FaxSettings,
    relay: &RelayEntry,
    identity: &Identity,
    number: Number,
    fax: &Fax,
) -> Result<(), Receive> {
    let client = &printers.fax.client;
    let record = get_line(client, &relay.url, fax.from)
        .await
        .map_err(Receive::Later)?
        .ok_or_else(|| Receive::Refused(format!("the relay no longer has {}", fax.from)))?;
    let sender = check(printers, record, settings.pin(fax.from))
        .await
        .map_err(Receive::Refused)?;
    let contents = fax
        .open(identity, number, &sender)
        .map_err(Receive::Refused)?;
    let contents: Contents = serde_json::from_slice(&contents)
        .map_err(|e| Receive::Refused(format!("the fax's contents are not a job: {e}")))?;
    pin(printers, &sender).map_err(|p| Receive::Later(p.detail().to_owned()))?;
    let Some(profile) = settings.printer(&printers.data) else {
        return Err(Receive::Later(
            "there is no printer to print it on".to_owned(),
        ));
    };
    if profile.printer.host.trim().is_empty() {
        return Err(Receive::Later(format!("{} has no address", profile.name)));
    }
    let header = FaxHeader {
        name: sender.name.clone(),
        number: sender.number.to_string(),
        sent: local(fax.sent),
    };
    let payload = request_for(contents.job)
        .fax(&profile.printer, contents.image.map(Bytes::from), header)
        .await
        .map_err(|p| Receive::Refused(p.detail().to_owned()))?;
    printers::send(&profile.printer, payload, &printers.queue)
        .await
        .map_err(|p| Receive::Later(p.detail().to_owned()))?;
    Ok(())
}

/// Unix seconds on this server's clock, for the header.
fn local(seconds: u64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(seconds.try_into().unwrap_or(i64::MAX), 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local)
        .naive_local()
}

fn line_url(relay: &str, number: Number, rest: &str) -> String {
    format!("{relay}/v1/lines/{}{rest}", number.digits())
}

/// The record for `number`, or `None` if the relay does not have it.
async fn get_line(
    client: &reqwest::Client,
    relay: &str,
    number: Number,
) -> Result<Option<LineRecord>, String> {
    let response = client
        .get(line_url(relay, number, ""))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    read(response).await.map(Some)
}

async fn put_line(
    client: &reqwest::Client,
    relay: &str,
    record: &LineRecord,
) -> Result<(), String> {
    relay_ok(client.put(line_url(relay, record.number, "")).json(record)).await
}

async fn relay_json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T, String> {
    read(request.send().await.map_err(|e| e.to_string())?).await
}

async fn relay_ok(request: reqwest::RequestBuilder) -> Result<(), String> {
    let response = request.send().await.map_err(|e| e.to_string())?;
    if response.status().is_success() {
        return Ok(());
    }
    Err(refusal(response).await)
}

async fn read<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, String> {
    if !response.status().is_success() {
        return Err(refusal(response).await);
    }
    response.json().await.map_err(|e| e.to_string())
}

/// A relay's problem detail, or its status when it sent none.
async fn refusal(response: reqwest::Response) -> String {
    let status = response.status();
    response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|body| body["detail"].as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("the relay answered {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printers::PrintQueue;
    use starprint_workflows::Printer;
    use tokio::io::AsyncReadExt as _;
    use tokio::net::TcpListener;

    /// A relay on a port of its own, since the servers reach it over HTTP.
    async fn relay() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                starprint_relay::router(
                    "LONRELAY01".to_owned(),
                    Difficulty::TEST,
                    std::path::Path::new(":memory:"),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        });
        format!("http://{addr}")
    }

    fn server(printer_port: u16) -> Printers {
        let data = Data::ephemeral(
            vec![Profile {
                name: "sp743".to_owned(),
                printer: Printer::impact("127.0.0.1".to_owned(), printer_port),
                notes: String::new(),
            }],
            "t".to_owned(),
        );
        let mut printers = Printers::new(Arc::new(data), Arc::new(PrintQueue::default()));
        printers.fax = Faxing::new(Difficulty::TEST);
        printers
    }

    async fn active(printers: &Printers, name: &str, relay: &str) -> Number {
        configure(
            printers,
            SettingsRequest {
                name: name.to_owned(),
                printer: None,
            },
        )
        .unwrap();
        activate(printers).unwrap();
        while printers.data.activating.lock().unwrap().is_some() {
            tokio::task::yield_now().await;
        }
        let entry = add_relay(
            printers,
            AddRelay {
                url: format!("{relay}/"),
            },
        )
        .await
        .unwrap();
        assert_eq!(entry.name, "LONRELAY01");
        assert_eq!(entry.url, relay, "without the slash");
        printers.data.fax().line.unwrap().number
    }

    fn note() -> SendRequest {
        SendRequest {
            to: "*0000 000001".parse().unwrap(),
            job: serde_json::from_value(serde_json::json!({ "kind": "text", "text": "Hello Ben" }))
                .unwrap(),
        }
    }

    #[tokio::test]
    async fn a_fax_prints_on_the_other_side() {
        let relay = relay().await;
        let printer = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let anna = server(1);
        let ben = server(printer.local_addr().unwrap().port());
        active(&anna, "Anna", &relay).await;
        let ben_number = active(&ben, "Ben", &relay).await;

        let mut request = note();
        request.to = ben_number;
        let error = send(&anna, request, None).await.unwrap_err();
        assert_eq!(error.detail(), format!("No relay has {ben_number}."));

        // Ben's first poll publishes his line.
        let mut published = HashMap::new();
        poll(&ben, &mut published).await;
        let mut request = note();
        request.to = ben_number;
        let sent = send(&anna, request, None).await.unwrap();
        assert_eq!(sent.name, "Ben");
        assert_eq!(sent.relay, "LONRELAY01");

        let received = tokio::spawn(async move {
            let (mut socket, _) = printer.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        poll(&ben, &mut published).await;
        let bytes = received.await.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let anna_number = anna.data.fax().line.unwrap().number;
        let header = text.find(" FAX ").expect("a header");
        let from = text.find(&anna_number.to_string()).expect("the sender");
        let name = text.find("Anna").expect("the sender's name");
        let fax = text.find("Hello Ben").expect("the fax");
        assert!(header < from && from < name && name < fax, "{text}");
        let view = view(&ben);
        assert_eq!(view.relays[0].online, Some(true));

        // Confirmed, so nothing prints twice.
        poll(&ben, &mut published).await;
        assert!(ben.data.fax().pins.contains_key(&anna_number));
        assert!(anna.data.fax().pins.contains_key(&ben_number));
    }

    #[tokio::test]
    async fn sending_needs_a_line_and_a_relay() {
        let anna = server(1);
        let error = send(&anna, note(), None).await.unwrap_err();
        assert!(error.detail().starts_with("Activate"));
        activate(&anna).unwrap();
        while anna.data.activating.lock().unwrap().is_some() {
            tokio::task::yield_now().await;
        }
        let error = send(&anna, note(), None).await.unwrap_err();
        assert!(error.detail().starts_with("Add a relay"));
        assert_eq!(
            activate(&anna).unwrap_err().detail(),
            "The fax line is already active."
        );
    }

    #[tokio::test]
    async fn a_relay_is_known_by_the_name_it_gives() {
        let anna = server(1);
        let error = add_relay(
            &anna,
            AddRelay {
                url: "relay.example".to_owned(),
            },
        )
        .await
        .unwrap_err();
        assert!(error.detail().contains("https://"));
        let relay = relay().await;
        add_relay(&anna, AddRelay { url: relay.clone() })
            .await
            .unwrap();
        add_relay(&anna, AddRelay { url: relay }).await.unwrap();
        assert_eq!(anna.data.fax().relays.len(), 1, "one per name");
        assert!(remove_relay(&anna, "LONRELAY01").unwrap());
        assert!(!remove_relay(&anna, "LONRELAY01").unwrap());
    }
}
