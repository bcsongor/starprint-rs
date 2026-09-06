//! Printer lookup and serialised writes to each network endpoint.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::http::StatusCode;
use starprint::transport::{TcpTransport, Transport};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::config::Profile;
use crate::problem::Problem;

/// Shared by all printer connections in one process.
#[derive(Default)]
pub struct PrintQueue {
    endpoints: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl PrintQueue {
    /// Waits for exclusive access to this host and port. Hostname aliases
    /// are separate endpoints. Move the guard into the blocking task so
    /// cancelling its caller cannot release a connection still in use.
    pub async fn lock(&self, host: &str, port: u16) -> OwnedMutexGuard<()> {
        let host = host.trim().trim_matches(['[', ']']);
        let host = host
            .parse::<IpAddr>()
            .map_or_else(|_| host.to_ascii_lowercase(), |ip| ip.to_string());
        let turn = self
            .endpoints
            .lock()
            .await
            .entry(format!("{host}:{port}"))
            .or_default()
            .clone();
        turn.lock_owned().await
    }
}

pub struct Printers {
    entries: Vec<Entry>,
}

/// One printer from the profile file.
pub struct Entry {
    profile: Profile,
    queue: Arc<PrintQueue>,
}

impl Printers {
    pub fn new(profiles: Vec<Profile>, queue: Arc<PrintQueue>) -> Self {
        Self {
            entries: profiles
                .into_iter()
                .map(|profile| Entry {
                    profile,
                    queue: Arc::clone(&queue),
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
        let turn = self
            .queue
            .lock(&self.profile.printer.host, self.profile.printer.port)
            .await;
        // The transport sleeps between chunks to pace the Ethernet card,
        // so it cannot run on the async runtime.
        tokio::task::spawn_blocking(move || {
            let _turn = turn;
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
    use starprint::transport::Pacing;
    use starprint_workflows::{Paper, Printer, Speed};
    use std::future::Future as _;
    use std::pin::pin;
    use std::task::{Context, Waker};
    use std::time::{Duration, Instant};
    use tokio::io::AsyncReadExt as _;
    use tokio::net::TcpListener;
    use tokio::time::timeout;

    fn printers() -> Printers {
        Printers::new(
            vec![
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
            ],
            Arc::default(),
        )
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

    #[tokio::test]
    async fn profiles_share_the_endpoint_queue_with_other_clients() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let queue = Arc::new(PrintQueue::default());
        let turn = queue.lock("127.0.0.1", port).await;
        let printers = Printers::new(
            ["first", "second"]
                .map(|name| Profile {
                    name: name.to_owned(),
                    printer: Printer::impact("127.0.0.1".to_owned(), port),
                })
                .into(),
            queue,
        );
        let mut first = pin!(
            printers
                .find("first")
                .unwrap()
                .send(Bytes::from_static(b"first"))
        );
        let mut second = pin!(
            printers
                .find("second")
                .unwrap()
                .send(Bytes::from_static(b"second"))
        );
        let mut cx = Context::from_waker(Waker::noop());
        assert!(first.as_mut().poll(&mut cx).is_pending());
        assert!(second.as_mut().poll(&mut cx).is_pending());
        // A send must wait before opening its connection.
        assert!(
            timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );

        let received = tokio::spawn(async move {
            let mut jobs = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).await.unwrap();
                jobs.push(bytes);
            }
            jobs
        });
        drop(turn);
        let (first, second) = tokio::join!(first, second);
        assert_eq!(first.unwrap(), 5);
        assert_eq!(second.unwrap(), 6);
        assert_eq!(
            received.await.unwrap(),
            [b"first".to_vec(), b"second".to_vec()]
        );
    }

    #[tokio::test]
    async fn equivalent_hosts_share_a_queue_but_other_endpoints_do_not() {
        let queue = PrintQueue::default();
        let mut cx = Context::from_waker(Waker::noop());
        for (host, equivalent) in [
            (" PRINTER.local ", "printer.LOCAL"),
            ("[::1]", "0:0:0:0:0:0:0:1"),
        ] {
            let turn = queue.lock(host, 9100).await;
            let mut waiting = pin!(queue.lock(equivalent, 9100));
            assert!(waiting.as_mut().poll(&mut cx).is_pending());
            assert!(
                pin!(queue.lock(host, 9101))
                    .as_mut()
                    .poll(&mut cx)
                    .is_ready()
            );
            assert!(
                pin!(queue.lock("another.local", 9100))
                    .as_mut()
                    .poll(&mut cx)
                    .is_ready()
            );
            drop(turn);
            assert!(waiting.as_mut().poll(&mut cx).is_ready());
        }
    }

    #[tokio::test]
    async fn cancelling_a_sender_keeps_its_write_in_the_queue() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let queue = Arc::new(PrintQueue::default());
        let entry = Entry {
            profile: Profile {
                name: "printer".to_owned(),
                printer: Printer::impact("127.0.0.1".to_owned(), port),
            },
            queue: Arc::clone(&queue),
        };
        let pacing = Pacing::STAR_ETHERNET;
        let payload = Bytes::from(vec![b'x'; pacing.chunk_size * 10]);
        let expected = payload.clone();
        let started = Instant::now();
        let sender = tokio::spawn(async move { entry.send(payload).await });
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut first = [0];
        stream.read_exact(&mut first).await.unwrap();
        sender.abort();
        let _ = sender.await;

        let _turn = queue.lock("127.0.0.1", port).await;
        // All nine pacing delays must finish before another client gets in.
        assert!(started.elapsed() >= pacing.delay * 9);
        let mut received = first.to_vec();
        stream.read_to_end(&mut received).await.unwrap();
        assert_eq!(received, expected);
    }
}
