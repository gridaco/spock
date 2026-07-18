---
description: Every table derives a contract, REST reads, a GraphQL schema, and a pre-declared error vocabulary — none of it authored by you.
order: 2
---

# The derived API

You declared tables. Everything else on this page is derived from them: the
contract, the REST reads, the GraphQL schema — types, resolvers, naming — and
the error vocabulary a client can fail against. Nothing below is authored.
There is no route file, no serializer, no resolver map, no schema-tracking
step, and no per-table configuration to drift out of date. This derived
per-table surface is the floor: every table receives it, identically, on
every load. The functions a program declares on top of the floor are the
deliberate surface — those belong to [Functions and refusals](functions.md).

## The base program

Every example on this page runs against this program:

```spock
/// A person. The identity anchor.
auth table user {
  key id: uuid = auto
  /// The unique handle.
  username: text unique
}

/// A post someone wrote.
table post {
  key id: uuid = auto
  title: text
  subtitle: text?
  slug: text unique
  status: "draft" | "published" = "draft"
  author: user
}

seed {
  maya = user { id: "11111111-0000-7000-8000-000000000001", username: "maya" }
  rene = user { id: "11111111-0000-7000-8000-000000000002", username: "rene" }
  post { id: "22222222-0000-7000-8000-000000000001",
         title: "Hello, floor", slug: "hello-floor",
         status: "published", author: maya }
  post { id: "22222222-0000-7000-8000-000000000002",
         title: "Second draft", subtitle: "Not published yet",
         slug: "second-draft", author: rene }
}
```

The table syntax is covered in [Tables, types, and defaults](tables.md) and
the seed block in [Seed and disposable state](seed.md); the seed here pins
literal keys so every transcript below replays byte for byte. Serve it as a
standalone program and everything on this page is live:

```sh
spock run app.spock --port 4000
```

## The contract

Compilation produces one JSON document — the contract — served verbatim at
`GET /~contract`. It is the artifact the runtime loads and the artifact tools
consume, and it carries everything a client, a code generator, or an agent
needs before the first request: the tables with their fields, types, keys,
uniques, and defaults; the declared fns and records; every derived error each
table can produce, with its kind and HTTP status; and the `///` doc comments,
carried as `doc` strings on tables, fields, fns, and parameters. For the base
program the top level reads:

```json
{ "spock": "v0", "errors": [], "tables": [ ... ], "records": [], "fns": [], "seed": [ ... ] }
```

The contract shape is frozen additively for v0.x: new optional fields may
appear, and nothing already in the shape is renamed or removed (v0
specification, §6). Binding to it is the point — `spock gen types` and
`spock gen graphql-schema` are projections of this same document, and the
[CLI reference](../reference/cli.md) covers both.

## REST reads

Every table derives a list and a by-key read under `/rest/v1`:

```sh
curl -sS http://127.0.0.1:4000/rest/v1/post
```

```json
{
  "rows": [
    { "id": "22222222-0000-7000-8000-000000000001",
      "title": "Hello, floor", "subtitle": null, "slug": "hello-floor",
      "status": "published", "author": "11111111-0000-7000-8000-000000000001" },
    { "id": "22222222-0000-7000-8000-000000000002",
      "title": "Second draft", "subtitle": "Not published yet", "slug": "second-draft",
      "status": "draft", "author": "11111111-0000-7000-8000-000000000002" }
  ]
}
```

Lists accept `?limit=N` with a default of 50 and a ceiling of 200 — a
protocol default, not per-table syntax. Rows come back ordered by key
ascending; with the `auto` default minting UUIDv7 keys, key order
approximates insertion order, so a fresh row lands at the end of the list.
`GET /rest/v1/post/22222222-0000-7000-8000-000000000001` returns that one
row by key. Filters, ordering, and offset are available as query operators;
the [HTTP reference](../reference/http.md) owns the operator tables.

Tables are read-only over REST in v0: table writes are GraphQL-first, and
REST's write path is `POST /rest/v1/rpc/{fn}` — the deliberate surface, not
the floor. The [HTTP reference](../reference/http.md) has the full endpoint
table.

## GraphQL reads

The derived GraphQL schema mirrors Hasura's dialect conventions — the
auto-CRUD shape more humans and LLMs have seen than any other — and the
[GraphQL dialect specification](../spec/graphql.md) is the normative source.
The schema is a pure function of the contract, which forces the naming laws
to be total — defined for every legal program, with collisions failing at
startup, never at request time:

- **Object type = table name, verbatim.** Table `post` is type `post`.
- **`Query.<t>`** is the list, **`Query.<t>_by_pk`** the single row — a read
  miss is `null`.
- **Forward relationship = the reference field's own name.** `post.author`
  resolves to a `user` object; the raw key is one hop away (`author { id }`).
- **Reverse relationship = `<child>_by_<field>`.** `user.post_by_author`
  resolves to a collection. No pluralization — English inflection is not a
  total function, and two references from one child stay distinct.

`where`, `order_by`, and `offset` land on every list root and reverse
collection, and lists share REST's page discipline (default 50, ceiling 200):

```graphql
{
  post(where: { status: { _eq: "published" } }, order_by: [{ title: asc }]) {
    title slug author { username }
  }
  user_by_pk(id: "11111111-0000-7000-8000-000000000001") {
    username post_by_author { title status }
  }
}
```

```json
{
  "data": {
    "post": [
      { "title": "Hello, floor", "slug": "hello-floor",
        "author": { "username": "maya" } }
    ],
    "user_by_pk": {
      "username": "maya",
      "post_by_author": [ { "title": "Hello, floor", "status": "published" } ]
    }
  }
}
```

