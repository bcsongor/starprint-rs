//! Desktop setup and printer connectivity.

mod api;
mod hexdump;
mod job;
mod picture;
mod preview;
mod window;

use std::sync::Arc;

use picture::SourceCache;

/// Connects and drops without writing, so it cannot disturb a job.
#[tauri::command]
async fn probe_printer(host: String, port: u16) -> bool {
    // Only bounds how long a missing printer takes to show as offline.
    const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(400);

    let host = host.trim().to_owned();
    if host.is_empty() {
        return false;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), port)) else {
            return false;
        };
        addrs
            .into_iter()
            .any(|addr| std::net::TcpStream::connect_timeout(&addr, TIMEOUT).is_ok())
    })
    .await
    .unwrap_or(false)
}

pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(SourceCache::default()))
        .manage(api::Embedded::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            window::colour_title_bar(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            preview::task_card_layout,
            preview::text_layout,
            preview::note_layout,
            preview::note_preview,
            preview::qr_layout,
            preview::qr_preview,
            preview::test_page_sections,
            job::print_job,
            job::job_hexdump,
            preview::picture_preview,
            probe_printer,
            api::start_api,
            api::stop_api
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
