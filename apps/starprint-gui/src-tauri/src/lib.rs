//! Tauri commands over the `starprint` crate. The frontend sends a job and
//! a printer profile and gets back a layout, a preview or a result.

mod picture;
mod preview;
mod task_card;
mod test_page;
mod text;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use starprint::transport::TcpTransport;
use starprint::{Document, PrintSpeed};

use picture::{Picture, SourceCache};
use task_card::{Layout, TaskCard};
use test_page::{Section, TestPage};
use text::Text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrinterKind {
    Thermal,
    Impact,
}

impl PrinterKind {
    fn layout(self, card: &TaskCard, paper: Paper) -> Layout {
        match self {
            Self::Thermal => card.layout::<starprint::StarLine>(paper),
            Self::Impact => card.layout::<starprint::Impact>(paper),
        }
    }
}

/// Roll width of a thermal printer. The SP700's carriage is fixed, so
/// impact ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Paper {
    /// 72 mm print region, 576 dots.
    #[serde(rename = "80")]
    Mm80,
    /// 104 mm print region, 832 dots.
    #[serde(rename = "112")]
    Mm112,
}

impl Paper {
    pub fn dots(self) -> u32 {
        match self {
            Self::Mm80 => 576,
            Self::Mm112 => 832,
        }
    }

    /// Font A is 12 dots wide.
    pub fn columns(self) -> usize {
        (self.dots() / 12) as usize
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

/// A printer profile. `density`, `speed` and `paper` are thermal only.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Printer {
    pub kind: PrinterKind,
    pub host: String,
    pub port: u16,
    pub density: i8,
    pub speed: Speed,
    #[serde(default = "default_paper")]
    pub paper: Paper,
    pub cut: bool,
}

fn default_paper() -> Paper {
    Paper::Mm80
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Job {
    TaskCard(TaskCard),
    Text(Text),
    TestPage(TestPage),
    Picture(Picture),
}

impl Printer {
    /// The print mode outlives `ESC @` and the job that set it, so a
    /// double-resolution picture would otherwise leave the next job
    /// printing at half height.
    fn thermal(&self) -> starprint::Builder<starprint::StarLine> {
        starprint::starline()
            .print_mode(starprint::PrintMode::SingleColor)
            .print_density(self.density)
            .print_speed(self.speed.into())
    }

    fn document(&self, job: &Job, cache: &SourceCache) -> Result<Document, String> {
        match (job, self.kind) {
            (Job::TaskCard(card), _) if card.text.trim().is_empty() => {
                Err("The task text is empty.".to_owned())
            }
            (Job::TaskCard(card), PrinterKind::Thermal) => {
                Ok(card.document(self.thermal(), self.paper, self.cut))
            }
            (Job::TaskCard(card), PrinterKind::Impact) => {
                Ok(card.document(starprint::impact(), self.paper, self.cut))
            }
            (Job::Text(text), _) if text.text.trim().is_empty() => {
                Err("The text is empty.".to_owned())
            }
            (Job::Text(text), PrinterKind::Thermal) => {
                Ok(text.document(self.thermal(), self.paper, self.cut))
            }
            (Job::Text(text), PrinterKind::Impact) => {
                Ok(text.document(starprint::impact(), self.paper, self.cut))
            }
            (Job::TestPage(page), PrinterKind::Thermal) => Ok(test_page::thermal(
                self.thermal(),
                page,
                self.paper,
                self.cut,
            )),
            (Job::TestPage(_), PrinterKind::Impact) => {
                Ok(test_page::impact(starprint::impact(), self.cut))
            }
            (Job::Picture(picture), PrinterKind::Thermal) => {
                picture::thermal(self.thermal(), picture, self.paper, self.cut, cache)
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
fn task_card_layout(card: TaskCard, kind: PrinterKind, paper: Paper) -> Layout {
    kind.layout(&card, paper)
}

#[tauri::command]
fn text_layout(text: Text, kind: PrinterKind, paper: Paper) -> text::Layout {
    match kind {
        PrinterKind::Thermal => text.layout::<starprint::StarLine>(paper),
        PrinterKind::Impact => text.layout::<starprint::Impact>(paper),
    }
}

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
    // Image preparation is CPU-bound and the transport sleeps between
    // chunks; neither belongs on the async runtime.
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HexDump {
    pub bytes: usize,
    /// 16 bytes per row: offset, hex, ASCII.
    pub dump: String,
}

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

/// PNG bytes; the frontend shows them through a blob URL.
#[tauri::command]
async fn picture_preview(
    picture: Picture,
    kind: PrinterKind,
    paper: Paper,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<tauri::ipc::Response, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || picture.preview_png(kind, paper, &cache))
        .await
        .map_err(|e| e.to_string())?
        .map(tauri::ipc::Response::new)
}

/// Matches the title bar to the dark toolbar (shadcn's dark
/// `--background`, `oklch(0.145 0 0)` ≈ `#0a0a0a`).
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
        // SAFETY: live window handle; the attribute takes a COLORREF.
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
            text_layout,
            test_page_sections,
            print_job,
            job_hexdump,
            picture_preview,
            probe_printer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thermal() -> Printer {
        Printer {
            kind: PrinterKind::Thermal,
            host: "printer.invalid".to_owned(),
            port: 9100,
            density: 3,
            speed: Speed::Slow,
            paper: Paper::Mm80,
            cut: true,
        }
    }

    #[test]
    fn thermal_jobs_select_the_print_mode_before_printing() {
        let printer = thermal();
        let cache = SourceCache::default();
        let jobs = [
            Job::TaskCard(TaskCard {
                text: "Task".to_owned(),
                priority: false,
                due: None,
            }),
            Job::Text(Text {
                text: "Text".to_owned(),
                bold: false,
                wide: false,
                tall: false,
                accent: false,
            }),
            Job::TestPage(TestPage {
                double_resolution: false,
            }),
        ];
        for job in jobs {
            let document = printer.document(&job, &cache).expect("builds");
            let bytes = document.as_bytes();
            let first = bytes
                .windows(4)
                .find(|w| w[..3] == [0x1b, 0x1e, b'C'])
                .expect("selects a print mode");
            assert_eq!(first[3], 0, "starts in single colour: {job:?}");
        }
    }
}
