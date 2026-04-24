#!/usr/bin/env bash
# Worker build wrapper.
#
# `worker-build` runs cargo + wasm-bindgen + wasm-opt + esbuild. Its esbuild
# step hardcodes only `cloudflare:sockets` and `cloudflare:workers` as
# externals, so any wasm-bindgen import from `cloudflare:email` fails to
# bundle. We let worker-build do its prep work, then re-run esbuild with
# `cloudflare:email` added.
set -euo pipefail

# Run worker-build. The bundle step is expected to fail on the
# `cloudflare:email` import — everything before it (cargo build,
# wasm-bindgen, wasm-opt) still runs and produces build/.
worker-build --release || true

# Find esbuild: prefer one on PATH (nix devShell), fall back to the copy
# worker-build downloaded into ~/.cache.
ESBUILD="$(command -v esbuild || true)"
if [ -z "$ESBUILD" ]; then
  ESBUILD="$(find "${HOME:-/root}/.cache/worker-build" -type f -name esbuild -executable 2>/dev/null | head -n 1)"
fi
if [ -z "$ESBUILD" ] || [ ! -x "$ESBUILD" ]; then
  echo "scripts/build.sh: esbuild not found on PATH or in ~/.cache/worker-build" >&2
  exit 1
fi

# Re-bundle with cloudflare:email added to the external list.
mkdir -p build/worker
"$ESBUILD" \
  build/shim.js \
  --bundle \
  --external:./index_bg.wasm \
  --external:cloudflare:sockets \
  --external:cloudflare:workers \
  --external:cloudflare:email \
  --format=esm \
  --outfile=build/worker/shim.mjs

cp build/index_bg.wasm build/worker/
