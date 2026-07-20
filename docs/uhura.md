---
description: What Uhura is, what it owns versus Spock, and what ships with the spock CLI today.
badge:
  text: Incubating
  variant: caution
---

# Uhura

Uhura is a declarative UI language and deterministic experience runtime, and
the optional client half of a Spock framework project. An Uhura program
defines what an interface presents, the local UI state that drives it, and how
semantic events advance that state; a runtime evaluates it into a
renderer-neutral semantic view.

Uhura is developed in its own repository:
[github.com/gridaco/uhura](https://github.com/gridaco/uhura).

## The ownership split

The two languages keep separate responsibilities:

- **Spock** specifies authoritative backend state and guarded product
  behavior — the authority.
- **Uhura** specifies non-authoritative interface state and experience
  behavior — the experience. Renderers own layout and presentation.

Integration crosses versioned port and provider contracts, and one rule
governs the seam: a fact should never be authoritative in both systems. Your
product truth lives in the Spock authority; your interface behavior lives in
the Uhura client; neither restates the other.

## Status: incubating

Uhura is incubating. Its grammar, ABI, package structure, and compatibility
policy may change between releases — the 0.5.3 toolchain, for example,
renamed core widgets in a deliberately breaking pre-v1 update. There is no
accepted Uhura specification yet, which is why this site documents no Uhura
syntax: anything copied here would fossilize a draft that Uhura's own process
treats as disposable. For current material, use the
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

## The canonical full-stack example

The complete framework example — a Spock authority plus a full Uhura
Instagram client served by one `spock start` — lives in the Uhura repository
at
[gridaco/uhura/examples/instagram](https://github.com/gridaco/uhura/tree/main/examples/instagram).
Its Play provider needs a one-time build the CLI does not perform (see that
example's README for the exact `pnpm` commands) before `spock start
examples/instagram` serves it.
