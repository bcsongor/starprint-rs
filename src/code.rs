//! Barcode and QR code definitions (Star Line Mode / thermal printers).
//!
//! The dot impact command set has no barcode or 2D-code commands, so these
//! types are only accepted by `Builder<StarLine>`.

use crate::error::{Error, Result};

/// One-dimensional barcode symbologies supported by `ESC b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Symbology {
    /// UPC-E (6 digits).
    UpcE,
    /// UPC-A (11–12 digits).
    UpcA,
    /// EAN-8 / JAN-8 (7–8 digits).
    Ean8,
    /// EAN-13 / JAN-13 (12–13 digits).
    Ean13,
    /// Code 39.
    Code39,
    /// Interleaved 2 of 5 (even number of digits).
    Itf,
    /// Code 128.
    Code128,
    /// Code 93.
    Code93,
    /// NW-7 / Codabar.
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

    /// Largest valid module-width mode (`n3`) for this symbology: the
    /// fixed-ratio symbologies accept 1–3, the two-width symbologies
    /// (Code 39, ITF, NW-7) accept 1–9.
    fn max_module(self) -> u8 {
        match self {
            Self::Code39 | Self::Itf | Self::Nw7 => 9,
            _ => 3,
        }
    }

    /// Returns an error if `data` contains bytes this symbology cannot
    /// encode.
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
            // Code 128 / Code 93 accept the full printable-ASCII range.
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

/// A one-dimensional barcode, printed with `ESC b n1 n2 n3 n4 … RS`.
///
/// Construct with [`Barcode::new`] (which validates the payload against the
/// symbology's character set), then adjust the optional settings:
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
    /// Creates a barcode, validating `data` against the symbology's
    /// character set.
    ///
    /// Defaults: no human-readable text, module-width mode 2, height 80
    /// dots (10 mm at 8 dots/mm).
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

    /// Sets the module-width mode (`n3` in the Star manual's
    /// per-symbology width table).
    ///
    /// For UPC/EAN/Code 128/Code 93 the valid modes are 1–3 (minimum
    /// module of 2, 3 or 4 dots); for Code 39/ITF/NW-7 modes 1–9 pick a
    /// narrow:wide ratio. Out-of-range values are clamped — the printer
    /// would otherwise discard the whole command.
    #[must_use]
    pub fn module_width(mut self, mode: u8) -> Self {
        self.module = mode.clamp(1, self.symbology.max_module());
        self
    }

    /// Sets the bar height in dots, 1–255 (8 dots ≈ 1 mm on 203 dpi
    /// models).
    #[must_use]
    pub fn height(mut self, dots: u8) -> Self {
        self.height = dots.max(1);
        self
    }
}

/// QR code model, per the Star 2D-code commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QrModel {
    /// Model 1 — the original, smaller-capacity specification.
    Model1,
    /// Model 2 — the common modern variant (printer/industry default).
    #[default]
    Model2,
}

/// QR error-correction level: the fraction of the symbol that can be
/// damaged and still scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QrErrorCorrection {
    /// ~7% recovery.
    L,
    /// ~15% recovery (a good receipt default).
    #[default]
    M,
    /// ~25% recovery.
    Q,
    /// ~30% recovery.
    H,
}

/// A QR code, printed with the `ESC GS y` 2D-code command family.
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
    /// Maximum payload accepted by the data command.
    pub const MAX_DATA: usize = 7089;

    /// Creates a QR code with model 2, error-correction level M and a
    /// 4-dot cell size.
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

    /// Selects the QR model (default: model 2).
    #[must_use]
    pub fn model(mut self, model: QrModel) -> Self {
        self.model = model;
        self
    }

    /// Selects the error-correction level (default: M).
    #[must_use]
    pub fn error_correction(mut self, level: QrErrorCorrection) -> Self {
        self.ec = level;
        self
    }

    /// Sets the module (cell) size in dots, 1–8 (default: 4). Clamped.
    #[must_use]
    pub fn cell_size(mut self, dots: u8) -> Self {
        self.cell_size = dots.clamp(1, 8);
        self
    }
}
