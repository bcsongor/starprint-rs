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

## Supported printers

| Command set | Printers | Builder |
|---|---|---|
| Star Line Mode | Thermal: TSP100 (Line Mode), TSP650II, TSP700II, TSP800II | `starprint::starline()` |
| Star Mode, dot impact | SP700 series: SP712, SP742, SP717, SP747 | `starprint::impact()` |

Each builder only has the commands its printer understands, so `qr_code` on the impact builder is a compile error. Bytes come from Star's command specifications in `manuals/`.

## Printing images

The `image` feature adds a pipeline tuned on real hardware: auto-contrast, tone curve, unsharp mask, resize for the head, dither. A `DeviceProfile` picks the geometry and tone curve.

On a thermal printer, with the settings that printed best on a TSP800II:

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

On an SP700:

```rust
use starprint::graphics::{Density, ImagePipeline};

let prepared = ImagePipeline::new()
    .density(Density::Double)
    .prepare_bytes(&std::fs::read("photo.jpg")?)?;

let doc = starprint::impact().bit_image(&prepared.image).build();
```

`prepared.preview` is a grayscale for the screen. Without the feature, `graphics::Bitmap` prints your own pixels.

## Examples

Run the driver examples with `cargo run --example <name> -- <printer-ip>`; the photo examples also need `--features image`.

- `thermal_receipt`: a receipt with a Code 128 barcode and a QR code.
- `impact_kitchen_ticket`: a red/black ticket for an SP700.
- `receipt`: a till receipt for either printer; pass `thermal` or `impact`.
- `task_card`: the desktop GUI's task card, for either printer; run with `cargo run -p starprint-workflows --example task_card -- …`.
- `thermal_test_pattern`: a head-check page.
- `thermal_image`, `impact_image`: a photo, with `rotate`, `double`, `slow`, `density=N` and `gamma=F` flags.

## Desktop app

`apps/starprint-gui` is a Tauri app that prints task cards, text, pictures and test pages. Run it with `bun install && bun tauri dev` from that directory, or `bun tauri build` for an installer.

## HTTP API

`apps/starprint-api` serves the same jobs to other local programs, mostly AI agents. Name your printers in a TOML file and post jobs to them:

```console
$ cp apps/starprint-api/printers.toml.example printers.toml
$ cargo run -p starprint-api
starprint-api: tsp800ii, sp743 from printers.toml
starprint-api: listening on http://127.0.0.1:9110
```

```console
$ curl -X POST http://127.0.0.1:9110/v1/printers/tsp800ii/jobs \
    -H 'Content-Type: application/json' \
    -d '{"job":{"kind":"task-card","text":"Renew passport","due":"2026-09-15"}}'
{"bytesSent":284}
```

It binds the loopback interface by default and has no authentication, so `--listen` anywhere the network can reach hands your printers to whoever asks.

`GET /v1/printers` lists what the profile file named. `POST /v1/printers/<name>/jobs` prints a `task-card`, `text`, `note`, `picture` or `test-page`, taking `cut`, `density` and `speed` overrides; a picture goes as `multipart/form-data` with the image in an `image` part. `POST /v1/printers/<name>/raw` takes bytes that already carry their own commands. Both return `{"bytesSent": N}`, which reports a completed socket write and nothing more.

## Notes

- Star's Ethernet cards drop a large job sent at once, so `TcpTransport` writes 1400 bytes every 20 ms. Send one job at a time.
- `PrintMode::DoubleResolution` prints 16 rows/mm. Pair it with a `*_DOUBLE_RESOLUTION` profile and switch back afterwards; the mode survives `ESC @`.
- The TSP800II buffers about 2,560 raster rows and pauses to print them, leaving a faint line past roughly 320 mm (160 mm at double resolution).
- `text()` encodes CP437; unmappable characters print as `?`. For other code pages call `code_page()` and pass encoded bytes to `raw()`.
- Not implemented: stored logos, status back (ASB), StarPRNT (mc-Print, TSP100IV), USB and serial transports.

## License

MIT or Apache-2.0, at your option.
