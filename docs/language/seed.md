---
description: How seed blocks populate a program's world through its own contract, and why replay — not migration — is the state life cycle.
order: 4
---

# Seed and disposable state

A Spock program declares its world and populates it in the same file, under the
same rules. A `seed` block is a list of rows written in source; at startup the
runtime inserts them through the program's own contract, exactly as a client
would. There is no side door — no SQL dump, no fixtures directory, no loader
that bypasses validation. If the seed states something the contract forbids,
the program does not start.

That loop is the whole life cycle. The database is rebuilt from source and seed
on every load — [disposable state is doctrine](../status.md) — so seed replay
is the program's test suite, and there are no migrations. Changing the schema
means changing the source and reloading; the world is rebuilt to match, every
time, from the one artifact you version-control.

## Syntax

A `seed` block appears at the top level of the file. A program may declare
several; they concatenate in source order into one seed, and statements execute
in that order. Each statement names a table and provides fields:

```spock
table author {
  key id: uuid = auto
  name: text unique
}

table book {
  key id: uuid = auto
  author: author on delete cascade
  title: text
  status: "draft" | "published" = "draft"
}

table review {
  key id: uuid = auto
  book: book on delete cascade
  body: text
  rating: int
}

seed {
  ursula = author { name: "ursula" }
  tove = author { name: "tove" }

  dispossessed = book { author: ursula, title: "The Dispossessed", status: "published" }
  book { author: tove, title: "The Summer Book" }

  review { book: dispossessed, body: "still thinking about it", rating: 5 }
}
```

The `name = table { ... }` form is a **binding**: it captures the inserted
row's key so later statements can fill [reference fields](tables.md) with it —
`author: ursula` stores ursula's generated `id`. Bindings share one namespace
across every seed block in the file, and a binding must be defined before it is
used, because statements execute in order and a reference can only point at a
row that exists:

```spock check=fail:E024
table author {
  key id: uuid = auto
  name: text unique
}

table book {
  key id: uuid = auto
  author: author
  title: text
}

seed {
  book { author: ursula, title: "The Left Hand of Darkness" }
  ursula = author { name: "ursula" }
}
```

A binding fills references to the bound row's table and nothing else: pointing
it at a reference with a different target is E025, using it as the value of a
non-reference field is E026, and reusing a binding name is E027. The
[errors reference](../reference/errors.md) lists the full E02x family.

## Every seed row is a real write

Seed rows pass the same validation as writes arriving over the derived API
(v0 specification, §7.3): unknown fields are rejected, values are type- and
format-checked, closed-set membership is enforced, defaults apply, references
are checked for existence, and unique and key constraints hold. A seed that
violates the contract aborts startup with the [derived error](derived-api.md)
— `author_name_taken`, say — and the offending seed statement's source
location.

You never have to start a server to find out. `spock check` is the full load
proof: it materializes the schema in an in-memory engine and replays the seed,
so a duplicate username or a dangling reference fails the check on your desk,
not the deploy. This is what makes seed replay a test suite — every load
re-proves that the world you described is a world the contract accepts.

## Seed is actorless

Seed runs at startup, before any request, so there is no actor. A field
defaulted `= me` normally stamps the current actor at insert time; in seed
there is nobody to stamp, so every required `= me` field must be named
explicitly in the seed row, and omitting one is E022. The
[tutorial](../start/tutorial.md) walks into this rule the first time a seeded
table carries `= me`; the default itself, and the whole identity seam, belong
to [the actor page](actor.md).

## Seed cannot call functions

A seed statement inserts a row; it cannot invoke a `fn` or `mut fn`. Functions
execute with an actor and may refuse — they are operations, and seed is a
statement of facts. When you want seeded state that a function would normally
produce, state the resulting rows directly: the
[instagram example](../examples.md) seeds notifications alongside the likes
that would have generated them.

## Seed assets

A field that references the builtin `storage_object` table can be seeded from
a real file. Given this asset next to the program:

```json path=welcome.json
{ "tagline": "a program and its assets are one bundle" }
```

```spock
table document {
  key id: uuid = auto
  title: text
  attachment: storage_object
}

seed {
  document { title: "welcome", attachment: file("./welcome.json") }
}
```

The path is relative to the `.spock` file and must stay inside its directory —
an absolute or escaping path is E050, and `file()` on a field that does not
reference `storage_object` is E049. At load the runtime reads the bytes, stores
them, and materializes a committed `storage_object` row — name, content type,
size, checksum — whose id the field takes; the bytes are then served over the
[storage plane](../reference/http.md) like any upload.

The program plus its assets are one bundle. The source names the files, the
files ride alongside the source, and a missing asset fails replay — at
`spock check` too, not only at start.

## The personas pattern

An anchor row seeded with a fixed key is a **persona**: a known identity to
impersonate during development. Write the ids as literals rather than leaving
them to `= auto`, because `auto` mints a fresh UUID on every replay and every
restart is a replay — a persona with an auto id is a moving target for saved
requests and client code, while a fixed id survives every restart:

```spock
auth table user {
  key id: uuid = auto
  username: text unique
}

table note {
  key id: uuid = auto
  owner: user = me
  body: text
}

seed {
  maya = user { id: "10000000-0000-4000-8000-000000000001", username: "maya" }
  luis = user { id: "10000000-0000-4000-8000-000000000002", username: "luis" }

  note { owner: maya, body: "welcome" }
  note { owner: luis, body: "second voice" }
}
```

`note.owner` is `= me`, so each seeded note names its owner explicitly — the
actorless rule above in practice. A running server lists the anchor table's
rows as a picker:

```sh
curl -s http://127.0.0.1:4000/~personas
```

```json
[
  { "actor": "10000000-0000-4000-8000-000000000001", "label": "maya" },
  { "actor": "10000000-0000-4000-8000-000000000002", "label": "luis" }
]
```

Each entry's `actor` is verbatim what goes in the `X-Spock-Actor` header.
Impersonation itself — the header, the persona labels, `spock_actor()`, and
why the seam is deliberately unverified — is [the actor page](actor.md).
