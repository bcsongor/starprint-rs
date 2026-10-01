//! The server's fax line: its settings, activating it, sending faxes
//! through relays, and the loop that polls the relays and prints what
//! arrives. The protocol is `starprint-fax`'s; this is the part that
//! talks to relays and printers, and what a fax carries.
//!
//! Every fax prints, under a header saying who sent it and when: there
//! is no approving strangers yet. A number is the address of its line's
//! identity key, so a relay cannot hand out another key under it.
//!
//! The key faxes are sealed to is replaced every week and forgotten
//! once no fax sealed to it can still be waiting, so secrets stolen
//! later open only the last few weeks. The line also remembers the faxes
//! it has printed for as long, so one handed over twice prints once.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Bytes;
use axum::http::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use starprint_fax::{
    Collect, Fax, FaxKey, Held, HeldFaxes, Identity, Line, LineRecord, NAME_LIMIT, Number,
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
const DAY: u64 = 24 * 60 * 60;
/// How long a fax key is the one the record carries.
const ROTATE_AFTER: u64 = 7 * DAY;
/// How long a relay holds a fax nobody polls for.
const RELAY_HOLD: u64 = 30 * DAY;
/// How long a fax key and the memory of a printed fax are kept: until
/// the last fax sealed to the key has left its relay, and a day over for
/// clocks that disagree.
const KEEP: u64 = ROTATE_AFTER + RELAY_HOLD + DAY;

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

/// `fax.json`: the line, the relays it uses and the fax book.
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
    /// The fax book, by name.
    #[serde(default)]
    pub contacts: Vec<Contact>,
}

