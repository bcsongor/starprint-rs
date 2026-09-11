# starprint-api

An HTTP API for printing to Star receipt printers and managing their
profiles and schedules. Version 1, under `/v1`. JSON in, JSON out;
errors are RFC 9457 problem details.

The server listens on `http://127.0.0.1:9110` by default. The desktop
app runs the same server on the same data directory while it is open,
and its API button can put it on a LAN address instead. One server per
directory: the app and the command line cannot run at once. For how to
use the API from an agent, see the
[skill](../../skills/starprint-print/SKILL.md).

| Method | Path | What it does |
| --- | --- | --- |
| `GET` | [`/v1/printers`](#get-v1printers) | List the profiles and the server's version |
| `PUT` | [`/v1/printers/{name}`](#put-v1printersname) | Create or replace a profile |
| `DELETE` | [`/v1/printers/{name}`](#delete-v1printersname) | Remove a profile |
| `GET` | [`/v1/printers/{name}/status`](#get-v1printersnamestatus) | Whether the printer answers |
| `POST` | [`/v1/printers/{name}/jobs`](#post-v1printersnamejobs) | Print a job |
| `POST` | [`/v1/printers/{name}/preview`](#post-v1printersnamepreview) | Show a job without printing it |
| `POST` | [`/v1/printers/{name}/raw`](#post-v1printersnameraw) | Send bytes as they are |
| `GET` | [`/v1/schedules`](#get-v1schedules) | List the schedules |
| `POST` | [`/v1/schedules`](#post-v1schedules) | Create a schedule |
| `PUT` | [`/v1/schedules/{id}`](#put-v1schedulesid) | Replace a schedule |
| `DELETE` | [`/v1/schedules/{id}`](#delete-v1schedulesid) | Remove a schedule |

## Authentication

Every request carries the server's token:

```
Authorization: Bearer <token>
```

Anything else is a `401`. The token is printed when the server starts
and kept in the [data directory](#data-directory). There is no endpoint
that returns it.

A browser may call the API from any origin: `OPTIONS` is answered with
the CORS headers, and every response allows any origin to read it. The
token is what admits a request.

## Running the server

```
starprint-api [--data <dir>] [--listen <addr>] [--token <value>]
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--data <dir>` | `starprint` under the platform's configuration directory | Where profiles, schedules and the token are kept |
| `--listen <addr>` | `127.0.0.1:9110` | Address and port to bind |
| `--token <value>` | The one on file, or a new one | Sets the token and writes it to the data directory |

## Printers

A profile names a printer and holds its settings. Jobs are addressed to
a profile by name.

### Profile

| Field | Type | Notes |
| --- | --- | --- |
| `name` | string | The path segment a printer is addressed by. No `/`. Response only |
| `host` | string | Required |
| `port` | integer | Required. Usually 9100 |
| `kind` | `thermal` or `impact` | Required |
| `cut` | boolean | Required. Whether jobs end with a cut |
| `paper` | `80` or `112` | Thermal only, required there. Roll width in millimetres, matching the print width memory switch |
| `density` | integer, -3 to 4 | Thermal only, required there. The printer's own -3 to +3 scale; 4 is two-colour mode. Cheap paper pinholes in solid black below +2 at the default speed |
| `speed` | `high`, `medium` or `slow` | Thermal only, required there |

Thermal fields on an impact profile are refused, as is any field not
listed.

### `GET /v1/printers`

```json
{
  "version": "1.0.0",
  "printers": [
    { "name": "tsp800ii", "host": "192.168.1.180", "port": 9100, "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
    { "name": "sp743", "host": "192.168.1.141", "port": 9100, "kind": "impact", "cut": true }
  ]
}
```

`version` is the server's, so a client can tell an older server from
one it expects. Profiles come in the order the data directory lists
them; a new one goes last.

### `PUT /v1/printers/{name}`

Body: a [profile](#profile) without `name`.

```json
{ "host": "192.168.1.180", "port": 9100, "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" }
```

Creates the profile with `201`, or replaces the one of that name in
place with `200`. Either way the response is the profile with its
`name`.

### `DELETE /v1/printers/{name}`

`204` with no body. Schedules for that printer are kept and do not run
until a printer of that name is back. `404` if there is no such profile.

### `GET /v1/printers/{name}/status`

```json
{ "online": true }
```

`online` means something accepted a TCP connection at the printer's
address. Each resolved address gets a 400 ms connection timeout, after
DNS lookup and waiting for the printer's turn in the queue.

## Jobs

### `POST /v1/printers/{name}/jobs`

Body: a job request, as `application/json`, or as
`multipart/form-data` with the request in a `job` part and, for a
picture, the image in an `image` part.

| Field | Type | Notes |
| --- | --- | --- |
| `job` | [job](#job-kinds) | Required |
| `cut` | boolean | Default: the profile's |
| `density` | integer, -3 to 4 | Thermal only. Default: the profile's |
| `speed` | `high`, `medium` or `slow` | Thermal only. Default: the profile's |

`host`, `port`, `kind` and `paper` cannot be set on a job; the
request is refused. `density` or `speed` on an impact printer is
refused rather than ignored.

```json
{ "job": { "kind": "task-card", "text": "Renew passport", "priority": true, "due": "2026-09-15" }, "density": 4 }
```

Response:

```json
{ "bytesSent": 284 }
```

A completed socket write, and nothing more: the server cannot tell
whether the printer accepted or printed the job.

Jobs to the same host and port are written one at a time, whichever
profile they came through.

### `POST /v1/printers/{name}/preview`

Body: as [`jobs`](#post-v1printersnamejobs). Nothing is sent to the
printer, so it answers while the printer is off.

Response: the job as it will look on that printer, tagged by `kind`
like the job. A text job is its layout: `columns` and the `lines` the
printer's own font will set. A job printed as dots carries `image`, a
`data:` URL of a PNG drawn from the bitmap that prints, dot for dot,
with thermal dots widened as the head widens them.

```json
{ "kind": "task-card", "columns": 48, "lines": ["Renew passport"], "priority": null, "reference": null, "due": "                                     15 SEP 2026" }
```

```json
{ "kind": "qr", "columns": 48, "caption": [], "align": "center", "modules": 33, "widthMm": 53.625, "heightMm": 53.625, "image": "data:image/png;base64,iVBORw0K..." }
```

`note` has its layout and an `image`, `picture` an `image` alone, and
`test-page` its `sections`, each a `title` and what a fault looks like
in `check`. Empty text previews as an empty layout rather than a `400`.

### Job kinds

`kind` picks the job; the other fields are that job's own. Every field
but the ones marked required has a default.

**`task-card`**

| Field | Type | Notes |
| --- | --- | --- |
| `text` | string | Required |
| `priority` | boolean | `false` |
| `reference` | string | A short identifier, such as an issue key |
| `due` | string | An ISO date, printed as `28 AUG 2026`, or free text printed as is |

**`text`**

| Field | Type | Notes |
| --- | --- | --- |
| `text` | string | Required |
| `bold`, `wide`, `tall`, `accent` | boolean | `false`. `accent` is red on impact, inverse on thermal |

**`note`**

| Field | Type | Notes |
| --- | --- | --- |
| `rule` | `blank`, `dots`, `lines` or `squares` | `lines` |
| `rows` | integer | `10` |
| `pitch` | integer | `7`. Millimetres between rules |

**`qr`**

| Field | Type | Notes |
| --- | --- | --- |
| `data` | string | Required. What the code carries |
| `caption` | string | Printed above the symbol |
| `errorCorrection` | `l`, `m`, `q` or `h` | `m` |
| `size` | integer, 10 to 80 | `30`. Width in millimetres. Out of range is clamped, not refused |
| `radius` | integer, 0 to 100 | `0`. Corner rounding; 0 is square. Above 100 counts as 100 |
| `align` | `left`, `center` or `right` | `center` |

**`test-page`**

| Field | Type | Notes |
| --- | --- | --- |
| `doubleResolution` | boolean | `false`. Thermal only |

**`picture`**

Must be sent as a form with an `image` part: PNG, JPEG, WebP or BMP.

| Field | Type | Notes |
| --- | --- | --- |
| `double` | boolean | `false`. Double density on impact, double resolution on thermal. Ignored at density 4, which has one resolution |
| `dither` | `floyd-steinberg`, `atkinson`, `threshold` or `bayer` | `floyd-steinberg` |
| `threshold` | integer, 0 to 255 | `128`. Ignored by `bayer` |
| `brightness` | number | `1.0` |
| `contrast` | number | `1.0` |

### `POST /v1/printers/{name}/raw`

Body: `application/octet-stream`, written to the printer as it is. No
validation and none of the profile's settings apply. For programs that
build their own command streams.

Response: `{ "bytesSent": 12 }`.

## Schedules

A schedule is a job request with the printer it goes to and a cron
expression. The server runs it on its own local clock, through the same
queue as any other job. There is no history, no catching up on a missed
minute and no retry.

Stopping the server cancels scheduled jobs still waiting to print.
A write already in progress finishes before another job can use that
printer. The server checks a schedule's job before accepting it.

### Schedule

| Field | Type | Notes |
| --- | --- | --- |
| `id` | string | Assigned by the server. Response only |
| `printer` | string | Required. A profile's name, which must exist when the schedule is written |
| `cron` | string | Required. Five fields, in the server's local time |
| `enabled` | boolean | `true` |
| `due` | `run-day` or `next-day` | Task cards only, refused on any other job: the date the card gets when it prints. Left out, the card prints undated |
| `job` | [job](#job-kinds) | Required. Not a picture. Prints with the profile's settings: a schedule carries no `cut`, `density` or `speed` |

### `GET /v1/schedules`

```json
[
  { "id": "f91d77b5-57be-48a1-b694-99dd5204a695", "printer": "tsp800ii", "cron": "0 9 * * 1-5", "enabled": true, "due": "run-day", "job": { "kind": "task-card", "text": "Standup", "priority": false } }
]
```

### `POST /v1/schedules`

Body: a [schedule](#schedule) without `id`.

```json
{ "printer": "tsp800ii", "cron": "0 9 * * 1-5", "due": "run-day", "job": { "kind": "task-card", "text": "Standup" } }
```

`201` with the schedule, `id` included.

### `PUT /v1/schedules/{id}`

Body: as `POST`. `200` with the schedule, or `404` if there is no
schedule with that id.

### `DELETE /v1/schedules/{id}`

`204` with no body, or `404`.

## Errors

`application/problem+json`, per RFC 9457. `detail` is a sentence
saying what went wrong.

```json
{ "type": "about:blank", "title": "Not Found", "status": 404, "detail": "No printer named `desk`." }
```

| Status | When |
| --- | --- |
| `400` | Bad JSON, an invalid field, an unknown request, profile or schedule field, or a job that could not be built. Unknown fields inside `job` are ignored |
| `401` | No token, or not this server's |
| `404` | No such printer, schedule or endpoint |
| `405` | Not a method the endpoint takes |
| `413` | Body too big: 1 MiB for JSON, 16 MiB for a form or raw bytes |
| `415` | Wrong `Content-Type` |
| `500` | A change could not be written to the data directory; nothing changed |
| `502` | The printer could not be reached or written to. It may have received none, some or all of the job |

## Data directory

| File | Holds |
| --- | --- |
| `printers.json` | The profiles, as an object keyed by name, each in the shape [`PUT`](#put-v1printersname) takes |
| `schedules.json` | The schedules, as an object keyed by id, each in the shape [`POST`](#post-v1schedules) takes |
| `token` | The token |
| `lock` | Held while a server has the directory open. A second server on the same directory fails to start |

Each file is read once at startup and rewritten whole whenever the API
changes it, so a hand edit means a restart and does not survive the
next change made over the API. A file the server cannot read stops it.
The server trusts what it finds there, the token included, so the
directory should be the server user's own.

A profile file for one printer of each kind:

```json
{
  "tsp800ii": { "host": "192.168.1.180", "port": 9100, "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
  "sp743": { "host": "192.168.1.141", "port": 9100, "kind": "impact", "cut": true }
}
```
