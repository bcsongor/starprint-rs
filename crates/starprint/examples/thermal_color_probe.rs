//! Compares single-colour black with two-colour black and red on plain paper.
//!
//! Usage: cargo run --example thermal_color_probe -- <printer-host> [80|112] [density=N]
//!
//! Two-colour paper develops red at a higher temperature than black, so
//! the red pass is the head's hotter profile; on single-colour paper it
//! can only come out black. The mode has one fixed speed and ignores
//! `ESC RS r`, so the slow setting applies to the first pass only, and
//! `ESC RS d` adjusts only the red density while the mode is on.

mod common;

use starprint::graphics::Bitmap;
use starprint::transport::TcpTransport;
use starprint::{Alignment, Color, Cut, PrintMode, PrintSpeed, RasterQuality};

/// Heights in dot rows at 8 rows/mm, so 20 mm, 10 mm and 15 mm.
const SOLID: u32 = 160;
const CHECKER: u32 = 80;
const RAMP: u32 = 120;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let common::Probe { host, width, flags } =
        common::probe("usage: thermal_color_probe <printer-host> [80|112] [density=N]")?;
    let density = common::density(&flags)?.unwrap_or(3);

    let solid = Bitmap::from_fn(width, SOLID, |_, _| true);
    let checker = Bitmap::from_fn(width, CHECKER, |x, y| (x + y) % 2 == 0);
    let ramp = common::ramp(width, RAMP);

    let mut doc = starprint::starline()
        .print_mode(PrintMode::SingleColor)
        .print_density(density)
        .print_speed(PrintSpeed::Slow)
        .align(Alignment::Center)
        .bold(true)
        .line("BLACK DENSITY: COLOUR MODES")
        .bold(false)
        .line(&format!("{width} dots, density {density:+}"))
        .feed(1)
        .align(Alignment::Left);

    for (mode, color, label) in [
        (
            PrintMode::SingleColor,
            Color::Black,
            "-- SINGLE COLOUR, SLOW --",
        ),
        (PrintMode::TwoColor, Color::Black, "-- TWO COLOUR, BLACK --"),
        (PrintMode::TwoColor, Color::Red, "-- TWO COLOUR, RED --"),
    ] {
        doc = doc
            .print_mode(mode)
            // The mode keeps a density of its own for red.
            .print_density(density)
            .color(color)
            .bold(true)
            .line(label)
            .bold(false)
            .line("solid / checkerboard / ramp, 20-10-15 mm");
        for bitmap in [&solid, &checker, &ramp] {
            doc = doc.raster_color(bitmap, RasterQuality::High, color).feed(1);
        }
        doc = doc.color(Color::Black);
    }

    // The mode outlives ESC @, so leave the printer in single colour.
    let doc = doc
        .print_mode(PrintMode::SingleColor)
        .align(Alignment::Center)
        .line("compare the three solid bars end to end")
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!("sent {} bytes to {host}", doc.as_bytes().len());
    Ok(())
}
