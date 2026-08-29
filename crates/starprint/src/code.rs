//! Barcodes and QR codes. The impact command set has neither, so only
//! `Builder<StarLine>` accepts these.

use crate::error::{Error, Result};

/// Barcode symbologies of `ESC b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Symbology {
    /// 6 digits.
    UpcE,
    /// 11–12 digits.
    UpcA,
    /// 7–8 digits.
    Ean8,
    /// 12–13 digits.
    Ean13,
    Code39,
    /// Interleaved 2 of 5; an even number of digits.
    Itf,
    Code128,
    Code93,
    /// Codabar.
    Nw7,
}

impl Symbology {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::UpcE => b'0',
            Self::UpcA => b'1',
            Self::Ean8 => b'2',
            Self::Ean13 => b'3',
            Self::Code39 => b'4',
            Self::Itf => b'5',
            Self::Code128 => b'6',
            Self::Code93 => b'7',
            Self::Nw7 => b'8',
        }
    }

    /// Largest module-width mode (`n3`): the two-width symbologies take
    /// 1–9, the rest 1–3.
    fn max_module(self) -> u8 {
        match self {
            Self::Code39 | Self::Itf | Self::Nw7 => 9,
            _ => 3,
        }
    }

    fn validate(self, data: &[u8]) -> Result<()> {
        let ok: fn(u8) -> bool = match self {
            Self::UpcE | Self::UpcA | Self::Ean8 | Self::Ean13 | Self::Itf => {
                |b| b.is_ascii_digit()
            }
            Self::Code39 => {
                |b| matches!(b, b'0'..=b'9' | b'A'..=b'Z' | b' ' | b'-' | b'.' | b'$' | b'/' | b'+' | b'%')
            }
            Self::Nw7 => {
                |b| matches!(b, b'0'..=b'9' | b'A'..=b'D' | b'a'..=b'd' | b'-' | b'$' | b':' | b'/' | b'.' | b'+')
            }
            Self::Code128 | Self::Code93 => |b| matches!(b, 0x20..=0x7E),
        };
        if let Some(&bad) = data.iter().find(|&&b| !ok(b)) {
            return Err(Error::InvalidData {
                reason: format!(
                    "byte 0x{bad:02X} ({:?}) is not encodable in {self:?}",
                    bad as char
                ),
            });
        }
        Ok(())
    }
}

/// A barcode for `ESC b`.
///
/// ```
/// use starprint::{Barcode, Symbology};
///
/// let code = Barcode::new(Symbology::Code128, "No.123456")?
///     .height(80)
///     .human_readable(true);
/// # Ok::<(), starprint::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barcode {
    pub(crate) symbology: Symbology,
    pub(crate) data: Vec<u8>,
    pub(crate) hri: bool,
    pub(crate) module: u8,
    pub(crate) height: u8,
}

impl Barcode {
    /// Validates `data` against the symbology's character set. Defaults
    /// to no human-readable text, module mode 2 and 80 dots (10 mm) high.
    pub fn new(symbology: Symbology, data: impl AsRef<[u8]>) -> Result<Self> {
        let data = data.as_ref();
        if data.is_empty() {
            return Err(Error::InvalidData {
                reason: "barcode data must not be empty".into(),
            });
        }
        if data.len() > 255 {
            return Err(Error::DataTooLong {
                max: 255,
                actual: data.len(),
            });
        }
        symbology.validate(data)?;
        Ok(Self {
            symbology,
            data: data.to_vec(),
            hri: false,
            module: 2,
            height: 80,
        })
    }

    /// Prints the human-readable interpretation under the bars.
    #[must_use]
    pub fn human_readable(mut self, on: bool) -> Self {
        self.hri = on;
        self
    }

    /// Sets the module-width mode (`n3` in the manual's width table):
    /// 1–3 for UPC/EAN/Code 128/Code 93, 1–9 for Code 39/ITF/NW-7.
    /// Clamped, as the printer discards the whole command otherwise.
    #[must_use]
    pub fn module_width(mut self, mode: u8) -> Self {
        self.module = mode.clamp(1, self.symbology.max_module());
        self
    }

    /// Sets the bar height in dots (8 dots ≈ 1 mm), at least 1.
    #[must_use]
    pub fn height(mut self, dots: u8) -> Self {
        self.height = dots.max(1);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QrModel {
    /// The original, with less capacity.
    Model1,
    #[default]
    Model2,
}

/// How much of the symbol can be damaged and still scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QrErrorCorrection {
    /// ~7%.
    L,
    /// ~15%.
    #[default]
    M,
    /// ~25%.
    Q,
    /// ~30%.
    H,
}

/// A QR code for `ESC GS y`.
///
/// ```
/// use starprint::{QrCode, QrErrorCorrection};
///
/// let code = QrCode::new("https://example.com/receipt/42")?
///     .error_correction(QrErrorCorrection::H)
///     .cell_size(4);
/// # Ok::<(), starprint::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    pub(crate) data: Vec<u8>,
    pub(crate) model: QrModel,
    pub(crate) ec: QrErrorCorrection,
    pub(crate) cell_size: u8,
}

impl QrCode {
    pub const MAX_DATA: usize = 7089;

    /// Defaults to model 2, level M and 4-dot cells.
    pub fn new(data: impl AsRef<[u8]>) -> Result<Self> {
        let data = data.as_ref();
        if data.is_empty() {
            return Err(Error::InvalidData {
                reason: "QR data must not be empty".into(),
            });
        }
        if data.len() > Self::MAX_DATA {
            return Err(Error::DataTooLong {
                max: Self::MAX_DATA,
                actual: data.len(),
            });
        }
        Ok(Self {
            data: data.to_vec(),
            model: QrModel::default(),
            ec: QrErrorCorrection::default(),
            cell_size: 4,
        })
    }

    #[must_use]
    pub fn model(mut self, model: QrModel) -> Self {
        self.model = model;
        self
    }

    #[must_use]
    pub fn error_correction(mut self, level: QrErrorCorrection) -> Self {
        self.ec = level;
        self
    }

    /// Cell size in dots, clamped to 1–8.
    #[must_use]
    pub fn cell_size(mut self, dots: u8) -> Self {
        self.cell_size = dots.clamp(1, 8);
        self
    }
}
