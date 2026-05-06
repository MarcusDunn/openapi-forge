#!/usr/bin/env bash
# Build the generator-typescript-cli plugin into plugin.wasm.
#
# Steps:
#   1. `npm ci` — install jco / componentize-js / esbuild / typescript into
#      a plugin-local node_modules. Idempotent across runs.
#   2. `npm run typecheck` — `tsc --noEmit` for early type errors.
#   3. `npm run bundle` — esbuild bundles the multi-file TS source into a
#      single ESM JS file at dist/component.js. jco's componentize-js does
#      not accept .ts directly and does not support multi-file ESM.
#   4. `npm run componentize` — jco runs componentize-js to produce a
#      wasip2 component matching forge:plugin/code-generator. `--disable all`
#      strips the JS-runtime WASI imports we don't need (the host's
#      wasmtime_wasi sandbox would supply them anyway, but this keeps the
#      component's import surface minimal).
#
# Required toolchain (provided by `nix develop`):
#   - node ≥ 20 (npm bundled)
#
# Output: ./plugin.wasm (~10–13 MB; StarlingMonkey JS runtime is the floor).
# plugin.wasm and dist/ are .gitignored — this script is the source of truth.

set -euo pipefail
cd "$(dirname "$0")"

# Serialize concurrent invocations (the integration-test suite spawns one
# build per test process). flock is part of util-linux; macOS ships it via
# the `util-linux` brew formula. If neither is present, fall back to a
# best-effort sentinel — concurrent runs will race but each is idempotent.
LOCK_FD=9
LOCK_FILE=".build.lock"
if command -v flock >/dev/null; then
    exec 9>"$LOCK_FILE"
    flock $LOCK_FD
fi

# Skip the rebuild if plugin.wasm is newer than every input file. After the
# lock is held, this lets N parallel test processes share a single build:
# the first does the work, the rest see a fresh artifact and exit.
if [ -f plugin.wasm ] && [ -z "${FORGE_FORCE_REBUILD:-}" ]; then
    newest_input=$(find src package.json tsconfig.json -type f -printf '%T@\n' 2>/dev/null | sort -nr | head -1)
    wasm_mtime=$(stat -c '%Y' plugin.wasm 2>/dev/null || stat -f '%m' plugin.wasm)
    if [ -n "$newest_input" ] && awk -v a="$wasm_mtime" -v b="$newest_input" 'BEGIN{exit !(a >= b)}'; then
        echo "build.sh: plugin.wasm is up to date; skipping (set FORGE_FORCE_REBUILD=1 to override)"
        exit 0
    fi
fi

# Install or refresh deps. `npm ci` is faster than `npm install` and uses
# package-lock.json if present; falls back to `npm install` for first run.
if [ -f package-lock.json ]; then
    npm ci --silent
else
    npm install --silent
fi

npm run --silent typecheck
npm run --silent bundle
npm run --silent componentize

echo "build.sh: wrote $(stat -c %s plugin.wasm 2>/dev/null || stat -f %z plugin.wasm) bytes to plugin.wasm"
