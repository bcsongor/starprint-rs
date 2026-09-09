# Working on starprint

For contributors; the README is for users.

## Layout

A Cargo workspace. `crates/starprint` is the library and default member,
`crates/starprint-workflows` holds the jobs, and two front ends print them.

- `crates/starprint/src/document.rs`: `Builder<P>` and the command
  encoding. Every command cites the manual it comes from.
- `crates/starprint/tests/fixtures/`: golden output from the Python
  reference in the sibling `starprint` repo (`generate.py`). The impact
  pipeline, dithering and `ESC ^` serialisation must stay byte-identical.
- `crates/starprint-workflows/`: the printer profile, the tagged `Job`
  and the six jobs: task cards, text, note slips, QR codes, pictures and
  test pages. It reads no files and opens no sockets, so a picture job
  is handed its image. Job behaviour belongs here, not in a front end,
  or the two drift. `preview` draws what a job will look like: a
  layout for the printer's own fonts, and a PNG from the bitmap that
  prints, with thermal dots widened to the measured size. Both front
  ends show previews from it and nothing else. QR symbols are encoded here by `qrcodegen` and
  printed as dots on both heads. The SP700 has no QR command, and one
  bitmap gives one path, a known version, a preview that draws what
  prints and rounded modules, none of which `ESC GS y` would.
- `apps/starprint-gui/`: Vite + React + shadcn/ui over a Rust side in
  `src-tauri/`. Run with `bun tauri dev` from that directory. It adds
  file selection, the previews, the API button and the Linear
  auto-print, and the schedules. Its preview commands call the
  workflows crate's `preview`. A preview that kept a rule of its own
  drifted once, and squared finder patterns went unnoticed until they
  came off the printer. A schedule is a job as its form stood, a
  profile and a five-field cron expression, kept in the settings store
  by the frontend and shown in a drawer off the header. The Rust side
  (`src-tauri/src/scheduler.rs`) checks the current local minute with
  `croner` and spawns matching jobs through the shared print queue.
  Only schedule definitions are stored. Schedules without a host do
  not run. A task card's due date follows its rule when it prints.
  This is the app's own copy of what the API now does; the app becomes
  a client of its embedded server next, and the copy goes then.
  The API button runs `starprint-api`'s server in-process on the
  profiles that have a host, at an address and port picked under the
  button, behind a token kept in the settings store, and starts it
  again when a profile or the address changes. That server's data is
  in memory only, so a profile or schedule made through it lasts until
  the server next starts, which releasing the button, editing any
  profile or changing the address all cause. Auto-print is frontend
  only: a personal API key in the settings store, a `fetch` against
  Linear's GraphQL endpoint every 10 seconds, and a task card for each
  newly assigned open issue. Linear answers the webview's CORS
  preflight, so no HTTP plugin is involved.
- `apps/starprint-api/`: an HTTP server over the same jobs, for other
  local programs, and the home of everything that has to run
  unattended. A library with a thin command line on top, so the desktop
  app can run the same server. One concern per module; anything new
  goes in whichever of `body`, `job`, `printers`, `config`, `data`,
  `schedule`, `problem`, `app` or `server` owns it. `data` is the data
  directory: `printers.json`, `schedules.json` and `token`, each read
  once at startup and written whole under its lock after every change,
  or kept in memory when the desktop app supplies the profiles. The
  directory is locked while open, so two servers cannot share one.
  `schedule` holds the schedule shape and the loop that prints each one
  on the local clock through the queue; it runs for as long as the
  server does. Shutdown cancels queued scheduled jobs before draining
  HTTP requests; blocking writes already in progress finish under
  their queue guards. Profiles are addressed by name, `PUT` creates or
  replaces, and jobs still cannot carry `host` or `port`. `/preview`
  takes a job's body and answers with the workflows crate's `Preview`,
  so a client needs no job code of its own. The connection
  probe lives here too, behind `/status` and exported as `reachable`,
  so it takes the printer's turn like a job. `printers::PrintQueue`,
  exported at the crate root, serialises connections by host and port.
  The GUI shares one queue across manual jobs, Linear jobs, probes and
  the embedded API, including API restarts. Its guard must live inside
  the blocking task so cancellation cannot release a write still in
  progress.
- `manuals/README.md`: links to Star's specifications, which are Star's
  copyright and not kept here. Check bytes there, not from memory.
- `skills/starprint-print/`: the skill users install into their own
  agents to print through the API. `apps/starprint-api/API.md` is the
  reference: every route, field and status, in tables. Anything the API
  gains goes in both, the reference for what it is and the skill for
  how an agent should use it.
- `.macroscope/`: Macroscope's ignore list and the check-run agent that
  carries the rules below. Macroscope reads nothing else. Codex reads
  the Code Review Rules at the end of this file.

## Conventions

