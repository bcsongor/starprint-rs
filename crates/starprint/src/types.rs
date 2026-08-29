//! Shared value types used by the document builders.

/// Horizontal alignment, set with `ESC GS a n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
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

/// Paper cut for `ESC d n`. The `FeedThen*` variants first feed what has
/// printed past the blade, which is what a receipt wants. A partial cut
/// leaves a tab so the paper does not fall; the SP700 series only does
/// partial cuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cut {
    Full,
    Partial,
    FeedThenFull,
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

/// The printer's two peripheral-drive circuits, normally cash drawers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Drawer {
    One,
    Two,
}

/// ISO 646 national variant selected with `ESC R n`: swaps `#`, `$`, `@`,
/// brackets and a few others for regional glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum InternationalCharset {
    #[default]
    Usa,
    France,
    Germany,
    Uk,
    Denmark,
    Sweden,
    Italy,
    Spain,
    Japan,
    Norway,
    Denmark2,
    Spain2,
    LatinAmerica,
    Korea,
    Ireland,
    /// Swaps in §, ¶, ©, ®, ™ and ¢.
    Legal,
}

impl InternationalCharset {
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

/// A code page selected with `ESC GS t n`. [`text`](crate::Builder::text)
/// always encodes CP437; for another page send pre-encoded bytes with
/// [`raw`](crate::Builder::raw).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodePage(pub(crate) u8);

impl CodePage {
    /// Star's own "Normal" table, which is not CP437.
    pub const NORMAL: Self = Self(0);
    pub const CP437: Self = Self(1);
    pub const KATAKANA: Self = Self(2);
    /// Multilingual with euro sign; Star's nearest to CP850.
    pub const CP858: Self = Self(4);
    /// Latin-2.
    pub const CP852: Self = Self(5);
    /// Portuguese.
    pub const CP860: Self = Self(6);
    /// Icelandic.
    pub const CP861: Self = Self(7);
    /// Canadian French.
    pub const CP863: Self = Self(8);
    /// Nordic.
    pub const CP865: Self = Self(9);
    /// Cyrillic (Russian).
    pub const CP866: Self = Self(10);
    /// Cyrillic (Bulgarian).
    pub const CP855: Self = Self(11);
    /// Turkish.
    pub const CP857: Self = Self(12);
    /// Hebrew.
    pub const CP862: Self = Self(13);
    /// Arabic.
    pub const CP864: Self = Self(14);
    /// Greek.
    pub const CP737: Self = Self(15);
    /// Thai.
    pub const CP874: Self = Self(21);
    pub const WINDOWS_1252: Self = Self(32);
    pub const WINDOWS_1250: Self = Self(33);
    pub const WINDOWS_1251: Self = Self(34);
    /// "Spec E/F" firmware only.
    pub const UTF8: Self = Self(128);
    pub const USER_DEFINED: Self = Self(255);

    /// A code page by its identifier from the printer manual.
    #[must_use]
    pub const fn custom(n: u8) -> Self {
        Self(n)
    }
}

/// Character font on thermal printers, selected with `ESC RS F n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThermalFont {
    /// 12×24 dots.
    #[default]
    A,
    /// 9×24 dots.
    B,
    /// 16×24 dots. Code page and international settings are off while
    /// it is selected.
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

/// Print mode on thermal printers, selected with `ESC RS C n`. Survives
/// `ESC @`; not every model has every mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PrintMode {
    #[default]
    SingleColor,
    /// Two-colour thermal paper (TSP700II, TSP800II).
    TwoColor,
    /// Low peak current for weak power supplies; speed is fixed.
    LowPower,
    /// 16 dot rows per millimetre instead of 8 (TSP700II, TSP800II).
    /// Prepare images with a `*_DOUBLE_RESOLUTION`
    /// [`DeviceProfile`](crate::graphics::DeviceProfile).
    DoubleResolution,
}

impl PrintMode {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::SingleColor => 0,
            Self::TwoColor => 1,
            Self::LowPower => 16,
            Self::DoubleResolution => 32,
        }
    }
}

/// Line-mode print speed on thermal printers, set with `ESC RS r n`.
/// Slower gives the head more time per row, so dense output prints
/// darker and more evenly.
///
/// Values follow the "Spec. A" table (TSP700II, TSP800II, TSP650II,
/// FVP10); the TUP500 and TSP650IISK number them differently. Ignored in
/// double-resolution, two-colour and low-power modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PrintSpeed {
    #[default]
    High,
    Medium,
    Slow,
}

impl PrintSpeed {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Slow => 2,
        }
    }
}

/// Speed/quality trade-off for raster graphics, set with `ESC * r Q n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RasterQuality {
    HighSpeed,
    #[default]
    Normal,
    /// Slowest; use it for photographs.
    High,
}

impl RasterQuality {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::HighSpeed => b'0',
            Self::Normal => b'1',
            Self::High => b'2',
        }
    }
}

/// Line-feed pitch on thermal printers, selected with `ESC z n`. 4 mm is
/// the usual receipt pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineSpacing {
    ThreeMm,
    FourMm,
}

/// Character font on SP700-series impact printers. Columns are for 76 mm
/// paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImpactFont {
    /// 42 columns.
    #[default]
    SevenByNine,
    /// 35 columns.
    FiveByNine,
    /// 23 columns.
    FiveByNineWide,
}

/// Print colour on impact printers with a black/red ribbon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Color {
    #[default]
    Black,
    /// Needs [`two_color`](crate::Builder::two_color). Red passes print
    /// one way only, so they are slower.
    Red,
}
