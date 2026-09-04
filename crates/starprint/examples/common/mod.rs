//! What the thermal probes share: their arguments and a grey ramp.
//! Each example compiles this on its own, so a helper one of them does
//! not call is expected.
#![allow(dead_code)]

use std::error::Error;

use starprint::graphics::{Bitmap, Dithering, Grayscale};

/// `<printer-host> [80|112] [flags…]`, with the paper as dots across.
pub struct Probe {
    pub host: String,
    pub width: u32,
    pub flags: Vec<String>,
}

pub fn probe(usage: &str) -> Result<Probe, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let host = args.next().ok_or(usage)?;
    let width = match args.next().as_deref() {
        None | Some("80") => 576,
        Some("112") => 832,
        Some(other) => return Err(format!("unknown paper width {other:?}; {usage}").into()),
    };
    Ok(Probe {
        host,
        width,
        flags: args.collect(),
    })
}

/// The `density=N` flag, if given.
pub fn density(flags: &[String]) -> Result<Option<i8>, Box<dyn Error>> {
    Ok(flags
        .iter()
        .find_map(|flag| flag.strip_prefix("density="))
        .map(str::parse)
        .transpose()?)
}

/// A dithered ramp from black on the left to white on the right.
pub fn ramp(width: u32, height: u32) -> Bitmap {
    let pixels = (0..height)
        .flat_map(|_| (0..width).map(move |x| (x * 255 / (width - 1)) as u8))
        .collect();
    Dithering::FloydSteinberg { threshold: 128 }
        .apply(&Grayscale::new(width, height, pixels).expect("sized buffer"))
        .to_bitmap()
}
