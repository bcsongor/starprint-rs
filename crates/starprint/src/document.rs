//! Protocol-aware document building.
//!
//! A [`Builder`] renders a receipt into the raw command stream of one of
//! the two supported Star command sets, chosen at the type level:
//!
//! * [`StarLine`] is **Star Line Mode**, the native command set of Star
//!   thermal receipt printers (TSP100 Line Mode, TSP650II, TSP700II,
//!   TSP800II, …), per Star's *Line Thermal Printer, Star Line Mode
//!   Command Specifications*.
//! * [`Impact`] is **Star Mode for dot impact printers**, the native
//!   command set of the SP700 series (SP712, SP742, SP717, SP747), per
//!   Star's *Dot Impact Printer, STAR Command Specifications*. It shares
//!   most of Star Line Mode's vocabulary but adds two-colour (red/black)
//!   printing and omits thermal-only features such as barcodes.
//!
//! Commands only exist on the builder whose printer supports them, so an
//! unsupported command is a compile error: [`Builder::qr_code`] is
//! thermal-only, `double_wide` is impact-only.

use std::marker::PhantomData;

use crate::code::{Barcode, QrCode, QrErrorCorrection, QrModel};
use crate::cp437;
use crate::graphics::{self, BitImage, Bitmap, Density};
use crate::types::{
    Alignment, CodePage, Color, Cut, Drawer, ImpactFont, InternationalCharset, LineSpacing,
    PrintMode, PrintSpeed, RasterQuality, ThermalFont,
};

const ESC: u8 = 0x1B;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::StarLine {}
    impl Sealed for super::Impact {}
}

/// A Star command set targeted by a [`Builder`].
///
/// This trait is sealed; the two implementations are [`StarLine`] and
/// [`Impact`].
pub trait Protocol: sealed::Sealed + 'static {}

/// Marker for **Star Line Mode**, the thermal receipt printers.
///
/// Build documents for it with [`crate::starline()`].
#[derive(Debug)]
pub enum StarLine {}

impl Protocol for StarLine {}

/// Marker for **Star Mode on dot impact printers**, the SP700 series of
/// two-colour dot-matrix kitchen printers.
///
/// Build documents for it with [`crate::impact()`].
#[derive(Debug)]
pub enum Impact {}

impl Protocol for Impact {}

/// A rendered command stream, ready for a
/// [`Transport`](crate::transport::Transport).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    bytes: Vec<u8>,
}

impl Document {
    /// The rendered command stream.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the document, returning the rendered command stream.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl AsRef<[u8]> for Document {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl From<Document> for Vec<u8> {
    fn from(doc: Document) -> Self {
        doc.bytes
    }
}

/// Builds a [`Document`] one command at a time. `P` decides which
/// commands exist and how they are encoded.
///
/// # Examples
///
/// ```
/// use starprint::{Alignment, Cut};
///
/// let doc = starprint::starline()
///     .align(Alignment::Center)
///     .wide(2)
///     .tall(2)
///     .line("ACME STORE")
///     .wide(1)
///     .tall(1)
///     .align(Alignment::Left)
///     .line("1x Coffee            3.50")
///     .feed(2)
///     .cut(Cut::FeedThenPartial)
///     .build();
/// # let _ = doc;
/// ```
#[derive(Debug, Clone)]
pub struct Builder<P: Protocol> {
    buf: Vec<u8>,
    _protocol: PhantomData<P>,
}

impl<P: Protocol> Default for Builder<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: Protocol> Builder<P> {
    /// Starts a document with `ESC @` and the CP437 code page that
    /// [`text`](Self::text) encodes for.
    ///
    /// `ESC @` does not reset everything: drawer pulse settings, print
    /// mode and the impact printers' two-colour mode survive it.
    #[must_use]
    pub fn new() -> Self {
        Self::without_init()
            .raw([ESC, b'@'])
            .code_page(CodePage::CP437)
    }

    /// Starts a document without the reset, to continue a job whose
    /// styling an earlier document set up.
    #[must_use]
    pub fn without_init() -> Self {
        Self {
            buf: Vec::with_capacity(256),
            _protocol: PhantomData,
        }
    }

    /// Continues a finished document where it ended, without copying it.
    #[must_use]
    pub fn resume(document: Document) -> Self {
        Self {
            buf: document.into_bytes(),
            _protocol: PhantomData,
        }
    }

