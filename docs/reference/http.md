---
description: Every endpoint the runtime serves — the meta surface, REST reads and filter operators, rpc, the GraphQL binding, wire errors, and the storage plane.
order: 2
---

# HTTP API

A Spock server is one origin on one port: plain HTTP and JSON, no TLS, no
subdomains. The root namespace is protocol-owned — the `~` prefix carries the
meta surface, `/rest/v1` the REST reads and rpc, `/graphql/v1` the GraphQL
reads and writes, and `/storage/v1` the byte plane when the contract uses
storage. Because every mount is claimed by the protocol, a table name can
never collide with one, and future surfaces (an `/auth/v1`, say) claim mounts
the same way. The one exception worth knowing: the `rpc` path segment under
`/rest/v1` is protocol-owned too, so a table named `rpc` fails startup.

In a framework project, this ownership is also a client-admission rule.
Spock asks Uhura whether routes selected by the deployed machine's
`web.history` port overlap `/~*`, `/api`, `/graphql`, `/rest`, or `/storage`
and their descendants. It consumes Uhura's checked path semantics; it does not
parse source route strings. The composition check runs for `spock check`,
before `spock start` binds or loads browser assets, and for client generations
observed by `spock dev`. A rejected development edit still publishes the
current Editor graph and diagnostic while Play keeps serving the last good
client generation. Uhura remains framework-agnostic: these namespaces are
reserved by the Spock host, not by the Uhura language. Similar ordinary names
such as `/graphical`, `/restroom`, and `/storage-unit` remain application
routes.

The examples on this page assume this program, served with
`spock run api.spock`:

```spock
auth table user {
  key id: uuid = auto
  username: text unique
}

table post {
  key id: uuid = auto
  author: user = me
  title: text
  body: text?
  likes: int = 0
  status: "draft" | "published" | "archived"
  created_at: timestamp = now
}

/// This post is archived and no longer accepts likes.
error post_archived

fn top_posts(min_likes: int) -> [post] {
  unchecked sql("""
    SELECT * FROM post WHERE likes >= :min_likes
    ORDER BY likes DESC LIMIT 20
  """)
}

mut fn like_post(post: uuid) -> post ! post_archived | not_found {
  unchecked sql("""
    SELECT spock_refuse('post_archived')
    WHERE EXISTS (SELECT 1 FROM post WHERE id = :post AND status = 'archived')
  """)
  unchecked sql("""
    UPDATE post SET likes = likes + 1 WHERE id = :post RETURNING *
  """)
}

seed {
  maya = user { username: "maya" }
  arjun = user { username: "arjun" }
  post { author: maya, title: "hello", likes: 3, status: "published" }
  post { author: arjun, title: "drafting", status: "draft" }
}
```

## The meta surface

Everything under `~` is about the server rather than the data.

| Endpoint | Serves |
| --- | --- |
| `GET /~health` | Liveness. A standalone program answers `{"ok":true}`; a framework project answers an aggregate `{"ok","ready","degraded"}` and returns 503 until the generation is ready. |
| `GET /~contract` | The compiled [contract](../language/derived-api.md), verbatim — the same JSON `spock build` writes. |
| `GET /~studio` | Studio, the developer console: browse tables, impersonate personas, run fns. Served same-origin and fully offline. |
| `GET /~personas` | The seeded actor rows projected to `[{actor, label}]` — the impersonation picker. Development-only. |
| `GET /~whoami` | Echo of the current actor: `{actor, anonymous, known}`. Never rejects. Development-only. |
| `GET /~project/status` | Framework generation status. Framework projects only. |
| `GET /~project/environment` | Machine-readable environment: mode, generation ids, and the mount paths the authority serves. Framework projects only. |

The `/~project/*` routes exist under `spock start` and `spock dev` because
they describe a [framework project](../status.md)'s generations; `spock run`
has no project to describe. A framework project without a configured client
also redirects `GET /` to `/~studio`.

## The actor header

Requests to `/graphql/v1` and `/rest/v1/rpc/{fn}` may carry
`X-Spock-Actor: <key>` — the anchor row's key, verbatim and unverified.
It populates `spock_actor()` and `= me` for that request; an absent or
unparseable header is anonymous, never an error.
The [actor page](../language/actor.md) owns the seam.

## REST reads

`GET /rest/v1/{table}` lists rows as `{"rows":[...]}`, ordered by key
ascending — with `auto` UUIDv7 keys, that approximates insertion order. The
default page is 50 rows and the ceiling is 200: a larger `limit` is clamped,
a negative one is a 400. `offset` pages forward, to a depth ceiling of
10,000 rows — deep paging is a keyset cursor's job, and v0 refuses to fake
one.

`GET /rest/v1/{table}/{id}` fetches one row by key on single-key tables; on a
composite-key table the path form cannot name the row, so the request is a
400 — list with filters instead.

