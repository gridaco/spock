---
description: How Spock functions declare typed contracts over SQL escape bodies — polarity, return shapes, records, product errors, and refusals.
order: 3
---

# Functions and refusals

Every table in a program derives [the floor](derived-api.md) — the per-table
read and write surface that exists without a line of authored code. Functions
are **the deliberate surface**: the operations you choose to add on top. A
function is a named, typed contract whose body is SQL, and declaring one adds
a callable field to both derived dialects with its name, parameters, return
shape, and failure surface visible in the contract before any request is made.

## The deliberate surface

The floor answers "rows of `t`". Anything that deserves a name of its own — a
feed, a search, a projection across tables, a guarded write — is a function:

```spock
table post {
  key id: uuid = auto
  title: text
  status: "draft" | "published" = "draft"
  published_at: timestamp?
}

fn recent_posts(count: int) -> [post] {
  unchecked sql("""
    SELECT * FROM post
    WHERE status = 'published'
    ORDER BY published_at DESC
    LIMIT :count
  """)
}
```

`recent_posts` becomes a `Query` field on the GraphQL surface and
`/rest/v1/rpc/recent_posts` on REST, taking one `int` argument and answering
with `post` rows. Table syntax, closed sets, and defaults belong to
[tables](tables.md); this page owns everything after the `fn` keyword.

## Polarity

An unmarked `fn` is a **read**; `mut fn` may write. The unmarked form is the
read because the forgotten-`mut` failure is the one you want to see: the
engine checks every statement of an unmarked fn for read-onlyness at load,
and a statement that may write aborts startup naming the fn and the
statement — `spock check` on such a program fails with
`fn 'touch': the statement may write, but the fn is not marked 'mut'`.
Read-onlyness is engine-enforced, never asserted. `mut` grants permission to
write; it does not require writes.

Polarity decides where the function lands and how it runs:

| Polarity | GraphQL root | `/rest/v1/rpc/<fn>` | Transaction |
| --- | --- | --- | --- |
| `fn` (read) | `Query` | `GET` and `POST` | `DEFERRED` — never takes the write lock |
| `mut fn` | `Mutation` | `POST` only; `GET` answers 405 | `IMMEDIATE` |

One call is one transaction spanning every statement of the body
(v0 specification, §7.4; GraphQL dialect specification, §5.1).

## The signature and the escapes

A function splits cleanly in two. The **signature** — name, typed parameters,
return shape, and the `!` error clause — is language-owned contract. The
**body** is a sequence of **escapes**: each `unchecked sql("...")` carries
exactly one SQL statement, verbatim, into the contract. The `unchecked`
marker states the true thing: the checker does not verify what this SQL
means. The escape may replace the body, never the contract.

The engine does validate every body at load — a broken body aborts startup,
never a request:

- each escape is exactly one statement, and every statement is a row
  statement (`SELECT`, `INSERT`, `UPDATE`, `DELETE`, `REPLACE`, `WITH`,
  `VALUES`) — a fn body reads and writes rows; engine state is the engine's;
- SQL syntax and table/column resolution are checked by the engine itself,
  and its message surfaces verbatim;
- placeholders are `:param` only, matched against the signature in both
  directions: a placeholder that is not a declared parameter is an error,
  and so is a parameter no statement uses;
- the last statement's result columns must equal the declared return shape's
  fields, by name.

`spock check` runs this same load proof without starting a server, and its
summary reports the escape count — `(3 unchecked escapes)` — the ledger of
what the language did not verify.

## Statement sequences

Statements execute in declaration order inside the one transaction. The
**last** statement produces the return value; earlier statements stage or
guard, and their result rows are discarded. A statement that answers with
DML uses `RETURNING`:

```spock
fn non_negative(value: int) -> bool {
  unchecked sql("SELECT :value >= 0")
}

table event {
  key id: uuid = auto
  name: text
  seats_left: int check non_negative
}

table attendee {
  key id: uuid = auto
  event: event
  name: text
}

mut fn register(event: uuid, name: text) -> attendee
  ! event_seats_left_invalid | bad_request {
  unchecked sql("""
    UPDATE event SET seats_left = seats_left - 1
    WHERE id = :event
  """)
  unchecked sql("""
    INSERT INTO attendee (event, name)
    VALUES (:event, :name)
    RETURNING *
  """)
}
```

