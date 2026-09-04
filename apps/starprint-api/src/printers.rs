//! The printers the profile file named, and getting bytes to one of
//! them. Nothing here cares what the bytes mean.

use axum::body::Bytes;
use axum::http::StatusCode;
use starprint::transport::{TcpTransport, Transport};
use tokio::sync::Mutex;

use crate::config::Profile;
use crate::problem::Problem;

pub struct Printers {
    entries: Vec<Entry>,
}

/// One printer from the profile file.
pub struct Entry {
    profile: Profile,
    turn: Mutex<()>,
}

impl Printers {
    pub fn new(profiles: Vec<Profile>) -> Self {
        Self {
            entries: profiles
                .into_iter()
                .map(|profile| Entry {
                    profile,
                    turn: Mutex::new(()),
                })
                .collect(),
        }
    }

    /// In the order the file listed them.
    pub fn profiles(&self) -> impl Iterator<Item = &Profile> {
        self.entries.iter().map(|entry| &entry.profile)
    }

    pub fn find(&self, name: &str) -> Result<&Entry, Problem> {
        self.entries
            .iter()
            .find(|entry| entry.profile.name == name)
            .ok_or_else(|| Problem::not_found(format!("No printer named `{name}`.")))
    }
}

impl Entry {
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Writes `payload` and reports how much went out.
    ///
    /// A completed write is all this can promise. Without automatic
    /// status back there is no way to ask whether the printer accepted
    /// the job, let alone printed it.
    pub async fn send(&self, payload: Bytes) -> Result<usize, Problem> {
        let address = self.profile.printer.address();
        let sent = payload.len();
        let _turn = self.turn.lock().await;
        // The transport sleeps between chunks to pace the Ethernet card,
        // so it cannot run on the async runtime.
        tokio::task::spawn_blocking(move || {
            let mut transport = TcpTransport::connect(&address)?;
            transport.send(&payload)
        })
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("The job could not be written: {e}."),
            )
        })?
        .map_err(|e: starprint::Error| {
            Problem::bad_gateway(format!(
                "The printer could not be written to: {e}. It may have received none, some or \
                 all of the job."
            ))
        })?;
        Ok(sent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starprint_workflows::{Paper, Printer, Speed};

    fn printers() -> Printers {
        Printers::new(vec![
            Profile {
                name: "tsp800ii".to_owned(),
                printer: Printer::thermal(
                    "127.0.0.1".to_owned(),
                    9100,
                    Paper::Mm80,
                    3,
                    Speed::Slow,
                ),
            },
            Profile {
                name: "sp743".to_owned(),
                printer: Printer::impact("127.0.0.1".to_owned(), 9100),
            },
        ])
    }

    #[test]
    fn a_printer_is_found_by_name_and_only_by_name() {
        let printers = printers();
        assert_eq!(printers.find("sp743").unwrap().profile().name, "sp743");
        let Err(problem) = printers.find("SP743") else {
            panic!("names are matched exactly")
        };
        let json = serde_json::to_value(&problem).unwrap();
        assert_eq!(json["status"], 404);
        assert_eq!(json["detail"], "No printer named `SP743`.");
    }
}
