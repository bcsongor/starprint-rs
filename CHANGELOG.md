# Changelog

## Unreleased

### HTTP API

- **The server keeps a data directory** with its profiles, schedules
  and token, at `starprint` under the platform's configuration
  directory or wherever `--data` points. `--config` and `printers.toml`
  are gone: profiles are `printers.json` in that directory, in the shape
  the API takes them. The token now persists across restarts; `--token`
  sets it.
- **`GET /v1/printers` returns `{ "version", "printers" }`** instead of
  a bare array, and each profile now includes its `host` and `port`.
- Profiles are managed over the API: `PUT /v1/printers/{name}` creates
  or replaces one, `DELETE` removes it, and `GET
  /v1/printers/{name}/status` reports whether the printer answers.
- Schedules run on the server: `GET`, `POST`, `PUT` and `DELETE` under
  `/v1/schedules`, printed on the server's local clock. The reference
  is [`apps/starprint-api/API.md`](apps/starprint-api/API.md).

### Desktop app

- The scheduler no longer stops at the hour the clocks go back.

## 1.0.0

**Print to Star receipt printers from Windows, macOS, scripts and AI agents.**
Starprint 1.0 is the first public release.

### Printing

- Jobs to the same host and port share a queue, including duplicate
  profiles and manual, Linear and API jobs. Probes wait for jobs, and
  cancelling a request no longer lets another job interrupt its write.

### HTTP API

- **Every API request now requires a bearer token.** Requests without
  one get a `401`. Pass `--token` to the server or use the token it
  generates and prints at startup.
- The desktop app generates and saves its own token. The **API** button
  shows the URL and token with copy buttons, lets you choose the listen
  address and port, and stops the server. It listens on loopback port
  9110 by default.

### Desktop app

- **Task and Text previews no longer jump when you change paper size.**
  Previews on macOS also stop repeatedly resizing.
- The **Print** button stays enabled while picture settings refresh the preview.
- Impact profiles show **76 mm paper** and hide density and speed controls.
- New profiles start without an address.
- API auto-start waits for the saved profiles and listen address to load.
- Task-card previews preserve blank lines, matching the printed card.
- Arrow keys move focus between dates in the calendar.

## 0.4.0

### Printing

- Density +4 on a thermal profile, in the desktop app, the API and the
  profile file, selects the printer's two-colour mode, which prints a
  darker black than +3 on plain paper. The mode ignores the density and
  speed commands, so neither is sent at +4, and anything in double
  resolution prints at +3 instead. The desktop app disables double
  resolution for pictures at +4.
- A QR code on a thermal printer now lands where its alignment says.
  Raster rows start at the raster margin and ignore `ESC GS a`, so
  left, centre and right all printed at the left edge. The symbol is
  padded with blank dots to its place; the caption still uses text
  alignment.
- A `qr` job's `radius` defaults to 0, square, since no rounded symbol
  has been checked against a phone on either head yet.
- A task card without a due date keeps its reference where a dated one
  puts it. The header reserved no room for the date, so the reference
  drifted right whenever an issue had none.

### Desktop app

- Only the active workflow's preview is drawn, and preview PNGs are
  encoded for speed, so switching printers and dragging sliders is
  quicker. Print is enabled as soon as a QR code has data rather than
  once its preview lands, so the button no longer flickers on a drag.
- A picture too short to print at the paper's width is reported by the
  preview rather than failing at print time.
- Smaller fixes: note ruling stays crisp when the window is resized,
  the preview keeps still while a popup opens, switching printers no
  longer flashes a scrollbar, the task input no longer shrinks below
  one line, profiles are listed alphabetically and the density
  selector's digits line up.

### Library

- `ImagePipeline::prepare` returns the `BitImage` and `prepare_preview`
  the `Grayscale` for the screen, in place of `PreparedImage`, so a
  preview no longer builds the raster it does not show. Both reject an
  image too short to print with the same error.
- Rasters are packed straight into the document and dithering keeps
  three rows of error rather than a copy of the whole image, so a photo
  prepares with far less memory and copying. The desktop app shares the
  decoded picture between previews instead of copying it.
- `Builder::resume` continues a finished document where it ended, which
  is how a two-colour job hands the printer back in single colour
  without copying its bytes.

## 0.3.0

### Linear

- The desktop app can print a task card for each open Linear issue newly
  assigned to you. A personal API key and a toggle sit under the Task
  tab; while it is on, the app asks Linear every 10 seconds and prints
  whatever the previous answer did not list. The first answer only takes
  stock, so switching it on does not print the backlog.

### HTTP API

