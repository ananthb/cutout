# Deployment Guide (Forking)

Cutout is designed to be easily forked and deployed to your own Cloudflare account using GitHub Actions.

## Self-hosting / Forking

1.  **Fork this repository** to your own GitHub account.
2.  **Create a KV namespace** via `wrangler kv namespace create KV`.
3.  **Configure GitHub Secrets** in your forked repository (Settings → Secrets and variables → Actions):
    *   `CLOUDFLARE_API_TOKEN`: A Cloudflare API token with `Workers`, `KV`, and `Email Routing` permissions.
    *   `CLOUDFLARE_ACCOUNT_ID`: Your Cloudflare Account ID.
    *   `KV_NAMESPACE_ID`: The ID of your `KV` namespace.
    *   `D1_DATABASE_ID`: The ID of your `cutout-db` D1 database.
    *   `CF_ACCESS_TEAM` / `CF_ACCESS_AUD`: Credentials for Cloudflare Access (protects `/manage`).
    *   `CF_API_TOKEN` *(optional)*: A separate token with `Account Analytics: Read` scope. Unlocks the dashboard stats panels (forwarded/dropped totals, per-rule matches, top senders). The live event tail works without this — it reads from KV.
4.  **Push to `main`** to trigger the deployment workflow.

## Post-deployment Setup
Once the worker is deployed, follow the [Setup Guide](setup.md) to configure your domains.
