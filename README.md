# Cutout

Transparent email alias proxy — like [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/ananthb/cutout)

**[Documentation](https://ananthb.github.io/cutout/)**

## How it works

1. You own one or more domains with **Cloudflare Email Routing** (inbound) and **Email Service** (outbound) enabled
2. Write routing rules in the `/manage` UI — glob patterns on the local and domain parts, with a Forward or Drop action
3. A Forward rule can fan out to **one or more destinations** of mixed kinds: email, Telegram chat, Discord channel
4. For email destinations: CF's native `EmailMessage.forward()` preserves original `From`/`To`/DKIM/attachments; a `Reply-To: reply+<uuid>@yourdomain.com` header is overlaid so replies route through the proxy
5. For Telegram/Discord destinations: the bot posts the content to the chat and stores a reply context in KV. Replies in the chat (Telegram's native reply, Discord's "Reply" button → modal) route back to the original sender via email
6. Destination availability is driven by which secrets are set — telegram/discord only appear in the UI when their bot tokens are configured

## Features

- **Rule-based routing**: ordered glob patterns on local and domain parts, evaluated top-to-bottom, first match wins; a `*@*` Drop catch-all is always pinned at the end
- **Multi-destination forwards**: one rule can forward to any mix of email / Telegram / Discord targets
- **CF-native email forwarding** via `EmailMessage.forward()`: original `From`, `To`, DKIM, and attachments preserved exactly
- **Chat forwarding**: email → Telegram chat and email → Discord channel, with replies routed back to the sender as email
- **Reverse-alias reply routing** across all channels: a reply in any channel reaches the original sender without exposing your real address
- **Rule validation** on save: empty patterns, empty destinations, duplicate patterns, unreachable rules (via glob subsumption), and destinations using unconfigured channels are all caught
- **Rule tester**: `/manage/test` evaluates the rule set against a supplied recipient and shows which rule fires
- **Multi-domain**: one worker serves any number of zones; rules use the domain glob to differentiate
- **Management UI**: HTMX rule editor at `/manage`, protected by Cloudflare Access

## Architecture

- [Cloudflare Workers](https://workers.cloudflare.com/) (Rust compiled to WebAssembly)
- [Cloudflare Email Routing](https://developers.cloudflare.com/email-routing/) — inbound MX + catch-all → worker, plus `EmailMessage.forward()` for the Forward path (destinations are verified via Email Routing's built-in Destination Addresses flow)
- [Cloudflare Email Service](https://developers.cloudflare.com/email-service/) — used only on the reverse-alias reply path, where the recipient (the original sender of the forwarded mail) isn't in Destination Addresses
- [Cloudflare KV](https://developers.cloudflare.com/kv/) — rule list, reverse-alias mappings, and per-message bot reply contexts
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/) — protects `/manage`
- [botrelay-rs](https://github.com/ananthb/botrelay-rs) — shared crate providing the Telegram + Discord bot clients and reply-context primitives

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
