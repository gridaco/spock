---
description: Every spock command — check, new, init, start, dev, run, build, and gen — with flags, defaults, and the output each one prints.
order: 1
---

# CLI

The toolchain is one binary named `spock`. It serves two shapes of work: a
[framework project](../status.md) — a directory composed by a
[`spock.toml` manifest](spock-toml.md) — and a standalone program — one
explicit `.spock` file. Commands that accept a `PATH` resolve the nearest
enclosing project; commands that accept a `FILE` operate on exactly the file
you name.

## Invocation

`npx spock` runs the toolchain without a global install; `npm i -g spock`
makes the bare `spock` command available. Platform requirements and the
distribution model live on the [install page](../start/install.md).

```sh
$ spock --version
spock 0.5.3
```

`-V` is the short form. Every command supports `--help`.

## spock check [PATH]

Prove a target loads, without binding a listener or touching named state. The
command is polymorphic on its target.

**An explicit `.spock` path** selects the standalone one-file proof — the
mode is chosen by the path's shape, so a missing `.spock` file fails as a
standalone target rather than falling back to project discovery. This is a full load proof, not a parse: the
contract is compiled, the schema is materialized in an in-memory database,
fn bodies and validator checks are executed against it, defaults are proven,
and the seed is replayed through the ordinary write path. Everything
`spock run` would reject before serving, `check` rejects here. The summary
counts what the contract declares, including its escape ledger:

```text
$ spock check blog.spock
ok: 2 table(s), 0 record(s), 1 fn(s) (1 unchecked escapes), 2 seed row(s)
```

**No target, or a directory target**, resolves the nearest `spock.toml` —
walking from the working directory (or the named directory) toward the
filesystem root — and checks the whole framework project: manifest, backend,
client when one is configured, and the links between them that are currently
provable.

```text
$ spock check
ok: project `demo` — 0 table(s), 0 record(s), 0 fn(s), 0 seed row(s), 1 preview(s), 0 replay-derived preview(s), 1 unchecked link(s), 1 warning(s)
warning: link: application-owned provider adapter code remains unchecked
```

An explicit `spock.toml` path selects exactly that manifest's directory, with
no walk.

## spock new NAME [--backend-only]

Create a new framework project as a child directory of the working directory.
The default scaffold is full-stack; `--backend-only` omits the Uhura client.
The full-stack inventory:

```text
demo/
├── spock.toml
├── backend/
│   └── app.spock
└── client/
    ├── uhura.toml
    ├── app/home/page.uhura
    ├── app/home/page.examples.uhura
    ├── catalog/base.toml
    └── fixtures/…
```

`new` is create-new-only: the destination must not exist, and a conflicting
entry fails the whole plan before any write. `NAME` must be a single safe path
component — an invalid name fails with, for example:

```text
error: invalid project name `nested/name`: nested paths are not allowed; NAME must be one safe path component
```

On success the command prints what it created and the next step:
`next: run `spock dev` from the project directory above`.

## spock init [PATH]

Adopt an existing directory as a framework project, in place. Unlike
discovery, `init` never walks to a parent: it inventories exactly the
selected directory (the working directory when `PATH` is omitted), finds the
one `.spock` backend candidate and the one `uhura.toml` client candidate, and
writes a manifest that points at them. Existing sources are never moved or
rewritten. Two `.spock` files is an ambiguity error before any write:

```text
error: SPP015: found multiple Spock backend candidates; choose one explicitly
  note: one.spock
  note: two.spock
```

A directory with only an Uhura client gets the required backend added: `init`
creates `backend/app.spock` with the empty-authority placeholder, because a
framework project always has a backend even before the authority is designed.
A directory that already contains `spock.toml` is already adopted, and `init`
refuses.

## spock start [PATH] [--port 4000] [--db FILE]

Check a framework project once, then serve one fixed generation of it. The
server binds `127.0.0.1` only — the port is local, and the
[actor seam](../language/actor.md) it exposes is a development surface.
`--db FILE` names a disposable database file; disposable means it is
reconstructed from source and seed on every process start, never migrated
([seed replay](../language/seed.md) is the only state model).

