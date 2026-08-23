//! Prints a photo on a Star Line Mode thermal printer via raster mode.
//!
//! Usage: cargo run --example thermal_image --features image -- <printer-host> <image-path> [double]
//!
//! `double` selects the TSP700II's double-resolution print mode (16 dot
//! rows per mm) and prepares the image accordingly.

use starprint::graphics::{DeviceProfile, ImagePipeline};
use starprint::transport::TcpTransport;
use starprint::{Cut, PrintMode, RasterQuality};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: thermal_image <printer-host> <image-path> [double]";
    let host = args.next().ok_or(usage)?;
    let path = args.next().ok_or(usage)?;
    let double = match args.next().as_deref() {
        None => false,
        Some("double") => true,
        Some(other) => return Err(format!("unknown option {other:?}; {usage}").into()),
    };

    let profile = if double {
        DeviceProfile::THERMAL_80MM_DOUBLE_RESOLUTION
    } else {
        DeviceProfile::THERMAL_80MM
    };
    let prepared = ImagePipeline::new()
        .profile(profile)
        .prepare_bytes(&std::fs::read(&path)?)?;

    let mut doc = starprint::starline();
    if double {
        doc = doc.print_mode(PrintMode::DoubleResolution);
    }
    doc = doc.raster(&prepared.image, RasterQuality::High);
    if double {
        doc = doc.print_mode(PrintMode::SingleColor); // the mode outlives ESC @
    }
    let doc = doc
        .feed(2)
        .line(&path)
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!(
        "printed {}x{} dots — {} bytes sent",
        prepared.image.bitmap().width(),
        prepared.image.bitmap().height(),
        doc.as_bytes().len()
    );
    Ok(())
}
