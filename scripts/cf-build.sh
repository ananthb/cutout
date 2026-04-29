#!/usr/bin/env bash
# Bootstrap script for Cloudflare Builds (Workers CI).
# CF Builds doesn't ship the Rust toolchain by default, so we install it here
# and apply pending D1 migrations before the deploy step packages the worker.
# The matching Deploy command must source $HOME/.cargo/env so wrangler picks
# up worker-build on PATH when it runs `[build] command = "worker-build --release"`
# from wrangler.toml. Configure that in the dashboard as:
#   . "$HOME/.cargo/env" && npx wrangler deploy
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain none
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
rustup show

cargo install --locked worker-build

npx --yes wrangler d1 migrations apply cutout-db --remote
