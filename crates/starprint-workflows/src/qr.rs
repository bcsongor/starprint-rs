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
use starprint::{Alignment, Builder, Document, Impact, Protocol, RasterQuality, StarLine};

use crate::text::{TextStyle, wrap_by_words};
use crate::{MM_PER_INCH, Paper, finish_graphic};

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
    /// Corner radius as a percentage of 1.5 modules, capped at half the
    /// shorter adjoining edge. A lone module becomes a circle near 33.
    /// Larger blocks keep rounding up to 100. Defaults to zero: square corners.
    #[serde(default)]
    pub radius: u8,
    /// Carries the caption with it.
    #[serde(default)]
    pub align: Align,
}

/// Where the code sits and what it measures, for the preview. The dots
/// themselves come from [`Qr::bitmap`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub columns: usize,
    /// Wrapped to the paper; empty when there is no caption.
    pub caption: Vec<String>,
    pub align: Align,
    /// Modules across the block, quiet zone included.
    pub modules: u32,
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

impl Default for Qr {
    fn default() -> Self {
        Self {
            data: String::new(),
            caption: None,
            error_correction: Ecc::default(),
            size: default_size(),
            radius: 0,
            align: Align::default(),
        }
    }
}

/// The blank margin the QR specification asks for, in modules. Printed
/// rather than left to the paper, since a caption sits right above it.
const QUIET_MODULES: u32 = 4;

const MIN_SIZE_MM: u8 = 10;
const MAX_SIZE_MM: u8 = 80;

/// The dial's top, as a percentage of [`MAX_RADIUS_MODULES`].
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
    fn draw(self, symbol: Bitmap, paper: Paper, align: Align) -> Result<Self, String>;
}

impl QrStyle for Builder<StarLine> {
    const DENSITY: Density = Density::Single;

    fn profile(paper: Paper) -> &'static DeviceProfile {
        paper.profile()
    }

    fn draw(self, symbol: Bitmap, paper: Paper, align: Align) -> Result<Self, String> {
        // Raster rows start at their own left margin, ignoring ESC GS a
        // (Star Line Mode 3-73, 3-81). Blank dots position the symbol.
        let spare = Self::profile(paper)
            .max_width(Self::DENSITY)
            .saturating_sub(symbol.width());
        let left = match align {
            Align::Left => 0,
            Align::Center => spare / 2,
            Align::Right => spare,
        };
        let placed = Bitmap::from_fn(symbol.width() + left, symbol.height(), |x, y| {
            x >= left && symbol.get(x - left, y)
        });
        Ok(self.raster(placed, RasterQuality::High))
    }
}

impl QrStyle for Builder<Impact> {
    const DENSITY: Density = Density::Double;

    fn profile(_paper: Paper) -> &'static DeviceProfile {
        &DeviceProfile::SP700
    }

    fn draw(self, symbol: Bitmap, _paper: Paper, _align: Align) -> Result<Self, String> {
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

/// The largest corner radius, in modules. An arc much past 1.7 cuts the
/// centre of the module in the corner, which is the point a scanner
/// samples. This leaves a margin, and happens to make a circle of the
/// 3 by 3 block at the heart of each finder pattern.
const MAX_RADIUS_MODULES: f32 = 1.5;

/// A corner of the dark shape, found at a grid vertex where one of the
/// four modules around it differs from the other three.
struct Corner {
    /// The vertex, in modules from the symbol's top-left.
    vx: i32,
    vy: i32,
    /// The quadrant the odd module is in: `-1` or `1` on each axis.
    sx: i32,
    sy: i32,
    /// A dark module among light ones is a convex corner and loses
    /// ink; a light one among dark is concave and gains it.
    convex: bool,
    /// In modules. What was asked for, or less where the edges that
    /// meet here are short.
    radius: f32,
}

/// Every corner the shape has, with the radius each can take: half the
/// shorter of the two edges that meet there at most, so a lone module
/// stops at its circle while a finder pattern's ring keeps rounding.
/// Where two dark modules touch only at a point, neither light corner
/// between them is filled, so rounding never joins them.
fn corners(symbol: &Symbol, radius: f32) -> Vec<Corner> {
    let n = symbol.size();
    let dark = |x: i32, y: i32| symbol.get_module(x, y);
    let mut out = Vec::new();
    for vy in 0..=n {
        for vx in 0..=n {
            // The module in a quadrant of this vertex.
            let module = |sx: i32, sy: i32| (vx + (sx - 1) / 2, vy + (sy - 1) / 2);
            let at = |sx: i32, sy: i32| {
                let (x, y) = module(sx, sy);
                dark(x, y)
            };
            for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let (this, beside, above, across) =
                    (at(sx, sy), at(-sx, sy), at(sx, -sy), at(-sx, -sy));
                let convex = this && !beside && !above;
                let concave = !this && beside && above && across;
                if !convex && !concave {
                    continue;
                }

                // How far this module's colour runs along each edge
                // from the vertex, with the other colour across it.
                let (mx, my) = module(sx, sy);
                let along_x = (0..)
                    .take_while(|&k| {
                        dark(mx + k * sx, my) == this && dark(mx + k * sx, my - sy) != this
                    })
                    .count();
                let along_y = (0..)
                    .take_while(|&k| {
                        dark(mx, my + k * sy) == this && dark(mx - sx, my + k * sy) != this
                    })
                    .count();
                out.push(Corner {
                    vx,
                    vy,
                    sx,
                    sy,
                    convex,
                    radius: radius.min(along_x.min(along_y) as f32 / 2.0),
                });
            }
        }
    }
    // Ink comes off before it goes on.
    out.sort_by_key(|corner| !corner.convex);
    out
}

