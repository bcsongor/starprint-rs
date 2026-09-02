---
name: starprint-print
description: Print task cards, text, note slips, QR codes, pictures and test pages on a Star receipt printer through the local starprint-api server. Use when the user asks to print something, or mentions a receipt printer, task card, note slip, QR code or test page.
---

# Printing with starprint

`starprint-api` prints to Star receipt printers on the local network.
It listens on `http://127.0.0.1:9110` unless it was started with
`--listen`.

Printing is physical and cannot be undone. It spends paper, and ribbon
on the impact printer. Ask before printing anything the user did not
ask for, and never retry a failed print in a loop.

On Windows PowerShell, `curl` is an alias for `Invoke-WebRequest` and
takes none of these flags. Use `curl.exe` and backticks to continue
lines.

## Start here

Every job goes to a printer named in the server's profile file, so find
out what exists before printing:

```bash
curl -s http://127.0.0.1:9110/v1/printers
```

```json
[
  { "name": "tsp800ii", "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
  { "name": "sp743", "kind": "impact", "cut": true }
]
```

If this fails to connect, the server is not running. Say so rather than
guessing a printer name. If more than one printer is listed and the
user did not say which, ask.

## Printing

Post to `/v1/printers/<name>/jobs`. Only the content is required;
everything else has a default.

Task card, the common case:

```bash
curl -s -X POST http://127.0.0.1:9110/v1/printers/tsp800ii/jobs \
  -H 'Content-Type: application/json' \
  -d '{"job":{"kind":"task-card","text":"Renew passport","priority":true,"reference":"OPC-123","due":"2026-09-15"}}'
```

`priority` defaults to false. `reference` is a short identifier such as
an issue key, printed between the banner and the date; it and `due` may
be left out.

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
`center` or `right`. `radius` rounds the corners of the symbol. 0 is
square. 100, the default, rounds each corner as far as its shape allows,
so a lone module becomes a circle early on and the finder patterns keep
going. Both printers can print one.

Test page, which takes nothing:

```json
{ "job": { "kind": "test-page" } }
```

A picture must go as a form, with the image in its own part. Sent as
plain JSON it is a `400`:

```bash
curl -s -X POST http://127.0.0.1:9110/v1/printers/tsp800ii/jobs \
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
`density` (-3 to 3) and `speed` (`high`, `medium` or `slow`). Omitted,
they come from the profile. Sending `density` or `speed` to an impact
printer is an error rather than being ignored.

Leave these alone unless the user asks. The profile holds settings that
were tuned against the actual hardware.

You cannot set `host`, `port`, `kind` or `paper`. Those are the profile
file's alone and the request will be rejected.

## When it goes wrong

Errors are RFC 9457 problem details, and `detail` says what happened in
a sentence worth reading back to the user.

| Status | Meaning |
| --- | --- |
| `400` | Bad JSON, an invalid option, or a job that could not be built |
| `404` | No printer by that name; list them again |
| `413` | Too big: 1 MiB of JSON, 16 MiB for a form |
| `415` | Wrong content type |
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
