//! Prints the same patterns twice, once in normal resolution and once in
//! double-resolution mode, so the paper can say how much blacker the
//! extra rows make it.
//!
//! Usage: cargo run --example thermal_black_probe -- <printer-host> [80|112] [density=N]
//!
//! Star's density table gives double resolution less energy per dot at
//! the top of the scale (1.2 against 1.3); the strip shows whether twice
//! the rows more than makes up for it. Print speed is fixed in that mode,
//! so the slow setting only applies to the first half.

mod common;

use starprint::graphics::Bitmap;
use starprint::transport::TcpTransport;
use starprint::{Alignment, Cut, PrintMode, PrintSpeed, RasterQuality};

/// Heights in dot rows at 8 rows/mm, so 20 mm, 10 mm and 15 mm.
const SOLID: u32 = 160;
const CHECKER: u32 = 80;
const RAMP: u32 = 120;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let common::Probe { host, width, flags } =
        common::probe("usage: thermal_black_probe <printer-host> [80|112] [density=N]")?;
    let density = common::density(&flags)?.unwrap_or(3);

    let solid = Bitmap::from_fn(width, SOLID, |_, _| true);
    let checker = Bitmap::from_fn(width, CHECKER, |x, y| (x + y) % 2 == 0);
    let ramp = common::ramp(width, RAMP);

    let mut doc = starprint::starline()
        .print_density(density)
        .print_speed(PrintSpeed::Slow)
        .align(Alignment::Center)
        .bold(true)
        .line("BLACK DENSITY: RESOLUTION MODES")
        .bold(false)
        .line(&format!("{width} dots, density {density:+}, slow"))
        .feed(1)
        .align(Alignment::Left);

    for double in [false, true] {
        let factor = if double { 2 } else { 1 };
        doc = doc
            .print_mode(PrintMode::SingleColor)
            .bold(true)
            .line(if double {
                "-- DOUBLE RESOLUTION (16 rows/mm) --"
            } else {
                "-- NORMAL RESOLUTION (8 rows/mm) --"
            })
            .bold(false)
            .line("solid / checkerboard / ramp, 20-10-15 mm");
        if double {
            doc = doc.print_mode(PrintMode::DoubleResolution);
        }
        doc = doc
            .raster(stretch(&solid, factor), RasterQuality::High)
            .feed(1)
            .raster(stretch(&checker, factor), RasterQuality::High)
            .feed(1)
            .raster(stretch(&ramp, factor), RasterQuality::High)
            .feed(1);
    }

    // The mode outlives ESC @, so leave the printer in single colour.
    let doc = doc
        .print_mode(PrintMode::SingleColor)
        .align(Alignment::Center)
        .line("compare the two solid bars end to end")
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!("sent {} bytes to {host}", doc.as_bytes().len());
    Ok(())
}

/// Repeats every row `factor` times: the same picture at the same size on
/// paper, drawn with the rows its print mode gives it.
fn stretch(source: &Bitmap, factor: u32) -> Bitmap {
    Bitmap::from_fn(source.width(), source.height() * factor, |x, y| {
        source.get(x, y / factor)
    })
}