- Every change is a pull request, squashed onto `main`. No direct
  pushes, force pushes or merge commits. Merging needs CI green and
  every review thread resolved, Codex's and Macroscope's included.
- Before opening one, run `cargo test --all-targets`, clippy with
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
- The preview draws thermal dots at 150 % of the pitch at normal
  resolution and 200 % in double, measured against printed step wedges.
- `Pacing::STAR_ETHERNET` (1400 bytes every 20 ms) is the rate at which
  the IFBD-HE07/08 cards never dropped a job.
- Two-colour mode prints a darker black than single colour at +3 on
  plain paper, measured on the TSP700II, which is what density +4 in a
  profile selects. In that mode the density command does nothing to
  black (a sweep from -3 to +3 printed three identical bars) and the
  speed command is ignored, so +4 sections send neither command.
  Double-resolution sections use +3 instead. The GUI disables picture
  double resolution at +4 for both preview and printing. The text colour
  and the raster colour both survive `ESC @`, so every job sets them.
- A QR module is drawn 7 dots by 3 on the SP700, at double density: 169
  dots to the inch across against 72 down, which comes within 1 % of
  square. A 30 mm symbol printed that way scans off the ribbon, so the
  default size stands and the ratio is not to be adjusted by eye.
- A corner's arc is an ellipse in dots, so that it is a circle on
  paper. A `qr` job's `radius` sets it, 0 (the default) for square, 100 for
  a module and a half. Past about 1.7 modules an arc reaches the centre
  of the module in the corner, which is the point a scanner samples, so
  do not raise the top. No rounded symbol has been scanned yet on either
  head, so check one against a phone before trusting it.
- The finder patterns round with everything else. Keeping them square
  was tried and looks like an oversight in print. They are the largest
  blocks on the symbol, so a corner shows there most.

Behaviour that shapes how jobs are written:

- `PrintMode::DoubleResolution` outlives `ESC @` and the job that set
  it, so select the mode at the start of a job and switch back at the
  end. It doubles rows only (16 rows/mm at 8 dots/mm across), darkens
  rasters, and prints the printer's fonts at half height.
- The TSP800II buffers about 2,560 raster rows and pauses to print them,
  leaving a faint line across a long image (~320 mm at normal
  resolution, ~160 mm in double).
- `ESC GS a` places a bit image on the SP700, not only text: left, centre
  and right all move an `ESC ^` graphic as they move a line of type.
  Pictures and QR codes both rely on it.
- Thermal raster mode ignores `ESC GS a`. QR printing adds blank dots
  before the symbol to position it within the selected paper's print
  width. The caption still uses text alignment.

## Diagnostics

- `cargo run --example thermal_test_pattern -- <host> 80 slow density=3`
  prints a head-check page.
- `cargo run -p starprint-workflows --example qr -- <host> thermal|impact
  "<data>" [size=MM] [radius=PCT] [ecc=l|m|q|h] [align=left|center|right]
  [caption=TEXT]` prints a QR code, for checking a module size or a
  radius against a phone.
- `cargo run --example thermal_image --features image -- <host> photo.jpg
  slow density=3 [double] [rotate] [gamma=F] [equalize=0|1]` for photos.
- `cargo run --example impact_image --features image -- <host> photo.jpg
  double [rotate]` for the SP700.
- Four probes print the same content under different settings for
  comparison: `thermal_black_probe` (normal against double resolution),
  `thermal_color_probe` (single colour against two-colour black and red,
  since the red drive is the hotter one), `thermal_text_probe` (double
  resolution on the printer's fonts) and `thermal_speed_probe` (times
  each `ESC RS r` value).

## Code Review Rules

For Codex. CI runs tests, clippy and fmt, so style is not a finding.
Only changed lines count.

### Measured values

Both tone curves, the Ethernet pacing, the preview dot sizes, the QR
module size and radius ceiling, and what density +4 sends were tuned
on the printers. Changing one without a hardware test described in the
PR is a finding.

### Command bytes

Every command `document.rs` emits cites its manual. A new or changed
byte sequence without one is a finding.

### Golden fixtures

`crates/starprint/tests/fixtures/` is byte-identical output from the
Python reference. Changing a fixture, or the impact pipeline, dithering
or `ESC ^` serialisation behind one, is a finding unless the PR says
the reference was rerun.

### Job behaviour

Jobs live in `crates/starprint-workflows`; front ends call them. A
printing rule in a front end is a finding. So is a preview drawn from
anything but the bitmap that prints, a QR symbol sent as `ESC GS y`
instead of a bitmap, and a job that selects
`PrintMode::DoubleResolution` without switching back at the end.

### The API skill and reference

An endpoint, job field, profile field or schedule field added to
`apps/starprint-api` without the matching change in both
`apps/starprint-api/API.md` and `skills/starprint-print/` is a finding.
