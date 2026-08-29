//! Prints a photo on a Star SP700-series impact printer.
//!
//! Usage: cargo run --example impact_image --features image -- <printer-host> <image-path> [single|double] [rotate]
//!
//! `rotate` turns a landscape picture 90° so it runs along the paper.

use starprint::graphics::Density;
use starprint::graphics::ImagePipeline;
use starprint::transport::TcpTransport;
use starprint::{Alignment, Cut};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: impact_image <printer-host> <image-path> [single|double] [rotate]";
    let host = args.next().ok_or(usage)?;
    let path = args.next().ok_or(usage)?;
    let flags: Vec<String> = args.collect();
    let density = match flags
        .iter()
        .find(|f| f.as_str() != "rotate")
        .map(String::as_str)
    {
        None | Some("single") => Density::Single,
        Some("double") => Density::Double,
        Some(other) => return Err(format!("unknown option {other:?}; {usage}").into()),
    };
    let rotate = flags.iter().any(|f| f == "rotate");

    let mut source = image::load_from_memory(&std::fs::read(&path)?)?;
    if rotate {
        source = source.rotate90();
    }
    let prepared = ImagePipeline::new().density(density).prepare(&source)?;

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
        "printed {}x{} dots ({density:?} density, preview height {bitmap_h}); {} bytes sent",
        prepared.preview.width(),
        bitmap_h,
        doc.as_bytes().len()
    );
    Ok(())
}
