<h1>
  <img src="doc/cutout-mark.svg" width="28" height="28" align="absmiddle" alt="">
  Cutout
</h1>

Transparent email alias proxy: similar to [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/ananthb/cutout)

**[Documentation](https://ananthb.github.io/cutout/)**

## How it works

1. You own one or more domains with **Cloudflare Email Routing** (inbound + outbound). Add **Cloudflare Email Service** if you want to send to addresses you have not verified.
2. Someone emails `anything@yourdomain.com`
3. Cutout worker is triggered, matches the recipient against your rules, and forwards the email to your real address
4. For email destinations:
    - **Native mode** uses CF's native `EmailMessage.forward()`; original bytes (PGP, attachments) pass through untouched.
    - **Proxy mode** reconstructs the email via `send_email` to ensure `Reply-To` works reliably when replying via the same domain (strips signatures/attachments).
    Both modes inject an `X-Original-From` header for permanent archival in your inbox.
    Which backend sends them is set by `OUTBOUND_EMAIL_BACKEND`: `routing` (default) is free but only delivers to destinations verified in the dashboard; `service` is paid and delivers to anyone. See [Sending](#sending).
5. For Telegram/Discord destinations: the bot posts the content to the chat and stores a reply context in KV. Replies in the chat (Telegram's native reply, Discord's "Reply" button → modal) route back to the original sender via email
6. Destinations depend on your secrets: Telegram and Discord only appear in the UI when their bot tokens are configured

## Sending

Cloudflare offers two outbound backends and Cutout picks one with the `OUTBOUND_EMAIL_BACKEND` variable:

| Value | Backend | Cost | Recipients |
|---|---|---|---|
| `routing` (default) | Email Routing | free | only addresses verified under Email Routing → Destination Addresses |
| `service` | Email Service | paid (Workers Paid plan, per message) | anyone |

`routing` is the default and needs no setup beyond verifying destinations. `service` needs the sending domain onboarded to Email Service; it sends every email destination through `send_email` (so attachments and signed MIME are dropped), while `routing` sends the first non-proxy email destination with native `EmailMessage.forward()` and the rest through `send_email`.

The variable isn't in `wrangler.toml`, so the code default is `routing` and a fork is free out of the box. Setting it in the dashboard alone won't stick: `wrangler deploy` rewrites the Worker's variables from `wrangler.toml`, so the next build drops it. To keep your own instance on `service` without changing the shared file, set the Worker's **Deploy command** under **Settings → Builds** to:

```
npm run deploy -- --var OUTBOUND_EMAIL_BACKEND:service
```

`--var` sets the variable on every deploy (it overrides `wrangler.toml`), so it survives redeploys and only applies to that Worker.

## Getting Started

The fastest path is the **Deploy to Cloudflare** button above. It forks the repo into your account, creates the Worker, provisions the storage and queues declared in [`wrangler.toml`](wrangler.toml) (D1, KV, R2, Queues), and connects the fork to **Cloudflare Builds**. You still add the Email send-binding, the secrets, the Cloudflare Access app, and Email Routing in the dashboard.

The **[Deploy guide](https://ananthb.github.io/cutout/deploy.html)** walks through all of it step by step. To wire up a fork by hand instead:

1. In the Cloudflare dashboard, create a Worker named (e.g.) `cutout` and connect this repo under **Settings → Builds**.
   - **Build command:** leave default (CF Builds runs `npm install` from `package.json`)
   - **Deploy command:** `npm run deploy` (defined in `package.json` — installs `worker-build`, applies pending D1 migrations, then runs `wrangler deploy`)
2. Bind a D1 database (`DB`), KV namespace (`KV`), R2 bucket (`EMAILS`), Queues (`RETRIES` producer + `cutout-retries`/`cutout-retries-dlq` consumers), Email send-binding (`EMAIL`), and Analytics Engine dataset (`EVENTS`) under **Settings → Bindings**. Names must match the `binding` values in [`wrangler.toml`](wrangler.toml). Replace the `database_id` there with the one `wrangler d1 create` prints.
3. Set runtime secrets under **Settings → Variables and Secrets** — see the list at the bottom of [`wrangler.toml`](wrangler.toml). `CF_ACCESS_TEAM` and `CF_ACCESS_AUD` are required. Add `CF_API_TOKEN` (with *Account Analytics: Read*) and `CF_ACCOUNT_ID` to turn on the dashboard stats panels (the two are a pair — with either missing the panels stay hidden), and `PUBLIC_BASE_URL` for inbox links posted to Telegram/Discord. Use the **Secret** type for anything that must persist: a plain-text **Variable** set only in the dashboard is dropped on the next `wrangler deploy`. `OUTBOUND_EMAIL_BACKEND` is optional and passed on the Deploy command (see [Sending](#sending)).
4. Push to `main`. Cloudflare Builds runs the build command (which applies any pending D1 migrations), then `wrangler deploy` — which picks up `[build] command = "worker-build --release"` from `wrangler.toml` to compile the Rust crate to WASM.

## Features

- **Rule-based routing**: ordered glob patterns on local and domain parts, evaluated top-to-bottom, first match wins; a `*@*` Drop catch-all is always pinned at the end
- **Dual-mode email forwarding**: choose between high-fidelity **Native mode** (preserves PGP/attachments) and reliable **Proxy mode** (ensures `Reply-To` routing works via your custom domain)
- **Free or paid sending**: default to free Email Routing (verified destinations only), or switch to paid Email Service to send to any address
- **Permanent Archival**: an `X-Original-From` header is injected into all forwarded mail so you never lose the sender's identity even if KV mappings expire
- **Multi-destination forwards**: one rule can forward to any mix of email / Telegram / Discord targets
- **Bot-relay replies**: reply directly from Telegram or Discord chat; Cutout routes it back to the original sender via email
- **Elm management app**: a single-page frontend for managing rules, gated by Cloudflare Access; the pipeline animates which rule a test address would fire
- **Reader inbox**: `/inbox` lists the stored mail the signed-in Access identity may read; a rule's email destinations are implicit readers and the operator can grant more per rule
- **JSON API**: the frontend talks to `/api/*` and nothing else
- **Safety first**: automatic loop detection; kinds whose secrets aren't set are hidden from the UI and rejected by the validator
- **Live tester**: the workbench tester evaluates a recipient address against the full ruleset on every keystroke, highlighting which rule actually fires
- **Multi-domain**: one worker serves any number of zones; rules use the domain glob to differentiate

## Architecture

- [Cloudflare Workers](https://workers.cloudflare.com/): Rust compiled to WebAssembly
- [Elm](https://elm-lang.org/): the management SPA, compiled to `public/app.js`
- [Cloudflare Email Routing](https://developers.cloudflare.com/email-routing/): inbound MX + catch-all -> worker, plus `EmailMessage.forward()` for the Native Forward path
- [Cloudflare Email Service](https://developers.cloudflare.com/email-service/): outbound `send_email`; free for verified destinations, or paid to send to anyone (`OUTBOUND_EMAIL_BACKEND=service`)
- [Cloudflare KV](https://developers.cloudflare.com/kv/): rule list, reverse-alias mappings, and per-message bot reply contexts
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/): protects `/manage`, `/inbox` and `/api`
- [botrelay-rs](https://github.com/ananthb/botrelay-rs): shared crate providing the Telegram + Discord bot clients and reply-context primitives

## Development

```bash
nix develop              # enter dev shell with all tools (Nix-only; CI does not use Nix)
npm ci                   # install elm + wrangler
npm run build:frontend   # compile the Elm app to public/app.js
cargo test               # run tests
cargo clippy             # lint
wrangler dev             # local dev server
nix flake check          # run all CI checks (tests, clippy, fmt, elm-format, pre-commit)
```

Copy [`.dev.vars.example`](.dev.vars.example) to `.dev.vars` before `wrangler dev`: it holds the Cloudflare Access variables and the optional secrets.

`build.rs` hashes `public/app.js` and the CSS modules into the shell's integrity
attributes, so the frontend has to be built before the crate will compile —
`npm run build:frontend` (or `scripts/build-worker.sh`, which does it for you).

Nix is for local convenience only — Cloudflare Builds installs the same toolchain via rustup, which reads the channel from [`rust-toolchain.toml`](rust-toolchain.toml).

## License

[AGPL-3.0](LICENSE)
