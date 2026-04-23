# Cutout

Transparent email alias proxy — like [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/ananthb/cutout)

**[Documentation](https://ananthb.github.io/cutout/)**

## How it works

1. You own one or more domains with **Cloudflare Email Routing** (inbound) and **Email Service** (outbound) enabled
2. Write routing rules in the `/manage` UI — glob patterns on the local and domain parts, with a Forward or Drop action
3. Forwarded destinations confirm ownership via a one-time verification link before mail starts flowing
4. Mail matching a Forward rule is rewritten through a `reply+<uuid>@yourdomain.com` reverse-alias and sent on to the verified destination
5. Replies to that reverse-alias are routed back to the original sender — your real address is never exposed

## Features

- **Rule-based routing**: ordered glob patterns on local and domain parts, evaluated top-to-bottom, first match wins; a `*@*` Drop catch-all is always pinned at the end
- **Forward or drop** per rule, with multiple destinations per Forward
- **Reverse-alias replies**: `reply+<uuid>@yourdomain.com` mappings in KV (30-day TTL)
- **Destination verification**: new destinations receive a one-time link from `verify@<your-cutout-host>`; unverified destinations are silently skipped at forward time
- **Multi-domain**: one worker serves any number of zones; rules use the domain glob to differentiate
- **Management UI**: HTMX-based rule editor at `/manage`, protected by Cloudflare Access
- **Zero storage**: everything lives in KV — rules, reverse aliases, verified destinations, pending tokens

## Architecture

- [Cloudflare Workers](https://workers.cloudflare.com/) (Rust compiled to WebAssembly)
- [Cloudflare Email Routing](https://developers.cloudflare.com/email-routing/) — inbound MX + catch-all → worker
- [Cloudflare Email Service](https://developers.cloudflare.com/email-service/) — outbound via the `send_email` binding, delivers to any recipient
- [Cloudflare KV](https://developers.cloudflare.com/kv/) — rules, reverse aliases, verified destinations, pending tokens
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/) — protects `/manage` only; `/verify/<token>` stays public

## Deploy

Click the button above, or deploy manually:

```bash
# Enter dev shell (installs Rust, wrangler, worker-build, wasm-pack, Node.js)
nix develop

# Create KV namespace
wrangler kv namespace create KV
# Copy the namespace ID into wrangler.toml

# Dashboard steps, per email domain:
#   Email Routing: enable + catch-all → worker
#   Email Sending: onboard domain (adds cf-bounce.<domain> records)
#
# Dashboard step, once:
#   Access: application policy covering cutout.<yourdomain>/manage/*
#   then set CF_ACCESS_TEAM and CF_ACCESS_AUD in wrangler.toml

# Deploy
wrangler deploy
```

See the [setup guide](https://ananthb.github.io/cutout/setup.html) for full instructions.

## Development

```bash
nix develop        # enter dev shell with all tools
cargo test         # run tests
cargo clippy       # lint
wrangler dev       # local dev server
nix flake check    # run all CI checks (tests, clippy, fmt, pre-commit)
```

## License

[AGPL-3.0](LICENSE)
