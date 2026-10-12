#!/usr/bin/env bash
# wrangler [build].command — builds both halves of the app:
#   1. the Elm frontend into public/app.js
#   2. the Rust worker into wasm, via worker-build
#
# The frontend is built first: `build.rs` hashes public/app.js and the CSS
# modules into the shell's Subresource Integrity attributes, so the crate
# cannot compile until the bundle exists. Building the wasm first would just
# move the failure one step later.
#
# Wraps `worker-build --release`, making sure the worker-build on PATH is new
# enough for the `worker` pin. Cloudflare Workers Builds runs
# `npx wrangler versions upload` directly; it never goes through
# `npm run deploy`, so the `cargo install worker-build` there does not apply.
# worker-build refuses a crate whose `worker` pin is older than itself:
#
#   Unsupported version worker@0.7.4, expected at least worker@0.8.6
#
# Bootstrapping inside the [build] command makes the version explicit wherever
# the build is invoked from. Deliberately checks the version rather than mere
# presence: a builder carrying a stale worker-build would otherwise be
# silently accepted and fail at the refusal above.
set -euo pipefail

cd "$(dirname "$0")/.."

./scripts/build-frontend.sh

# Must track the `worker` major in Cargo.toml, and the pin in
# package.json's deploy script.
REQUIRED_MAJOR_MINOR='0.8'
CARGO_VERSION_REQ="^${REQUIRED_MAJOR_MINOR}"

have_new_enough() {
    command -v worker-build >/dev/null 2>&1 || return 1
    local v
    v="$(worker-build --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+' | head -1)" || return 1
    [ "$v" = "$REQUIRED_MAJOR_MINOR" ]
}

if ! have_new_enough; then
    if ! command -v cargo >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --profile minimal --default-toolchain none
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
    # Present but stale, or absent entirely: --force so an existing
    # binary of the wrong version is replaced rather than kept.
    cargo install -q --force worker-build --version "$CARGO_VERSION_REQ"
fi

exec worker-build --release
