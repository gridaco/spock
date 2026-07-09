# spock

> It's only logical.

Spock is an early programming language for describing application backends as a
small, inspectable source of truth.

Most application backends spread the same intent across many layers: database
schema, API serializers, mutation handlers, validation, authorization, and
tests. The result is often a system where the real product logic exists, but
only as an accident of several files agreeing with each other.

Spock exists to make that logic first-class.

## Why this exists

Modern apps are mostly rules:

- what data exists
- what public shape that data has
- what operations can be called
- what each operation accepts and returns

Those rules are usually implemented in separate tools that do not understand
each other. A schema migration does not know about the API response. A route
handler does not know if its input shape still matches the public contract. A
validator does not know which model it is really protecting.

Spock is an attempt to describe those relationships directly.

```spock
model post {
    id: uuid
    title: text
    body: text
    published: bool
}

view post_preview from post {
    id: .id
    title: .title
    published: .published
}

fn publish_post(id: uuid) -> post_preview {
    // rpc exposed by the backend
}
```

The goal is not to define every part of a backend on day one. The first useful
version should define the core contract: data, public views, and callable
functions.

## What Spock is exploring

The first buildable version of Spock should stay small:

- `model` declarations for persistent application data
- `view` declarations for public projections
- `fn` declarations for RPC-style backend operations

The long-term direction is a logic container that can be reasoned about,
compiled, checked, tested, and presented as data.

## Implementation

Spock will be implemented as a real language with a compiler and runtime built
on Rust.

The npm package metadata lives under `npm/` only to reserve the package name.
It is not the primary implementation target.

## Repository Layout

- `examples/` contains current-valid Spock examples only.
- `docs/rfd/` contains discussion drafts and proposal-only language ideas.
- `npm/` contains package metadata for npm name reservation.

## Status

Spock is currently a design-stage proposal. There is no compiler, runtime, or
stable specification yet.

The older, more ambitious draft has been moved to
`docs/rfd/0000-vision.spock`. It is a sketch of possible direction, not the v0
implementation target.

The name and phrase stay:

> Spock. It's only logical.
