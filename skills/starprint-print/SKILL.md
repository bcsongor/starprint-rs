---
name: starprint-print
description: Print task cards, text, note slips, QR codes, pictures and test pages on a Star receipt printer through the local starprint-api server, and manage its printer profiles and schedules. Use when the user asks to print something, to print on a schedule, or mentions a receipt printer, task card, note slip, QR code or test page.
---

# Printing with starprint

`starprint-api` prints to Star receipt printers on the local network.
It listens on `http://127.0.0.1:9110` unless it was started with
`--listen`. The Starprint desktop app serves the same API while its
API button is pressed, at loopback on port 9110 unless another of the
machine's addresses or another port was chosen under that button, with
printers named as its profiles are. Against the app's server, profiles
and schedules made over the API are kept in memory only and are gone
when that server next starts, which happens when the button is
released, a profile is edited or the address changes. Say so before
creating a schedule there; the command line's server keeps them.

Every request carries a bearer token, or gets a `401`. The server
prints its token on the line that says where it is listening. The token
lives in the server's data directory and stays the same across
restarts, unless the server is started with `--token`. In the desktop
app, the API button shows the token with a copy button while it serves.
Ask the user for the token if you do not have it; there is no way to
fetch it.

Printing is physical and cannot be undone. It spends paper, and ribbon
on the impact printer. Ask before printing anything the user did not
ask for, and never retry a failed print in a loop.

On Windows PowerShell, `curl` is an alias for `Invoke-WebRequest` and
takes none of these flags. Use `curl.exe` and backticks to continue
lines.

## Start here

Every job goes to a printer by name, so find out what exists before
printing:

```bash
curl -s -H 'Authorization: Bearer <token>' http://127.0.0.1:9110/v1/printers
```

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
one it expects. If this fails to connect, nothing is serving the API.
Neither the server nor the desktop app's button is on. Say so rather
than guessing a printer name. If more than one printer is listed and
the user did not say which, ask.

To find out whether a printer is switched on and reachable without
printing anything:

```bash
curl -s -H 'Authorization: Bearer <token>' http://127.0.0.1:9110/v1/printers/tsp800ii/status
```

```json
{ "online": true }
```

This waits for any job in progress on that printer to finish first, so
it never lands in the middle of one.

## Printing

Post to `/v1/printers/<name>/jobs`. Only the content is required;
everything else has a default.

Jobs to the same host and port wait for each other, even through different
profiles. The desktop app shares this queue with manual and Linear jobs.
Separate processes and different hostnames for the same printer have
separate queues.

Task card, the common case:

```bash
curl -s -X POST http://127.0.0.1:9110/v1/printers/tsp800ii/jobs \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"job":{"kind":"task-card","text":"Renew passport","priority":true,"reference":"OPC-123","due":"2026-09-15"}}'
```

`priority` defaults to false. `reference` is a short identifier such as
an issue key, printed between the banner and the date; it and `due` may
be left out.

To see a job before spending paper on it, post the same body to
`/v1/printers/<name>/preview`. The reply is the job as it will print:
for text jobs the `lines` the printer will set and how many `columns`
it has, and for anything drawn as dots an `image`, a `data:` URL of a
PNG you can show the user or open yourself. Nothing reaches the
printer, so it works while the printer is off.

Text, for anything freeform:

```json
{ "job": { "kind": "text", "text": "Back in 10 minutes", "bold": true, "wide": true } }
```

`bold`, `wide`, `tall` and `accent` all default to false.

Note slip, for handwriting on:

```json
{ "job": { "kind": "note", "rule": "lines", "rows": 10, "pitch": 7 } }
```

`rule` is `blank`, `dots`, `lines` or `squares`. `rows` is how many
rows to write in and `pitch` the millimetres between rules.

QR code, for handing a link or a password to a phone:

```json
{ "job": { "kind": "qr", "data": "https://example.com/r/42", "caption": "Order 42" } }
```

`data` is whatever the code should carry, so a Wi-Fi network is
`WIFI:T:WPA;S:<ssid>;P:<password>;;` and a phone number is `tel:+44…`.
`caption` prints above the symbol and may be left out, though a code with
nothing written on it is unidentifiable an hour later. `size` is the
symbol's width in millimetres (10 to 80, 30 by default),
`errorCorrection` is `l`, `m`, `q` or `h`, and `align` is `left`,
`center` or `right`. `radius` rounds the corners of the symbol. 0, the
default, keeps them square. 100 rounds each corner as far as its shape allows,
so a lone module becomes a circle early on and the finder patterns keep
going. Both printers can print one. A `size` or `radius` out of range
is clamped, not refused.

Test page:

```json
{ "job": { "kind": "test-page" } }
```

`doubleResolution` repeats its grey ramp in double resolution on a
thermal printer, and is off by default.

A picture must go as a form, with the image in its own part. Sent as
plain JSON it is a `400`:

```bash
curl -s -X POST http://127.0.0.1:9110/v1/printers/tsp800ii/jobs \
  -H 'Authorization: Bearer <token>' \
  -F 'job={"job":{"kind":"picture","double":true}}' \
  -F 'image=@photo.jpg'
```

