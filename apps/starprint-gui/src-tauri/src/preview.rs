//! Layout and image preview commands, drawn by `starprint-workflows`.

use std::sync::Arc;

use starprint::{Impact, StarLine};
use starprint_workflows::{Note, Paper, Png, PrinterKind, Qr, TaskCard, TestPage, Text};
use starprint_workflows::{note, qr, task_card, test_page, text};

use crate::picture::{PicturePath, SourceCache};

#[tauri::command]
pub fn task_card_layout(card: TaskCard, kind: PrinterKind, paper: Paper) -> task_card::Layout {
    match kind {
        PrinterKind::Thermal => card.layout::<StarLine>(paper),
        PrinterKind::Impact => card.layout::<Impact>(paper),
    }
}

#[tauri::command]
pub fn text_layout(text: Text, kind: PrinterKind, paper: Paper) -> text::Layout {
    match kind {
        PrinterKind::Thermal => text.layout::<StarLine>(paper),
        PrinterKind::Impact => text.layout::<Impact>(paper),
    }
}

#[tauri::command]
pub fn note_layout(note: Note, kind: PrinterKind, paper: Paper) -> note::Layout {
    match kind {
        PrinterKind::Thermal => note.layout::<StarLine>(paper),
        PrinterKind::Impact => note.layout::<Impact>(paper),
    }
}

#[tauri::command]
pub async fn note_preview(
    note: Note,
    kind: PrinterKind,
    paper: Paper,
) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bitmap = match kind {
            PrinterKind::Thermal => note.bitmap::<StarLine>(paper),
            PrinterKind::Impact => note.bitmap::<Impact>(paper),
        };
        Png::from_bitmap(&bitmap, kind).map(Png::into_bytes)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(tauri::ipc::Response::new)
}

/// Returns an error when the data is too long to encode.
#[tauri::command]
pub fn qr_layout(code: Qr, kind: PrinterKind, paper: Paper) -> Result<qr::Layout, String> {
    match kind {
        PrinterKind::Thermal => code.layout::<StarLine>(paper),
        PrinterKind::Impact => code.layout::<Impact>(paper),
    }
}

/// Renders the printable symbol as a PNG, with thermal dot gain.
/// Runs off the async runtime to keep sliders responsive.
#[tauri::command]
pub async fn qr_preview(
    code: Qr,
    kind: PrinterKind,
    paper: Paper,
) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bitmap = match kind {
            PrinterKind::Thermal => code.bitmap::<StarLine>(paper),
            PrinterKind::Impact => code.bitmap::<Impact>(paper),
        }?;
        Png::from_bitmap(&bitmap, kind).map(Png::into_bytes)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(tauri::ipc::Response::new)
}

#[tauri::command]
pub fn test_page_sections(page: TestPage, kind: PrinterKind) -> Vec<test_page::Section> {
    match kind {
        PrinterKind::Thermal => test_page::thermal_sections(&page),
        PrinterKind::Impact => test_page::impact_sections(),
    }
}

/// PNG bytes; the frontend shows them through a blob URL.
#[tauri::command]
pub async fn picture_preview(
    picture: PicturePath,
    kind: PrinterKind,
    paper: Paper,
    cache: tauri::State<'_, Arc<SourceCache>>,
) -> Result<tauri::ipc::Response, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || {
        let source = picture.source(kind, paper, &cache)?;
        let prepared = picture.picture.preview(kind, paper, &source)?;
        Png::from_grayscale(&prepared, kind, picture.picture.double).map(Png::into_bytes)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(tauri::ipc::Response::new)
}