    /// Appends bytes verbatim, for commands this builder does not model
    /// or text pre-encoded for another code page.
    #[must_use]
    pub fn raw(mut self, bytes: impl AsRef<[u8]>) -> Self {
        self.buf.extend_from_slice(bytes.as_ref());
        self
    }

    /// Prints text encoded as CP437; characters it cannot represent print
    /// as `?`. `'\n'` prints and feeds the line. For another code page,
    /// select it with [`code_page`](Self::code_page) and send pre-encoded
    /// bytes with [`raw`](Self::raw).
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        cp437::encode_into(text, &mut self.buf);
        self
    }

    /// Prints text followed by a line feed.
    #[must_use]
    pub fn line(self, text: &str) -> Self {
        self.text(text).raw(b"\n")
    }

    /// Feeds `lines` blank lines (`ESC a n`), clamped to 1–127.
    #[must_use]
    pub fn feed(self, lines: u8) -> Self {
        self.raw([ESC, b'a', lines.clamp(1, 127)])
    }

    /// Sets the alignment of subsequent lines (`ESC GS a n`). Takes effect
    /// at the start of a line.
    #[must_use]
    pub fn align(self, alignment: Alignment) -> Self {
        self.raw([ESC, 0x1D, b'a', alignment.code()])
    }

    /// Switches emphasized (bold) printing on or off (`ESC E` / `ESC F`).
    ///
    /// Impact printers print emphasized text in a second pass, halving
    /// print speed.
    #[must_use]
    pub fn bold(self, on: bool) -> Self {
        self.raw([ESC, if on { b'E' } else { b'F' }])
    }

    /// Switches underlining on or off (`ESC - n`).
    #[must_use]
    pub fn underline(self, on: bool) -> Self {
        self.raw([ESC, b'-', on as u8])
    }

    /// Switches the overline (upper line) on or off (`ESC _ n`).
    #[must_use]
    pub fn overline(self, on: bool) -> Self {
        self.raw([ESC, b'_', on as u8])
    }

    /// Selects an international character-set variant (`ESC R n`).
    #[must_use]
    pub fn international(self, set: InternationalCharset) -> Self {
        self.raw([ESC, b'R', set.code()])
    }

    /// Selects the printer's code page (`ESC GS t n`). This changes how the
    /// printer reads bytes ≥ 0x80, not how [`text`](Self::text) encodes.
    #[must_use]
    pub fn code_page(self, page: CodePage) -> Self {
        self.raw([ESC, 0x1D, b't', page.0])
    }

    /// Cuts the paper (`ESC d n`). Tear-bar models (SP712, SP717) ignore
    /// the non-feeding variants and feed to the tear bar for the others.
    #[must_use]
    pub fn cut(self, cut: Cut) -> Self {
        self.raw([ESC, b'd', cut.code()])
    }

    /// Pulses a cash drawer (`BEL` / `SUB`). Drawer 1 uses the timing from
    /// [`drawer_pulse`](Self::drawer_pulse); drawer 2 is fixed at 200 ms.
    #[must_use]
    pub fn open_drawer(self, drawer: Drawer) -> Self {
        match drawer {
            Drawer::One => self.raw([0x07]),
            Drawer::Two => self.raw([0x1A]),
        }
    }

    /// Sets drawer 1's pulse (`ESC BEL n1 n2`) in units of 10 ms, clamped
    /// to 1–127. The printer default is 20/20.
    #[must_use]
    pub fn drawer_pulse(self, on_10ms: u8, off_10ms: u8) -> Self {
        self.raw([ESC, 0x07, on_10ms.clamp(1, 127), off_10ms.clamp(1, 127)])
    }

    #[must_use]
    pub fn build(self) -> Document {
        Document { bytes: self.buf }
    }
}

impl Builder<StarLine> {
    /// Sets the character width multiplier (`ESC W n`), clamped to 1–6.
    #[must_use]
    pub fn wide(self, multiplier: u8) -> Self {
        self.raw([ESC, b'W', multiplier.clamp(1, 6) - 1])
    }

    /// Sets the character height multiplier (`ESC h n`), clamped to 1–6.
    #[must_use]
    pub fn tall(self, multiplier: u8) -> Self {
        self.raw([ESC, b'h', multiplier.clamp(1, 6) - 1])
    }

    /// Switches white-on-black printing on or off (`ESC 4` / `ESC 5`).
    /// On impact printers the same bytes select red/black instead.
    #[must_use]
    pub fn invert(self, on: bool) -> Self {
        self.raw([ESC, if on { b'4' } else { b'5' }])
    }

