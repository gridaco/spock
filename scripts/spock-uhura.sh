#!/usr/bin/env bash
# Run one Spock authority and one Uhura project through the default Editor.
# The Editor hosts Play at /play, so both modes share one supervised runtime.
# Node/pnpm are build-time tools only; this command runs checked-in web assets
# and the locally built Wasm bundle.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPOCK_PORT="${SPOCK_PORT:-4000}"
UHURA_PORT="${UHURA_PORT:-8787}"

usage() {
  echo "Usage: $0 [--spock-port PORT] [--uhura-port PORT] <app.spock> <uhura-project>" >&2
  echo >&2
  echo "Starts Spock and the Uhura Editor; the Editor hosts Play at /play." >&2
  echo "The Uhura project's provider configuration must address the selected Spock port." >&2
}

while (($# > 0)); do
  case "$1" in
    --spock-port)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      SPOCK_PORT="$2"
      shift 2
      ;;
    --uhura-port)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      UHURA_PORT="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "spock-uhura: unknown option: $1" >&2
      usage
      exit 2
      ;;
    *)
      break
      ;;
  esac
done

if [[ $# -ne 2 ]]; then
  usage
  exit 2
fi

for tool in cargo curl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "spock-uhura: required command not found: $tool" >&2
    exit 2
  fi
done

SPOCK_PROGRAM="$1"
UHURA_PROJECT="$2"
[[ "$SPOCK_PROGRAM" = /* ]] || SPOCK_PROGRAM="$ROOT/$SPOCK_PROGRAM"
[[ "$UHURA_PROJECT" = /* ]] || UHURA_PROJECT="$ROOT/$UHURA_PROJECT"

if [[ ! -f "$SPOCK_PROGRAM" ]]; then
  echo "spock-uhura: no Spock program: $SPOCK_PROGRAM" >&2
  exit 2
fi
if [[ ! -f "$UHURA_PROJECT/uhura.toml" ]]; then
  echo "spock-uhura: no Uhura project manifest: $UHURA_PROJECT/uhura.toml" >&2
  exit 2
fi

for port in "$SPOCK_PORT" "$UHURA_PORT"; do
  if [[ ! "$port" =~ ^[0-9]+$ ]]; then
    echo "spock-uhura: invalid port: $port" >&2
    exit 2
  fi
  if ((10#$port < 1 || 10#$port > 65535)); then
    echo "spock-uhura: invalid port: $port" >&2
    exit 2
  fi
done

SPOCK_HEALTH="http://127.0.0.1:${SPOCK_PORT}/~health"
UHURA_URL="http://127.0.0.1:${UHURA_PORT}/"
UHURA_HEALTH="$UHURA_URL"

http_ready() {
  curl --connect-timeout 1 --max-time 1 --silent --fail "$1" >/dev/null 2>&1
}

if ((10#$SPOCK_PORT == 10#$UHURA_PORT)); then
  echo "spock-uhura: Spock and Uhura ports must differ" >&2
  exit 2
fi

required_artifacts=(
  "$ROOT/uhura/web/dist/play/index.html"
  "$ROOT/uhura/crates/uhura-wasm/pkg/web/uhura_wasm.js"
  "$ROOT/uhura/crates/uhura-wasm/pkg/web/uhura_wasm_bg.wasm"
)

for artifact in "${required_artifacts[@]}"; do
  if [[ ! -s "$artifact" ]]; then
    echo "spock-uhura: missing shared build artifact: ${artifact#"$ROOT/"}" >&2
    echo "Run the Uhura frontend check and uhura/scripts/build-wasm.sh once, then retry." >&2
    exit 2
  fi
done

if ! compgen -G "$ROOT/uhura/web/dist/play/assets/*.js" >/dev/null \
  || ! compgen -G "$ROOT/uhura/web/dist/play/assets/*.css" >/dev/null; then
  echo "spock-uhura: the shared Uhura Play asset bundle is incomplete" >&2
  echo "Run the Uhura frontend check once, then retry." >&2
  exit 2
fi

if http_ready "$SPOCK_HEALTH"; then
  echo "spock-uhura: port ${SPOCK_PORT} already has a Spock server" >&2
  echo "Stop it first so this command can own the authority process." >&2
  exit 2
fi

cd "$ROOT"
echo "Building Spock and Uhura launchers..."
cargo build --locked -p spock-cli
cargo build --locked --manifest-path uhura/Cargo.toml -p uhura-cli

spock_pid=""
uhura_pid=""
canvas_out=""
cleanup() {
  trap - EXIT INT TERM
  if [[ -n "$uhura_pid" ]] && kill -0 "$uhura_pid" 2>/dev/null; then
    kill -TERM "$uhura_pid" 2>/dev/null || true
    wait "$uhura_pid" 2>/dev/null || true
  fi
  if [[ -n "$spock_pid" ]] && kill -0 "$spock_pid" 2>/dev/null; then
    kill -TERM "$spock_pid" 2>/dev/null || true
    wait "$spock_pid" 2>/dev/null || true
  fi
  if [[ -n "$canvas_out" ]]; then
    rm -rf "$canvas_out"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

canvas_out="$(mktemp -d "${TMPDIR:-/tmp}/spock-uhura.XXXXXX")"

echo "Starting Spock authority from ${SPOCK_PROGRAM#"$ROOT/"} on port ${SPOCK_PORT}..."
"$ROOT/target/debug/spock" run "$SPOCK_PROGRAM" --port "$SPOCK_PORT" &
spock_pid=$!

ready=false
deadline=$((SECONDS + 60))
while ((SECONDS < deadline)); do
  if http_ready "$SPOCK_HEALTH"; then
    ready=true
    break
  fi
  if ! kill -0 "$spock_pid" 2>/dev/null; then
    wait "$spock_pid" || true
    echo "spock-uhura: Spock stopped before becoming ready" >&2
    exit 1
  fi
  sleep 0.25
done

if [[ "$ready" != true ]]; then
  echo "spock-uhura: Spock did not become ready within 60 seconds" >&2
  exit 1
fi

echo "Starting Uhura Editor for ${UHURA_PROJECT#"$ROOT/"} on port ${UHURA_PORT}..."
"$ROOT/uhura/target/debug/uhura" editor "$UHURA_PROJECT" \
  --port "$UHURA_PORT" --out "$canvas_out" &
uhura_pid=$!

ready=false
deadline=$((SECONDS + 60))
while ((SECONDS < deadline)); do
  if http_ready "$UHURA_HEALTH"; then
    ready=true
    break
  fi
  if ! kill -0 "$uhura_pid" 2>/dev/null; then
    set +e
    wait "$uhura_pid"
    rc=$?
    set -e
    echo "spock-uhura: Uhura stopped before becoming ready" >&2
    if ((rc == 0)); then
      rc=1
    fi
    exit "$rc"
  fi
  if ! kill -0 "$spock_pid" 2>/dev/null; then
    set +e
    wait "$spock_pid"
    rc=$?
    set -e
    echo "spock-uhura: Spock stopped while Uhura was starting" >&2
    if ((rc == 0)); then
      rc=1
    fi
    exit "$rc"
  fi
  sleep 0.25
done

if [[ "$ready" != true ]]; then
  echo "spock-uhura: Uhura did not become ready within 60 seconds" >&2
  exit 1
fi

echo "Spock and Uhura Editor are ready at $UHURA_URL"
echo "Use the Editor's Play button to open the live prototype."
echo "Press Ctrl-C to stop both runtimes."

while kill -0 "$spock_pid" 2>/dev/null && kill -0 "$uhura_pid" 2>/dev/null; do
  sleep 0.5
done

if ! kill -0 "$spock_pid" 2>/dev/null; then
  set +e
  wait "$spock_pid"
  rc=$?
  set -e
  echo "spock-uhura: Spock stopped; stopping Uhura" >&2
else
  set +e
  wait "$uhura_pid"
  rc=$?
  set -e
  echo "spock-uhura: Uhura stopped; stopping Spock" >&2
fi
exit "$rc"
