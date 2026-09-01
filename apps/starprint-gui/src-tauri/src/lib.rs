//! The commands the frontend calls. Each one reads a picture if the job
//! names one, hands the rest to `starprint-workflows` and returns what
//! came back.

mod hexdump;
mod job;
mod picture;
mod preview;
mod window;

use std::sync::Arc;

use serde::Serialize;
use starprint::graphics::{Bitmap, Grayscale};
use starprint::transport::TcpTransport;
use starprint::{Impact, StarLine};
use starprint_workflows::{Note, Paper, Printer, PrinterKind, Qr, TaskCard, TestPage, Text};
use starprint_workflows::{note, qr, task_card, test_page, text};

use hexdump::HexDump;
use job::JobRequest;
use picture::{PicturePath, SourceCache};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintReport {
    pub bytes: usize,
}

// The previews are the app's alone, so the choice of head is made here
// rather than in the shared crate.
#[tauri::command]
fn task_card_layout(card: TaskCard, kind: PrinterKind, paper: Paper) -> task_card::Layout {
    match kind {
        PrinterKind::Thermal => card.layout::<StarLine>(paper),
        PrinterKind::Impact => card.layout::<Impact>(paper),
    }
}

#[tauri::command]
fn text_layout(text: Text, kind: PrinterKind, paper: Paper) -> text::Layout {
    match kind {
        PrinterKind::Thermal => text.layout::<StarLine>(paper),
        PrinterKind::Impact => text.layout::<Impact>(paper),
    }
}

#[tauri::command]
fn note_layout(note: Note, kind: PrinterKind, paper: Paper) -> note::Layout {
    match kind {
        PrinterKind::Thermal => note.layout::<StarLine>(paper),
        PrinterKind::Impact => note.layout::<Impact>(paper),
    }
}

/// Fails on data too long to encode, which the preview reports rather
/// than leaving the last good symbol on screen.
#[tauri::command]
fn qr_layout(code: Qr, kind: PrinterKind, paper: Paper) -> Result<qr::Layout, String> {
    match kind {
        PrinterKind::Thermal => code.layout::<StarLine>(paper),
        PrinterKind::Impact => code.layout::<Impact>(paper),
    }
}

/// PNG bytes of the symbol as it will print, dot for dot, so the
/// preview needs no rule of its own for how a corner is rounded. On a
/// thermal head the dots are bloomed as [`picture_preview`]'s are, and
/// like it this runs off the main thread, so a slider does not stall.
#[tauri::command]
async fn qr_preview(
    code: Qr,
    kind: PrinterKind,
    paper: Paper,
) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bitmap = match kind {
            PrinterKind::Thermal => code.bitmap::<StarLine>(paper),
            PrinterKind::Impact => code.bitmap::<Impact>(paper),
        }?;
        let drawn = grayscale(&bitmap);
        let shown = match kind {
            PrinterKind::Thermal => preview::dot_gain(&drawn, 150),
            PrinterKind::Impact => drawn,
        };
        preview::png(&shown)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(tauri::ipc::Response::new)
}

/// Ink as black on white.
fn grayscale(bitmap: &Bitmap) -> Grayscale {
    let (width, height) = (bitmap.width(), bitmap.height());
    let pixels = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| if bitmap.get(x, y) { 0 } else { 255 })
        .collect();
    Grayscale::new(width, height, pixels).expect("sized from the bitmap")
}

#[tauri::command]
fn test_page_sections(page: TestPage, kind: PrinterKind) -> Vec<test_page::Section> {
    match kind {
        PrinterKind::Thermal => test_page::thermal_sections(&page),
        PrinterKind::Impact => test_page::impact_sections(),
    }
}

#[tauri::command]
async fn print_job(
    job: JobRequest,
    printer: Printer,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<PrintReport, String> {
    let cache = Arc::clone(&cache);
    // Image preparation is CPU-bound and the transport sleeps between
    // chunks; neither belongs on the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        let (job, image) = job.resolve(printer.head.kind(), printer.head.paper(), &cache)?;
        let document = printer.document(&job, image.as_ref())?;
        let mut transport = TcpTransport::connect(&printer.address()).map_err(|e| e.to_string())?;
        transport.print(&document).map_err(|e| e.to_string())?;
        Ok(PrintReport {
            bytes: document.as_bytes().len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn job_hexdump(
    job: JobRequest,
    printer: Printer,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<HexDump, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || {
        let (job, image) = job.resolve(printer.head.kind(), printer.head.paper(), &cache)?;
        Ok(hexdump::of(
            printer.document(&job, image.as_ref())?.as_bytes(),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// PNG bytes; the frontend shows them through a blob URL.
///
/// A thermal head blooms each dot wider than its pitch, so the preview
/// widens them to match. See [`preview::dot_gain`].
#[tauri::command]
async fn picture_preview(
    picture: PicturePath,
    kind: PrinterKind,
    paper: Paper,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<tauri::ipc::Response, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || {
        let source = picture.source(kind, paper, &cache)?;
        let prepared = picture.picture.prepare(kind, paper, &source)?.preview;
        let shown = match kind {
            PrinterKind::Thermal => {
                preview::dot_gain(&prepared, picture.picture.thermal_dot_size())
            }
            PrinterKind::Impact => prepared,
        };
        preview::png(&shown)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(tauri::ipc::Response::new)
}

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
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            window::colour_title_bar(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            task_card_layout,
            text_layout,
            note_layout,
            qr_layout,
            qr_preview,
            test_page_sections,
            print_job,
            job_hexdump,
            picture_preview,
            probe_printer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