- The desktop app runs the API server itself, behind a button in the
  toolbar. It serves the profiles that have a host, under the names the
  picker shows, and starts again on the new set when a profile is
  edited. Nothing else is needed for an agent to print through the app.

### Desktop app

- The preview on an SP700 profile no longer scrolls the window. A hidden
  span that measures the preview font was wider than the SP700's sheet
  and leaked past the pane edge.

### QR codes

- A `qr` job prints a QR code, with an optional caption above it, a size
  in millimetres, an alignment and a choice of error correction. Both
  heads print one.
- The symbol is encoded by `starprint-workflows` and drawn as dots
  rather than handed to `ESC GS y`. The SP700 has no QR command at all,
  and one bitmap for both heads means one code path, the same square on
  either printer, and a version known here rather than chosen inside the
  firmware. `Builder::qr_code` is untouched for callers of the library.
- The SP700's dots are 169 to the inch across and 72 down, so a module
  is drawn 7 dots by 3 at double density, within 1 % of square. A symbol
  too wide for the paper shrinks until it fits. A 30 mm code printed
  this way scans off the ribbon, which is what set the default size.
- The desktop app has a QR tab. Because the modules are ours rather than
  the firmware's, its preview draws the symbol that will print, at the
  millimetres it will measure, rather than an approximation of one.
- A `radius` rounds the corners of the symbol, each as far as its shape
  allows, so a lone module becomes a circle early on while the finder
  patterns keep rounding. 100, as round as it goes, by default.

## 0.2.0

### HTTP API

- `starprint-api` serves the desktop app's jobs to other local programs
  over HTTP. It reads named printer profiles from a TOML file at
  startup, prints task cards, text, note slips, pictures and test pages
  to them, and takes raw bytes for anything else.
- A response reports a completed socket write and nothing more. Without
  status back the server cannot tell whether the printer accepted or
  printed the job, so a `502` does not prove that nothing printed.
- It binds the loopback interface by default and has no authentication,
  so `--listen` past it is a decision, not a default. It reaches only
  the printers its profile file names.

### For agents

- A skill at `skills/starprint-print/` tells an agent how to
  print through the API: which printers exist, what each job takes and
  what `bytesSent` does not promise. It stands on its own, so it can be
  copied into any agent that reads the format.

### Shared jobs

- The jobs both front ends print moved from the desktop app into
  `starprint-workflows`, so a card printed from either is byte for byte
  the same. The crate reads no files: a picture job is handed its image.
- Every job now takes defaults for the settings a caller does not care
  about, so a job can be as short as its content.

## 0.1.0

First release. `starprint` is the library, Starprint the desktop app
built on it.

### Library

- `starline()` builds documents for thermal printers, `impact()` for the
  SP700 dot matrix series. Each builder holds only the commands its
  printer understands, so `qr_code()` on the impact builder does not
  compile.
- Text with alignment, bold, underline, overline, wide and tall sizing,
  line spacing and font choice. `text()` encodes CP437 and prints what
  it cannot map as `?`. `code_page()` and `international()` switch
  character sets.
- Barcodes in nine symbologies (UPC-A, UPC-E, EAN-8, EAN-13, Code 39,
  Code 93, Code 128, ITF, NW-7) and QR codes, on thermal printers.
- Raster images on thermal printers, bit image graphics on impact.
  Impact printers also print red as well as black.
- Print speed, print mode and print density on the printer's own -3 to
  +3 scale.
- Cuts, feeds and cash drawer pulses.
- `TcpTransport` sends a finished document to port 9100.
- The `image` feature prepares photos: auto contrast, tone curve,
  unsharp mask, resize for the head, then dither. `DeviceProfile` sets
  the geometry and tone curve for a paper width and head, including
  112 mm and double resolution.
- `manuals/` holds Star's command specifications. Every command in the
  source cites the manual its bytes come from.

### Desktop app

- Prints task cards, plain text, pictures and test pages.
- Keeps a profile per printer and shows which ones answer.
- Draws previews on the paper the printer holds, at the size the dots
  come out rather than one screen pixel each.

### Hardware notes

Printing on a TSP800II and an SP700 set these defaults.

- Star's Ethernet cards drop a large job sent in one go, so
  `TcpTransport` writes 1400 bytes every 20 ms.
- Thermal and impact heads want different tone curves. The impact curve
  crushes shadows on a thermal head, so the profile picks the curve by
  head kind.
- Cheap thermal paper pinholes in solid black at the default speed. Slow
  speed with density +2 or +3 clears it.
- Double resolution outlives `ESC @` and the job that set it, so a job
  selects the mode it wants at the start and switches back at the end.

### Not implemented

Stored logos, status back (ASB), StarPRNT for the mc-Print and TSP100IV,
and USB or serial transports.
