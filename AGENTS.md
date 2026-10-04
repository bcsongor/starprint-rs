# Working on starprint

For contributors; the README is for users.

## Layout

A Cargo workspace. `crates/starprint` is the library and default member,
`crates/starprint-workflows` holds the jobs, and two front ends print them.
`crates/starprint-fax` is the fax protocol, which the API and
`apps/starprint-relay` speak.

- `crates/starprint/src/document.rs`: `Builder<P>` and the command
  encoding. Every command cites the manual it comes from.
- `crates/starprint/tests/fixtures/`: golden output from the Python
  reference in the sibling `starprint` repo (`generate.py`). The impact
  pipeline, dithering and `ESC ^` serialisation must stay byte-identical.
- `crates/starprint-workflows/`: the printer profile, the tagged `Job`
  and the six jobs: task cards, text, note slips, QR codes, pictures and
  test pages. It reads no files and opens no sockets, so a picture job
  is handed its image. Job behaviour belongs here, not in a front end,
  or the two drift. The job types derive `JsonSchema`, and the API
  hands that schema to agents, so a field's doc comment is written for
  them too. `preview` draws what a job will look like: a
  layout for the printer's own fonts, and a PNG from the bitmap that
  prints, with thermal dots widened to the measured size. Both front
  ends show previews from it and nothing else. `FaxHeader` is the bar a
  received fax prints under, on the same builder as the job; it is not
  a job, so nothing previews it. QR symbols are encoded here by `qrcodegen` and
  printed as dots on both heads. The SP700 has no QR command, and one
  bitmap gives one path, a known version, a preview that draws what
  prints and rounded modules, none of which `ESC GS y` would.
- `apps/starprint-gui/`: Vite + React + shadcn/ui over a Rust side in
  `src-tauri/`. Run with `bun tauri dev` from that directory. The app
  is a web client of `starprint-api`, which it runs in-process on the
  same data directory the command line uses, so the two cannot run at
  once; the app says so and offers a retry. The Rust side is a
  launcher: `start_server` opens the directory on its first call and
  binds the server at the address it is given, replacing one already
  running, and `list_addresses` names the adapters. `window.rs` makes
  the title bar match the dark toolbar: by DWM on Windows, and on
  Linux by asking GTK for its dark theme and styling the bar GTK
  draws, which a window manager that draws its own ignores. The Linux
  release is a `.deb` built on `ubuntu-22.04`, the oldest runner, so
  it starts on that glibc and anything newer. Everything else
  is `fetch` from `src/lib/api.ts`: profiles, jobs, previews,
  schedules and the connection dot's status. Profiles, schedules and
  statuses are read again every ten seconds, so a change another
  program makes over the API shows up. The server answers the
  webview's CORS preflight for that. A picture is a `File` from a file
  input, sent as the multipart form. The settings store keeps only the
  Linear key, the LAN switch and address, another server's URL and
  token and whether it is in use, and the active profile's name;
  profiles and schedules are the server's. The server listens at
  loopback on port 9110; the API button's switch also puts it on a LAN
  address, which is a restart there, and the button shows the URL and
  token, the MCP address with a copy menu that gives an agent's client
  its whole config, `mcpServers` key and all, as an HTTP server or for
  Claude Desktop through `mcp-remote`, and on the LAN a QR code of the phone page's
  URL with the token in the fragment, drawn by `qrcode.react`. A LAN address that
  will not bind falls back to loopback, and turning the switch on
  before an address was chosen takes the first adapter's. The same button switches the app to another machine's
  server: `App` hands `Workspace` whichever `Server` the settings name
  and nothing else changes, since every request goes through the one
  client in `src/lib/api.ts`. `use-server` starts over when the client
  changes and reports whether the server answered and its `version`
  from the printer list. The button's dot shows the first, and is left
  out on loopback, where the app is only talking to itself. The button
  compares the version with the app's own, since the two are released
  together. The embedded server keeps running either way, and the app
  still needs it to start. The schedules drawer says whose clock the
  cron expressions run on.
  Every preview is the server's `Preview` drawn as it comes: a layout
  in the printer's font, an `<img>` of the `data:` PNG. A preview that
  kept a rule of its own drifted once, and squared finder patterns went
  unnoticed until they came off the printer. Schedules are the API's,
  shown in a drawer off the header and written through it, one `PUT`
  each when the drawer moves them all to another printer; the dialog
  checks a cron expression with npm `croner`, the same dialect as the
  server's crate. Pictures cannot be scheduled. Auto-print is frontend
  only: a personal API key in the settings store, a `fetch` against
  Linear's GraphQL endpoint every 10 seconds, and a task card for each
  newly assigned open issue, posted to the server like any other job.
  Fax is the server's too. The phone button opens a drawer to activate
  the line, choose its printer and add relays, and to load the number
  into the QR form as a card to print. **Fax** beside **Print** sends
  the job in the form to a number or a name in the fax book, which is
  kept in the same popover with the recent numbers it lacks.
  Every number on screen is drawn by
  `FaxNumber`: chunks of uneven length, each its own colour with a gap
  before it, cut by a hash of the whole number
  (`src/lib/fax-number.ts`), so a one-character change moves every
  chunk. It is for the eye only. The phone page shows numbers too and
  has no build step, so it carries a copy of the rule; change one and
  the other must follow.
