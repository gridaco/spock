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
spock init PATH                  # adopt existing sources without moving them
spock new api --backend-only     # omit the optional Uhura client

# language-level escape hatches remain
spock check backend/app.spock    # parse + check one program
spock run backend/app.spock      # materialize + serve (GraphQL, REST, /~studio)
spock gen types backend/app.spock        # emit TypeScript types
spock gen graphql-schema backend/app.spock
```

`spock run` serves REST at `/rest/v1`, the contract at `/~contract`, and the
studio console at `/~studio` — all from the single binary, offline. It also
serves GraphQL at `/graphql/v1` when the contract derives an operation root; an
empty authority reports that capability as unavailable instead of inventing a
placeholder field.

For a project with a configured client, `spock start` and `spock dev` serve the
Uhura Editor at `/` and integrated Play at `/play`. Both modes serve Spock
Studio and the framework protocols on the same origin. A backend-only project
redirects `/` to Studio and returns structured 404 responses for client routes.
The npm package's shared sidecar provides the browser and WebAssembly assets
offline.

In `spock dev`, valid client saves publish live while invalid saves keep the
last good client generation. Backend inputs—including the `.spock` source and
referenced seed assets—and topology-affecting `spock.toml` saves are noticed
and reported as restart-required, but never migrate, reseed, or replace the
running database. Restarting the command reconstructs backend state from seed.

The one-command host does not merge language ownership. Spock remains the
authority for durable facts, policy, and mutations; Uhura owns the client
experience and non-authoritative UI-session state. `spock.toml` composes their
roots, while each language keeps its own checker and configuration.

## Links

- Source & docs: https://github.com/gridaco/spock
- Issues: https://github.com/gridaco/spock/issues

MIT
