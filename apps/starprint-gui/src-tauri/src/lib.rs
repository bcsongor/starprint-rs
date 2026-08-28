//! Tauri side of the Starprint desktop app: thin commands over the
//! `starprint` crate. The frontend never sees printer bytes; it sends a
//! form payload and a printer profile and gets back a layout or a result.

mod picture;
mod preview;
mod task_card;
mod test_page;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use starprint::transport::TcpTransport;
use starprint::{Document, PrintSpeed};

use picture::{Picture, SourceCache};
use task_card::{Layout, TaskCard};
use test_page::{Section, TestPage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrinterKind {
    /// Star Line Mode thermal printer (TSP650II, TSP700II, TSP800II, …).
    Thermal,
    /// SP700 series dot-impact printer, red/black ribbon.
    Impact,
}

impl PrinterKind {
    fn layout(self, card: &TaskCard) -> Layout {
        match self {
            Self::Thermal => card.layout::<starprint::StarLine>(),
            Self::Impact => card.layout::<starprint::Impact>(),
        }
    }
}

/// Mirrors [`PrintSpeed`], which has no serde support of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Speed {
    High,
    Medium,
    Slow,
}

impl From<Speed> for PrintSpeed {
    fn from(speed: Speed) -> Self {
        match speed {
            Speed::High => Self::High,
            Speed::Medium => Self::Medium,
            Speed::Slow => Self::Slow,
        }
    }
}

/// Where and how to print. Persisted by the frontend via the store plugin.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Printer {
    pub kind: PrinterKind,
    pub host: String,
    pub port: u16,
    /// Thermal only: print density, -3..=3.
    pub density: i8,
    /// Thermal only: slow gives the best text quality.
    pub speed: Speed,
    /// Cut the paper after the card.
    pub cut: bool,
}

/// What to print: one variant per workflow the app offers.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Job {
    TaskCard(TaskCard),
    TestPage(TestPage),
    Picture(Picture),
}

impl Printer {
    fn thermal(&self) -> starprint::Builder<starprint::StarLine> {
        starprint::starline()
            .print_density(self.density)
            .print_speed(self.speed.into())
    }

    /// Builds the job; fails when its input is missing or unreadable.
    fn document(&self, job: &Job, cache: &SourceCache) -> Result<Document, String> {
        match (job, self.kind) {
            (Job::TaskCard(card), _) if card.text.trim().is_empty() => {
                Err("The task text is empty.".to_owned())
            }
            (Job::TaskCard(card), PrinterKind::Thermal) => {
                Ok(card.document(self.thermal(), self.cut))
            }
            (Job::TaskCard(card), PrinterKind::Impact) => {
                Ok(card.document(starprint::impact(), self.cut))
            }
            (Job::TestPage(page), PrinterKind::Thermal) => {
                Ok(test_page::thermal(self.thermal(), page, self.cut))
            }
            (Job::TestPage(_), PrinterKind::Impact) => {
                Ok(test_page::impact(starprint::impact(), self.cut))
            }
            (Job::Picture(picture), PrinterKind::Thermal) => {
                picture::thermal(self.thermal(), picture, self.cut, cache)
            }
            (Job::Picture(picture), PrinterKind::Impact) => {
                picture::impact(starprint::impact(), picture, self.cut, cache)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintReport {
    pub bytes: usize,
}

#[tauri::command]
fn task_card_layout(card: TaskCard, kind: PrinterKind) -> Layout {
    kind.layout(&card)
}

/// The numbered sections of the test page for this printer, for the
/// preview.
#[tauri::command]
fn test_page_sections(page: TestPage, kind: PrinterKind) -> Vec<Section> {
    match kind {
        PrinterKind::Thermal => test_page::thermal_sections(&page),
        PrinterKind::Impact => test_page::impact_sections(),
    }
}

#[tauri::command]
async fn print_job(
    job: Job,
    printer: Printer,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<PrintReport, String> {
    let address = format!("{}:{}", printer.host.trim(), printer.port);
    let cache = Arc::clone(&cache);
    // Image preparation is CPU-bound and the transport paces its writes
    // with sleeps, so keep both off the async runtime's threads.
    tauri::async_runtime::spawn_blocking(move || {
        let document = printer.document(&job, &cache)?;
        let mut transport = TcpTransport::connect(&address).map_err(|e| e.to_string())?;
        transport.print(&document).map_err(|e| e.to_string())?;
        Ok(PrintReport {
            bytes: document.as_bytes().len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether the printer answers on its port.
///
/// The probe opens a connection and drops it without writing, so it
/// cannot disturb a job: Star's Ethernet cards accept one job at a time
/// and discard anything sent while the printer is busy.
#[tauri::command]
async fn probe_printer(host: String, port: u16) -> bool {
    // Printers on the LAN answer in single-digit milliseconds; this only
    // bounds how long a missing one takes to show as offline.
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HexDump {
    pub bytes: usize,
    /// 16 bytes per row: offset, hex, ASCII.
    pub dump: String,
}

/// The bytes the job would send, as a hex dump, for checking without a
/// printer.
#[tauri::command]
async fn job_hexdump(
    job: Job,
    printer: Printer,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<HexDump, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || {
        let document = printer.document(&job, &cache)?;
        Ok(hexdump(document.as_bytes()))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn hexdump(bytes: &[u8]) -> HexDump {
    let dump = bytes
        .chunks(16)
        .enumerate()
        .map(|(row, chunk)| {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            let ascii: String = chunk
                .iter()
                .map(|&b| {
                    if (0x20..0x7f).contains(&b) {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!("{:04x}  {:<47}  {ascii}", row * 16, hex.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n");
    HexDump {
        bytes: bytes.len(),
        dump,
    }
}

/// The dithered picture as it will print, encoded as a PNG. Returned as
/// raw bytes, which the frontend shows through a blob URL.
#[tauri::command]
async fn picture_preview(
    picture: Picture,
    kind: PrinterKind,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<tauri::ipc::Response, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || picture.preview_png(kind, &cache))
        .await
        .map_err(|e| e.to_string())?
        .map(tauri::ipc::Response::new)
}

/// Colours the Windows title bar like the app's dark toolbar
/// (shadcn's dark `--background`, `oklch(0.145 0 0)` ≈ `#0a0a0a`), so
/// frame and toolbar read as one surface. Other platforms draw their
/// own frames.
#[cfg(windows)]
fn colour_title_bar(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };

    // COLORREF is 0x00BBGGRR.
    const BACKGROUND: u32 = 0x000a0a0a;
    const FOREGROUND: u32 = 0x00fafafa;

    let Ok(hwnd) = window.hwnd() else { return };
    for (attribute, colour) in [
        (DWMWA_CAPTION_COLOR, BACKGROUND),
        (DWMWA_BORDER_COLOR, BACKGROUND),
        (DWMWA_TEXT_COLOR, FOREGROUND),
    ] {
        // SAFETY: the handle comes from the live window; the attribute
        // takes a COLORREF-sized value.
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0 as _,
                attribute as u32,
                (&raw const colour).cast(),
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(SourceCache::default()))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(windows)]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    colour_title_bar(&window);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            task_card_layout,
            test_page_sections,
            print_job,
            job_hexdump,
            picture_preview,
            probe_printer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
