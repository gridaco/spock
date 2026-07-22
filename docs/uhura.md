---
description: What Uhura is, what it owns versus Spock, and what ships with the spock CLI today.
badge:
  text: Incubating
  variant: caution
---

# Uhura

Uhura is a deterministic machine language with an explicit, optional Web UI
profile. It is the client half of a Spock framework project when that project
needs one. An Uhura machine defines state, typed inputs, observations, and
deterministic transitions. A `ui` module may then project an observation into
checked Web semantics without becoming a second state model.

Uhura is developed in its own repository:
[github.com/gridaco/uhura](https://github.com/gridaco/uhura).

## The ownership split

The two languages keep separate responsibilities:

- **Spock** specifies authoritative backend state and guarded product
  behavior — the authority.
- **Uhura** specifies deterministic, non-authoritative client state and
  behavior — the experience. Its optional Web UI profile specifies semantic
  presentation; renderers own physical layout and device mechanics.

Integration crosses versioned port and provider contracts, and one rule
governs the seam: a fact should never be authoritative in both systems. Your
product truth lives in the Spock authority; your interface behavior lives in
the Uhura client; neither restates the other.

## Status: incubating

Uhura is incubating. Its grammar, ABI, package structure, and compatibility
policy may change between releases. `spock@0.5.4` ships the strict Uhura 0.4
candidate end to end: its machine-first language, explicit UI and evidence,
host/provider boundaries, checker, runtime, Editor, Play, and `web-app@1`
profile. Framework releases `spock@0.5.0` through `0.5.3` embed the retired
frontend and cannot run a strict 0.4 project; releases through `0.4.0` are
standalone-only.

The 0.4 documents are candidate specifications, not a compatibility promise.
For the exact current language and its status, use the
[Uhura repository](https://github.com/gridaco/uhura) directly.

## What ships with `spock`

The `spock` npm package bundles the Uhura Editor and Play runtime as a
platform-independent sidecar. In a framework project with a client
configured, `spock start` and `spock dev` serve everything on one origin:

- `/` — the Uhura Editor, a read-only browser for checked previews;
- `/play` — Play mode, running the experience against your live Spock
  authority on the same origin;
- `/~studio` — Spock Studio for the authority itself.

Backend-only projects redirect `/` to Studio and return structured 404s on
client routes.

## What `spock new` scaffolds

Unless you pass `--backend-only`, `spock new` creates a minimal,
dependency-free Uhura client alongside the backend:

```text
client/
├── uhura.toml
├── host.toml
├── machine.uhura
├── ui.uhura
└── evidence.uhura
```

The scaffold is a complete Uhura 0.4 counter: a standalone machine, an
explicit web UI projection, its static evidence, and the host entry that binds
them. You can open it in the Editor immediately; `spock dev` republishes valid
client saves live while you edit.

The starter does not force a meta-framework topology. An application that
selects `[framework] profile = "web-app"` in `uhura.toml` instead gains checked
filesystem conventions for pages, reusable pure components, surfaces, and
colocated examples. Uhura generates the route table and root `Application` in
memory, validates them with the authored machine and UI, and the same Spock
host serves the result. The profile is explicit; arbitrary files do not become
routes or components by ambient convention.

## The canonical full-stack example

The complete framework example — a Spock authority plus a full Uhura
Instagram client served by one `spock start` — lives in the Uhura repository
at
[gridaco/uhura/examples/instagram](https://github.com/gridaco/uhura/tree/main/examples/instagram).
It is strict Uhura 0.4 and uses `web-app@1`, so `spock@0.5.4` can check and
serve it. The example's application-owned TypeScript provider is not compiled
by the host; its README records that separate provider build and the
source-checkout workflow for contributors.
