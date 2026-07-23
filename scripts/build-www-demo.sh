#!/usr/bin/env bash
# Build the checked, listenerless Uhura Editor/Play bundle served by www.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UHURA_ROOT="$REPO_ROOT/uhura"
EXPORT_WEB_DIST="$UHURA_ROOT/web/dist-export"
WASM_DIST="$UHURA_ROOT/crates/uhura-wasm/pkg/web"
DEMO_SOURCE="$UHURA_ROOT/examples/instagram/client"

if [ "$#" -gt 1 ]; then
  echo "usage: $0 [output-directory]" >&2
  exit 2
fi

DEMO_OUTPUT="${1:-"$REPO_ROOT/www/public/demo"}"

corepack pnpm@10.11.0 -C "$UHURA_ROOT/web" build:export

corepack pnpm@10.11.0 -C "$UHURA_ROOT/web" build:provider

bash "$UHURA_ROOT/scripts/build-wasm.sh"

UHURA_EXPORT_WEB_DIST="$EXPORT_WEB_DIST" \
UHURA_WASM_DIST="$WASM_DIST" \
  cargo run \
    --manifest-path "$UHURA_ROOT/Cargo.toml" \
    --locked \
    -p uhura-cli \
    -- \
    export "$DEMO_SOURCE" \
    --out "$DEMO_OUTPUT" \
    --mount /demo/ \
    --play-entry /play