impl Corner {
    /// Takes the corner off, or fills it in, dot by dot.
    ///
    /// The radius is scaled to each axis so that the arc is round on
    /// paper rather than in dots. The SP700 lays a module seven dots
    /// across and three down, where an arc circular in dots would print
    /// as an oval.
    fn apply(&self, dots: &mut [bool], width: u32, across: u32, down: u32) {
        let (rx, ry) = (self.radius * across as f32, self.radius * down as f32);
        let quiet = QUIET_MODULES as i32;
        let (vx, vy) = (
            (self.vx + quiet) * across as i32,
            (self.vy + quiet) * down as i32,
        );
        for j in 0..ry.ceil() as i32 {
            for i in 0..rx.ceil() as i32 {
                let (u, v) = ((rx - i as f32 - 0.5) / rx, (ry - j as f32 - 0.5) / ry);
                if u > 0.0 && v > 0.0 && u * u + v * v > 1.0 {
                    let x = if self.sx > 0 { vx + i } else { vx - 1 - i };
                    let y = if self.sy > 0 { vy + j } else { vy - 1 - j };
                    dots[(y as u32 * width + x as u32) as usize] = !self.convex;
                }
            }
        }
    }
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
        let total = symbol.size() as u32 + 2 * QUIET_MODULES;
        let (width, height) = (total * across, total * down);
        let quiet = QUIET_MODULES as i32;
        let mut dots: Vec<bool> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| symbol.get_module((x / across) as i32 - quiet, (y / down) as i32 - quiet))
            .collect();

        let radius = MAX_RADIUS_MODULES * f32::from(self.radius.min(MAX_RADIUS)) / 100.0;
        for corner in corners(&symbol, radius) {
            corner.apply(&mut dots, width, across, down);
        }
        Ok(Bitmap::from_fn(width, height, |x, y| {
            dots[(y * width + x) as usize]
        }))
    }

    /// The symbol as it will print, for a preview drawn from the dots
    /// themselves rather than from a rule of its own.
    pub fn bitmap<P: Protocol>(&self, paper: Paper) -> Result<Bitmap, String>
    where
        Builder<P>: QrStyle,
    {
        self.symbol(
            <Builder<P> as QrStyle>::profile(paper),
            <Builder<P> as QrStyle>::DENSITY,
        )
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

    /// The caption and what the block measures on paper, for the
    /// preview to place the symbol by.
    pub fn layout<P: Protocol>(&self, paper: Paper) -> Result<Layout, String>
    where
        Builder<P>: QrStyle,
    {
        let profile = <Builder<P> as QrStyle>::profile(paper);
        let density = <Builder<P> as QrStyle>::DENSITY;
        let (symbol, across, down) = self.encode(profile, density)?;
        let modules = symbol.size() as u32 + 2 * QUIET_MODULES;

        Ok(Layout {
            columns: <Builder<P> as TextStyle>::columns(paper),
            caption: self.caption_lines::<P>(paper),
            align: self.align,
            modules,
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
        let symbol = self.bitmap::<P>(paper)?;

        let mut doc = builder.align(self.align.into());
        for line in self.caption_lines::<P>(paper) {
            doc = doc.line(&line);
        }
        let doc = doc.draw(symbol, paper, self.align)?;
        Ok(finish_graphic(doc, cut))
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
        // The finder pattern starts with a square corner by default.
        assert!(bitmap.get(QUIET_MODULES * across, QUIET_MODULES * down));
    }

    /// A head to draw for: its profile and the density a symbol uses.
    type Head = (&'static DeviceProfile, Density);
    const THERMAL: Head = (&DeviceProfile::THERMAL_80MM, Density::Single);
    const IMPACT: Head = (&DeviceProfile::SP700, Density::Double);

    /// Dots inked in the symbol at a given radius.
    fn ink(head: Head, radius: u8) -> usize {
        let code = Qr {
            radius,
            ..qr("https://example.com/r/42")
        };
        let bitmap = code.symbol(head.0, head.1).unwrap();
        let dots = (0..bitmap.width()).flat_map(|x| (0..bitmap.height()).map(move |y| (x, y)));
        dots.filter(|&(x, y)| bitmap.get(x, y)).count()
    }

    /// The dot `dx`,`dy` into module `mx`,`my` of the symbol.
    fn dot(bitmap: &Bitmap, across: u32, down: u32) -> impl Fn(i32, i32, u32, u32) -> bool + '_ {
        move |mx, my, dx, dy| {
            let quiet = QUIET_MODULES as i32;
            bitmap.get(
                (mx + quiet) as u32 * across + dx,
                (my + quiet) as u32 * down + dy,
            )
        }
    }

    #[test]
    fn no_module_centre_is_ever_touched() {
        // The centre is what a scanner samples, so however far the
        // corners go, ink comes off dark modules and goes onto light
        // ones only at their edges.
        for head in [THERMAL, IMPACT] {
            for radius in [0, 33, 66, 100] {
                let code = Qr {
                    radius,
                    ..qr("https://example.com/r/42")
                };
                let (symbol, across, down) = code.encode(head.0, head.1).unwrap();
                let bitmap = code.symbol(head.0, head.1).unwrap();
                let dot = dot(&bitmap, across, down);
                for my in 0..symbol.size() {
                    for mx in 0..symbol.size() {
                        assert_eq!(
                            dot(mx, my, across / 2, down / 2),
                            symbol.get_module(mx, my),
                            "module {mx},{my} at {radius} on {}",
                            head.0.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_lone_module_stops_at_its_circle_while_the_eye_keeps_going() {
        let code = Qr {
            radius: MAX_RADIUS,
            ..qr("https://example.com/r/42")
        };
        let (symbol, across, down) = code.encode(THERMAL.0, THERMAL.1).unwrap();
        let bitmap = code.symbol(THERMAL.0, THERMAL.1).unwrap();
        let dot = dot(&bitmap, across, down);

        // A module with light on all four sides is capped at half a
        // module of radius, so the middle of its side is on the arc
        // and survives; a radius past that would have taken it.
        let n = symbol.size();
        let (lx, ly) = (0..n)
            .flat_map(|y| (0..n).map(move |x| (x, y)))
            .find(|&(x, y)| {
                symbol.get_module(x, y)
                    && !symbol.get_module(x - 1, y)
                    && !symbol.get_module(x + 1, y)
                    && !symbol.get_module(x, y - 1)
                    && !symbol.get_module(x, y + 1)
            })
            .expect("a lone module somewhere in the data");
        assert!(!dot(lx, ly, 0, 0), "its corner goes");
        assert!(dot(lx, ly, 0, down / 2), "the middle of its side stays");

        // The 3 by 3 block at the eye's centre has edges of three, so
        // its corners take the full radius and it becomes a circle:
        // half way along the corner module's top edge is still outside.
        assert!(!dot(2, 2, 0, 0));
        assert!(!dot(2, 2, across / 2, 0));
        assert!(dot(3, 2, across / 2, 0), "the block's top-middle is on it");
        assert!(dot(2, 2, across / 2, down / 2));
    }

    #[test]
    fn the_eye_ring_rounds_outside_and_fills_inside() {
        let code = Qr {
            radius: MAX_RADIUS,
            ..qr("https://example.com/r/42")
        };
        let (_, across, down) = code.encode(THERMAL.0, THERMAL.1).unwrap();
        let bitmap = code.symbol(THERMAL.0, THERMAL.1).unwrap();
        let dot = dot(&bitmap, across, down);

        // The ring's outer edges run seven modules, so its corner takes
        // an arc wider than the module and the next module along loses
        // the start of its edge too.
        assert!(!dot(0, 0, 0, 0));
        assert!(!dot(1, 0, 0, 0), "the neighbour's corner goes with it");
        assert!(dot(1, 0, across / 2, 0), "but not its middle");

        // The hollow's corner is a concave corner of the ring and is
        // filled, short of the light module's centre.
        assert!(dot(1, 1, 0, 0), "filled in");
        assert!(!dot(1, 1, across / 2, down / 2), "but not to the centre");
        // The hollow's corner beside the centre block is left alone:
        // that vertex has two dark modules and two light.
        assert!(!dot(1, 1, across - 1, down - 1));
    }

    #[test]
    fn a_coarser_module_is_slower_to_round() {
        // The first radius at which a head's symbol changes at all: the
        // SP700's 7 by 3 needs a wider arc than a 10 by 10 before its
        // outermost dot's centre falls outside it.
        let first = |head: Head| (1..=MAX_RADIUS).find(|&pct| ink(head, pct) != ink(head, 0));
        assert_eq!(first(THERMAL), Some(12));
        assert_eq!(first(IMPACT), Some(27));
    }

    #[test]
    fn the_radius_runs_from_square_to_as_round_as_each_shape_allows() {
        assert!(ink(THERMAL, 0) > ink(THERMAL, 33), "corners cost ink");
        assert_eq!(ink(THERMAL, 255), ink(THERMAL, 100), "held at the top");

        // With large blocks rounding past a module, the dial has many
        // shapes on it rather than the handful a lone module has.
        let shapes: std::collections::BTreeSet<usize> =
            (0..=MAX_RADIUS).map(|pct| ink(THERMAL, pct)).collect();
        assert!(shapes.len() >= 20, "only {} shapes", shapes.len());
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
    fn thermal_raster_alignment_matches_the_print_area() {
        for paper in [Paper::Mm80, Paper::Mm112] {
            let profile = <Builder<StarLine> as QrStyle>::profile(paper);
            let width = profile.max_width(Density::Single);
            for align in [Align::Left, Align::Center, Align::Right] {
                let code = Qr { align, ..qr("x") };
                let (_, across, down) = code.encode(profile, Density::Single).unwrap();
                let document = code.document(starprint::starline(), paper, false).unwrap();
                let bytes = document.as_bytes();
                let start = bytes.windows(4).position(|w| w == b"\x1b*rA").unwrap();
                let raster = &bytes[start..];
                // Skip the NUL-terminated ESC * r settings, so that a
                // setting byte is never taken for the first row command.
                let mut start = 4;
                while raster[start..].starts_with(&[0x1b, b'*']) {
                    start += raster[start..].iter().position(|&byte| byte == 0).unwrap() + 1;
                }
                assert_eq!(raster[start], b'b');
                let row_bytes = u16::from_le_bytes([raster[start + 1], raster[start + 2]]) as usize;

                // The first ink row crosses both finder patterns, so its
                // outermost dots measure the margins of the whole symbol.
                let row = start + (QUIET_MODULES * down) as usize * (3 + row_bytes) + 3;
                let ink: Vec<u32> = raster[row..row + row_bytes]
                    .iter()
                    .enumerate()
                    .flat_map(|(x, byte)| {
                        (0..8).filter_map(move |bit| {
                            (byte & (0x80 >> bit) != 0).then_some(x as u32 * 8 + bit)
                        })
                    })
                    .collect();
                let left = *ink.first().unwrap();
                let right = width - ink.last().unwrap() - 1;
                match align {
                    Align::Left => assert_eq!(left, QUIET_MODULES * across),
                    Align::Center => {
                        assert!(left.abs_diff(right) <= 1, "{paper:?}: {left}, {right}")
                    }
                    Align::Right => assert_eq!(right, QUIET_MODULES * across),
                }
            }
        }
    }

    #[test]
    fn the_layout_measures_the_block_that_prints() {
        let code = Qr {
            caption: Some("Order 42".to_owned()),
            align: Align::Right,
            ..qr("https://example.com/r/42")
        };
        let layout = code.layout::<StarLine>(Paper::Mm80).unwrap();

        assert_eq!(layout.columns, 48);
        assert_eq!(layout.caption, ["Order 42"]);
        assert_eq!(layout.align, Align::Right);

        // The same block the bitmap is drawn to, and the same size.
        let (symbol, across, _) = code
            .encode(&DeviceProfile::THERMAL_80MM, Density::Single)
            .unwrap();
        assert_eq!(layout.modules, symbol.size() as u32 + 2 * QUIET_MODULES);
        let bitmap = code.bitmap::<StarLine>(Paper::Mm80).unwrap();
        assert_eq!(bitmap.width(), layout.modules * across);

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