    /// Selects the base character font (`ESC RS F n`).
    #[must_use]
    pub fn font(self, font: ThermalFont) -> Self {
        self.raw([ESC, 0x1E, b'F', font.code()])
    }

    /// Sets the line-feed pitch (`ESC z n`).
    #[must_use]
    pub fn line_spacing(self, spacing: LineSpacing) -> Self {
        let n = match spacing {
            LineSpacing::ThreeMm => 0,
            LineSpacing::FourMm => 1,
        };
        self.raw([ESC, b'z', n])
    }

    /// Selects the print mode (`ESC RS C n`).
    ///
    /// The printer flushes its line buffer before changing mode, so a
    /// document may switch part-way through. The setting **survives
    /// `ESC @`** and the job: switch back to [`PrintMode::SingleColor`] at
    /// the end, and select the mode you want at the start rather than
    /// trusting what the last job left.
    ///
    /// Double resolution is for rasters prepared at twice the rows; the
    /// printer's fonts come out half height in it.
    #[must_use]
    pub fn print_mode(self, mode: PrintMode) -> Self {
        self.raw([ESC, 0x1E, b'C', mode.code()])
    }

    /// Selects the text colour in two-colour mode (`ESC RS c n`).
    /// Ignored outside it. The setting survives `ESC @`.
    #[must_use]
    pub fn color(self, color: Color) -> Self {
        self.raw([ESC, 0x1E, b'c', (color == Color::Red) as u8])
    }

    /// Sets the line-mode print speed (`ESC RS r n`). Ignored in
    /// double-resolution, two-colour and low-power modes; rasters use
    /// [`RasterQuality`] instead.
    #[must_use]
    pub fn print_speed(self, speed: PrintSpeed) -> Self {
        self.raw([ESC, 0x1E, b'r', speed.code()])
    }

    /// Sets print density (`ESC RS d n`), `-3` to `+3` around the memory
    /// switch standard, clamped. The setting survives `ESC @`.
    ///
    /// `+2`/`+3` fills solid areas that pinhole at the default. In
    /// double-resolution mode `+3` is 1.2× standard rather than 1.3×.
    #[must_use]
    pub fn print_density(self, level: i8) -> Self {
        // The command counts the other way: n = 0 is +3, n = 3 is standard,
        // n = 6 is -3.
        let n = (3 - level.clamp(-3, 3)) as u8;
        self.raw([ESC, 0x1E, b'd', n])
    }

    /// Prints a black bitmap in raster mode (`ESC * r`), one dot row per
    /// command, so there is no stripe banding. The printer crops rows
    /// wider than its print area.
    ///
    /// The printer buffers about 2,560 rows (TSP800II) and pauses to print
    /// them when full, which leaves a faint line across a picture longer
    /// than ~320 mm (~160 mm in double resolution).
    ///
    /// For double resolution, prepare the image with a
    /// `*_DOUBLE_RESOLUTION` [`DeviceProfile`](crate::graphics::DeviceProfile)
    /// and select [`PrintMode::DoubleResolution`] first.
    #[must_use]
    pub fn raster(self, image: impl AsRef<Bitmap>, quality: RasterQuality) -> Self {
        self.raster_color(image, quality, Color::Black)
    }

    /// Prints a bitmap with the given raster colour (`ESC * r K n NUL`).
    /// Red requires [`PrintMode::TwoColor`]; otherwise colour is ignored.
    /// Text colour is set separately by [`Self::color`].
    #[must_use]
    pub fn raster_color(
        mut self,
        image: impl AsRef<Bitmap>,
        quality: RasterQuality,
        color: Color,
    ) -> Self {
        let bitmap = image.as_ref();
        if bitmap.width() == 0 || bitmap.height() == 0 {
            return self;
        }
        // Entering raster mode resets its settings, so they follow ESC * r A.
        // Numeric parameters are ASCII digits terminated by NUL.
        self.buf.extend_from_slice(&[ESC, b'*', b'r', b'A']);
        self.buf
            .extend_from_slice(&[ESC, b'*', b'r', b'P', b'0', 0]); // continuous length
        self.buf
            .extend_from_slice(&[ESC, b'*', b'r', b'E', b'1', 0]); // EOT: print only
        self.buf
            .extend_from_slice(&[ESC, b'*', b'r', b'Q', quality.code(), 0]);
        // Raster colour survives both raster entry and ESC @.
        let color = if color == Color::Red { b'1' } else { b'0' };
        self.buf
            .extend_from_slice(&[ESC, b'*', b'r', b'K', color, 0]);

        let bytes_per_row = bitmap.width().div_ceil(8);
        let [n1, n2] = (bytes_per_row as u16).to_le_bytes();
        self.buf
            .reserve(bitmap.height() as usize * (3 + bytes_per_row as usize) + 4);
        for y in 0..bitmap.height() {
            self.buf.extend_from_slice(&[b'b', n1, n2]);
            graphics::pack_row(bitmap, y, &mut self.buf);
        }
        self.buf.extend_from_slice(&[ESC, b'*', b'r', b'B']);
        self
    }

