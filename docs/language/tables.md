---
description: "Declaring durable truth in Spock: tables, builtin types, keys, defaults, uniqueness, references, closed sets, and validator checks."
order: 1
---

# Tables, types, and defaults

A table declares durable truth — the data the authority owns and answers for.
Everything else in a Spock program exists in relation to tables: functions
read and guard them, the derived API exposes them, seed fills them. This page
covers the whole table surface: fields and types, keys, optionality, defaults,
uniqueness, references, closed-set types, and validator checks.

## What a table is

Each `table` compiles to exactly one SQLite table, with declaration order
preserved: columns appear in the order you wrote the fields (v0
specification, §7.1). The database file is plain SQL that any SQLite
reader can open: types, keys, uniques, checks, and foreign keys all land
as ordinary DDL. The database is rebuilt from source and seed on every
start; [Seed and disposable state](seed.md) covers that lifecycle.

```spock
table user {
  key id: uuid = auto
  username: text unique
}

table post {
  key id: uuid = auto
  author: user on delete cascade
  slug: text
  body: text
  published_at: timestamp?
  unique (author, slug)
}
```

This is a complete program. `spock check` compiles it, and `spock run`
serves a working API from it — the floor described in
[The derived API](derived-api.md).

## Fields and builtin types

A field is `name: type` plus optional modifiers, in a fixed order:

```text
[key] name: type [?] [check fn] [unique] [= default]
      [on delete restrict|cascade|set null]
```

Identifiers are lowercase `snake_case`. There are six builtin types
(v0 specification, §5.1):

| Type | JSON on the wire | SQLite storage | Notes |
| --- | --- | --- | --- |
| `text` | string | `TEXT` | |
| `int` | number (integer) | `INTEGER` | signed 64-bit |
| `float` | number | `REAL` | IEEE 754 double; NaN and infinity have no JSON spelling and render as `null` |
| `bool` | boolean | `INTEGER` (0/1) | converted at the edge |
| `timestamp` | string, RFC 3339 | `TEXT` | stored canonically in UTC with fixed six-digit fractional seconds, so text order is chronological order; inputs in any RFC 3339 offset are canonicalized on write |
| `uuid` | string, hyphenated | `TEXT` | stored lowercase-canonical |

A type that names another table is a reference — covered below. A type
that is a set of string literals is a closed set — also below.

## Keys

Every table declares exactly one key. A single-field key is the inline
modifier; a multi-field key is the composite form:

```spock
table session {
  key token: uuid = auto
  label: text
}

table membership {
  key (org, member)
  org: text
  member: text
}
```

The key is the row's identity: it names the row in by-key reads, it is
what references store, and it is immutable — an update can never touch a
key field, because changing the identity would be a different row. Key
fields cannot be optional. A table with no key is diagnostic E005; a
table with two is E006:

```spock check=fail:E006
table user {
  key id: uuid = auto
  key email: text
}
```

## Optionality

`?` after the type makes a field optional. Optionality is absence, not a
value: an absent optional field is JSON `null` on the wire and SQL `NULL`
in storage, and providing `null` for a required field is the same
validation error as omitting it. There is no distinction to learn between
"not set" and "set to null" — they are one state.

The single carve-out is update inputs, where a change set must be able to
say both "keep this field" and "clear this field";
[The derived API](derived-api.md) covers that write semantics.

## Defaults

A default is `= value` after the type, and it fills the field when an
insert omits it:

```spock
table article {
  key id: uuid = auto
  title: text
  status: "draft" | "published" | "archived" = "draft"
  pinned: bool = false
  created_at: timestamp = now
}
```

Three kinds of default exist (v0 specification, §5.2):

- **`auto`**, on `uuid` fields: the runtime generates a UUIDv7 at insert.
  UUIDv7 is time-ordered, so key order roughly tracks insert order.
- **`now`**, on `timestamp` fields: the runtime captures UTC now at insert.
- **A literal**, on `text`, `int`, `float`, and `bool` fields — and on a
  closed-set field, where the literal must be one of the set's values.

Defaults apply at insert only; an update never applies them, because an
update edits a row that already has every value. `auto` and `now` are also
installed as engine `DEFAULT` clauses backed by the same generators the
runtime uses, so SQL written in function bodies receives identical values
and the two paths cannot drift.

The fourth default, `= me`, stamps the current actor into a reference to
the program's `auth` table at insert; it belongs to the identity seam and
is covered in [Identity: the actor seam](actor.md).

## Unique fields and groups

