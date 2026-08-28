//! Prints a demo kitchen ticket on a Star SP700-series impact printer.
//!
//! Usage: cargo run --example impact_kitchen_ticket -- <printer-host-or-ip>

use starprint::transport::TcpTransport;
use starprint::{Alignment, Color, Cut};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::args()
        .nth(1)
        .ok_or("usage: impact_kitchen_ticket <printer-host-or-ip>")?;

    let ticket = starprint::impact()
        .two_color(true) // needed for red; requires a black/red ribbon
        .align(Alignment::Center)
        .double_wide(true)
        .double_tall(true)
        .line("TABLE 7")
        .double_wide(false)
        .double_tall(false)
        .line("Order #42 - 19:05")
        .feed(1)
        .align(Alignment::Left)
        .line("2x Carbonara")
        .line("1x Margherita")
        .line("   + extra basil")
        .color(Color::Red)
        .double_tall(true)
        .line("** ALLERGY: NUTS **")
        .double_tall(false)
        .color(Color::Black)
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&ticket)?;
    println!("sent {} bytes to {host}", ticket.as_bytes().len());
    Ok(())
}
