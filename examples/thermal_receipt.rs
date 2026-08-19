//! Prints a demo receipt on a Star Line Mode (thermal) printer.
//!
//! Usage: cargo run --example thermal_receipt -- <printer-host-or-ip>

use starprint::code::{Barcode, QrCode, Symbology};
use starprint::transport::{TcpTransport, TransportExt};
use starprint::{Alignment, Cut};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::args()
        .nth(1)
        .ok_or("usage: thermal_receipt <printer-host-or-ip>")?;

    let receipt = starprint::starline()
        .align(Alignment::Center)
        .wide(2)
        .tall(2)
        .line("ACME STORE")
        .wide(1)
        .tall(1)
        .line("123 Example Street")
        .line("Springfield")
        .feed(1)
        .align(Alignment::Left)
        .line("1x Flat white             4.20")
        .line("1x Croissant              3.80")
        .line("------------------------------")
        .bold(true)
        .tall(2)
        .line("TOTAL                     8.00")
        .tall(1)
        .bold(false)
        .feed(1)
        .align(Alignment::Center)
        .barcode(&Barcode::new(Symbology::Code128, "R-000042")?.hri(true))
        .feed(1)
        .qr_code(&QrCode::new("https://example.com/receipt/42")?)
        .feed(1)
        .line("Thank you!")
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&receipt)?;
    println!("sent {} bytes to {host}", receipt.as_bytes().len());
    Ok(())
}