`POST /graphql/v1` executes the standard `{query, variables, operationName}`
body; `GET` serves GraphiQL, and introspection is on — the schema is the
contract's metadata.

## GraphQL writes

Every table derives three single-row mutations:

- `insert_<t>_one(object: <t>_insert_input!)` — insert one row, defaults
  applied on omission.
- `update_<t>_by_pk(pk_columns: ..., _set: ...)` — update one row by key.
  Key fields never appear in `_set`: keys are immutable, the key is the
  row's identity.
- `delete_<t>_by_pk(...)` — delete one row, returning it as read before
  deletion.

Update's `_set` carries the one carve-out in the language where absence and
`null` mean different things: **absent = keep, explicit `null` = clear** (or
the derived `required` error on a required field). Publishing the second
post while clearing its subtitle:

```graphql
mutation {
  update_post_by_pk(
    pk_columns: { id: "22222222-0000-7000-8000-000000000002" }
    _set: { status: "published", subtitle: null }
  ) { title subtitle status }
}
```

```json
{
  "data": {
    "update_post_by_pk": {
      "title": "Second draft", "subtitle": null, "status": "published"
    }
  }
}
```

`title` was absent from `_set`, so it kept its value; `subtitle` was an
explicit `null`, so it cleared.

A by-key write that finds no row is an error with code `not_found`, never a
silent `null` — deviation D1 in the dialect's register, because a write that
did not happen must shout. Top-level mutation fields execute serially, each
in its own transaction: an earlier field's committed write survives a later
field's error.

## Derived errors

Every constraint in the base program minted an error code at compile time.
The kinds, their code templates, and their REST statuses (v0 specification,
§6.1):

| Kind | Derived from | Code | Status |
|---|---|---|---|
| `key` | the key | `<table>_already_exists` | 409 |
| `unique` | each unique field/group | `<table>_<fields joined by _>_taken` | 409 |
| `required` | each required field that can be absent on insert (no default) or cleared on update (non-key) | `<table>_<field>_required` | 422 |
| `ref_not_found` | each reference field | `<table>_<field>_not_found` | 422 |
| `restricted` | any inbound `restrict` reference | `<table>_restricted` | 409 |
| `invalid` | each closed-set field or `check`-validated field/group | `<table>_<fields joined by _>_invalid` | 422 |

The split is semantic: a 409 (`key`, `unique`, `restricted`) is a conflict
with existing state — the same payload could succeed against a different
database. A 422 (`required`, `ref_not_found`, `invalid`) is intrinsic to the
payload and fails against any database.

One further kind is not derived from schema: `refused`, status 409 — a
refusal raised from a fn body via `spock_refuse`, its code owned by a
top-level `error` declaration. [Functions and refusals](functions.md) owns
that mechanism.

Codes are API: clients match on `post_slug_taken`, not on message strings,
and the vocabulary is frozen additively for v0.x alongside the contract
shape. Every code is visible before any request is made — in the contract's
per-table `errors` array:

```json
[
  { "code": "post_already_exists",   "kind": "key",           "fields": ["id"],     "status": 409 },
  { "code": "post_slug_taken",       "kind": "unique",        "fields": ["slug"],   "status": 409 },
  { "code": "post_title_required",   "kind": "required",      "fields": ["title"],  "status": 422 },
  { "code": "post_slug_required",    "kind": "required",      "fields": ["slug"],   "status": 422 },
  { "code": "post_status_required",  "kind": "required",      "fields": ["status"], "status": 422 },
  { "code": "post_author_required",  "kind": "required",      "fields": ["author"], "status": 422 },
  { "code": "post_author_not_found", "kind": "ref_not_found", "fields": ["author"], "status": 422 },
  { "code": "post_status_invalid",   "kind": "invalid",       "fields": ["status"], "status": 422 }
]
```

and in each generated mutation's description, so an agent introspecting the
schema learns the failure surface without triggering it:

```text
insert_post_one: Insert one post row. Errors: post_already_exists,
post_slug_taken, post_title_required, post_slug_required,
post_author_required, post_author_not_found, post_status_invalid.
```

In flight, a derived error carries its code as machine data. Inserting a
second post with the seeded slug:

```json
{
  "data": null,
  "errors": [
    {
      "message": "post.slug is already taken",
      "locations": [{ "line": 1, "column": 12 }],
      "extensions": { "code": "post_slug_taken", "fields": ["slug"],
                      "kind": "unique", "table": "post" }
    }
  ]
}
```

The full code tables, including the reserved codes and compile-time
diagnostics, live in the [errors reference](../reference/errors.md).

## Two planes, one envelope each

The same failure wears a different envelope per plane, with the same machine
data inside. REST puts the HTTP status to work and wraps the error (v0
specification, §8.1) — reading a post that does not exist answers `404`
with:

```json
{
  "error": { "code": "not_found", "kind": "not_found", "table": null,
             "fields": [], "message": "no post row with this key" }
}
```

GraphQL answers HTTP 200 and carries the identical payload in
`errors[].extensions`, with `status` dropped because the transport no longer
speaks it (v0 specification, §8.2) — deleting that same missing post:

```json
{
  "data": null,
  "errors": [
    {
      "message": "no post row with this key",
      "locations": [{ "line": 1, "column": 12 }],
      "extensions": { "code": "not_found", "fields": [],
                      "kind": "not_found", "table": null }
    }
  ]
}
```

Match on `extensions.code` over GraphQL and `error.code` over REST; the
vocabulary is one and the same.

## Not derived yet

Bulk writes, `on_conflict` upserts, aggregates, and subscriptions are not
part of the derived surface today; [Project status](../status.md) tracks
each of them.
