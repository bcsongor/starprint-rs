# starprint-relay

The public server starprint faxes travel through. It hands out line
records and holds sealed faxes until their line polls for them, since
printers sit behind home routers and cannot be dialled directly.

```
cargo run --release -p starprint-relay -- --name LONRELAY01
```

Put it on a public host behind a TLS proxy, and servers add it by its
`https://` address.

```
starprint-relay --name <NAME> [--listen <addr>] [--db <file>]
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--name <NAME>` | Required | What servers call it: capitals, digits and dashes, like `LONRELAY01` |
| `--listen <addr>` | `0.0.0.0:9120` | Address and port to bind |
| `--db <file>` | `relay.db` | SQLite file for lines and faxes, created if missing |

It stores line records and holds sealed faxes, up to 100 or 64 MiB a
line and for 30 days. It takes no token: records check themselves,
faxes are signed by lines it holds, and only a line's identity can poll
for its faxes or confirm them.

Everything is in the `--db` file, so a restart loses nothing. Nothing
is deleted either: a confirmed fax is marked delivered and a fax
waiting past 30 days is no longer handed out, but both stay in the file,
sealed. Each fax taken and delivered is logged, with its sender,
recipient and size:

```
2026-09-27 14:32:05 fax 6241486a from *3878 498397 to *2629 618032, 4.5 KB
2026-09-27 14:32:11 fax 6241486a from *3878 498397 delivered to *2629 618032
```

| Method | Path | What it does |
| --- | --- | --- |
| `GET` | `/v1/relay` | `{ "name", "version" }` |
| `GET` | `/v1/lines/{digits}` | The line record for a number, as its ten digits |
| `PUT` | `/v1/lines/{digits}` | Store a line record. `409` if the number belongs to another identity here or a newer record is held |
| `POST` | `/v1/lines/{digits}/faxes` | Hold a sealed fax for the line. `202` with its `id` |
| `POST` | `/v1/lines/{digits}/poll` | The faxes held for the line, for a request signed by it |
| `POST` | `/v1/lines/{digits}/confirm` | Mark faxes delivered by `id`, for a request signed by the line |

Refusals are `application/problem+json`, like the API's. A record or
request is limited to 1 MiB, a fax to 32 MiB.
