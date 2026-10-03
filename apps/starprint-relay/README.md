# starprint-relay

The public server starprint faxes travel through. It hands out line
records and holds sealed faxes until their line polls for them, since
printers sit behind home routers and cannot be dialled directly.

```
cargo run --release -p starprint-relay -- --name LONRELAY01
```

Put it on a public host behind a TLS proxy, with `--behind-proxy`, and
servers add it by its `https://` address.

```
starprint-relay --name <NAME> [--listen <addr>] [--db <file>] [--behind-proxy]
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--name <NAME>` | Required | What servers call it: capitals, digits and dashes, like `LONRELAY01` |
| `--listen <addr>` | `0.0.0.0:9120` | Address and port to bind |
| `--db <file>` | `relay.db` | SQLite file for lines and faxes, created if missing |
| `--behind-proxy` | Off | Take each client's address from the last `X-Forwarded-For` entry, the one the TLS proxy adds. Only behind a proxy: without one, anyone could claim any address |

It stores up to 10,000 line records. It holds sealed faxes for 30 days:
up to 100 of them or 64 MiB for one line, and 8 GiB for all lines
together. It takes no token: records check themselves, faxes are signed
by lines it holds, and only a line's identity can poll for its faxes or
confirm them, with a request that names this relay.

Every fax arrives and prints, so the relay limits each client address
instead: a burst of 10 faxes, then 30 an hour, and a burst of 5 new
lines, then 10 an hour, since each is a new identity to fax from.
Polls, confirms and a line republishing itself are not limited. Past a
limit the relay answers `429` with `Retry-After`. A poll answers with
everything held for its line, so limit connections and slow readers at
the proxy.

Everything is in the `--db` file, so a restart loses nothing. A fax
is deleted as soon as its line confirms it printed, and a fax nobody
collects is deleted after 30 days, so the file holds only what is still
on its way. Each fax taken and delivered is logged, with its sender,
recipient and size:

```
2026-09-27 14:32:05 fax 6241486a from *star15089lwj8gn70m8gepwymguzl5qkangla to *star1en2su3z68yscvky0n3j3l2qwny4dkq7s, 4.5 KB
2026-09-27 14:32:11 fax 6241486a from *star15089lwj8gn70m8gepwymguzl5qkangla delivered to *star1en2su3z68yscvky0n3j3l2qwny4dkq7s
```

| Method | Path | What it does |
| --- | --- | --- |
| `GET` | `/v1/relay` | `{ "name", "version" }` |
| `GET` | `/v1/lines/{number}` | The line record for a number, written without its star: `star1en2su3z68yscvky0n3j3l2qwny4dkq7s` |
| `PUT` | `/v1/lines/{number}` | Store a line record. `400` if it does not check, `409` if a newer one is held, `507` if the relay holds as many lines as it can |
| `POST` | `/v1/lines/{number}/faxes` | Hold a sealed fax for the line. `202` with its `id`, `507` if too much is waiting |
| `POST` | `/v1/lines/{number}/poll` | The faxes held for the line, for a request signed by it |
| `POST` | `/v1/lines/{number}/confirm` | Delete faxes that printed, by `id`, for a request signed by the line |

Refusals are `application/problem+json`, like the API's. A record or
request is limited to 1 MiB, a fax to 32 MiB.
