//! Runs `starprint-api` inside the app, on the same data directory the
//! command line uses.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use starprint_api::{Data, PrintQueue, Server};
use tauri::async_runtime::Mutex;

/// The data directory and the server on it. Held across a whole start
/// so two in a row cannot cross.
#[derive(Default)]
pub struct Embedded(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    /// Opened on the first start and kept, so the directory stays this
    /// app's for as long as it runs.
    data: Option<Arc<Data>>,
    server: Option<Server>,
    /// Shared across restarts, so a job in progress keeps its turn.
    queue: Arc<PrintQueue>,
}

impl Inner {
    async fn start(&mut self, dir: &Path, listen: SocketAddr) -> Result<Started, String> {
        let data = match &self.data {
            Some(data) => Arc::clone(data),
            None => {
                let data = Arc::new(Data::open(dir, None)?);
                self.data = Some(Arc::clone(&data));
                data
            }
        };
        if let Some(running) = self.server.take() {
            running.shutdown().await?;
        }
        let server = Server::bind(listen, Arc::clone(&data), Arc::clone(&self.queue)).await?;
        let started = Started {
            url: format!("http://{}", server.local_addr()),
            token: data.token().to_owned(),
        };
        self.server = Some(server);
        Ok(started)
    }
}

/// What the frontend needs to talk to the server.
#[derive(Serialize)]
pub struct Started {
    pub url: String,
    pub token: String,
}

/// Starts the server at `ip` and `port`, replacing one already running,
/// so a changed address takes effect by starting again. The data
/// directory is opened on the first call; if another server holds it,
/// that is the error, and the next call tries again.
#[tauri::command]
pub async fn start_server(
    ip: IpAddr,
    port: u16,
    state: tauri::State<'_, Embedded>,
) -> Result<Started, String> {
    let dir = Data::default_dir()?;
    state
        .0
        .lock()
        .await
        .start(&dir, SocketAddr::new(ip, port))
        .await
}

/// An IPv4 address of this machine and the adapter it belongs to.
#[derive(Serialize)]
pub struct Address {
    pub ip: Ipv4Addr,
    pub name: String,
}

/// Loopback first, then each adapter's IPv4 address. Link-local
/// addresses are left out.
#[tauri::command]
pub fn list_addresses() -> Vec<Address> {
    let mut addresses: Vec<Address> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(ip) if !ip.is_link_local() => Some(Address {
                ip,
                name: interface.name,
            }),
            _ => None,
        })
        .collect();
    addresses.sort_by_key(|a| (!a.ip.is_loopback(), a.ip));
    addresses.dedup_by_key(|a| a.ip);
    addresses
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_port() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    }

    /// A restart keeps the directory and its token, and another server
    /// cannot take the directory while the app has it.
    #[tokio::test]
    async fn restarting_keeps_the_directory_open() {
        let dir = tempfile::tempdir().unwrap();
        let mut embedded = Inner::default();
        let first = embedded.start(dir.path(), any_port()).await.unwrap();
        let second = embedded.start(dir.path(), any_port()).await.unwrap();
        assert_eq!(first.token, second.token);
        assert_ne!(first.url, second.url, "a new port each time here");
        assert!(
            Data::open(dir.path(), None).is_err(),
            "the directory is still the app's"
        );
        assert!(first.url.starts_with("http://127.0.0.1:"));
    }
}