`start` and `dev` refuse an explicit `.spock` target rather than silently
reinterpreting it as a project:

```text
$ spock start blog.spock
error: `blog.spock` selects standalone `.spock` file mode; run `spock run` with that file instead
```

## spock dev [PATH] [--port 4000] [--db FILE]

Serve a framework project with live client publication. Target resolution,
binding, and `--db` semantics match `start`; the difference is that `dev`
observes source changes. Its reload semantics are deliberately asymmetric —
see [reload semantics](#reload-semantics-under-dev) below. At startup it
states the policy plainly:

```text
warning: backend inputs (including referenced seed assets) and spock.toml topology changes are observed but not applied; restart `spock dev` to reconstruct backend state from seed
```

## spock run FILE [--port 4000] [--db FILE]

Compile, materialize, seed, and serve one standalone program. Without `--db`
the database is in-memory; with it, the named file is recreated on every run.
The startup line reports the load and the seed replay:

```text
spock v0 — contract loaded: 2 table(s), 1 fn(s), 2 seed row(s) replayed
listening on http://127.0.0.1:4000
```

## spock build FILE [-o FILE]

Compile a standalone program to its [contract](../language/derived-api.md) —
the compiled JSON artifact the runtime serves verbatim at `GET /~contract`. The
JSON goes to stdout, or to the file named by `-o`/`--out`.

## spock gen

Generate derived artifacts from a standalone program. Both subcommands print
to stdout or write to `-o`/`--out`.

### spock gen types FILE [-o FILE]

TypeScript types: one interface per table row, insert and update shapes, and
the error-code unions the contract declares.

### spock gen graphql-schema FILE [-o FILE]

The exact GraphQL SDL the runtime serves for this program, for offline schema
tooling. Seed data does not gate this artifact — the schema is derived from
the contract alone.

## Reload semantics under dev

`spock dev` treats the two languages differently, on purpose: client
publication is safe to do live, while backend state is disposable by doctrine
and must only ever be reconstructed, never patched in place.

- **Valid client saves publish live.** Each save is reported as a building
  and then published revision.
- **Invalid client saves keep the last-known-good** Play generation serving
  while Editor diagnostics report the rejection. When no valid generation has
  existed yet, `/~project/status` reports the client as `cold_invalid`.
- **Backend edits — the `.spock` entry, referenced seed assets, and
  topology-affecting `spock.toml` saves — are observed but never applied.**
  A running database is never reseeded, migrated, or replaced under a live
  process. The server reports:

  ```text
  backend: restart required; active state remains pinned (changed: backend/app.spock)
  ```

  Restarting `dev` reconstructs backend state from seed. The open design for
  richer development-state reload is
  [RFD 0023](../rfd/0023-development-state-reload.md).

## Diagnostic format

Compile-time diagnostics render as `path:line:col: error[CODE]: message`, and
the codes are stable:

```text
broken.spock:3:9: error[E003]: unknown type `texxt` (not a builtin, not a declared table)
error: 1 diagnostic(s), contract not produced
```

Project-layer failures — manifest, discovery, and scaffold planning — carry
their own stable `SPP` codes, rendered as `error: SPP015: message` with
`note:` lines for detail. The full code tables live in
[error codes](errors.md).

## Endpoints printed at listen

Framework serving (`start` and `dev`) prints its surface at bind time:

| Endpoint | Serves |
| --- | --- |
| `GET /` and `GET /play` | Uhura Editor and Play, when a client is configured |
| `GET /~studio` | Spock Studio |
| `GET /~contract` | the active contract, as data |
| `GET /~project/status` | framework generation status |
| `GET /~health` | aggregate readiness |
| `* /rest/v1/*` | authority REST and RPC |
| `POST /graphql/v1` | GraphQL, when the contract is non-empty |

Standalone `run` prints the same authority surface plus Studio, and
`* /storage/v1/object` when the program uses storage. The full protocol —
REST operators, GraphQL binding, the storage plane, and wire statuses — is
the [HTTP API reference](http.md).