/// A number in the fax book, under the name its owner here gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contact {
    pub name: String,
    pub number: Number,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineSecret {
    pub number: Number,
    #[serde(with = "base64_bytes")]
    pub seed: Vec<u8>,
    /// When the record last changed, so relays keep the newest.
    pub updated: u64,
    /// The keys faxes are sealed to, oldest first. The last is the one
    /// the record carries; the rest open faxes still on their way.
    #[serde(default)]
    pub fax_keys: Vec<StoredKey>,
    /// The faxes printed, by [`Fax::id`], and when.
    #[serde(default)]
    pub printed: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredKey {
    /// Unix seconds.
    pub created: u64,
    #[serde(with = "base64_bytes")]
    pub seed: Vec<u8>,
}

impl StoredKey {
    fn new(created: u64) -> Self {
        Self {
            created,
            seed: FaxKey::generate().seed().to_vec(),
        }
    }

    fn key(&self) -> FaxKey {
        FaxKey::from_seed(self.seed.as_slice().try_into().expect("a 32-byte seed"))
    }
}

impl LineSecret {
    fn new(identity: &Identity, now: u64) -> Self {
        Self {
            number: identity.number(),
            seed: identity.seed().to_vec(),
            updated: now,
            fax_keys: vec![StoredKey::new(now)],
            printed: BTreeMap::new(),
        }
    }

    fn identity(&self) -> Identity {
        Identity::from_seed(self.seed.as_slice().try_into().expect("a 32-byte seed"))
    }

    /// Whether the newest fax key is due to be replaced, or missing.
    fn is_stale(&self, now: u64) -> bool {
        self.fax_keys
            .last()
            .is_none_or(|key| now.saturating_sub(key.created) >= ROTATE_AFTER)
    }

    /// Makes a new fax key if the newest is due, and forgets the keys
    /// and printed faxes kept long enough. The record changes with the
    /// key, so it is dated again.
    fn rotate(&mut self, now: u64) {
        if self.is_stale(now) {
            self.fax_keys.push(StoredKey::new(now));
            self.updated = now.max(self.updated + 1);
        }
        let newest = self.fax_keys.len() - 1;
        let mut index = 0;
        self.fax_keys.retain(|key| {
            let keep = index == newest || now.saturating_sub(key.created) < KEEP;
            index += 1;
            keep
        });
        self.printed
            .retain(|_, printed| now.saturating_sub(*printed) < KEEP);
    }

    /// The record's fax key. There is one once [`Self::rotate`] has run.
    fn fax_key(&self) -> Result<FaxKey, String> {
        self.fax_keys
            .last()
            .map(StoredKey::key)
            .ok_or_else(|| "the line has no fax key yet".to_owned())
    }
}

/// The settings, once the line's fax key is fresh: rotating it writes
/// `fax.json`, so that happens only when it is due.
fn settings(printers: &Printers) -> Result<FaxSettings, String> {
    let now = now();
    let settings = printers.data.fax();
    if !settings
        .line
        .as_ref()
        .is_some_and(|line| line.is_stale(now))
    {
        return Ok(settings);
    }
    printers.data.change_fax(|settings| {
        if let Some(line) = &mut settings.line {
            line.rotate(now);
        }
        Ok(settings.clone())
    })
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
}

/// What the routes and the poll loop share for as long as a server runs.
pub struct Faxing {
    client: reqwest::Client,
    /// Each relay's last poll by name: `None` when it went through.
    polled: Mutex<HashMap<String, Option<String>>>,
}

impl Default for Faxing {
    fn default() -> Self {
        Self {
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
    relays: Vec<RelayView>,
    contacts: Vec<Contact>,
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
        contacts: settings.contacts,
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

/// Gives the line a new identity, whose address is its number. With
/// `replace`, an active line gets one too, and nobody holding its old
/// number reaches it after. Without it, a `409` if the line is already
/// active.
pub fn activate(printers: &Printers, replace: bool) -> Result<(), Problem> {
    if !replace && printers.data.fax().line.is_some() {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            "The fax line is already active.",
        ));
    }
    let line = LineSecret::new(&Identity::generate(), now());
    printers
        .data
        .change_fax(|settings| {
            settings.line = Some(line);
            Ok(())
        })
        .map_err(Problem::not_saved)
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
    // It just answered, so it is online until a poll says otherwise.
    printers
        .fax
        .polled
        .lock()
        .unwrap()
        .insert(entry.name.clone(), None);
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

/// `PUT /v1/fax/contacts/{number}`: the name to file a number under.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContactRequest {
    name: String,
}

/// Files `number` under a name in the fax book, replacing its old one.
/// A `409` if another number already has that name, since faxes can be
/// sent to a name.
pub fn put_contact(
    printers: &Printers,
    number: Number,
    request: ContactRequest,
) -> Result<Contact, Problem> {
    let name = request.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > NAME_LIMIT {
        return Err(Problem::bad_request(format!(
            "`name` must be 1 to {NAME_LIMIT} characters."
        )));
    }
    if name.parse::<Number>().is_ok() {
        return Err(Problem::bad_request("`name` cannot be a number."));
    }
    let contact = Contact { name, number };
    let filed = contact.clone();
    let taken = printers
        .data
        .change_fax(|settings| {
            if settings
                .contacts
                .iter()
                .any(|c| c.number != number && c.name.eq_ignore_ascii_case(&filed.name))
            {
                return Ok(true);
            }
            settings.contacts.retain(|c| c.number != number);
            settings.contacts.push(filed);
            settings.contacts.sort_by_key(|c| c.name.to_lowercase());
            Ok(false)
        })
        .map_err(Problem::not_saved)?;
    if taken {
        return Err(Problem::new(
            StatusCode::CONFLICT,
            format!("Another number is already called `{}`.", contact.name),
        ));
    }
    Ok(contact)
}

/// `false` when the number was not in the fax book.
pub fn remove_contact(printers: &Printers, number: Number) -> Result<bool, Problem> {
    printers
        .data
        .change_fax(|settings| {
            let before = settings.contacts.len();
            settings.contacts.retain(|c| c.number != number);
            Ok(settings.contacts.len() != before)
        })
        .map_err(Problem::not_saved)
}

/// `POST /v1/fax/send`: a number, or a name in the fax book, and a job
/// as `/jobs` takes it, without overrides, since the recipient's profile
/// decides.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendRequest {
    pub(crate) to: String,
    pub(crate) job: Job,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    to: Number,
    /// The recipient's name in the fax book, or as its line gives it.
    name: String,
    /// Whether `to` is in the fax book.
    contact: bool,
    relay: String,
}

impl FaxSettings {
    /// The contact filed under `number`, if any.
    fn contact(&self, number: Number) -> Option<&Contact> {
        self.contacts.iter().find(|c| c.number == number)
    }

    /// `to` as a number: one written out, or a name in the fax book. A
    /// `400` for text that starts like a number but is not one, so a typo
    /// is reported as one.
    fn resolve(&self, to: &str) -> Result<Number, Problem> {
        let to = to.trim();
        let numberish = to.starts_with('*') || to.to_lowercase().starts_with("star1");
        match to.parse::<Number>() {
            Ok(number) => Ok(number),
            Err(e) if numberish => Err(Problem::bad_request(format!("{e}."))),
            Err(_) => self
                .contacts
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(to))
                .map(|c| c.number)
                .ok_or_else(|| {
                    Problem::not_found(format!("No one called `{to}` in the fax book."))
                }),
        }
    }
}

