//! Prints the same text twice, once in normal resolution and once in
//! double-resolution mode, to find out what that mode does to the
//! printer's own fonts.
//!
//! Usage: cargo run --example thermal_text_probe -- <printer-host> [80|112] [density=N]
//!
//! The specification does not say whether the firmware doubles its fonts
//! or prints them at half height. Inverse lines are the one text style
//! that lays down solid black, so they show the darkening too.

mod common;

use starprint::transport::TcpTransport;
use starprint::{Alignment, Cut, PrintMode, PrintSpeed, ThermalFont};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let common::Probe { host, width, flags } =
        common::probe("usage: thermal_text_probe <printer-host> [80|112] [density=N]")?;
    // Font A is 12 dots wide.
    let columns = (width / 12) as usize;
    let density = common::density(&flags)?.unwrap_or(3);

    let fox = "The quick brown fox 0123456789";
    let bar = " ".repeat(columns);

    let mut doc = starprint::starline()
        .print_density(density)
        .print_speed(PrintSpeed::Slow)
        .align(Alignment::Center)
        .bold(true)
        .line("TEXT: RESOLUTION MODES")
        .bold(false)
        .line(&format!("{columns} columns, density {density:+}, slow"))
        .feed(1)
        .align(Alignment::Left);

    for double in [false, true] {
        doc = doc
            .print_mode(PrintMode::SingleColor)
            .bold(true)
            .line(if double {
                "-- DOUBLE RESOLUTION (16 rows/mm) --"
            } else {
                "-- NORMAL RESOLUTION (8 rows/mm) --"
            })
            .bold(false);
        if double {
            doc = doc.print_mode(PrintMode::DoubleResolution);
        }
        doc = doc
            .line(&format!("A plain: {fox}"))
            .bold(true)
            .line(&format!("A bold:  {fox}"))
            .bold(false)
            .invert(true)
            .line(&format!("A inverse: {fox}"))
            .line(&bar)
            .invert(false)
            .wide(2)
            .tall(2)
            .line("Quad size")
            .wide(1)
            .tall(1)
            .font(ThermalFont::B)
            .line(&format!("B plain: {fox}"))
            .invert(true)
            .line(&format!("B inverse: {fox}"))
            .invert(false)
            .font(ThermalFont::A)
            .feed(1);
    }

    // The mode outlives ESC @, so leave the printer in single colour.
    let doc = doc
        .print_mode(PrintMode::SingleColor)
        .align(Alignment::Center)
        .line("same lines, same order, both halves")
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!("sent {} bytes to {host}", doc.as_bytes().len());
    Ok(())
}
