//! Protocol-aware document building.
//!
//! A [`Builder`] renders a receipt into the raw command stream of one of
//! the two supported Star command sets, chosen at the type level:
//!
//! * [`StarLine`] — **Star Line Mode**, the native command set of Star
//!   thermal receipt printers (TSP100 Line Mode, TSP650II, TSP700II,
//!   TSP800II, …), per Star's *Line Thermal Printer — Star Line Mode
//!   Command Specifications*.
//! * [`Impact`] — **Star Mode for dot impact printers**, the native
//!   command set of the SP700 series (SP712, SP742, SP717, SP747), per
//!   Star's *Dot Impact Printer — STAR Command Specifications*. It shares
//!   most of Star Line Mode's vocabulary but adds two-colour (red/black)
//!   printing and omits thermal-only features such as barcodes.
//!
//! Features that only exist on one protocol are only *available* on that
//! protocol: e.g. [`Builder::qr_code`] exists for `Builder<StarLine>` but
//! not for `Builder<Impact>`, so an unsupported command is a compile
//! error, not a runtime surprise. Where the protocols share a command but
//! differ in what it accepts (e.g. [`Builder::wide`]), the builder bounds
//! parameters to its protocol's range.

use std::marker::PhantomData;

use crate::code::{Barcode, QrCode, QrErrorCorrection, QrModel};
use crate::cp437;
use crate::types::{
    Alignment, CodePage, Color, Cut, Drawer, Font, ImpactFont, International, LineSpacing,
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
pub trait Protocol: sealed::Sealed + 'static {
    /// Largest character size multiplier per axis: `ESC W` / `ESC h`
    /// accept ×1–×6 on Star Line Mode but only ×1–×2 (double-wide /
    /// double-tall) on impact Star Mode.
    const MAX_MAGNIFICATION: u8;
}

/// Marker for **Star Line Mode** — thermal receipt printers.
///
/// Build documents for it with [`crate::starline()`].
#[derive(Debug)]
pub enum StarLine {}

impl Protocol for StarLine {
    const MAX_MAGNIFICATION: u8 = 6;
}

/// Marker for **Star Mode on dot impact printers** — the SP700 series of
/// two-colour dot-matrix kitchen printers.
///
/// Build documents for it with [`crate::impact()`].
#[derive(Debug)]
pub enum Impact {}

impl Protocol for Impact {
    const MAX_MAGNIFICATION: u8 = 2;
}

/// A fully rendered command stream, ready to send to a printer.
///
/// Produced by [`Builder::build`]; consumed by a
/// [`Transport`](crate::transport::Transport). A `Document` is plain
/// bytes: it can also be spooled to disk, queued, or concatenated.
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

