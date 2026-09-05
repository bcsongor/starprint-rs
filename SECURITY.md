# Security

Report a vulnerability through
[GitHub's private reporting](https://github.com/bcsongor/starprint-rs/security/advisories/new)
rather than a public issue. Expect a reply within a week.

The HTTP API binds loopback and refuses requests from web pages, but it
has no authentication. Anyone who can reach the address it listens on
can print. `--listen` on a network address is a choice to make knowingly.
