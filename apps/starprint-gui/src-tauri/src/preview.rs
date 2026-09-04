//! Layout and image preview commands, thermal dot gain and PNG encoding.

use std::io::Cursor;
use std::sync::Arc;

use starprint::graphics::{Bitmap, Grayscale};
use starprint::{Impact, StarLine};
use starprint_workflows::{Note, Paper, PrinterKind, Qr, TaskCard, TestPage, Text};
use starprint_workflows::{note, qr, task_card, test_page, text};

use crate::picture::{PicturePath, SourceCache};

/// Dot diameters as percentages of pitch, measured on a TSP700II at
/// slow speed and density +3. Double resolution halves the row pitch.
const THERMAL_DOT_SIZE: u32 = 150;
const THERMAL_DOUBLE_DOT_SIZE: u32 = 200;

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
        let drawn = grayscale(&bitmap);
        let shown = match kind {
            PrinterKind::Thermal => dot_gain(&drawn, THERMAL_DOT_SIZE),
            PrinterKind::Impact => drawn,
        };
        png(&shown)
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
pub fn test_page_sections(page: TestPage, kind: PrinterKind) -> Vec<test_page::Section> {
    match kind {
        PrinterKind::Thermal => test_page::thermal_sections(&page),
        PrinterKind::Impact => test_page::impact_sections(),
    }
}

/// PNG bytes; the frontend shows them through a blob URL.
///
/// A thermal head blooms each dot wider than its pitch, so the preview
/// widens them to match. See [`dot_gain`].
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
        let prepared = picture.picture.prepare(kind, paper, &source)?.preview;
        let shown = match kind {
            PrinterKind::Thermal => {
                let dot_size = if picture.picture.double {
                    THERMAL_DOUBLE_DOT_SIZE
                } else {
                    THERMAL_DOT_SIZE
                };
                dot_gain(&prepared, dot_size)
            }
            PrinterKind::Impact => prepared,
        };
        png(&shown)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(tauri::ipc::Response::new)
}

/// Below 100 % solids would show gaps; above 300 % the 3×3 kernel no
/// longer holds the dot.
const DOT_SIZE_RANGE: std::ops::RangeInclusive<u32> = 100..=300;

/// Every ink pixel becomes a disc `dot_size` percent of the pitch wide.
/// Each output pixel combines the coverage of its own and its eight
/// neighbours' discs, assuming independent overlap.
fn dot_gain(image: &Grayscale, dot_size: u32) -> Grayscale {
    let dot_size = dot_size.clamp(*DOT_SIZE_RANGE.start(), *DOT_SIZE_RANGE.end());
    let kernel = dot_kernel(f64::from(dot_size) / 100.0);
    let (width, height) = (image.width(), image.height());
    let pixels = image.pixels();
    let ink = |x: i64, y: i64| {
        x >= 0
            && y >= 0
            && x < i64::from(width)
            && y < i64::from(height)
            && pixels[(y as u32 * width + x as u32) as usize] < 128
    };
    let out = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let mut white = 1.0;
            for (dy, row) in kernel.iter().enumerate() {
                for (dx, &coverage) in row.iter().enumerate() {
                    if ink(i64::from(x) + dx as i64 - 1, i64::from(y) + dy as i64 - 1) {
                        white *= 1.0 - coverage;
                    }
                }
            }
            (white * 255.0).round() as u8
        })
        .collect();
    Grayscale::new(width, height, out).expect("sized from the input")
}

/// How much of each cell a disc of `diameter` pitches on the middle cell
/// covers.
fn dot_kernel(diameter: f64) -> [[f64; 3]; 3] {
    const SAMPLES: u32 = 32;
    let r2 = (diameter / 2.0).powi(2);
    let mut kernel = [[0.0; 3]; 3];
    for (cy, row) in kernel.iter_mut().enumerate() {
        for (cx, cell) in row.iter_mut().enumerate() {
            let inside = (0..SAMPLES * SAMPLES)
                .filter(|i| {
                    let sx = f64::from(i % SAMPLES) + 0.5;
                    let sy = f64::from(i / SAMPLES) + 0.5;
                    let x = cx as f64 - 1.5 + sx / f64::from(SAMPLES);
                    let y = cy as f64 - 1.5 + sy / f64::from(SAMPLES);
                    x * x + y * y <= r2
                })
                .count();
            *cell = f64::from(inside as u32) / f64::from(SAMPLES * SAMPLES);
        }
    }
    kernel
}

fn png(image: &Grayscale) -> Result<Vec<u8>, String> {
    let (width, height) = (image.width(), image.height());
    let gray = image::GrayImage::from_raw(width, height, image.pixels().to_vec())
        .expect("buffer matches its dimensions");
    let mut out = Cursor::new(Vec::new());
    gray.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single_dot() -> Grayscale {
        let mut pixels = vec![255u8; 9];
        pixels[4] = 0;
        Grayscale::new(3, 3, pixels).unwrap()
    }

    #[test]
    fn pitch_sized_dot_stays_in_its_cell() {
        let k = dot_kernel(1.0);
        assert!((k[1][1] - std::f64::consts::FRAC_PI_4).abs() < 0.01);
        assert_eq!(k[0][1], 0.0);
        assert_eq!(k[0][0], 0.0);
    }

    #[test]
    fn larger_dots_bleed_into_neighbours() {
        let out = dot_gain(&single_dot(), 150);
        let p = out.pixels();
        assert!(p[4] < 10, "centre is near black: {}", p[4]);
        assert!(p[1] < 255 && p[1] > 128, "edge neighbour is grey: {}", p[1]);
        assert_eq!(p[0], 255, "corner untouched at 150 %");
        let wider = dot_gain(&single_dot(), 200);
        assert!(wider.pixels()[0] < 255, "corner touched at 200 %");
    }

    #[test]
    fn solid_black_stays_black() {
        let black = Grayscale::new(4, 4, vec![0; 16]).unwrap();
        assert!(dot_gain(&black, 300).pixels().iter().all(|&p| p == 0));
    }
}
