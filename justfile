set positional-arguments

repo_root := justfile_directory()

# Source builds must use the matching local Uhura web and Wasm artifacts.
export SPOCK_UHURA_WEB_DIST := repo_root + "/uhura/web/dist"
export SPOCK_UHURA_WASM_DIST := repo_root + "/uhura/crates/uhura-wasm/pkg/web"

# List the local developer commands.
default:
    @just --list

# Run the local Rust CLI with the same arguments as the installed `spock` command.
spock *args:
    #!/usr/bin/env bash
    set -euo pipefail
    exec cargo run --locked -p spock-cli -- "$@"

# Check a framework project at the exact path supplied by the caller.
check path *args:
    #!/usr/bin/env bash
    set -euo pipefail
    path="$1"
    shift
    exec cargo run --locked -p spock-cli -- check "${path}" "$@"

# Serve a fixed generation of a framework project at the exact supplied path.
start path *args:
    #!/usr/bin/env bash
    set -euo pipefail
    path="$1"
    shift
    exec cargo run --locked -p spock-cli -- start "${path}" "$@"

# Serve a framework project at the exact supplied path with client live reload.
dev path *args:
    #!/usr/bin/env bash
    set -euo pipefail
    path="$1"
    shift
    exec cargo run --locked -p spock-cli -- dev "${path}" "$@"