/// A fluent, protocol-aware builder for printer documents.
///
/// Methods append commands in order and return the builder, so a whole
/// receipt reads as one expression. The protocol parameter `P` decides
/// which commands are available and how they are encoded.
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
    /// Starts a new document with a known-good baseline: `ESC @` resets
    /// the printer's command state to its power-on defaults (alignment,
    /// emphasis, magnification, …), and the code page is set to CP437 so
    /// that [`text`](Self::text) prints exactly what it encoded.
    ///
    /// Note that `ESC @` deliberately does *not* reset everything on the
    /// printer — e.g. cash-drawer pulse settings and the impact printers'
    /// two-colour mode survive it.
    #[must_use]
    pub fn new() -> Self {
        Self::empty().raw([ESC, b'@']).code_page(CodePage::CP437)
    }

    /// Starts a document *without* the leading reset, inheriting whatever
    /// state the printer is in.
    ///
    /// Use this to continue a print job whose styling was set up by an
    /// earlier document.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            buf: Vec::with_capacity(256),
            _protocol: PhantomData,
        }
    }

    /// Appends raw bytes verbatim — the escape hatch for commands this
    /// builder does not model (or text pre-encoded for a non-CP437 code
    /// page).
    #[must_use]
    pub fn raw(mut self, bytes: impl AsRef<[u8]>) -> Self {
        self.buf.extend_from_slice(bytes.as_ref());
        self
    }

    /// Prints text, encoded as code page 437 (which
    /// [`new`](Self::new) selects on the printer).
    ///
    /// Characters CP437 cannot represent are printed as `?`. Embedded
    /// `'\n'` characters print and feed the line, so multi-line strings
    /// work as expected. To print in another code page, select it with
    /// [`code_page`](Self::code_page) and send pre-encoded bytes via
    /// [`raw`](Self::raw).
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        cp437::encode_into(text, &mut self.buf);
        self
    }

    /// Prints text followed by a line feed.
    #[must_use]
    pub fn line(self, text: &str) -> Self {
        self.text(text).raw([b'\n'])
    }

    /// Feeds `lines` blank lines (`ESC a n`; the command accepts 1–127,
    /// out-of-range values are clamped).
    #[must_use]
    pub fn feed(self, lines: u8) -> Self {
        self.raw([ESC, b'a', lines.clamp(1, 127)])
    }

    /// Sets horizontal alignment for subsequent lines (`ESC GS a n`).
    ///
    /// Takes effect at the start of a line; the printer default is
    /// [`Alignment::Left`].
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

    /// Sets the character width multiplier (`ESC W n`).
    ///
    /// `multiplier` starts at 1 (normal width) and is clamped to the
    /// protocol's supported range: ×1–×6 on [`StarLine`], ×1–×2 on
    /// [`Impact`]. Width and height are independent — combine with
    /// [`tall`](Self::tall) for proportionally bigger characters.
    #[must_use]
    pub fn wide(self, multiplier: u8) -> Self {
        self.raw([ESC, b'W', multiplier.clamp(1, P::MAX_MAGNIFICATION) - 1])
    }

    /// Sets the character height multiplier (`ESC h n`).
    ///
    /// `multiplier` starts at 1 (normal height) and is clamped to the
    /// protocol's supported range: ×1–×6 on [`StarLine`], ×1–×2 on
    /// [`Impact`]. Width and height are independent — combine with
    /// [`wide`](Self::wide) for proportionally bigger characters.
    #[must_use]
    pub fn tall(self, multiplier: u8) -> Self {
        self.raw([ESC, b'h', multiplier.clamp(1, P::MAX_MAGNIFICATION) - 1])
    }

    /// Selects an international character-set variant (`ESC R n`).
    #[must_use]
    pub fn international(self, set: International) -> Self {
        self.raw([ESC, b'R', set.code()])
    }

    /// Selects the printer's active code page (`ESC GS t n`).
    ///
    /// This changes how the *printer* interprets bytes ≥ 0x80; it does
    /// not change how [`text`](Self::text) encodes (always CP437). Pair a
    /// non-default page with pre-encoded [`raw`](Self::raw) bytes.
    #[must_use]
    pub fn code_page(self, page: CodePage) -> Self {
        self.raw([ESC, 0x1D, b't', page.0])
    }

    /// Cuts the paper (`ESC d n`).
    ///
    /// For a normal receipt use [`Cut::FeedThenPartial`] (or
    /// [`Cut::FeedThenFull`]), which first feeds the printed content past
    /// the cutter blade. On tear-bar models (e.g. SP712/SP717) the
    /// non-feeding variants are ignored and the feeding variants advance
    /// to the tear-bar position instead.
    #[must_use]
    pub fn cut(self, cut: Cut) -> Self {
        self.raw([ESC, b'd', cut.code()])
    }

    /// Fires the pulse that opens a cash drawer connected to the given
    /// peripheral-drive circuit (`BEL` / `SUB`).
    ///
    /// Drawer 1 uses the pulse timing configured by
    /// [`drawer_pulse`](Self::drawer_pulse) (default 200 ms); drawer 2's
    /// timing is fixed at 200 ms.
    #[must_use]
    pub fn open_drawer(self, drawer: Drawer) -> Self {
        match drawer {
            Drawer::One => self.raw([0x07]),
            Drawer::Two => self.raw([0x1A]),
        }
    }

    /// Configures the drive pulse for drawer 1 (`ESC BEL n1 n2`):
    /// energize time `on_10ms` × 10 ms, then delay `off_10ms` × 10 ms.
    /// Both values are clamped to 1–127; the printer default is 20/20
    /// (200 ms each).
    #[must_use]
    pub fn drawer_pulse(self, on_10ms: u8, off_10ms: u8) -> Self {
        self.raw([ESC, 0x07, on_10ms.clamp(1, 127), off_10ms.clamp(1, 127)])
    }

    /// Finishes the document, returning the rendered command stream.
    #[must_use]
    pub fn build(self) -> Document {
        Document { bytes: self.buf }
    }
}

impl Builder<StarLine> {
    /// Switches white-on-black (inverted) printing on or off
    /// (`ESC 4` / `ESC 5`).
    ///
    /// Thermal printers only: on impact models the same byte pair selects
    /// the red/black colour instead — see the impact builder's `color`
    /// method.
    #[must_use]
    pub fn invert(self, on: bool) -> Self {
        self.raw([ESC, if on { b'4' } else { b'5' }])
    }

    /// Selects the base character font (`ESC RS F n`).
    #[must_use]
    pub fn font(self, font: Font) -> Self {
        self.raw([ESC, 0x1E, b'F', font.code()])
    }

    /// Sets the line-feed pitch (`ESC z n`): 4 mm is the usual receipt
    /// pitch, 3 mm packs lines tighter.
    #[must_use]
    pub fn line_spacing(self, spacing: LineSpacing) -> Self {
        let n = match spacing {
            LineSpacing::ThreeMm => 0,
            LineSpacing::FourMm => 1,
        };
        self.raw([ESC, b'z', n])
    }

