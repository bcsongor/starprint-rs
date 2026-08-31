//! Prints a QR code through the shared workflow, on either head.
//!
//! The module size is worth checking against a phone before trusting a
//! small `size` on the SP700: its ribbon spreads dots, and 30 mm is the
//! smallest that has been printed here.
//!
//! Usage: cargo run -p starprint-workflows --example qr --
//!        <printer-host> thermal|impact "<data>" [size=MM]
//!        [ecc=l|m|q|h] [align=left|center|right] [caption=TEXT]

use starprint::transport::TcpTransport;
use starprint_workflows::qr::{Align, Ecc};
use starprint_workflows::{Job, Paper, Printer, Qr, Speed};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: qr <printer-host> thermal|impact \"<data>\" [size=MM] [ecc=l|m|q|h] [align=left|center|right] [caption=TEXT]";
    let mut args = std::env::args().skip(1);
    let host = args.next().ok_or(USAGE)?;
    let kind = args.next().ok_or(USAGE)?;
    let data = args.next().ok_or(USAGE)?;

    let flags: Vec<String> = args.collect();
    if let Some(bad) = flags.iter().find(|flag| {
        !["size=", "ecc=", "align=", "caption="]
            .iter()
            .any(|prefix| flag.starts_with(prefix))
    }) {
        return Err(format!("unknown option {bad:?}; {USAGE}").into());
    }
    let value = |key: &str| flags.iter().find_map(|flag| flag.strip_prefix(key));

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
        size: value("size=").map(str::parse).transpose()?.unwrap_or(30),
        align: match value("align=") {
            None | Some("center") => Align::Center,
            Some("left") => Align::Left,
            Some("right") => Align::Right,
            Some(bad) => return Err(format!("unknown alignment {bad:?}").into()),
        },
    };

    let printer = match kind.as_str() {
        "thermal" => Printer::thermal(host.clone(), 9100, Paper::Mm80, 3, Speed::Slow),
        "impact" => Printer::impact(host.clone(), 9100),
        _ => return Err(USAGE.into()),
    };
    let document = printer.document(&Job::Qr(code), None)?;

    let mut transport = TcpTransport::connect(&host)?;
    transport.print(&document)?;
    println!("sent {} bytes to {host}", document.as_bytes().len());
    Ok(())
}
