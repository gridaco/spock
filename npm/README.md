# 🖖 spock

> It's only logical.

Spock is an early meta-framework for prototyping an authoritative application
backend and an optional Uhura client as one inspectable project. The Spock
language materializes the backend from tables, functions, and rules; Uhura
defines the client experience; one command checks and serves both on one
origin.

## Install

```sh
# run without installing
npx spock new demo
cd demo
npx spock dev

# or install globally
npm i -g spock
```

This package bundles a prebuilt native binary for your platform
(macOS arm64/x64, GNU-libc Linux x64, Windows x64), plus one shared Uhura
Editor/Play web and WebAssembly sidecar. Alpine and other musl-based Linux
distributions are not supported yet. There is no build step and no network
access at install time.

## Usage

```sh
spock new demo                   # create a full-stack project
cd demo
spock check                      # check manifest, backend, and client
spock dev                        # client-live, backend-pinned development host
spock start                      # fixed combined generation

# language-level escape hatches remain
spock check backend/app.spock    # parse + check one program
spock run backend/app.spock      # materialize + serve (GraphQL, REST, /~studio)
spock gen types backend/app.spock        # emit TypeScript types
spock gen graphql-schema backend/app.spock
```

`spock run` serves the GraphQL API at `/graphql/v1`, REST at `/rest/v1`, the
contract at `/~contract`, and the studio console at `/~studio` — all from the
single binary, offline.

`spock start` and `spock dev` serve the Uhura Editor at `/`, integrated Play at
`/play`, and the framework status and environment protocols under
`/~project/*`. The npm package's shared sidecar provides those browser and
WebAssembly assets offline.

In `spock dev`, valid client saves publish live while invalid saves keep the
last good client generation. Backend `.spock` and `spock.toml` saves are
noticed and reported as restart-required, but never migrate, reseed, or replace
the running database. Restarting the command reconstructs backend state from
seed.

## Links

- Source & docs: https://github.com/gridaco/spock
- Issues: https://github.com/gridaco/spock/issues

MIT
