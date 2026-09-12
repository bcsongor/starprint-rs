//! The data directory: profiles, schedules and the token, on disk and
//! in memory.
//!
//! Each file is read once at startup and written whole after every
//! change, under the lock that guards its contents, so the last write
//! wins and the file always holds a complete list. A change is written
//! before it is kept, so a failed write leaves memory and disk as they
//! were. A hand edit while the server runs is overwritten by the next
//! change made over the API. One server at a time: the directory is
//! locked for as long as it is open, since two would overwrite each
//! other's files.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::config::{self, Profile};
use crate::schedule::ScheduleSpec;

const PROFILES: &str = "printers.json";
const SCHEDULES: &str = "schedules.json";
const TOKEN: &str = "token";
const LOCK: &str = "lock";

pub struct Data {
    /// `None` keeps everything in memory, for the tests.
    dir: Option<PathBuf>,
    /// Held for as long as the directory is open.
    _lock: Option<File>,
    token: String,
    profiles: RwLock<Vec<Profile>>,
    schedules: RwLock<BTreeMap<String, ScheduleSpec>>,
}

impl Data {
    /// `starprint` under the platform's configuration directory, the
    /// command line's default.
    pub fn default_dir() -> Result<PathBuf, String> {
        dirs::config_dir()
            .map(|dir| dir.join("starprint"))
            .ok_or_else(|| "this system has no configuration directory; pass --data".to_owned())
    }

    /// Reads `dir`, creating it if need be. `token` replaces the one on
    /// file; without one, the file's is used, or a new one is written.
    /// A file the server cannot work from is an error here.
    pub fn open(dir: &Path, token: Option<String>) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let lock = File::create(dir.join(LOCK)).map_err(|e| format!("{}: {e}", dir.display()))?;
        lock.try_lock()
            .map_err(|_| format!("{}: another server has it open", dir.display()))?;
        let profiles = config::read(&dir.join(PROFILES))?;
        let schedules = read_schedules(&dir.join(SCHEDULES))?;
        let token_path = dir.join(TOKEN);
        // Trimmed either way, as a request's bearer is.
        let token = match token {
            Some(token) => {
                let token = token.trim().to_owned();
                replace(&token_path, token.as_bytes())?;
                token
            }
            None => match std::fs::read_to_string(&token_path) {
                Ok(token) if !token.trim().is_empty() => token.trim().to_owned(),
                Ok(_) => generate_token(&token_path)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => generate_token(&token_path)?,
                Err(e) => return Err(format!("{}: {e}", token_path.display())),
            },
        };
        Ok(Self {
            dir: Some(dir.to_owned()),
            _lock: Some(lock),
            token,
            profiles: RwLock::new(profiles),
            schedules: RwLock::new(schedules),
        })
    }

    /// Nothing is written anywhere; changes last as long as the process.
    #[cfg(test)]
    pub fn ephemeral(profiles: Vec<Profile>, token: String) -> Self {
        Self {
            dir: None,
            _lock: None,
            token,
            profiles: RwLock::new(profiles),
            schedules: RwLock::default(),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// In the order the file lists them; a new profile goes last.
    pub fn profiles(&self) -> Vec<Profile> {
        self.profiles.read().unwrap().clone()
    }

    pub fn profile(&self, name: &str) -> Option<Profile> {
        self.profiles
            .read()
            .unwrap()
            .iter()
            .find(|profile| profile.name == name)
            .cloned()
    }

    /// Replaces the profile of that name in place, or adds one. `true`
    /// when it was new.
    pub fn put_profile(&self, profile: Profile) -> Result<bool, String> {
        let mut profiles = self.profiles.write().unwrap();
        let mut next = profiles.clone();
        let created = match next.iter_mut().find(|p| p.name == profile.name) {
            Some(existing) => {
                *existing = profile;
                false
            }
            None => {
                next.push(profile);
                true
            }
        };
        self.save_profiles(&next)?;
        *profiles = next;
        Ok(created)
    }

    /// `false` when there was none. Its schedules stay, and do not run.
    pub fn remove_profile(&self, name: &str) -> Result<bool, String> {
        let mut profiles = self.profiles.write().unwrap();
        let Some(index) = profiles.iter().position(|p| p.name == name) else {
            return Ok(false);
        };
        let mut next = profiles.clone();
        next.remove(index);
        self.save_profiles(&next)?;
        *profiles = next;
        Ok(true)
    }

    /// By id.
    pub fn schedules(&self) -> Vec<(String, ScheduleSpec)> {
        self.schedules
            .read()
            .unwrap()
            .iter()
            .map(|(id, spec)| (id.clone(), spec.clone()))
            .collect()
    }

    /// Returns the id the schedule was given.
    pub fn add_schedule(&self, spec: ScheduleSpec) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut schedules = self.schedules.write().unwrap();
        let mut next = schedules.clone();
        next.insert(id.clone(), spec);
        self.save_schedules(&next)?;
        *schedules = next;
        Ok(id)
    }

    /// `false` when there is no schedule with that id.
    pub fn put_schedule(&self, id: &str, spec: ScheduleSpec) -> Result<bool, String> {
        let mut schedules = self.schedules.write().unwrap();
        if !schedules.contains_key(id) {
            return Ok(false);
        }
        let mut next = schedules.clone();
        next.insert(id.to_owned(), spec);
        self.save_schedules(&next)?;
        *schedules = next;
        Ok(true)
    }

    /// `false` when there was none.
    pub fn remove_schedule(&self, id: &str) -> Result<bool, String> {
        let mut schedules = self.schedules.write().unwrap();
        let mut next = schedules.clone();
        if next.remove(id).is_none() {
            return Ok(false);
        }
        self.save_schedules(&next)?;
        *schedules = next;
        Ok(true)
    }

    fn save_profiles(&self, profiles: &[Profile]) -> Result<(), String> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        config::write(&dir.join(PROFILES), profiles)
    }

    fn save_schedules(&self, schedules: &BTreeMap<String, ScheduleSpec>) -> Result<(), String> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        let text = serde_json::to_vec_pretty(schedules).map_err(|e| e.to_string())?;
        replace(&dir.join(SCHEDULES), &text)
    }
}

