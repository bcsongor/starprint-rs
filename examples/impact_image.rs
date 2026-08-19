//! Prints a photo on a Star SP700-series impact printer.
//!
//! Usage: cargo run --example impact_image --features image -- <printer-host> <image-path> [single|double]

use starprint::graphics::Density;
use starprint::pipeline::ImagePipeline;
use starprint::transport::TcpTransport;
use starprint::{Alignment, Cut};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: impact_image <printer-host> <image-path> [single|double]";
    let host = args.next().ok_or(usage)?;
    let path = args.next().ok_or(usage)?;
    let density = match args.next().as_deref() {
        None | Some("single") => Density::Single,
        Some("double") => Density::Double,
        Some(other) => return Err(format!("unknown density {other:?}; {usage}").into()),
    };

    let photo = std::fs::read(&path)?;
    let prepared = ImagePipeline::new()
        .density(density)
        .prepare_bytes(&photo)?;

    let bitmap_h = prepared.preview.height();
    let doc = starprint::impact()
        .align(Alignment::Center)
        .bit_image(&prepared.image)
        .feed(2)
        .line(&path)
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!(
        "printed {}x{} dots ({density:?} density, preview height {bitmap_h}) — {} bytes sent",
        prepared.preview.width(),
        bitmap_h,
        doc.as_bytes().len()
    );
    Ok(())
}
