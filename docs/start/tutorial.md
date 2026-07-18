---
description: Build a mini-Instagram backend — users, posts, likes, comments — from an empty project to a contract with a named refusal, one verified step at a time.
order: 3
---

# Tutorial: build a mini-Instagram backend

This tutorial builds **minigram**, a photo feed in miniature: users, posts,
likes, and comments. Every step ends with a program you can check, and most
end with a request you can send. Along the way you will meet the ideas the
rest of this site keeps returning to — the anchor, seed replay, the floor and
the deliberate surface, derived errors, and a refusal you write yourself.

You need the toolchain from [Install](install.md). Everything else starts from
an empty directory.

## Step 1 — Create the project

`spock new` scaffolds a framework project; `--backend-only` skips the Uhura
client, because this tutorial is about the authority.

```sh
$ spock new minigram --backend-only
created backend-only project `minigram` at /work/minigram
next: run `spock dev` from the project directory above
```

The topology is two files:

```text
minigram/
├── backend/
│   └── app.spock
└── spock.toml
```

`spock.toml` is the manifest that makes this directory a framework project,
served as a unit by `spock dev` — [spock.toml](../reference/spock-toml.md)
documents it, and [CLI](../reference/cli.md) covers the commands. (A single
`.spock` file with no manifest is the other shape, a standalone program;
everything this page teaches applies to both.) `backend/app.spock` is the one
source file you will grow for the rest of this page. An empty backend is
already a valid program:

```sh
$ cd minigram
$ spock check .
ok: project `minigram` — 0 table(s), 0 record(s), 0 fn(s), 0 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

`spock check` is the verification loop you will run after every step. It does
far more than parse: it materializes the schema in an in-memory engine,
validates every function body, and replays the seed through the runtime write
path — everything the server would reject at startup surfaces here, with no
server running.

## Step 2 — The anchor

Every identity-bearing program declares exactly one `auth table` — the
**anchor**. Its key is the actor value: the thing `= me` stamps and
`spock_actor()` returns. For minigram the anchor is `user`. Replace the
contents of `backend/app.spock`:

```spock
//! minigram — a photo feed in miniature.

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}
```

One table, three decisions. `key id: uuid = auto` gives every user an
engine-minted UUIDv7 identity. `username: text unique` makes the handle a
uniqueness constraint the whole surface will enforce — you will meet the error
it derives in step 9. `display_name: text?` is optional: absent until set.
The field syntax — types, keys, `unique`, defaults — is
[Tables](../language/tables.md)' territory.

The `///` lines are doc comments, and they are not decoration: they travel
into the compiled contract, and from there into GraphiQL's schema docs and
every generated artifact. Documenting the program is documenting the API,
so this tutorial keeps the habit from the first table.

```sh
$ spock check .
ok: project `minigram` — 1 table(s), 0 record(s), 0 fn(s), 0 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

## Step 3 — Seed the personas

A backend with no rows is hard to poke at. Seed three **personas** — seeded
actor rows you will impersonate for the rest of the tutorial. Their ids are
fixed literals rather than `auto` values so every curl command on this page
can hardcode who is calling.

```spock
//! minigram — a photo feed in miniature.

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }
}
```

The `maya = user { ... }` form is a binding: later seed rows can name
`maya` wherever a reference to `user` is expected, and step 4 will.

Seed is not a SQL dump and not a migration — there are no migrations. On
every start the database is rebuilt from source and the seed replayed through
the ordinary write path, the same validation every API write goes through. A
seed row that violates the contract fails `check`. That makes the seed your
first test suite: every constraint this tutorial adds is exercised against
these rows on every single start. [Seed](../language/seed.md) owns the full
semantics.

```sh
$ spock check .
ok: project `minigram` — 1 table(s), 0 record(s), 0 fn(s), 3 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

Now start the development server and meet the personas:

```sh
$ spock dev .
warning: backend inputs (including referenced seed assets) and spock.toml topology changes are observed but not applied; restart `spock dev` to reconstruct backend state from seed
listening on http://127.0.0.1:4000
  GET  /~studio              Spock Studio
  GET  /~contract            active Spock contract
  GET  /~project/status      framework generation status
  GET  /~health              aggregate readiness
  *    /rest/v1/*            authority REST and RPC
  POST /graphql/v1           GraphQL when the contract is non-empty
```