/// The schedules by id, with their cron and job kind checked. Their
/// printers may have been deleted since they were saved.
fn read_schedules(path: &Path) -> Result<BTreeMap<String, ScheduleSpec>, String> {
    let schedules: BTreeMap<String, ScheduleSpec> = match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    for (id, spec) in &schedules {
        spec.check()
            .map_err(|problem| format!("{}: {id}: {}", path.display(), problem.detail()))?;
    }
    Ok(schedules)
}

fn generate_token(path: &Path) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().to_string();
    replace(path, token.as_bytes())?;
    Ok(token)
}

/// Writes the whole file through a sibling and a rename, so a crash
/// mid-write leaves the old file rather than half of the new one. The
/// token is a secret, so on Unix the file is the owner's alone; the
/// other two get the same treatment for want of a reason not to.
pub fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let staging = path.with_file_name(format!("{name}.tmp"));
    create_private(&staging)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&staging, path))
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

/// Windows files inherit the profile directory's ACL, which is already
/// the user's own.
#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::create(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use starprint_workflows::Printer;

    fn profile(name: &str) -> Profile {
        Profile {
            name: name.to_owned(),
            printer: Printer::impact("printer.invalid".to_owned(), 9100),
            notes: String::new(),
        }
    }

    fn spec(printer: &str) -> ScheduleSpec {
        serde_json::from_value(serde_json::json!({
            "printer": printer,
            "cron": "0 9 * * *",
            "job": { "kind": "test-page" },
        }))
        .unwrap()
    }

    #[test]
    fn an_empty_directory_is_created_with_a_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("starprint");
        let data = Data::open(&path, None).unwrap();
        assert!(data.profiles().is_empty());
        assert!(data.schedules().is_empty());
        assert_eq!(
            std::fs::read_to_string(path.join("token")).unwrap(),
            data.token()
        );
        assert!(!path.join("printers.json").exists(), "nothing to write yet");
    }

    #[test]
    fn the_token_is_kept_and_replaced_on_request() {
        let dir = tempfile::tempdir().unwrap();
        let first = Data::open(dir.path(), None).unwrap().token().to_owned();
        assert_eq!(Data::open(dir.path(), None).unwrap().token(), first);

        let given = Data::open(dir.path(), Some(" s3cret ".to_owned())).unwrap();
        assert_eq!(given.token(), "s3cret", "trimmed, as a bearer is");
        drop(given);
        assert_eq!(Data::open(dir.path(), None).unwrap().token(), "s3cret");
    }

    #[test]
    fn a_directory_is_one_servers_until_it_closes() {
        let dir = tempfile::tempdir().unwrap();
        let data = Data::open(dir.path(), None).unwrap();
        assert!(Data::open(dir.path(), None).is_err());
        drop(data);
        assert!(Data::open(dir.path(), None).is_ok());
    }

    #[test]
    fn changes_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let data = Data::open(dir.path(), None).unwrap();
        assert!(data.put_profile(profile("b")).unwrap());
        assert!(data.put_profile(profile("a")).unwrap());
        let mut changed = profile("b");
        changed.printer.port = 9101;
        assert!(!data.put_profile(changed).unwrap(), "replaced in place");
        let id = data.add_schedule(spec("a")).unwrap();
        let orphan = data.add_schedule(spec("b")).unwrap();
        assert!(data.put_schedule(&id, spec("b")).unwrap());
        assert!(!data.put_schedule("nope", spec("b")).unwrap());
        assert!(data.remove_schedule(&orphan).unwrap());
        assert!(!data.remove_schedule(&orphan).unwrap());
        assert!(data.remove_profile("a").unwrap());
        assert!(!data.remove_profile("a").unwrap());

        drop(data);
        let again = Data::open(dir.path(), None).unwrap();
        let names: Vec<String> = again.profiles().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["b"]);
        assert_eq!(again.profile("b").unwrap().printer.port, 9101);
        let schedules = again.schedules();
        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].0, id);
        assert_eq!(schedules[0].1.printer, "b", "kept without its printer");
        assert!(!dir.path().join("printers.json.tmp").exists());
    }

    #[test]
    fn a_deleted_printers_schedule_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let data = Data::open(dir.path(), None).unwrap();
        data.put_profile(profile("a")).unwrap();
        let id = data.add_schedule(spec("a")).unwrap();
        data.remove_profile("a").unwrap();

        drop(data);
        let again = Data::open(dir.path(), None).unwrap();
        assert!(again.profiles().is_empty());
        assert_eq!(again.schedules()[0].0, id);
        assert_eq!(again.schedules()[0].1.printer, "a");
    }

    /// A directory where the staging file should go makes every write
    /// fail, and the failed change must not be kept in memory either.
    #[test]
    fn a_change_that_cannot_be_written_is_not_kept() {
        let dir = tempfile::tempdir().unwrap();
        let data = Data::open(dir.path(), None).unwrap();
        data.put_profile(profile("a")).unwrap();
        let id = data.add_schedule(spec("a")).unwrap();
        std::fs::create_dir(dir.path().join("printers.json.tmp")).unwrap();
        std::fs::create_dir(dir.path().join("schedules.json.tmp")).unwrap();

        assert!(data.put_profile(profile("b")).is_err());
        assert!(data.remove_profile("a").is_err());
        assert!(data.add_schedule(spec("a")).is_err());
        assert!(data.remove_schedule(&id).is_err());
        let names: Vec<String> = data.profiles().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["a"]);
        assert_eq!(data.schedules().len(), 1);
    }

    #[test]
    fn a_file_that_will_not_read_stops_the_server() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("schedules.json"), b"{").unwrap();
        let Err(error) = Data::open(dir.path(), None) else {
            panic!("a broken file is refused")
        };
        assert!(error.contains("schedules.json"), "{error}");
    }

    /// A schedule the API would refuse is refused from the file too,
    /// naming the schedule, rather than sitting in the list unfired.
    #[test]
    fn a_schedule_the_api_would_refuse_stops_the_server() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("schedules.json"),
            br#"{ "s1": { "printer": "p", "cron": "0 9 * *", "job": { "kind": "note" } } }"#,
        )
        .unwrap();
        let Err(error) = Data::open(dir.path(), None) else {
            panic!("four fields are refused")
        };
        assert!(error.contains("s1") && error.contains("cron"), "{error}");
    }

    #[test]
    fn an_ephemeral_store_writes_nothing() {
        let data = Data::ephemeral(vec![profile("a")], "t".to_owned());
        assert!(data.put_profile(profile("b")).unwrap());
        assert!(data.add_schedule(spec("a")).is_ok());
        assert_eq!(data.profiles().len(), 2);
    }
}
