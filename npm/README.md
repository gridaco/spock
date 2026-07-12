# 🖖 spock

> It's only logical.

Spock is an early programming language for prototyping application backends as
a small, inspectable source of truth. You describe your tables, functions, and
rules once; Spock materializes a running backend (embedded SQLite) and serves
it over GraphQL and REST, with generated TypeScript types.

## Install

```sh
# run without installing
npx spock run app.spock

# or install globally
npm i -g spock
spock run app.spock
```

This package bundles a prebuilt native binary for your platform
(macOS arm64/x64, Linux x64, Windows x64). There is no build step and no
network access at install time.

## Usage

```sh
spock check app.spock            # parse + check a program
spock run app.spock              # materialize + serve (GraphQL, REST, /~studio)
spock gen types app.spock        # emit TypeScript types
spock gen graphql-schema app.spock
```

`spock run` serves the GraphQL API at `/graphql/v1`, REST at `/rest/v1`, the
contract at `/~contract`, and the studio console at `/~studio` — all from the
single binary, offline.

## Links

- Source & docs: https://github.com/gridaco/spock
- Issues: https://github.com/gridaco/spock/issues

MIT
