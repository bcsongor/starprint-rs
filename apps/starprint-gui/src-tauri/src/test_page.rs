//! Diagnostic test pages: one per head technology.
//!
//! The thermal page is the `thermal_test_pattern` example from the library
//! crate; the impact page is its counterpart for a 9-pin head, where the
//! things that fail are pins (horizontal gaps), the ribbon (faint or
//! missing red) and dot registration between densities.

use serde::{Deserialize, Serialize};
use starprint::graphics::{BitImage, Bitmap, Density, Dithering, Grayscale};
use starprint::{
    Alignment, Barcode, Builder, Color, Cut, Document, Impact, ImpactFont, PrintMode, QrCode,
    RasterQuality, StarLine, Symbology, ThermalFont,
};

/// Paper width of a thermal printer; decides the raster width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Paper {
    /// 80 mm roll: 72 mm print region, 576 dots.
    #[serde(rename = "80")]
    Mm80,
    /// 112 mm roll: 104 mm print region, 832 dots.
    #[serde(rename = "112")]
    Mm112,
}

impl Paper {
    fn dots(self) -> u32 {
        match self {
            Self::Mm80 => 576,
            Self::Mm112 => 832,
        }
    }
}

/// Options for the test page. Impact ignores both.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPage {
    pub paper: Paper,
    /// Thermal: repeat the grey ramp in double-resolution mode.
    pub double_resolution: bool,
}

/// One numbered section of the page, for the preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    /// What a fault looks like in this section.
    pub check: String,
}

fn section(title: &str, check: &str) -> Section {
    Section {
        title: title.to_owned(),
        check: check.to_owned(),
    }
}

pub fn thermal_sections(page: &TestPage) -> Vec<Section> {
    let mut sections = vec![
        section("Solid bar", "white hairline = dead element"),
        section("Lines every 8 dots", "faint or missing = weak element"),
        section("One-dot checkerboard", "sharpness and dot gain"),
        section("Grey ramp", "banding = platen or heat; streaks = drift"),
    ];
    if page.double_resolution {
        sections.push(section("Grey ramp, double resolution", "as above"));
    }
    sections.push(section(
        "Fonts A and B, double size, Code 128, QR",
        "general sanity",
    ));
    sections
}

pub fn impact_sections() -> Vec<Section> {
    vec![
        section("Solid block", "white horizontal line = dead pin"),
        section(
            "Every other pin row",
            "missing row = dead pin; uneven = feed",
        ),
        section(
            "Checkerboard, single and double density",
            "registration between passes",
        ),
        section("Fonts 7×9, 5×9, 5×9 wide; double size", "character forming"),
        section("Red and black", "faint red = ribbon; offset = colour shift"),
        section("Character set", "all printable ASCII"),
    ]
}

fn ramp(width: u32, height: u32) -> Bitmap {
    let pixels = (0..height)
        .flat_map(|_| (0..width).map(move |x| (x * 255 / (width - 1)) as u8))
        .collect();
    Dithering::FloydSteinberg { threshold: 128 }
        .apply(&Grayscale::new(width, height, pixels).expect("sized buffer"))
        .to_bitmap()
}

pub fn thermal(builder: Builder<StarLine>, page: &TestPage, cut: bool) -> Document {
    let width = page.paper.dots();
    let black_bar = Bitmap::from_fn(width, 64, |_, _| true);
    let hairlines = Bitmap::from_fn(width, 48, |x, _| x % 8 == 0);
    let checkerboard = Bitmap::from_fn(width, 32, |x, y| (x + y) % 2 == 0);

    let mut doc = builder
        .align(Alignment::Center)
        .bold(true)
        .line("STARPRINT HEAD CHECK")
        .bold(false)
        .line(&format!("{width} dots"))
        .feed(1)
        .align(Alignment::Left)
        .line("1 solid bar: white hairline = dead dot")
        .raster(&black_bar, RasterQuality::High)
        .line("2 lines every 8 dots: faint/missing = weak dot")
        .raster(&hairlines, RasterQuality::High)
        .line("3 one-dot checkerboard: sharpness")
        .raster(&checkerboard, RasterQuality::High)
        .line("4 grey ramp: banding / streaks")
        .raster(ramp(width, 96), RasterQuality::High);

    if page.double_resolution {
        doc = doc
            .line("4b grey ramp, double resolution")
            .print_mode(PrintMode::DoubleResolution)
            .raster(ramp(width, 192), RasterQuality::High)
            .print_mode(PrintMode::SingleColor);
    }

    let barcode = Barcode::new(Symbology::Code128, "STARPRINT")
        .expect("static barcode data")
        .human_readable(true);
    let qr = QrCode::new("https://github.com/bcsongor/starprint-rs").expect("static QR data");
    let doc = doc
        .line("5 text and codes")
        .font(ThermalFont::A)
        .line("Font A: The quick brown fox 0123456789")
        .font(ThermalFont::B)
        .line("Font B: The quick brown fox 0123456789")
        .font(ThermalFont::A)
        .wide(2)
        .tall(2)
        .line("Double size")
        .wide(1)
        .tall(1)
        .align(Alignment::Center)
        .barcode(&barcode)
        .feed(1)
        .qr_code(&qr)
        .feed(2);
    finish(doc, cut)
}

