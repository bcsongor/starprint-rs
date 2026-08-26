//! Prints a photo on a Star Line Mode thermal printer via raster mode.
//!
//! Usage: cargo run --example thermal_image --features image -- <printer-host> <image-path> [double] [slow] [density=N] [brightness=F] [contrast=F]
//!
//! `double` selects the double-resolution print mode of the TSP700II /
//! TSP800II (16 dot rows per mm) and prepares the image accordingly;
//! `slow` prints at slow speed; `density=N` sets print density from -3
//! (lightest) to 3 (darkest), 0 being the printer's standard;
//! `brightness=F` / `contrast=F` are pipeline adjustments (1.0 = as is);
//! `gamma=F` and `equalize=0|1` override the profile's tone curve.

use starprint::graphics::{DeviceProfile, ImagePipeline, ToneCurve};
use starprint::transport::TcpTransport;
use starprint::{Cut, PrintMode, PrintSpeed, RasterQuality};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: thermal_image <printer-host> <image-path> [double] [slow] [density=N] [brightness=F] [contrast=F] [gamma=F] [equalize=0|1]";
    let host = args.next().ok_or(usage)?;
    let path = args.next().ok_or(usage)?;
    let flags: Vec<String> = args.collect();
    if let Some(bad) = flags.iter().find(|f| {
        !matches!(f.as_str(), "double" | "slow")
            && ![
                "density=",
                "brightness=",
                "contrast=",
                "gamma=",
                "equalize=",
            ]
            .iter()
            .any(|p| f.starts_with(p))
    }) {
        return Err(format!("unknown option {bad:?}; {usage}").into());
    }
    let double = flags.iter().any(|f| f == "double");
    let slow = flags.iter().any(|f| f == "slow");
    let value = |key: &str| flags.iter().find_map(|f| f.strip_prefix(key));
    let density: Option<i8> = value("density=").map(str::parse).transpose()?;
    let brightness: f64 = value("brightness=").map_or(Ok(1.0), str::parse)?;
    let contrast: f64 = value("contrast=").map_or(Ok(1.0), str::parse)?;

    let profile = if double {
        DeviceProfile::THERMAL_80MM_DOUBLE_RESOLUTION
    } else {
        DeviceProfile::THERMAL_80MM
    };
    let mut tone = ToneCurve::for_head(profile.head);
    if let Some(g) = value("gamma=") {
        tone.gamma = g.parse()?;
    }
    if let Some(e) = value("equalize=") {
        tone.equalize = e == "1";
    }
    println!(
        "tone curve: gamma {} equalize {}",
        tone.gamma, tone.equalize
    );

    let prepared = ImagePipeline::new()
        .profile(profile)
        .tone(tone)
        .brightness(brightness)
        .contrast(contrast)
        .prepare_bytes(&std::fs::read(&path)?)?;

    let mut doc = starprint::starline();
    if slow {
        doc = doc.print_speed(PrintSpeed::Slow);
    }
    if let Some(level) = density {
        doc = doc.print_density(level);
    }
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
