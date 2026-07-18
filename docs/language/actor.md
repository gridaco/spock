---
description: How Spock v0 carries identity — the auth table anchor, the X-Spock-Actor header, spock_actor(), = me, and the impersonation workflow.
order: 5
---

# Identity: the actor seam

Every request to a Spock program may carry an identity, and the language gives
that identity exactly one path through the system: the actor seam. A program
declares one `auth table` — the anchor — and the whole seam derives from that
single declaration: a request header that names an actor, an engine builtin
that reads the actor inside function bodies, and a field default that stamps
the actor onto writes. The seam's job in v0 is plumbing, not proof — it makes
viewer-relative reads and identity-stamped writes expressible today, and
leaves verifying the identity to a later version. The header is unverified by
design and v0 secures nothing — see [Project status](../status.md). The rest
of this page describes what the seam does, precisely.

## The anchor

The anchor is declared by marking one table `auth`. It is otherwise an
ordinary table — same fields, keys, and constraints as any other (see
[Tables](tables.md)) — and the compiled contract records it with the table
flag `anchor`.

```spock
auth table user {
  key id: uuid = auto
  username: text unique
  display_name: text
}

table post {
  key id: uuid = auto
  author: user = me
  body: text
  at: timestamp = now
}
```

Two rules shape the anchor. A program has at most one, because the actor
space has one identity table:

```spock check=fail:E045
auth table user {
  key id: uuid = auto
  username: text unique
}

auth table service_account {
  key id: uuid = auto
  name: text unique
}
```

And its key must be a single scalar column (E046), because the actor is one
value — the header carries it, and `spock_actor()` returns it.

Identity relationships need no special syntax: `author: user` above is an
ordinary reference to the anchor, stored, checked, and expanded like any
other reference. The one construct reserved for the anchor is the `= me`
default, covered below.

## The header

A request names its actor with one header:

```text
X-Spock-Actor: 10000000-0000-4000-8000-000000000001
```

The value is the actor's key, read verbatim — no scheme, no token format. It
is honored on `/graphql/v1` and on `/rest/v1/rpc/{fn}` calls, and the
GraphQL actor is bound per request, never process-global (v0 specification,
§8). The runtime canonicalizes the value by the anchor's key type, so an
uppercased or braced uuid still matches its row.

An absent or unparseable header is anonymous — the actor is NULL — and never
an error. Anonymity is a legitimate state on an open surface, not a fault, so
the seam degrades to it silently and lets the program's own rules decide what
anonymous callers may do.

## Where the actor flows

The seam has two consumers: `spock_actor()` in function bodies, and `= me`
on table fields.

### spock_actor() in escapes

Inside a fn body's escape, the engine builtin `spock_actor()` returns the
current actor's key, or NULL when the request is anonymous. It is
request-scoped: the runtime binds it to the request's actor before each fn
transaction opens, so every statement in the body sees the same value. It is
registered only when the program declares an anchor — a body that calls it
without one fails at load, not at request time, because identity with no
identity table is a program error.

Polarity does not gate it: read fns bind the actor exactly as `mut` fns do,
which is what makes viewer-relative queries a plain `fn`:

```spock
auth table user {
  key id: uuid = auto
  username: text unique
}

table post {
  key id: uuid = auto
  author: user = me
  body: text
}

fn my_feed() -> [post] {
  unchecked sql("""
    SELECT * FROM post
    WHERE author = spock_actor()
    ORDER BY id
  """)
}
```

Called with the header, `my_feed` answers with the caller's rows; called
anonymously, `spock_actor()` is NULL, the comparison matches nothing, and the
result is empty:

```sh
$ curl -H "X-Spock-Actor: 10000000-0000-4000-8000-000000000001" \
    localhost:4000/rest/v1/rpc/my_feed
{"rows":[{"id":"019f75…","author":"10000000-0000-4000-8000-000000000001","body":"hello"}]}

$ curl localhost:4000/rest/v1/rpc/my_feed
{"rows":[]}
```

