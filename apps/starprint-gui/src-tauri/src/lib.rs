//! Tauri side of the Starprint desktop app: thin commands over the
//! `starprint` crate. The frontend never sees printer bytes; it sends a
//! form payload and a printer profile and gets back a layout or a result.

mod task_card;

use serde::{Deserialize, Serialize};
use starprint::transport::TcpTransport;
use starprint::{Document, PrintSpeed};

use task_card::{CardStyle, Layout, TaskCard};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrinterKind {
    /// Star Line Mode thermal printer (TSP650II, TSP700II, TSP800II, …).
    Thermal,
    /// SP700 series dot-impact printer, red/black ribbon.
    Impact,
}

impl PrinterKind {
    fn columns(self) -> usize {
        match self {
            Self::Thermal => <starprint::Builder<starprint::StarLine> as CardStyle>::COLUMNS,
            Self::Impact => <starprint::Builder<starprint::Impact> as CardStyle>::COLUMNS,
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
    /// Thermal only: slow print speed for the best text quality.
    pub slow: bool,
}

impl Printer {
    fn task_card(&self, card: &TaskCard) -> Document {
        match self.kind {
            PrinterKind::Thermal => {
                let mut builder = starprint::starline().print_density(self.density);
                if self.slow {
                    builder = builder.print_speed(PrintSpeed::Slow);
                }
                card.document(builder)
            }
            PrinterKind::Impact => card.document(starprint::impact()),
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
    card.layout(kind.columns())
}

#[tauri::command]
async fn print_task_card(card: TaskCard, printer: Printer) -> Result<PrintReport, String> {
    if card.text.trim().is_empty() {
        return Err("The task text is empty.".to_owned());
    }
    let document = printer.task_card(&card);
    let address = format!("{}:{}", printer.host.trim(), printer.port);
    // The transport paces its writes with sleeps, so keep it off the
    // async runtime's threads.
    tauri::async_runtime::spawn_blocking(move || {
        let mut transport = TcpTransport::connect(&address).map_err(|e| e.to_string())?;
        transport.print(&document).map_err(|e| e.to_string())?;
        Ok(PrintReport {
            bytes: document.as_bytes().len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The bytes the job would send, as a hex dump, for checking without a
/// printer.
#[tauri::command]
fn task_card_hexdump(card: TaskCard, printer: Printer) -> String {
    let document = printer.task_card(&card);
    document
        .as_bytes()
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
        .join("\n")
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            task_card_layout,
            print_task_card,
            task_card_hexdump
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
