---
description: The strict version-1 spock.toml manifest — schema, validation diagnostics, the canonical scaffolds, and how commands discover a project.
order: 4
---

# spock.toml

`spock.toml` marks a directory as a [framework project](../status.md) and
composes its parts: one required Spock backend, one optional Uhura client,
and the lifecycle that checks and serves them together. Composition is the
whole job — the manifest never merges the two languages or moves authority
between them, because a fact should never be authoritative in both systems
(see [Uhura](../uhura.md)). It is deliberately small: no ports, no build
configuration, no dependency lists — topology only. Everything operational
lives on the [CLI](cli.md) as flags.

## The schema

The manifest is strict. Version 1 is the only accepted version, every listed
field is required unless marked optional, and unknown fields are rejected at
every level rather than ignored.

| Field | Meaning |
| --- | --- |
| `version` | Manifest version. `1` is the only accepted value. |
| `[project] name` | The project's display name. Any non-empty string without control characters or leading/trailing whitespace. |
| `[backend] root` | The backend directory, relative to the manifest. Normalized: no absolute paths, no `..`, no escaping the project. `"."` is valid. |
| `[backend] entry` | The entry source file, relative to `backend.root`. Must name a `.spock` file. |
| `[client] root` | Optional. The Uhura client directory, with the same path rules as `backend.root`. |

The client directory carries its own `uhura.toml`; Spock composes that root
into the project lifecycle but treats its contents as opaque — Uhura owns
their meaning.

Violations are structured diagnostics with stable `SPP` codes, one per
problem, reported together rather than one at a time. A misspelled or
invented field looks like this:

```text
error: SPP003: /work/demo/spock.toml: unknown manifest field `watch`
```

A manifest from a future toolchain is refused rather than half-read:

```text
error: SPP005: /work/demo/spock.toml: unsupported manifest version 2; this tool supports version 1
```

Missing and mistyped fields report the shape they expected — `missing
required `[backend]` table`, `` `project.name` must be a string `` — and
path violations render the rule directly, for example `` invalid
`backend.root`: path must not contain `..` or escape its base directory ``.
The full code table is in [error codes](errors.md).

## Canonical examples

`spock new demo` writes exactly this manifest for the default full-stack
scaffold:

```toml
version = 1

[project]
name = "demo"

[backend]
root = "backend"
entry = "app.spock"

[client]
root = "client"
```

`spock new api --backend-only` writes the same shape without the `[client]`
table:

```toml
version = 1

[project]
name = "api"

[backend]
root = "backend"
entry = "app.spock"
```

## An empty backend entry is valid

The configured entry file must exist, but it may be empty. Both scaffolds
create `backend/app.spock` containing only this placeholder:

```text
// This project has no authority contract yet. Keep this file empty until it does.
```

A project in this state still checks and serves: health, status, the contract
metadata, and — with a client configured — the Uhura Editor and Play all run
while the authority is still being designed.

## Discovery

`spock check`, `spock start`, and `spock dev` resolve their project by
walking from the working directory (or the directory you name) toward the
filesystem root and selecting the nearest `spock.toml`; at nested project
boundaries the closer manifest wins. Naming a `spock.toml` path explicitly
selects exactly its parent directory, with no walk.

`spock init` is the opposite: it adopts exactly the selected directory,
without walking. It inventories that directory — skipping `.git`,
`.spock`, `node_modules`, and `target` as operational noise — and requires
an unambiguous topology: at most one `.spock` backend candidate and one
`uhura.toml` client candidate, with ambiguity or a symlinked candidate
refused before any write. It then writes only the manifest, pointing at the
sources where they already are. A Uhura-only directory additionally gets the
required empty backend at `backend/app.spock`. Existing files are never
moved, rewritten, or overwritten, and a directory that already has a
`spock.toml` is refused as already adopted.

## Topology changes under dev

Edits to `spock.toml` while `spock dev` is running are observed but not
applied: the server reports `restart required` and keeps serving the pinned
generation, because a running database is never migrated. The full reload
semantics are on the [CLI page](cli.md).