PNG, JPEG, WebP and BMP decode. The server will not read a path, so the
file has to be uploaded. Picture settings are `double`, `dither`
(`floyd-steinberg`, `atkinson`, `threshold` or `bayer`), `threshold`
(ignored by `bayer`), `brightness` and `contrast`. The defaults are what
the desktop app starts from and are usually right.

## Per-job overrides

Alongside `job`, a request may carry `cut`, and on thermal printers
`density` (-3 to 4) and `speed` (`high`, `medium` or `slow`). Omitted,
they come from the profile. Sending `density` or `speed` to an impact
printer is an error rather than being ignored.

`density` 4 selects the printer's two-colour mode, which prints a
darker black than +3 on plain paper. The mode has one speed, so
`speed` does nothing at 4. A picture with `double` prints in double
resolution at +3 instead. A test page's double-resolution section
also uses +3; its other sections use two-colour mode.

Leave these alone unless the user asks. The profile holds settings that
were tuned against the actual hardware.

You cannot set `host`, `port`, `kind` or `paper` on a job. Those are
the profile's alone and the request will be rejected. To point at a
different printer, change the profile.

## Printing on a schedule

A schedule is a job request as above, with the printer it goes to and
a five-field cron expression. The server runs it on its own local
clock, so `0 9 * * 1-5` is nine in the morning where the server is,
whether or not anyone is logged in.

```bash
curl -s -X POST http://127.0.0.1:9110/v1/schedules \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"printer":"tsp800ii","cron":"0 9 * * 1-5","due":"run-day","job":{"kind":"task-card","text":"Standup"}}'
```

```json
{ "id": "f91d77b5-57be-48a1-b694-99dd5204a695", "printer": "tsp800ii", "cron": "0 9 * * 1-5", "enabled": true, "due": "run-day", "job": { "kind": "task-card", "text": "Standup", "priority": false } }
```

`enabled` defaults to true; set it to false to keep a schedule without
running it. `due` applies to task cards only and dates the card when it
prints: `run-day` for that day, `next-day` for the day after. Left out,
the card prints with no date, whatever `job` says. A schedule prints
with the profile's settings and takes no `cut`, `density` or `speed`
of its own. A picture cannot be scheduled, since the server would have
no image to print it from.

`GET /v1/schedules` lists them. `PUT /v1/schedules/<id>` replaces one
with the same body as `POST`, without `id`, and `DELETE
/v1/schedules/<id>` removes it. A schedule whose printer has been
deleted is kept and does not run until a printer of that name is back.
Stopping the server cancels scheduled jobs still waiting to print.
Writes already in progress finish. The server checks the job before
accepting a schedule, so correct a rejected job before trying again.

A schedule prints without asking anyone, every time it fires. Confirm
the expression with the user before creating one, and read back what
exists before changing or deleting anything.

## Managing printers

A profile is created or replaced by name with `PUT`:

```bash
curl -s -X PUT http://127.0.0.1:9110/v1/printers/tsp800ii \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"host":"192.168.1.180","port":9100,"kind":"thermal","paper":80,"cut":true,"density":3,"speed":"slow"}'
```

A new name answers `201`, an existing one `200`, and the profile keeps
its place in the list. `host`, `port`, `kind` and `cut` are required.
`paper` (80 or 112), `density` and `speed` are required on a thermal
printer and refused on an impact one. `DELETE /v1/printers/<name>`
removes a profile and leaves its schedules in place.

Profiles hold settings tuned against the printers. Do not change one
unless the user asks, and do not guess a host.

## The data directory

The server keeps its profiles, schedules and token in one directory:
`starprint` under the platform's configuration directory, or wherever
`--data` points. `printers.json` holds the profiles by name and
`schedules.json` the schedules by id, each entry in the shape the API
takes it; `token` holds the token. Each file is read once at startup
and rewritten whole after a change over the API, so a hand edit means
a restart and is best done while the server is stopped. One server per
directory: a second one on the same directory fails to start.

## When it goes wrong

Errors are RFC 9457 problem details, and `detail` says what happened in
a sentence worth reading back to the user.

| Status | Meaning |
| --- | --- |
| `400` | Bad JSON, an invalid option, a profile or schedule that will not do, or a job that could not be built. A misspelt field inside `job` is ignored rather than refused, so check names against this skill |
| `401` | No token, or not this server's; ask the user for it |
| `404` | No printer by that name or no schedule by that id; list them again |
| `413` | Too big: 1 MiB of JSON, 16 MiB for a form |
| `415` | Wrong content type |
| `500` | A change could not be written to the data directory; nothing changed |
| `502` | The printer could not be reached |

A success returns `{"bytesSent": 284}`. That is a completed socket
write and nothing else. The server cannot tell whether the printer
accepted the job or printed it, so do not report to the user that
something printed. Report that it was sent.

A `502` does not mean nothing printed. The printer may have taken some
or all of the bytes before the connection failed. Do not retry it
automatically; tell the user and let them look at the paper.

## Not for you

`/v1/printers/<name>/raw` takes arbitrary bytes straight to the printer
with no validation. It is for programs that build their own command
streams. Do not use it. If a job seems to need it, say what you were
trying to do instead.