Escapes, statement rules, and refusals belong to [Functions](functions.md).

### = me on table fields

A field defaulted `= me` — legal only on a reference to the anchor (E047) —
is stamped with the request actor when the floor inserts a row. Unlike
`auto` and `now`, this stamp is applied by the runtime on the write path and
is deliberately not a SQL `DEFAULT` clause: the actor is request-scoped
state, and the actor-blind parts of the engine must never evaluate it. An
escape that stores the actor writes `spock_actor()` itself — the same value
from the same source (v0 specification, §5.2 and §7.1).

Because the runtime owns the stamp, the field is removed from the client
write surface entirely: the derived GraphQL insert and update inputs omit it,
and so do the interfaces `spock gen types` emits. For the program above,
`post_insert_input` and the generated `post_insert` type carry `id` and
`body` — there is no `author` to send, so identity cannot be forged through
the floor's request body. What remains forgeable is the header itself; that
is the seam.

An anonymous insert of a required `= me` field fails with the field's derived
`required` error — the floor never stores an unauthenticated identity:

```graphql
mutation { insert_post_one(object: { body: "hello" }) { id } }
```

```json
{
  "data": null,
  "errors": [{
    "message": "post.author is required",
    "extensions": {
      "code": "post_author_required",
      "kind": "required",
      "table": "post",
      "fields": ["author"]
    }
  }]
}
```

Seed replay runs before any actor exists, so a seed row must name every
required `= me` column explicitly (E022) — see [Seed](seed.md).

## What the seam does not govern

The seam carries identity; it does not gate access. The floor's reads stay
open to every caller, anonymous included, and its writes are constrained only
where a field says `= me` — everything else on the derived surface (see
[The derived API](derived-api.md)) accepts any request. Per-row governance,
roles, and verified claims arrive with `policy` in a later version
([Project status](../status.md)).

## The impersonation workflow

Since any header value is honored, development identity is a matter of
picking a known row. A persona is a seeded anchor row used exactly this way,
and the runtime ships two endpoints for the loop.

`GET /~personas` lists the anchor's rows as `[{actor, label}]` — `actor` is
the row's key, verbatim what belongs in the header; `label` is the first
unique text field, or the key when there is none. A program with no anchor
answers `[]`.

```sh
$ curl localhost:4000/~personas
[{"actor":"10000000-0000-4000-8000-000000000001","label":"alice"},
 {"actor":"10000000-0000-4000-8000-000000000002","label":"bob"}]
```

`GET /~whoami` echoes the actor the server resolved from your header, which
makes it the first probe when impersonation misbehaves:

```sh
$ curl localhost:4000/~whoami
{"actor":null,"anonymous":true,"known":false}

$ curl -H "X-Spock-Actor: 10000000-0000-4000-8000-000000000001" \
    localhost:4000/~whoami
{"actor":"10000000-0000-4000-8000-000000000001","anonymous":false,"known":true}
```

The two flags separate the failure modes: `anonymous` reports whether the
header was present, and `known` reports whether the resolved key exists as a
row. A header that parses but matches no row reads `"anonymous": false,
"known": false` — and so does a header that is not the key type at all (a
username sent where the key is a uuid), which is the signal that
distinguishes a wrong value from a missing one.

The interactive surfaces build on the same pieces. GraphiQL, served at
`GET /graphql/v1`, has a headers pane — set `X-Spock-Actor` there and every
query and mutation you run carries it. Studio, at `/~studio`, goes one step
further with an actor picker fed by `/~personas`: select a persona and Studio
attaches the header for you. And in every client — curl, GraphiQL, Studio, or
your own — testing the anonymous path is one action: omit the header.

Endpoint details live in the [HTTP reference](../reference/http.md); seeding
personas is covered in [Seed](seed.md).
