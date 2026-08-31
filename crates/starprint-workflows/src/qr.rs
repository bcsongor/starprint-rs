//! A QR code, printed as a raster (thermal) or a bit image (impact).
//!
//! The SP700 has no QR command, so the symbol is encoded here and drawn
//! as dots. Star Line Mode does have `ESC GS y`, but the thermal head is
//! given the same bitmap: one path prints the same square on both
//! printers, and the version is known here rather than chosen inside the
//! firmware. Owning the dots is also what lets a module be drawn with
//! rounded corners, which `ESC GS y` would not have done.

use qrcodegen::{QrCode as Symbol, QrCodeEcc};
use serde::{Deserialize, Serialize};
use starprint::graphics::{BitImage, Bitmap, Density, DeviceProfile};
use starprint::{Alignment, Builder, Cut, Document, Impact, Protocol, RasterQuality, StarLine};

use crate::text::{TextStyle, wrap_by_words};
use crate::{MM_PER_INCH, Paper};

/// Where the code sits across the paper. Mirrors
/// [`starprint::Alignment`], which has no serde support of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

impl From<Align> for Alignment {
    fn from(align: Align) -> Self {
        match align {
            Align::Left => Self::Left,
            Align::Center => Self::Center,
            Align::Right => Self::Right,
        }
    }
}

/// How much of the symbol can be lost and still scan. Mirrors
/// [`starprint::QrErrorCorrection`], which has no serde support of its
/// own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ecc {
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

impl From<Ecc> for QrCodeEcc {
    fn from(ecc: Ecc) -> Self {
        match ecc {
            Ecc::L => Self::Low,
            Ecc::M => Self::Medium,
            Ecc::Q => Self::Quartile,
            Ecc::H => Self::High,
        }
    }
}

/// `data` is the only part a caller must supply.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Qr {
    pub data: String,
    /// Printed above the symbol, so a slip on a desk says what it is.
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub error_correction: Ecc,
    /// The symbol's width in millimetres, quiet zone excluded. Clamped,
    /// and reduced further if the symbol will not fit the paper.
    #[serde(default = "default_size")]
    pub size: u8,
    /// How far a module's corners are taken off, as a percentage of
    /// half the module: 100 draws one with nothing beside it as a
    /// circle, 0 leaves it square. Clamped, and rounded down to whole
    /// dots, so a coarse head prints square whatever this asks for.
    #[serde(default = "default_radius")]
    pub radius: u8,
    /// Carries the caption with it.
    #[serde(default)]
    pub align: Align,
}

/// The code as it will print, for the preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub columns: usize,
    /// Wrapped to the paper; empty when there is no caption.
    pub caption: Vec<String>,
    pub align: Align,
    /// Modules across the block, quiet zone included.
    pub modules: u32,
    /// Row-major, `true` is dark. `modules * modules` long.
    pub matrix: Vec<bool>,
    /// The corner radius a module is drawn with, as a fraction of the
    /// module across and down, for a preview that works in modules
    /// rather than dots. Zero where the head's dots are too coarse to
    /// round, which is the SP700 at the sizes it is usually asked for.
    pub radius_x: f32,
    pub radius_y: f32,
    /// What the block measures on paper, quiet zone included. The two
    /// differ by under a percent, and only because a module is a whole
    /// number of dots on a head whose dots are not square.
    pub width_mm: f32,
    pub height_mm: f32,
}

/// Scans from a phone at arm's length on either head, and leaves most of
/// an 80 mm roll spare.
fn default_size() -> u8 {
    30
}

/// As round as the dots allow.
fn default_radius() -> u8 {
    MAX_RADIUS
}

impl Default for Qr {
    fn default() -> Self {
        Self {
            data: String::new(),
            caption: None,
            error_correction: Ecc::default(),
            size: default_size(),
            radius: default_radius(),
            align: Align::default(),
        }
    }
}

/// The blank margin the QR specification asks for, in modules. Printed
/// rather than left to the paper, since a caption sits right above it.
const QUIET_MODULES: u32 = 4;

/// Below this nothing scans; above it the code is a poster.
const MIN_SIZE_MM: u8 = 10;
const MAX_SIZE_MM: u8 = 80;

