//! Prints a demo kitchen ticket on a Star SP700-series impact printer.
//!
//! Usage: cargo run --example impact_kitchen_ticket -- <printer-host-or-ip>

use starprint::transport::{TcpTransport, TransportExt};
use starprint::{Alignment, Color, Cut};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::args()
        .nth(1)
        .ok_or("usage: impact_kitchen_ticket <printer-host-or-ip>")?;

    let ticket = starprint::impact()
        .two_color(true) // needed for red; requires a black/red ribbon
        .align(Alignment::Center)
        .wide(2)
        .tall(2)
        .line("TABLE 7")
        .wide(1)
        .tall(1)
        .line("Order #42 - 19:05")
        .feed(1)
        .align(Alignment::Left)
        .line("2x Carbonara")
        .line("1x Margherita")
        .line("   + extra basil")
        .color(Color::Red)
        .tall(2)
        .line("** ALLERGY: NUTS **")
        .tall(1)
        .color(Color::Black)
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&ticket)?;
    println!("sent {} bytes to {host}", ticket.as_bytes().len());
    Ok(())
}
