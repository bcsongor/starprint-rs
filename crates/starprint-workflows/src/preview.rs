//! A job as it will look on paper, before it prints: the lines the
//! printer's own fonts will set, and a PNG of whatever is drawn as
//! dots. The PNG is drawn from the bitmap that prints, so what is shown
//! is what comes off the printer.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::DynamicImage;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use serde::{Serialize, Serializer};
use starprint::graphics::{Bitmap, Grayscale};
use starprint::{Builder, Impact, Protocol, StarLine};

use crate::note::NoteStyle;
use crate::qr::QrStyle;
use crate::task_card::CardStyle;
use crate::test_page::{self, Section};
use crate::{Job, Paper, Printer, PrinterKind, note, qr, task_card, text};

/// Dot diameters as percentages of pitch, measured on a TSP700II at
/// slow speed and density +3. Double resolution halves the row pitch.
const THERMAL_DOT_SIZE: u32 = 150;
const THERMAL_DOUBLE_DOT_SIZE: u32 = 200;

/// Tagged by `kind` like [`Job`]. Text jobs are their layout; anything
/// printed as dots carries an `image` beside it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Preview {
    TaskCard(task_card::Layout),
    Text(text::Layout),
    Note {
        #[serde(flatten)]
        layout: note::Layout,
        image: Png,
    },
    Qr {
        #[serde(flatten)]
        layout: qr::Layout,
        image: Png,
    },
    TestPage {
        sections: Vec<Section>,
    },
    Picture {
        image: Png,
    },
}

/// PNG bytes. Serialises as a `data:` URL, so a web client can put it
/// straight into an `<img>`.
#[derive(Debug, Clone)]
pub struct Png(Vec<u8>);

impl Png {
    /// `bitmap` as it prints, with thermal dots widened to the size the
    /// head blooms them to.
    pub fn from_bitmap(bitmap: &Bitmap, kind: PrinterKind) -> Result<Self, String> {
        Self::from_grayscale(&grayscale(bitmap), kind, false)
    }

    /// A picture's preview, widened for the resolution it prints at.
    pub fn from_grayscale(
        image: &Grayscale,
        kind: PrinterKind,
        double: bool,
    ) -> Result<Self, String> {
        let shown = match (kind, double) {
            (PrinterKind::Thermal, false) => dot_gain(image, THERMAL_DOT_SIZE),
            (PrinterKind::Thermal, true) => dot_gain(image, THERMAL_DOUBLE_DOT_SIZE),
            (PrinterKind::Impact, _) => image.clone(),
        };
        png(&shown).map(Self)
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl Serialize for Png {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("data:image/png;base64,{}", BASE64.encode(&self.0)))
    }
}

impl Printer {
    /// What `job` looks like on this printer. A [`Job::Picture`] needs
    /// `image`; the other kinds ignore it. Empty text previews as an
    /// empty layout rather than an error, since a preview is looked at
    /// while the job is still being written.
    pub fn preview(&self, job: &Job, image: Option<&DynamicImage>) -> Result<Preview, String> {
        let kind = self.head.kind();
        let paper = self.head.paper();
        match kind {
            PrinterKind::Thermal => render::<StarLine>(job, kind, paper, image),
            PrinterKind::Impact => render::<Impact>(job, kind, paper, image),
        }
    }
}

fn render<P: Protocol>(
    job: &Job,
    kind: PrinterKind,
    paper: Paper,
    image: Option<&DynamicImage>,
) -> Result<Preview, String>
where
    Builder<P>: CardStyle + NoteStyle + QrStyle,
{
    Ok(match job {
        Job::TaskCard(card) => Preview::TaskCard(card.layout::<P>(paper)),
        Job::Text(text) => Preview::Text(text.layout::<P>(paper)),
        Job::Note(note) => Preview::Note {
            layout: note.layout::<P>(paper),
            image: Png::from_bitmap(&note.bitmap::<P>(paper), kind)?,
        },
        Job::Qr(code) => Preview::Qr {
            layout: code.layout::<P>(paper)?,
            image: Png::from_bitmap(&code.bitmap::<P>(paper)?, kind)?,
        },
        Job::TestPage(page) => Preview::TestPage {
            sections: match kind {
                PrinterKind::Thermal => test_page::thermal_sections(page),
                PrinterKind::Impact => test_page::impact_sections(),
            },
        },
        Job::Picture(picture) => {
            let image = image.ok_or("No picture supplied.")?;
            Preview::Picture {
                image: Png::from_grayscale(
                    &picture.preview(kind, paper, image)?,
                    kind,
                    picture.double,
                )?,
            }
        }
    })
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

/// Looked at once and dropped, so the fastest encode wins over the
/// smallest file.
fn png(image: &Grayscale) -> Result<Vec<u8>, String> {
    let (width, height) = (image.width(), image.height());
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::NoFilter)
        .write_image(image.pixels(), width, height, ExtendedColorType::L8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Speed;
    use serde_json::{Value, json};

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

    /// A PNG that decodes to the drawn size, as a `data:` URL.
    fn decoded(preview: &Value) -> DynamicImage {
        let url = preview["image"].as_str().expect("an image");
        let bytes = BASE64
            .decode(
                url.strip_prefix("data:image/png;base64,")
                    .expect("a PNG data URL"),
            )
            .unwrap();
        image::load_from_memory(&bytes).unwrap()
    }

    #[test]
    fn every_kind_previews_as_its_layout_or_its_image() {
        let thermal = Printer::thermal("h".to_owned(), 9100, Paper::Mm80, 3, Speed::Slow);
        let impact = Printer::impact("h".to_owned(), 9100);
        let preview = |printer: &Printer, job: Value, image: Option<&DynamicImage>| {
            let job: Job = serde_json::from_value(job).unwrap();
            serde_json::to_value(printer.preview(&job, image).unwrap()).unwrap()
        };

        let card = preview(
            &thermal,
            json!({ "kind": "task-card", "text": "Standup" }),
            None,
        );
        assert_eq!(card["kind"], "task-card");
        assert_eq!(card["lines"][0], "Standup");
        assert_eq!(card["columns"], 48);

        let text = preview(&impact, json!({ "kind": "text", "text": "" }), None);
        assert_eq!(text["kind"], "text", "empty text still lays out");

        let note = preview(&thermal, json!({ "kind": "note" }), None);
        assert_eq!(note["kind"], "note");
        assert!(note["date"].is_string(), "the layout sits beside the image");
        assert_eq!(decoded(&note).width(), 576, "one 80 mm head wide");

        let code = preview(
            &impact,
            json!({ "kind": "qr", "data": "https://x.y" }),
            None,
        );
        assert_eq!(code["kind"], "qr");
        assert!(code["modules"].as_u64().unwrap() > 0);
        decoded(&code);

        let page = preview(&thermal, json!({ "kind": "test-page" }), None);
        assert_eq!(page["sections"][0]["title"], "Solid bar");

        let photo = DynamicImage::new_luma8(64, 32);
        let picture = preview(&thermal, json!({ "kind": "picture" }), Some(&photo));
        assert_eq!(picture["kind"], "picture");
        assert_eq!(decoded(&picture).width(), 576);
        let job: Job = serde_json::from_value(json!({ "kind": "picture" })).unwrap();
        assert_eq!(
            thermal.preview(&job, None).unwrap_err(),
            "No picture supplied."
        );
    }
}
