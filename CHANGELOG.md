# Changelog

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
  `TcpTransport` writes 1400 bytes every 20 ms and callers send one job
  at a time.
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
