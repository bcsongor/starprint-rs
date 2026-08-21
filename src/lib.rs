//! Talk to Star Micronics receipt printers from Rust.
//!
//! `starprint` implements the two classic Star command sets over Ethernet:
//!
//! * **Star Line Mode** — the native protocol of Star *thermal* receipt
//!   printers (TSP100 Line Mode, TSP650II, TSP700II, TSP800II, …), per
//!   Star's *Star Line Mode Command Specifications*. See [`StarLine`].
//! * **Star Mode for dot impact printers** — the native protocol of the
//!   SP700 series of *impact* printers (SP712, SP742, …), per Star's *Dot
//!   Impact Printer STAR Command Specifications*, including red/black
//!   two-colour printing. See [`Impact`].
//!
//! The crate is split into a pure protocol layer and a transport layer:
//! a [`Builder`] renders a [`Document`] (plain bytes) and a
//! [`Transport`](transport::Transport) delivers it. Ethernet (raw-socket
//! printing on TCP port 9100) ships in the box as
//! [`TcpTransport`](transport::TcpTransport).
//!
//! Impact printers can also print pictures: the [`graphics`] module covers
//! 1-bit rasters and dithering without dependencies, and the optional
//! `image` cargo feature adds `graphics::ImagePipeline` — a
//! hardware-tuned decode/tone-map/resize pipeline that turns a photo
//! into a print-ready [`BitImage`](graphics::BitImage).
//!
//! # Quick start
//!
//! ```no_run
//! use starprint::{Alignment, Cut};
//! use starprint::transport::TcpTransport;
//!
//! let receipt = starprint::starline()
//!     .align(Alignment::Center)
//!     .wide(2)
//!     .tall(2)
//!     .line("ACME STORE")
//!     .wide(1)
//!     .tall(1)
//!     .feed(1)
//!     .align(Alignment::Left)
//!     .line("1x Flat white           4.20")
//!     .line("1x Croissant            3.80")
//!     .bold(true)
//!     .line("TOTAL                   8.00")
//!     .bold(false)
//!     .feed(2)
//!     .cut(Cut::FeedThenPartial)
//!     .build();
//!
//! let mut printer = TcpTransport::connect("192.168.1.60")?;
//! printer.print(&receipt)?;
//! # Ok::<(), starprint::Error>(())
//! ```
//!
//! Unsupported features are unrepresentable: the compiler rejects, say, a
//! QR code on an impact printer, because [`Builder::qr_code`] only exists
//! for `Builder<StarLine>`.

mod code;
mod cp437;
mod document;
mod error;
mod types;

pub mod graphics;
pub mod transport;

pub use code::{Barcode, QrCode, QrErrorCorrection, QrModel, Symbology};
pub use document::{Builder, Document, Impact, Protocol, StarLine};
pub use error::{Error, Result};
pub use types::{
    Alignment, CodePage, Color, Cut, Drawer, ImpactFont, InternationalCharset, LineSpacing,
    ThermalFont,
};

/// Starts a document for a **Star Line Mode** (thermal) printer.
///
/// Equivalent to `Builder::<StarLine>::new()`; the document begins with an
/// `ESC @` reset.
#[must_use]
pub fn starline() -> Builder<StarLine> {
    Builder::new()
}

/// Starts a document for a **Star Mode dot impact** (SP700-series)
/// printer.
///
/// Equivalent to `Builder::<Impact>::new()`; the document begins with an
/// `ESC @` reset.
#[must_use]
pub fn impact() -> Builder<Impact> {
    Builder::new()
}
