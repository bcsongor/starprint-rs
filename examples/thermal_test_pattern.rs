//! Prints a head/platen diagnostic page on a Star Line Mode thermal printer.
//!
//! Usage: cargo run --example thermal_test_pattern -- <printer-host> [80|112] [double] [slow] [density=N]
//!
//! Sections, top to bottom:
//! 1. Solid black bar — any white vertical hairline is a dead head element.
//! 2. Single-dot vertical lines every 8 dots — a missing or faint line is a
//!    weak element; uneven spacing means feed/platen trouble.
//! 3. One-dot checkerboard — sharpness and dot gain.
//! 4. Dithered grey ramp, dark to light — look for horizontal banding
//!    (platen / heat) and vertical streaks (element drift).
//! 5. Text in both fonts, sizes and a barcode/QR — general sanity.
//!
//! With `double`, the ramp is repeated in double-resolution mode. `slow`
//! prints at slow speed; `density=N` sets print density from -3
//! (lightest) to 3 (darkest), 0 being the printer's standard.

use starprint::graphics::{Bitmap, Dithering, Grayscale};
use starprint::transport::TcpTransport;
use starprint::{
    Alignment, Barcode, Cut, PrintMode, PrintSpeed, QrCode, RasterQuality, Symbology, ThermalFont,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: thermal_test_pattern <printer-host> [80|112] [double] [slow] [density=N]";
    let host = args.next().ok_or(usage)?;
    let width: u32 = match args.next().as_deref() {
        None | Some("80") => 576,
        Some("112") => 832,
        Some(other) => return Err(format!("unknown paper width {other:?}; {usage}").into()),
    };
    let flags: Vec<String> = args.collect();
    let double = flags.iter().any(|f| f == "double");
    let slow = flags.iter().any(|f| f == "slow");
    let density: Option<i8> = flags
        .iter()
        .find_map(|f| f.strip_prefix("density="))
        .map(|n| n.parse())
        .transpose()?;

    let black_bar = Bitmap::from_fn(width, 64, |_, _| true);
    let hairlines = Bitmap::from_fn(width, 48, |x, _| x % 8 == 0);
    let checkerboard = Bitmap::from_fn(width, 32, |x, y| (x + y) % 2 == 0);
    let ramp = |height: u32| {
        let pixels = (0..height)
            .flat_map(|_| (0..width).map(move |x| (x * 255 / (width - 1)) as u8))
            .collect();
        Dithering::FloydSteinberg { threshold: 128 }
            .apply(&Grayscale::new(width, height, pixels).expect("sized buffer"))
            .to_bitmap()
    };

    let mut doc = starprint::starline();
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
        .raster(ramp(96), RasterQuality::High);

    if double {
        doc = doc
            .line("4b grey ramp, double resolution")
            .print_mode(PrintMode::DoubleResolution)
            .raster(ramp(192), RasterQuality::High)
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
