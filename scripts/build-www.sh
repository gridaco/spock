#!/usr/bin/env bash
# Build the complete production website from a clean source checkout.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UHURA_ROOT="$REPO_ROOT/uhura"
TOOL_ROOT="$UHURA_ROOT/target/tools"

read_toolchain() {
  sed -n 's/^[[:space:]]*channel = "\([^"]*\)"/\1/p' "$1"
}

ROOT_TOOLCHAIN="$(read_toolchain "$REPO_ROOT/rust-toolchain.toml")"
UHURA_TOOLCHAIN="$(read_toolchain "$UHURA_ROOT/rust-toolchain.toml")"

if [ -z "$ROOT_TOOLCHAIN" ] || [ "$ROOT_TOOLCHAIN" != "$UHURA_TOOLCHAIN" ]; then
  echo "root and Uhura Rust toolchain pins must match" >&2
  exit 1
fi

export CARGO_HOME="${CARGO_HOME:-"$HOME/.cargo"}"
export RUSTUP_HOME="${RUSTUP_HOME:-"$HOME/.rustup"}"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_TOOLCHAIN="$ROOT_TOOLCHAIN"

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
    sh -s -- \
      -y \
      --no-modify-path \
      --profile minimal \
      --default-toolchain "$ROOT_TOOLCHAIN"
fi

rustup toolchain install "$ROOT_TOOLCHAIN" --profile minimal
rustup target add wasm32-unknown-unknown --toolchain "$ROOT_TOOLCHAIN"

WASM_BINDGEN_VERSION="$(
  sed -n '/name = "wasm-bindgen"/{n;s/version = "\([^"]*\)"/\1/p;q;}' \
    "$UHURA_ROOT/Cargo.lock"
)"
WASM_BINDGEN="$TOOL_ROOT/bin/wasm-bindgen"
INSTALLED_WASM_BINDGEN=""

if [ -x "$WASM_BINDGEN" ]; then
  INSTALLED_WASM_BINDGEN="$("$WASM_BINDGEN" --version | awk '{print $2}')"
fi

if [ -z "$WASM_BINDGEN_VERSION" ]; then
  echo "could not read the wasm-bindgen version from Uhura's Cargo.lock" >&2
  exit 1
fi

if [ "$INSTALLED_WASM_BINDGEN" != "$WASM_BINDGEN_VERSION" ]; then
  cargo install wasm-bindgen-cli \
    --version "$WASM_BINDGEN_VERSION" \
    --locked \
    --force \
    --root "$TOOL_ROOT"
fi

test "$("$WASM_BINDGEN" --version | awk '{print $2}')" = "$WASM_BINDGEN_VERSION"

WASM_BINDGEN="$WASM_BINDGEN" bash "$REPO_ROOT/scripts/build-www-demo.sh"
corepack pnpm@10.11.0 -C "$REPO_ROOT/www" build
