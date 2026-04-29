<h1>
  <img src="doc/cutout-mark.svg" width="28" height="28" align="absmiddle" alt="">
  Cutout
</h1>

Transparent email alias proxy: similar to [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

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
6. Destinations depend on your secrets: Telegram and Discord only appear in the UI when their bot tokens are configured

## Getting Started

See the **[Deploy guide](https://ananthb.github.io/cutout/deploy.html)** for step-by-step instructions on forking and deploying your own instance of Cutout to Cloudflare.

CI/CD is handled by **Cloudflare Builds** (Workers CI), which builds and deploys directly from this repo without needing GitHub Actions or Nix.

To wire up your fork:

1. In the Cloudflare dashboard, create a Worker named (e.g.) `cutout` and connect this repo under **Settings → Builds**.
   - **Build command:** `cargo install -q worker-build && npx wrangler d1 migrations apply cutout-db --remote`
   - **Deploy command:** leave default (`npx wrangler deploy`)
2. Bind a D1 database (`DB`), KV namespace (`KV`), R2 bucket (`EMAILS`), Queues (`RETRIES` producer + `cutout-retries`/`cutout-retries-dlq` consumers), Email send-binding (`EMAIL`), and Analytics Engine dataset (`EVENTS`) under **Settings → Bindings**. Names must match the `binding` values in [`wrangler.toml`](wrangler.toml).
3. Set runtime variables and secrets under **Settings → Variables and Secrets** — see the list at the bottom of [`wrangler.toml`](wrangler.toml).
4. Push to `main`. Cloudflare Builds runs the build command (which applies any pending D1 migrations), then `wrangler deploy` — which picks up `[build] command = "worker-build --release"` from `wrangler.toml` to compile the Rust crate to WASM.

## Features

- **Rule-based routing**: ordered glob patterns on local and domain parts, evaluated top-to-bottom, first match wins; a `*@*` Drop catch-all is always pinned at the end
- **Dual-mode email forwarding**: choose between high-fidelity **Native mode** (preserves PGP/attachments) and reliable **Proxy mode** (ensures `Reply-To` routing works via your custom domain)
- **Permanent Archival**: an `X-Original-From` header is injected into all forwarded mail so you never lose the sender's identity even if KV mappings expire
- **Multi-destination forwards**: one rule can forward to any mix of email / Telegram / Discord targets
- **Bot-relay replies**: reply directly from Telegram or Discord chat; Cutout routes it back to the original sender via email
- **HTMX Management UI**: clean, responsive web interface for managing rules, gated by Cloudflare Access
- **Safety first**: automatic loop detection; kinds whose secrets aren't set are hidden from the UI and rejected by the validator
- **Live tester**: each rule's inspector includes an interactive tester that evaluates a recipient address against this rule plus the full ruleset, highlighting which rule actually fires
- **Multi-domain**: one worker serves any number of zones; rules use the domain glob to differentiate

## Architecture

- [Cloudflare Workers](https://workers.cloudflare.com/): Rust compiled to WebAssembly
- [Cloudflare Email Routing](https://developers.cloudflare.com/email-routing/): inbound MX + catch-all -> worker, plus `EmailMessage.forward()` for the Native Forward path
- [Cloudflare Email Service](https://developers.cloudflare.com/email-service/): used for reverse-alias replies, Proxy mode forwarding, and fanning out beyond the first destination
- [Cloudflare KV](https://developers.cloudflare.com/kv/): rule list, reverse-alias mappings, and per-message bot reply contexts
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/): protects `/manage`
- [botrelay-rs](https://github.com/ananthb/botrelay-rs): shared crate providing the Telegram + Discord bot clients and reply-context primitives

## Development

```bash
nix develop        # enter dev shell with all tools (Nix-only; CI does not use Nix)
cargo test         # run tests
cargo clippy       # lint
wrangler dev       # local dev server
nix flake check    # run all CI checks (tests, clippy, fmt, pre-commit)
```

Nix is for local convenience only — Cloudflare Builds installs the same toolchain via rustup, which reads the channel from [`rust-toolchain.toml`](rust-toolchain.toml).

## License

[AGPL-3.0](LICENSE)
