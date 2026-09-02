//! The HTTP API, embedded. The frontend starts it on the profiles it
//! has and stops it again; the server itself is the one
//! `starprint-api` runs on its own.

use starprint_api::{DEFAULT_LISTEN, Profile, Server};
use tauri::async_runtime::Mutex;

/// The running server, if any. Held across a whole start or stop so
/// two clicks in a row cannot cross.
#[derive(Default)]
pub struct Embedded(Mutex<Option<Server>>);

/// Starts the server on `printers`, replacing one already running, so
/// a changed profile takes effect by starting again. Returns the URL.
/// Loopback on the standalone server's port, so a client set up for
/// one finds the other.
#[tauri::command]
pub async fn start_api(
    printers: Vec<Profile>,
    state: tauri::State<'_, Embedded>,
) -> Result<String, String> {
    let mut slot = state.0.lock().await;
    if let Some(running) = slot.take() {
        running.shutdown().await?;
    }
    let server = Server::bind(DEFAULT_LISTEN, printers).await?;
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