    /// Prints a barcode (`ESC b n1 n2 n3 n4 … RS`) at the current
    /// alignment. The printer feeds past it before the next line.
    #[must_use]
    pub fn barcode(mut self, barcode: &Barcode) -> Self {
        // n2: 1 = no HRI text, 2 = HRI text below; both feed after the bars.
        self.buf.extend_from_slice(&[
            ESC,
            b'b',
            barcode.symbology.code(),
            if barcode.hri { b'2' } else { b'1' },
            b'0' + barcode.module,
            barcode.height,
        ]);
        self.buf.extend_from_slice(&barcode.data);
        self.buf.push(0x1E); // RS
        self
    }

    /// Prints a QR code (`ESC GS y`) at the current alignment.
    #[must_use]
    pub fn qr_code(mut self, qr: &QrCode) -> Self {
        let model = match qr.model {
            QrModel::Model1 => 1,
            QrModel::Model2 => 2,
        };
        let ec = match qr.ec {
            QrErrorCorrection::L => 0,
            QrErrorCorrection::M => 1,
            QrErrorCorrection::Q => 2,
            QrErrorCorrection::H => 3,
        };
        let len = (qr.data.len() as u16).to_le_bytes(); // MAX_DATA fits in u16.
        self.buf
            .extend_from_slice(&[ESC, 0x1D, b'y', b'S', b'0', model]);
        self.buf
            .extend_from_slice(&[ESC, 0x1D, b'y', b'S', b'1', ec]);
        self.buf
            .extend_from_slice(&[ESC, 0x1D, b'y', b'S', b'2', qr.cell_size]);
        // ESC GS y D 1 m nL nH d1…dk, m = 0 for automatic encoding.
        self.buf
            .extend_from_slice(&[ESC, 0x1D, b'y', b'D', b'1', 0, len[0], len[1]]);
        self.buf.extend_from_slice(&qr.data);
        self.buf.extend_from_slice(&[ESC, 0x1D, b'y', b'P']);
        self
    }
}

impl Builder<Impact> {
    /// Switches double-wide characters on or off (`ESC W n`).
    #[must_use]
    pub fn double_wide(self, on: bool) -> Self {
        self.raw([ESC, b'W', on as u8])
    }

    /// Switches double-tall characters on or off (`ESC h n`).
    #[must_use]
    pub fn double_tall(self, on: bool) -> Self {
        self.raw([ESC, b'h', on as u8])
    }

    /// Switches two-colour mode on or off (`ESC RS C n`). The default
    /// comes from the DIP switches and the setting survives `ESC @`.
    /// [`color`](Self::color) only prints red while it is on.
    #[must_use]
    pub fn two_color(self, on: bool) -> Self {
        self.raw([ESC, 0x1E, b'C', on as u8])
    }

    /// Selects the print colour (`ESC 4` red / `ESC 5` black). Colours
    /// can be mixed within a line. In single-colour mode the printer
    /// substitutes a decoration such as inverse for red.
    #[must_use]
    pub fn color(self, color: Color) -> Self {
        self.raw([ESC, if color == Color::Red { b'4' } else { b'5' }])
    }

