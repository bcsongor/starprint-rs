//! Desktop setup. The app is a web client of the API server it runs;
//! this side opens the window, starts that server and lists the
//! addresses it may listen on.

mod api;
mod window;

pub fn run() {
    tauri::Builder::default()
        .manage(api::Embedded::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            window::colour_title_bar(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::start_server,
            api::list_addresses
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
