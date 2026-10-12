#!/usr/bin/env bash
# Compile the Elm app to public/app.js.
#
# Runs in two environments and must behave the same in both:
#   - the nix devShell, which has `elm` from nixpkgs on PATH
#   - Cloudflare Workers Builds, which has node but no elm, so the `elm`
#     npm package (a thin wrapper over the same binary) is used instead
#
# Both are elm 0.19.2. A mismatch would be a hard failure rather than a
# subtle one: elm refuses to build against an elm.json pinned to another
# version, which is the behaviour we want.
set -euo pipefail

cd "$(dirname "$0")/.."

# Resolution order: PATH (the nix devShell), then the local install (npm,
# which is how Cloudflare Builds gets it).
#
# `elm` is a regular dependency rather than a devDependency for exactly this
# reason: it compiles the artifact that gets deployed, so an install that
# omits dev packages — `NODE_ENV=production`, `npm ci --omit=dev` — must still
# produce a working build.
if command -v elm >/dev/null 2>&1; then
    ELM=elm
elif [ -x node_modules/.bin/elm ]; then
    # Absolute: the compiler runs from frontend/ below, so a relative path
    # would resolve against the wrong directory.
    ELM="$PWD/node_modules/.bin/elm"
else
    # Deliberately not falling back to `npx elm`, which would fetch some
    # arbitrary version off the network mid-build. Fail with the fix instead.
    cat >&2 <<'MSG'
build-frontend.sh: no Elm compiler found.

Looked for:
  - `elm` on PATH            (provided by the nix devShell)
  - ./node_modules/.bin/elm  (provided by `npm ci`; `elm` is a dependency,
                              not a devDependency, so an install that omits
                              dev packages should still provide it)

Run `npm ci` — or enter the nix devShell with `nix develop`.
MSG
    exit 1
fi

# --optimize turns on Elm's dead-code elimination and refuses Debug.* calls,
# which is the check we actually want in CI: a stray Debug.log can't reach
# production. Local iteration uses `dev` below.
MODE="${1:-optimize}"
case "$MODE" in
    optimize) FLAGS=(--optimize) ;;
    dev)      FLAGS=() ;;
    *) echo "usage: $0 [optimize|dev]" >&2; exit 2 ;;
esac

cd frontend
# shellcheck disable=SC2086
$ELM make src/Main.elm "${FLAGS[@]}" --output=../public/app.js

cd ..
echo "built public/app.js ($(wc -c < public/app.js) bytes)"