    /// Prints a 9-dot bit image (`ESC ^ n n1 n2 d…`) in stripes of 9 rows.
    /// Line spacing is set to one stripe (`ESC 3`, 27/216″) so the stripes
    /// tile without gaps, and restored afterwards (`ESC 2`).
    #[must_use]
    pub fn bit_image(mut self, image: &BitImage) -> Self {
        let bitmap = &image.bitmap;
        let density = match image.density {
            Density::Single => 0,
            Density::Double => 1,
        };
        let [n1, n2] = (bitmap.width() as u16).to_le_bytes();

        self.buf.extend_from_slice(&[
            ESC,
            b'3',
            graphics::STRIPE_HEIGHT as u8 * graphics::LINE_FEED_UNITS_PER_DOT,
        ]);
        let stripes = bitmap.height().div_ceil(graphics::STRIPE_HEIGHT) as usize;
        self.buf
            .reserve(stripes * (6 + 2 * bitmap.width() as usize) + 2);
        let mut top = 0;
        while top < bitmap.height() {
            self.buf.extend_from_slice(&[ESC, b'^', density, n1, n2]);
            graphics::pack_stripe(bitmap, top, &mut self.buf);
            top += graphics::STRIPE_HEIGHT;
            if top < bitmap.height() {
                self.buf.push(b'\n');
            }
        }
        self.buf.extend_from_slice(&[ESC, b'2']);
        self
    }