/// Seals the job to the number and leaves it at the first relay that
/// has the line.
pub async fn send(
    printers: &Printers,
    request: SendRequest,
    image: Option<Bytes>,
) -> Result<Sent, Problem> {
    let settings = settings(printers).map_err(Problem::not_saved)?;
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
    let to = settings.resolve(&request.to)?;
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
    let fax_key = secret.fax_key().map_err(Problem::not_saved)?;
    let contents = serde_json::to_vec(&Contents {
        job: request.job,
        image: image.map(|bytes| bytes.to_vec()),
    })
    .expect("contents serialise");
    let client = &printers.fax.client;
    let mut problems = Vec::new();
    for relay in &settings.relays {
        let record = match get_line(client, &relay.url, to).await {
            Ok(Some(record)) => record,
            Ok(None) => continue,
            Err(e) => {
                problems.push(format!("{}: {e}", relay.name));
                continue;
            }
        };
        let line = checked(record, to).map_err(|e| Problem::bad_gateway(format!("{e}.")))?;
        // The relay holds faxes only from lines it has, so publish ours
        // there first. It is cheap for a relay that already has it.
        let own = identity.record(&fax_key, &settings.name, secret.updated);
        put_line(client, &relay.url, &own)
            .await
            .map_err(|e| Problem::bad_gateway(format!("{}: {e}.", relay.name)))?;
        let fax = Fax::seal(&identity, secret.number, &line, &contents, now());
        relay_ok(client.post(line_url(&relay.url, to, "/faxes")).json(&fax))
            .await
            .map_err(|e| Problem::bad_gateway(format!("{}: {e}.", relay.name)))?;
        let contact = settings.contact(to);
        return Ok(Sent {
            to,
            name: contact.map_or(line.name, |c| c.name.clone()),
            contact: contact.is_some(),
            relay: relay.name.clone(),
        });
    }
    Err(if problems.is_empty() {
        Problem::not_found(format!("No relay has {}.", to))
    } else {
        Problem::bad_gateway(format!(
            "{} was not found, and some relays did not answer: {}.",
            to,
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

/// `record` checked, and for `number`, since a relay could answer with
/// another line's.
fn checked(record: LineRecord, number: Number) -> Result<Line, String> {
    let line = record.check()?;
    if line.number != number {
        return Err(format!(
            "the relay answered for {} with {}",
            number, line.number
        ));
    }
    Ok(line)
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
    let settings = match settings(printers) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("starprint-api: the fax key could not be replaced: {e}");
            return;
        }
    };
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
        let record = identity.record(&secret.fax_key()?, &settings.name, secret.updated);
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
        match receive(printers, settings, relay, secret, &fax).await {
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
    secret: &LineSecret,
    fax: &Fax,
) -> Result<(), Receive> {
    // Read afresh, since this round may have printed it already.
    let id = fax.id();
    let printed = |settings: &FaxSettings| {
        settings
            .line
            .as_ref()
            .is_some_and(|line| line.printed.contains_key(&id))
    };
    if printed(&printers.data.fax()) {
        return Err(Receive::Refused("it has already printed".to_owned()));
    }
    let client = &printers.fax.client;
    let record = get_line(client, &relay.url, fax.from)
        .await
        .map_err(Receive::Later)?
        .ok_or_else(|| Receive::Refused(format!("the relay no longer has {}", fax.from)))?;
    let sender = checked(record, fax.from).map_err(Receive::Refused)?;
    // Newest first: all but a fax that waited is sealed to that one.
    let contents = secret
        .fax_keys
        .iter()
        .rev()
        .map(|key| fax.open(&key.key(), secret.number, &sender))
        .reduce(|opened, next| opened.or(next))
        .unwrap_or_else(|| Err("the line has no fax key".to_owned()))
        .map_err(Receive::Refused)?;
    let contents: Contents = serde_json::from_slice(&contents)
        .map_err(|e| Receive::Refused(format!("the fax's contents are not a job: {e}")))?;
    let Some(profile) = settings.printer(&printers.data) else {
        return Err(Receive::Later(
            "there is no printer to print it on".to_owned(),
        ));
    };
    if profile.printer.host.trim().is_empty() {
        return Err(Receive::Later(format!("{} has no address", profile.name)));
    }
    let header = FaxHeader {
        name: settings
            .contact(sender.number)
            .map_or_else(|| sender.name.clone(), |c| c.name.clone()),
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
    // It printed whether or not this is saved, so a failure is only
    // logged: the worst it costs is a second copy.
    let remembered = printers.data.change_fax(|settings| {
        if let Some(line) = &mut settings.line {
            line.printed.insert(id, now());
        }
        Ok(())
    });
    if let Err(e) = remembered {
        eprintln!("starprint-api: a printed fax could not be remembered: {e}");
    }
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
    format!("{relay}/v1/lines/{}{rest}", number.slug())
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
                    starprint_relay::Clients::Direct,
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
        Printers::new(Arc::new(data), Arc::new(PrintQueue::default()))
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
        activate(printers, false).unwrap();
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
            to: "*star1y0stjmvgr0el9qnxd0uy8c2ge52hd4p2".to_owned(),
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
        request.to = ben_number.to_string();
        let error = send(&anna, request, None).await.unwrap_err();
        assert_eq!(error.detail(), format!("No relay has {ben_number}."));

        // Ben's first poll publishes his line.
        let mut published = HashMap::new();
        poll(&ben, &mut published).await;
        let mut request = note();
        request.to = ben_number.to_string();
        let sent = send(&anna, request, None).await.unwrap();
        assert_eq!(sent.name, "Ben", "as his line gives it");
        assert!(!sent.contact);
        assert_eq!(sent.relay, "LONRELAY01");

        // Ben files Anna under a name of his own, which her faxes print
        // under from now on.
        let anna_number = anna.data.fax().line.unwrap().number;
        put_contact(
            &ben,
            anna_number,
            ContactRequest {
                name: "Mum".to_owned(),
            },
        )
        .unwrap();

        // Ben replaces his fax key while the fax waits; the one it was
        // sealed to is kept, so it still opens.
        ben.data
            .change_fax(|settings| {
                let line = settings.line.as_mut().unwrap();
                line.rotate(now() + ROTATE_AFTER);
                Ok(())
            })
            .unwrap();
        let ben_line = ben.data.fax().line.unwrap();
        assert_eq!(ben_line.fax_keys.len(), 2);
        let held: HeldFaxes = ben
            .fax
            .client
            .post(line_url(&relay, ben_number, "/poll"))
            .json(&Collect::poll(&ben_line.identity(), ben_number))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let waiting = held.faxes[0].fax.clone();

        let received = tokio::spawn(async move {
            let (mut socket, _) = printer.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        poll(&ben, &mut published).await;
        let bytes = received.await.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let header = text.find(" FAX ").expect("a header");
        let from = text.find(&anna_number.to_string()).expect("the sender");
        let name = text.find("Mum").expect("the sender's name in the fax book");
        let fax = text.find("Hello Ben").expect("the fax");
        assert!(header < from && from < name && name < fax, "{text}");
        let view = view(&ben);
        assert_eq!(view.relays[0].online, Some(true));

        // Confirmed, so nothing prints twice.
        poll(&ben, &mut published).await;

        // Nor does a relay handing the same fax over again: the printer
        // is no longer listening, and nothing tries to reach it.
        let settings = ben.data.fax();
        assert_eq!(settings.line.as_ref().unwrap().printed.len(), 1);
        let again = receive(&ben, &settings, &settings.relays[0], &ben_line, &waiting).await;
        assert!(
            matches!(&again, Err(Receive::Refused(why)) if why.contains("already printed")),
            "printed once"
        );
    }

    #[tokio::test]
    async fn sending_needs_a_line_and_a_relay() {
        let anna = server(1);
        let error = send(&anna, note(), None).await.unwrap_err();
        assert!(error.detail().starts_with("Activate"));
        activate(&anna, false).unwrap();
        let error = send(&anna, note(), None).await.unwrap_err();
        assert!(error.detail().starts_with("Add a relay"));
        assert_eq!(
            activate(&anna, false).unwrap_err().detail(),
            "The fax line is already active."
        );

        // Replacing mines a new number under a new identity.
        let before = anna.data.fax().line.unwrap();
        activate(&anna, true).unwrap();
        let after = anna.data.fax().line.unwrap();
        assert_ne!(after.seed, before.seed);
        assert_ne!(after.number, before.number);
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
        assert_eq!(view(&anna).relays[0].online, Some(true), "it just answered");
        assert!(remove_relay(&anna, "LONRELAY01").unwrap());
        assert!(!remove_relay(&anna, "LONRELAY01").unwrap());
    }

    #[test]
    fn a_fax_key_is_replaced_weekly_and_forgotten_once_nothing_needs_it() {
        let mut line = LineSecret::new(&Identity::generate(), 0);
        line.printed.insert("an old fax".to_owned(), 0);
        let first = line.fax_keys[0].seed.clone();
        line.rotate(ROTATE_AFTER - 1);
        assert_eq!((line.fax_keys.len(), line.updated), (1, 0), "not yet due");

        line.rotate(ROTATE_AFTER);
        assert_eq!(line.fax_keys.len(), 2, "a new one, the old one kept");
        assert_eq!(line.fax_keys[0].seed, first);
        assert_eq!(line.updated, ROTATE_AFTER, "the record is dated again");
        assert_eq!(line.printed.len(), 1);

        // A fax sealed to the first key can wait at a relay for as long
        // as the relay holds it, so the key outlives its week by that.
        line.rotate(KEEP);
        let seeds: Vec<_> = line.fax_keys.iter().map(|key| &key.seed).collect();
        assert_eq!(seeds.len(), 2, "one more made, the first forgotten");
        assert!(!seeds.contains(&&first));
        assert!(line.printed.is_empty(), "and so is what printed then");
    }

    #[test]
    fn the_fax_book_files_a_number_under_one_name() {
        let anna = server(1);
        let (ben, carl) = (Number::of(b"ben"), Number::of(b"carl"));
        let name = |name: &str| ContactRequest {
            name: name.to_owned(),
        };
        put_contact(&anna, ben, name(" Ben ")).unwrap();
        put_contact(&anna, carl, name("Carl")).unwrap();
        let error = put_contact(&anna, carl, name("ben")).unwrap_err();
        assert_eq!(error.status(), StatusCode::CONFLICT, "names are unique");
        put_contact(&anna, ben, name("Benjamin")).unwrap();

        let settings = anna.data.fax();
        let names: Vec<_> = settings.contacts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Benjamin", "Carl"], "renamed, in order");
        assert_eq!(settings.resolve("carl").unwrap(), carl, "by name");
        assert_eq!(
            settings.resolve(&ben.to_string()).unwrap(),
            ben,
            "by number"
        );
        let typo = ben.to_string().replace('q', "p").replace('z', "q");
        assert_eq!(
            settings.resolve(&typo).unwrap_err().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            settings.resolve("Dora").unwrap_err().status(),
            StatusCode::NOT_FOUND
        );

        assert!(remove_contact(&anna, ben).unwrap());
        assert!(!remove_contact(&anna, ben).unwrap());
    }
}