/// A whole half-module of radius, past which a corner would eat into
/// the module beside it rather than round any further.
const MAX_RADIUS: u8 = 100;

/// What differs between the heads when drawing a symbol.
pub trait QrStyle: TextStyle + Sized {
    /// The SP700's dots are only near enough to square at double
    /// density, so that is what a symbol is drawn for. Thermal heads
    /// have no horizontal double density and ignore this.
    const DENSITY: Density;

    /// The head and roll as geometry.
    fn profile(paper: Paper) -> &'static DeviceProfile;

    /// A raster on one head and a bit image on the other.
    fn draw(self, symbol: Bitmap) -> Result<Self, String>;
}

impl QrStyle for Builder<StarLine> {
    const DENSITY: Density = Density::Single;

    fn profile(paper: Paper) -> &'static DeviceProfile {
        match paper {
            Paper::Mm80 => &DeviceProfile::THERMAL_80MM,
            Paper::Mm112 => &DeviceProfile::THERMAL_112MM,
        }
    }

    fn draw(self, symbol: Bitmap) -> Result<Self, String> {
        Ok(self.raster(symbol, RasterQuality::High))
    }
}

impl QrStyle for Builder<Impact> {
    const DENSITY: Density = Density::Double;

    fn profile(_paper: Paper) -> &'static DeviceProfile {
        &DeviceProfile::SP700
    }

    fn draw(self, symbol: Bitmap) -> Result<Self, String> {
        let image = BitImage::new(symbol, Density::Double).map_err(|e| e.to_string())?;
        Ok(self.bit_image(&image))
    }
}

/// Dots per module, across and down.
///
/// At double density the SP700 lays 169 dots to the inch across but only
/// 72 down, so a module that is square on paper is nothing like square
/// in dots. The width is derived from the height to hold the shape, and
/// both shrink together until the symbol and its quiet zone fit the
/// paper.
fn scale(modules: u32, size_mm: f32, profile: &DeviceProfile, density: Density) -> (u32, u32) {
    let ratio = profile.horizontal_dpi_at(density) as f32 / profile.vertical_dpi as f32;
    let across = |down: u32| ((down as f32 * ratio).round() as u32).max(1);

    let total = modules + 2 * QUIET_MODULES;
    let down_per_mm = profile.vertical_dpi as f32 / MM_PER_INCH;
    let mut down = ((size_mm / modules as f32) * down_per_mm).round().max(1.0) as u32;
    while down > 1 && total * across(down) > profile.max_width(density) {
        down -= 1;
    }
    (across(down), down)
}

/// Dots at a head pitch, in millimetres.
fn mm(dots: u32, dpi: f64) -> f32 {
    dots as f32 / (dpi as f32 / MM_PER_INCH)
}

/// Which end of a module a dot sits at, and how far its centre is past
/// the corner arc's, in dots. `None` on the straight side between two
/// corners, which at a radius of nothing is the whole module.
fn corner(dot: u32, span: u32, radius: u32) -> Option<(i32, f32)> {
    let centre = dot as f32 + 0.5;
    if dot < radius {
        Some((-1, centre - radius as f32))
    } else if dot >= span - radius {
        Some((1, centre - (span - radius) as f32))
    } else {
        None
    }
}

/// A module as dots: how many it takes each way, and the corner radius
/// it is drawn with.
#[derive(Debug, Clone, Copy)]
struct Module {
    across: u32,
    down: u32,
    rx: u32,
    ry: u32,
}

impl Module {
    /// `percent` of half the module, taken on each axis separately so
    /// that the arc is round on paper rather than in dots. The SP700
    /// lays a module seven dots across and three down, where a radius
    /// circular in dots would print as an oval.
    ///
    /// Both radii are whole dots. A module with too few to give up,
    /// those three rows at the default size, clips nothing and stays
    /// square of its own accord, whatever was asked for.
    fn new(across: u32, down: u32, percent: u8) -> Self {
        let share = |span: u32| span * u32::from(percent.min(MAX_RADIUS)) / 200;
        Self {
            across,
            down,
            rx: share(across),
            ry: share(down),
        }
    }