    /// Prints a one-dimensional barcode (`ESC b n1 n2 n3 n4 … RS`).
    ///
    /// The payload was validated when the [`Barcode`] was constructed, so
    /// this cannot fail. The barcode is placed using the current
    /// [`align`](Self::align) setting, and the printer feeds past it
    /// automatically before the next line.
    #[must_use]
    pub fn barcode(mut self, barcode: &Barcode) -> Self {
        // n2: 1 = no HRI text, 2 = HRI text below; both with automatic
        // line feed after the bars.
        self.buf.extend_from_slice(&[
            ESC,
            b'b',
            barcode.symbology.code(),
            if barcode.hri { b'2' } else { b'1' },
            b'0' + barcode.module,
            barcode.height,
        ]);
        self.buf.extend_from_slice(&barcode.data);
        self.buf.push(0x1E); // RS terminates the payload.
        self
    }

    /// Prints a QR code (`ESC GS y` command family).
    ///
    /// Emits the model, error-correction and cell-size settings, stores
    /// the payload (automatic encoding mode), and prints it at the
    /// current alignment.
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
        // Store data: ESC GS y D 1 m nL nH d1…dk (m = 0: automatic mode).
        self.buf
            .extend_from_slice(&[ESC, 0x1D, b'y', b'D', b'1', 0, len[0], len[1]]);
        self.buf.extend_from_slice(&qr.data);
        // Print the stored symbol.
        self.buf.extend_from_slice(&[ESC, 0x1D, b'y', b'P']);
        self
    }
}

impl Builder<Impact> {
    /// Selects or cancels two-colour printing mode (`ESC RS C n`).
    ///
    /// The power-on default comes from the printer's DIP switches, and
    /// the setting survives `ESC @`. Two-colour mode must be selected for
    /// [`color`](Self::color) to produce red.
    #[must_use]
    pub fn two_color(self, on: bool) -> Self {
        self.raw([ESC, 0x1E, b'C', on as u8])
    }

    /// Selects the print colour (`ESC 4` red / `ESC 5` black).
    ///
    /// Requires two-colour mode (see [`two_color`](Self::two_color)) and
    /// a black/red ribbon; red and black can be mixed within one line.
    /// On a printer configured for single-colour operation the same
    /// commands render the firmware's substitute decoration (e.g.
    /// inverted printing) instead of red.
    #[must_use]
    pub fn color(self, color: Color) -> Self {
        self.raw([ESC, if color == Color::Red { b'4' } else { b'5' }])
    }

    /// Selects the ANK character font (`ESC M` / `ESC P` / `ESC :`).
    #[must_use]
    pub fn font(self, font: ImpactFont) -> Self {
        let cmd = match font {
            ImpactFont::SevenByNine => b'M',
            ImpactFont::FiveByNine => b'P',
            ImpactFont::FiveByNineLarge => b':',
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
        assert_eq!(bytes(Builder::<StarLine>::empty()), []);
    }

    #[test]
    fn styling_commands() {
        let doc = bytes(
            Builder::<StarLine>::empty()
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
        // ESC W n / ESC h n carry multiplier - 1.
        assert_eq!(
            bytes(Builder::<StarLine>::empty().wide(2).tall(3)),
            [0x1B, 0x57, 1, 0x1B, 0x68, 2]
        );
        // StarLine clamps to x6, and 0 is promoted to normal size.
        assert_eq!(
            bytes(Builder::<StarLine>::empty().wide(9).tall(0)),
            [0x1B, 0x57, 5, 0x1B, 0x68, 0]
        );
        // Impact clamps to x2 (double-wide / double-tall only).
        assert_eq!(
            bytes(Builder::<Impact>::empty().wide(4).tall(1)),
            [0x1B, 0x57, 1, 0x1B, 0x68, 0]
        );
    }

    #[test]
    fn text_feed_cut_drawer() {
        let doc = bytes(
            Builder::<StarLine>::empty()
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
        assert_eq!(bytes(Builder::<StarLine>::empty().feed(0)), [0x1B, 0x61, 1]);
        assert_eq!(
            bytes(Builder::<Impact>::empty().feed(200)),
            [0x1B, 0x61, 127]
        );
    }

    #[test]
    fn barcode_command() {
        let code = Barcode::new(Symbology::Code128, "R-42").unwrap().hri(true);
        let doc = bytes(Builder::<StarLine>::empty().barcode(&code));
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
        let doc = bytes(Builder::<StarLine>::empty().qr_code(&qr));
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
    fn impact_two_color() {
        let doc = bytes(
            Builder::<Impact>::empty()
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
