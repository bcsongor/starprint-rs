<div align="center">

# starprint

**Rust driver for Star Micronics receipt printers over Ethernet**

Build a receipt, ticket or photo with a typed builder and send it to the printer on port 9100.

</div>

## Install

```toml
[dependencies]
starprint = { git = "https://github.com/bcsongor/starprint-rs" }
```

Add `features = ["image"]` to print photos.

## Quickstart

```rust
use starprint::{Alignment, Cut, QrCode};
use starprint::transport::TcpTransport;

fn main() -> Result<(), starprint::Error> {
    let receipt = starprint::starline()
        .align(Alignment::Center)
        .wide(2).tall(2)
        .line("ACME STORE")
        .wide(1).tall(1)
        .align(Alignment::Left)
        .line("1x Flat white           4.20")
        .qr_code(&QrCode::new("https://example.com/r/42")?)
        .cut(Cut::FeedThenPartial)
        .build();

    TcpTransport::connect("192.168.1.60")?.print(&receipt)
}
```

A thermal printer prints the header in double size, the line item, a QR code, then feeds and cuts.

## Supported printers

| Command set | Printers | Builder |
|---|---|---|
| Star Line Mode | Thermal: TSP100 (Line Mode), TSP650II, TSP700II, TSP800II | `starprint::starline()` |
| Star Mode, dot impact | SP700 series: SP712, SP742, SP717, SP747 | `starprint::impact()` |

Each builder exposes only the commands its printer understands. `qr_code` exists on the thermal builder and red/black `color` on the impact builder; calling the wrong one is a compile error. Byte sequences come from Star's published command specifications in `manuals/`.

## Printing images

The `image` feature adds a pipeline tuned on real hardware: auto-contrast, tone curve, unsharp mask, resize for the head's dot geometry, then dithering (Floyd-Steinberg by default; Atkinson, Bayer 8x8 and threshold available). A `DeviceProfile` chooses the geometry and tone curve for the head.

Photo on a thermal printer, with the settings that printed best on a TSP800II:

```rust
use starprint::graphics::{DeviceProfile, ImagePipeline};
use starprint::{PrintSpeed, RasterQuality};

let prepared = ImagePipeline::new()
    .profile(DeviceProfile::THERMAL_80MM)
    .prepare_bytes(&std::fs::read("photo.jpg")?)?;

let doc = starprint::starline()
    .print_speed(PrintSpeed::Slow)
    .print_density(3)
    .raster(&prepared.image, RasterQuality::High)
    .build();
```

Photo on an SP700 impact printer:

```rust
use starprint::graphics::{Density, ImagePipeline};

let prepared = ImagePipeline::new()
    .density(Density::Double)
    .prepare_bytes(&std::fs::read("photo.jpg")?)?;

let doc = starprint::impact().bit_image(&prepared.image).build();
```

`prepared.preview` is a square-pixel grayscale for showing on screen. Without the feature, `graphics::Bitmap` still lets you print your own pixels.

## Examples

Run with `cargo run --example <name> -- <printer-ip>`; the photo examples also need `--features image`.

- **thermal_receipt:** a receipt with a total, a Code 128 barcode and a QR code.
- **impact_kitchen_ticket:** a red/black ticket for an SP700.
- **receipt:** a till receipt with line items, service charge and total, built once for either printer; pass `thermal` or `impact` after the address.
- **task_card:** the task card from the desktop GUI, with priority and due-date shortcuts; supports both thermal and impact printers.
- **thermal_test_pattern:** a head-check page; a white hairline through the solid bar means a dead element.
- **thermal_image, impact_image:** print a photo, with `rotate`, `double`, `slow`, `density=N` and `gamma=F` flags.

## Desktop app

`apps/starprint-gui` is a Tauri app for printing without writing code; it currently prints task cards. Run it with `bun install && bun tauri dev` from that directory, or `bun tauri build` for an installer.

## Notes

- **Pacing:** Star's Ethernet cards drop a large job sent all at once, so `TcpTransport` writes 1400 bytes every 20 ms. Send one job at a time; `set_pacing` tunes or disables it.
- **Double resolution:** `PrintMode::DoubleResolution` prints 16 rows/mm. Pair it with a `*_DOUBLE_RESOLUTION` profile and switch back afterwards; the mode survives `ESC @`.
- **Long images:** the TSP800II buffers about 2,560 raster rows and pauses to print them, leaving a faint line past roughly 320 mm (160 mm at double resolution).
- **Text encoding:** `text()` encodes into CP437; unmappable characters print as `?`. For other code pages call `code_page()` and pass encoded bytes to `raw()`.
- **Not implemented:** stored logos, status back (ASB), StarPRNT (mc-Print, TSP100IV), USB and serial transports.

## License

MIT or Apache-2.0, at your option.
