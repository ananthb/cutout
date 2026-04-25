# Cutout

Transparent email alias proxy — like [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/ananthb/cutout)

**[Documentation](https://ananthb.github.io/cutout/)**

## How it works

1. You own one or more domains with **Cloudflare Email Routing** (inbound) and **Email Service** (outbound)
2. Someone emails `anything@yourdomain.com`
3. Cutout worker is triggered, matches the recipient against your rules, and forwards the email to your real address
4. For email destinations:
    - **Native mode** uses CF's native `EmailMessage.forward()`; original bytes (PGP, attachments) pass through untouched.
    - **Proxy mode** reconstructs the email via `send_email` to ensure `Reply-To` works reliably when replying via the same domain (strips signatures/attachments).
    Both modes inject an `X-Original-From` header for permanent archival in your inbox.
5. For Telegram/Discord destinations: the bot posts the content to the chat and stores a reply context in KV. Replies in the chat (Telegram's native reply, Discord's "Reply" button → modal) route back to the original sender via email
6. Destination availability is driven by which secrets are set — telegram/discord only appear in the UI when their bot tokens are configured

## Getting Started

See the **[Deployment Guide](doc/deploy.md)** for instructions on how to fork and deploy your own instance of Cutout using GitHub Actions.

For a detailed manual setup, see the **[Setup Guide](doc/setup.md)**.

## Features

- **Rule-based routing**: ordered glob patterns on local and domain parts, evaluated top-to-bottom, first match wins; a `*@*` Drop catch-all is always pinned at the end
- **Dual-mode email forwarding**: choose between high-fidelity **Native mode** (preserves PGP/attachments) and reliable **Proxy mode** (ensures `Reply-To` routing works via your custom domain)
- **Permanent Archival**: an `X-Original-From` header is injected into all forwarded mail so you never lose the sender's identity even if KV mappings expire
- **Multi-destination forwards**: one rule can forward to any mix of email / Telegram / Discord targets
- **Bot-relay replies**: reply directly from Telegram or Discord chat; Cutout routes it back to the original sender via email
- **HTMX Management UI**: clean, responsive web interface for managing rules, gated by Cloudflare Access
- **Safety first**: automatic loop detection; kinds whose secrets aren't set are hidden from the UI and rejected by the validator
- **Rule tester**: `/manage/test` evaluates the rule set against a supplied recipient and shows which rule fires
- **Multi-domain**: one worker serves any number of zones; rules use the domain glob to differentiate

## Architecture

- [Cloudflare Workers](https://workers.cloudflare.com/) — Rust compiled to WebAssembly
- [Cloudflare Email Routing](https://developers.cloudflare.com/email-routing/) — inbound MX + catch-all → worker, plus `EmailMessage.forward()` for the Native Forward path
- [Cloudflare Email Service](https://developers.cloudflare.com/email-service/) — used for reverse-alias replies, Proxy mode forwarding, and fanning out beyond the first destination
- [Cloudflare KV](https://developers.cloudflare.com/kv/) — rule list, reverse-alias mappings, and per-message bot reply contexts
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/) — protects `/manage`
- [botrelay-rs](https://github.com/ananthb/botrelay-rs) — shared crate providing the Telegram + Discord bot clients and reply-context primitives

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