Keep that warning in mind — it is the reload rule, and step 8 collects on
it. In another terminal:

```sh
$ curl -sS http://127.0.0.1:4000/~personas
[
  { "actor": "10000000-0000-4000-8000-000000000001", "label": "maya" },
  { "actor": "10000000-0000-4000-8000-000000000002", "label": "luis" },
  { "actor": "10000000-0000-4000-8000-000000000003", "label": "noor" }
]
```

`/~personas` lists the anchor's seed rows as impersonation candidates — the
development surface behind Studio's actor picker.

## Step 4 — Posts, and the actor stamp

A post is a caption by an author, and the author field carries the tutorial's
first piece of actor wiring. In the `post` table below, `author: user = me`
means: when a request inserts a post, the runtime stamps `author` with the
request's actor. A client cannot forge authorship, because `author` is not
even accepted as input on the floor's insert.

Add the table, then seed a post the obvious way — and watch it fail:

```spock check=fail:E022
//! minigram — a photo feed in miniature.

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    caption: "first light over the bay",
  }
}
```

```sh
$ spock check .
error: backend: SPH003: .../minigram/backend/app.spock: error[E022]: seed row for `post` omits required field `author` (no default)
```

This diagnostic is the actor seam teaching its most important lesson: **`= me`
is a runtime stamp, not a stored default.** There is no value sitting in the
schema waiting to fill `author`; the value arrives with the request, and seed
replay is actorless — nobody is signed in while the database is being rebuilt.
So a seed row must name its author explicitly:

```spock
//! minigram — a photo feed in miniature.

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    author: maya,
    caption: "first light over the bay",
  }
}
```

```sh
$ spock check .
ok: project `minigram` — 2 table(s), 0 record(s), 0 fn(s), 4 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

The post's id is a fixed literal for the same reason the personas' are:
step 13 wants to like it from the command line. The whole actor story —
`X-Spock-Actor`, `spock_actor()`, `= me` — lives in
[Actor](../language/actor.md).

## Step 5 — Likes: identity as a pair

A like has no id of its own. Its identity *is* the pair — this post, this
user — and the key can say exactly that: `key (post, user)`, a composite key
over two reference fields, with no surrogate id anywhere.

The composite key makes liking **idempotent by construction**: there is
nowhere for a second like by the same user to go. You do not write a guard,
a query, or an upsert — the second insert is a derived error, and step 14
triggers it. `on delete cascade` decides the other lifecycle question:
deleting a post takes its likes with it.

The program so far:

```spock
//! minigram — a photo feed in miniature.

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

/// A like on a post. The (post, user) pair is the key, so liking is
/// idempotent by construction.
table like {
  key (post, user)
  /// The liked post; deleting the post takes its likes with it.
  post: post on delete cascade
  /// The user who liked it, stamped from the request actor.
  user: user = me
  at: timestamp = now
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    author: maya,
    caption: "first light over the bay",
  }

  like { post: sunrise, user: noor }
}
```

```sh
$ spock check .
ok: project `minigram` — 3 table(s), 0 record(s), 0 fn(s), 5 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

The seeded like names `user: noor` explicitly — the same actorless-seed rule
as step 4, now routine.

## Step 6 — Comments, and a value rule

Comments introduce the last table and the first function. The product rule —
a comment body is non-empty and at most 2200 characters — is written once, as
a validator: an ordinary read `fn` returning `bool`, whose body is one
clauseless boolean `SELECT`. Attaching it to a field with `check valid_body`
turns it into a named constraint the entire surface obeys — the floor,
functions, and seed replay alike.

The comment table also references itself: `parent: comment?` makes a comment
optionally a reply. `on delete set null` keeps replies alive when their
parent dies, with the link cleared.

