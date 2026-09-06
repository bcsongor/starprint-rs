//! Desktop setup and printer connectivity.

mod api;
mod hexdump;
mod job;
mod picture;
mod preview;
mod scheduler;
mod window;

use std::sync::Arc;

use picture::SourceCache;
use starprint_api::PrintQueue;

/// Checks connectivity between jobs without writing to the printer.
#[tauri::command]
async fn probe_printer(
    host: String,
    port: u16,
    queue: tauri::State<'_, Arc<PrintQueue>>,
) -> Result<bool, ()> {
    // Only bounds how long a missing printer takes to show as offline.
    const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(400);

    let host = host.trim().to_owned();
    if host.is_empty() {
        return Ok(false);
    }
    let turn = queue.lock(&host, port).await;
    tauri::async_runtime::spawn_blocking(move || {
        let _turn = turn;
        let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), port)) else {
            return false;
        };
        addrs
            .into_iter()
            .any(|addr| std::net::TcpStream::connect_timeout(&addr, TIMEOUT).is_ok())
    })
    .await
    .map_err(|_| ())
}

pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(SourceCache::default()))
        .manage(Arc::new(PrintQueue::default()))
        .manage(api::Embedded::default())
        .manage(scheduler::Scheduler::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            window::colour_title_bar(app);
            scheduler::spawn(app.handle().clone());
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
            api::list_addresses,
            api::start_api,
            api::stop_api,
            scheduler::set_schedules,
            scheduler::print_scheduled,
            scheduler::next_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
