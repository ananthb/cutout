# Internal Deployment Guide (Hosted Version)

This document describes the deployment process for the official Cutout instance hosted at `cutout.calculon.tech`.

## CI/CD Pipeline
Deployment is automated via GitHub Actions in `.github/workflows/deploy.yml`.

Every push to the `main` branch:
1.  **Assembles the configuration**: Generates `wrangler.production.toml` dynamically.
2.  **Applies Migrations**: Runs `wrangler d1 migrations apply cutout-db --remote` to ensure the D1 schema is up to date.
3.  **Deploys the Worker**: Runs `wrangler deploy` to update the code on Cloudflare.
4.  **Syncs Secrets**: Updates worker secrets from the repository's GitHub Secrets.

## Required GitHub Secrets
The following secrets must be configured in the GitHub repository for the official deployment:

| Secret | Description |
|--------|-------------|
| `CLOUDFLARE_API_TOKEN` | API token with Workers, D1, KV, and Email Routing permissions. |
| `CLOUDFLARE_ACCOUNT_ID` | Official Cloudflare Account ID (`59b9363d35105560f2df047c995be5a2`). |
| `D1_DATABASE_ID` | ID of the official `cutout-db` (`304914b0-5657-4b4f-bf43-4be3e2b8c03b`). |
| `KV_NAMESPACE_ID` | ID of the official `KV` namespace (`66c8000103c6420f91cd3943216d5828`). |
| `CF_ACCESS_TEAM` / `_AUD` | Cloudflare Access credentials for the `/manage` super-admin panel. |

## Manual Deployment
If manual deployment is necessary:
1.  Ensure you have `nix` installed.
2.  Run `nix develop`.
3.  Ensure your environment variables match the production requirements.
4.  Run `wrangler deploy -c wrangler.toml` (or use a production-specific config).