```spock
//! minigram — a photo feed in miniature.

/// A comment body is non-empty after trimming and at most 2200 characters.
fn valid_body(
  /// The candidate comment body.
  body: text,
) -> bool {
  unchecked sql("""SELECT length(trim(:body)) >= 1 AND length(:body) <= 2200""")
}

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

/// A like on a post. The (post, user) pair is the key, so liking is
/// idempotent by construction.
table like {
  key (post, user)
  /// The liked post; deleting the post takes its likes with it.
  post: post on delete cascade
  /// The user who liked it, stamped from the request actor.
  user: user = me
  at: timestamp = now
}

/// A comment on a post, optionally a reply to another comment.
table comment {
  key id: uuid = auto
  post: post on delete cascade
  /// The comment this one replies to, if any. Deleting the parent keeps
  /// the reply and nulls this link.
  parent: comment? on delete set null
  author: user = me
  /// The comment text — non-empty, at most 2200 chars (`valid_body`).
  body: text check valid_body
  at: timestamp = now
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    author: maya,
    caption: "first light over the bay",
  }

  like { post: sunrise, user: noor }

  first = comment { post: sunrise, author: luis, body: "great shot" }
  comment { post: sunrise, author: noor, body: "which beach?", parent: first }
}
```

```sh
$ spock check .
ok: project `minigram` — 4 table(s), 0 record(s), 1 fn(s), 7 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

Both seeded comments pass `valid_body` on every start, because seed replay
goes through the same write path — the first test suite is growing with the
program.

## Step 7 — Read the contract

Four tables and a validator compile to a **contract**: one JSON document that
names every operation the backend serves and every way it can fail. The
server exposes it verbatim:

```sh
$ curl -sS http://127.0.0.1:4000/~contract
```

Skim the `errors` array on each table. Nobody wrote these; each is a
**derived error**, minted from a constraint you declared, with a stable code
clients are expected to match on:

| Code | Derived from | Kind |
| --- | --- | --- |
| `user_username_taken` | `username: text unique` | `unique` |
| `like_already_exists` | `key (post, user)` | `key` |
| `comment_body_invalid` | `body: text check valid_body` | `invalid` |
| `comment_parent_not_found` | `parent: comment?` | `ref_not_found` |
| `post_author_required` | `author: user = me` | `required` |

This is the deal the rest of the tutorial cashes in: declare the constraint
once, and the failure vocabulary is part of the contract before any request
is made. Error codes are API — clients match on them, and the derivation
templates are frozen for v0.x, so `user_username_taken` means the same thing
in every Spock program ever compiled. The contract also carries every table,
field, function signature, and doc comment; it is the artifact code
generators and clients bind to. [Derived API](../language/derived-api.md)
walks the whole surface; [Errors](../reference/errors.md) tabulates every
code.

## Step 8 — Browse the floor in GraphiQL

You have edited the backend while `spock dev` was running, so the server is
still serving the step-3 generation. The reload rule, once: client edits
publish live, but backend edits are observed and reported as
`restart_required` (visible at `/~project/status`) — restarting `spock dev`
rebuilds the database from source and replays the seed. State in v0 is
disposable by doctrine, so a restart is never a loss; it is a reset to a
known world. Restart it:

```sh
$ spock dev .
listening on http://127.0.0.1:4000
```

Open [http://127.0.0.1:4000/graphql/v1](http://127.0.0.1:4000/graphql/v1) in
a browser and GraphiQL loads, schema docs included. Every table received its
derived read and write fields — **the floor**: `user` and `user_by_pk`,
`insert_post_one`, `update_comment_by_pk`, and so on, with your doc comments
as their descriptions and each mutation's description listing the derived
error codes it can produce. Browse `Query.user`, then ask for a post with
its relationships; references walk forward (`author`) and backward
(`comment_by_post`) without any resolver code:

```graphql
{
  post_by_pk(id: "20000000-0000-4000-8000-000000000001") {
    caption
    author { username }
    comment_by_post { body author { username } }
  }
}
```

```json
{
  "data": {
    "post_by_pk": {
      "caption": "first light over the bay",
      "author": { "username": "maya" },
      "comment_by_post": [
        { "body": "great shot", "author": { "username": "luis" } },
        { "body": "which beach?", "author": { "username": "noor" } }
      ]
    }
  }
}
```

Writes are on the floor too. Add the header `X-Spock-Actor:
10000000-0000-4000-8000-000000000002` in GraphiQL's Headers pane — you are
luis now — and post something. Watch `= me` do its work: `author` is not in
the input object, yet the response names luis.

```graphql
mutation {
  insert_post_one(object: { caption: "harbor at noon" }) {
    id
    caption
    author { username }
  }
}
```

```json
{
  "data": {
    "insert_post_one": {
      "id": "019f751e-a49c-71d0-bad2-862c0ee5cfb4",
      "caption": "harbor at noon",
      "author": { "username": "luis" }
    }
  }
}
```

## Step 9 — Trigger a derived error

In step 2 you declared `username: text unique`. Claim a taken handle and the
constraint answers with its derived code:

```graphql
mutation {
  insert_user_one(object: { username: "maya" }) { id username }
}
```

```json
{
  "data": null,
  "errors": [
    {
      "message": "user.username is already taken",
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

The code in `extensions` is the same `user_username_taken` you read in the
contract at step 7 — the failure was public API before the request existed.

## Step 10 — Trigger a value rule

Now the validator from step 6. Comment as luis with a whitespace body:

```graphql
mutation {
  insert_comment_one(
    object: { post: "20000000-0000-4000-8000-000000000001", body: "   " }
  ) { id }
}
```

```json
{
  "data": null,
  "errors": [
    {
      "message": "`comment.body` failed check `valid_body`",
      "extensions": {
        "code": "comment_body_invalid",
        "kind": "invalid",
        "table": "comment",
        "fields": ["body"]
      }
    }
  ]
}
```

Over REST these two failures wear different statuses: a `unique` violation is
409, because it conflicts with existing state and the same payload could
succeed against a different database, while an `invalid` value is 422,
because the payload is wrong in itself. GraphQL stays HTTP 200 and carries
the same envelope in `extensions` — the code is the contract, the status is
the transport.

## Step 11 — The feed

The floor serves rows; a product serves a *feed*. `fn feed() -> [post]` in
the listing below is the first entry in the **deliberate surface** — a
declared function whose body is a SQL **escape**, carried verbatim in the
contract and marked `unchecked` at the declaration site.

The signature is the checked half: `fn` (not `mut fn`) declares read
**polarity**, which the engine enforces at load — a statement that could
write aborts startup — and the surface reflects: `feed` becomes a `Query`
root field and answers `GET /rest/v1/rpc/feed`, while a `mut fn` lives on
the `Mutation` root and refuses `GET` outright. The `-> [post]` return shape
is validated against the statement's result columns at load, again with no
server running. Only the SQL between the quotes is the escape: yours to own,
honestly labeled, and counted. [Functions](../language/functions.md) owns
the full story.

```spock
//! minigram — a photo feed in miniature.

/// A comment body is non-empty after trimming and at most 2200 characters.
fn valid_body(
  /// The candidate comment body.
  body: text,
) -> bool {
  unchecked sql("""SELECT length(trim(:body)) >= 1 AND length(:body) <= 2200""")
}

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

/// A like on a post. The (post, user) pair is the key, so liking is
/// idempotent by construction.
table like {
  key (post, user)
  /// The liked post; deleting the post takes its likes with it.
  post: post on delete cascade
  /// The user who liked it, stamped from the request actor.
  user: user = me
  at: timestamp = now
}

/// A comment on a post, optionally a reply to another comment.
table comment {
  key id: uuid = auto
  post: post on delete cascade
  /// The comment this one replies to, if any. Deleting the parent keeps
  /// the reply and nulls this link.
  parent: comment? on delete set null
  author: user = me
  /// The comment text — non-empty, at most 2200 chars (`valid_body`).
  body: text check valid_body
  at: timestamp = now
}

/// The newest twenty posts, newest first.
fn feed() -> [post] {
  unchecked sql("""
    SELECT * FROM post
    ORDER BY at DESC, id DESC
    LIMIT 20
  """)
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    author: maya,
    caption: "first light over the bay",
  }

  like { post: sunrise, user: noor }

  first = comment { post: sunrise, author: luis, body: "great shot" }
  comment { post: sunrise, author: noor, body: "which beach?", parent: first }
}
```

```sh
$ spock check .
ok: project `minigram` — 4 table(s), 0 record(s), 2 fn(s), 7 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

Restart `spock dev` and call it both ways:

```sh
$ curl -sS http://127.0.0.1:4000/rest/v1/rpc/feed
{"rows":[{"id":"20000000-0000-4000-8000-000000000001","author":"10000000-0000-4000-8000-000000000001","caption":"first light over the bay","at":"2026-07-18T12:07:12.835204Z"}]}
```

```graphql
{ feed { caption author { username } } }
```

```json
{
  "data": {
    "feed": [
      {
        "caption": "first light over the bay",
        "author": { "username": "maya" }
      }
    ]
  }
}
```

One row. Luis's "harbor at noon" from step 8 is gone — the restart rebuilt
the world from seed, which is disposable state doing exactly what it
promises: the program and its seed are the whole truth, so every start is
reproducible.

## Step 12 — A refusal

One rule the schema cannot derive: you may not like your own post. That is
not a uniqueness fact or a value shape — it is a product decision, and
product decisions get named.

> **Experimental — RFD 0024 implementation preview.** Top-level `error`
> declarations ship in toolchain 0.5.2 and later as an implementation
> preview. [RFD 0024](../rfd/0024-error-declarations.md) remains a draft;
> this syntax is not yet part of the normative v0 specification.

A top-level `error` declaration mints a **product error**; a `mut fn` lists
it after `!` and raises it with `spock_refuse` — a **refusal**. The guard
shape is a conditional selection: when the `WHERE` clause selects no rows,
`spock_refuse` is never evaluated and the body falls through to its insert.
Here is the complete program, refusal included:

```spock
//! minigram — a photo feed in miniature.

/// A liked post must belong to someone else.
error cannot_like_own_post

/// A comment body is non-empty after trimming and at most 2200 characters.
fn valid_body(
  /// The candidate comment body.
  body: text,
) -> bool {
  unchecked sql("""SELECT length(trim(:body)) >= 1 AND length(:body) <= 2200""")
}

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

/// A like on a post. The (post, user) pair is the key, so liking is
/// idempotent by construction.
table like {
  key (post, user)
  /// The liked post; deleting the post takes its likes with it.
  post: post on delete cascade
  /// The user who liked it, stamped from the request actor.
  user: user = me
  at: timestamp = now
}

/// A comment on a post, optionally a reply to another comment.
table comment {
  key id: uuid = auto
  post: post on delete cascade
  /// The comment this one replies to, if any. Deleting the parent keeps
  /// the reply and nulls this link.
  parent: comment? on delete set null
  author: user = me
  /// The comment text — non-empty, at most 2200 chars (`valid_body`).
  body: text check valid_body
  at: timestamp = now
}

/// The newest twenty posts, newest first.
fn feed() -> [post] {
  unchecked sql("""
    SELECT * FROM post
    ORDER BY at DESC, id DESC
    LIMIT 20
  """)
}

/// Like a post as the signed-in user. Refuses your own post.
mut fn like_post(
  /// The post to like.
  post: post,
) -> like ! cannot_like_own_post {
  unchecked sql("""
    SELECT spock_refuse('cannot_like_own_post')
    WHERE (SELECT author FROM post WHERE id = :post) = spock_actor()
  """)
  unchecked sql("""
    INSERT INTO "like" (post, user)
    VALUES (:post, spock_actor())
    RETURNING *
  """)
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    author: maya,
    caption: "first light over the bay",
  }

  like { post: sunrise, user: noor }

  first = comment { post: sunrise, author: luis, body: "great shot" }
  comment { post: sunrise, author: noor, body: "which beach?", parent: first }
}
```

```sh
$ spock check .
ok: project `minigram` — 4 table(s), 0 record(s), 3 fn(s), 7 seed row(s), backend only, 0 unchecked link(s), 0 warning(s)
```

Read `like_post` closely. It takes no `user` parameter — the liker is
`spock_actor()`, the request's identity, on both the guard and the insert.
The two statements run in order inside one transaction; a raised refusal
rolls everything back. And the final `INSERT ... RETURNING *` answers the
call with the authority's own row, echo included, because the last statement
of a body is always the return value.

The routing is strict in both directions. `spock_refuse` may only raise a
code this fn lists after `!`, and a declared product error is the only kind
of code it may raise at all — derived errors like `like_already_exists` can
only ever be produced by their own constraints, which is what keeps them
trustworthy as evidence. The contract now carries all three vocabularies at
once: derived errors from the schema, one product error from you, and the
reserved protocol codes underneath.

## Step 13 — Impersonate

Restart `spock dev` one more time. The **actor seam** is the header
`X-Spock-Actor`: whatever key it carries is the actor for that request, and
an absent or unparseable header is anonymous — never an error. Ask the
server who you are:

```sh
$ curl -sS http://127.0.0.1:4000/~whoami
{"actor":null,"anonymous":true,"known":false}

$ curl -sS -H 'X-Spock-Actor: 10000000-0000-4000-8000-000000000002' \
    http://127.0.0.1:4000/~whoami
{"actor":"10000000-0000-4000-8000-000000000002","anonymous":false,"known":true}
```

Now be maya, and like maya's own post:

```sh
$ curl -sS -i -X POST http://127.0.0.1:4000/rest/v1/rpc/like_post \
    -H 'Content-Type: application/json' \
    -H 'X-Spock-Actor: 10000000-0000-4000-8000-000000000001' \
    -d '{"post":"20000000-0000-4000-8000-000000000001"}'
HTTP/1.1 409 Conflict
{"error":{"code":"cannot_like_own_post","kind":"refused","table":null,"fields":[],"message":"fn `like_post` refused: cannot_like_own_post"}}
```

The refusal you declared in step 12, on the wire: kind `refused`, status 409,
your code verbatim. Be luis instead and the same request succeeds:

```sh
$ curl -sS -X POST http://127.0.0.1:4000/rest/v1/rpc/like_post \
    -H 'Content-Type: application/json' \
    -H 'X-Spock-Actor: 10000000-0000-4000-8000-000000000002' \
    -d '{"post":"20000000-0000-4000-8000-000000000001"}'
{"post":"20000000-0000-4000-8000-000000000001","user":"10000000-0000-4000-8000-000000000002","at":"2026-07-18T12:07:14.949129Z"}
```

Anonymity has an answer too. Drop the header entirely and `spock_actor()` is
NULL, so the insert lands nothing where `like.user` is required — and that
failure routes to the same derived `required` error the contract already
listed, because an unauthenticated identity is never stored:

```sh
$ curl -sS -i -X POST http://127.0.0.1:4000/rest/v1/rpc/like_post \
    -H 'Content-Type: application/json' \
    -d '{"post":"20000000-0000-4000-8000-000000000001"}'
HTTP/1.1 422 Unprocessable Entity
{"error":{"code":"like_user_required","kind":"required","table":"like","fields":["user"],"message":"like.user is required"}}
```

The header is deliberately forgeable — v0's identity seam is unverified by
design, and [Project status](../status.md) carries that whole disclaimer.

## Step 14 — Like it again

Send luis's like a second time:

```sh
$ curl -sS -i -X POST http://127.0.0.1:4000/rest/v1/rpc/like_post \
    -H 'Content-Type: application/json' \
    -H 'X-Spock-Actor: 10000000-0000-4000-8000-000000000002' \
    -d '{"post":"20000000-0000-4000-8000-000000000001"}'
HTTP/1.1 409 Conflict
{"error":{"code":"like_already_exists","kind":"key","table":"like","fields":["post","user"],"message":"a like with this key already exists"}}
```

`like_already_exists` — the derived error the composite key promised in
step 5, produced without a line of guard code. Idempotency here is not
something `like_post` implements; it is a guarantee the schema derives, and
the function inherits it for free.

## Step 15 — Where you are

Here is the finished `backend/app.spock`, end to end — the accumulation of
every step on this page, in one copyable listing:

```spock
//! minigram — a photo feed in miniature.

/// A liked post must belong to someone else.
error cannot_like_own_post

/// A comment body is non-empty after trimming and at most 2200 characters.
fn valid_body(
  /// The candidate comment body.
  body: text,
) -> bool {
  unchecked sql("""SELECT length(trim(:body)) >= 1 AND length(:body) <= 2200""")
}

/// A person on the network. This is the identity anchor: `user.id` is the
/// actor value the runtime stamps and returns.
auth table user {
  key id: uuid = auto
  /// The handle — shown everywhere, unique across the network.
  username: text unique
  /// Display name; absent until the user sets one.
  display_name: text?
}

/// A post: a caption by an author, stamped at publish time.
table post {
  key id: uuid = auto
  /// The author, stamped from the request actor (`= me`).
  author: user = me
  caption: text
  at: timestamp = now
}

/// A like on a post. The (post, user) pair is the key, so liking is
/// idempotent by construction.
table like {
  key (post, user)
  /// The liked post; deleting the post takes its likes with it.
  post: post on delete cascade
  /// The user who liked it, stamped from the request actor.
  user: user = me
  at: timestamp = now
}

/// A comment on a post, optionally a reply to another comment.
table comment {
  key id: uuid = auto
  post: post on delete cascade
  /// The comment this one replies to, if any. Deleting the parent keeps
  /// the reply and nulls this link.
  parent: comment? on delete set null
  author: user = me
  /// The comment text — non-empty, at most 2200 chars (`valid_body`).
  body: text check valid_body
  at: timestamp = now
}

/// The newest twenty posts, newest first.
fn feed() -> [post] {
  unchecked sql("""
    SELECT * FROM post
    ORDER BY at DESC, id DESC
    LIMIT 20
  """)
}

/// Like a post as the signed-in user. Refuses your own post.
mut fn like_post(
  /// The post to like.
  post: post,
) -> like ! cannot_like_own_post {
  unchecked sql("""
    SELECT spock_refuse('cannot_like_own_post')
    WHERE (SELECT author FROM post WHERE id = :post) = spock_actor()
  """)
  unchecked sql("""
    INSERT INTO "like" (post, user)
    VALUES (:post, spock_actor())
    RETURNING *
  """)
}

seed {
  maya = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "maya",
    display_name: "Maya Chen",
  }
  luis = user {
    id: "10000000-0000-4000-8000-000000000002",
    username: "luis",
  }
  noor = user {
    id: "10000000-0000-4000-8000-000000000003",
    username: "noor",
    display_name: "Noor Haddad",
  }

  sunrise = post {
    id: "20000000-0000-4000-8000-000000000001",
    author: maya,
    caption: "first light over the bay",
  }

  like { post: sunrise, user: noor }

  first = comment { post: sunrise, author: luis, body: "great shot" }
  comment { post: sunrise, author: noor, body: "which beach?", parent: first }
}
```

About a hundred lines. In them: an anchor, three more tables, a reusable
value rule, a derived surface with a documented failure vocabulary, a
deliberate read, and a guarded mutation that refuses by name — plus a seed
that re-proves all of it on every start.

Run the escape ledger directly against the source file:

```sh
$ spock check backend/app.spock
ok: 4 table(s), 0 record(s), 3 fn(s) (4 unchecked escapes), 7 seed row(s)
```

Four escapes — one in `valid_body`, one in `feed`, two in `like_post` — is
the exact count of SQL statements the language did not verify. Everything
else on this page sits inside the checked envelope.

Where to go next:

- The [full Instagram authority](../../examples/instagram/v0.spock) is this
  tutorial at product scale — the follow graph, blocks and restrictions,
  media carousels, collections, notifications — in one program with the same
  vocabulary.
- The [full-stack Instagram example](https://github.com/gridaco/uhura/tree/main/examples/instagram)
  adds an Uhura client in front of the same backend; it needs a one-time
  `pnpm` provider build that the CLI does not perform, and its README walks
  through it.
- [Tables](../language/tables.md), [Functions](../language/functions.md),
  [Seed](../language/seed.md), and [Actor](../language/actor.md) each own the
  full depth of what this page used in passing, and the
  [CLI](../reference/cli.md) and [HTTP](../reference/http.md) references
  cover every command and endpoint.
