---
description: Scaffold a project, declare a two-table authority, serve it, and meet your first derived error — in about ten minutes.
order: 2
---

# Quickstart

In about ten minutes you will scaffold a project, declare a two-table
authority, serve it, read from it over REST, write to it over GraphQL, hit
your first derived error, and read the contract that predicted it. It assumes
the `spock` CLI from [Install](install.md) is on your path.

## Scaffold a project

```sh
spock new demo
cd demo
```

```text
demo/
├── spock.toml
├── backend/
│   └── app.spock
└── client/
    ├── uhura.toml
    ├── host.toml
    ├── machine.uhura
    ├── ui.uhura
    └── evidence.uhura
```

This is a framework project: a `spock.toml` manifest composing a Spock
backend with an Uhura client. The scaffolded `backend/app.spock` is
deliberately empty — a project whose authority is not designed yet can
already serve its client, health, and status surfaces. This page fills the
backend; the client half has [its own page](../uhura.md).

## Declare the authority

Replace the contents of `backend/app.spock` with a complete program:

```spock
auth table user {
  key id: uuid = auto
  username: text unique
}

table post {
  key id: uuid = auto
  author: user
  caption: text
  at: timestamp = now
}

seed {
  ada = user { username: "ada" }
  lin = user { username: "lin" }
  post { author: ada, caption: "hello from the authority" }
  post { author: lin, caption: "works on my machine" }
}
```

Two tables and a seed. `auth table user` makes `user` the program's
[anchor](../language/actor.md) — the one table identity references point at.
`author: user` is a reference; `= auto` and `= now` are defaults; `unique` is
a constraint the whole surface will enforce. Field syntax, types, and
constraints are covered in [Tables, types, and defaults](../language/tables.md). The `seed`
block populates the database through the ordinary write path on every start —
[seed replay](../language/seed.md), the reason a prototype always boots into
believable state.

## Check it

```sh
spock check
```

```text
ok: project `demo` — 2 table(s), 0 record(s), 0 fn(s), 4 seed row(s), 2 preview(s), 1 replay-derived preview(s), 1 unchecked link(s), 1 warning(s)
warning: link: application-owned provider adapter code remains unchecked
```

`check` does more than parse: it materializes the schema in memory, validates
every declaration, and replays the seed through the runtime write path. The
counts after the seed rows — previews, links, the warning — describe the
Uhura client half of the project, which Spock observes but does not check.

## Serve it

```sh
spock dev
```

```text
warning: backend inputs (including referenced seed assets) and spock.toml topology changes are observed but not applied; restart `spock dev` to reconstruct backend state from seed
listening on http://127.0.0.1:4000
  GET  /                     Uhura Editor
  GET  /play                 Uhura Play
  GET  /~studio              Spock Studio
  GET  /~contract            active Spock contract
  GET  /~project/status      framework generation status
  GET  /~health              aggregate readiness
  *    /rest/v1/*            authority REST and RPC
  POST /graphql/v1           GraphQL when the contract is non-empty
```

One process, one origin: the root path serves the [Uhura Editor](../uhura.md),
and the authority protocols mount beside it. The warning is the development
doctrine in one line — state is disposable, so backend edits take effect by
restarting and replaying the seed, never by migrating in place.

## First read

```sh
curl -sS http://127.0.0.1:4000/rest/v1/user
```

```json
{
  "rows": [
    { "id": "019f751c-3a75-7cb3-ba39-92e36fb6dd87", "username": "ada" },
    { "id": "019f751c-3a75-7cb3-ba39-92fb2cbe32ca", "username": "lin" }
  ]
}
```

You declared no route and no serializer. Every table receives a derived
read/write surface — [the floor](../language/derived-api.md) — with list and
by-key reads over REST, ordered by key ascending.

## First write

Open <http://127.0.0.1:4000/graphql/v1> in a browser: GET serves GraphiQL,
with the full derived schema behind introspection. Run an insert:

```graphql
mutation {
  insert_user_one(object: { username: "grace" }) {
    id
    username
  }
}
```

```json
{
  "data": {
    "insert_user_one": {
      "id": "019f751c-7c1d-7b83-a31f-a5201b4d2ebe",
      "username": "grace"
    }
  }
}
```

`insert_user_one` was derived from the table declaration, in the
Hasura-mirrored dialect specified in the [GraphQL specification](../spec/graphql.md) —
standard GraphQL tooling consumes it unmodified.

## First derived error

Run the same mutation again:

```json
{
  "data": null,
  "errors": [
    {
      "message": "user.username is already taken",
      "locations": [{ "line": 1, "column": 12 }],
      "extensions": {
        "code": "user_username_taken",
        "kind": "unique",
        "table": "user",
        "fields": ["username"]
      }
    }
  ]
}
```

`user_username_taken` was minted at compile time from the `unique` constraint
on `username` — the code existed in the contract before any request was made,
and generated clients ship it as a typed union rather than parsing a message
string. Derived errors are the product surface of your constraints, not
incidental runtime strings; [The derived API](../language/derived-api.md)
catalogs the kinds.

## The contract

Everything above was derived from one artifact, served verbatim:

```sh
curl -sS http://127.0.0.1:4000/~contract
```

The contract is the compiled JSON the program produces: its tables, fields,
seed, and — the part the previous section exercised — each table's derived
errors, declared before any request:

```json
[
  { "code": "user_already_exists",    "kind": "key",        "fields": ["id"],       "status": 409 },
  { "code": "user_username_taken",    "kind": "unique",     "fields": ["username"], "status": 409 },
  { "code": "user_username_required", "kind": "required",   "fields": ["username"], "status": 422 },
  { "code": "user_restricted",        "kind": "restricted", "fields": [],           "status": 409 }
]
```

That array is `tables[0].errors` — the failure surface of the `user` table,
enumerated by the compiler. The contract's shape is normative in the
v0 specification, §6; `spock gen types` turns it into TypeScript when you
want the codes as literal unions.

## Where next

For the backend on its own:

- [The tutorial](tutorial.md) — build a mini-Instagram backend: functions,
  refusals, personas, and the deliberate surface.
- [The language guides](../language/tables.md) — tables onward: types,
  references, seeds, functions, and the actor seam.
- [The reference](../reference/README.md) — endpoint tables, error codes,
  CLI commands, and the manifest.

For the full stack with Uhura:

- [The tutorial](tutorial.md) still comes first — the authority is the same
  either way.
- [Uhura](../uhura.md) — what the client language owns, and the Editor and
  Play surfaces you saw at `/`.
- [Examples](../examples.md) — the portfolio, including the canonical
  full-stack Instagram project.