    /// Whether a dot falls outside the module's corner arc, and which
    /// corner that is: `-1` or `1` on each axis.
    fn clipped(self, dx: u32, dy: u32) -> Option<(i32, i32)> {
        let (hx, ox) = corner(dx, self.across, self.rx)?;
        let (hy, oy) = corner(dy, self.down, self.ry)?;
        let (u, v) = (ox / self.rx as f32, oy / self.ry as f32);
        (u * u + v * v > 1.0).then_some((hx, hy))
    }

    /// Whether corners round at all: a radius under a whole dot has
    /// nothing to take off.
    fn rounds(self) -> bool {
        self.clipped(0, 0).is_some()
    }

    /// The radius as a fraction of the module, across and down, for a
    /// preview that works in modules rather than dots.
    fn fraction(self) -> (f32, f32) {
        if !self.rounds() {
            return (0.0, 0.0);
        }
        (
            self.rx as f32 / self.across as f32,
            self.ry as f32 / self.down as f32,
        )
    }
}

/// Whether a dot of a dark module is inked once the corners are taken
/// off it.
///
/// A corner rounds only where both of the modules it faces are light,
/// so a run of dark modules stays joined and it is the outside of the
/// run that curves rather than every module in it. Finder patterns are
/// no exception: squaring them was tried, and reads as an oversight on
/// paper.
fn inked(symbol: &Symbol, mx: i32, my: i32, dx: u32, dy: u32, module: Module) -> bool {
    let Some((hx, hy)) = module.clipped(dx, dy) else {
        return true;
    };
    symbol.get_module(mx + hx, my) || symbol.get_module(mx, my + hy)
}

impl Qr {
    fn size_mm(&self) -> f32 {
        f32::from(self.size.clamp(MIN_SIZE_MM, MAX_SIZE_MM))
    }

    /// The encoded symbol and the dots one of its modules takes, across
    /// and down.
    fn encode(
        &self,
        profile: &DeviceProfile,
        density: Density,
    ) -> Result<(Symbol, u32, u32), String> {
        let data = self.data.trim();
        let symbol = Symbol::encode_text(data, self.error_correction.into()).map_err(|_| {
            format!(
                "The QR data is too long to encode: {} characters.",
                data.chars().count()
            )
        })?;
        let (across, down) = scale(symbol.size() as u32, self.size_mm(), profile, density);
        Ok((symbol, across, down))
    }

    /// The symbol as dots, quiet zone included.
    fn symbol(&self, profile: &DeviceProfile, density: Density) -> Result<Bitmap, String> {
        let (symbol, across, down) = self.encode(profile, density)?;
        let module = Module::new(across, down, self.radius);
        let total = symbol.size() as u32 + 2 * QUIET_MODULES;
        let quiet = QUIET_MODULES as i32;
        Ok(Bitmap::from_fn(total * across, total * down, |x, y| {
            let (mx, my) = ((x / across) as i32 - quiet, (y / down) as i32 - quiet);
            symbol.get_module(mx, my) && inked(&symbol, mx, my, x % across, y % down, module)
        }))
    }

    /// The caption wrapped to the paper; empty when there is none.
    fn caption_lines<P: Protocol>(&self, paper: Paper) -> Vec<String>
    where
        Builder<P>: TextStyle,
    {
        self.caption
            .as_deref()
            .map(str::trim)
            .filter(|caption| !caption.is_empty())
            .map(|caption| wrap_by_words(caption, <Builder<P> as TextStyle>::columns(paper)))
            .unwrap_or_default()
    }

    /// The block as modules, with what it measures on paper, for the
    /// preview. The symbol is drawn here rather than by the printer, so
    /// this is the same grid that prints.
    pub fn layout<P: Protocol>(&self, paper: Paper) -> Result<Layout, String>
    where
        Builder<P>: QrStyle,
    {
        let profile = <Builder<P> as QrStyle>::profile(paper);
        let density = <Builder<P> as QrStyle>::DENSITY;
        let (symbol, across, down) = self.encode(profile, density)?;
        let modules = symbol.size() as u32 + 2 * QUIET_MODULES;

        let quiet = QUIET_MODULES as i32;
        let mut matrix = Vec::with_capacity((modules * modules) as usize);
        for y in 0..modules {
            for x in 0..modules {
                matrix.push(symbol.get_module(x as i32 - quiet, y as i32 - quiet));
            }
        }

        let (radius_x, radius_y) = Module::new(across, down, self.radius).fraction();

        Ok(Layout {
            columns: <Builder<P> as TextStyle>::columns(paper),
            caption: self.caption_lines::<P>(paper),
            align: self.align,
            modules,
            matrix,
            radius_x,
            radius_y,
            width_mm: mm(modules * across, profile.horizontal_dpi_at(density)),
            height_mm: mm(modules * down, profile.vertical_dpi),
        })
    }

