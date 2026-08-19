//! Shared value types used by the document builders.

/// Horizontal alignment of subsequent lines.
///
/// Rendered with `ESC GS a n` (Star Line Mode command specifications,
/// "Specify position alignment").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    /// Align to the left edge of the print area (printer default).
    #[default]
    Left,
    /// Centre within the print area.
    Center,
    /// Align to the right edge of the print area.
    Right,
}

impl Alignment {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Center => 1,
            Self::Right => 2,
        }
    }
}

/// Paper cut variants for `ESC d n`.
///
/// The "feed" variants first advance the paper so that everything printed
/// so far clears the cutter blade — this is what a typical receipt wants.
/// The non-feeding variants cut at the current position.
///
/// A *partial* cut leaves a small tab of paper so the receipt does not fall;
/// a *full* cut severs it completely. Impact printers in the SP700 series
/// only perform partial cuts: full-cut requests are executed as partial cuts
/// by the printer itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cut {
    /// Full cut at the current position.
    Full,
    /// Partial cut at the current position.
    Partial,
    /// Feed to the cutting position, then cut fully.
    FeedThenFull,
    /// Feed to the cutting position, then cut partially.
    FeedThenPartial,
}

impl Cut {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Full => 0,
            Self::Partial => 1,
            Self::FeedThenFull => 2,
            Self::FeedThenPartial => 3,
        }
    }
}

/// Peripheral-drive circuits on the printer, normally wired to cash drawers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Drawer {
    /// External device 1 (the standard cash-drawer connector pin).
    One,
    /// External device 2.
    Two,
}

/// An international character-set variant selected with `ESC R n`.
///
/// This swaps a handful of code points (e.g. `#`, `$`, `@`, brackets) for
/// region-specific glyphs, following the classic ISO 646 national variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum International {
    /// USA (printer default).
    #[default]
    Usa,
    /// France.
    France,
    /// Germany.
    Germany,
    /// United Kingdom.
    Uk,
    /// Denmark (variant I).
    Denmark,
    /// Sweden.
    Sweden,
    /// Italy.
    Italy,
    /// Spain (variant I).
    Spain,
    /// Japan.
    Japan,
    /// Norway.
    Norway,
    /// Denmark (variant II).
    Denmark2,
    /// Spain (variant II).
    Spain2,
    /// Latin America.
    LatinAmerica,
    /// Korea.
    Korea,
    /// Ireland.
    Ireland,
    /// Legal (swaps in §, ¶, ©, ®, ™, ¢ glyphs).
    Legal,
}

impl International {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Usa => 0,
            Self::France => 1,
            Self::Germany => 2,
            Self::Uk => 3,
            Self::Denmark => 4,
            Self::Sweden => 5,
            Self::Italy => 6,
            Self::Spain => 7,
            Self::Japan => 8,
            Self::Norway => 9,
            Self::Denmark2 => 10,
            Self::Spain2 => 11,
            Self::LatinAmerica => 12,
            Self::Korea => 13,
            Self::Ireland => 14,
            Self::Legal => 64,
        }
    }
}

/// A single-byte code page selected with `ESC GS t n`.
///
/// The constants cover the pages most Star models ship with; other values
/// from the model's command manual can be passed with [`CodePage::custom`].
/// Note that [`text`](crate::Builder::text) always encodes as CP437 — after
/// switching the printer to a different page, send pre-encoded bytes with
/// [`raw`](crate::Builder::raw).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodePage(pub(crate) u8);

impl CodePage {
    /// Star's own "Normal" character table (this is *not* CP437).
    pub const NORMAL: Self = Self(0);
    /// Code page 437 (USA / standard Europe) — what
    /// [`text`](crate::Builder::text) encodes.
    pub const CP437: Self = Self(1);
    /// Katakana.
    pub const KATAKANA: Self = Self(2);
    /// Code page 858 (multilingual with euro sign). Star has no CP850;
    /// this is its closest superset.
    pub const CP858: Self = Self(4);
    /// Code page 852 (Latin-2, Central Europe).
    pub const CP852: Self = Self(5);
    /// Code page 860 (Portuguese).
    pub const CP860: Self = Self(6);
    /// Code page 861 (Icelandic).
    pub const CP861: Self = Self(7);
    /// Code page 863 (Canadian French).
    pub const CP863: Self = Self(8);
    /// Code page 865 (Nordic).
    pub const CP865: Self = Self(9);
    /// Code page 866 (Cyrillic, Russian).
    pub const CP866: Self = Self(10);
    /// Code page 855 (Cyrillic, Bulgarian).
    pub const CP855: Self = Self(11);
    /// Code page 857 (Turkish).
    pub const CP857: Self = Self(12);
    /// Code page 862 (Hebrew).
    pub const CP862: Self = Self(13);
    /// Code page 864 (Arabic).
    pub const CP864: Self = Self(14);
    /// Code page 737 (Greek).
    pub const CP737: Self = Self(15);
    /// Code page 874 (Thai).
    pub const CP874: Self = Self(21);
    /// Windows-1252 (Latin-1).
    pub const WINDOWS_1252: Self = Self(32);
    /// Windows-1250 (Latin-2).
    pub const WINDOWS_1250: Self = Self(33);
    /// Windows-1251 (Cyrillic).
    pub const WINDOWS_1251: Self = Self(34);
    /// UTF-8 (only on recent firmware — "Spec E/F" models).
    pub const UTF8: Self = Self(128);
    /// The user-defined (initially blank) code page.
    pub const USER_DEFINED: Self = Self(255);

    /// Selects a code page by its raw identifier from the printer manual.
    #[must_use]
    pub const fn custom(n: u8) -> Self {
        Self(n)
    }
}

/// Base character font on thermal printers, selected with `ESC RS F n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThermalFont {
    /// Font A, 12×24 dots (printer default).
    #[default]
    A,
    /// Font B, 9×24 dots — narrower, fits more columns per line.
    B,
    /// OCR-B, 16×24 dots. While selected, code-page and international
    /// character settings are disabled.
    OcrB,
}

impl ThermalFont {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::OcrB => 16,
        }
    }
}

/// Line-feed pitch on thermal printers, selected with `ESC z n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineSpacing {
    /// 3 mm (1/8″) per line — denser tickets.
    ThreeMm,
    /// 4 mm (1/6″) per line — the usual receipt pitch.
    FourMm,
}

/// ANK character font on SP700-series impact printers.
///
/// Column counts below are for 76 mm paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImpactFont {
    /// 7×9 half-dot font — 42 columns (printer default).
    #[default]
    SevenByNine,
    /// 5×9 font (2P-1 pitch) — 35 columns.
    FiveByNine,
    /// 5×9 font (3P-1 pitch) — 23 wide columns.
    FiveByNineLarge,
}

/// Print colour on two-colour (black/red ribbon) impact printers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Color {
    /// Black (printer default).
    #[default]
    Black,
    /// Red. Requires two-colour mode
    /// ([`two_color`](crate::Builder::two_color)) and a black/red ribbon
    /// cartridge. Red passes print uni-directionally, so they are slower.
    Red,
}