pub fn impact(builder: Builder<Impact>, cut: bool) -> Document {
    const SINGLE: u32 = 210;
    const DOUBLE: u32 = 420;
    // Nine pins per stripe; four stripes make a block tall enough to see.
    const ROWS: u32 = 36;

    let image = |bitmap: Bitmap, density: Density| {
        BitImage::new(bitmap, density).expect("sized for the SP700 head")
    };
    let block = image(Bitmap::from_fn(SINGLE, ROWS, |_, _| true), Density::Single);
    let pin_rows = image(
        Bitmap::from_fn(SINGLE, ROWS, |_, y| y % 2 == 0),
        Density::Single,
    );
    let checker_single = image(
        Bitmap::from_fn(SINGLE, 18, |x, y| (x + y) % 2 == 0),
        Density::Single,
    );
    let checker_double = image(
        Bitmap::from_fn(DOUBLE, 18, |x, y| (x + y) % 2 == 0),
        Density::Double,
    );
    let ascii: String = (0x20u8..0x7f).map(char::from).collect();

    let doc = builder
        .two_color(true)
        .align(Alignment::Center)
        .double_wide(true)
        .double_tall(true)
        .line("STARPRINT")
        .double_wide(false)
        .double_tall(false)
        .line("PIN AND RIBBON CHECK")
        .feed(1)
        .align(Alignment::Left)
        .line("1 solid block: white line = dead pin")
        .bit_image(&block)
        .line("2 every other pin row: gap = dead pin")
        .bit_image(&pin_rows)
        .line("3 checkerboard, single then double")
        .bit_image(&checker_single)
        .bit_image(&checker_double)
        .line("4 fonts and sizes")
        .font(ImpactFont::SevenByNine)
        .line("7x9  The quick brown fox 0123456789")
        .font(ImpactFont::FiveByNine)
        .line("5x9  The quick brown fox 0123456789")
        .font(ImpactFont::FiveByNineWide)
        .line("5x9w The quick brown fox 0123456789")
        .font(ImpactFont::SevenByNine)
        .double_wide(true)
        .line("Double wide")
        .double_wide(false)
        .double_tall(true)
        .line("Double tall")
        .double_wide(true)
        .line("Double size")
        .double_wide(false)
        .double_tall(false)
        .line("5 red and black")
        .color(Color::Red)
        .line("RED: faint or patchy = ribbon worn")
        .color(Color::Black)
        .line("BLACK: ")
        .color(Color::Red)
        .text("red ")
        .color(Color::Black)
        .text("black ")
        .color(Color::Red)
        .text("red ")
        .color(Color::Black)
        .line("black on one line")
        .line("6 character set")
        .line(&ascii[..42])
        .line(&ascii[42..84])
        .line(&ascii[84..])
        .feed(2);
    finish(doc, cut)
}

fn finish<P: starprint::Protocol>(doc: Builder<P>, cut: bool) -> Document {
    if cut {
        doc.cut(Cut::FeedThenPartial).build()
    } else {
        doc.feed(3).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(paper: Paper, double_resolution: bool) -> TestPage {
        TestPage {
            paper,
            double_resolution,
        }
    }

    #[test]
    fn thermal_page_builds_for_both_papers() {
        for paper in [Paper::Mm80, Paper::Mm112] {
            let doc = thermal(starprint::starline(), &page(paper, true), true);
            assert!(
                doc.as_bytes().len() > 10_000,
                "{paper:?} page is a raster job"
            );
            assert!(
                doc.as_bytes().ends_with(&[0x1b, b'd', 3]),
                "ends with a cut"
            );
        }
    }

    #[test]
    fn double_resolution_adds_a_section() {
        assert_eq!(thermal_sections(&page(Paper::Mm80, false)).len(), 5);
        assert_eq!(thermal_sections(&page(Paper::Mm80, true)).len(), 6);
    }

    #[test]
    fn impact_page_fits_the_head_and_prints_red() {
        let doc = impact(starprint::impact(), false);
        let bytes = doc.as_bytes();
        assert!(
            bytes.windows(2).any(|w| w == [0x1b, b'4']),
            "switches to red"
        );
        assert!(!bytes.ends_with(&[0x1b, b'd', 3]), "no cut when disabled");
    }

    #[test]
    fn character_set_splits_into_three_lines() {
        let ascii: String = (0x20u8..0x7f).map(char::from).collect();
        assert_eq!(ascii.len(), 95);
        assert_eq!(ascii[84..].len(), 11);
    }
}
