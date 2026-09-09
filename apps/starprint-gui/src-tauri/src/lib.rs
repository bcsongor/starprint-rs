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
    Ok(starprint_api::reachable(&host, port, &queue).await)
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
            scheduler::next_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
