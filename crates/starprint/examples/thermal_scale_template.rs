//! Prints a photograph at a specified physical size on an 80 mm thermal
//! printer, tiling it across overlapping strips when it is wider than the
//! 72 mm print region.
//!
//! Usage: cargo run --example thermal_scale_template --features image --
//!        <printer-host> <image-path> <width-mm> <height-mm>

use image::imageops::FilterType;
use starprint::graphics::{Bitmap, Dithering, Grayscale};
use starprint::transport::TcpTransport;
use starprint::{Alignment, Cut, PrintSpeed, RasterQuality};

const DOTS_PER_MM: f64 = 8.0;
const PAGE_WIDTH_DOTS: u32 = 576;
const TILE_WIDTH_DOTS: u32 = 520;
const OVERLAP_DOTS: u32 = 80;
const WHITE_CUTOFF: u8 = 245;

fn mm_to_dots(mm: f64) -> Result<u32, &'static str> {
    if !mm.is_finite() || mm <= 0.0 {
        return Err("physical dimensions must be positive numbers");
    }
    Ok((mm * DOTS_PER_MM).round() as u32)
}

fn crop_white_border(source: &image::DynamicImage) -> Result<image::GrayImage, &'static str> {
    let gray = source.to_luma8();
    let (mut left, mut top) = (gray.width(), gray.height());
    let (mut right, mut bottom) = (0, 0);

    for (x, y, pixel) in gray.enumerate_pixels() {
        if pixel[0] < WHITE_CUTOFF {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }

    if left > right || top > bottom {
        return Err("image contains no non-white subject");
    }

    Ok(image::imageops::crop_imm(&gray, left, top, right - left + 1, bottom - top + 1).to_image())
}

fn tile_starts(width: u32) -> Vec<u32> {
    if width <= PAGE_WIDTH_DOTS {
        return vec![0];
    }

    let step = TILE_WIDTH_DOTS - OVERLAP_DOTS;
    let mut starts = vec![0];
    let mut last = 0;
    while last + TILE_WIDTH_DOTS < width {
        last = (last + step).min(width - TILE_WIDTH_DOTS);
        starts.push(last);
    }
    starts
}

fn page(source: &Bitmap, start: u32, content_width: u32) -> Bitmap {
    let offset = (PAGE_WIDTH_DOTS - content_width) / 2;
    Bitmap::from_fn(PAGE_WIDTH_DOTS, source.height(), |x, y| {
        x >= offset && x < offset + content_width && source.get(start + x - offset, y)
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: thermal_scale_template <printer-host> <image-path> <width-mm> <height-mm>";
    let mut args = std::env::args().skip(1);
    let host = args.next().ok_or(usage)?;
    let path = args.next().ok_or(usage)?;
    let width_mm: f64 = args.next().ok_or(usage)?.parse()?;
    let height_mm: f64 = args.next().ok_or(usage)?.parse()?;
    if args.next().is_some() {
        return Err(usage.into());
    }

    let width = mm_to_dots(width_mm)?;
    let height = mm_to_dots(height_mm)?;
    let source = image::load_from_memory(&std::fs::read(&path)?)?;
    let cropped = crop_white_border(&source)?;
    let sized = image::imageops::resize(&cropped, width, height, FilterType::Lanczos3);
    let gray = Grayscale::new(width, height, sized.into_raw())?;
    let bitmap = Dithering::FloydSteinberg { threshold: 128 }
        .apply(&gray)
        .to_bitmap();
    let starts = tile_starts(width);

    let mut doc = starprint::starline()
        .print_speed(PrintSpeed::Slow)
        .print_density(2);
    for (index, &start) in starts.iter().enumerate() {
        let content_width = (width - start).min(if starts.len() == 1 {
            PAGE_WIDTH_DOTS
        } else {
            TILE_WIDTH_DOTS
        });
        let bitmap = page(&bitmap, start, content_width);
        let from_mm = f64::from(start) / DOTS_PER_MM;
        let to_mm = f64::from(start + content_width) / DOTS_PER_MM;

        doc = doc
            .align(Alignment::Center)
            .line(&format!(
                "TEMPLATE TILE {}/{}  x={from_mm:.1}-{to_mm:.1} mm",
                index + 1,
                starts.len()
            ))
            .align(Alignment::Left)
            .raster(&bitmap, RasterQuality::High)
            .feed(2)
            .cut(Cut::FeedThenPartial);
    }
    let doc = doc.build();

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!(
        "printed {width_mm:.1} x {height_mm:.1} mm as {} overlapping tile(s); {} bytes sent",
        starts.len(),
        doc.as_bytes().len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_template_uses_overlapping_tiles() {
        assert_eq!(tile_starts(955), [0, 435]);
    }

    #[test]
    fn narrow_template_stays_on_one_page() {
        assert_eq!(tile_starts(576), [0]);
    }
}