REST tables are read-only in v0; table writes go through
[GraphQL](#the-graphql-binding), and `POST /rest/v1/rpc/{fn}` is REST's one
deliberate write path.

### Filter operators

List endpoints accept PostgREST-shaped query parameters. This surface is
specified by [RFD 0021](../rfd/0021-filter.md); the v0 specification, §8,
predates it. Each filter is `?column=op.value`, and repeated filters AND
together:

```sh
$ curl 'localhost:4000/rest/v1/post?status=eq.published&likes=gte.2'
```

| Operator | Meaning |
| --- | --- |
| `eq` / `neq` | equals / not equals |
| `gt` `gte` `lt` `lte` | ordered comparison |
| `in` / `nin` | membership in a parenthesised list: `status=in.(draft,published)` |
| `is` | identity checks: `is.null`, `is.true`, `is.false` (`is.unknown` reads as `is.null`) |
| `ilike` | case-insensitive pattern match on text columns; `*` aliases `%` |

A `not.` prefix negates any condition — `body=not.is.null` is the spelling
for IS NOT NULL. Conditions group with `and=(...)`, `or=(...)`,
`not.and=(...)`, and `not.or=(...)`, nesting to a depth of 32; inside a
group, members are dot-joined (`or=(likes.gte.2,status.eq.draft)`).

`order=col.asc,col2.desc` sorts (`asc` is the default direction; nulls sort
last ascending, first descending), and the runtime always appends the key
columns as a tiebreak so paging never skips or duplicates rows across ties.

The vocabulary above is complete, and its edges are deliberate:

- **There is no `like`.** SQLite's `LIKE` is ASCII-case-insensitive, so a
  case-sensitive operator cannot be offered honestly; requesting `like`
  returns a 400 that points at `ilike`. `ilike` is refused on non-text
  columns rather than silently coerced.
- **The control words `order`, `limit`, `offset`, `select`, `and`, `or`,
  `not` are reserved.** A table with a column named one of these fails at
  startup, so a query key is always unambiguous. `select` is recognized and
  refused: column projection is not part of v0.
- **A closed-set operand off the set is a 422 `type_mismatch`**, not an
  empty result — a typo in an enum filter fails loudly.

Values are always bound parameters; operators map through a closed enum. A
filter string can never reach SQL as text.

## rpc

`POST /rest/v1/rpc/{fn}` calls a declared [fn](../language/functions.md) of
any polarity. Arguments arrive as a JSON object; an absent or empty body
means `{}`, so zero-parameter fns are curl-friendly. The response is a row,
`null`, or `{"rows":[...]}` per the fn's declared return shape. An unknown
fn is a 404; a non-object body is `bad_request`.

`GET /rest/v1/rpc/{fn}` is the safe-method form for read fns: arguments ride
the query string and parse by declared parameter type.

```sh
$ curl 'localhost:4000/rest/v1/rpc/top_posts?min_likes=2'
{"rows":[{"id":"019f75…","author":"019f75…","title":"hello","body":null,
          "likes":3,"status":"published","created_at":"2026-07-18T12:05:05.583184Z"}]}
```

A `mut` fn refuses GET with 405 — a safe method must not write:

```sh
$ curl -i 'localhost:4000/rest/v1/rpc/like_post?post=019f75…'
HTTP/1.1 405 Method Not Allowed

{"error":{"code":"bad_request","kind":"bad_request","table":null,"fields":[],
          "message":"fn `like_post` is `mut`; call it with POST"}}
```

## The GraphQL binding

`POST /graphql/v1` executes a standard request body —
`{"query", "variables", "operationName"}`, content type `application/json`
required. `GET /graphql/v1` serves GraphiQL in the browser. Introspection is
enabled — the schema is the contract's metadata, and each mutation's
description lists the derived-error codes it can produce. Query depth is
bounded at 32, and list fields carry the same page discipline as REST:
default 50, ceiling 200. The dialect itself — naming laws, read and write
shapes, conformance tiers — is specified in the
[GraphQL specification](../spec/graphql.md); the derived surface is toured in
[the derived API](../language/derived-api.md).

## Errors on the wire

Every REST failure is one envelope:

```json
{ "error": { "code": "post_archived", "kind": "refused", "table": null,
             "fields": [], "message": "fn `like_post` refused: post_archived" } }
```

| Status | Used for |
| --- | --- |
| 400 | malformed JSON body; by-key GET on a composite-key table; filter and paging violations |
| 404 | unknown table, unknown path, unknown fn, row not found |
| 405 | GET rpc on a `mut` fn |
| 409 | `key`, `unique`, `restricted`, and `refused` errors |
| 422 | `required`, `ref_not_found`, `invalid`, unknown field, type mismatch |
| 500 | `internal` — the body broke its contract |

GraphQL returns HTTP 200 and carries the same payload in
`errors[].extensions` as `{code, kind, table, fields}` — the status is
dropped because GraphQL's transport does not use it. The full code
vocabulary, envelope fields, and the derivation rules live on the
[error codes page](errors.md).

## The storage plane

When the contract uses the builtin `storage_object` table, the runtime
mounts a byte plane at `/storage/v1`. Files move out of band: metadata is a
`storage_object` row, bytes live in the runtime's blob store, and an object
is `pending` at mint, `committed` once its bytes land.

| Method + path | Behavior |
| --- | --- |
| `POST /storage/v1/object/upload/sign` | mint a `pending` object (`owner` = `X-Spock-Actor`, else null) and return `{id, url}` — a signed PUT URL |
| `PUT /storage/v1/object/{id}?exp&sig` | store the body as the object's bytes and flip `pending → committed` in one transaction |
| `POST /storage/v1/object/sign/{id}` | return `{url}` — a signed GET URL for a committed object |
| `GET /storage/v1/object/{id}?exp&sig` | serve the bytes with their recorded content type |

The lifecycle is sign → PUT → attach: after upload, set a `storage_object`
reference field to the object id through an ordinary write, like any other
field. Signed URLs are HMAC-SHA256 bearer tokens binding method, object id,
and expiry; the signing secret is minted per run, so a URL does not survive
a restart — and with disposable state, neither does the object. The plane
has its own reserved codes: a missing, malformed, or expired signature is a
401 `unauthorized`; a PUT to an object that is not `pending` is a 409
`conflict`; a GET of a non-committed or absent object is a 404; a body past
the size cap is a 413. Objects never uploaded or never referenced are
reclaimed by an in-process sweep. The v0 specification, §8.3, owns the
details.