    /// Selects the ANK character font (`ESC M` / `ESC P` / `ESC :`).
    #[must_use]
    pub fn font(self, font: ImpactFont) -> Self {
        let cmd = match font {
            ImpactFont::SevenByNine => b'M',
            ImpactFont::FiveByNine => b'P',
            ImpactFont::FiveByNineWide => b':',
        };
        self.raw([ESC, cmd])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::{Barcode, QrCode, Symbology};
    use crate::types::{Alignment, Color, Cut, Drawer};

    fn bytes(builder: Builder<impl Protocol>) -> Vec<u8> {
        builder.build().into_bytes()
    }

    #[test]
    fn new_document_resets_and_selects_cp437() {
        let expected = [0x1B, b'@', 0x1B, 0x1D, b't', 1];
        assert_eq!(bytes(Builder::<StarLine>::new()), expected);
        assert_eq!(bytes(Builder::<Impact>::new()), expected);
        assert_eq!(bytes(Builder::<StarLine>::without_init()), []);
    }

    #[test]
    fn styling_commands() {
        let doc = bytes(
            Builder::<StarLine>::without_init()
                .align(Alignment::Center)
                .bold(true)
                .underline(true)
                .bold(false)
                .underline(false),
        );
        #[rustfmt::skip]
        assert_eq!(doc, [
            0x1B, 0x1D, 0x61, 1,
            0x1B, 0x45,
            0x1B, 0x2D, 1,
            0x1B, 0x46,
            0x1B, 0x2D, 0,
        ]);
    }

    #[test]
    fn character_size_per_protocol() {
        // ESC W n / ESC h n carry multiplier - 1 on thermal printers.
        assert_eq!(
            bytes(Builder::<StarLine>::without_init().wide(2).tall(3)),
            [0x1B, 0x57, 1, 0x1B, 0x68, 2]
        );
        // StarLine clamps to x6, and 0 is promoted to normal size.
        assert_eq!(
            bytes(Builder::<StarLine>::without_init().wide(9).tall(0)),
            [0x1B, 0x57, 5, 0x1B, 0x68, 0]
        );
        // Impact only toggles double size.
        assert_eq!(
            bytes(
                Builder::<Impact>::without_init()
                    .double_wide(true)
                    .double_tall(false)
            ),
            [0x1B, 0x57, 1, 0x1B, 0x68, 0]
        );
    }

    #[test]
    fn text_feed_cut_drawer() {
        let doc = bytes(
            Builder::<StarLine>::without_init()
                .line("Café")
                .feed(3)
                .cut(Cut::FeedThenPartial)
                .open_drawer(Drawer::One)
                .open_drawer(Drawer::Two),
        );
        #[rustfmt::skip]
        assert_eq!(doc, [
            b'C', b'a', b'f', 0x82, b'\n',
            0x1B, 0x61, 3,
            0x1B, 0x64, 3,
            0x07,
            0x1A,
        ]);
    }

    #[test]
    fn feed_is_clamped_to_command_range() {
        assert_eq!(
            bytes(Builder::<StarLine>::without_init().feed(0)),
            [0x1B, 0x61, 1]
        );
        assert_eq!(
            bytes(Builder::<Impact>::without_init().feed(200)),
            [0x1B, 0x61, 127]
        );
    }

    #[test]
    fn barcode_command() {
        let code = Barcode::new(Symbology::Code128, "R-42")
            .unwrap()
            .human_readable(true);
        let doc = bytes(Builder::<StarLine>::without_init().barcode(&code));
        #[rustfmt::skip]
        assert_eq!(doc, [
            0x1B, b'b',
            b'6',       // n1: Code 128
            b'2',       // n2: HRI below, with line feed
            b'2',       // n3: default module mode 2
            80,         // n4: default height in dots
            b'R', b'-', b'4', b'2',
            0x1E,
        ]);
    }

    #[test]
    fn qr_command() {
        let qr = QrCode::new("AB").unwrap();
        let doc = bytes(Builder::<StarLine>::without_init().qr_code(&qr));
        #[rustfmt::skip]
        assert_eq!(doc, [
            0x1B, 0x1D, b'y', b'S', b'0', 2,        // model 2
            0x1B, 0x1D, b'y', b'S', b'1', 1,        // EC level M
            0x1B, 0x1D, b'y', b'S', b'2', 4,        // cell size 4
            0x1B, 0x1D, b'y', b'D', b'1', 0, 2, 0,  // store 2 bytes, auto mode
            b'A', b'B',
            0x1B, 0x1D, b'y', b'P',                 // print
        ]);
    }

    #[test]
    fn thermal_raster_command_stream() {
        // 10 x 2 dots: row 0 inks x = 0 and x = 9, row 1 inks x = 3.
        let bmp = Bitmap::from_fn(10, 2, |x, y| {
            (y == 0 && (x == 0 || x == 9)) || (y == 1 && x == 3)
        });
        let doc = bytes(Builder::<StarLine>::without_init().raster(&bmp, RasterQuality::High));
        #[rustfmt::skip]
        assert_eq!(doc, [
            0x1B, b'*', b'r', b'A',             // enter raster mode
            0x1B, b'*', b'r', b'P', b'0', 0,    // continuous page length
            0x1B, b'*', b'r', b'E', b'1', 0,    // EOT mode: print, no cut
            0x1B, b'*', b'r', b'Q', b'2', 0,    // high quality
            0x1B, b'*', b'r', b'K', b'0', 0,    // black, whatever the last job set
            b'b', 2, 0, 0b1000_0000, 0b0100_0000,
            b'b', 2, 0, 0b0001_0000, 0b0000_0000,
            0x1B, b'*', b'r', b'B',             // quit raster mode
        ]);
        // A BitImage is accepted as a bitmap too.
        let image = BitImage::new(bmp, Density::Single).unwrap();
        assert_eq!(
            bytes(Builder::<StarLine>::without_init().raster(&image, RasterQuality::High)),
            doc
        );
        // Empty images emit nothing.
        let empty = Bitmap::from_fn(0, 0, |_, _| false);
        assert_eq!(
            bytes(Builder::<StarLine>::without_init().raster(&empty, RasterQuality::Normal)),
            []
        );
    }

    #[test]
    fn thermal_raster_defaults_to_black_after_red() {
        let bitmap = Bitmap::from_fn(1, 1, |_, _| true);
        let doc = bytes(
            Builder::<StarLine>::without_init()
                .print_mode(PrintMode::TwoColor)
                .raster_color(&bitmap, RasterQuality::High, Color::Red)
                .raster(&bitmap, RasterQuality::High),
        );
        let colors: Vec<_> = doc
            .windows(6)
            .filter(|w| w[..4] == [0x1b, b'*', b'r', b'K'])
            .map(|w| (w[4], w[5]))
            .collect();
        assert_eq!(colors, [(b'1', 0), (b'0', 0)]);
    }

    #[test]
    fn thermal_print_mode_speed_and_density() {
        let doc = bytes(
            Builder::<StarLine>::without_init()
                .print_mode(PrintMode::DoubleResolution)
                .print_speed(PrintSpeed::Slow)
                .print_density(2)
                .print_density(0)
                .print_density(-9)
                .print_mode(PrintMode::SingleColor),
        );
        #[rustfmt::skip]
        assert_eq!(doc, [
            0x1B, 0x1E, b'C', 32,
            0x1B, 0x1E, b'r', 2,
            0x1B, 0x1E, b'd', 1,   // +2
            0x1B, 0x1E, b'd', 3,   // standard
            0x1B, 0x1E, b'd', 6,   // clamped to -3
            0x1B, 0x1E, b'C', 0,
        ]);
    }

    #[test]
    fn impact_two_color() {
        let doc = bytes(
            Builder::<Impact>::without_init()
                .two_color(true)
                .color(Color::Red)
                .color(Color::Black),
        );
        #[rustfmt::skip]
        assert_eq!(doc, [
            0x1B, 0x1E, b'C', 1,
            0x1B, 0x34,
            0x1B, 0x35,
        ]);
    }
}