The first statement is a guard that never answers: it spends a seat, and when
the count would go negative, the `check` constraint trips the derived error
`event_seats_left_invalid` and the whole transaction rolls back — no attendee
row survives. The second statement is the answer. When the event id does not
exist, the `UPDATE` matches nothing and the `INSERT` trips the foreign key
instead, surfacing as `bad_request`. Both outcomes appear in the `!` clause,
so clients can read the failure surface off the contract.

A body that wants to answer with a row an earlier statement touched re-reads
it in the same transaction; one extra `SELECT` is the price of a flat grammar.

## Return shapes and arity

The return shape is a declared table, a declared record, or a builtin scalar,
in three arities: `t` (exactly one), `t?` (zero or one), `[t]` (any number).
Arity is enforced on the last statement's rows **before commit** — a violated
arity rolls the whole transaction back, earlier effects included
(v0 specification, §7.4):

| Declared | Rows | Result |
| --- | --- | --- |
| `-> t` | 1 | the row |
| `-> t` | 0 | error `not_found` — a write that did not happen must shout |
| `-> t` | 2 or more | error `internal` — the body broke its contract |
| `-> t?` | 0 / 1 | `null` / the row |
| `-> t?` | 2 or more | error `internal` |
| `-> [t]` | any | the rows — uncapped; the author owns `LIMIT` |

The floor's list page cap governs derived lists, not authored SQL: a `[t]`
function returns whatever its statement selects, which is why
`recent_posts` writes its own `LIMIT`.

Result columns must match the declared shape's fields by name — row mapping
is by name, and duplicates are rejected at load. A scalar return (`-> int`
and friends) requires exactly one result column instead, its name irrelevant,
and answers with bare values: `-> int` is a number, `-> [text]` an array of
strings.

## Records

A `record` declares a named return shape for projections that match no single
table — its fields are builtin scalars, and it exists only as a return shape,
never as a parameter type and never as storage:

```spock
table author {
  key id: uuid = auto
  username: text unique
  display_name: text
}

table post {
  key id: uuid = auto
  author: author on delete cascade
  body: text
  at: timestamp = now
}

record post_card {
  id: uuid
  body: text
  at: timestamp
  author_name: text
}

fn feed(count: int) -> [post_card] {
  unchecked sql("""
    SELECT post.id AS id,
           post.body AS body,
           post.at AS at,
           author.display_name AS author_name
    FROM post
    JOIN author ON author.id = post.author
    ORDER BY post.at DESC
    LIMIT :count
  """)
}
```

The `AS` aliases are what satisfy the by-name column matching; the record
registers as an object type on the GraphQL surface under its bare name.

## The NULL law

A SQL `NULL` cannot smuggle itself under a non-optional type. When a required
result column comes back `NULL` — a `LEFT JOIN` miss, an aggregate over zero
rows — the call fails as `internal` **before commit**: column names are
validated at load, but nullability is only knowable per row, and a `null`
under a non-nullable type would make the GraphQL binding violate its own
schema. The same law covers un-suffixed scalar returns and every element of a
`[t]`. Declare `t?`, or the record field optional, wherever null is honestly
possible; for `-> t?` a missing row and a `NULL` value are both `null`,
indistinguishable by design.

## Error routing

A constraint the SQL trips routes to the owning table's **derived error** —
cross-table, with no write-set declaration, because the engine's constraint
message names the table and column. `UNIQUE` and `PRIMARY KEY` violations map
to the table's `taken` and `already_exists` codes, `NOT NULL` to the derived
`required` code, `CHECK` to the derived `invalid` code. A foreign-key
violation maps to the **reserved code** `bad_request`: the engine reports no
table detail for FK failures, and Spock reports truth over guesses.