    /// Without `cut` the symbol only feeds clear of the head.
    pub fn document<P: Protocol>(
        &self,
        builder: Builder<P>,
        paper: Paper,
        cut: bool,
    ) -> Result<Document, String>
    where
        Builder<P>: QrStyle,
    {
        let symbol = self.symbol(
            <Builder<P> as QrStyle>::profile(paper),
            <Builder<P> as QrStyle>::DENSITY,
        )?;

        let mut doc = builder.align(self.align.into());
        for line in self.caption_lines::<P>(paper) {
            doc = doc.line(&line);
        }
        let doc = doc.draw(symbol)?;

        Ok(if cut {
            doc.feed(2).cut(Cut::FeedThenPartial).build()
        } else {
            doc.feed(3).build()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qr(data: &str) -> Qr {
        Qr {
            data: data.to_owned(),
            ..Qr::default()
        }
    }

    #[test]
    fn the_impact_axes_are_scaled_apart_to_keep_the_module_square() {
        // A 29-module symbol at 30 mm wants modules of 1.03 mm.
        let (across, down) = scale(29, 30.0, &DeviceProfile::SP700, Density::Double);
        assert_eq!((across, down), (7, 3));

        let profile = &DeviceProfile::SP700;
        let width = mm(29 * across, profile.horizontal_dpi_at(Density::Double));
        let height = mm(29 * down, profile.vertical_dpi);
        assert!(
            (width - height).abs() < 0.5,
            "square on paper, not in dots: {width} mm by {height} mm"
        );
        assert!(
            (width - 30.0).abs() < 1.0,
            "and the size asked for: {width}"
        );
    }

    #[test]
    fn a_thermal_head_scales_both_axes_alike() {
        // 203.2 DPI both ways, so a module is a square block of dots.
        assert_eq!(
            scale(29, 30.0, &DeviceProfile::THERMAL_80MM, Density::Single),
            (8, 8)
        );
        // 60 mm over 29 modules is 2.07 mm each, 16.6 dots at 8 a millimetre.
        assert_eq!(
            scale(29, 60.0, &DeviceProfile::THERMAL_112MM, Density::Single),
            (17, 17)
        );
    }

    #[test]
    fn a_symbol_too_big_for_the_paper_shrinks_until_it_fits() {
        // 117 modules and a quiet zone is 125, against the SP700's 420
        // dots: the 5 across that 80 mm asks for would need 625.
        let dots = scale(117, 80.0, &DeviceProfile::SP700, Density::Double);
        assert_eq!(dots, (2, 1), "the floor is one dot row a module");
        assert!(125 * dots.0 <= 420);

        // The same symbol has room to spare on 112 mm of thermal paper.
        assert_eq!(
            scale(117, 80.0, &DeviceProfile::THERMAL_112MM, Density::Single),
            (5, 5)
        );
    }

    #[test]
    fn every_symbol_fits_the_head_it_is_drawn_for() {
        let long = "https://example.com/".repeat(40);
        for (profile, density) in [
            (&DeviceProfile::SP700, Density::Double),
            (&DeviceProfile::THERMAL_80MM, Density::Single),
            (&DeviceProfile::THERMAL_112MM, Density::Single),
        ] {
            for data in ["1", "https://example.com/r/42", long.as_str()] {
                let code = Qr {
                    size: MAX_SIZE_MM,
                    ..qr(data)
                };
                let symbol = code.symbol(profile, density).expect("encodes");
                assert!(
                    symbol.width() <= profile.max_width(density),
                    "{} dots on {} for {} characters",
                    symbol.width(),
                    profile.name,
                    data.len()
                );
            }
        }
    }

    #[test]
    fn the_quiet_zone_is_blank_and_the_symbol_starts_after_it() {
        let code = qr("https://example.com/r/42");
        let profile = &DeviceProfile::THERMAL_80MM;
        let (_, across, down) = code.encode(profile, Density::Single).unwrap();
        let bitmap = code.symbol(profile, Density::Single).unwrap();

        for y in 0..QUIET_MODULES * down {
            for x in 0..bitmap.width() {
                assert!(!bitmap.get(x, y), "ink in the quiet zone at {x},{y}");
            }
        }
        // The finder pattern's outer ring is the first module in, and
        // dark. Sampled at its centre, the corner having been rounded.
        assert!(bitmap.get(
            QUIET_MODULES * across + across / 2,
            QUIET_MODULES * down + down / 2
        ));
    }

    #[test]
    fn a_module_rounds_only_the_corners_that_face_blank_paper() {
        let code = qr("https://example.com/r/42");
        let profile = &DeviceProfile::THERMAL_80MM;
        let (symbol, across, down) = code.encode(profile, Density::Single).unwrap();
        assert_eq!((across, down), (10, 10), "dots enough to round");
        let bitmap = code.symbol(profile, Density::Single).unwrap();

        let dot = |mx: i32, my: i32, dx: u32, dy: u32| {
            let quiet = QUIET_MODULES as i32;
            bitmap.get(
                (mx + quiet) as u32 * across + dx,
                (my + quiet) as u32 * down + dy,
            )
        };

        let (mut rounded, mut joined) = (0, 0);
        for my in 0..symbol.size() {
            for mx in 0..symbol.size() {
                if !symbol.get_module(mx, my) {
                    continue;
                }
                // The top-left dot goes when the module has nothing
                // above it or to its left, and stays given either.
                let attached = symbol.get_module(mx - 1, my) || symbol.get_module(mx, my - 1);
                assert_eq!(dot(mx, my, 0, 0), attached, "module {mx},{my}");
                joined += u32::from(attached);
                rounded += u32::from(!attached);
                assert!(dot(mx, my, across / 2, down / 2), "centre {mx},{my}");
            }
        }
        assert!(
            rounded > 0 && joined > 0,
            "both cases occur: {rounded} rounded, {joined} joined"
        );
    }

    #[test]
    fn the_impact_head_keeps_its_modules_square() {
        // A module is three dot rows there, and half of three is one:
        // that dot's centre still falls inside the arc, so the corner
        // survives and the head opts itself out.
        assert!(!Module::new(7, 3, MAX_RADIUS).rounds());
        assert!(
            Module::new(8, 8, MAX_RADIUS).rounds(),
            "a thermal module has the dots to spare"
        );

        let code = qr("https://example.com/r/42");
        let (symbol, across, down) = code.encode(&DeviceProfile::SP700, Density::Double).unwrap();
        assert!(
            !Module::new(across, down, code.radius).rounds(),
            "{across} by {down} dots a module"
        );

        let bitmap = code.symbol(&DeviceProfile::SP700, Density::Double).unwrap();
        let quiet = QUIET_MODULES as i32;
        for my in 0..symbol.size() {
            for mx in 0..symbol.size() {
                if !symbol.get_module(mx, my) {
                    continue;
                }
                let (x, y) = ((mx + quiet) as u32 * across, (my + quiet) as u32 * down);
                assert!(bitmap.get(x, y), "clipped a corner at {mx},{my}");
            }
        }
    }

    #[test]
    fn the_finder_patterns_round_with_everything_else() {
        let code = qr("https://example.com/r/42");
        let profile = &DeviceProfile::THERMAL_80MM;
        let (symbol, across, down) = code.encode(profile, Density::Single).unwrap();
        let bitmap = code.symbol(profile, Density::Single).unwrap();
        assert!(symbol.get_module(0, 0), "the eye's outermost module");

        // Its outside corner faces the quiet zone both ways, so it goes
        // the way any other lone corner does.
        let (x, y) = (QUIET_MODULES * across, QUIET_MODULES * down);
        assert!(!bitmap.get(x, y), "the corner of the eye is taken off");
        assert!(bitmap.get(x + across / 2, y), "the edge between is not");
        assert!(bitmap.get(x, y + down / 2));
    }

    #[test]
    fn the_layout_reports_the_radius_the_dots_were_drawn_with() {
        let code = qr("https://example.com/r/42");

        // 10 dots a module, rounded by 5: half the module.
        let thermal = code.layout::<StarLine>(Paper::Mm80).unwrap();
        assert_eq!((thermal.radius_x, thermal.radius_y), (0.5, 0.5));

        // Square on the SP700, so the preview draws it square too.
        let impact = code.layout::<Impact>(Paper::Mm80).unwrap();
        assert_eq!((impact.radius_x, impact.radius_y), (0.0, 0.0));

        // And square wherever a caller asks for it.
        let square = Qr { radius: 0, ..code };
        let thermal = square.layout::<StarLine>(Paper::Mm80).unwrap();
        assert_eq!((thermal.radius_x, thermal.radius_y), (0.0, 0.0));
    }

    #[test]
    fn the_radius_runs_from_square_to_as_round_as_the_dots_allow() {
        // Ten dots a module: five of radius is the whole half module.
        assert_eq!(Module::new(10, 10, 0).fraction(), (0.0, 0.0));
        assert_eq!(Module::new(10, 10, 50).fraction(), (0.2, 0.2));
        assert_eq!(Module::new(10, 10, 100).fraction(), (0.5, 0.5));
        assert_eq!(
            Module::new(10, 10, 255).fraction(),
            (0.5, 0.5),
            "held at the half module rather than eating the one beside it"
        );
        assert_eq!(
            Module::new(10, 10, 5).fraction(),
            (0.0, 0.0),
            "under a whole dot there is nothing to take off"
        );

        // Rounding costs ink, and asking for more of it costs more.
        let profile = &DeviceProfile::THERMAL_80MM;
        let ink = |radius: u8| {
            let code = Qr {
                radius,
                ..qr("https://example.com/r/42")
            };
            let bitmap = code.symbol(profile, Density::Single).unwrap();
            let dots = (0..bitmap.width()).flat_map(|x| (0..bitmap.height()).map(move |y| (x, y)));
            dots.filter(|&(x, y)| bitmap.get(x, y)).count()
        };
        assert!(ink(0) > ink(50), "square keeps the most");
        assert!(ink(50) > ink(100), "and a fuller radius the least");
    }

    #[test]
    fn data_that_will_not_encode_is_an_error() {
        let code = Qr {
            error_correction: Ecc::H,
            ..qr(&"x".repeat(5000))
        };
        assert_eq!(
            code.symbol(&DeviceProfile::THERMAL_80MM, Density::Single)
                .unwrap_err(),
            "The QR data is too long to encode: 5000 characters."
        );
    }

    #[test]
    fn the_impact_head_prints_a_double_density_bit_image() {
        let document = qr("https://example.com")
            .document(starprint::impact(), Paper::Mm80, true)
            .unwrap();
        let bytes = document.as_bytes();
        assert!(
            bytes.windows(3).any(|w| w == [0x1b, b'^', 1]),
            "ESC ^ 1 is double density: {bytes:02x?}"
        );
        assert!(
            bytes.windows(4).any(|w| w == [0x1b, 0x1d, b'a', 1]),
            "centred"
        );
        assert!(bytes.ends_with(&[0x1b, b'd', 3]), "ends with the cut");
    }

    #[test]
    fn the_thermal_head_rasters_it_rather_than_sending_esc_gs_y() {
        let document = qr("https://example.com")
            .document(starprint::starline(), Paper::Mm80, false)
            .unwrap();
        let bytes = document.as_bytes();
        assert!(
            bytes.windows(4).any(|w| w == [0x1b, b'*', b'r', b'A']),
            "enters raster mode"
        );
        assert!(
            !bytes.windows(4).any(|w| w == [0x1b, 0x1d, b'y', b'P']),
            "and never asks the firmware to print a QR of its own"
        );
        assert!(
            bytes.ends_with(&[0x1b, b'a', 3]),
            "feeds clear without a cut"
        );
    }

    #[test]
    fn a_caption_is_wrapped_above_the_symbol() {
        let mut code = qr("https://example.com");
        code.caption = Some("  Wi-Fi password for the flat  ".to_owned());
        assert_eq!(
            code.caption_lines::<Impact>(Paper::Mm80),
            ["Wi-Fi password for the flat"],
            "trimmed, and short enough for one line"
        );

        let document = code
            .document(starprint::impact(), Paper::Mm80, true)
            .unwrap();
        let bytes = document.as_bytes();
        let caption = b"Wi-Fi password for the flat\n";
        let text = bytes
            .windows(caption.len())
            .position(|w| w == caption)
            .expect("prints the caption");
        let image = bytes
            .windows(2)
            .position(|w| w == [0x1b, b'^'])
            .expect("prints the symbol");
        assert!(text < image, "the caption comes first");

        code.caption = Some("   ".to_owned());
        assert!(
            code.caption_lines::<StarLine>(Paper::Mm80).is_empty(),
            "blank is the same as none"
        );
        assert!(qr("x").caption_lines::<Impact>(Paper::Mm80).is_empty());
    }

    #[test]
    fn alignment_carries_the_caption_with_the_symbol() {
        for (align, code) in [(Align::Left, 0), (Align::Center, 1), (Align::Right, 2)] {
            let document = Qr {
                align,
                caption: Some("Order 42".to_owned()),
                ..qr("https://example.com")
            }
            .document(starprint::impact(), Paper::Mm80, true)
            .unwrap();
            let bytes = document.as_bytes();

            // `ESC GS a` is set once, before the caption, so both the
            // text and the symbol under it move together.
            let alignments: Vec<u8> = bytes
                .windows(4)
                .filter(|w| w[..3] == [0x1b, 0x1d, b'a'])
                .map(|w| w[3])
                .collect();
            assert_eq!(alignments, [code], "{align:?}");
            let at = bytes
                .windows(3)
                .position(|w| w[..3] == [0x1b, 0x1d, b'a'])
                .unwrap();
            let caption = bytes.windows(8).position(|w| w == b"Order 42").unwrap();
            assert!(at < caption, "{align:?} is set before the caption prints");
        }
    }

    #[test]
    fn the_layout_is_the_grid_that_prints() {
        let code = Qr {
            caption: Some("Order 42".to_owned()),
            align: Align::Right,
            ..qr("https://example.com/r/42")
        };
        let layout = code.layout::<StarLine>(Paper::Mm80).unwrap();

        assert_eq!(layout.columns, 48);
        assert_eq!(layout.caption, ["Order 42"]);
        assert_eq!(layout.align, Align::Right);
        assert_eq!(
            layout.matrix.len(),
            (layout.modules * layout.modules) as usize
        );

        // The same modules the bitmap is drawn from, and the same size.
        let (symbol, across, down) = code
            .encode(&DeviceProfile::THERMAL_80MM, Density::Single)
            .unwrap();
        assert_eq!(layout.modules, symbol.size() as u32 + 2 * QUIET_MODULES);
        let bitmap = code
            .symbol(&DeviceProfile::THERMAL_80MM, Density::Single)
            .unwrap();
        assert_eq!(bitmap.width(), layout.modules * across);
        // Sampled at the centre, which rounding never takes off.
        for (index, dark) in layout.matrix.iter().enumerate() {
            let (x, y) = (index as u32 % layout.modules, index as u32 / layout.modules);
            let dot = bitmap.get(x * across + across / 2, y * down + down / 2);
            assert_eq!(*dark, dot, "module {x},{y}");
        }

        // 8 dots to the millimetre both ways, so the block is square.
        assert_eq!(layout.width_mm, layout.height_mm);
        // The block carries a 4-module quiet zone each side, so it is
        // the symbol itself that should measure what was asked for.
        let module_mm = layout.width_mm / layout.modules as f32;
        let symbol_mm = module_mm * (layout.modules - 2 * QUIET_MODULES) as f32;
        assert!(
            (symbol_mm - 30.0).abs() < 2.0,
            "the size asked for: {symbol_mm} mm"
        );
    }

    #[test]
    fn a_layout_reports_data_it_cannot_encode() {
        let code = Qr {
            error_correction: Ecc::H,
            ..qr(&"x".repeat(5000))
        };
        assert!(code.layout::<Impact>(Paper::Mm80).is_err());
    }

    #[test]
    fn the_size_is_clamped_to_something_printable() {
        assert_eq!(Qr { size: 0, ..qr("x") }.size_mm(), f32::from(MIN_SIZE_MM));
        assert_eq!(
            Qr {
                size: 255,
                ..qr("x")
            }
            .size_mm(),
            f32::from(MAX_SIZE_MM)
        );
    }
}
