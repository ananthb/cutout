<h1>
  <img src="doc/cutout-mark.svg" width="28" height="28" align="absmiddle" alt="">
  Cutout
</h1>

Transparent email alias proxy: similar to [SimpleLogin](https://simplelogin.io) or [addy.io](https://addy.io), built entirely on Cloudflare Workers. No servers, no containers, no monthly VM bills.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/ananthb/cutout)

**[Documentation](https://ananthb.github.io/cutout/)**

## How it works

- Point your domain's Email Routing catch-all at Cutout.
- Match inbound addresses against ordered glob rules (`local@domain`) to **forward** (to email, Telegram, or Discord), **store** (to the built-in `/inbox`), or **drop**.
- Reply from your email client or directly inside Telegram/Discord; Cutout routes the reply back through your alias so your real address never leaks.

## Sending

Email destinations default to **Proxy mode** (`send_email`), which rewrites `Reply-To` to a reverse alias so replies go through Cutout, or opt into **Direct mode** (`EmailMessage.forward()`), which passes the original message and `From` untouched. Verified addresses under **Email Routing → Destination Addresses** work out of the box for free; onboarding your domain to **Email Service** lets Proxy mode send to unverified recipients too.

## Getting Started

1. Click **Deploy to Cloudflare** (or fork and connect **Cloudflare Builds** with deploy command `npm run deploy`).
2. Add the `EMAIL` send-binding and your `CF_ACCESS_TEAM` / `CF_ACCESS_AUD` secrets (plus optional Telegram/Discord/Analytics secrets — see [`wrangler.toml`](wrangler.toml)).
3. Route your domain's catch-all to the Worker under **Email Routing** and protect `/manage` and `/inbox` with **Cloudflare Access**.

Full step-by-step instructions are in the **[Deploy guide](https://ananthb.github.io/cutout/deploy.html)**.

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

Copy [`.dev.vars.example`](.dev.vars.example) to `.dev.vars` before `wrangler dev`. Build the frontend with `npm run build:frontend` (or `scripts/build-worker.sh`) before compiling the Rust crate, as `build.rs` hashes `public/app.js` and the CSS modules.

## License

[AGPL-3.0](LICENSE)
