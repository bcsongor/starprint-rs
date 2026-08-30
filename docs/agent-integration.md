# Integrating agents

Agents print by calling `starprint-api` over HTTP. Nothing needs
building for this. `docs/rest-api.md` is the reference, and
`.claude/skills/starprint-print/SKILL.md` is what an agent reads to
learn the server exists at all.

That second file is the whole integration. An agent that has never seen
this repo does not know there is a printer, which profiles are
configured, or that a picture goes up as a form. Telling it those three
things is a documentation job, and a skill file does it.

## Why not an MCP server

We planned one and dropped it. The reasoning is worth writing down
because it will come up again.

A model writes MCP tool arguments a token at a time, so image bytes
cannot travel through them. Spelling out a photograph as base64 costs
roughly a million tokens, slowly and unreliably. That leaves two
workarounds. A local file path confines printing to images already on
the machine running the server. A URL means putting an outbound HTTP
client inside a printer server and then defending it: scheme
allowlists, private address ranges, redirect re-validation, DNS
rebinding.

The API solved this before the question came up.
`multipart/form-data` carries the image in a part of the request body,
from wherever the agent got it, and there is nothing to fetch and
nothing to defend.

MCP would have added two things. Discovery, which the skill covers. And
clients that cannot make HTTP calls at all, which is the one thing that
would change this decision. If we ever want to print from a client with
no HTTP client and no shell, build the MCP server then, as a front end
linking `starprint-workflows` directly rather than wrapping the API.

## What an agent may reach

Two limits hold however an agent gets to the API.

The `raw` endpoint stays out of the skill. It takes arbitrary bytes
with no validation, and exists for programs that build their own
command streams. The five jobs are the surface an agent needs.

`host`, `port`, `kind` and `paper` cannot be overridden, which
`JobRequest` already enforces with `deny_unknown_fields`. The profile
file decides where a job goes and how wide the paper is, and a request
cannot argue.

## Still open

`picture::decode` calls `image::load_from_memory`, which applies
`Limits::default()`: a 512 MiB allocation cap, and no limit on width or
height. The paper is at most 832 dots across, so an image decoded far
larger than that is never useful to us. Setting explicit dimension
limits costs a few lines and covers the desktop app as well as the API.
