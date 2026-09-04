//! What the examples share: flag checking, the test printer for a head
//! and the send. Each example compiles this on its own.

use std::error::Error;

use starprint::transport::TcpTransport;
use starprint_workflows::{Job, Paper, Printer, Speed};

/// Rejects a flag not in `allowed`, where an entry ending in `=` allows
/// anything with that prefix.
pub fn check_flags(flags: &[String], allowed: &[&str], usage: &str) -> Result<(), Box<dyn Error>> {
    let known = |flag: &str| {
        allowed.iter().any(|a| {
            if a.ends_with('=') {
                flag.starts_with(a)
            } else {
                flag == *a
            }
        })
    };
    match flags.iter().find(|flag| !known(flag)) {
        Some(bad) => Err(format!("unknown option {bad:?}; {usage}").into()),
        None => Ok(()),
    }
}

/// The test printer for `kind`: an 80 mm thermal roll at slow speed, or
/// the SP700, which has no density to set.
pub fn printer(
    kind: &str,
    host: &str,
    density: Option<i8>,
    usage: &str,
) -> Result<Printer, Box<dyn Error>> {
    match (kind, density) {
        ("thermal", density) => Ok(Printer::thermal(
            host.to_owned(),
            9100,
            Paper::Mm80,
            density.unwrap_or(3),
            Speed::Slow,
        )),
        ("impact", None) => Ok(Printer::impact(host.to_owned(), 9100)),
        ("impact", Some(_)) => Err("density is only available for thermal printers".into()),
        _ => Err(usage.into()),
    }
}

/// Builds the job on the printer and sends it there.
pub fn send(printer: &Printer, job: &Job) -> Result<(), Box<dyn Error>> {
    let document = printer.document(job, None)?;
    let mut transport = TcpTransport::connect(&printer.address())?;
    transport.print(&document)?;
    println!(
        "sent {} bytes to {}",
        document.as_bytes().len(),
        printer.host
    );
    Ok(())
}
