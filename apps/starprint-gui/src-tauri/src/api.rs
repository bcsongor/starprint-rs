//! The HTTP API, embedded. The frontend starts it on the profiles it
//! has and stops it again; the server itself is the one
//! `starprint-api` runs on its own.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::Serialize;
use starprint_api::{Profile, Server};
use tauri::async_runtime::Mutex;

/// The running server, if any. Held across a whole start or stop so
/// two clicks in a row cannot cross.
#[derive(Default)]
pub struct Embedded(Mutex<Option<Server>>);

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

/// Starts the server on `printers` behind `token`, at `ip` and `port`,
/// replacing one already running, so a changed profile or address
/// takes effect by starting again. Returns the URL.
#[tauri::command]
pub async fn start_api(
    printers: Vec<Profile>,
    token: String,
    ip: IpAddr,
    port: u16,
    state: tauri::State<'_, Embedded>,
) -> Result<String, String> {
    let mut slot = state.0.lock().await;
    if let Some(running) = slot.take() {
        running.shutdown().await?;
    }
    let server = Server::bind(SocketAddr::new(ip, port), printers, token).await?;
    let url = format!("http://{}", server.local_addr());
    *slot = Some(server);
    Ok(url)
}

/// Lets requests in flight finish. Nothing to stop is not an error.
#[tauri::command]
pub async fn stop_api(state: tauri::State<'_, Embedded>) -> Result<(), String> {
    let mut slot = state.0.lock().await;
    match slot.take() {
        Some(running) => running.shutdown().await,
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_comes_first_and_link_local_is_left_out() {
        let addresses = list_addresses();
        assert_eq!(addresses.first().map(|a| a.ip), Some(Ipv4Addr::LOCALHOST));
        assert!(addresses.iter().all(|a| !a.ip.is_link_local()));
    }
}
