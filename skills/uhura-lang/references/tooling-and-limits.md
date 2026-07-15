# Uhura Tooling and Current Limits

## Published toolchain

Require Node.js 18 or newer and use the npm-distributed framework CLI:

```sh
npm install --global spock@latest
spock --version
```

The distributed Spock package carries the framework host, Editor, Play
runtime, and matching runtime assets. Do not require a separate Uhura binary or
an additional implementation toolchain.

Framework commands:

```sh
spock new my-app
spock init path/to/existing-project
spock check path/to/project
spock dev path/to/project --port 4000
spock start path/to/project --port 4000
```

`spock check` validates the configured backend and Uhura client as one project.
`start` serves one fixed checked generation. `dev` publishes valid client
changes live while reporting backend and topology changes as
`restart_required`; it does not migrate, reseed, or replace the active
database.

The published CLI does not currently expose standalone Uhura `fmt`, `trace`,
`project`, `editor`, or `play` commands. Do not invent replacements for those
missing public commands.

## One-origin framework surface

After `spock start` or `spock dev` on port 4000:

```text
http://127.0.0.1:4000/                 Uhura Editor and read-only Canvas
http://127.0.0.1:4000/play             live Uhura Play
http://127.0.0.1:4000/~studio          Spock Studio
http://127.0.0.1:4000/~health          readiness
http://127.0.0.1:4000/~project/status  generation and reload state
```

Editor and Play use the same hosted generation and same-origin authority
capabilities. Do not start a separate backend or Uhura server for a normal
framework project.

Canvas is a deterministic, read-only projection of checked examples. Its
available provenance, annotation, and relationship affordances vary with the
installed Spock version. It cannot edit source or prove live provider behavior.

Play runs the real experience machine against the selected provider. Restart
resets the Uhura session but not necessarily provider truth. Confirm actor
selection and inspect Studio or the affected endpoint when durable state is
expected to change.

## Diagnostics and live reload

- Fix the earliest project diagnostic first; later errors may cascade.
- A valid Uhura save may publish a new client generation in `spock dev`.
- An invalid client save retains the last good Play generation when available.
- Backend, seed, or topology changes require a process restart.
- An occupied port or a final bind failure means the project is not running,
  even if earlier output reported successful checking.
- Inspect `/~project/status` before attributing stale Play behavior to source.

## Current limits

- The Editor is read-only and has no source editing surface.
- The public CLI has no standalone Uhura formatter, trace printer, or static
  Canvas export command.
- Do not assume workflow connectors, automatic edge routing, or first-class
  surface hierarchy controls are present; inspect the installed Editor and
  report the version when a requested affordance is unavailable.
- There is no Canvas-to-source round trip.
- Browser history reconciliation, command cancellation/timeouts, retained
  session migration, slots, shared layouts, and surface results remain limited
  or deferred.
- Provider identity seams used for local actor switching are not production
  authentication or authorization.

Do not hide these gaps in CSS, fixtures, renderer callbacks, or duplicated
provider state. Name the boundary and keep authoritative facts in Spock.
