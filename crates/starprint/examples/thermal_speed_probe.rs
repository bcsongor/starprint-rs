//! Times the same block at each print speed a Star Line Mode printer
//! defines, to find out what the fourth one does on this model.
//!
//! Usage: cargo run --example thermal_speed_probe -- <printer-host> [80|112] [density=N]
//!
//! `ESC RS r n` takes `0 ≤ n ≤ 3` on Spec A printers (TSP700II, TSP800II,
//! TSP650II, FVP10): 0 high, 1 mid, 2 slow, and 3 an "option speed" that
//! the Line Mode specification says "depends on the model" without saying
//! how. [`PrintSpeed`](starprint::PrintSpeed) stops at slow, so the
//! command is written out by hand here.
//!
//! Speed shows up in how long a block takes, not in how it looks, so each
//! block is timed rather than eyeballed: the printer is asked for its
//! status with `ENQ`, whose bit 5 says whether the reception buffer has
//! drained. The blocks are rasters, both because that is what photos use
//! and because they are far bigger than the few-kilobyte buffer — a page
//! of text would be swallowed whole and report itself finished while the
//! head was still printing it.
//!
//! Two things can hide a difference: the pacing the Ethernet cards need
//! (~70 KB/s) can be slower than the head, in which case every speed
//! measures the same because the data is the bottleneck; and the setting
//! is ignored outright in two-colour, low-peak-current and
//! double-resolution modes, which is why single-colour mode is selected
//! first — double resolution outlives `ESC @`.
//!
//! Read the numbers with care. `ENQ` is documented as unusable while ASB
//! (automatic status back) is on, and the IFBD-HE07 answers with ASB
//! frames rather than a bare status byte, so the single byte read here
//! can be a frame byte that happens to have bit 5 set. Measurements on a
//! TSP700II repeated to within a millisecond, so they track something
//! real, but an interface with ASB enabled wants `ESC RS a` switched off
//! for the run, or the frames parsed, before the numbers are trusted.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use starprint::graphics::Bitmap;
use starprint::{Alignment, Cut, PrintMode, RasterQuality};

const ESC: u8 = 0x1B;
const RS: u8 = 0x1E;
/// Real-time status request; the printer answers with one byte.
const ENQ: u8 = 0x05;
/// Bit 5 of the ENQ status: set when the reception buffer is empty.
const BUFFER_EMPTY: u8 = 0x20;

/// The pacing `TcpTransport` uses, mirrored here because this example
/// needs to read from the socket as well as write to it.
const CHUNK: usize = 1400;
const CHUNK_PAUSE: Duration = Duration::from_millis(20);

/// How often the printer is asked whether it has caught up.
const POLL: Duration = Duration::from_millis(20);

/// The speeds to print, in the order they are printed. Slow is printed
/// again at the end so that the head warming up cannot be mistaken for a
/// difference between the settings.
const SPEEDS: [(u8, &str); 4] = [
    (2, "2 slow"),
    (3, "3 option"),
    (0, "0 high"),
    (2, "2 slow #2"),
];

/// Dot rows per block: 40 mm of paper at 8 dots/mm, and about 29 KB,
/// which is several times the reception buffer.
const ROWS: u32 = 320;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: thermal_speed_probe <printer-host> [80|112] [density=N]";
    let host = args.next().ok_or(usage)?;
    let width: u32 = match args.next().as_deref() {
        None | Some("80") => 576,
        Some("112") => 832,
        Some(other) => return Err(format!("unknown paper width {other:?}; {usage}").into()),
    };
    let density: Option<i8> = args
        .find_map(|f| {
            f.strip_prefix("density=")
                .map(|n| n.parse::<i8>().map_err(|e| e.to_string()))
        })
        .transpose()?;

    let block = Bitmap::from_fn(width, ROWS, |x, y| (x + y) % 2 == 0);

    let mut printer = TcpStream::connect((host.as_str(), 9100))?;
    printer.set_nodelay(true)?;
    printer.set_read_timeout(Some(Duration::from_millis(500)))?;

    println!(
        "{:<12}{:>10}{:>10}{:>10}",
        "speed", "sent", "drained", "total"
    );
    let mut results = Vec::new();
    for (n, name) in SPEEDS {
        let mut doc = starprint::starline()
            // The speed setting is ignored in double-resolution mode,
            // which survives ESC @ and so may still be on from earlier.
            .print_mode(PrintMode::SingleColor);
        if let Some(level) = density {
            doc = doc.print_density(level);
        }
        let doc = doc
            .raw([ESC, RS, b'r', n])
            .align(Alignment::Left)
            .line(&format!("ESC RS r {n}: {name}"))
            .raster(&block, RasterQuality::High)
            .feed(1)
            .build();

        wait_until_idle(&mut printer)?;
        let start = Instant::now();
        send_paced(&mut printer, doc.as_bytes())?;
        let sent = start.elapsed();
        let drained = wait_until_idle(&mut printer)?;
        let total = start.elapsed();
        println!("{name:<12}{sent:>10.2?}{drained:>10.2?}{total:>10.2?}");
        results.push((name, total));
    }

    let mut end = starprint::starline().raw([ESC, RS, b'r', 2]);
    end = end.feed(1).cut(Cut::FeedThenPartial);
    send_paced(&mut printer, end.build().as_bytes())?;

    let slowest = results.iter().map(|(_, t)| *t).max().unwrap_or_default();
    let fastest = results.iter().map(|(_, t)| *t).min().unwrap_or_default();
    println!(
        "\nspread {:.2?} between fastest and slowest",
        slowest - fastest
    );
    if slowest.saturating_sub(fastest) < Duration::from_millis(100) {
        println!(
            "that is within the noise: at this pacing the data, not the \
             head, is setting the pace"
        );
    }
    Ok(())
}

/// Writes at the rate the Star Ethernet cards tolerate.
fn send_paced(printer: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    for chunk in bytes.chunks(CHUNK) {
        printer.write_all(chunk)?;
        printer.flush()?;
        std::thread::sleep(CHUNK_PAUSE);
    }
    Ok(())
}

/// Polls `ENQ` until the printer says its reception buffer is empty, and
/// answers with how long that took. A busy printer sometimes lets a
/// status request go unanswered, so a few silent replies are tolerated
/// before giving up.
fn wait_until_idle(printer: &mut TcpStream) -> Result<Duration, Box<dyn std::error::Error>> {
    const SILENCE_ALLOWED: u32 = 20;

    let start = Instant::now();
    let mut silent = 0;
    loop {
        printer.write_all(&[ENQ])?;
        let mut status = [0u8; 1];
        match printer.read_exact(&mut status) {
            Ok(()) if status[0] & BUFFER_EMPTY != 0 => return Ok(start.elapsed()),
            Ok(()) => silent = 0,
            Err(e) if silent < SILENCE_ALLOWED => {
                silent += 1;
                eprintln!("  no status ({e}), retrying");
            }
            Err(e) => {
                return Err(format!(
                    "the printer stopped answering ({e}); is ASB enabled on this interface?"
                )
                .into());
            }
        }
        std::thread::sleep(POLL);
    }
}
