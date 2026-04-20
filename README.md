# Cutout

Transparent email alias proxy — like [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/ananthb/cutout)

**[Documentation](https://ananthb.github.io/cutout/)**

## How it works

1. You own a domain with **Cloudflare Email Routing** enabled
2. Configure routing rules via the HTMX management UI at `/manage`
3. Inbound mail matching a rule is forwarded to your real address with headers rewritten
4. Replies are sent back through the alias — your real address is never exposed

## Features

- **Email aliasing**: generate random `reply+<uuid>@yourdomain.com` aliases that forward to your real inbox
- **Ordered routing rules**: glob patterns on local and domain parts, evaluated top-to-bottom, first match wins
- **Actions**: forward to one or more addresses, or drop
- **Reply routing**: replies go back through the alias so your real address stays hidden
- **Management UI**: HTMX-based rule editor at `/manage`, protected by Cloudflare Access
- **Zero storage**: all config in KV, reverse aliases expire after 30 days

## Architecture

- [Cloudflare Workers](https://workers.cloudflare.com/) (Rust compiled to WebAssembly)
- [Cloudflare Email Routing](https://developers.cloudflare.com/email-routing/) (inbound email handling)
- [Cloudflare send_email API](https://developers.cloudflare.com/email-routing/email-workers/send-email-workers/) (outbound replies through aliases)
- [Cloudflare KV](https://developers.cloudflare.com/kv/) (routing config and reverse alias mappings)
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/) (management UI authentication)

## Deploy

Click the button above, or deploy manually:

```bash
# Enter dev shell (installs Rust, wrangler, worker-build, wasm-pack, Node.js)
nix develop

# Create KV namespace
wrangler kv namespace create KV
# Copy the namespace ID into wrangler.toml or wrangler.production.toml

# Configure Email Routing
# In Cloudflare dashboard: set up a catch-all rule that sends all email to the worker

# Configure Cloudflare Access
# Create an Access application for /manage, then set CF_ACCESS_TEAM and CF_ACCESS_AUD

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