- `apps/starprint-api/`: an HTTP server over the same jobs, for other
  local programs, and the home of everything that has to run
  unattended. A library with a thin command line on top, so the desktop
  app can run the same server. One concern per module; anything new
  goes in whichever of `body`, `job`, `printers`, `config`, `data`,
  `schedule`, `faxing`, `problem`, `app`, `mcp` or `server` owns it. `data` is the data directory:
  `printers.json`, `schedules.json`, `token` and `fax.json`, each read
  once at startup and written whole under its lock after every change.
  The directory is locked while open, so two servers cannot share one,
  the desktop app's included. `app` answers `OPTIONS` with the CORS
  headers for any origin, since the app's webview is a browser; the
  token is what admits a request.
  `schedule` holds the schedule shape and the loop that prints each one
  on the local clock through the queue; it runs for as long as the
  server does. Shutdown cancels queued scheduled jobs before draining
  HTTP requests; blocking writes already in progress finish under
  their queue guards. Profiles are addressed by name, `PUT` creates or
  replaces, and jobs still cannot carry `host` or `port`. `/preview`
  takes a job's body and answers with the workflows crate's `Preview`,
  so a client needs no job code of its own. The connection
  probe lives here too, behind `/status` and exported as `reachable`,
  so it takes the printer's turn like a job. `src/phone.html` is the
  phone page, served at `/` without the token: one file, no framework
  and no build step, so the command line serves it too. It prints and
  faxes task cards and pictures through the routes, and keeps the token
  in the URL fragment, since an iOS home-screen shortcut has storage of
  its own. It draws the server's `Preview` as it comes, like the desktop
  app. Once the line can send it keeps the fax book as well: contacts,
  recent numbers and the line's own number, read again every ten
  seconds. The number's QR code, for another phone's camera, is the
  `Preview` of a `qr` job, so the page encodes nothing. Activating the
  line, relays and the fax printer stay on the desktop. Printing is
  what the page is for, so faxing sits below the preview and is drawn
  lighter. A LAN address is plain HTTP, where a browser gives a page
  no clipboard API and no camera. Copying falls back to a selection,
  and a number comes off a printed card through the phone's own camera
  app and a paste.
  `printers::PrintQueue`, exported at the crate root, serialises
  connections by host and port.
  `faxing` is the server's side of fax: `fax.json` in the data
  directory, activating the line, sending, what a fax carries, and the loop that polls relays
  and prints through the queue like the scheduler. The loop also
  replaces the line's fax key every week, which is the line's forward
  secrecy. An old key goes at the first weekly replacement once 31 days
  have passed since its own. That is the relay's 30-day hold and a day,
  so the two must stay in step. A fax's id is remembered for as long as
  a key that opens it, so a fax handed over twice prints once. Neither
  a relay nor a sender is trusted. A relay's answer is read up to a
  limit, a request for the line's faxes names the relay it is for, and
  a fax may take only so much paper. A received fax
  prints under the workflows crate's `FaxHeader`, through
  `Printer::fax`, so the API still builds no bytes of its own. Its tests send
  through a real `starprint-relay`, a dev-dependency only, so no relay
  code ships in the server or the app.
  The GUI keeps one queue across restarts of its server. Its guard must
  live inside the blocking task so cancellation cannot release a write
  still in progress.
  `mcp` serves agents MCP over streamable HTTP at `/mcp`, mounted by
  `app` under the same token, on the official Rust SDK. It keeps no
  session and answers each call as plain JSON, so a client survives a
  restart and no stream holds a shutdown up. A tool calls what its
  route calls, which is why the work behind a route lives in `job`,
  `printers`, `schedule` and `faxing` and not in a handler, and it
  fails with the route's problem details as its error. Arguments that
  do not fit a tool's schema are the exception. The SDK refuses those
  in plain text before the tool runs.
  The SDK runs a tool on a task of its own and only signals when the
  client goes, where axum drops a route's handler with its connection,
  so `call_tool` drops the tool then. Without that a job waiting for
  its printer prints for nobody, after a shutdown too. The tools are
  the printer list and status, print, preview, the schedules, sending a
  fax and the fax book. Raw bytes, profiles and setting up the fax line
  have none, since a description is a weaker fence than a missing
  tool. A picture travels as base64 in the call, which
  only suits a small one, so the `image` argument points an agent at
  the form route for a file. An agent knows nothing but the tools'
  descriptions and their arguments' schemas, and both are doc comments:
  a tool's on its function, an argument's on the field that reads it,
  in `mcp`, in `schedule` or on the job types in the workflows crate,
  which derive `JsonSchema`. So what a comment there says is what an
  agent does. A preview's bitmap comes back as an image beside the
  layout, not as a `data:` URL in the text.
