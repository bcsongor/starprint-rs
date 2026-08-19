# starprint

Talk to Star Micronics receipt printers from Rust — dependency-free, over
Ethernet.

`starprint` implements the two classic Star command sets:

| Protocol | Marker type | Printers | Reference |
|---|---|---|---|
| **Star Line Mode** | `StarLine` | Thermal receipt printers: TSP100 (Line Mode), TSP650II, TSP700II, TSP800II, … | *Line Thermal Printer — Star Line Mode Command Specifications* |
| **Star Mode (dot impact)** | `Impact` | SP700-series impact printers: SP712, SP742, SP717, SP747 | *Dot Impact Printer — STAR Command Specifications* |

The design splits cleanly in two:

* **Protocol layer** — a fluent `Builder<P>` renders a `Document` (plain
  bytes). It is pure: no I/O, trivially testable, and documents can be
  spooled or queued.
* **Transport layer** — a `Transport` delivers the bytes. `TcpTransport`
  ships in the box (raw-socket printing on TCP port 9100, which every
  networked Star printer supports). Implement the one-method `Transport`
  trait to add USB/serial/Bluetooth later.

Capabilities are enforced by the type system: `qr_code`, `barcode`,
`invert` and thermal fonts exist only on `Builder<StarLine>`; red/black
two-colour printing and the impact fonts exist only on `Builder<Impact>`.
Sending an unsupported command is a compile error, not a mangled receipt.
Where capabilities differ in shape, so does the API: thermal printers
scale characters ×1–×6 (`wide(n)`/`tall(n)`), while impact printers only
toggle double size (`double_wide(bool)`/`double_tall(bool)`).

## Quick start

```rust
use starprint::{Alignment, Cut, QrCode};
use starprint::transport::TcpTransport;

let receipt = starprint::starline()
    .align(Alignment::Center)
    .wide(2)
    .tall(2)
    .line("ACME STORE")
    .wide(1)
    .tall(1)
    .feed(1)
    .align(Alignment::Left)
    .line("1x Flat white           4.20")
    .line("1x Croissant            3.80")
    .bold(true)
    .line("TOTAL                   8.00")
    .bold(false)
    .feed(1)
    .align(Alignment::Center)
    .qr_code(&QrCode::new("https://example.com/receipt/42")?)
    .cut(Cut::FeedThenPartial)
    .build();

let mut printer = TcpTransport::connect("192.168.1.60")?; // port 9100 implied
printer.print(&receipt)?;
```

A kitchen ticket on an SP700 impact printer, with the allergy warning in
red:

```rust
use starprint::{Alignment, Color, Cut};

let ticket = starprint::impact()
    .two_color(true) // requires a black/red ribbon
    .align(Alignment::Center)
    .double_wide(true)
    .double_tall(true)
    .line("TABLE 7")
    .double_wide(false)
    .double_tall(false)
    .align(Alignment::Left)
    .line("2x Carbonara")
    .line("1x Margherita")
    .color(Color::Red)
    .line("** ALLERGY: NUTS **")
    .color(Color::Black)
    .cut(Cut::FeedThenPartial)
    .build();
```

Runnable versions live in `examples/`:

```console
cargo run --example thermal_receipt -- 192.168.1.60
cargo run --example impact_kitchen_ticket -- 192.168.1.61
cargo run --example impact_image --features image -- 192.168.1.61 photo.jpg double
```

## Feature overview

Shared by both protocols: text (CP437-encoded), alignment, bold,
underline, overline, line feeds, paper cut, cash-drawer pulses,
international character sets, code-page selection, and a `raw` escape
hatch for anything the builder does not model.

Star Line Mode (thermal) additionally: character scaling up to ×6 per
axis (`wide`/`tall`), 1D barcodes (UPC-A/E, EAN-8/13, Code 39/93/128,
ITF, NW-7) with payload validation at construction time, QR codes,
white/black inverted printing, font A/B/OCR-B, line-spacing selection.

Star Mode dot impact additionally: double-wide/double-tall characters,
red/black two-colour printing, the 7×9 / 5×9 impact fonts, and 9-dot
bit-image graphics (see below).

## Printing images (impact)

Image support is layered so the core crate stays dependency-free:

* `graphics` — a 1-bit `Bitmap`, and `BitImage` which validates it
  against the head width (210 dots single / 420 dots double density on
  the SP700) so `Builder::bit_image` cannot fail. Emitted as `ESC ^`
  9-dot stripes with stripe-exact line spacing.
* `dither` — grayscale buffers plus Floyd–Steinberg, Atkinson, Bayer 8×8
  and threshold dithering.
* `pipeline` (cargo feature `image`) — the full preparation pipeline,
  ported from a reference implementation tuned on real hardware:
  auto-contrast → gamma 1.8 → unsharp mask → histogram equalisation →
  density-dependent brightening → optional user brightness/contrast →
  anisotropic-DPI-compensated resize → dither. Returns the print-ready
  `BitImage` plus a square-pixel preview (min-pooled at double density
  to simulate dot overlap).

```rust
use starprint::graphics::Density;
use starprint::pipeline::ImagePipeline;

let photo = std::fs::read("photo.jpg")?;
let prepared = ImagePipeline::new()
    .density(Density::Double)
    .prepare_bytes(&photo)?;
let doc = starprint::impact().bit_image(&prepared.image).build();
```

The dithering algorithms, `ESC ^` serialisation and every LUT-based tone
stage are verified byte-for-byte against fixtures generated by the
reference implementation (`tests/fixtures/generate.py`); resampling and
the unsharp blur are equivalent but not bit-identical (Lanczos3 and a
true Gaussian in place of Pillow's C resampler and box-blur
approximation).

## Text encoding

Star printers do not speak UTF-8; bytes are looked up in the selected
code page. `Builder::new()` therefore starts every document with `ESC @`
(reset) followed by selecting CP437, and `text()` encodes strings as
CP437 (unmappable characters print as `?`). For other code pages, select
one with `code_page()` and pass pre-encoded bytes through `raw()`.

## Scope and roadmap

Current scope is deliberate: the two classic command sets over TCP,
plus impact bit-image graphics. Not yet implemented: thermal raster
graphics / stored logos, status back (ASB) parsing, the newer
**StarPRNT** protocol (mc-Print, TSP100IV), and USB/serial transports.

## License

MIT OR Apache-2.0
