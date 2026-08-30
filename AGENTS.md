# Working on starprint

For contributors; the README is for users.

## Layout

A Cargo workspace: `crates/starprint` is the library and default member,
with two front ends on the jobs in `crates/starprint-workflows`.

- `crates/starprint/src/document.rs`: `Builder<P>` and the command
  encoding. Every command cites the manual it comes from.
- `crates/starprint/tests/fixtures/`: golden output from the Python
  reference in the sibling `starprint` repo (`generate.py`). The impact
  pipeline, dithering and `ESC ^` serialisation must stay byte-identical.
- `crates/starprint-workflows/`: the printer profile, the tagged `Job`
  and the five jobs both front ends print: task cards, text, note slips,
  pictures and test pages. It reads no files and opens no sockets, so a
  picture job is handed its image. Job behaviour belongs here, not in a
  front end, or the two drift.
- `apps/starprint-gui/`: Vite + React + shadcn/ui, with the Rust side in
  `src-tauri/`. Run with `bun tauri dev` from that directory. It adds
  file selection, the source cache and the previews.
- `apps/starprint-api/`: an HTTP server over the same jobs, for other
  local programs. The README covers the endpoints. One concern per
  module: `body` reads a request, `job` turns it into printable bytes,
  `printers` writes them, `config` reads the profile file, `problem` is
  the only error shape and `app` wires them together. Anything new goes
  in whichever of those owns it.
- `manuals/`: the Star specifications. Check bytes there, not from memory.

## Conventions

- Before committing, run `cargo test --all-targets`, clippy with
  `-D warnings` and `cargo fmt --all --check`, both with and without
  `--features image`, then the same for
  `-p starprint-workflows -p starprint-api`. CI does the same. The
  desktop app is checked with `-p starprint-gui`.
- `rust-toolchain.toml` pins the compiler, so those checks give the same
  answer here as on CI. Bumping it can turn up new lints; do it on its
  own commit.
- Rust API guidelines for names; anything the `Builder` accepts lives at
  the crate root.
- British English. Commits are subject-only, imperative, at most 72
  characters.

## Hardware

Test printers, named by series: Star TSP700II and TSP800II (thermal; the
TSP800II runs 80 mm paper via a spacer, print width memory switch set to
80 mm) and a Star SP700 (impact). Nothing here can be checked without
them, so do not retune these values from a screen:

- `ToneCurve::IMPACT` (gamma 1.8, equalise) is the hardware-tuned
  reference and must not change. `ToneCurve::THERMAL` (gamma 0.55, no
  equalise) was dialled in at slow speed, density +3, `RasterQuality::High`;
  those are the settings photos print best with.
- The GUI preview draws thermal dots at 150 % of the pitch at normal
  resolution and 200 % in double, measured against printed step wedges.
- `Pacing::STAR_ETHERNET` (1400 bytes every 20 ms) is the rate at which
  the IFBD-HE07/08 cards never dropped a job.

Behaviour that shapes how jobs are written:

- `PrintMode::DoubleResolution` outlives `ESC @` and the job that set
  it, so select the mode at the start of a job and switch back at the
  end. It doubles rows only (16 rows/mm at 8 dots/mm across), darkens
  rasters, and prints the printer's fonts at half height.
- The TSP800II buffers about 2,560 raster rows and pauses to print them,
  leaving a faint line across a long image (~320 mm at normal
  resolution, ~160 mm in double).

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