`unique` on a field constrains that one column; `unique (a, b)` as a
table item constrains the combination. The `user` and `post` tables above
show both: every `username` is distinct, and each author can use a `slug`
once while different authors can share one.

Rows in which an optional unique field is absent never conflict with each
other. Absence is not a value, so two absences are not the same value —
this is the standard SQL `UNIQUE` treatment of `NULL`, and Spock keeps it.

## References

A field whose type is another table's name is a reference: it holds the
key of a row in that table. In the program at the top of this page,
`author: user` stores a `user` key, and the runtime rejects any write
whose value does not name an existing row. References can only target
tables with a single-field key — there is no composite-key reference
target in v0 (diagnostic E010). A table can reference itself: a threaded
`comment` table carries `reply_to: comment?`.

`on delete` declares what happens to the referencing row when the target
row is deleted:

- **`restrict`** — the default: the delete is refused while referencing
  rows exist.
- **`cascade`** — deleting the target deletes the referencing rows.
- **`set null`** — deleting the target clears the reference field, which
  therefore must be optional; a required field has no cleared state to
  take:

```spock check=fail:E040
table user {
  key id: uuid = auto
  username: text unique
}

table post {
  key id: uuid = auto
  author: user on delete set null
  body: text
}
```

Change `author` to `user?` and the program compiles.

### Join tables

A reference field can be a member of a composite key, and that combination
is the join-table pattern — many-to-many association without an artificial
surrogate id:

```spock
table user {
  key id: uuid = auto
  username: text unique
}

table follow {
  key (follower, followed)
  follower: user on delete cascade
  followed: user on delete cascade
  at: timestamp = now
}
```

The composite key makes each pairing exist at most once, and `cascade` on
both sides means deleting a user dissolves their follow edges instead of
stranding them.

## Closed-set types

A type written as string literals joined by `|` is a closed set: the
field's value is one of the listed strings, checked at every layer.

```spock
table ticket {
  key id: uuid = auto
  title: text
  priority: "low" | "normal" | "urgent" = "normal"
}
```

Storage is `TEXT` plus a named `CHECK` constraint — the constraint is
named `<table>_<field>_invalid`, and that name is verbatim the derived
error code a violating write receives, so the database itself routes the
failure. A set needs at least two distinct, non-empty values.

A closed set is a stored table-field type only in v0: it cannot be a key
field, a `record` field, or a `fn` parameter. The set's members also bound
its defaults and seed values — `= "normal"` compiles because `"normal"`
is a member.

## Validator check functions

For rules beyond membership in a set, a field or a group of fields can
name a validator function:

```spock
fn nonempty(value: text) -> bool {
  unchecked sql("SELECT length(trim(:value)) > 0")
}

fn valid_range(low: int, high: int) -> bool {
  unchecked sql("SELECT :low <= :high")
}

table booking {
  key id: uuid = auto
  guest_name: text check nonempty
  starts_at: int
  ends_at: int
  check (starts_at, ends_at) valid_range
}
```

The field form is `name: type check fn`; the row form is
`check (a, b) fn` as a table item, mirroring the shape of a unique group.
Parameters bind to the checked fields positionally, so `valid_range`
receives `starts_at` and `ends_at` in that order.

A validator is an unmarked (read) `fn` returning `bool` from a single
clauseless `SELECT`: one deterministic boolean expression over its
parameters, with no `FROM`, no `WHERE`, and no subquery. The laws are
strict because of what the compiler does with the body — it inline-expands
into a SQL `CHECK` constraint, each `:param` replaced by its bound column,
named `<table>_<fields>_invalid` like a closed set's constraint. The
database file stays pure SQL, and the rule is enforced by the engine for
every write path, including SQL in function bodies that bypasses the
floor's validation.

A validator is also an ordinary read function: it appears on the derived
Query surface like any other `fn`, so a client can call `nonempty` or
`valid_range` directly and pre-validate a form against the exact rule the
database will enforce. A `check` cannot attach to a closed-set field,
which already validates itself, or to an `auto`/`now`-defaulted field,
whose value the runtime mints.

## Every constraint is API

Each construct on this page mints a derived error with a stable code —
`user_username_taken` from a unique field, `post_author_slug_taken` from
the group, `post_author_not_found` from the reference,
`ticket_priority_invalid` from the closed set,
`booking_starts_at_ends_at_invalid` from the row check — and shapes the
derived read and write surface every table receives. [The derived
API](derived-api.md) walks that surface, error vocabulary and all.
