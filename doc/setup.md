# Setup Guide (Manual)

Deploy your own Cutout instance manually using the Wrangler CLI.

## Prerequisites
*   A [Cloudflare account](https://dash.cloudflare.com) with at least one domain.
*   [Nix](https://nixos.org/download) installed (or Rust + wrangler + `worker-build` manually).

## 1. Clone and enter the dev shell
```bash
git clone https://github.com/ananthb/cutout.git
cd cutout
nix develop
```

## 2. Create the KV namespace
```bash
wrangler kv namespace create KV
```
Put the returned `id` into the `[[kv_namespaces]]` block of `wrangler.toml`.

## 3. Create the D1 database
```bash
wrangler d1 create cutout-db
```
Put the returned `database_id` into the `[[d1_databases]]` block of `wrangler.toml`.

## 4. Apply migrations
```bash
wrangler d1 migrations apply cutout-db --local
```

## 5. Enable Cloudflare Email Routing (inbound)
In the dashboard for every email domain you want Cutout to handle:
1.  Open **Email → Email Routing** and click **Enable**.
2.  Under **Routes**, edit the **catch-all address**, set the action to **Send to a Worker**, and pick the `cutout` worker.

## 6. Onboard each domain to Cloudflare Email Service (outbound)
1.  Dashboard → **Email → Email Sending → Onboard Domain**
2.  Accept the DNS records (MX / SPF / DKIM / DMARC) on the `cf-bounce.<yourdomain>` subdomain.

## 7. Verify your destination addresses
1.  Dashboard → **Email → Email Routing → Destination addresses**
2.  Click **Add destination address** and enter the address (e.g. your real Gmail).
3.  Click the confirmation link sent to your email.

## 8. Configure Cloudflare Access
Create an Access application whose policy covers `cutout.<yourdomain>/manage/*`.
Set `CF_ACCESS_TEAM` and `CF_ACCESS_AUD` in `wrangler.toml`.

## 9. Deploy
```bash
wrangler deploy
```

## 10. Write rules
Visit `https://<your-cutout-host>/manage`. Add Forward rules using glob patterns.
For email destinations, use the **Proxy via rewrite mode** toggle to ensure Reply-To works when replying via your custom domain.
