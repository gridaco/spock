---
description: Install the Spock toolchain from npm — prebuilt native binaries for macOS, Linux, and Windows, with no build step and no network access at install time.
order: 1
---

# Install

Spock ships as one npm package named `spock`. A single install carries the
whole toolchain — compiler, embedded runtime, Studio, and the Uhura Editor and
Play — and every command runs offline once the package is on disk.

## Requirements

- **Node.js 18 or later.** npm is the distribution channel, not a runtime
  dependency: Node runs only the small launcher shim that selects your
  platform's binary, and the toolchain itself is native code.
- **A supported platform:**
  - macOS arm64 or x64
  - Linux x64 with GNU libc — Alpine and other musl-based distributions are
    not supported in 0.5.x; see [project status](../status.md)
  - Windows x64

## Install

`npx` runs Spock without installing anything globally:

```sh
npx spock new demo
cd demo
npx spock dev
```

For a persistent `spock` command, install globally:

```sh
npm i -g spock
```

Verify the installed version:

```sh
spock --version
# spock 0.5.3
```

## What the package contains

The package bundles four prebuilt native binaries — macOS arm64, macOS x64,
Linux x64, Windows x64 — plus one platform-independent [Uhura](../uhura.md)
Editor/Play web and WebAssembly sidecar shared by all four. A zero-dependency
Node shim selects the binary matching your host and hands the process over to
it.

There is no build step, no postinstall script, and no network access at
install time. Installing Spock downloads exactly one package, and everything
after that — checking, serving, the derived API, Studio, the Editor, Play —
runs from the installed files, offline. Each release is published from CI
through npm Trusted Publishing (tokenless OIDC), and the exact tarball
is exercised install-and-run on macOS, Linux, and Windows before it ships.

## Which version you need

Registry releases through `0.4.0` expose only the standalone language
commands: `spock check`, `spock run`, and `spock gen` over one explicit
`.spock` file. `0.5.0` added the framework commands — `spock new`,
`spock init`, and project-wide `check`, `start`, and `dev` for a directory
with a `spock.toml` manifest — so a framework project requires `0.5.0` or
later. The experimental error-declaration preview requires `0.5.2` or later;
[project status](../status.md) records the standing of every surface. The
[changelog](../../CHANGELOG.md) records each cut.

## Building from source

The npm package is the no-checkout path, and for using Spock it is the only
path you need. A source build additionally requires the Rust toolchain, an
initialized recursive `uhura` submodule, and the Studio and Uhura asset builds
before the host can serve a client. The [repository README](../../README.md)
carries the exact commands; this site does not duplicate them.

## VS Code syntax highlighting

The canonical TextMate grammar for `.spock` files lives in
[`editors/vscode`](../../editors/vscode/). It is not published to a
marketplace; you package it locally as a VSIX and install it into your VS Code
profile, and the directory's README walks through packaging, installation, and
updates. The grammar provides highlighting only — the compiler remains the
source of diagnostics.

## Next

Continue to the [quickstart](quickstart.md): create a project, start the
development server, and make your first requests against the derived API.
