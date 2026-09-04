//! Prints a QR code through the shared workflow, on either head.
//!
//! The module size is worth checking against a phone before trusting a
//! small `size` on the SP700: its ribbon spreads dots, and 30 mm is the
//! smallest that has been printed here.
//!
//! Usage: cargo run -p starprint-workflows --example qr --
//!        <printer-host> thermal|impact "<data>" [size=MM] [radius=PCT]
//!        [ecc=l|m|q|h] [align=left|center|right] [caption=TEXT]
//!
//! `radius` is worth a scan of its own. It takes ink off the corners of
//! the symbol and adds it inside them, and nobody has yet measured how
//! much of that a phone will forgive on either head.

mod common;

use starprint_workflows::qr::{Align, Ecc};
use starprint_workflows::{Job, Qr};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: qr <printer-host> thermal|impact \"<data>\" [size=MM] [radius=PCT] [ecc=l|m|q|h] [align=left|center|right] [caption=TEXT]";
    let mut args = std::env::args().skip(1);
    let host = args.next().ok_or(USAGE)?;
    let kind = args.next().ok_or(USAGE)?;
    let data = args.next().ok_or(USAGE)?;

    let flags: Vec<String> = args.collect();
    common::check_flags(
        &flags,
        &["size=", "radius=", "ecc=", "align=", "caption="],
        USAGE,
    )?;
    let value = |key: &str| flags.iter().find_map(|flag| flag.strip_prefix(key));

    let defaults = Qr::default();
    let code = Qr {
        data,
        caption: value("caption=").map(str::to_owned),
        error_correction: match value("ecc=") {
            None | Some("m") => Ecc::M,
            Some("l") => Ecc::L,
            Some("q") => Ecc::Q,
            Some("h") => Ecc::H,
            Some(bad) => return Err(format!("unknown error correction {bad:?}").into()),
        },
        size: value("size=")
            .map(str::parse)
            .transpose()?
            .unwrap_or(defaults.size),
        radius: value("radius=")
            .map(str::parse)
            .transpose()?
            .unwrap_or(defaults.radius),
        align: match value("align=") {
            None | Some("center") => Align::Center,
            Some("left") => Align::Left,
            Some("right") => Align::Right,
            Some(bad) => return Err(format!("unknown alignment {bad:?}").into()),
        },
    };

    let printer = common::printer(&kind, &host, None, USAGE)?;
    common::send(&printer, &Job::Qr(code))
}