- `crates/starprint-fax/`: the fax protocol, which does no I/O. A
  number is the Bech32m address of an identity key (`number`), a line
  record is signed by Ed25519 and ML-DSA-65 (`line`), a fax is sealed to
  a random X-Wing key the line replaces now and then, under
  ChaCha20-Poly1305 (`fax`), and `relay` holds the
  requests a line makes of a relay. A fax's contents are bytes here; the
  API decides they are a job.
- `apps/starprint-relay/`: the public server faxes travel through, a
  router in `lib.rs`, its SQLite file in `store.rs` and a command line
  in `main.rs`. It keeps a fax only until its line confirms it
  printed, or 30 days. It holds only so many lines, and so many bytes
  for one line and for all of them. Since every fax prints, `limit`
  holds each client address to so many faxes, new lines and polls an
  hour, the address coming from `X-Forwarded-For` only with
  `--behind-proxy`. It logs who faxed whom, never what, and knows
  nothing of printers.
- `manuals/README.md`: links to Star's specifications, which are Star's
  copyright and not kept here. Check bytes there, not from memory.
- `apps/starprint-api/API.md`: the reference, with every route, field,
  status and tool in tables. Anything the API gains goes there. There
  is no skill to keep beside it: an agent connects to `/mcp` and reads
  the tools as they are.

## Conventions

- Every change is a pull request, squashed onto `main`. No direct
  pushes, force pushes or merge commits. Merging needs CI green and
  every review thread resolved, Codex's included.
- Before opening one, run `cargo test --all-targets`, clippy with
  `-D warnings` and `cargo fmt --all --check`, both with and without
  `--features image`, then the same for
  `-p starprint-workflows -p starprint-fax -p starprint-api -p starprint-relay`.
  CI does the same. The
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
  speed command is ignored, so +4 sections send neither command. The
  test page's double-resolution section uses +3 instead. A picture's
  `double` is ignored at +4, in print and preview alike, by
  `Printer::picture` in the workflows crate: the darker black is what
  +4 is for. The text colour and the raster colour both survive
  `ESC @`, so every job sets them.
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
printing rule in a front end is a finding, the desktop app's frontend
included: it builds no bytes and draws no preview of its own. So is a
preview drawn from anything but the bitmap that prints, a QR symbol
sent as `ESC GS y` instead of a bitmap, and a job that selects
`PrintMode::DoubleResolution` without switching back at the end.

### The fax protocol

How a number is made from a key, the transcript
labels and the record, fax and request fields in `crates/starprint-fax`,
and the fields of the contents
`faxing` seals, are what every server and relay must agree on. Changing one
breaks every line made or fax sent before it, so it is a finding
unless the PR bumps the protocol version and says what happens to
existing lines.

### The API reference and its tools

An endpoint, job field, profile field, schedule field, fax field or
tool added to `apps/starprint-api` without the matching change in
`apps/starprint-api/API.md` is a finding. So is a tool for raw bytes,
for changing a profile or for setting up the fax line, and a tool that
does its route's work itself instead of calling what the route calls.
