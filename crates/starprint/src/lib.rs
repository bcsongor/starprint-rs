//! Talk to Star Micronics receipt printers from Rust.
//!
//! `starprint` implements the two classic Star command sets over Ethernet:
//!
//! * **Star Line Mode**, the native protocol of Star *thermal* receipt
//!   printers (TSP100 Line Mode, TSP650II, TSP700II, TSP800II, …), per
//!   Star's *Star Line Mode Command Specifications*. See [`StarLine`].
//! * **Star Mode for dot impact printers**, the native protocol of the
//!   SP700 series of *impact* printers (SP712, SP742, …), per Star's *Dot
//!   Impact Printer STAR Command Specifications*, including red/black
//!   two-colour printing. See [`Impact`].
//!
//! A [`Builder`] renders a [`Document`] (plain bytes) and a
//! [`Transport`](transport::Transport) delivers it;
//! [`TcpTransport`](transport::TcpTransport) covers port 9100.
//!
//! The [`graphics`] module has 1-bit rasters and dithering; the `image`
//! feature adds `graphics::ImagePipeline`, which turns a photo into a
//! [`BitImage`](graphics::BitImage) for [`Builder::bit_image`] (impact)
//! or [`Builder::raster`] (thermal).
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
//! Commands a printer lacks are compile errors: [`Builder::qr_code`] only
//! exists for `Builder<StarLine>`.

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
    PrintMode, PrintSpeed, RasterQuality, ThermalFont,
};

/// Starts a document for a Star Line Mode (thermal) printer.
#[must_use]
pub fn starline() -> Builder<StarLine> {
    Builder::new()
}

/// Starts a document for an SP700-series impact printer.
#[must_use]
pub fn impact() -> Builder<Impact> {
    Builder::new()
}
