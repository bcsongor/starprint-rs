# Working on starprint

For contributors; the README is for users.

## Layout

A Cargo workspace: `crates/starprint` is the library and default member,
`apps/starprint-gui` a Tauri app on it.

- `crates/starprint/src/document.rs`: `Builder<P>` and the command
  encoding. Every command cites the manual it comes from.
- `crates/starprint/tests/fixtures/`: golden output from the Python
  reference in the sibling `starprint` repo (`generate.py`). The impact
  pipeline, dithering and `ESC ^` serialisation must stay byte-identical.
- `apps/starprint-gui/`: Vite + React + shadcn/ui, with the Rust side in
  `src-tauri/`. Run with `bun tauri dev` from that directory.
- `manuals/`: the Star specifications. Check bytes there, not from memory.

## Conventions

- Run `cargo test --all-targets` and `cargo test --features image`, plus
  clippy with `-D warnings` and `cargo fmt --check`, in both feature
  configurations before committing. CI does the same.
- Rust API guidelines for names; anything the `Builder` accepts lives at
  the crate root.
- British English. Commits are subject-only, imperative, at most 72
  characters.

## Hardware facts learnt on real printers

Test printers: a Star TSP800II (thermal, 80 mm paper via a spacer, print
width set to 80 mm) and a Star SP700 (impact).

- **Ethernet cards drop data.** The IFBD-HE07/08 cards do not apply TCP
  back-pressure: fed a large job at once, or a job while busy, they
  discard it. Hence `TcpTransport` paces writes (1400 bytes every 20 ms,
  the rate that never lost a job) and callers send one job at a time.
  Status back (ASB) is the proper fix and is not implemented.
- **Raster image buffer.** The TSP800II buffers roughly 2,560 raster rows
  and pauses to print them when full, leaving a faint line across a long
  image: ~320 mm at normal resolution, ~160 mm in double.
- **Thermal photo settings that printed best:** normal resolution, slow
  speed (`print_speed(PrintSpeed::Slow)`), density +3
  (`print_density(3)`), `RasterQuality::High`, with the thermal tone
  curve (`ToneCurve::THERMAL`, gamma 0.55, no equalisation). Double
  resolution (`PrintMode::DoubleResolution` plus a
  `*_DOUBLE_RESOLUTION` profile) is smoother on short images but subject
  to the buffer limit above.
- **Tone curves differ by head.** The impact curve (gamma 1.8 +
  equalise) is the hardware-tuned reference and must not change; on a
  thermal head it crushes shadows, hence `HeadKind`.
- **Cheap thermal paper** pinholes in solid black at default speed and
  density; slow speed and +2/+3 density cure it.
- **Narrow paper on the TSP800II.** Set the print width memory switch to
  80 mm before printing on 80 mm rolls, or the head fires onto bare
  platen. Star warns of head wear from long narrow use; irrelevant at
  hobby volumes.
- **Thermal dot gain.** Each fired element blooms past its pitch, so a
  1-bit dither looks lighter on screen than on paper. Against printed
  step wedges on the TSP700II (slow, density +3), dots are about 150 % of
  the pitch at normal resolution and 200 % in double. The GUI preview
  draws them at those sizes.
- **Double resolution is vertical only:** 16 rows/mm at the same 8
  dots/mm across. The mode outlives `ESC @` *and the job that set it*, so
  select the mode you want at the start of a job and switch back at the
  end. Within a job the printer flushes the line buffer before changing,
  so both halves of one document come out as asked.
- **Double resolution is what darkens a raster, and it is for rasters
  only.** Twice the rows lay down about 1.85× the energy even though
  Star's density table gives that mode a lower ceiling (`+3` is 1.2×
  standard there against 1.3×). The printer's fonts are *not* doubled:
  text prints at half height.
- **Print speed has a fourth value.** `ESC RS r 3` ("option speed") is
  not slower than slow on the TSP700II; slow is the floor. Low peak
  current mode ignores the density command.

## Diagnostics

- `cargo run --example thermal_test_pattern -- <host> 80 slow density=3`
  prints a head-check page.
- `cargo run --example thermal_image --features image -- <host> photo.jpg
  slow density=3 [double] [rotate] [gamma=F] [equalize=0|1]` for photos.
- `cargo run --example impact_image --features image -- <host> photo.jpg
  double [rotate]` for the SP700.
- Three probes print the same content under two settings for comparison:
  `thermal_black_probe` (normal against double resolution),
  `thermal_text_probe` (double resolution on the printer's fonts) and
  `thermal_speed_probe` (times each `ESC RS r` value).