The `! code | code` clause is introspection metadata for clients — it travels
in the contract and in the GraphQL field description as the declared failure
surface. Listing a code does not change which mechanism can produce it, and a
code the body can produce but the signature does not declare still surfaces
truthfully at runtime. The full code tables live in the
[errors reference](../reference/errors.md); the wire envelopes belong to
[the derived API](derived-api.md).

## Product errors and refusals

> **Experimental — RFD 0024 implementation preview.** Top-level `error`
> declarations ship in toolchain 0.5.2 and later as an implementation
> preview. [RFD 0024](../rfd/0024-error-declarations.md) remains a draft;
> this syntax is not yet part of the normative v0 specification.

Derived errors are evidence of schema constraints. A **product error** is a
rule of your product with no constraint behind it — "you cannot follow
yourself" — and it gets a top-level declaration that owns the code once,
program-wide, so several functions can share it:

```spock
auth table user {
  key id: uuid = auto
  username: text unique
  private: bool = false
}

table follow {
  key (follower, followed)
  follower: user on delete cascade
  followed: user on delete cascade
  at: timestamp = now
}

/// The actor cannot follow themselves.
error cannot_follow_self

/// The target account only accepts approved followers.
error account_private

mut fn follow_user(target: user) -> follow
  ! cannot_follow_self | account_private {
  unchecked sql("""
    SELECT spock_refuse('cannot_follow_self')
    WHERE spock_actor() = :target
  """)
  unchecked sql("""
    SELECT spock_refuse('account_private')
    WHERE EXISTS (SELECT 1 FROM user WHERE id = :target AND private)
  """)
  unchecked sql("""
    INSERT INTO follow (follower, followed)
    VALUES (spock_actor(), :target)
    RETURNING *
  """)
}
```

The `///` doc comment on a declaration is carried into the contract as the
code's documentation — visible to every client and generator, ahead of any
call. `spock_actor()` is the current actor's key, bound before the
transaction opens; the [actor seam](actor.md) owns how it gets there.

A fn's `!` clause resolves against three populations: declared product
errors, schema-derived codes, and the five reserved codes the function plane
can emit (`not_found`, `type_mismatch`, `unknown_field`, `bad_request`,
`internal`). Three diagnostics police the namespace:

- **E051** — two `error` declarations use the same identifier;
- **E052** — a `!` code resolves to none of the three populations;
- **E053** — a declaration collides with a derived or reserved code, which
  authored code cannot impersonate.

There is no implicit minting: an unknown identifier in `!` is a diagnostic,
not a new code —

```spock check=fail:E052
table post {
  key id: uuid = auto
  body: text
}

mut fn hide_post(id: uuid) -> post ! post_hidden {
  unchecked sql("""
    UPDATE post SET body = '[hidden]'
    WHERE id = :id
    RETURNING *
  """)
}
```

— and the diagnostic names the fix: declare it at top level with
`error post_hidden`.

The raise channel is the engine builtin `spock_refuse('code')`, which always
errors when evaluated, so the guard shape is a conditional selection:
`SELECT spock_refuse('code') WHERE <condition>`. Zero rows selected, nothing
raised. A raised code routes as a **refusal** only when this fn lists it in
`!`: kind `refused`, REST status 409, GraphQL's usual HTTP 200 with the code
in the error extensions. Raising an unlisted code is `internal`, before
commit — an unlisted code is the body breaking its signature, and derived or
reserved codes can only ever be produced by their own mechanisms; if bodies
could raise `user_username_taken` by hand, derived errors would stop being
evidence. A refusal raised mid-body rolls the whole transaction back like any
other failure. Read fns refuse too: `SELECT spock_refuse(...)` is a read-only
statement, so a `fn` may guard visibility the same way a `mut fn` guards a
write.

## Calling

`spock check` proves the program, `spock dev` serves it, and every function
answers on `/rest/v1/rpc/<fn>` and its GraphQL root — the endpoint tables
live in the [HTTP reference](../reference/http.md) and the commands in the
[CLI reference](../reference/cli.md).
