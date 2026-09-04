//! Prints a head/platen diagnostic page on a Star Line Mode thermal printer.
//!
//! Usage: cargo run --example thermal_test_pattern -- <printer-host> [80|112] [double] [slow] [density=N]
//!
//! Sections, top to bottom:
//! 1. Solid black bar. Any white vertical hairline is a dead head element.
//! 2. Single-dot vertical lines every 8 dots. A missing or faint line is a
//!    weak element; uneven spacing means feed/platen trouble.
//! 3. One-dot checkerboard, for sharpness and dot gain.
//! 4. Dithered grey ramp, dark to light. Look for horizontal banding
//!    (platen / heat) and vertical streaks (element drift).
//! 5. Text in both fonts, sizes and a barcode/QR, for general sanity.
//!
//! With `double`, the ramp is repeated in double-resolution mode. `slow`
//! prints at slow speed; `density=N` sets print density from -3
//! (lightest) to 3 (darkest), 0 being the printer's standard.

mod common;

use starprint::graphics::Bitmap;
use starprint::transport::TcpTransport;
use starprint::{
    Alignment, Barcode, Cut, PrintMode, PrintSpeed, QrCode, RasterQuality, Symbology, ThermalFont,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let common::Probe { host, width, flags } = common::probe(
        "usage: thermal_test_pattern <printer-host> [80|112] [double] [slow] [density=N]",
    )?;
    let double = flags.iter().any(|f| f == "double");
    let slow = flags.iter().any(|f| f == "slow");
    let density = common::density(&flags)?;

    let black_bar = Bitmap::from_fn(width, 64, |_, _| true);
    let hairlines = Bitmap::from_fn(width, 48, |x, _| x % 8 == 0);
    let checkerboard = Bitmap::from_fn(width, 32, |x, y| (x + y) % 2 == 0);

    // The mode outlives ESC @ and the job that set it, so start from a
    // known one rather than from whatever printed last.
    let mut doc = starprint::starline().print_mode(PrintMode::SingleColor);
    if slow {
        doc = doc.print_speed(PrintSpeed::Slow);
    }
    if let Some(level) = density {
        doc = doc.print_density(level);
    }
    let mut doc = doc
        .align(Alignment::Center)
        .bold(true)
        .line("STARPRINT HEAD CHECK")
        .bold(false)
        .line(&format!(
            "{width} dots, {} speed, density {}",
            if slow { "slow" } else { "default" },
            density.map_or("default".to_string(), |d| format!("{d:+}"))
        ))
        .feed(1)
        .align(Alignment::Left)
        .line("1 solid bar: white hairline = dead dot")
        .raster(&black_bar, RasterQuality::High)
        .line("2 lines every 8 dots: faint/missing = weak dot")
        .raster(&hairlines, RasterQuality::High)
        .line("3 one-dot checkerboard: sharpness")
        .raster(&checkerboard, RasterQuality::High)
        .line("4 grey ramp: banding / streaks")
        .raster(common::ramp(width, 96), RasterQuality::High);

    if double {
        doc = doc
            .line("4b grey ramp, double resolution")
            .print_mode(PrintMode::DoubleResolution)
            .raster(common::ramp(width, 192), RasterQuality::High)
            .print_mode(PrintMode::SingleColor);
    }

    let doc = doc
        .line("5 text and codes")
        .font(ThermalFont::A)
        .line("Font A: The quick brown fox 0123456789")
        .font(ThermalFont::B)
        .line("Font B: The quick brown fox 0123456789")
        .font(ThermalFont::A)
        .wide(2)
        .tall(2)
        .line("Double size")
        .wide(1)
        .tall(1)
        .align(Alignment::Center)
        .barcode(&Barcode::new(Symbology::Code128, "STARPRINT")?.human_readable(true))
        .feed(1)
        .qr_code(&QrCode::new("https://github.com/bcsongor/starprint-rs")?)
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!("sent {} bytes to {host}", doc.as_bytes().len());
    Ok(())
}
