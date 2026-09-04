//! The printer a job prints on: what it is, what it is set to, and the
//! bytes a [`Job`] comes to on it.

use image::DynamicImage;
use serde::{Deserialize, Serialize};
use starprint::{Builder, Color, Document, PrintMode, PrintSpeed, StarLine};

use crate::{Job, picture, test_page};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrinterKind {
    Thermal,
    Impact,
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

    /// Roll width in millimetres, as the printer is sold.
    pub fn mm(self) -> u16 {
        match self {
            Self::Mm80 => 80,
            Self::Mm112 => 112,
        }
    }

    /// The inverse of [`mm`](Self::mm).
    pub fn from_mm(mm: u16) -> Option<Self> {
        match mm {
            80 => Some(Self::Mm80),
            112 => Some(Self::Mm112),
            _ => None,
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

/// Selects two-colour mode for darker black on plain paper.
/// Double-resolution sections use +3 instead.
pub const TWO_COLOR_DENSITY: i8 = 4;

/// The printer's density scale plus our two-colour setting.
const DENSITY_RANGE: std::ops::RangeInclusive<i8> = -3..=TWO_COLOR_DENSITY;

/// `starprint` clamps a density it cannot use. A profile or a request
/// that names one is a mistake worth reporting instead, so both front
/// ends check it here and word it the same way.
pub fn check_density(density: i8) -> Result<(), String> {
    if DENSITY_RANGE.contains(&density) {
        return Ok(());
    }
    Err(format!(
        "`density` is {density}, outside {} to {}",
        DENSITY_RANGE.start(),
        DENSITY_RANGE.end()
    ))
}

/// A printer profile. `density`, `speed` and `paper` are thermal only.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Printer {
    pub host: String,
    pub port: u16,
    /// Flattened, so the wire format stays one flat object with `kind`
    /// beside `host` and `port`.
    #[serde(flatten)]
    pub head: Head,
    pub cut: bool,
}

/// The head, and the settings only that head has. An SP700 has no
/// density, speed or roll width, so on impact there is nowhere to put
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Head {
    Thermal {
        #[serde(default = "default_paper")]
        paper: Paper,
        density: i8,
        speed: Speed,
    },
    Impact,
}

fn default_paper() -> Paper {
    Paper::Mm80
}

impl Head {
    pub fn kind(self) -> PrinterKind {
        match self {
            Self::Thermal { .. } => PrinterKind::Thermal,
            Self::Impact => PrinterKind::Impact,
        }
    }

    /// The roll a thermal printer holds. The SP700's carriage is fixed,
    /// so impact reports 80 mm and every job ignores it.
    pub fn paper(self) -> Paper {
        match self {
            Self::Thermal { paper, .. } => paper,
            Self::Impact => default_paper(),
        }
    }
}

impl Printer {
    pub fn thermal(host: String, port: u16, paper: Paper, density: i8, speed: Speed) -> Self {
        Self {
            host,
            port,
            head: Head::Thermal {
                paper,
                density,
                speed,
            },
            cut: true,
        }
    }

    pub fn impact(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            head: Head::Impact,
            cut: true,
        }
    }

    /// `host:port`, ready for [`starprint::transport::TcpTransport`].
    pub fn address(&self) -> String {
        format!("{}:{}", self.host.trim(), self.port)
    }

    /// The bytes `job` prints on this printer. A [`Job::Picture`] needs
    /// `image`; the other kinds ignore it.
    pub fn document(&self, job: &Job, image: Option<&DynamicImage>) -> Result<Document, String> {
        match job {
            Job::TaskCard(card) if card.text.trim().is_empty() => {
                return Err("The task text is empty.".to_owned());
            }
            Job::Text(text) if text.text.trim().is_empty() => {
                return Err("The text is empty.".to_owned());
            }
            Job::Qr(code) if code.data.trim().is_empty() => {
                return Err("The QR data is empty.".to_owned());
            }
            _ => {}
        }
        let cut = self.cut;
        match self.head {
            Head::Thermal {
                paper,
                density,
                speed,
            } => {
                // The print mode outlives `ESC @` and the job that set
                // it, so every job selects its mode first, and a
                // two-colour job leaves the printer in single colour.
                let two_color = density == TWO_COLOR_DENSITY;
                let mode = if two_color {
                    PrintMode::TwoColor
                } else {
                    PrintMode::SingleColor
                };
                let head = starprint::starline().print_mode(mode).color(Color::Black);
                let head = if two_color {
                    head
                } else {
                    head.print_density(density).print_speed(speed.into())
                };
                let doc = match job {
                    Job::TaskCard(card) => card.document(head, paper, cut),
                    Job::Text(text) => text.document(head, paper, cut),
                    Job::Note(note) => note.document(head, paper, cut),
                    Job::Qr(code) => code.document(head, paper, cut)?,
                    Job::TestPage(page) => test_page::thermal(head, page, paper, cut, mode),
                    Job::Picture(pic) => {
                        let image = image.ok_or("No picture supplied.")?;
                        picture::thermal(head, pic, paper, cut, image, mode)?
                    }
                };
                if two_color {
                    Ok(Builder::<StarLine>::without_init()
                        .raw(doc)
                        .print_mode(PrintMode::SingleColor)
                        .build())
                } else {
                    Ok(doc)
                }
            }
            Head::Impact => {
                // The carriage is fixed. These jobs still take a roll
                // width and every one of them ignores it.
                let paper = Paper::Mm80;
                let head = starprint::impact();
                match job {
                    Job::TaskCard(card) => Ok(card.document(head, paper, cut)),
                    Job::Text(text) => Ok(text.document(head, paper, cut)),
                    Job::Note(note) => Ok(note.document(head, paper, cut)),
                    Job::Qr(code) => code.document(head, paper, cut),
                    Job::TestPage(_) => Ok(test_page::impact(head, cut)),
                    Job::Picture(pic) => {
                        let image = image.ok_or("No picture supplied.")?;
                        picture::impact(head, pic, cut, image)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Rule;
    use crate::{Note, Picture, Qr, TaskCard, TestPage, Text};

    fn thermal() -> Printer {
        Printer::thermal(
            "printer.invalid".to_owned(),
            9100,
            Paper::Mm80,
            3,
            Speed::Slow,
        )
    }

    #[test]
    fn thermal_jobs_select_the_print_mode_before_printing() {
        let printer = thermal();
        let jobs = [
            Job::TaskCard(TaskCard {
                text: "Task".to_owned(),
                ..TaskCard::default()
            }),
            Job::Text(Text {
                text: "Text".to_owned(),
                ..Text::default()
            }),
            Job::Note(Note {
                rule: Rule::Lines,
                rows: 4,
                pitch: 7,
            }),
            Job::TestPage(TestPage::default()),
        ];
        for job in jobs {
            let document = printer.document(&job, None).expect("builds");
            let bytes = document.as_bytes();
            let first = bytes
                .windows(4)
                .find(|w| w[..3] == [0x1b, 0x1e, b'C'])
                .expect("selects a print mode");
            assert_eq!(first[3], 0, "starts in single colour: {job:?}");
        }
    }

    #[test]
    fn an_empty_task_or_text_is_an_error() {
        let printer = thermal();
        let card = Job::TaskCard(TaskCard {
            text: "   ".to_owned(),
            ..TaskCard::default()
        });
        assert_eq!(
            printer.document(&card, None).unwrap_err(),
            "The task text is empty."
        );
        let text = Job::Text(Text::default());
        assert_eq!(
            printer.document(&text, None).unwrap_err(),
            "The text is empty."
        );
        let code = Job::Qr(Qr::default());
        assert_eq!(
            printer.document(&code, None).unwrap_err(),
            "The QR data is empty."
        );
    }

    #[test]
    fn a_picture_job_without_an_image_is_an_error() {
        let job = Job::Picture(Picture::default());
        assert_eq!(
            thermal().document(&job, None).unwrap_err(),
            "No picture supplied."
        );
    }

    #[test]
    fn an_impact_printer_ignores_the_roll_and_the_thermal_settings() {
        let printer = Printer::impact("printer.invalid".to_owned(), 9100);
        let job = Job::Text(Text {
            text: "Hi".to_owned(),
            ..Text::default()
        });
        let bytes = printer.document(&job, None).unwrap();
        let bytes = bytes.as_bytes();
        assert!(
            !bytes.windows(3).any(|w| w[..2] == [0x1b, 0x1e]),
            "no density, speed or print mode: {bytes:02x?}"
        );
    }

    #[test]
    fn the_density_above_the_scale_is_two_colour_mode_for_the_job() {
        let job = Job::Text(Text {
            text: "Hi".to_owned(),
            ..Text::default()
        });
        let mut printer = thermal();
        printer.head = Head::Thermal {
            paper: Paper::Mm80,
            density: TWO_COLOR_DENSITY,
            speed: Speed::Slow,
        };
        let doc = printer.document(&job, None).unwrap();
        let bytes = doc.as_bytes();
        let first = bytes
            .windows(4)
            .find(|w| w[..3] == [0x1b, 0x1e, b'C'])
            .expect("selects a print mode");
        assert_eq!(first[3], 1, "starts in two-colour mode");
        assert!(
            bytes.windows(4).any(|w| w == [0x1b, 0x1e, b'c', 0]),
            "text in black, whatever the last job left: {bytes:02x?}"
        );
        assert!(
            !bytes
                .windows(3)
                .any(|w| w[..2] == [0x1b, 0x1e] && (w[2] == b'd' || w[2] == b'r')),
            "no density or speed, which the mode ignores: {bytes:02x?}"
        );
        assert!(
            bytes.ends_with(&[0x1b, b'd', 3, 0x1b, 0x1e, b'C', 0]),
            "cuts, then hands the printer back in single colour"
        );
    }

    #[test]
    fn double_resolution_at_plus_four_sets_density_before_the_raster() {
        let printer = Printer::thermal(
            "printer.invalid".to_owned(),
            9100,
            Paper::Mm80,
            TWO_COLOR_DENSITY,
            Speed::Slow,
        );
        let image = DynamicImage::new_rgb8(1, 1);
        for job in [
            Job::Picture(Picture {
                double: true,
                ..Picture::default()
            }),
            Job::TestPage(TestPage {
                double_resolution: true,
            }),
        ] {
            let document = printer.document(&job, Some(&image)).unwrap();
            let bytes = document.as_bytes();
            let double = bytes
                .windows(4)
                .position(|w| w == [0x1b, 0x1e, b'C', 32])
                .expect("selects double resolution");
            assert_eq!(
                &bytes[double + 4..double + 12],
                &[0x1b, 0x1e, b'd', 0, 0x1b, b'*', b'r', b'A'],
                "sets +3 before entering raster mode: {job:?}"
            );
            let end = bytes[double + 12..]
                .windows(4)
                .position(|w| w[..3] == [0x1b, 0x1e, b'C'])
                .expect("restores the job's mode");
            assert_eq!(bytes[double + 12 + end + 3], 1);
            assert!(bytes.ends_with(&[0x1b, 0x1e, b'C', 0]));
        }
    }

    #[test]
    fn cut_decides_how_a_job_ends() {
        let job = Job::TestPage(TestPage::default());
        let cut = thermal().document(&job, None).unwrap();
        assert!(cut.as_bytes().ends_with(&[0x1b, b'd', 3]));

        let mut printer = thermal();
        printer.cut = false;
        let feed = printer.document(&job, None).unwrap();
        assert!(feed.as_bytes().ends_with(&[0x1b, b'a', 3]));
    }

    #[test]
    fn an_address_is_the_host_and_port_with_the_whitespace_gone() {
        let mut printer = thermal();
        printer.host = "  192.168.1.180 ".to_owned();
        assert_eq!(printer.address(), "192.168.1.180:9100");
    }

    /// The desktop app stores a profile as one flat object and has done
    /// since before the head was a variant, so the settings sit beside
    /// `host` on the wire whatever shape they take in Rust.
    #[test]
    fn a_profile_deserialises_from_one_flat_object() {
        let json = r#"{"kind":"thermal","host":"h","port":9100,
                       "density":-2,"speed":"medium","paper":"112","cut":false}"#;
        let printer: Printer = serde_json::from_str(json).unwrap();
        assert_eq!(
            printer.head,
            Head::Thermal {
                paper: Paper::Mm112,
                density: -2,
                speed: Speed::Medium
            }
        );
        assert!(!printer.cut);

        // Impact profiles the app saved carry leftover thermal fields.
        let json = r#"{"kind":"impact","host":"h","port":9100,
                       "density":3,"speed":"slow","paper":"80","cut":true}"#;
        let printer: Printer = serde_json::from_str(json).unwrap();
        assert_eq!(printer.head, Head::Impact);
        assert_eq!(printer.head.kind(), PrinterKind::Impact);
        assert_eq!(printer.head.paper(), Paper::Mm80);
    }

    #[test]
    fn paper_round_trips_through_millimetres() {
        for paper in [Paper::Mm80, Paper::Mm112] {
            assert_eq!(Paper::from_mm(paper.mm()), Some(paper));
        }
        assert_eq!(Paper::from_mm(58), None);
    }
}
