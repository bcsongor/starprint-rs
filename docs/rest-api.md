# REST API

`apps/starprint-api`.

## What it is for

A small HTTP server exposing the desktop app's jobs to other local
programs, mostly AI agents. See `agent-integration.md` for the MCP server
planned alongside it.

The API is synchronous. A successful response means that every byte was
written to the printer's TCP connection, not that the printer finished the
job.

## Running it

```
starprint-api [--config <path>] [--listen <addr>]
```

`--config` defaults to `printers.toml` in the working directory and
`--listen` to `127.0.0.1:9110`. `--listen` binds whatever it is given;
see the trust model below before reaching past the loopback interface.

## Shared job builders

`crates/starprint` remains the printer command library. The reusable job
builders live in `crates/starprint-workflows`:

- `Job`, `PrinterKind`, `Paper`, `Speed` and the document-building settings
- task cards, text, notes, QR codes, pictures and test pages
- layout, validation and golden byte tests for those jobs

The shared crate does not depend on a GUI file path. Picture processing takes
image data supplied by its caller. The Tauri app keeps file selection and its
source cache; the API supplies the image from a multipart request.

The Tauri commands and `apps/starprint-api` are thin adapters over the shared
crate.

## Printer profiles

The server reads named printer profiles from a TOML file at startup;
`apps/starprint-api/printers.toml.example` is a commented copy of this:

```toml
[printers.tsp800ii]
host = "192.168.1.180"
port = 9100
kind = "thermal"
paper = 80
cut = true
density = 3
speed = "slow"

[printers.sp743]
host = "192.168.1.141"
port = 9100
kind = "impact"
cut = true
```

`host`, `port`, `kind` and, for thermal printers, `paper` cannot be overridden
by a request. `cut` and, for thermal printers, `density` and `speed` are
defaults that a job request may override.

All applicable fields are required. Unknown fields, invalid values and
thermal-only fields on an impact profile prevent the server from starting.
The file is read once; changing it requires a restart. There is no API for
managing profiles.

## Conventions

The printer is the resource, so its profile name is in the path. Everything
else about a request is in the body; there are no query parameters.

In JSON, enum values are kebab-case (`task-card`, `slow`) and field names are
camelCase (`bytesSent`).

## Printers

```http
GET /v1/printers
```

```json
[
  { "name": "tsp800ii", "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
  { "name": "sp743", "kind": "impact", "cut": true }
]
```

A read-only view of the profile file, so a client can pick a printer and see
which options apply to it.

## Jobs

```http
POST /v1/printers/tsp800ii/jobs
Content-Type: application/json

{
  "density": 3,
  "job": { "kind": "task-card", "text": "Renew passport", "priority": true, "due": "2026-09-15" }
}
```

| Field | Required | Notes |
| --- | --- | --- |
| `job` | yes | The shared tagged `Job` enum; `kind` selects `task-card`, `text`, `note`, `qr`, `picture` or `test-page` |
| `cut` | no | Overrides the profile |
| `density` | no | Thermal override, from `-3` to `3` |
| `speed` | no | Thermal override: `high`, `medium` or `slow` |

Omitted options come from the profile. Supplying `density` or `speed` for an
impact profile is an error.

Within a job, only the content is required: the task card's and text job's
`text`, and a picture's image. Everything else takes the default the desktop
app starts from, so `{ "kind": "test-page" }` and `{ "kind": "picture" }` are
whole jobs.

| Kind | Required | Defaults |
| --- | --- | --- |
| `task-card` | `text` | `priority` false, no `due` |
| `text` | `text` | `bold`, `wide`, `tall`, `accent` all false |
| `note` | nothing | `rule` `lines`, `rows` 10, `pitch` 7 |
| `qr` | `data` | no `caption`, `errorCorrection` `m`, `size` 30, `align` `center` |
| `test-page` | nothing | `doubleResolution` false |
| `picture` | the `image` part | `double` false, `dither` `floyd-steinberg`, `threshold` 128, `brightness` and `contrast` 1.0 |

A `qr` job's `size` is the symbol's width in millimetres, quiet zone
excluded, clamped to 10–80 and reduced further if the symbol will not fit
the paper. `errorCorrection` is `l`, `m`, `q` or `h`, in rising order of
how much of the symbol can be lost and still scan. `align` is `left`,
`center` or `right`, and takes the caption with it. The symbol is encoded
by the server rather than by the printer, so an impact printer prints one
as readily as a thermal one; data too long to encode is a `400`.

The same endpoint accepts `multipart/form-data`: a `job` part containing the
whole JSON object above, and an `image` part containing the image bytes in
PNG, JPEG, WebP or BMP. Any job kind may be sent this way; a `picture` job
must be, and sent as plain JSON it is a `400`. A client cannot ask the server
to read an image path. No other part is accepted, and an `image` sent with a
job that prints none is ignored.

## Raw jobs

```http
POST /v1/printers/tsp800ii/raw
Content-Type: application/octet-stream

<bytes>
```

Raw bytes go through `TcpTransport`, including `Pacing::STAR_ETHERNET`. The
bytes already contain all printer commands, so raw jobs accept no cut, density
or speed options. Hex and other text encodings are not supported.

## Response

Both print endpoints return:

```json
{ "bytesSent": 284 }
```

This reports a completed socket write only. Without automatic status back,
the server cannot confirm that the printer accepted or printed the job.

## Errors

Errors use RFC 9457 problem details with
`Content-Type: application/problem+json`.

| Status | When |
| --- | --- |
| `400` | Malformed JSON, invalid options, options the printer kind does not accept, or a job that cannot be built |
| `404` | Unknown printer or endpoint |
| `405` | A method the endpoint does not take |
| `413` | The request or image is too large: 1 MiB for a JSON body, 16 MiB for a form or raw bytes |
| `415` | Unsupported content type |
| `502` | DNS, connection, timeout or write failure while contacting the printer |

A `502` does not prove that nothing printed: the printer may have received
none, some or all of the bytes. Clients must not retry it automatically.

## Deployment

The server binds `127.0.0.1` by default and does not enable CORS. It must not
be exposed on the local network without revisiting authentication and the
trust model. Requests can reach only the destinations in the profile file.

## Left out on purpose

No preview endpoint, API key, job IDs, asynchronous status, idempotency keys,
printer discovery or profile-management API. The server builds a job, writes
it and returns.
